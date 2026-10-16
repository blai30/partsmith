//! Edits, transactions, coalescing, undo and redo, and parameter renames (engine.md section 3.3).

use partsmith_features::{FeatureError, Registry};
use partsmith_model::{Document, Edit, Id, RecordKind, Step, Transaction};
use partsmith_protocol::DocHandle;

use crate::document::OpenTransaction;
use crate::error::EngineError;
use crate::session::Session;
use crate::state::{feature_label, is_active};
use crate::status::render_feature_error;
use crate::types::{Committed, Stepped};
use crate::wire::saturating_u32;

impl Session {
    /// Applies edits atomically; returns the new revision.
    pub fn apply_edits(
        &mut self,
        doc: DocHandle,
        label: Option<&str>,
        edits: Vec<Edit>,
    ) -> Result<u64, EngineError> {
        let open = self.open(doc)?;
        let label = match (&open.transaction, label) {
            (None, None) => return Err(EngineError::LabelRequired),
            (Some(transaction), Some(_)) => {
                return Err(EngineError::LabelNotAllowed {
                    label: transaction.label.clone(),
                });
            }
            (None, Some(label)) => label.to_string(),
            (Some(_), None) => String::new(),
        };
        self.apply_batch(doc, &label, edits)
    }

    /// Renames a parameter and rewrites the expressions that use it; returns the new revision.
    pub fn rename_parameter(
        &mut self,
        doc: DocHandle,
        parameter: &Id,
        name: &str,
    ) -> Result<u64, EngineError> {
        let open = self.open(doc)?;
        let edits = open
            .document
            .rename_parameter_edits(parameter, name, &*self.registry)
            .map_err(|source| EngineError::EditRejected {
                index: None,
                source,
            })?;
        let old = open
            .document
            .parameter(parameter)
            .map(|found| found.name.clone())
            .unwrap_or_default();
        self.apply_batch(doc, &format!("Rename parameter {old} to {name}"), edits)
    }

    /// Opens a transaction.
    pub fn begin_transaction(&mut self, doc: DocHandle, label: &str) -> Result<(), EngineError> {
        let open = self.open_mut(doc)?;
        open.require_no_transaction(&format!("start \"{label}\""))?;
        open.transaction = Some(OpenTransaction {
            label: label.to_string(),
            steps: Vec::new(),
            focus: None,
        });
        Ok(())
    }

    /// Sets or clears the focus feature of the open transaction.
    pub fn set_focus(&mut self, doc: DocHandle, feature: Option<&Id>) -> Result<(), EngineError> {
        let registry = std::sync::Arc::clone(&self.registry);
        let open = self.open_mut(doc)?;
        let transaction = open
            .transaction
            .as_ref()
            .ok_or(EngineError::NoTransaction)?;
        if let Some(feature) = feature {
            if open.document.feature(feature).is_none() {
                return Err(EngineError::UnknownFeature {
                    id: feature.clone(),
                });
            }
            if !is_active(&open.document, feature) {
                return Err(EngineError::FocusInactive {
                    id: feature.clone(),
                    name: feature_label(&registry, &open.document, feature),
                });
            }
        }
        if transaction.focus.as_ref() == feature {
            return Ok(());
        }
        if let Some(transaction) = &mut open.transaction {
            transaction.focus = feature.cloned();
        }
        self.rerun(doc)
    }

    /// Records the open transaction as one undo entry.
    pub fn commit_transaction(&mut self, doc: DocHandle) -> Result<Committed, EngineError> {
        let open = self.open_mut(doc)?;
        let transaction = open.transaction.take().ok_or(EngineError::NoTransaction)?;
        let recorded = !transaction.steps.is_empty();
        if recorded {
            open.undo.record(Transaction {
                label: transaction.label,
                steps: transaction.steps,
            });
        }
        let revision = open.revision;
        if transaction.focus.is_some() {
            self.rerun(doc)?;
        }
        Ok(Committed { revision, recorded })
    }

    /// Reverts the open transaction; returns the new revision.
    pub fn cancel_transaction(&mut self, doc: DocHandle) -> Result<u64, EngineError> {
        let open = self.open_mut(doc)?;
        let transaction = open
            .transaction
            .as_ref()
            .ok_or(EngineError::NoTransaction)?;
        let reverted = if transaction.steps.is_empty() {
            None
        } else {
            let mut document = Document::clone(&open.document);
            let inverses = transaction
                .steps
                .iter()
                .rev()
                .map(|step| step.inverse.clone())
                .collect();
            document
                .apply_all(inverses)
                .map_err(EngineError::unexpected)?;
            Some(document)
        };
        let focused = transaction.focus.is_some();
        open.transaction = None;
        let changed = reverted.is_some();
        if let Some(document) = reverted {
            open.change(document);
        }
        let revision = open.revision;
        if changed || focused {
            self.rerun(doc)?;
        }
        Ok(revision)
    }

    /// Reverts the newest undo entry.
    pub fn undo(&mut self, doc: DocHandle) -> Result<Stepped, EngineError> {
        self.step(doc, true)
    }

    /// Reapplies the newest redo entry.
    pub fn redo(&mut self, doc: DocHandle) -> Result<Stepped, EngineError> {
        self.step(doc, false)
    }

    /// Undoes (`backward`) or redoes one undo entry.
    fn step(&mut self, doc: DocHandle, backward: bool) -> Result<Stepped, EngineError> {
        let open = self.open_mut(doc)?;
        open.require_no_transaction(if backward { "undo" } else { "redo" })?;
        let mut document = Document::clone(&open.document);
        let stepped = if backward {
            open.undo.undo(&mut document)
        } else {
            open.undo.redo(&mut document)
        }
        .map_err(EngineError::unexpected)?;
        let label = match stepped {
            Some(transaction) => transaction.label.clone(),
            None if backward => return Err(EngineError::NothingToUndo),
            None => return Err(EngineError::NothingToRedo),
        };
        open.change(document);
        let revision = open.revision;
        self.rerun(doc)?;
        Ok(Stepped { revision, label })
    }

    /// Applies a batch atomically and records it: outside a transaction as one undo entry
    /// labeled `label`, inside one appended to it (section 3.3). Returns the revision.
    pub(crate) fn apply_batch(
        &mut self,
        doc: DocHandle,
        label: &str,
        edits: Vec<Edit>,
    ) -> Result<u64, EngineError> {
        let open = self.open(doc)?;
        if edits.is_empty() {
            return Ok(open.revision);
        }
        let mut document = Document::clone(&open.document);
        let mut steps = Vec::with_capacity(edits.len());
        for (position, edit) in edits.into_iter().enumerate() {
            let index = saturating_u32(position);
            let edit = canonical_edit(&self.registry, &document, edit, index)?;
            let step = document
                .apply_all(vec![edit])
                .map_err(|error| EngineError::EditRejected {
                    index: Some(index),
                    source: error.error,
                })?
                .pop()
                .ok_or_else(|| EngineError::internal("an applied edit has no step"))?;
            steps.push(step);
        }
        let open = self.open_mut(doc)?;
        open.change(document);
        match &mut open.transaction {
            Some(transaction) => {
                for step in steps {
                    push_coalesced(&mut transaction.steps, step);
                }
                if let Some(focus) = &transaction.focus
                    && !is_active(&open.document, focus)
                {
                    transaction.focus = None;
                }
            }
            None => open.undo.record(Transaction {
                label: label.to_string(),
                steps,
            }),
        }
        let revision = open.revision;
        self.rerun(doc)?;
        Ok(revision)
    }
}

/// The edit with the registry's canonical params when it adds or updates a feature
/// (`invalid_params` when the registry rejects them).
fn canonical_edit(
    registry: &Registry,
    document: &Document,
    edit: Edit,
    edit_index: u32,
) -> Result<Edit, EngineError> {
    let feature = match &edit {
        Edit::AddFeature { feature, .. } | Edit::UpdateFeature { feature } => feature,
        _ => return Ok(edit),
    };
    let validated = registry
        .validate_and_canonicalize(feature)
        .map_err(|error| invalid_params(registry, document, &edit, &error, edit_index))?;
    Ok(match edit {
        Edit::AddFeature { index, .. } => Edit::AddFeature {
            index,
            feature: validated.record,
        },
        _ => Edit::UpdateFeature {
            feature: validated.record,
        },
    })
}

/// The `invalid_params` error of a feature edit the registry rejects (section 4): the registry's
/// error worded for an edit that was not applied, naming the feature by its label in the working
/// copy with the edit applied, and the feature type, its id and the parser's text as the detail.
fn invalid_params(
    registry: &Registry,
    document: &Document,
    edit: &Edit,
    error: &FeatureError,
    edit_index: u32,
) -> EngineError {
    let (Edit::AddFeature { feature, .. } | Edit::UpdateFeature { feature }) = edit else {
        return EngineError::internal("only feature edits have params");
    };
    let mut edited = document.clone();
    if edited.apply(edit.clone()).is_err() {
        edited = document.clone();
    }
    let label = || feature_label(registry, &edited, &feature.id);
    let message = match error {
        FeatureError::InvalidParams {
            field: Some(field), ..
        } => format!(
            "{field_label} of '{feature_label}' cannot be read, so this change was not applied. Set {field_label} again.",
            field_label = field.label,
            feature_label = label(),
        ),
        FeatureError::InvalidParams { field: None, .. } => format!(
            "The settings of '{}' cannot be read, so this change was not applied.",
            label()
        ),
        FeatureError::UnknownType { type_name } => format!(
            "This version of Partsmith has no feature of type '{type_name}', so this change was not applied."
        ),
        other => render_feature_error(other, &edited, registry, &feature.id),
    };
    EngineError::InvalidParams {
        index: edit_index,
        feature: feature.id.clone(),
        feature_type: feature.kind.clone(),
        message,
        detail: format!(
            "{} {}: {}",
            feature.kind,
            feature.id,
            error.detail().unwrap_or_else(|| error.to_string())
        ),
    }
}

/// Appends a step to a transaction, merging it with the previous one when both update the same
/// record (section 3.3): the merged step keeps the earlier inverse and the later edit.
fn push_coalesced(steps: &mut Vec<Step>, step: Step) {
    if let Some(last) = steps.last_mut()
        && let (Some(previous), Some(next)) = (updated(&last.edit), updated(&step.edit))
        && previous == next
    {
        last.edit = step.edit;
        return;
    }
    steps.push(step);
}

/// The kind and id of the record an update edit replaces.
fn updated(edit: &Edit) -> Option<(RecordKind, &Id)> {
    match edit {
        Edit::UpdateParameter { parameter } => Some((RecordKind::Parameter, &parameter.id)),
        Edit::UpdateBody { body } => Some((RecordKind::Body, &body.id)),
        Edit::UpdateFeature { feature } => Some((RecordKind::Feature, &feature.id)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use partsmith_model::{Body, FeatureRecord, Parameter, Units};

    use super::*;
    use crate::test_support::id;

    fn set_height(expr: &str, previous: &str) -> Step {
        Step {
            edit: Edit::UpdateParameter {
                parameter: Parameter::new(id("p-h"), "height", expr),
            },
            inverse: Edit::UpdateParameter {
                parameter: Parameter::new(id("p-h"), "height", previous),
            },
        }
    }

    #[test]
    fn repeated_updates_of_one_record_merge() {
        let mut steps = Vec::new();

        push_coalesced(&mut steps, set_height("41 mm", "40 mm"));
        push_coalesced(&mut steps, set_height("42 mm", "41 mm"));

        assert_eq!(steps, [set_height("42 mm", "40 mm")]);
    }

    #[test]
    fn updates_of_different_records_stay_apart() {
        let mut steps = vec![set_height("41 mm", "40 mm")];
        let body = Step {
            edit: Edit::UpdateBody {
                body: Body::new(id("p-h"), "Block"),
            },
            inverse: Edit::UpdateBody {
                body: Body::new(id("p-h"), "Old"),
            },
        };

        push_coalesced(&mut steps, body);
        push_coalesced(&mut steps, set_height("42 mm", "41 mm"));

        assert_eq!(steps.len(), 3);
    }

    #[test]
    fn other_edits_never_merge() {
        let mut steps = Vec::new();
        let units = |units| Step {
            edit: Edit::SetUnits { units },
            inverse: Edit::SetUnits { units: Units::Mm },
        };

        push_coalesced(&mut steps, units(Units::Cm));
        push_coalesced(&mut steps, units(Units::In));

        assert_eq!(steps.len(), 2);
    }

    /// A box record of 10 mm in `b-1`, with or without its body.
    fn box_record(feature: &str, with_body: bool) -> FeatureRecord {
        let mut record = serde_json::json!({
            "id": feature, "type": "box", "size": ["10", "10", "10"], "op": "new"
        });
        if with_body {
            record["body"] = serde_json::json!("b-1");
        }
        serde_json::from_value(record).expect("a well-formed record")
    }

    #[test]
    fn unknown_feature_types_are_invalid_params() {
        let registry = Registry::new();
        let document = Document::new(Units::Mm);
        let edit = Edit::AddFeature {
            index: 0,
            feature: FeatureRecord::new(id("f-1"), "wedge"),
        };

        let error = canonical_edit(&registry, &document, edit, 3).expect_err("wedge is unknown");

        assert_eq!(error.edit_index(), Some(3));
        assert_eq!(
            error.to_string(),
            "This version of Partsmith has no feature of type 'wedge', so this change was not applied."
        );
        let detail = error.detail().expect("the error has a detail");
        assert!(detail.starts_with("wedge f-1: "), "{detail}");
    }

    #[test]
    fn invalid_params_name_the_field_and_the_feature_label_of_the_edited_document() {
        let registry = Registry::with_builtins();
        let mut document = Document::new(Units::Mm);
        document.features.push(box_record("f-box1", true));
        let edit = Edit::AddFeature {
            index: 1,
            feature: box_record("f-box2", false),
        };

        let error = canonical_edit(&registry, &document, edit, 0).expect_err("the box has no body");

        assert_eq!(
            error.to_string(),
            "Body of 'Box 2' cannot be read, so this change was not applied. Set Body again."
        );
        let detail = error.detail().expect("the error has a detail");
        assert!(detail.starts_with("box f-box2: "), "{detail}");
    }

    #[test]
    fn non_feature_edits_pass_through() {
        let edit = Edit::SetUnits { units: Units::In };

        let result = canonical_edit(&Registry::new(), &Document::new(Units::Mm), edit.clone(), 0);

        assert_eq!(result, Ok(edit));
    }
}
