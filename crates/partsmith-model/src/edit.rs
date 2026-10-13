//! Edits with exact inverses, and atomic transactions of edits (model.md sections 3.9 and 3.10).

use std::fmt;
use std::mem;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::document::RecordProblem;
use crate::names::{LabelError, LabelField, NameError, validate_label, validate_parameter_name};
use crate::{Body, Document, FeatureRecord, Id, Parameter, Units};

/// One change to a document. Every change goes through an Edit; applying one returns its exact inverse.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
// Edits are read only from protocol requests, which reject unknown fields (protocol.md section
// 2.3). Serde enforces this for struct variants only: a unit variant would ignore unknown fields.
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    /// Inserts a parameter at `index` (0 to the number of parameters).
    AddParameter {
        /// Position of the new parameter.
        index: usize,
        /// The new parameter.
        parameter: Parameter,
    },
    /// Replaces the parameter with the same id.
    UpdateParameter {
        /// The new record.
        parameter: Parameter,
    },
    /// Removes a parameter.
    RemoveParameter {
        /// The parameter to remove.
        id: Id,
    },
    /// Moves a parameter so that it ends up at `index`.
    MoveParameter {
        /// The parameter to move.
        id: Id,
        /// Its position after the move.
        index: usize,
    },
    /// Inserts a body at `index`.
    AddBody {
        /// Position of the new body.
        index: usize,
        /// The new body.
        body: Body,
    },
    /// Replaces the body with the same id.
    UpdateBody {
        /// The new record.
        body: Body,
    },
    /// Removes a body.
    RemoveBody {
        /// The body to remove.
        id: Id,
    },
    /// Moves a body so that it ends up at `index`.
    MoveBody {
        /// The body to move.
        id: Id,
        /// Its position after the move.
        index: usize,
    },
    /// Inserts a feature record at timeline position `index`.
    AddFeature {
        /// Timeline position of the new feature.
        index: usize,
        /// The new feature record.
        feature: FeatureRecord,
    },
    /// Replaces the feature record with the same id.
    UpdateFeature {
        /// The new record.
        feature: FeatureRecord,
    },
    /// Removes a feature record.
    RemoveFeature {
        /// The feature to remove.
        id: Id,
    },
    /// Moves a feature record so that it ends up at timeline position `index`.
    MoveFeature {
        /// The feature to move.
        id: Id,
        /// Its timeline position after the move.
        index: usize,
    },
    /// Sets the rollback marker; `None` means nothing is rolled back.
    SetRollback {
        /// The first rolled-back feature.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rollback: Option<Id>,
    },
    /// Sets the document units.
    SetUnits {
        /// The new units.
        units: Units,
    },
    /// Sets or clears the printer profile id.
    SetPrinter {
        /// The printer profile id.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        printer: Option<String>,
    },
}

impl Edit {
    /// The edit with every number in its feature params normalized (file-format.md section 3.3).
    fn normalized(mut self) -> Edit {
        if let Edit::AddFeature { feature, .. } | Edit::UpdateFeature { feature } = &mut self {
            feature.normalize();
        }
        self
    }
}

/// An applied edit together with its inverse.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Step {
    /// The edit as applied.
    pub edit: Edit,
    /// The edit that undoes it.
    pub inverse: Edit,
}

/// A labeled group of steps that undo and redo as one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Transaction {
    /// Text shown as `Undo <label>`, such as "Extrude".
    pub label: String,
    /// Steps in the order they were applied.
    pub steps: Vec<Step>,
}

/// Kind of record named in `EditError::NotFound`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RecordKind {
    /// Worded "parameter".
    Parameter,
    /// Worded "body".
    Body,
    /// Worded "feature".
    Feature,
}

impl fmt::Display for RecordKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let wording = match self {
            RecordKind::Parameter => "parameter",
            RecordKind::Body => "body",
            RecordKind::Feature => "feature",
        };
        formatter.write_str(wording)
    }
}

/// Why an edit cannot be applied; the document is unchanged.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum EditError {
    /// No record of this kind has the id.
    #[error("There is no {kind} with the id {id}.")]
    NotFound {
        /// The kind of record looked for.
        kind: RecordKind,
        /// The id looked for.
        id: Id,
    },
    /// A new record's id is already used.
    #[error("The id {id} is already used in this document. Use a new id.")]
    IdInUse {
        /// The id.
        id: Id,
    },
    /// A parameter name breaks the name rules.
    #[error("{0}")]
    InvalidParameterName(NameError),
    /// Another parameter already has the name.
    #[error("A parameter named {name} already exists. Pick another name.")]
    NameInUse {
        /// The name.
        name: String,
    },
    /// A body name, material, feature name or printer breaks the label rules.
    #[error("{0}")]
    InvalidLabel(LabelError),
    /// A feature type name breaks the type rules.
    #[error(
        "'{kind}' is not a valid feature type. Type names use lowercase letters, digits and _, and start with a letter."
    )]
    InvalidFeatureType {
        /// The type name.
        kind: String,
    },
    /// Feature params contain a key the record itself uses.
    #[error("Feature parameters cannot use the key '{key}', because the record itself uses it.")]
    ReservedParamKey {
        /// The first reserved key, in params order.
        key: String,
    },
    /// A position beyond the end of the list.
    #[error("Position {index} is out of range. Use a position from 0 to {max}.")]
    IndexOutOfRange {
        /// The position given.
        index: usize,
        /// The largest valid position.
        max: usize,
    },
    /// The feature to remove holds the rollback marker.
    #[error("Feature {id} holds the rollback marker. Move the rollback marker before removing it.")]
    RemovesRollbackFeature {
        /// The feature.
        id: Id,
    },
    /// The rollback marker would point at a feature that does not exist.
    #[error("Cannot roll back to {id} because there is no feature with that id.")]
    UnknownRollbackFeature {
        /// The id given.
        id: Id,
    },
}

/// A failed transaction: the edit at `index` failed and every earlier edit was undone.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("Change {} of {count} could not be applied: {error}", .index + 1)]
pub struct TransactionError {
    /// 0-based position of the failing edit.
    pub index: usize,
    /// Number of edits in the batch.
    pub count: usize,
    /// Why it failed.
    pub error: EditError,
}

impl Document {
    /// Applies one edit and returns its exact inverse; on error the document is unchanged.
    pub fn apply(&mut self, edit: Edit) -> Result<Edit, EditError> {
        self.apply_located(edit.normalized())
            .map(|(inverse, _)| inverse)
    }

    /// Applies edits in order, all or nothing, and returns each edit with its inverse.
    ///
    /// When an edit fails, the edits before it are undone by position (`unwind`) rather than by
    /// applying their inverses, so the document comes back exactly as it was even when it breaks
    /// the structural rules (a repeated id, for example), where an inverse could fail or find the
    /// wrong record. This never panics.
    pub fn apply_all(&mut self, edits: Vec<Edit>) -> Result<Vec<Step>, TransactionError> {
        let count = edits.len();
        let mut steps: Vec<Step> = Vec::with_capacity(count);
        let mut positions: Vec<usize> = Vec::with_capacity(count);
        for (index, edit) in edits.into_iter().enumerate() {
            let edit = edit.normalized();
            match self.apply_located(edit.clone()) {
                Ok((inverse, position)) => {
                    steps.push(Step { edit, inverse });
                    positions.push(position);
                }
                Err(error) => {
                    for (step, position) in steps.into_iter().zip(positions).rev() {
                        self.unwind(step.inverse, position);
                    }
                    return Err(TransactionError {
                        index,
                        count,
                        error,
                    });
                }
            }
        }
        Ok(steps)
    }

    /// Applies one edit, whose feature params are already normalized, and returns its inverse
    /// together with the position, in its list, of the record the edit inserted, replaced,
    /// removed or moved (its position after the edit; where it was, for a removal). The position
    /// is 0 for edits that set a document field.
    fn apply_located(&mut self, edit: Edit) -> Result<(Edit, usize), EditError> {
        match edit {
            Edit::AddParameter { index, parameter } => {
                self.check_unused_id(&parameter.id)?;
                check_parameter_name(&parameter.name)?;
                if self.parameter_by_name(&parameter.name).is_some() {
                    return Err(EditError::NameInUse {
                        name: parameter.name,
                    });
                }
                let id = parameter.id.clone();
                insert(&mut self.parameters, index, parameter)?;
                Ok((Edit::RemoveParameter { id }, index))
            }
            Edit::UpdateParameter { parameter } => {
                let position = position(&self.parameters, &parameter.id)?;
                check_parameter_name(&parameter.name)?;
                let name_taken =
                    self.parameters.iter().enumerate().any(|(other, existing)| {
                        other != position && existing.name == parameter.name
                    });
                if name_taken {
                    return Err(EditError::NameInUse {
                        name: parameter.name,
                    });
                }
                let previous = mem::replace(&mut self.parameters[position], parameter);
                Ok((
                    Edit::UpdateParameter {
                        parameter: previous,
                    },
                    position,
                ))
            }
            Edit::RemoveParameter { id } => {
                let index = position(&self.parameters, &id)?;
                let parameter = self.parameters.remove(index);
                Ok((Edit::AddParameter { index, parameter }, index))
            }
            Edit::MoveParameter { id, index } => {
                let previous = move_to(&mut self.parameters, &id, index)?;
                Ok((
                    Edit::MoveParameter {
                        id,
                        index: previous,
                    },
                    index,
                ))
            }
            Edit::AddBody { index, body } => {
                self.check_unused_id(&body.id)?;
                check_body(&body)?;
                let id = body.id.clone();
                insert(&mut self.bodies, index, body)?;
                Ok((Edit::RemoveBody { id }, index))
            }
            Edit::UpdateBody { body } => {
                let position = position(&self.bodies, &body.id)?;
                check_body(&body)?;
                let previous = mem::replace(&mut self.bodies[position], body);
                Ok((Edit::UpdateBody { body: previous }, position))
            }
            Edit::RemoveBody { id } => {
                let index = position(&self.bodies, &id)?;
                let body = self.bodies.remove(index);
                Ok((Edit::AddBody { index, body }, index))
            }
            Edit::MoveBody { id, index } => {
                let previous = move_to(&mut self.bodies, &id, index)?;
                Ok((
                    Edit::MoveBody {
                        id,
                        index: previous,
                    },
                    index,
                ))
            }
            Edit::AddFeature { index, feature } => {
                self.check_unused_id(&feature.id)?;
                check_feature(&feature)?;
                let id = feature.id.clone();
                insert(&mut self.features, index, feature)?;
                Ok((Edit::RemoveFeature { id }, index))
            }
            Edit::UpdateFeature { feature } => {
                let position = position(&self.features, &feature.id)?;
                check_feature(&feature)?;
                let previous = mem::replace(&mut self.features[position], feature);
                Ok((Edit::UpdateFeature { feature: previous }, position))
            }
            Edit::RemoveFeature { id } => {
                let index = position(&self.features, &id)?;
                if self.rollback.as_ref() == Some(&id) {
                    return Err(EditError::RemovesRollbackFeature { id });
                }
                let feature = self.features.remove(index);
                Ok((Edit::AddFeature { index, feature }, index))
            }
            Edit::MoveFeature { id, index } => {
                let previous = move_to(&mut self.features, &id, index)?;
                Ok((
                    Edit::MoveFeature {
                        id,
                        index: previous,
                    },
                    index,
                ))
            }
            Edit::SetRollback { rollback } => {
                if let Some(id) = &rollback
                    && self.feature(id).is_none()
                {
                    return Err(EditError::UnknownRollbackFeature { id: id.clone() });
                }
                let previous = mem::replace(&mut self.rollback, rollback);
                Ok((Edit::SetRollback { rollback: previous }, 0))
            }
            Edit::SetUnits { units } => {
                let previous = mem::replace(&mut self.units, units);
                Ok((Edit::SetUnits { units: previous }, 0))
            }
            Edit::SetPrinter { printer } => {
                if let Some(printer) = &printer {
                    validate_label(LabelField::Printer, printer)
                        .map_err(EditError::InvalidLabel)?;
                }
                let previous = mem::replace(&mut self.printer, printer);
                Ok((Edit::SetPrinter { printer: previous }, 0))
            }
        }
    }

    /// Undoes an edit that `apply_located` just applied, given its inverse and the position it
    /// returned. It works by position and checks nothing, so it cannot fail, and it puts back the
    /// exact records (numbers included) that the edit replaced or removed.
    fn unwind(&mut self, inverse: Edit, position: usize) {
        match inverse {
            Edit::RemoveParameter { .. } => {
                self.parameters.remove(position);
            }
            Edit::UpdateParameter { parameter } => self.parameters[position] = parameter,
            Edit::AddParameter { index, parameter } => self.parameters.insert(index, parameter),
            Edit::MoveParameter { index, .. } => move_back(&mut self.parameters, position, index),
            Edit::RemoveBody { .. } => {
                self.bodies.remove(position);
            }
            Edit::UpdateBody { body } => self.bodies[position] = body,
            Edit::AddBody { index, body } => self.bodies.insert(index, body),
            Edit::MoveBody { index, .. } => move_back(&mut self.bodies, position, index),
            Edit::RemoveFeature { .. } => {
                self.features.remove(position);
            }
            Edit::UpdateFeature { feature } => self.features[position] = feature,
            Edit::AddFeature { index, feature } => self.features.insert(index, feature),
            Edit::MoveFeature { index, .. } => move_back(&mut self.features, position, index),
            Edit::SetRollback { rollback } => self.rollback = rollback,
            Edit::SetUnits { units } => self.units = units,
            Edit::SetPrinter { printer } => self.printer = printer,
        }
    }

    /// `IdInUse` when a parameter, body or feature already has `id`.
    fn check_unused_id(&self, id: &Id) -> Result<(), EditError> {
        if self.contains_id(id) {
            Err(EditError::IdInUse { id: id.clone() })
        } else {
            Ok(())
        }
    }
}

/// A record stored in one of the document's lists.
trait Record {
    /// The kind named when it is not found.
    const KIND: RecordKind;

    /// Its permanent id.
    fn id(&self) -> &Id;
}

impl Record for Parameter {
    const KIND: RecordKind = RecordKind::Parameter;

    fn id(&self) -> &Id {
        &self.id
    }
}

impl Record for Body {
    const KIND: RecordKind = RecordKind::Body;

    fn id(&self) -> &Id {
        &self.id
    }
}

impl Record for FeatureRecord {
    const KIND: RecordKind = RecordKind::Feature;

    fn id(&self) -> &Id {
        &self.id
    }
}

/// Position of the record with `id`, or `NotFound`.
fn position<T: Record>(records: &[T], id: &Id) -> Result<usize, EditError> {
    records
        .iter()
        .position(|record| record.id() == id)
        .ok_or_else(|| EditError::NotFound {
            kind: T::KIND,
            id: id.clone(),
        })
}

/// Inserts `record` at `index`, which may be at most the length of the list.
fn insert<T>(records: &mut Vec<T>, index: usize, record: T) -> Result<(), EditError> {
    if index > records.len() {
        return Err(EditError::IndexOutOfRange {
            index,
            max: records.len(),
        });
    }
    records.insert(index, record);
    Ok(())
}

/// Moves the record with `id` so that it ends up at `index`, and returns its previous position.
fn move_to<T: Record>(records: &mut Vec<T>, id: &Id, index: usize) -> Result<usize, EditError> {
    let previous = position(records, id)?;
    // The record exists, so the list is not empty.
    let max = records.len() - 1;
    if index > max {
        return Err(EditError::IndexOutOfRange { index, max });
    }
    let record = records.remove(previous);
    records.insert(index, record);
    Ok(previous)
}

/// Moves the record at `position` back to `index`, undoing `move_to`.
fn move_back<T>(records: &mut Vec<T>, position: usize, index: usize) {
    let record = records.remove(position);
    records.insert(index, record);
}

/// Checks a parameter name for an edit.
fn check_parameter_name(name: &str) -> Result<(), EditError> {
    validate_parameter_name(name).map_err(EditError::InvalidParameterName)
}

/// Checks a body's name and material for an edit: the first problem fails it.
fn check_body(body: &Body) -> Result<(), EditError> {
    match body.label_errors().next() {
        Some(error) => Err(EditError::InvalidLabel(error)),
        None => Ok(()),
    }
}

/// Checks a feature record's type, name and params keys for an edit: the first problem fails it.
fn check_feature(feature: &FeatureRecord) -> Result<(), EditError> {
    let Some(problem) = feature.problems().next() else {
        return Ok(());
    };
    Err(match problem {
        RecordProblem::InvalidType => EditError::InvalidFeatureType {
            kind: feature.kind.clone(),
        },
        RecordProblem::InvalidName(error) => EditError::InvalidLabel(error),
        RecordProblem::ReservedKey(key) => EditError::ReservedParamKey { key: key.clone() },
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn id(text: &str) -> Id {
        Id::new(text).expect("valid id")
    }

    fn document_with_bodies(names: &[&str]) -> Document {
        Document {
            bodies: names
                .iter()
                .map(|name| Body::new(id(&format!("b-{name}")), *name))
                .collect(),
            ..Document::default()
        }
    }

    fn body_names(document: &Document) -> Vec<&str> {
        document
            .bodies
            .iter()
            .map(|body| body.name.as_str())
            .collect()
    }

    #[test]
    fn moving_toward_the_front() {
        let mut document = document_with_bodies(&["a", "b", "c"]);

        let inverse = document.apply(Edit::MoveBody {
            id: id("b-c"),
            index: 0,
        });

        assert_eq!(body_names(&document), ["c", "a", "b"]);
        assert_eq!(
            inverse,
            Ok(Edit::MoveBody {
                id: id("b-c"),
                index: 2
            })
        );
    }

    #[test]
    fn moving_to_the_current_index_changes_nothing() {
        let mut document = document_with_bodies(&["a", "b"]);
        let edit = Edit::MoveBody {
            id: id("b-b"),
            index: 1,
        };

        assert_eq!(document.apply(edit.clone()), Ok(edit));
        assert_eq!(body_names(&document), ["a", "b"]);
    }

    #[test]
    fn no_op_updates_return_the_same_edit() {
        let mut document = document_with_bodies(&["a"]);
        let edit = Edit::UpdateBody {
            body: Body::new(id("b-a"), "a"),
        };

        assert_eq!(document.apply(edit.clone()), Ok(edit));
    }

    #[test]
    fn updating_a_parameter_may_keep_its_own_name() {
        let mut document = Document::default();
        document
            .parameters
            .push(Parameter::new(id("p-a"), "a", "1"));

        let result = document.apply(Edit::UpdateParameter {
            parameter: Parameter::new(id("p-a"), "a", "2"),
        });

        assert!(result.is_ok());
    }

    #[test]
    fn removing_an_unknown_body_names_the_kind() {
        let mut document = Document::default();

        assert_eq!(
            document.apply(Edit::RemoveBody { id: id("b-9") }),
            Err(EditError::NotFound {
                kind: RecordKind::Body,
                id: id("b-9")
            })
        );
    }

    #[test]
    fn update_feature_normalizes_numbers() {
        let mut document = Document::default();
        document.features.push(FeatureRecord::new(id("f-1"), "box"));
        let mut record = FeatureRecord::new(id("f-1"), "box");
        record.params.insert("x".to_string(), json!(2.0));

        document
            .apply(Edit::UpdateFeature { feature: record })
            .expect("the feature exists");

        assert!(document.features[0].params["x"].is_u64());
    }

    #[test]
    fn the_first_reserved_key_is_reported() {
        let mut record = FeatureRecord::new(id("f-1"), "box");
        record.params.insert("name".to_string(), json!(1));
        record.params.insert("id".to_string(), json!(1));

        assert_eq!(
            check_feature(&record),
            Err(EditError::ReservedParamKey {
                key: "name".to_string()
            })
        );
    }

    #[test]
    fn apply_all_returns_normalized_edits() {
        let mut document = Document::default();
        let mut record = FeatureRecord::new(id("f-1"), "box");
        record.params.insert("x".to_string(), json!(2.0));

        let steps = document
            .apply_all(vec![Edit::AddFeature {
                index: 0,
                feature: record,
            }])
            .expect("the edit applies");

        let Edit::AddFeature { feature, .. } = &steps[0].edit else {
            panic!("expected AddFeature, got {:?}", steps[0].edit);
        };
        assert!(feature.params["x"].is_u64());
    }

    /// A batch whose second edit fails, so the first must be undone.
    fn failing_after(edit: Edit) -> Vec<Edit> {
        vec![
            edit,
            Edit::RemoveBody {
                id: id("b-missing"),
            },
        ]
    }

    fn assert_restored(document: &mut Document, edit: Edit) {
        let before = document.clone();

        let result = document.apply_all(failing_after(edit));

        assert_eq!(
            result,
            Err(TransactionError {
                index: 1,
                count: 2,
                error: EditError::NotFound {
                    kind: RecordKind::Body,
                    id: id("b-missing")
                }
            })
        );
        assert_eq!(*document, before);
    }

    #[test]
    fn a_failed_batch_restores_a_document_with_repeated_ids() {
        let mut document = Document {
            parameters: vec![
                Parameter::new(id("p-1"), "a", "1"),
                Parameter::new(id("p-1"), "b", "2"),
            ],
            ..Document::default()
        };

        assert_restored(&mut document, Edit::RemoveParameter { id: id("p-1") });
    }

    #[test]
    fn a_failed_batch_moves_the_right_record_back() {
        let mut document = Document {
            parameters: vec![
                Parameter::new(id("p-1"), "a", "1"),
                Parameter::new(id("p-1"), "b", "2"),
                Parameter::new(id("p-2"), "c", "3"),
            ],
            ..Document::default()
        };

        assert_restored(
            &mut document,
            Edit::MoveParameter {
                id: id("p-1"),
                index: 2,
            },
        );
    }

    #[test]
    fn a_failed_batch_restores_invalid_previous_records() {
        let mut record = FeatureRecord::new(id("f-1"), "Not A Type");
        record.params.insert("x".to_string(), json!(2.0));
        let mut document = Document {
            printer: Some(String::new()),
            bodies: vec![Body::new(id("b-1"), "")],
            features: vec![record],
            ..Document::default()
        };

        assert_restored(
            &mut document,
            Edit::SetPrinter {
                printer: Some("Printer".to_string()),
            },
        );
        assert_restored(
            &mut document,
            Edit::UpdateBody {
                body: Body::new(id("b-1"), "Base"),
            },
        );
        assert_restored(
            &mut document,
            Edit::UpdateFeature {
                feature: FeatureRecord::new(id("f-1"), "box"),
            },
        );
        assert!(document.features[0].params["x"].is_f64());
    }

    #[test]
    fn a_failed_batch_removes_a_feature_the_rollback_marker_names() {
        let mut document = Document {
            rollback: Some(id("f-1")),
            ..Document::default()
        };

        assert_restored(
            &mut document,
            Edit::AddFeature {
                index: 0,
                feature: FeatureRecord::new(id("f-1"), "box"),
            },
        );
    }

    #[test]
    fn a_failed_batch_undoes_every_edit_kind() {
        let mut document = document_with_bodies(&["a", "b"]);
        document
            .parameters
            .push(Parameter::new(id("p-1"), "a", "1"));
        document.features = vec![
            FeatureRecord::new(id("f-1"), "box"),
            FeatureRecord::new(id("f-2"), "box"),
        ];
        let before = document.clone();
        let edits = vec![
            Edit::AddParameter {
                index: 0,
                parameter: Parameter::new(id("p-2"), "b", "2"),
            },
            Edit::UpdateParameter {
                parameter: Parameter::new(id("p-1"), "c", "3"),
            },
            Edit::MoveParameter {
                id: id("p-1"),
                index: 0,
            },
            Edit::RemoveParameter { id: id("p-2") },
            Edit::AddBody {
                index: 2,
                body: Body::new(id("b-c"), "c"),
            },
            Edit::UpdateBody {
                body: Body::new(id("b-a"), "renamed"),
            },
            Edit::MoveBody {
                id: id("b-a"),
                index: 2,
            },
            Edit::RemoveBody { id: id("b-b") },
            Edit::AddFeature {
                index: 1,
                feature: FeatureRecord::new(id("f-3"), "cylinder"),
            },
            Edit::UpdateFeature {
                feature: FeatureRecord::new(id("f-1"), "sphere"),
            },
            Edit::MoveFeature {
                id: id("f-1"),
                index: 2,
            },
            Edit::RemoveFeature { id: id("f-2") },
            Edit::SetRollback {
                rollback: Some(id("f-3")),
            },
            Edit::SetUnits { units: Units::In },
            Edit::SetPrinter {
                printer: Some("Printer".to_string()),
            },
            Edit::RemoveBody {
                id: id("b-missing"),
            },
        ];

        let result = document.apply_all(edits);

        assert!(matches!(result, Err(TransactionError { index: 15, .. })));
        assert_eq!(document, before);
    }

    #[test]
    fn empty_batches_succeed() {
        let mut document = Document::default();

        assert_eq!(document.apply_all(Vec::new()), Ok(Vec::new()));
    }

    #[test]
    fn edit_error_messages() {
        assert_eq!(
            EditError::NotFound {
                kind: RecordKind::Feature,
                id: id("f-2")
            }
            .to_string(),
            "There is no feature with the id f-2."
        );
        assert_eq!(
            EditError::IndexOutOfRange { index: 5, max: 2 }.to_string(),
            "Position 5 is out of range. Use a position from 0 to 2."
        );
    }

    /// Asserts that `edit` deserializes, and that it no longer does once it has the field `extra`.
    fn assert_unknown_field_rejected(mut edit: Value) {
        if let Err(error) = serde_json::from_value::<Edit>(edit.clone()) {
            panic!("{edit} does not deserialize: {error}");
        }
        edit["extra"] = json!(1);

        match serde_json::from_value::<Edit>(edit.clone()) {
            Ok(read) => panic!("{edit} is accepted as {read:?}"),
            Err(error) => assert!(
                error.to_string().contains("unknown field `extra`"),
                "{edit}: {error}"
            ),
        }
    }

    #[test]
    fn edits_with_a_position_and_a_record_reject_unknown_fields() {
        assert_unknown_field_rejected(json!({
            "type": "add_parameter",
            "index": 0,
            "parameter": {"id": "p-1", "name": "a", "expr": "1"}
        }));
        assert_unknown_field_rejected(json!({
            "type": "add_body",
            "index": 0,
            "body": {"id": "b-1", "name": "Base"}
        }));
        assert_unknown_field_rejected(json!({
            "type": "add_feature",
            "index": 0,
            "feature": {"id": "f-1", "type": "box"}
        }));
    }

    #[test]
    fn edits_with_a_record_reject_unknown_fields() {
        assert_unknown_field_rejected(json!({
            "type": "update_parameter",
            "parameter": {"id": "p-1", "name": "a", "expr": "1"}
        }));
        assert_unknown_field_rejected(json!({
            "type": "update_body",
            "body": {"id": "b-1", "name": "Base"}
        }));
        assert_unknown_field_rejected(json!({
            "type": "update_feature",
            "feature": {"id": "f-1", "type": "box"}
        }));
    }

    #[test]
    fn edits_with_an_id_reject_unknown_fields() {
        assert_unknown_field_rejected(json!({"type": "remove_parameter", "id": "p-1"}));
        assert_unknown_field_rejected(json!({"type": "remove_body", "id": "b-1"}));
        assert_unknown_field_rejected(json!({"type": "remove_feature", "id": "f-1"}));
    }

    #[test]
    fn edits_with_an_id_and_a_position_reject_unknown_fields() {
        assert_unknown_field_rejected(json!({"type": "move_parameter", "id": "p-1", "index": 0}));
        assert_unknown_field_rejected(json!({"type": "move_body", "id": "b-1", "index": 0}));
        assert_unknown_field_rejected(json!({"type": "move_feature", "id": "f-1", "index": 0}));
    }

    #[test]
    fn edits_with_a_required_value_reject_unknown_fields() {
        assert_unknown_field_rejected(json!({"type": "set_units", "units": "in"}));
    }

    #[test]
    fn edits_with_an_optional_value_reject_unknown_fields() {
        assert_unknown_field_rejected(json!({"type": "set_rollback", "rollback": "f-1"}));
        assert_unknown_field_rejected(json!({"type": "set_printer", "printer": "generic-fdm"}));
    }

    #[test]
    fn edits_that_omit_their_optional_value_reject_unknown_fields() {
        assert_unknown_field_rejected(json!({"type": "set_rollback"}));
        assert_unknown_field_rejected(json!({"type": "set_printer"}));
    }

    #[test]
    fn a_rollback_named_by_an_unknown_field_is_rejected() {
        let error =
            serde_json::from_str::<Edit>(r#"{"type":"set_rollback","feature":"f-1","extra":1}"#)
                .expect_err("unknown fields are rejected");

        assert!(
            error.to_string().contains("unknown field `feature`"),
            "{error}"
        );
    }

    #[test]
    fn unknown_keys_of_a_feature_in_an_edit_stay_params() {
        let edit: Edit = serde_json::from_value(json!({
            "type": "add_feature",
            "index": 0,
            "feature": {"id": "f-1", "type": "box", "size": [1, 2, 3], "note": "x"}
        }))
        .expect("feature params are open");

        let Edit::AddFeature { feature, .. } = edit else {
            panic!("expected AddFeature, got {edit:?}");
        };
        assert_eq!(
            feature
                .params
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["size", "note"]
        );
    }

    #[test]
    fn the_edit_schema_closes_every_variant() {
        let schema = schemars::schema_for!(Edit);
        let variants = schema.as_value()["oneOf"]
            .as_array()
            .expect("Edit lists its variants");

        assert!(!variants.is_empty());
        for variant in variants {
            assert_eq!(variant["additionalProperties"], json!(false), "{variant}");
        }
    }
}
