//! The `chamfer` feature: bevels edges of a body with a flat face (features/chamfer.md).

use partsmith_model::{
    Category, Expr, FeatureManifest, FieldHint, FieldUnit, Id, OptionHint, SelectionKind,
};
use partsmith_naming::{ElementKind, NamingMap, Reference, roles};
use partsmith_occt_sys::{ChamferEdge, ChamferSize, Topology, chamfer};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::dressup::{
    internal, kernel, modified, name_blend, python_references, run_or_search, select_edges,
    smallest_name, unchanged,
};
use crate::{
    Bound, BuildContext, Dependencies, Feature, FeatureError, FeatureOutput, FieldRef, Noun,
    PythonCall, PythonNames, PythonValue,
};

/// Bevels edges of a body.
pub struct Chamfer;

/// Params of a chamfer feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ChamferParams {
    /// The body whose edges are beveled.
    pub body: Id,
    /// Edges to bevel, at least one.
    pub edges: Vec<Reference>,
    /// Distances from each edge.
    pub distance: ChamferDistance,
}

/// How far a chamfer cuts back on each face of an edge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChamferDistance {
    /// `expr` on both faces. Length, greater than 0.
    Equal {
        /// The distance on both faces.
        expr: Expr,
    },
    /// `expr` on the first face and `second_expr` on the second face of each edge, where the first
    /// face is the one whose element name sorts first. Lengths, greater than 0. `flip` swaps them.
    TwoDistances {
        /// The distance on the first face.
        expr: Expr,
        /// The distance on the second face.
        second_expr: Expr,
        /// Swaps the two distances for every edge. Default false, omitted when false.
        #[serde(default, skip_serializing_if = "is_false")]
        flip: bool,
    },
}

/// True for `false`; for `skip_serializing_if`.
fn is_false(value: &bool) -> bool {
    !*value
}

/// The evaluated distances of a chamfer, in millimeters.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Distances {
    /// The same distance on both faces.
    Equal(f64),
    /// `on_first` on the face whose name sorts first, `on_other` on the other face (flip applied).
    TwoDistances {
        /// The distance on the first face.
        on_first: f64,
        /// The distance on the other face.
        on_other: f64,
    },
}

impl Distances {
    /// The larger distance, which the "too large" search scales (features.md section 4.3).
    fn larger(self) -> f64 {
        match self {
            Distances::Equal(distance) => distance,
            Distances::TwoDistances { on_first, on_other } => on_first.max(on_other),
        }
    }

    /// The kernel size of one edge with both distances multiplied by `scale`, where `first_face`
    /// is the edge's face that sorts first.
    fn size(self, scale: f64, first_face: u32) -> ChamferSize {
        match self {
            Distances::Equal(distance) => ChamferSize::Equal {
                distance: distance * scale,
            },
            Distances::TwoDistances { on_first, on_other } => ChamferSize::TwoDistances {
                distance: on_first * scale,
                other_distance: on_other * scale,
                face: first_face,
            },
        }
    }
}

impl Feature for Chamfer {
    const TYPE: &'static str = "chamfer";
    type Params = ChamferParams;

    fn manifest() -> FeatureManifest {
        let hint = |path: &str, label: &str, essential: bool| FieldHint {
            path: path.to_string(),
            label: label.to_string(),
            essential,
            unit: None,
            selection: None,
            options: Vec::new(),
        };
        let option = |value: &str, label: &str| OptionHint {
            value: value.to_string(),
            label: label.to_string(),
        };
        FeatureManifest {
            label: "Chamfer".to_string(),
            category: Category::Modify,
            icon: "chamfer".to_string(),
            version: 1,
            fields: vec![
                FieldHint {
                    selection: Some(SelectionKind::Edge),
                    ..hint("edges", "Edges", true)
                },
                FieldHint {
                    options: vec![
                        option("equal", "Equal distance"),
                        option("two_distances", "Two distances"),
                    ],
                    ..hint("distance.kind", "Type", true)
                },
                FieldHint {
                    unit: Some(FieldUnit::Length),
                    ..hint("distance.expr", "Distance", true)
                },
                FieldHint {
                    unit: Some(FieldUnit::Length),
                    ..hint("distance.second_expr", "Second distance", true)
                },
                hint("distance.flip", "Flip", false),
                FieldHint {
                    selection: Some(SelectionKind::Body),
                    ..hint("body", "Body", false)
                },
            ],
        }
    }

    fn dependencies(params: &ChamferParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_body_written(&params.body, "body");
        match &params.distance {
            ChamferDistance::Equal { expr } => {
                dependencies.add_expression("distance.expr", expr, FieldUnit::Length);
            }
            ChamferDistance::TwoDistances {
                expr, second_expr, ..
            } => {
                dependencies.add_expression("distance.expr", expr, FieldUnit::Length);
                dependencies.add_expression("distance.second_expr", second_expr, FieldUnit::Length);
            }
        }
        dependencies
    }

    fn validate(params: &ChamferParams) -> Result<(), FeatureError> {
        if params.edges.is_empty() {
            return Err(FeatureError::EmptySelection {
                field: FieldRef::new("edges", &Self::manifest()),
                noun: Noun::Edge,
            });
        }
        Ok(())
    }

    fn build(
        params: &ChamferParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let state = context.body(&params.body)?;
        let Some(shape) = state.shape() else {
            return Err(FeatureError::BodyEmpty {
                body: params.body.clone(),
                use_new: false,
            });
        };
        let (distances, field) = match &params.distance {
            ChamferDistance::Equal { expr } => (
                Distances::Equal(positive_length(context, "distance.expr", expr)?),
                "distance.expr",
            ),
            ChamferDistance::TwoDistances {
                expr,
                second_expr,
                flip,
            } => {
                let first = positive_length(context, "distance.expr", expr)?;
                let second = positive_length(context, "distance.second_expr", second_expr)?;
                let (on_first, on_other) = if *flip {
                    (second, first)
                } else {
                    (first, second)
                };
                (
                    Distances::TwoDistances { on_first, on_other },
                    "distance",
                )
            }
        };
        let selection = select_edges(context, &params.body, shape, &params.edges)?;
        if selection.edges.is_empty() {
            return Ok(unchanged(&params.body, state, selection.warnings));
        }
        let first_faces = match distances {
            Distances::Equal(_) => vec![0; selection.edges.len()],
            Distances::TwoDistances { .. } => {
                let topology = shape.topology().map_err(kernel)?;
                selection
                    .edges
                    .iter()
                    .map(|&edge| first_face(&topology, state.naming(), edge))
                    .collect::<Result<_, FeatureError>>()?
            }
        };
        let larger = distances.larger();
        let result = run_or_search(
            context,
            larger,
            |value| {
                let edges: Vec<ChamferEdge> = selection
                    .edges
                    .iter()
                    .zip(&first_faces)
                    .map(|(&edge, &face)| ChamferEdge {
                        edge,
                        size: distances.size(value / larger, face),
                    })
                    .collect();
                chamfer(shape, &edges)
            },
            |max, cause| FeatureError::ChamferTooLarge {
                field: context.field(field),
                distance: larger,
                max,
                cause,
            },
        )?;
        let naming = name_blend(context.feature(), roles::CHAMFER, state.naming(), &result)?;
        Ok(modified(
            &params.body,
            result.shape,
            naming,
            selection.warnings,
        ))
    }

    /// `chamfer(edges, distance)` for an equal distance: the edges as references, the distance as
    /// an expression; the generator's `with <part>:` block gives the body. A two-distance chamfer
    /// has no dedicated form.
    fn python(params: &ChamferParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let ChamferDistance::Equal { expr } = &params.distance else {
            return None;
        };
        let mut call = PythonCall::new("chamfer");
        call.positional = vec![
            python_references(&params.edges),
            PythonValue::expression(expr, names),
        ];
        Some(call)
    }
}

/// The declared length at `field`, which must be greater than 0 (chamfer.md section 3.3 step 2).
fn positive_length(
    context: &BuildContext<'_>,
    field: &str,
    expr: &Expr,
) -> Result<f64, FeatureError> {
    let value = context.eval_length(field, expr)?;
    if value <= 0.0 {
        return Err(FeatureError::OutOfRange {
            field: context.field(field),
            kind: FieldUnit::Length,
            value,
            bound: Bound::GreaterThan(0.0),
            bound_name: None,
        });
    }
    Ok(value)
}

/// The face next to `edge` whose element name sorts first in byte order: a two-distance chamfer
/// measures its first distance on it (chamfer.md section 3.3 step 4).
fn first_face(topology: &Topology, naming: &NamingMap, edge: u32) -> Result<u32, FeatureError> {
    let faces = edge
        .checked_sub(1)
        .and_then(|position| topology.edges.get(position as usize))
        .map(|entry| entry.faces.as_slice())
        .ok_or_else(|| internal(format!("edge {edge} is not in the body's topology")))?;
    let first = smallest_name(naming, ElementKind::Face, faces)
        .ok_or_else(|| internal(format!("edge {edge} has no named face")))?;
    naming
        .index(&first)
        .ok_or_else(|| internal(format!("the face {first} has no index")))
}

#[cfg(test)]
mod tests {
    use partsmith_model::{FeatureRecord, Units};
    use partsmith_naming::ElementName;
    use serde_json::json;

    use super::*;
    use crate::builtin::dressup::test_fixtures::{ScriptNames, reference};
    use crate::{Registry, test_support::id, test_support::named_box};

    /// `E-FT` of fillet.md section 6.
    const FRONT_TOP: &str =
        r#"{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}"#;

    fn params(edges: &[&str], distance: ChamferDistance) -> ChamferParams {
        ChamferParams {
            body: id("b-1"),
            edges: edges.iter().map(|edge| reference(edge)).collect(),
            distance,
        }
    }

    fn equal(expr: &str) -> ChamferDistance {
        ChamferDistance::Equal {
            expr: Expr::new(expr),
        }
    }

    fn two_distances(expr: &str, second_expr: &str, flip: bool) -> ChamferDistance {
        ChamferDistance::TwoDistances {
            expr: Expr::new(expr),
            second_expr: Expr::new(second_expr),
            flip,
        }
    }

    fn registry() -> Registry {
        let mut registry = Registry::new();
        registry
            .register::<Chamfer>()
            .expect("the chamfer passes the registration checks");
        registry
    }

    fn canonical(params: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        let mut record = json!({"id": "f-cha1", "type": "chamfer"});
        record
            .as_object_mut()
            .expect("an object")
            .extend(params.as_object().expect("an object").clone());
        let record: FeatureRecord = serde_json::from_value(record).expect("a record");
        registry()
            .validate_and_canonicalize(&record)
            .expect("valid")
            .record
            .params
    }

    #[test]
    fn the_manifest_matches_the_params() {
        let registry = registry();

        let manifest = registry.get("chamfer").expect("registered").manifest();

        assert_eq!(manifest.label, "Chamfer");
        assert_eq!(manifest.category, Category::Modify);
        assert_eq!(manifest.icon, "chamfer");
        assert_eq!(manifest.version, 1);
    }

    #[test]
    fn an_unflipped_two_distance_chamfer_omits_flip() {
        let params = canonical(json!({
            "body": "b-1", "edges": [],
            "distance": {"second_expr": "3 mm", "flip": false, "expr": "1 mm", "kind": "two_distances"}
        }));

        assert_eq!(
            serde_json::Value::Object(params["distance"].as_object().expect("an object").clone()),
            json!({"kind": "two_distances", "expr": "1 mm", "second_expr": "3 mm"})
        );
    }

    #[test]
    fn a_flipped_two_distance_chamfer_keeps_flip() {
        let params = canonical(json!({
            "body": "b-1", "edges": [],
            "distance": {"kind": "two_distances", "expr": "1 mm", "second_expr": "3 mm", "flip": true}
        }));

        let keys: Vec<&str> = params["distance"]
            .as_object()
            .expect("an object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["kind", "expr", "second_expr", "flip"]);
    }

    #[test]
    fn an_unknown_distance_kind_is_invalid() {
        let record: FeatureRecord = serde_json::from_value(json!({
            "id": "f-cha1", "type": "chamfer", "body": "b-1", "edges": [],
            "distance": {"kind": "angle", "expr": "1 mm"}
        }))
        .expect("a record");

        let error = registry()
            .validate_and_canonicalize(&record)
            .expect_err("angle is not a distance kind");

        assert_eq!(error.code(), "invalid_params");
    }

    #[test]
    fn no_edges_is_an_empty_selection() {
        let error = Chamfer::validate(&params(&[], equal("1"))).expect_err("no edges");

        assert_eq!(error.to_string(), "Select at least one edge for Edges.");
    }

    #[test]
    fn an_equal_distance_declares_one_length() {
        let dependencies = Chamfer::dependencies(&params(&[FRONT_TOP], equal("1 mm")));

        assert_eq!(dependencies.bodies_written[0].id, id("b-1"));
        let uses: Vec<(&str, &str)> = dependencies
            .expressions
            .iter()
            .map(|usage| (usage.field.as_str(), usage.expr.as_str()))
            .collect();
        assert_eq!(uses, [("distance.expr", "1 mm")]);
    }

    #[test]
    fn two_distances_declare_both_lengths_in_field_order() {
        let dependencies =
            Chamfer::dependencies(&params(&[FRONT_TOP], two_distances("1", "3", true)));

        let uses: Vec<(&str, &str, FieldUnit)> = dependencies
            .expressions
            .iter()
            .map(|usage| (usage.field.as_str(), usage.expr.as_str(), usage.kind))
            .collect();
        assert_eq!(
            uses,
            [
                ("distance.expr", "1", FieldUnit::Length),
                ("distance.second_expr", "3", FieldUnit::Length),
            ]
        );
    }

    #[test]
    fn the_first_face_of_an_edge_is_the_one_whose_name_sorts_first() {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let topology = cube.shape.topology().expect("topology");
        let index = |name: &str| {
            cube.naming
                .index(&ElementName::parse(name).expect("valid"))
                .expect("named")
        };
        let front_top = index("edge(f-box1/front|f-box1/top)");

        let first = first_face(&topology, &cube.naming, front_top).expect("a face");

        assert_eq!(first, index("f-box1/front"));
    }

    #[test]
    fn two_distances_measure_the_first_on_the_first_face() {
        let distances = Distances::TwoDistances {
            on_first: 1.0,
            on_other: 3.0,
        };

        assert_eq!(
            distances.size(0.5, 4),
            ChamferSize::TwoDistances {
                distance: 0.5,
                other_distance: 1.5,
                face: 4
            }
        );
        assert_eq!(distances.larger(), 3.0);
    }

    #[test]
    fn an_equal_distance_scales_alone() {
        assert_eq!(
            Distances::Equal(2.0).size(0.25, 0),
            ChamferSize::Equal { distance: 0.5 }
        );
    }

    #[test]
    fn python_writes_an_equal_distance() {
        let call = Chamfer::python(&params(&[FRONT_TOP], equal("1 mm")), &ScriptNames)
            .expect("a dedicated form");

        assert_eq!(
            call.render(0),
            r#"chamfer(Ref("edge(f-box1/front|f-box1/top)", "line|n=1,0,0|c=0,-10,20|l=20"), 1*mm)"#
        );
    }

    #[test]
    fn two_distances_have_no_dedicated_python_form() {
        let call = Chamfer::python(
            &params(&[FRONT_TOP], two_distances("1", "3", false)),
            &ScriptNames,
        );

        assert_eq!(call, None);
    }

    #[test]
    fn only_an_equal_distance_suggests_a_limit() {
        let cause = partsmith_occt_sys::KernelError {
            operation: partsmith_occt_sys::Operation::Chamfer,
            kind: partsmith_occt_sys::KernelErrorKind::Failed,
            exception: "StdFail_NotDone".to_string(),
            message: String::new(),
        };
        let manifest = Chamfer::manifest();
        let too_large = |field: &str| FeatureError::ChamferTooLarge {
            field: FieldRef::new(field, &manifest),
            distance: 3.0,
            max: Some(2.396),
            cause: cause.clone(),
        };

        assert_eq!(
            too_large("distance.expr").suggestion(Units::Mm),
            Some("2.3 mm".to_string())
        );
        assert_eq!(too_large("distance").suggestion(Units::Mm), None);
    }
}
