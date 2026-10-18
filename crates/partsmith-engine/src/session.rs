//! The session: options, the registry, open documents and their lifecycle, and the queries that
//! read a document's latest recompute (engine.md sections 2, 3.2, 3.4, 3.7 and 3.10).

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::Registry;
use partsmith_mesh::TessellationTolerance;
use partsmith_model::{
    Document, Edit, Expr, FORMAT_VERSION, IdGenerator, Quantity, Scope, Units, load,
};
use partsmith_occt_sys::OCCT_VERSION;
use partsmith_print::{PrinterProfile, PrinterValues, ProfileInput};
use partsmith_protocol::{
    CatalogEntry, DocHandle, FeatureStatus, PROTOCOL_VERSION, ParameterValue, TessellationQuality,
};
use partsmith_recompute::Recompute;

use crate::ENGINE_VERSION;
use crate::assets::{referenced_assets, stored_assets};
use crate::display::DisplayCache;
use crate::document::{OpenDocument, SavedText, canonical_text, canonicalize_params};
use crate::error::EngineError;
use crate::options::SessionOptions;
use crate::profiles::{Profiles, tolerance};
use crate::state::{self, Computed};
use crate::status::{feature_statuses, parameter_values};
use crate::types::{Asset, EngineInfo, Opened, Saved};
use crate::units::Inspections;

/// Bytes of one cached recompute result the engine assumes when it turns its byte budget into
/// recompute's entry limit (recompute evicts by entry count in M1, recompute.md section 1), so
/// the default 1 GiB budget keeps recompute's default of 4096 results.
const ESTIMATED_RESULT_BYTES: usize = 256 << 10;

/// The engine: open documents, caches and the feature registry.
pub struct Session {
    /// Produce display meshes and buffers.
    pub(crate) display: bool,
    /// Committed transactions kept per document.
    pub(crate) undo_limit: usize,
    /// Display quality of new and opened documents.
    quality: TessellationQuality,
    /// Ids for records the engine creates.
    pub(crate) ids: Box<dyn IdGenerator + Send>,
    /// The feature registry.
    pub(crate) registry: Arc<Registry>,
    /// The recompute engine with its result cache, shared by all documents.
    pub(crate) recompute: Recompute,
    /// Session printer profiles.
    pub(crate) profiles: Profiles,
    /// Open documents by handle.
    pub(crate) documents: BTreeMap<DocHandle, OpenDocument>,
    /// The handle of the next new or opened document.
    next_handle: u32,
    /// The id of the next `body_mesh` event.
    pub(crate) next_mesh_id: u64,
    /// Element tables and display meshes.
    pub(crate) cache: DisplayCache,
    /// The registry's inspection of the feature records of every document, memoized by content
    /// (section 3.25).
    pub(crate) inspections: Inspections,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Session")
            .field("display", &self.display)
            .field("documents", &self.documents.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

impl Session {
    /// Creates a session with the built-in feature registry and printer profiles.
    pub fn new(options: SessionOptions) -> Self {
        let registry = Arc::new(Registry::with_builtins());
        let recompute = Recompute::with_cache_limit(
            Arc::clone(&registry),
            options.cache_budget / ESTIMATED_RESULT_BYTES,
        );
        Session {
            display: options.display,
            undo_limit: options.undo_limit,
            quality: options.tessellation,
            ids: options.id_generator,
            registry,
            recompute,
            profiles: Profiles::default(),
            documents: BTreeMap::new(),
            next_handle: 1,
            next_mesh_id: 1,
            cache: DisplayCache::new(options.cache_budget),
            inspections: Inspections::default(),
        }
    }

    /// The feature registry.
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Engine and format versions.
    pub fn engine_info(&self) -> EngineInfo {
        EngineInfo {
            protocol_version: PROTOCOL_VERSION,
            engine_version: ENGINE_VERSION.to_string(),
            format_version: FORMAT_VERSION,
            kernel: format!("OCCT {OCCT_VERSION}"),
        }
    }

    /// Every registered feature type, sorted by type name.
    pub fn feature_catalog(&self) -> Vec<CatalogEntry> {
        self.registry
            .params_schemas()
            .into_iter()
            .filter_map(|schema| {
                let manifest = self.registry.get(&schema.kind)?.manifest().clone();
                Some(CatalogEntry {
                    kind: schema.kind,
                    manifest,
                    params_schema: schema.schema.to_value(),
                })
            })
            .collect()
    }

    /// Built-in profiles then session profiles, each sorted by id.
    pub fn printer_profiles(&self) -> Vec<PrinterProfile> {
        self.profiles.list()
    }

    /// Converts and validates a profile (print.md `PrinterProfile::try_from`), adds or replaces it
    /// as a session profile, and recomputes documents that use it.
    pub fn put_printer_profile(&mut self, profile: ProfileInput) -> Result<(), EngineError> {
        self.put_profile(profile).map(|_| ())
    }

    /// `put_printer_profile`, returning the documents it recomputed in handle order.
    pub(crate) fn put_profile(
        &mut self,
        profile: ProfileInput,
    ) -> Result<Vec<DocHandle>, EngineError> {
        let profile = self.profiles.put(profile)?;
        let affected: Vec<DocHandle> = self
            .documents
            .iter()
            .filter(|(_, open)| open.document.printer.as_deref() == Some(profile.id.as_str()))
            .map(|(&handle, _)| handle)
            .collect();
        for &handle in &affected {
            self.rerun(handle)?;
        }
        Ok(affected)
    }

    /// Creates an empty document.
    pub fn new_document(
        &mut self,
        units: Units,
        printer: Option<String>,
    ) -> Result<DocHandle, EngineError> {
        let mut document = Document::new(units);
        if printer.is_some() {
            document
                .apply(Edit::SetPrinter { printer })
                .map_err(|source| EngineError::EditRejected {
                    index: None,
                    source,
                })?;
        }
        self.insert(document, BTreeMap::new())
    }

    /// Parses, migrates and recomputes a `.ptsm` text.
    pub fn open_document(&mut self, text: &str, assets: Vec<Asset>) -> Result<Opened, EngineError> {
        let loaded = load(text).map_err(EngineError::from_load)?;
        let mut document = loaded.document;
        canonicalize_params(&self.registry, &mut document);
        let stored = referenced_assets(&self.registry, &document, assets)?;
        let doc = self.insert(document, stored)?;
        Ok(Opened {
            doc,
            migrated_from: loaded.migrated_from,
        })
    }

    /// Adds a document with its assets under the next handle and recomputes it.
    fn insert(
        &mut self,
        document: Document,
        assets: BTreeMap<String, Arc<[u8]>>,
    ) -> Result<DocHandle, EngineError> {
        let profile = self.profiles.resolve(document.printer.as_deref());
        let computed = state::run(&mut self.recompute, &document, &assets, profile, None, 0);
        let open = OpenDocument::new(document, assets, self.undo_limit, self.quality, computed)?;
        let handle = DocHandle(self.next_handle);
        self.next_handle += 1;
        self.documents.insert(handle, open);
        Ok(handle)
    }

    /// Canonical text, referenced assets and the revision they describe; does not change the
    /// dirty state.
    pub fn save_document(&mut self, doc: DocHandle) -> Result<Saved, EngineError> {
        let registry = Arc::clone(&self.registry);
        let open = self.open_mut(doc)?;
        let (text, recipe) = match &open.poisoned {
            Some(rescued) => (canonical_text(rescued)?, Arc::clone(rescued)),
            None => {
                open.require_no_transaction("save")?;
                (open.text()?.to_string(), Arc::clone(&open.document))
            }
        };
        let assets = stored_assets(&registry, &recipe, &open.assets);
        open.last_save = Some(SavedText {
            revision: open.revision,
            text: text.clone(),
        });
        Ok(Saved {
            text,
            assets,
            revision: open.revision,
        })
    }

    /// Moves the dirty baseline to the text of a finished save.
    pub fn mark_saved(&mut self, doc: DocHandle, revision: u64) -> Result<(), EngineError> {
        let open = self.open_mut(doc)?;
        if let Some(saved) = &open.last_save
            && saved.revision == revision
        {
            open.baseline = saved.text.clone();
        }
        Ok(())
    }

    /// Discards a document.
    pub fn close_document(&mut self, doc: DocHandle) -> Result<(), EngineError> {
        self.documents
            .remove(&doc)
            .ok_or(EngineError::UnknownDocument { doc })?;
        // Its displayed meshes are unpinned now.
        self.cache.trim();
        Ok(())
    }

    /// The current recipe, including uncommitted transaction edits.
    pub fn document(&self, doc: DocHandle) -> Result<&Document, EngineError> {
        Ok(&self.open(doc)?.document)
    }

    /// The current revision.
    pub fn revision(&self, doc: DocHandle) -> Result<u64, EngineError> {
        Ok(self.open(doc)?.revision)
    }

    /// Status of every feature in timeline order at the current state.
    pub fn feature_statuses(&self, doc: DocHandle) -> Result<Vec<FeatureStatus>, EngineError> {
        let open = self.open(doc)?;
        Ok(feature_statuses(
            &open.document,
            &self.registry,
            &open.computed,
            &open.emitted.display_failures(),
            open.revision,
            None,
        ))
    }

    /// Every parameter's evaluated value in document order.
    pub fn parameter_values(&self, doc: DocHandle) -> Result<Vec<ParameterValue>, EngineError> {
        Ok(parameter_values(&self.open(doc)?.computed))
    }

    /// Evaluates an expression against the document.
    pub fn evaluate(&self, doc: DocHandle, expr: &Expr) -> Result<Quantity, EngineError> {
        let open = self.open(doc)?;
        evaluate_in(&open.document, &open.computed, |scope| {
            expr.evaluate(scope, None)
                .map_err(|error| EngineError::Expression {
                    message: error.to_string(),
                    span: Some(error.span),
                })
        })
    }

    /// Sets the display quality; returns the resolved tolerance.
    pub fn set_tessellation(
        &mut self,
        doc: DocHandle,
        quality: TessellationQuality,
    ) -> Result<TessellationTolerance, EngineError> {
        let open = self.open_mut(doc)?;
        open.quality = quality;
        open.emitted.refresh_meshes = true;
        Ok(tolerance(&open.computed.profile.profile, quality))
    }

    /// The resolved tessellation tolerance for a quality (section 3.7); no side effects.
    pub fn tolerance(
        &self,
        doc: DocHandle,
        quality: TessellationQuality,
    ) -> Result<TessellationTolerance, EngineError> {
        Ok(tolerance(
            &self.open(doc)?.computed.profile.profile,
            quality,
        ))
    }

    /// The id of the document's resolved printer profile (section 3.7): the installed profile its
    /// `printer` names, else the default FDM profile. No side effects.
    pub fn printer_profile(&self, doc: DocHandle) -> Result<String, EngineError> {
        let profile = &self.open(doc)?.computed.profile.profile;
        Ok(profile.id.as_str().to_owned())
    }

    /// An open document (`unknown_document` otherwise).
    pub(crate) fn open(&self, doc: DocHandle) -> Result<&OpenDocument, EngineError> {
        self.documents
            .get(&doc)
            .ok_or(EngineError::UnknownDocument { doc })
    }

    /// An open document, for changes (`unknown_document` otherwise).
    pub(crate) fn open_mut(&mut self, doc: DocHandle) -> Result<&mut OpenDocument, EngineError> {
        self.documents
            .get_mut(&doc)
            .ok_or(EngineError::UnknownDocument { doc })
    }

    /// Recomputes a document with its current recipe, focus and resolved profile.
    pub(crate) fn rerun(&mut self, doc: DocHandle) -> Result<(), EngineError> {
        let open = self
            .documents
            .get_mut(&doc)
            .ok_or(EngineError::UnknownDocument { doc })?;
        let profile = self.profiles.resolve(open.document.printer.as_deref());
        let focus = open.focus().cloned();
        open.computed = state::run(
            &mut self.recompute,
            &open.document,
            &open.assets,
            profile,
            focus,
            open.computed.run,
        );
        Ok(())
    }
}

/// Runs `evaluate` with the document's parameters and the `printer.*` values of its profile.
pub(crate) fn evaluate_in<T>(
    document: &Document,
    computed: &Computed,
    evaluate: impl FnOnce(&Scope<'_>) -> T,
) -> T {
    let context = PrinterValues::new(&computed.profile.profile);
    let parameters = document.evaluate_parameters(&context);
    evaluate(&Scope {
        units: document.units,
        parameters: &parameters,
        context: &context,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session::new(SessionOptions::headless())
    }

    #[test]
    fn handles_count_up_from_one() {
        let mut session = session();

        let first = session
            .new_document(Units::Mm, None)
            .expect("an empty document opens");
        let second = session
            .new_document(Units::In, None)
            .expect("an empty document opens");

        assert_eq!((first, second), (DocHandle(1), DocHandle(2)));
        assert_eq!(
            session.document(second).map(|document| document.units),
            Ok(Units::In)
        );
    }

    #[test]
    fn invalid_printer_labels_are_rejected() {
        let mut session = session();

        let result = session.new_document(Units::Mm, Some(String::new()));

        assert!(matches!(result, Err(EngineError::EditRejected { .. })));
    }

    #[test]
    fn engine_info_names_the_kernel() {
        let info = session().engine_info();

        assert_eq!(info.kernel, format!("OCCT {OCCT_VERSION}"));
        assert_eq!(info.protocol_version, PROTOCOL_VERSION);
        assert_eq!(info.format_version, FORMAT_VERSION);
    }

    #[test]
    fn closed_documents_are_unknown() {
        let mut session = session();
        let doc = session
            .new_document(Units::Mm, None)
            .expect("an empty document opens");

        session.close_document(doc).expect("the document closes");

        assert_eq!(
            session.revision(doc),
            Err(EngineError::UnknownDocument { doc })
        );
        assert_eq!(
            session.close_document(doc),
            Err(EngineError::UnknownDocument { doc })
        );
    }

    #[test]
    fn evaluation_sees_parameters_and_printer_values() {
        let mut session = session();
        let doc = session
            .open_document(
                r#"{"format": "partsmith", "format_version": 1, "units": "mm", "parameters": [{"id": "p-h", "name": "height", "expr": "40 mm"}]}"#,
                Vec::new(),
            )
            .expect("the document opens")
            .doc;

        let doubled = session
            .evaluate(doc, &Expr::new("2 * height"))
            .expect("the expression evaluates");
        let slip = session.evaluate(doc, &Expr::new("printer.fit.slip"));
        let mismatch = session.evaluate(doc, &Expr::new("height + 30 deg"));

        assert_eq!(doubled, Quantity::length(80.0));
        assert!(slip.is_ok());
        assert!(matches!(
            mismatch,
            Err(EngineError::Expression { span: Some(_), .. })
        ));
    }

    /// Whether the document differs from its dirty baseline.
    fn dirty(session: &mut Session, doc: DocHandle) -> bool {
        session
            .open_mut(doc)
            .and_then(OpenDocument::dirty)
            .expect("the document is open")
    }

    #[test]
    fn saving_marks_nothing_until_the_host_confirms() {
        let mut session = session();
        let doc = session
            .new_document(Units::Mm, None)
            .expect("an empty document opens");
        session
            .apply_edits(
                doc,
                Some("Units"),
                vec![Edit::SetUnits { units: Units::In }],
            )
            .expect("the edit applies");

        let saved = session.save_document(doc).expect("the document saves");
        let dirty_after_save = dirty(&mut session, doc);
        session
            .mark_saved(doc, saved.revision + 1)
            .expect("a stale revision is not an error");
        let dirty_after_stale_mark = dirty(&mut session, doc);
        session
            .mark_saved(doc, saved.revision)
            .expect("the save is confirmed");

        assert_eq!(saved.revision, 1);
        assert!(saved.assets.is_empty());
        assert!(saved.text.ends_with("}\n"));
        assert!(dirty_after_save);
        assert!(dirty_after_stale_mark);
        assert!(!dirty(&mut session, doc));
    }
}
