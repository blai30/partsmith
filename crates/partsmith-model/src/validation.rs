//! Full document validation: structure, references, expressions and the parameter graph (model.md section 3.7).

use std::collections::{BTreeMap, BTreeSet};

use crate::expr::{ParameterGraph, cycle_sentence};
use crate::{
    Document, Expr, ExprError, FeatureInspector, Id, ParsedExpr, References, StructureError,
};

/// A problem found by `Document::validate`.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ValidationError {
    /// A structural problem.
    #[error("{0}")]
    Structure(StructureError),
    /// The inspector rejected the feature's params.
    #[error("Feature {feature}: {message}")]
    InvalidFeature {
        /// The feature.
        feature: Id,
        /// The inspector's message.
        message: String,
    },
    /// An id declared inside feature params is already used elsewhere in the document.
    #[error(
        "Feature {feature} defines the id {id}, which is already used elsewhere in the document. Give it a new id."
    )]
    DuplicateDeclaredId {
        /// The feature whose params declare the id.
        feature: Id,
        /// The declared id.
        id: Id,
    },
    /// The feature refers to a body that does not exist.
    #[error(
        "Feature {feature} uses body {body}, which does not exist. Pick an existing body or add one."
    )]
    UnknownBody {
        /// The feature.
        feature: Id,
        /// The missing body.
        body: Id,
    },
    /// The feature refers to a feature that does not exist.
    #[error(
        "Feature {feature} refers to {target}, which does not exist. Pick another input or remove the reference."
    )]
    UnknownFeature {
        /// The feature.
        feature: Id,
        /// The missing feature.
        target: Id,
    },
    /// The feature refers to itself.
    #[error("Feature {feature} refers to itself. Remove that reference.")]
    SelfReference {
        /// The feature.
        feature: Id,
    },
    /// The feature refers to a feature later in the timeline.
    #[error(
        "Feature {feature} refers to {target}, which comes after it in the timeline. Move {target} above {feature}."
    )]
    LaterFeature {
        /// The feature.
        feature: Id,
        /// The later feature.
        target: Id,
    },
    /// An expression of a parameter or feature does not parse.
    #[error("{owner}: the expression '{expr}' is invalid. {error}")]
    InvalidExpression {
        /// The parameter or feature holding the expression.
        owner: Id,
        /// The expression.
        expr: Expr,
        /// Why it does not parse.
        error: ExprError,
    },
    /// An expression uses a name that is not a parameter.
    #[error(
        "{owner} uses '{name}', but no parameter has that name. Add the parameter or fix the spelling."
    )]
    UnknownParameter {
        /// The parameter or feature holding the expression.
        owner: Id,
        /// The unknown name.
        name: String,
    },
    /// Parameters that depend on each other in a loop, in document order.
    #[error("{}", cycle_message(.names))]
    ParameterCycle {
        /// The parameter names.
        names: Vec<String>,
    },
}

/// The message of `ParameterCycle`: the cycle sentence after "Parameter" or "Parameters".
fn cycle_message(names: &[String]) -> String {
    let noun = if names.len() == 1 {
        "Parameter"
    } else {
        "Parameters"
    };
    format!("{noun} {}", cycle_sentence(names))
}

impl Document {
    /// Every id in use: parameter, body and feature ids plus ids declared inside feature params.
    pub fn used_ids(&self, inspector: &dyn FeatureInspector) -> BTreeSet<Id> {
        let mut ids = self.record_ids();
        ids.extend(
            self.features
                .iter()
                .filter_map(|feature| inspector.inspect(feature).ok())
                .flat_map(|links| links.declared_ids),
        );
        ids
    }

    /// Structural problems plus reference, expression and parameter-graph problems (model.md section 3.7).
    pub fn validate(&self, inspector: &dyn FeatureInspector) -> Vec<ValidationError> {
        let mut problems: Vec<ValidationError> = self
            .check_structure()
            .into_iter()
            .map(ValidationError::Structure)
            .collect();
        let graph = ParameterGraph::new(&self.parameters);
        for (parameter, parsed) in self.parameters.iter().zip(graph.parsed()) {
            let references = parsed
                .as_ref()
                .map(ParsedExpr::references)
                .map_err(ExprError::clone);
            expression_problems(
                &graph,
                &parameter.id,
                &parameter.expr,
                references,
                &mut problems,
            );
        }
        problems.extend(self.cycle_problems(&graph));
        self.feature_link_problems(inspector, &graph, &mut problems);
        problems
    }

    /// One `ParameterCycle` per loop of parameters, ordered by their first position.
    fn cycle_problems(&self, graph: &ParameterGraph<'_>) -> impl Iterator<Item = ValidationError> {
        graph
            .cycles()
            .into_iter()
            .map(|cycle| ValidationError::ParameterCycle {
                names: cycle
                    .into_iter()
                    .map(|position| self.parameters[position].name.clone())
                    .collect(),
            })
    }

    /// Problems of each feature's links in timeline order: params the inspector rejects, then
    /// repeated declared ids, unknown bodies, bad feature references and bad expressions.
    fn feature_link_problems(
        &self,
        inspector: &dyn FeatureInspector,
        graph: &ParameterGraph<'_>,
        problems: &mut Vec<ValidationError>,
    ) {
        // Every lookup below goes through these indexes, built once, so checking the links of
        // every feature takes O(n log n) rather than a scan of the record lists per link.
        // `taken` starts with the record ids and gains each declared id as it is met.
        let mut taken = self.record_ids();
        let body_ids: BTreeSet<&Id> = self.bodies.iter().map(|body| &body.id).collect();
        let mut feature_positions: BTreeMap<&Id, usize> = BTreeMap::new();
        for (position, feature) in self.features.iter().enumerate() {
            // The first position of a repeated id, like `Document::feature_index`.
            feature_positions.entry(&feature.id).or_insert(position);
        }
        for (position, feature) in self.features.iter().enumerate() {
            let links = match inspector.inspect(feature) {
                Ok(links) => links,
                Err(message) => {
                    problems.push(ValidationError::InvalidFeature {
                        feature: feature.id.clone(),
                        message,
                    });
                    continue;
                }
            };
            for id in links.declared_ids {
                // `replace` hands back the equal id that was already taken, if any.
                if let Some(repeated) = taken.replace(id) {
                    problems.push(ValidationError::DuplicateDeclaredId {
                        feature: feature.id.clone(),
                        id: repeated,
                    });
                }
            }
            for body in links.bodies {
                if !body_ids.contains(&body) {
                    problems.push(ValidationError::UnknownBody {
                        feature: feature.id.clone(),
                        body,
                    });
                }
            }
            problems.extend(links.features.into_iter().filter_map(|target| {
                feature_reference_problem(&feature.id, position, target, &feature_positions)
            }));
            for link in links.expressions {
                let references = link.expr.references();
                expression_problems(graph, &feature.id, &link.expr, references, problems);
            }
        }
    }

    /// The ids of every parameter, body and feature.
    fn record_ids(&self) -> BTreeSet<Id> {
        self.parameters
            .iter()
            .map(|parameter| parameter.id.clone())
            .chain(self.bodies.iter().map(|body| body.id.clone()))
            .chain(self.features.iter().map(|feature| feature.id.clone()))
            .collect()
    }
}

/// The problem with feature `feature` at `position` referring to `target`: itself, a missing
/// feature or a later one; `None` for an earlier feature.
fn feature_reference_problem(
    feature: &Id,
    position: usize,
    target: Id,
    feature_positions: &BTreeMap<&Id, usize>,
) -> Option<ValidationError> {
    if target == *feature {
        return Some(ValidationError::SelfReference {
            feature: feature.clone(),
        });
    }
    match feature_positions.get(&target).copied() {
        None => Some(ValidationError::UnknownFeature {
            feature: feature.clone(),
            target,
        }),
        Some(index) if index > position => Some(ValidationError::LaterFeature {
            feature: feature.clone(),
            target,
        }),
        Some(_) => None,
    }
}

/// Reports an expression of `owner` that does not parse, or else each name it uses that is not
/// the name of a parameter.
fn expression_problems(
    graph: &ParameterGraph<'_>,
    owner: &Id,
    expr: &Expr,
    references: Result<References, ExprError>,
    problems: &mut Vec<ValidationError>,
) {
    match references {
        Ok(references) => {
            problems.extend(
                references
                    .parameters
                    .into_iter()
                    .filter(|name| !graph.has_parameter(name))
                    .map(|name| ValidationError::UnknownParameter {
                        owner: owner.clone(),
                        name,
                    }),
            );
        }
        Err(error) => problems.push(ValidationError::InvalidExpression {
            owner: owner.clone(),
            expr: expr.clone(),
            error,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExprLink, FeatureLinks, FeatureRecord, OpaqueFeatures, Parameter};

    fn id(text: &str) -> Id {
        Id::new(text).expect("valid id")
    }

    /// Declares `e-1` twice for every record.
    struct RepeatingInspector;

    impl FeatureInspector for RepeatingInspector {
        fn inspect(&self, _feature: &FeatureRecord) -> Result<FeatureLinks, String> {
            Ok(FeatureLinks {
                declared_ids: vec![id("e-1"), id("e-1")],
                expressions: vec![ExprLink {
                    pointer: "/x".to_string(),
                    expr: Expr::new("mm + 1"),
                }],
                ..FeatureLinks::default()
            })
        }
    }

    #[test]
    fn a_repeat_within_one_feature_is_a_duplicate() {
        let mut document = Document::default();
        document.features.push(FeatureRecord::new(id("f-1"), "box"));

        assert_eq!(
            document.validate(&RepeatingInspector),
            [ValidationError::DuplicateDeclaredId {
                feature: id("f-1"),
                id: id("e-1")
            }]
        );
    }

    /// Reports `declares` and `uses` (string arrays) and `body` (a string) from the params.
    struct LinkInspector;

    impl FeatureInspector for LinkInspector {
        fn inspect(&self, feature: &FeatureRecord) -> Result<FeatureLinks, String> {
            let ids = |key: &str| -> Vec<Id> {
                feature
                    .params
                    .get(key)
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(serde_json::Value::as_str)
                    .map(id)
                    .collect()
            };
            Ok(FeatureLinks {
                declared_ids: ids("declares"),
                bodies: feature
                    .params
                    .get("body")
                    .and_then(serde_json::Value::as_str)
                    .map(id)
                    .into_iter()
                    .collect(),
                features: ids("uses"),
                expressions: Vec::new(),
            })
        }
    }

    fn linked_feature(id_text: &str, params: serde_json::Value) -> FeatureRecord {
        let mut record = FeatureRecord::new(id(id_text), "box");
        let serde_json::Value::Object(params) = params else {
            panic!("params must be an object");
        };
        record.params = params;
        record
    }

    #[test]
    fn declared_ids_collide_with_every_record_kind() {
        let document = Document {
            parameters: vec![Parameter::new(id("p-1"), "a", "1")],
            bodies: vec![crate::Body::new(id("b-1"), "Base")],
            features: vec![linked_feature(
                "f-1",
                serde_json::json!({"declares": ["p-1", "b-1", "f-1", "e-1"], "body": "b-2"}),
            )],
            ..Document::default()
        };

        assert_eq!(
            document.validate(&LinkInspector),
            [
                ValidationError::DuplicateDeclaredId {
                    feature: id("f-1"),
                    id: id("p-1")
                },
                ValidationError::DuplicateDeclaredId {
                    feature: id("f-1"),
                    id: id("b-1")
                },
                ValidationError::DuplicateDeclaredId {
                    feature: id("f-1"),
                    id: id("f-1")
                },
                ValidationError::UnknownBody {
                    feature: id("f-1"),
                    body: id("b-2")
                }
            ]
        );
    }

    #[test]
    fn a_repeated_feature_id_is_found_at_its_first_position() {
        let earlier = Document {
            features: vec![
                linked_feature("f-1", serde_json::json!({})),
                linked_feature("f-2", serde_json::json!({"uses": ["f-1"]})),
                linked_feature("f-1", serde_json::json!({})),
            ],
            ..Document::default()
        };
        let later = Document {
            features: vec![
                linked_feature("f-2", serde_json::json!({"uses": ["f-1"]})),
                linked_feature("f-1", serde_json::json!({})),
                linked_feature("f-1", serde_json::json!({})),
            ],
            ..Document::default()
        };

        assert!(
            !earlier
                .validate(&LinkInspector)
                .iter()
                .any(|problem| matches!(problem, ValidationError::LaterFeature { .. }))
        );
        assert!(
            later
                .validate(&LinkInspector)
                .contains(&ValidationError::LaterFeature {
                    feature: id("f-2"),
                    target: id("f-1")
                })
        );
    }

    #[test]
    fn used_ids_skip_features_the_inspector_rejects() {
        struct Rejecting;
        impl FeatureInspector for Rejecting {
            fn inspect(&self, _feature: &FeatureRecord) -> Result<FeatureLinks, String> {
                Err("No.".to_string())
            }
        }
        let mut document = Document::default();
        document.features.push(FeatureRecord::new(id("f-1"), "box"));

        assert_eq!(document.used_ids(&Rejecting), BTreeSet::from([id("f-1")]));
    }

    #[test]
    fn cycles_are_reported_after_every_parameter_expression() {
        let document = Document {
            parameters: vec![
                Parameter::new(id("p-a"), "a", "b"),
                Parameter::new(id("p-b"), "b", "a + c"),
            ],
            ..Document::default()
        };

        assert_eq!(
            document.validate(&OpaqueFeatures),
            [
                ValidationError::UnknownParameter {
                    owner: id("p-b"),
                    name: "c".to_string()
                },
                ValidationError::ParameterCycle {
                    names: vec!["a".to_string(), "b".to_string()]
                }
            ]
        );
    }

    #[test]
    fn cycle_messages() {
        let one = ValidationError::ParameterCycle {
            names: vec!["a".to_string()],
        };
        let three = ValidationError::ParameterCycle {
            names: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        };

        assert_eq!(
            one.to_string(),
            "Parameter a depends on itself. Change its expression so it does not use a."
        );
        assert_eq!(
            three.to_string(),
            "Parameters a, b and c depend on each other in a loop. Change one of them so it does not use the others."
        );
    }

    #[test]
    fn invalid_expression_message_quotes_the_expression() {
        let error = ValidationError::InvalidExpression {
            owner: id("p-1"),
            expr: Expr::new("1 +"),
            error: Expr::new("1 +").parse().expect_err("incomplete"),
        };

        assert_eq!(
            error.to_string(),
            "p-1: the expression '1 +' is invalid. The expression ends too early. Add the missing value."
        );
    }
}
