//! The `extrude` feature: sweeps closed regions of a sketch along its normal and combines the solid
//! with a body (features/extrude.md).

use partsmith_model::{Category, Expr, FeatureManifest, FieldUnit, Id, SelectionKind};
use partsmith_occt_sys::{Point, Shape, Vector, extrude};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    apply_tool, body_hint, body_op_hint, call, check_range, empty_selection, essential, flag, hint,
    is_false, manifest, op_keyword, options_hint, positive_length, profile_arguments,
    selection_hint, sweep_tool, unit_hint,
};
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::{Bound, FeatureError, Noun};
use crate::feature::Feature;
use crate::output::FeatureOutput;
use crate::params::{BodyOp, PlacementExt, ProfileRef};
use crate::python::{PythonCall, PythonNames, PythonValue};

/// How far past the farthest corner of the body's bounding box a through-all extrusion or hole
/// goes, in millimeters (decision D21).
pub(crate) const THROUGH_ALL_MARGIN: f64 = 1.0;

/// Extrudes closed sketch regions along the sketch normal and combines the result with a body.
#[derive(Clone, Copy, Debug)]
pub struct Extrude;

/// Params of an extrude feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExtrudeParams {
    /// Target body.
    pub body: Id,
    /// Closed regions of one sketch.
    pub profile: ProfileRef,
    /// How far the extrusion goes.
    pub extent: ExtrudeExtent,
    /// Extrude against the sketch normal. Default false, omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub reverse: bool,
    /// How the extrusion combines with the body. Required.
    pub op: BodyOp,
}

/// How far an extrusion goes, measured along its direction from the sketch plane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExtrudeExtent {
    /// From the sketch plane to `expr` along the direction. Length, greater than 0.
    Distance {
        /// The distance.
        expr: Expr,
    },
    /// Total length `expr`, centered on the sketch plane. Length, greater than 0.
    Symmetric {
        /// The total length.
        expr: Expr,
    },
    /// `expr` along the direction and `opposite_expr` against it. Lengths, at least 0, not both 0.
    TwoSides {
        /// The distance along the direction.
        expr: Expr,
        /// The distance against the direction.
        opposite_expr: Expr,
    },
    /// From the sketch plane through the whole target body along the direction.
    ThroughAll,
}

impl Feature for Extrude {
    const TYPE: &'static str = "extrude";
    type Params = ExtrudeParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Extrude",
            Category::Create,
            "extrude",
            vec![
                essential(selection_hint(
                    "profile",
                    "Profile",
                    SelectionKind::SketchRegion,
                )),
                essential(options_hint(
                    "extent.kind",
                    "Extent",
                    &[
                        ("distance", "Distance"),
                        ("symmetric", "Symmetric"),
                        ("two_sides", "Two sides"),
                        ("through_all", "Through all"),
                    ],
                )),
                essential(unit_hint("extent.expr", "Distance", FieldUnit::Length)),
                essential(unit_hint(
                    "extent.opposite_expr",
                    "Opposite distance",
                    FieldUnit::Length,
                )),
                body_op_hint(),
                hint("reverse", "Reverse"),
                body_hint(),
            ],
        )
    }

    fn validate(params: &ExtrudeParams) -> Result<(), FeatureError> {
        if params.profile.regions.is_empty() {
            return Err(empty_selection(
                &Extrude::manifest(),
                "profile",
                Noun::Region,
            ));
        }
        Ok(())
    }

    fn dependencies(params: &ExtrudeParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_feature(&params.profile.sketch, "profile.sketch");
        dependencies.add_body_written(&params.body, "body");
        match &params.extent {
            ExtrudeExtent::Distance { expr } | ExtrudeExtent::Symmetric { expr } => {
                dependencies.add_expression("extent.expr", expr, FieldUnit::Length);
            }
            ExtrudeExtent::TwoSides {
                expr,
                opposite_expr,
            } => {
                dependencies.add_expression("extent.expr", expr, FieldUnit::Length);
                dependencies.add_expression(
                    "extent.opposite_expr",
                    opposite_expr,
                    FieldUnit::Length,
                );
            }
            ExtrudeExtent::ThroughAll => {}
        }
        dependencies
    }

    fn build(
        params: &ExtrudeParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let profile = context.profile("profile", &params.profile)?;
        let plane = profile.sketch.placement.to_kernel();
        let symmetric = matches!(params.extent, ExtrudeExtent::Symmetric { .. });
        let direction = if params.reverse && !symmetric {
            plane.normal.scaled(-1.0)
        } else {
            plane.normal
        };
        let (start, end) = interval(params, context, plane.origin, direction)?;
        let faces = profile.faces()?;
        if context.is_canceled() {
            return Err(FeatureError::Canceled);
        }
        let sweep = extrude(&faces.profile, direction, start, end)
            .map_err(|error| FeatureError::Kernel { error })?;
        let tool = sweep_tool(context.feature(), sweep, &faces.entities)?;
        apply_tool(context, &params.body, params.op, tool, profile.warnings)
    }

    fn python(params: &ExtrudeParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let (sketch, regions) = profile_arguments(&params.profile, names)?;
        let mut positional = vec![sketch];
        let mut keywords: Vec<(String, PythonValue)> = regions.into_iter().collect();
        match &params.extent {
            ExtrudeExtent::Distance { expr } => {
                positional.push(PythonValue::expression(expr, names));
            }
            ExtrudeExtent::Symmetric { expr } => {
                positional.push(PythonValue::expression(expr, names));
                keywords.push(flag("symmetric"));
            }
            ExtrudeExtent::TwoSides { .. } => return None,
            ExtrudeExtent::ThroughAll => {}
        }
        if params.reverse {
            keywords.push(flag("reverse"));
        }
        if params.extent == ExtrudeExtent::ThroughAll {
            keywords.push(flag("through_all"));
        }
        keywords.extend(op_keyword(names, &params.body, params.op));
        Some(call("extrude", positional, keywords))
    }
}

/// The interval `[start, end]` swept along `direction`, measured from the sketch plane through
/// `origin` (features/extrude.md section 3.3).
fn interval(
    params: &ExtrudeParams,
    context: &BuildContext<'_>,
    origin: Point,
    direction: Vector,
) -> Result<(f64, f64), FeatureError> {
    let not_negative = |field: &str, expr: &Expr| -> Result<f64, FeatureError> {
        let value = context.eval_length(field, expr)?;
        check_range(
            context,
            field,
            FieldUnit::Length,
            value,
            Bound::AtLeast(0.0),
            None,
        )
    };
    match &params.extent {
        ExtrudeExtent::Distance { expr } => {
            Ok((0.0, positive_length(context, "extent.expr", expr)?))
        }
        ExtrudeExtent::Symmetric { expr } => {
            let length = positive_length(context, "extent.expr", expr)?;
            Ok((-length / 2.0, length / 2.0))
        }
        ExtrudeExtent::TwoSides {
            expr,
            opposite_expr,
        } => {
            let along = not_negative("extent.expr", expr)?;
            let against = not_negative("extent.opposite_expr", opposite_expr)?;
            if along + against <= 0.0 {
                return Err(FeatureError::BothSidesZero);
            }
            Ok((-against, along))
        }
        ExtrudeExtent::ThroughAll => {
            if params.op == BodyOp::New {
                return Err(FeatureError::ThroughAllNeedsBody);
            }
            let state = context.body(&params.body)?;
            let Some(shape) = state.shape() else {
                return Err(FeatureError::BodyEmpty {
                    body: params.body.clone(),
                    use_new: true,
                });
            };
            let reach = farthest_corner(shape, origin, direction)?;
            if reach <= 0.0 {
                return Err(FeatureError::ThroughAllMisses {
                    body: params.body.clone(),
                });
            }
            Ok((0.0, reach + THROUGH_ALL_MARGIN))
        }
    }
}

/// The largest signed distance `(corner - origin) · direction` over the eight corners of the
/// shape's tight bounding box.
pub(crate) fn farthest_corner(
    shape: &Shape,
    origin: Point,
    direction: Vector,
) -> Result<f64, FeatureError> {
    let bounds = shape
        .bounding_box()
        .map_err(|error| FeatureError::Kernel { error })?;
    let mut farthest = f64::NEG_INFINITY;
    for corner in 0..8 {
        let [x, y, z] = [0, 1, 2].map(|axis| {
            if corner & (1 << axis) == 0 {
                bounds.min[axis]
            } else {
                bounds.max[axis]
            }
        });
        let offset = Vector::new(x - origin.x, y - origin.y, z - origin.z);
        farthest = farthest.max(offset.dot(direction));
    }
    Ok(farthest)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use partsmith_model::FeatureRecord;
    use partsmith_occt_sys::{Plane, make_box};
    use serde_json::json;

    use super::*;
    use crate::output::{BodyChange, OutputData};
    use crate::registry::Registry;
    use crate::test_support::{DocumentRun, ScriptNames, assert_point, id, run, run_document};

    /// The rectangle (0, 0)-(80, 50) on XY.
    fn rectangle() -> serde_json::Value {
        json!({"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [
            {"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]},
            {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]},
            {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]},
            {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}
        ]})
    }

    /// One circle `e-5`.
    fn circle(center: [f64; 2], radius: f64) -> serde_json::Value {
        json!({"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [
            {"id": "e-5", "type": "circle", "center": center, "radius": radius}
        ]})
    }

    fn rectangle_profile() -> serde_json::Value {
        json!({"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]})
    }

    fn circle_profile(fingerprint: &str) -> serde_json::Value {
        json!({"sketch": "f-sk1", "regions": [{"key": "e-5", "fp": fingerprint}]})
    }

    fn extrusion(
        profile: serde_json::Value,
        extent: serde_json::Value,
        op: &str,
        reverse: bool,
    ) -> serde_json::Value {
        let mut record = json!({"id": "f-ext1", "type": "extrude", "body": "b-1", "profile": profile, "extent": extent, "op": op});
        if reverse {
            record["reverse"] = json!(true);
        }
        record
    }

    /// A 40 x 40 x 10 box below the XY plane.
    fn box_below() -> serde_json::Value {
        json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "position": ["0", "0", "-10"], "op": "new"})
    }

    fn rectangle_run(extent: serde_json::Value, reverse: bool) -> DocumentRun {
        run(
            &["b-1"],
            &[
                rectangle(),
                extrusion(rectangle_profile(), extent, "new", reverse),
            ],
        )
    }

    fn cap_z(run: &DocumentRun, cap: &str) -> f64 {
        run.face("b-1", &format!("f-ext1/{cap}")).centroid.z
    }

    /// Script names in which the rectangle's region is every region of `f-sk1`.
    fn rectangle_names() -> ScriptNames {
        let mut names = ScriptNames::default();
        names.regions.insert(
            id("f-sk1"),
            vec![
                serde_json::from_value(json!({"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}))
                    .expect("region"),
            ],
        );
        names
    }

    /// The farthest corner of the box x 0..10, y 0..20, z 0..30 from `origin` along `direction`.
    fn farthest_box_corner(origin: [f64; 3], direction: [f64; 3]) -> f64 {
        let shape = make_box(&Plane::XY, 10.0, 20.0, 30.0)
            .expect("the box builds")
            .shape;
        let [x, y, z] = origin;
        let [dx, dy, dz] = direction;
        farthest_corner(&shape, Point::new(x, y, z), Vector::new(dx, dy, dz))
            .expect("the box has bounds")
    }

    #[test]
    fn the_farthest_corner_is_measured_along_the_direction() {
        let up = farthest_box_corner([0.0, 0.0, 5.0], [0.0, 0.0, 1.0]);
        let down = farthest_box_corner([0.0, 0.0, 5.0], [0.0, 0.0, -1.0]);

        assert!((up - 25.0).abs() < 1e-9, "{up}");
        assert!((down - 5.0).abs() < 1e-9, "{down}");
    }

    #[test]
    fn a_body_behind_the_plane_has_no_positive_reach() {
        let reach = farthest_box_corner([0.0, 0.0, 40.0], [0.0, 0.0, 1.0]);

        assert!((reach + 10.0).abs() < 1e-9, "{reach}");
    }

    #[test]
    fn a_distance_extrusion_names_caps_and_sides() {
        let run = rectangle_run(json!({"kind": "distance", "expr": "30 mm"}), false);

        run.assert_ok("f-ext1");
        run.assert_volume("b-1", 120000.0);
        run.assert_faces("b-1", 6);
        run.assert_bounding_box("b-1", [0.0; 3], [80.0, 50.0, 30.0]);
        let start = run.face("b-1", "f-ext1/start-cap");
        assert_eq!(start.direction, Some(Vector::new(0.0, 0.0, -1.0)));
        assert_point("start cap", start.centroid, [40.0, 25.0, 0.0]);
        run.assert_face_area("b-1", "f-ext1/lateral/e-1", 2400.0);
        run.assert_face_area("b-1", "f-ext1/lateral/e-2", 1500.0);
        assert_point(
            "lateral e-3",
            run.face("b-1", "f-ext1/lateral/e-3").centroid,
            [40.0, 50.0, 15.0],
        );
        let OutputData::Bodies(updates) = &run.output("f-ext1").data else {
            panic!("an extrusion updates bodies");
        };
        assert_eq!(updates[0].change, BodyChange::New);
    }

    #[test]
    fn reverse_extrudes_against_the_normal() {
        let run = rectangle_run(json!({"kind": "distance", "expr": "30 mm"}), true);

        run.assert_bounding_box("b-1", [0.0, 0.0, -30.0], [80.0, 50.0, 0.0]);
        assert!(cap_z(&run, "end-cap") < -29.0);
    }

    #[test]
    fn symmetric_ignores_reverse() {
        let plain = rectangle_run(json!({"kind": "symmetric", "expr": "30 mm"}), false);
        let reversed = rectangle_run(json!({"kind": "symmetric", "expr": "30 mm"}), true);

        for run in [&plain, &reversed] {
            run.assert_ok("f-ext1");
            run.assert_bounding_box("b-1", [0.0, 0.0, -15.0], [80.0, 50.0, 15.0]);
            assert!((cap_z(run, "start-cap") + 15.0).abs() < 1e-9);
        }
    }

    #[test]
    fn two_sides_extend_both_ways() {
        let run = rectangle_run(
            json!({"kind": "two_sides", "expr": "20 mm", "opposite_expr": "10 mm"}),
            true,
        );

        run.assert_bounding_box("b-1", [0.0, 0.0, -20.0], [80.0, 50.0, 10.0]);
        assert!((cap_z(&run, "start-cap") - 10.0).abs() < 1e-9);
    }

    #[test]
    fn two_sides_of_zero_are_an_error() {
        let run = rectangle_run(
            json!({"kind": "two_sides", "expr": "0", "opposite_expr": "0"}),
            false,
        );

        assert_eq!(run.error_code("f-ext1"), "both_sides_zero");
        assert!(run.is_empty("b-1"));
    }

    #[test]
    fn a_negative_opposite_distance_is_out_of_range() {
        let run = rectangle_run(
            json!({"kind": "two_sides", "expr": "5", "opposite_expr": "-1"}),
            false,
        );

        assert_eq!(
            run.messages("f-ext1"),
            ["Opposite distance must be at least 0 mm, but it is -1 mm. Use 0 mm or more."]
        );
    }

    #[test]
    fn a_zero_distance_is_out_of_range() {
        let run = rectangle_run(json!({"kind": "distance", "expr": "0"}), false);

        assert_eq!(
            run.messages("f-ext1"),
            ["Distance must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."]
        );
    }

    #[test]
    fn bare_numbers_take_document_units() {
        let run = run_document(
            "in",
            &[],
            &["b-1"],
            &[
                rectangle(),
                extrusion(
                    rectangle_profile(),
                    json!({"kind": "distance", "expr": "1"}),
                    "new",
                    false,
                ),
            ],
            &Default::default(),
        );

        run.assert_volume("b-1", 101600.0);
    }

    #[test]
    fn a_region_with_a_hole_has_an_inner_wall() {
        let mut sketch = rectangle();
        sketch["entities"]
            .as_array_mut()
            .expect("entities")
            .push(json!({"id": "e-5", "type": "circle", "center": [40, 25], "radius": 10}));
        let run = run(
            &["b-1"],
            &[
                sketch,
                extrusion(
                    json!({"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4;~e-5", "fp": "c=40,25|a=3685.840735"}]}),
                    json!({"kind": "distance", "expr": "10"}),
                    "new",
                    false,
                ),
            ],
        );

        run.assert_volume("b-1", 36858.407346);
        run.assert_faces("b-1", 7);
        run.assert_face_area("b-1", "f-ext1/lateral/e-5", 200.0 * PI);
    }

    #[test]
    fn two_regions_extrude_as_their_union() {
        let mut sketch = rectangle();
        sketch["entities"]
            .as_array_mut()
            .expect("entities")
            .push(json!({"id": "e-5", "type": "circle", "center": [40, 25], "radius": 10}));
        let run = run(
            &["b-1"],
            &[
                sketch,
                extrusion(
                    json!({"sketch": "f-sk1", "regions": [
                        {"key": "e-1,e-2,e-3,e-4;~e-5", "fp": "c=40,25|a=3685.840735"},
                        {"key": "e-5", "fp": "c=40,25|a=314.159265"}
                    ]}),
                    json!({"kind": "distance", "expr": "10"}),
                    "new",
                    false,
                ),
            ],
        );

        run.assert_volume("b-1", 40000.0);
        run.assert_faces("b-1", 6);
        assert!(run.faces_named("b-1", "f-ext1/lateral/e-5").is_empty());
        run.assert_face_area("b-1", "f-ext1/start-cap", 4000.0);
    }

    #[test]
    fn a_join_keeps_the_body_faces() {
        let run = run(
            &["b-1"],
            &[
                box_below(),
                circle([0.0, 0.0], 5.0),
                extrusion(
                    circle_profile("c=0,0|a=78.539816"),
                    json!({"kind": "distance", "expr": "5"}),
                    "join",
                    false,
                ),
            ],
        );

        run.assert_volume("b-1", 16392.699082);
        run.assert_faces("b-1", 8);
        let names = run.face_names("b-1");
        assert!(names.contains(&"f-box1/top".to_string()));
        assert!(names.contains(&"f-ext1/end-cap".to_string()));
        assert!(!names.contains(&"f-ext1/start-cap".to_string()));
    }

    #[test]
    fn through_all_cuts_through_the_body() {
        let run = run(
            &["b-1"],
            &[
                box_below(),
                circle([0.0, 0.0], 5.0),
                extrusion(
                    circle_profile("c=0,0|a=78.539816"),
                    json!({"kind": "through_all"}),
                    "cut",
                    true,
                ),
            ],
        );

        run.assert_volume("b-1", 15214.601837);
        run.assert_faces("b-1", 7);
        assert!(
            run.faces_named("b-1", "f-ext1/")
                .contains(&"f-ext1/lateral/e-5".to_string())
        );
        assert_eq!(run.faces_named("b-1", "f-ext1/").len(), 1);
    }

    #[test]
    fn through_all_pointing_away_misses() {
        let run = run(
            &["b-1"],
            &[
                box_below(),
                circle([0.0, 0.0], 5.0),
                extrusion(
                    circle_profile("c=0,0|a=78.539816"),
                    json!({"kind": "through_all"}),
                    "cut",
                    false,
                ),
            ],
        );

        assert_eq!(
            run.messages("f-ext1"),
            [
                "Through all points away from body 'Body 1', so there is nothing to go through. Turn on Reverse, or use a distance."
            ]
        );
    }

    #[test]
    fn through_all_needs_a_body() {
        let run = rectangle_run(json!({"kind": "through_all"}), false);

        assert_eq!(run.error_code("f-ext1"), "through_all_needs_body");
    }

    #[test]
    fn a_cut_that_misses_has_no_effect() {
        let run = run(
            &["b-1"],
            &[
                box_below(),
                circle([100.0, 0.0], 5.0),
                extrusion(
                    circle_profile("c=100,0|a=78.539816"),
                    json!({"kind": "distance", "expr": "4"}),
                    "cut",
                    true,
                ),
            ],
        );

        assert_eq!(run.warning_codes("f-ext1"), ["no_effect"]);
        run.assert_unchanged_by("f-ext1", "b-1");
    }

    #[test]
    fn a_region_found_by_fingerprint_is_remapped() {
        let run = run(
            &["b-1"],
            &[
                rectangle(),
                extrusion(
                    json!({"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-9", "fp": "c=40,25|a=4000"}]}),
                    json!({"kind": "distance", "expr": "30"}),
                    "new",
                    false,
                ),
            ],
        );

        assert_eq!(run.warning_codes("f-ext1"), ["region_remapped"]);
        run.assert_volume("b-1", 120000.0);
    }

    #[test]
    fn a_datum_is_not_a_profile_sketch() {
        let run = run(
            &["b-1"],
            &[
                json!({"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}}),
                extrusion(
                    json!({"sketch": "f-dp1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]}),
                    json!({"kind": "distance", "expr": "30"}),
                    "new",
                    false,
                ),
            ],
        );

        assert_eq!(
            run.messages("f-ext1"),
            ["'Datum plane 1' is not a sketch. Pick a sketch for Profile."]
        );
    }

    #[test]
    fn an_empty_profile_is_an_empty_selection() {
        let run = run(
            &["b-1"],
            &[
                rectangle(),
                extrusion(
                    json!({"sketch": "f-sk1", "regions": []}),
                    json!({"kind": "distance", "expr": "30"}),
                    "new",
                    false,
                ),
            ],
        );

        assert_eq!(
            run.messages("f-ext1"),
            ["Select at least one region for Profile."]
        );
    }

    #[test]
    fn python_writes_the_spec_example() {
        let record: FeatureRecord = serde_json::from_value(json!({
            "id": "f-ext1", "type": "extrude", "body": "b-1", "profile": rectangle_profile(),
            "extent": {"kind": "distance", "expr": "30 mm"}, "op": "new"
        }))
        .expect("a valid record");

        let python = Registry::with_builtins().python(&record, &rectangle_names());

        assert_eq!(python.render(0), r#"extrude(sketch_1, 30*mm, id="f-ext1")"#);
    }

    #[test]
    fn python_writes_the_sketch_and_distance() {
        let names = rectangle_names();
        let params = |extent: serde_json::Value, op: &str| -> ExtrudeParams {
            serde_json::from_value(json!({"body": "b-1", "profile": rectangle_profile(), "extent": extent, "reverse": true, "op": op})).expect("valid params")
        };

        let distance = Extrude::python(
            &params(json!({"kind": "distance", "expr": "30 mm"}), "new"),
            &names,
        )
        .expect("a dedicated form");
        let through = Extrude::python(&params(json!({"kind": "through_all"}), "cut"), &names)
            .expect("a dedicated form");
        let two_sides = Extrude::python(
            &params(
                json!({"kind": "two_sides", "expr": "1", "opposite_expr": "2"}),
                "new",
            ),
            &names,
        );

        assert_eq!(distance.render(0), "extrude(sketch_1, 30*mm, reverse=True)");
        assert_eq!(
            through.render(0),
            r#"extrude(sketch_1, reverse=True, through_all=True, op="cut")"#
        );
        assert_eq!(two_sides, None);
    }

    #[test]
    fn python_writes_regions_that_are_not_every_region() {
        let names = {
            let mut names = ScriptNames::default();
            names.regions.insert(id("f-sk1"), Vec::new());
            names
        };
        let params: ExtrudeParams = serde_json::from_value(json!({"body": "b-1", "profile": rectangle_profile(), "extent": {"kind": "symmetric", "expr": "4"}, "op": "new"})).expect("valid params");

        let python = Extrude::python(&params, &names).expect("a dedicated form");

        // The call is longer than 88 characters on one line, so it takes one argument per line.
        assert_eq!(
            python.render(0),
            [
                "extrude(",
                "    sketch_1,",
                "    4,",
                r#"    regions=[Region("e-1,e-2,e-3,e-4", "c=40,25|a=4000")],"#,
                "    symmetric=True,",
                ")",
            ]
            .join("\n")
        );
    }
}
