//! The byte-level and message entry points: request checks, command dispatch, events, panic
//! containment and poisoned documents (engine.md sections 3.1, 3.4 and 3.15).

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use partsmith_model::Units;
use partsmith_protocol::{
    AssetData, Buffer, BufferIndex, Command, CommandResult, DocHandle, ErrorCode, ExportFile,
    FrameError, MAX_SAFE_INTEGER, Outcome, Request, Response, decode_request_frame,
    encode_response_frame, error_response_frame,
};

use crate::error::EngineError;
use crate::events::{Output, sketch_display};
use crate::session::Session;
use crate::sketch::Solved;
use crate::types::{Asset, ExportRequest, PrintRequest, Reply};

/// Why a command failed.
enum Failure {
    /// The command failed and changed nothing.
    Command(EngineError),
    /// The command changed its document, then its events could not be produced.
    Events(EngineError),
}

impl From<EngineError> for Failure {
    fn from(error: EngineError) -> Failure {
        Failure::Command(error)
    }
}

impl Session {
    /// Handles one request frame and returns one response frame; never panics.
    pub fn call(&mut self, request_frame: &[u8]) -> Vec<u8> {
        let (request, buffers) = match decode_request_frame(request_frame) {
            Ok(decoded) => decoded,
            Err(error) => {
                let message = match &error {
                    FrameError::InvalidMessage { message, .. } => EngineError::MalformedRequest {
                        detail: message.clone(),
                    }
                    .to_string(),
                    other => other.to_string(),
                };
                return error_response_frame(
                    error.request_id().unwrap_or(0),
                    error.code(),
                    &message,
                );
            }
        };
        let reply = self.execute(request, buffers);
        encode_response_frame(&reply.events, &reply.response, &reply.buffers).unwrap_or_else(
            |error| {
                let message = EngineError::unexpected(error).to_string();
                error_response_frame(reply.response.id, ErrorCode::InternalError, &message)
            },
        )
    }

    /// Handles one decoded request; never panics.
    pub fn execute(&mut self, request: Request, buffers: Vec<Buffer>) -> Reply {
        let reply = self.reply(request, buffers);
        self.cache.trim();
        reply
    }

    /// The reply to one request.
    fn reply(&mut self, request: Request, buffers: Vec<Buffer>) -> Reply {
        if request.id > MAX_SAFE_INTEGER {
            return error_reply(
                0,
                &EngineError::MalformedRequest {
                    detail: format!(
                        "The request id {} is larger than {MAX_SAFE_INTEGER}.",
                        request.id
                    ),
                },
            );
        }
        if let Err(error) = check_buffers(&request.command, &buffers) {
            return error_reply(request.id, &error);
        }
        let doc = document_of(&request.command);
        if let Some(doc) = doc {
            match self.documents.get(&doc) {
                None => return error_reply(request.id, &EngineError::UnknownDocument { doc }),
                Some(open) if open.poisoned.is_some() && !rescues(&request.command) => {
                    return error_reply(request.id, &EngineError::DocumentPoisoned { doc });
                }
                Some(_) => {}
            }
        }
        match self.contained(doc, |session| session.run(request.command, &buffers)) {
            Ok((result, output)) => Reply {
                events: output.events,
                response: Response {
                    id: request.id,
                    result: Outcome::Ok(result),
                },
                buffers: output.buffers,
            },
            Err(error) => error_reply(request.id, &error),
        }
    }

    /// Runs one command under panic containment (section 3.15). A panic becomes an internal
    /// error, and a panic or a failure after the command changed its document poisons the
    /// document `doc`, keeping the recipe it had before the command.
    fn contained<T>(
        &mut self,
        doc: Option<DocHandle>,
        command: impl FnOnce(&mut Session) -> Result<T, Failure>,
    ) -> Result<T, EngineError> {
        let snapshot = doc
            .and_then(|doc| self.documents.get(&doc))
            .map(|open| Arc::clone(&open.document));
        match catch_unwind(AssertUnwindSafe(|| command(self))) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(Failure::Command(error))) => Err(error),
            Ok(Err(Failure::Events(error))) => {
                self.poison(doc, snapshot);
                Err(error)
            }
            Err(panic) => {
                self.poison(doc, snapshot);
                Err(EngineError::internal(panic_message(panic.as_ref())))
            }
        }
    }

    /// Marks a document poisoned, keeping the recipe as of its last successful command.
    fn poison(&mut self, doc: Option<DocHandle>, snapshot: Option<Arc<partsmith_model::Document>>) {
        if let (Some(doc), Some(snapshot)) = (doc, snapshot)
            && let Some(open) = self.documents.get_mut(&doc)
        {
            open.poisoned.get_or_insert(snapshot);
        }
    }

    /// Runs one command through the typed API and collects its events and buffers.
    fn run(
        &mut self,
        command: Command,
        buffers: &[Buffer],
    ) -> Result<(CommandResult, Output), Failure> {
        let mut output = Output::default();
        let result = match command {
            Command::EngineInfo => {
                let info = self.engine_info();
                CommandResult::EngineInfo {
                    protocol_version: info.protocol_version,
                    engine_version: info.engine_version,
                    format_version: info.format_version,
                    kernel: info.kernel,
                }
            }
            Command::FeatureCatalog => CommandResult::FeatureCatalog {
                features: self.feature_catalog(),
            },
            Command::ListPrinterProfiles => CommandResult::ListPrinterProfiles {
                profiles: self.printer_profiles(),
            },
            Command::PutPrinterProfile { profile } => {
                for doc in self.put_profile(profile)? {
                    self.events(doc, &mut output)?;
                }
                CommandResult::PutPrinterProfile
            }
            Command::NewDocument { units, printer } => {
                let doc = self.new_document(units.unwrap_or(Units::Mm), printer)?;
                self.events(doc, &mut output)?;
                CommandResult::NewDocument { doc }
            }
            Command::OpenDocument { text, assets } => {
                let assets = assets
                    .into_iter()
                    .map(|asset| Asset {
                        bytes: bytes_of(buffers, asset.data),
                        name: asset.name,
                    })
                    .collect();
                let opened = self.open_document(&text, assets)?;
                self.events(opened.doc, &mut output)?;
                CommandResult::OpenDocument {
                    doc: opened.doc,
                    migrated_from: opened.migrated_from,
                }
            }
            Command::SaveDocument { doc } => {
                let saved = self.save_document(doc)?;
                let assets = saved
                    .assets
                    .into_iter()
                    .map(|asset| AssetData {
                        name: asset.name,
                        data: output.buffer(Buffer::U8(asset.bytes)),
                    })
                    .collect();
                CommandResult::SaveDocument {
                    text: saved.text,
                    assets,
                    revision: saved.revision,
                }
            }
            Command::CloseDocument { doc } => {
                self.close_document(doc)?;
                CommandResult::CloseDocument
            }
            Command::GetDocument { doc } => CommandResult::GetDocument {
                revision: self.revision(doc)?,
                document: self.document(doc)?.clone(),
            },
            Command::MarkSaved { doc, revision } => {
                self.mark_saved(doc, revision)?;
                self.events(doc, &mut output)?;
                CommandResult::MarkSaved
            }
            Command::ApplyEdits { doc, label, edits } => {
                let revision = self.apply_edits(doc, label.as_deref(), edits)?;
                self.events(doc, &mut output)?;
                CommandResult::ApplyEdits { revision }
            }
            Command::RenameParameter {
                doc,
                parameter,
                name,
            } => {
                let revision = self.rename_parameter(doc, &parameter, &name)?;
                self.events(doc, &mut output)?;
                CommandResult::RenameParameter { revision }
            }
            // No diagnostic offers an `apply` action yet, so every fix names an action its
            // feature does not have (section 3.22).
            Command::ApplyFix { .. } => return Err(EngineError::StaleFix.into()),
            Command::BeginTransaction { doc, label } => {
                self.begin_transaction(doc, &label)?;
                self.events(doc, &mut output)?;
                CommandResult::BeginTransaction
            }
            Command::SetFocus { doc, feature } => {
                self.set_focus(doc, feature.as_ref())?;
                self.events(doc, &mut output)?;
                CommandResult::SetFocus
            }
            Command::CommitTransaction { doc } => {
                let committed = self.commit_transaction(doc)?;
                self.events(doc, &mut output)?;
                CommandResult::CommitTransaction {
                    revision: committed.revision,
                    recorded: committed.recorded,
                }
            }
            Command::CancelTransaction { doc } => {
                let revision = self.cancel_transaction(doc)?;
                self.events(doc, &mut output)?;
                CommandResult::CancelTransaction { revision }
            }
            Command::Undo { doc } => {
                let stepped = self.undo(doc)?;
                self.events(doc, &mut output)?;
                CommandResult::Undo {
                    revision: stepped.revision,
                    label: stepped.label,
                }
            }
            Command::Redo { doc } => {
                let stepped = self.redo(doc)?;
                self.events(doc, &mut output)?;
                CommandResult::Redo {
                    revision: stepped.revision,
                    label: stepped.label,
                }
            }
            Command::SolveSketch {
                doc,
                feature,
                candidate,
            } => {
                let solved = self.solve_sketch_command(doc, &feature, candidate)?;
                CommandResult::SolveSketch {
                    solution: self.with_display(solved, &mut output),
                }
            }
            Command::DragSketch {
                doc,
                feature,
                point,
                to,
            } => {
                let solved = self.drag_sketch_command(doc, &feature, &point, to)?;
                CommandResult::DragSketch {
                    solution: self.with_display(solved, &mut output),
                }
            }
            Command::EndSketchDrag {
                doc,
                feature,
                commit,
            } => {
                let revision = self.end_sketch_drag(doc, &feature, commit)?;
                self.events(doc, &mut output)?;
                CommandResult::EndSketchDrag { revision }
            }
            Command::SetTessellation { doc, quality } => {
                let tolerance = self.set_tessellation(doc, quality)?;
                self.events(doc, &mut output)?;
                CommandResult::SetTessellation { tolerance }
            }
            Command::Evaluate { doc, expr } => {
                let quantity = self.evaluate(doc, &expr)?;
                CommandResult::Evaluate {
                    value: quantity.value,
                    dimension: quantity.dimension,
                }
            }
            Command::ListElements { doc, body, kind } => CommandResult::ListElements {
                elements: self.list_elements(doc, &body, kind)?,
            },
            Command::PickReference { doc, pick } => CommandResult::PickReference {
                element: Box::new(self.pick_reference(doc, pick)?),
            },
            Command::ResolveReferences { doc, references } => CommandResult::ResolveReferences {
                results: self.resolve_references(doc, &references)?,
            },
            Command::Measure { doc, targets } => {
                CommandResult::Measure(self.measure(doc, &targets)?)
            }
            Command::AnalyzePrint {
                doc,
                profile,
                orientation,
                bodies,
            } => {
                let request = PrintRequest {
                    profile,
                    orientation,
                    bodies,
                };
                CommandResult::AnalyzePrint {
                    health: self.analyze_print_command(doc, &request, &mut output)?,
                }
            }
            Command::Export {
                doc,
                format,
                bodies,
                tolerance,
                orientation,
            } => {
                let exported = self.export(
                    doc,
                    &ExportRequest {
                        format,
                        bodies,
                        tolerance,
                        orientation,
                    },
                )?;
                let files = exported
                    .files
                    .into_iter()
                    .map(|file| ExportFile {
                        name: file.name,
                        media_type: file.media_type,
                        data: output.buffer(Buffer::U8(file.bytes)),
                    })
                    .collect();
                CommandResult::Export {
                    files,
                    warnings: exported.warnings,
                }
            }
            Command::ImportStep { doc, data, name } => {
                let imported = self.import_step(doc, bytes_of(buffers, data), name.as_deref())?;
                self.events(doc, &mut output)?;
                CommandResult::ImportStep {
                    feature: imported.feature,
                    body: imported.body,
                }
            }
            Command::ExportPython { doc } => CommandResult::ExportPython {
                code: self.export_python(doc)?,
            },
        };
        Ok((result, output))
    }

    /// Collects a document's events; a failure here comes after the command changed it.
    fn events(&mut self, doc: DocHandle, output: &mut Output) -> Result<(), Failure> {
        self.collect_events(doc, output).map_err(Failure::Events)
    }

    /// A solution with its region fill buffers in display sessions.
    fn with_display(
        &self,
        solved: Solved,
        output: &mut Output,
    ) -> partsmith_protocol::SketchSolution {
        let mut solution = solved.solution;
        if self.display {
            solution.display = Some(sketch_display(&solved.regions, output));
        }
        solution
    }
}

/// The reply to a failed request: no events, one error response, no buffers.
fn error_reply(id: u64, error: &EngineError) -> Reply {
    Reply {
        events: Vec::new(),
        response: Response {
            id,
            result: Outcome::Error(error.to_info()),
        },
        buffers: Vec::new(),
    }
}

/// Checks that every buffer a command names exists and holds bytes (section 3.1).
fn check_buffers(command: &Command, buffers: &[Buffer]) -> Result<(), EngineError> {
    let referenced: Vec<BufferIndex> = match command {
        Command::OpenDocument { assets, .. } => assets.iter().map(|asset| asset.data).collect(),
        Command::ImportStep { data, .. } => vec![*data],
        _ => Vec::new(),
    };
    for index in referenced {
        match buffers.get(index.0 as usize) {
            None => {
                return Err(EngineError::MalformedRequest {
                    detail: format!("Buffer {} does not exist.", index.0),
                });
            }
            Some(Buffer::U8(_)) => {}
            Some(_) => {
                return Err(EngineError::MalformedRequest {
                    detail: format!("Buffer {} must be u8.", index.0),
                });
            }
        }
    }
    Ok(())
}

/// The bytes of a checked `u8` buffer.
fn bytes_of(buffers: &[Buffer], index: BufferIndex) -> Vec<u8> {
    buffers
        .get(index.0 as usize)
        .and_then(Buffer::as_u8)
        .map(<[u8]>::to_vec)
        .unwrap_or_default()
}

/// The document a command names, if any.
fn document_of(command: &Command) -> Option<DocHandle> {
    match command {
        Command::EngineInfo
        | Command::FeatureCatalog
        | Command::ListPrinterProfiles
        | Command::PutPrinterProfile { .. }
        | Command::NewDocument { .. }
        | Command::OpenDocument { .. } => None,
        Command::SaveDocument { doc }
        | Command::CloseDocument { doc }
        | Command::GetDocument { doc }
        | Command::MarkSaved { doc, .. }
        | Command::ApplyEdits { doc, .. }
        | Command::RenameParameter { doc, .. }
        | Command::ApplyFix { doc, .. }
        | Command::BeginTransaction { doc, .. }
        | Command::SetFocus { doc, .. }
        | Command::CommitTransaction { doc }
        | Command::CancelTransaction { doc }
        | Command::Undo { doc }
        | Command::Redo { doc }
        | Command::SolveSketch { doc, .. }
        | Command::DragSketch { doc, .. }
        | Command::EndSketchDrag { doc, .. }
        | Command::SetTessellation { doc, .. }
        | Command::Evaluate { doc, .. }
        | Command::ListElements { doc, .. }
        | Command::PickReference { doc, .. }
        | Command::ResolveReferences { doc, .. }
        | Command::Measure { doc, .. }
        | Command::AnalyzePrint { doc, .. }
        | Command::Export { doc, .. }
        | Command::ImportStep { doc, .. }
        | Command::ExportPython { doc } => Some(*doc),
    }
}

/// True for the commands a poisoned document still accepts (section 3.15).
fn rescues(command: &Command) -> bool {
    matches!(
        command,
        Command::SaveDocument { .. } | Command::MarkSaved { .. } | Command::CloseDocument { .. }
    )
}

/// The message of a panic payload.
fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic without a message".to_string())
}

#[cfg(test)]
mod tests {
    use partsmith_model::{Edit, Parameter};
    use partsmith_protocol::{ErrorInfo, FixRef};

    use super::*;
    use crate::test_support::{box_session, id};

    /// The edit that sets the box's `height` parameter.
    fn set_height(expr: &str) -> Edit {
        Edit::UpdateParameter {
            parameter: Parameter::new(id("p-h"), "height", expr),
        }
    }

    /// The error of a reply, or `None` when it succeeded.
    fn error_info(session: &mut Session, command: Command) -> Option<ErrorInfo> {
        let reply = session.execute(Request { id: 1, command }, Vec::new());
        match reply.response.result {
            Outcome::Ok(_) => None,
            Outcome::Error(info) => Some(info),
        }
    }

    /// The error code of a reply, or `None` when it succeeded.
    fn error_code(session: &mut Session, command: Command) -> Option<ErrorCode> {
        error_info(session, command).map(|info| info.code)
    }

    #[test]
    fn a_panic_is_an_internal_error_that_poisons_its_document() {
        let (mut session, doc) = box_session(false);

        let result: Result<(), EngineError> =
            session.contained(Some(doc), |_| panic!("the kernel fell over"));

        assert_eq!(
            result,
            Err(EngineError::internal("the kernel fell over".to_string()))
        );
        assert_eq!(
            error_code(&mut session, Command::Undo { doc }),
            Some(ErrorCode::DocumentPoisoned)
        );
        assert_eq!(
            error_code(&mut session, Command::GetDocument { doc }),
            Some(ErrorCode::DocumentPoisoned)
        );
    }

    #[test]
    fn a_poisoned_document_saves_its_recipe_from_before_the_failed_command() {
        let (mut session, doc) = box_session(false);
        session
            .begin_transaction(doc, "Resize")
            .expect("a transaction opens");
        session
            .apply_edits(doc, None, vec![set_height("50 mm")])
            .expect("the edit applies");

        let result: Result<(), EngineError> = session.contained(Some(doc), |session| {
            session
                .apply_edits(doc, None, vec![set_height("60 mm")])
                .expect("the edit applies");
            panic!("after the edit")
        });
        let saved = session
            .save_document(doc)
            .expect("a poisoned document saves");

        assert!(result.is_err());
        assert!(saved.text.contains("\"50 mm\""), "{}", saved.text);
        assert!(!saved.text.contains("\"60 mm\""), "{}", saved.text);
        assert_eq!(
            error_code(
                &mut session,
                Command::MarkSaved {
                    doc,
                    revision: saved.revision
                }
            ),
            None
        );
        assert_eq!(
            error_code(&mut session, Command::CloseDocument { doc }),
            None
        );
    }

    #[test]
    fn a_failure_after_the_document_changed_poisons_it() {
        let (mut session, doc) = box_session(false);

        let result: Result<(), EngineError> = session.contained(Some(doc), |_| {
            Err(Failure::Events(EngineError::internal("no events")))
        });

        assert_eq!(result, Err(EngineError::internal("no events")));
        assert_eq!(
            error_code(&mut session, Command::Undo { doc }),
            Some(ErrorCode::DocumentPoisoned)
        );
    }

    #[test]
    fn apply_fix_is_stale_while_no_diagnostic_offers_an_apply_action() {
        let (mut session, doc) = box_session(false);
        let fix = FixRef {
            feature: id("f-box1"),
            code: "fillet_too_large".to_string(),
            field: Some("/radius".to_string()),
            cause: false,
            index: 0,
            revision: session.revision(doc).expect("the document is open"),
        };

        let info = error_info(&mut session, Command::ApplyFix { doc, fix })
            .expect("a fix no diagnostic offers fails");

        assert_eq!(info.code, ErrorCode::StaleFix);
        assert_eq!(
            info.message,
            "This fix no longer applies because the document changed after it was offered. Look at the problem again."
        );
    }

    #[test]
    fn a_failed_command_does_not_poison_its_document() {
        let (mut session, doc) = box_session(false);

        let result: Result<(), EngineError> = session.contained(Some(doc), |_| {
            Err(Failure::Command(EngineError::NothingToUndo))
        });

        assert_eq!(result, Err(EngineError::NothingToUndo));
        assert_eq!(error_code(&mut session, Command::GetDocument { doc }), None);
    }

    #[test]
    fn missing_buffers_are_malformed() {
        let command = Command::ImportStep {
            doc: DocHandle(1),
            data: BufferIndex(3),
            name: None,
        };

        let error = check_buffers(&command, &[Buffer::U8(Vec::new())]).expect_err("no buffer 3");

        assert_eq!(
            error.to_string(),
            "The request is not valid: Buffer 3 does not exist."
        );
    }

    #[test]
    fn buffers_must_hold_bytes() {
        let command = Command::ImportStep {
            doc: DocHandle(1),
            data: BufferIndex(0),
            name: None,
        };

        let error = check_buffers(&command, &[Buffer::F32(Vec::new())]).expect_err("not bytes");

        assert_eq!(
            error.to_string(),
            "The request is not valid: Buffer 0 must be u8."
        );
    }

    #[test]
    fn only_saving_marking_and_closing_rescue_a_document() {
        assert!(rescues(&Command::SaveDocument { doc: DocHandle(1) }));
        assert!(rescues(&Command::CloseDocument { doc: DocHandle(1) }));
        assert!(!rescues(&Command::Undo { doc: DocHandle(1) }));
    }

    #[test]
    fn panic_messages_are_read_from_strings() {
        let text: Box<dyn Any + Send> = Box::new("boom");
        let owned: Box<dyn Any + Send> = Box::new("bang".to_string());
        let other: Box<dyn Any + Send> = Box::new(4);

        assert_eq!(panic_message(text.as_ref()), "boom");
        assert_eq!(panic_message(owned.as_ref()), "bang");
        assert_eq!(panic_message(other.as_ref()), "a panic without a message");
    }

    #[test]
    fn large_request_ids_are_malformed() {
        let mut session = Session::new(crate::SessionOptions::headless());

        let reply = session.execute(
            Request {
                id: MAX_SAFE_INTEGER + 1,
                command: Command::EngineInfo,
            },
            Vec::new(),
        );

        assert_eq!(reply.response.id, 0);
        let Outcome::Error(info) = reply.response.result else {
            panic!("an error response");
        };
        assert_eq!(info.code, ErrorCode::MalformedRequest);
    }
}
