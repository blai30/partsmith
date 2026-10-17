//! The `chamfer` feature: bevels edges of a body with a flat face (features/chamfer.md).

use partsmith_model::{Category, Expr, FeatureManifest, FieldUnit, Id, SelectionKind};
use partsmith_naming::{ElementKind, ElementName, Follow, Reference, roles};
use partsmith_occt_sys::{ChamferEdge, ChamferSize, chamfer};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    body_hint, essential, hint, is_false, manifest, options_hint, positive_length, references,
    require_selection, selection_hint, solid_body, unit_hint,
};
use super::dressup::{
    SelectedEdge, finish_blend, no_effect, run_or_search, select_edges, sorted_by_name,
};
use crate::{
    BodyState, BuildContext, Dependencies, Feature, FeatureError, FeatureOutput, FeatureWarning,
    Noun, PythonCall, PythonNames, PythonValue,
};

/// Bevels edges of a body.
#[derive(Clone, Copy, Debug)]
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
    /// face is the first face of the edge's stored reference (features/chamfer.md section 3.3).
    /// Lengths, greater than 0. `flip` swaps them.
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

/// The evaluated distances of a chamfer, in millimeters.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Distances {
    /// The same distance on both faces.
    Equal(f64),
    /// `on_first` on the first face of each edge (section 3.3 step 4), `on_other` on the other face
    /// (flip applied).
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
}

/// `size` with every distance multiplied by `factor`: the "too large" search scales both
/// distances of a two-distance chamfer together (features.md section 4.3).
fn scaled(size: ChamferSize, factor: f64) -> ChamferSize {
    match size {
        ChamferSize::Equal { distance } => ChamferSize::Equal {
            distance: distance * factor,
        },
        ChamferSize::TwoDistances {
            distance,
            other_distance,
            face,
        } => ChamferSize::TwoDistances {
            distance: distance * factor,
            other_distance: other_distance * factor,
            face,
        },
    }
}

impl Feature for Chamfer {
    const TYPE: &'static str = "chamfer";
    type Params = ChamferParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Chamfer",
            Category::Modify,
            "chamfer",
            vec![
                essential(selection_hint("edges", "Edges", SelectionKind::Edge)),
                essential(options_hint(
                    "distance.kind",
                    "Type",
                    &[
                        ("equal", "Equal distance"),
                        ("two_distances", "Two distances"),
                    ],
                )),
                essential(unit_hint("distance.expr", "Distance", FieldUnit::Length)),
                essential(unit_hint(
                    "distance.second_expr",
                    "Second distance",
                    FieldUnit::Length,
                )),
                hint("distance.flip", "Flip"),
                body_hint(),
            ],
        )
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
        require_selection(&params.edges, Self::manifest, "edges", Noun::Edge)
    }

    fn build(
        params: &ChamferParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let (state, shape) = solid_body(context, "body", &params.body)?;
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
                (Distances::TwoDistances { on_first, on_other }, "distance")
            }
        };
        let selection = select_edges(context, &params.body, state, shape, &params.edges)?;
        if selection.edges.is_empty() {
            return Ok(no_effect(&params.body, state, selection.warnings));
        }
        let mut warnings = selection.warnings;
        let edges = chamfer_edges(
            context,
            params,
            state,
            distances,
            sorted_by_name(state.naming(), &selection.edges)?,
            &mut warnings,
        )?;
        let larger = distances.larger();
        let result = run_or_search(
            context,
            larger,
            |value| {
                let factor = value / larger;
                let scaled_edges: Vec<ChamferEdge> = edges
                    .iter()
                    .map(|edge| ChamferEdge {
                        edge: edge.edge,
                        size: scaled(edge.size, factor),
                    })
                    .collect();
                chamfer(shape, &scaled_edges)
            },
            |max, cause| FeatureError::ChamferTooLarge {
                field: context.field(field),
                distance: larger,
                max,
                cause,
            },
        )?;
        finish_blend(
            context,
            &params.body,
            state,
            roles::CHAMFER,
            result,
            warnings,
        )
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
            references(&params.edges),
            PythonValue::expression(expr, names),
        ];
        Some(call)
    }
}

/// The kernel request for each of `edges`, in order: an equal distance has no side, and two distances measure
/// `on_first` on the edge's first face (chamfer.md section 3.3 step 4). Each side notice is added to `warnings`,
/// which the next edge's side reads.
fn chamfer_edges(
    context: &BuildContext<'_>,
    params: &ChamferParams,
    state: &BodyState,
    distances: Distances,
    edges: Vec<&SelectedEdge>,
    warnings: &mut Vec<FeatureWarning>,
) -> Result<Vec<ChamferEdge>, FeatureError> {
    let mut requests = Vec::with_capacity(edges.len());
    for edge in edges {
        let size = match distances {
            Distances::Equal(distance) => ChamferSize::Equal { distance },
            Distances::TwoDistances { on_first, on_other } => {
                let (face, notice) = side_of(context, params, state, edge, warnings)?;
                warnings.extend(notice);
                ChamferSize::TwoDistances {
                    distance: on_first,
                    other_distance: on_other,
                    face,
                }
            }
        };
        requests.push(ChamferEdge {
            edge: edge.index,
            size,
        });
    }
    Ok(requests)
}

/// The face next to `edge` of `state` that a two-distance chamfer measures its first distance on
/// (chamfer.md section 3.3 step 4), and whether it is the face the user picked the edge from: the
/// adjacent face in the family of the first face term of `reference`, the stored reference the edge
/// was resolved from, through canonical names (a face that kept the term's name, took it as an
/// alias when a join merged it, or is a piece of it), so a later join that renames the faces never
/// changes the side (corpus R22). When no adjacent face is, as for an edge reached by continuation
/// or relation, the adjacent face whose element name sorts first in byte order.
fn first_face(
    state: &BodyState,
    edge: &SelectedEdge,
    reference: &Reference,
) -> Result<(u32, bool), FeatureError> {
    let naming = state.naming();
    let mut faces: Vec<(&ElementName, u32)> = edge
        .faces
        .iter()
        .filter_map(|&face| Some((naming.name(ElementKind::Face, face)?, face)))
        .collect();
    faces.sort_unstable();
    let set = state.element_set()?;
    let family = reference
        .query
        .face_parts()
        .first()
        .map(|term| set.names().family(term, &reference.merged))
        .unwrap_or_default();
    if let Some(&(_, face)) = faces.iter().find(|(name, _)| family.contains(name)) {
        return Ok((face, true));
    }
    faces
        .first()
        .map(|&(_, face)| (face, false))
        .ok_or_else(|| FeatureError::internal(format!("edge {} has no named face", edge.index)))
}

/// The first face of `edge` for two distances (chamfer.md section 3.3 step 4), with the notice of
/// `side_notice` when it is not the face the edge was picked from and none of the `messages` so far
/// says anything about the edge's entry.
fn side_of(
    context: &BuildContext<'_>,
    params: &ChamferParams,
    state: &BodyState,
    edge: &SelectedEdge,
    messages: &[FeatureWarning],
) -> Result<(u32, Option<FeatureWarning>), FeatureError> {
    let reference = &params.edges[edge.source];
    let (face, picked) = first_face(state, edge, reference)?;
    let notice = if picked || mentions_entry(messages, edge.source) {
        None
    } else {
        Some(side_notice(context, &params.body, state, edge, reference)?)
    };
    Ok((face, notice))
}

/// True when `messages` say something about the entry at `position` of the edge list: a guess, a
/// widening or a follow of its reference.
fn mentions_entry(messages: &[FeatureWarning], position: usize) -> bool {
    messages.iter().any(|message| match message {
        FeatureWarning::ReferenceGuessed {
            position: entry, ..
        }
        | FeatureWarning::ReferenceFollowed {
            position: entry, ..
        }
        | FeatureWarning::ReferenceWidened {
            position: entry, ..
        } => *entry == position,
        _ => false,
    })
}

/// The notice a two-distance chamfer adds when an edge has no face of the one its reference was
/// picked from and the resolution said nothing about it (chamfer.md section 3.3 step 4): the edge
/// used only relates to the one stored, so the first distance falls on the face whose name sorts
/// first.
fn side_notice(
    context: &BuildContext<'_>,
    body: &Id,
    state: &BodyState,
    edge: &SelectedEdge,
    reference: &Reference,
) -> Result<FeatureWarning, FeatureError> {
    let set = state.element_set()?;
    let used = state
        .naming()
        .name(ElementKind::Edge, edge.index)
        .and_then(|name| set.element(name))
        .map(|info| Reference::from_element(info, &set))
        .ok_or_else(|| {
            FeatureError::internal(format!("the selected edge {} has no element", edge.index))
        })?;
    Ok(FeatureWarning::ReferenceFollowed {
        field: context.field(&format!("edges[{}]", edge.source)),
        position: edge.source,
        body: body.clone(),
        reference: reference.clone(),
        elements: vec![used],
        follow: Follow::Related,
        via: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use partsmith_model::{FeatureRecord, Units};
    use serde_json::json;

    use super::*;
    use crate::test_support::{ScriptNames, id, named_box, reference};
    use crate::{FieldRef, Registry, Tool};

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

    /// `E-FT` as JSON.
    fn edge() -> serde_json::Value {
        serde_json::from_str(FRONT_TOP).expect("valid JSON")
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

    /// The front top edge of a 10 mm cube `f-box1` with its two faces, and the index of the
    /// front face.
    fn front_top_edge(cube: &Tool) -> (SelectedEdge, u32) {
        let index = |name: &str| {
            cube.naming
                .index(&ElementName::parse(name).expect("valid"))
                .expect("named")
        };
        let edge = index("edge(f-box1/front|f-box1/top)");
        let topology = cube.shape.topology().expect("topology");
        let faces = topology.edges[edge as usize - 1].faces.clone();
        (
            SelectedEdge {
                index: edge,
                faces,
                source: 0,
            },
            index("f-box1/front"),
        )
    }

    /// The state of a body holding `cube`.
    fn cube_state(cube: &Tool) -> BodyState {
        BodyState::new(cube.shape.clone(), Arc::new(cube.naming.clone()))
    }

    #[test]
    fn the_manifest_matches_the_spec_table() {
        let registry = registry();

        let manifest = registry.get("chamfer").expect("registered").manifest();

        assert_eq!(
            serde_json::to_value(manifest).expect("a manifest serializes"),
            json!({
                "label": "Chamfer", "category": "modify", "icon": "chamfer", "version": 1,
                "fields": [
                    {"path": "edges", "label": "Edges", "essential": true, "selection": "edge"},
                    {"path": "distance.kind", "label": "Type", "essential": true, "options": [
                        {"value": "equal", "label": "Equal distance"},
                        {"value": "two_distances", "label": "Two distances"},
                    ]},
                    {"path": "distance.expr", "label": "Distance", "essential": true, "unit": "length"},
                    {"path": "distance.second_expr", "label": "Second distance", "essential": true, "unit": "length"},
                    {"path": "distance.flip", "label": "Flip"},
                    {"path": "body", "label": "Body", "selection": "body"},
                ]
            })
        );
    }

    #[test]
    fn the_feature_type_is_copy_and_debug() {
        fn debug_text<T: Copy + std::fmt::Debug>(value: T) -> String {
            format!("{value:?}")
        }

        assert_eq!(debug_text(Chamfer), "Chamfer");
    }

    #[test]
    fn an_unflipped_two_distance_chamfer_omits_flip() {
        let params = canonical(json!({
            "body": "b-1", "edges": [edge()],
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
            "body": "b-1", "edges": [edge()],
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
            "id": "f-cha1", "type": "chamfer", "body": "b-1", "edges": [edge()],
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

        assert_eq!(error.code(), "empty_selection");
        assert_eq!(error.to_string(), "Select at least one edge for Edges.");
        assert_eq!(
            error.field().map(FieldRef::pointer),
            Some("/edges".to_string())
        );
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
    fn the_first_face_of_an_edge_is_the_first_face_term_of_its_reference() {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let (edge, front) = front_top_edge(&cube);

        let first = first_face(&cube_state(&cube), &edge, &reference(FRONT_TOP)).expect("a face");

        assert_eq!(first, (front, true));
    }

    #[test]
    fn an_edge_without_a_face_of_its_reference_takes_the_face_whose_name_sorts_first() {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let (edge, front) = front_top_edge(&cube);
        let elsewhere = reference(
            r#"{"q": "edge(f-box9/side|f-box9/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}"#,
        );

        let first = first_face(&cube_state(&cube), &edge, &elsewhere).expect("a face");

        assert_eq!(first, (front, false));
    }

    #[test]
    fn an_edge_without_named_faces_has_no_first_face() {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let edge = SelectedEdge {
            index: 1,
            faces: vec![99],
            source: 0,
        };

        let error = first_face(&cube_state(&cube), &edge, &reference(FRONT_TOP))
            .expect_err("face 99 has no name");

        assert_eq!(error.code(), "internal");
    }

    #[test]
    fn the_side_notice_is_a_related_follow_of_its_entry() {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let state = cube_state(&cube);
        let fixture = crate::test_support::Fixture::new("f-cha1").with_body("b-1", state.clone());
        let (edge, _) = front_top_edge(&cube);
        let edge = SelectedEdge { source: 1, ..edge };

        let notice = side_notice(
            &fixture.context(),
            &id("b-1"),
            &state,
            &edge,
            &reference(FRONT_TOP),
        )
        .expect("the edge is named");

        assert!(
            matches!(&notice, FeatureWarning::ReferenceFollowed { position: 1, follow: Follow::Related, elements, .. }
                if elements.len() == 1 && elements[0].query.as_str() == "edge(f-box1/front|f-box1/top)"),
            "{notice:?}"
        );
        assert!(mentions_entry(std::slice::from_ref(&notice), 1));
        assert!(!mentions_entry(std::slice::from_ref(&notice), 0));
        assert!(!mentions_entry(&[], 1));
    }

    #[test]
    fn the_search_scales_the_larger_distance() {
        let distances = Distances::TwoDistances {
            on_first: 1.0,
            on_other: 3.0,
        };

        assert_eq!(distances.larger(), 3.0);
        assert_eq!(Distances::Equal(2.0).larger(), 2.0);
    }

    #[test]
    fn scaling_multiplies_every_distance_and_keeps_the_side() {
        assert_eq!(
            scaled(ChamferSize::Equal { distance: 2.0 }, 0.25),
            ChamferSize::Equal { distance: 0.5 }
        );
        assert_eq!(
            scaled(
                ChamferSize::TwoDistances {
                    distance: 1.0,
                    other_distance: 3.0,
                    face: 4
                },
                0.5
            ),
            ChamferSize::TwoDistances {
                distance: 0.5,
                other_distance: 1.5,
                face: 4
            }
        );
    }

    #[test]
    fn python_writes_an_equal_distance() {
        let call = Chamfer::python(
            &params(&[FRONT_TOP], equal("1 mm")),
            &ScriptNames::default(),
        )
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
            &ScriptNames::default(),
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
