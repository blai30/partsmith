//! Structural rules every document in memory satisfies (model.md section 3.6).

use std::collections::BTreeSet;

use crate::document::RecordProblem;
use crate::names::{LabelError, LabelField, NameError, validate_label, validate_parameter_name};
use crate::{Document, FORMAT, FORMAT_VERSION, Id};

/// A violation of the structural rules (model.md section 3.6).
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum StructureError {
    /// `format` is not "partsmith".
    #[error("The format field is '{found}', but a Partsmith document needs 'partsmith'.")]
    WrongFormat {
        /// The format found.
        found: String,
    },
    /// `format_version` is not FORMAT_VERSION.
    #[error(
        "The document is at format version {found}, but documents in memory must be at version {version}. Load it from text so it is migrated.",
        version = FORMAT_VERSION
    )]
    WrongFormatVersion {
        /// The version found.
        found: u32,
    },
    /// An id appears on more than one parameter, body or feature; reported at each repeat.
    #[error("The id {id} is used by more than one record. Give the later record a new id.")]
    DuplicateId {
        /// The repeated id.
        id: Id,
    },
    /// A parameter name breaks the name rules.
    #[error("Parameter {id}: {error}")]
    InvalidParameterName {
        /// The parameter.
        id: Id,
        /// What is wrong with its name.
        error: NameError,
    },
    /// A parameter name is already used by an earlier parameter.
    #[error(
        "Parameter {id} is named {name}, but an earlier parameter already uses that name. Rename one of them."
    )]
    DuplicateParameterName {
        /// The later parameter.
        id: Id,
        /// The shared name.
        name: String,
    },
    /// A body name, material, feature name or the printer breaks the label rules; `id` is `None` for the printer.
    #[error("{}", label_message(.id.as_ref(), .error))]
    InvalidLabel {
        /// The body or feature, or `None` for the printer.
        id: Option<Id>,
        /// What is wrong with the label.
        error: LabelError,
    },
    /// A feature type name breaks the type rules.
    #[error(
        "Feature {id} has the type '{kind}', which is not a valid type name. Type names use lowercase letters, digits and _, and start with a letter."
    )]
    InvalidFeatureType {
        /// The feature.
        id: Id,
        /// Its type name.
        kind: String,
    },
    /// Feature params contain a reserved key.
    #[error(
        "Feature {id} has a parameter named '{key}', which the record itself uses. Rename that parameter."
    )]
    ReservedParamKey {
        /// The feature.
        id: Id,
        /// The reserved key.
        key: String,
    },
    /// `rollback` names no feature.
    #[error(
        "The rollback marker points at {id}, which is not a feature in this document. Move the marker to an existing feature or to the end."
    )]
    UnknownRollback {
        /// The id the marker names.
        id: Id,
    },
}

/// Reports `id` as a `DuplicateId` when an earlier record of the walk used it.
fn check_repeat<'a>(
    seen_ids: &mut BTreeSet<&'a Id>,
    id: &'a Id,
    problems: &mut Vec<StructureError>,
) {
    if !seen_ids.insert(id) {
        problems.push(StructureError::DuplicateId { id: id.clone() });
    }
}

/// `{id}: {error}`, or `{error}` alone for the printer.
fn label_message(id: Option<&Id>, error: &LabelError) -> String {
    match id {
        Some(id) => format!("{id}: {error}"),
        None => error.to_string(),
    }
}

impl Document {
    /// Structural problems (model.md section 3.6) in a fixed order; empty for a structurally valid document.
    pub fn check_structure(&self) -> Vec<StructureError> {
        let mut problems = Vec::new();
        if self.format != FORMAT {
            problems.push(StructureError::WrongFormat {
                found: self.format.clone(),
            });
        }
        if self.format_version != FORMAT_VERSION {
            problems.push(StructureError::WrongFormatVersion {
                found: self.format_version,
            });
        }
        if let Some(printer) = &self.printer
            && let Err(error) = validate_label(LabelField::Printer, printer)
        {
            problems.push(StructureError::InvalidLabel { id: None, error });
        }

        let mut seen_ids: BTreeSet<&Id> = BTreeSet::new();
        let mut seen_names: BTreeSet<&str> = BTreeSet::new();
        for parameter in &self.parameters {
            check_repeat(&mut seen_ids, &parameter.id, &mut problems);
            if let Err(error) = validate_parameter_name(&parameter.name) {
                problems.push(StructureError::InvalidParameterName {
                    id: parameter.id.clone(),
                    error,
                });
            } else if seen_names.contains(parameter.name.as_str()) {
                problems.push(StructureError::DuplicateParameterName {
                    id: parameter.id.clone(),
                    name: parameter.name.clone(),
                });
            }
            seen_names.insert(&parameter.name);
        }
        for body in &self.bodies {
            check_repeat(&mut seen_ids, &body.id, &mut problems);
            problems.extend(
                body.label_errors()
                    .map(|error| StructureError::InvalidLabel {
                        id: Some(body.id.clone()),
                        error,
                    }),
            );
        }
        for feature in &self.features {
            check_repeat(&mut seen_ids, &feature.id, &mut problems);
            problems.extend(feature.problems().map(|problem| match problem {
                RecordProblem::InvalidType => StructureError::InvalidFeatureType {
                    id: feature.id.clone(),
                    kind: feature.kind.clone(),
                },
                RecordProblem::InvalidName(error) => StructureError::InvalidLabel {
                    id: Some(feature.id.clone()),
                    error,
                },
                RecordProblem::ReservedKey(key) => StructureError::ReservedParamKey {
                    id: feature.id.clone(),
                    key: key.clone(),
                },
            }));
        }
        if let Some(rollback) = &self.rollback
            && self.feature(rollback).is_none()
        {
            problems.push(StructureError::UnknownRollback {
                id: rollback.clone(),
            });
        }
        problems
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{Body, FeatureRecord, Parameter};

    fn id(text: &str) -> Id {
        Id::new(text).expect("valid id")
    }

    #[test]
    fn every_repeat_of_an_id_is_reported_once() {
        let mut document = Document::default();
        document.parameters.push(Parameter::new(id("x"), "a", "1"));
        document.bodies.push(Body::new(id("x"), "Base"));
        document.features.push(FeatureRecord::new(id("x"), "box"));

        assert_eq!(
            document.check_structure(),
            [
                StructureError::DuplicateId { id: id("x") },
                StructureError::DuplicateId { id: id("x") }
            ]
        );
    }

    #[test]
    fn an_invalid_name_is_not_also_a_duplicate() {
        let mut document = Document::default();
        document
            .parameters
            .push(Parameter::new(id("p-1"), "pi", "1"));
        document
            .parameters
            .push(Parameter::new(id("p-2"), "pi", "1"));

        assert_eq!(document.check_structure().len(), 2);
        assert!(
            document
                .check_structure()
                .iter()
                .all(|problem| matches!(problem, StructureError::InvalidParameterName { .. }))
        );
    }

    #[test]
    fn invalid_feature_names_are_labels() {
        let mut document = Document::default();
        let mut record = FeatureRecord::new(id("f-1"), "box");
        record.name = Some("a\nb".to_string());
        document.features.push(record);

        assert_eq!(
            document.check_structure(),
            [StructureError::InvalidLabel {
                id: Some(id("f-1")),
                error: LabelError::ControlCharacter {
                    field: LabelField::FeatureName
                }
            }]
        );
    }

    #[test]
    fn body_name_and_material_are_both_checked() {
        let mut document = Document::default();
        let mut body = Body::new(id("b-1"), "");
        body.material = Some(" ".to_string());
        document.bodies.push(body);

        assert_eq!(document.check_structure().len(), 2);
    }

    #[test]
    fn reserved_keys_come_after_the_type_and_name() {
        let mut document = Document::default();
        let mut record = FeatureRecord::new(id("f-1"), "Box");
        record.params.insert("id".to_string(), json!(1));
        document.features.push(record);

        assert_eq!(
            document.check_structure(),
            [
                StructureError::InvalidFeatureType {
                    id: id("f-1"),
                    kind: "Box".to_string()
                },
                StructureError::ReservedParamKey {
                    id: id("f-1"),
                    key: "id".to_string()
                }
            ]
        );
    }

    #[test]
    fn label_messages_lead_with_the_record_id() {
        let error = StructureError::InvalidLabel {
            id: Some(id("b-1")),
            error: LabelError::Empty {
                field: LabelField::BodyName,
            },
        };
        let printer = StructureError::InvalidLabel {
            id: None,
            error: LabelError::Empty {
                field: LabelField::Printer,
            },
        };

        assert_eq!(error.to_string(), "b-1: Body name cannot be empty.");
        assert_eq!(printer.to_string(), "Printer cannot be empty.");
    }

    #[test]
    fn invalid_parameter_name_message_names_the_parameter() {
        let error = StructureError::InvalidParameterName {
            id: id("p-1"),
            error: NameError::Empty,
        };

        assert_eq!(
            error.to_string(),
            "Parameter p-1: A parameter name cannot be empty."
        );
    }

    #[test]
    fn wrong_format_version_names_the_current_version() {
        let error = StructureError::WrongFormatVersion { found: 0 };

        assert!(
            error
                .to_string()
                .contains(&format!("must be at version {FORMAT_VERSION}.")),
            "{error}"
        );
    }
}
