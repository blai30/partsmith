//! The `revolve` feature: sweeps closed regions of one sketch around an axis in the sketch plane
//! and combines the solid with a body (features/revolve.md).

use std::f64::consts::{PI, TAU};

use partsmith_model::{
    ANGULAR_TOLERANCE, Category, Expr, FeatureManifest, FieldUnit, Id, LINEAR_TOLERANCE,
    SelectionKind,
};
use partsmith_occt_sys::{Axis, Point, Vector, revolve};
use partsmith_sketch::{Placement, ProfileCurve};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    apply_tool, body_hint, body_op_hint, call, check_range, empty_selection, essential, flag, hint,
    is_false, manifest, op_keyword, options_hint, profile_arguments, selection_hint, sweep_tool,
    unit_hint,
};
use crate::context::{BuildContext, ResolvedProfile};
use crate::deps::Dependencies;
use crate::error::{Bound, FeatureError, Noun};
use crate::feature::Feature;
use crate::output::FeatureOutput;
use crate::params::{AxisRef, BodyOp, PlacementExt, ProfileRef};
use crate::python::{PythonCall, PythonNames, PythonValue};

/// Revolves closed sketch regions around an axis and combines the result with a body.
#[derive(Clone, Copy, Debug)]
pub struct Revolve;

/// Params of a revolve feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RevolveParams {
    /// Target body.
    pub body: Id,
    /// Closed regions of one sketch.
    pub profile: ProfileRef,
    /// The axis of revolution. Must lie in the sketch plane.
    pub axis: AxisRef,
    /// How far the profile turns.
    pub extent: RevolveExtent,
    /// Turn in the negative sense about the axis direction. Default false, omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub reverse: bool,
    /// How the solid combines with the body. Required.
    pub op: BodyOp,
}

/// How far a revolve turns.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RevolveExtent {
    /// Turn by `expr`. Angle, greater than 0 and at most 360 deg.
    Angle {
        /// The angle.
        expr: Expr,
    },
    /// A full turn.
    Full,
}

impl Feature for Revolve {
    const TYPE: &'static str = "revolve";
    type Params = RevolveParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Revolve",
            Category::Create,
            "revolve",
            vec![
                essential(selection_hint(
                    "profile",
                    "Profile",
                    SelectionKind::SketchRegion,
                )),
                essential(selection_hint("axis", "Axis", SelectionKind::Axis)),
                essential(options_hint(
                    "extent.kind",
                    "Extent",
                    &[("angle", "Angle"), ("full", "Full")],
                )),
                essential(unit_hint("extent.expr", "Angle", FieldUnit::Angle)),
                body_op_hint(),
                hint("reverse", "Reverse"),
                body_hint(),
            ],
        )
    }

    fn validate(params: &RevolveParams) -> Result<(), FeatureError> {
        if params.profile.regions.is_empty() {
            return Err(empty_selection(
                &Revolve::manifest(),
                "profile",
                Noun::Region,
            ));
        }
        Ok(())
    }

    /// The profile's sketch, the written body, what the axis reads (a sketch line's sketch, or an
    /// edge's or face's body other than the target) and the angle (features/revolve.md section 3.2).
    fn dependencies(params: &RevolveParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_feature(&params.profile.sketch, "profile.sketch");
        dependencies.add_body_written(&params.body, "body");
        dependencies.add_axis("axis", &params.axis);
        if let RevolveExtent::Angle { expr } = &params.extent {
            dependencies.add_expression("extent.expr", expr, FieldUnit::Angle);
        }
        dependencies
    }

    /// Revolves the union of the resolved regions about the axis and applies the op
    /// (features/revolve.md section 3.3).
    fn build(
        params: &RevolveParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let profile = context.profile("profile", &params.profile)?;
        let axis = context.axis("axis", &params.axis)?;
        if !lies_in_plane(&axis, &profile.sketch.placement) {
            return Err(FeatureError::AxisNotInPlane);
        }
        if crosses_axis(&profile, &axis) {
            return Err(FeatureError::ProfileCrossesAxis);
        }
        let angle = turn(params, context)?.sweep_angle(params.reverse);
        let faces = profile.faces()?;
        if context.is_canceled() {
            return Err(FeatureError::Canceled);
        }
        let sweep = revolve(&faces.profile, &axis, angle)?;
        let tool = sweep_tool(context.feature(), sweep, &faces.entities)?;
        apply_tool(context, &params.body, params.op, tool, profile.warnings)
    }

    /// `revolve(profile, axis, angle, regions=..., reverse=True, op=...)`, without `angle` for a
    /// full turn (features/revolve.md section 2, Python).
    fn python(params: &RevolveParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let (sketch, regions) = profile_arguments(&params.profile, names)?;
        let mut positional = vec![sketch, PythonValue::axis(&params.axis, names)?];
        if let RevolveExtent::Angle { expr } = &params.extent {
            positional.push(PythonValue::expression(expr, names));
        }
        let mut keywords: Vec<(String, PythonValue)> = regions.into_iter().collect();
        if params.reverse {
            keywords.push(flag("reverse"));
        }
        keywords.extend(op_keyword(names, &params.body, params.op));
        Some(call("revolve", positional, keywords))
    }
}

/// How far the profile turns.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Turn {
    /// A full turn, which has no caps and ignores `reverse`.
    Full,
    /// A partial turn by this angle in radians, in (0, 2π).
    Partial(f64),
}

impl Turn {
    /// The signed angle the kernel sweeps: a full turn is 2π whatever `reverse` says, a reversed
    /// partial turn is negative (features/revolve.md section 3.3 step 5).
    fn sweep_angle(self, reverse: bool) -> f64 {
        match self {
            Turn::Full => TAU,
            Turn::Partial(angle) if reverse => -angle,
            Turn::Partial(angle) => angle,
        }
    }
}

/// The turn of the extent with the range checks of features/revolve.md section 3.3 step 4: an
/// angle within the angular tolerance of 360 deg is a full turn.
fn turn(params: &RevolveParams, context: &BuildContext<'_>) -> Result<Turn, FeatureError> {
    let RevolveExtent::Angle { expr } = &params.extent else {
        return Ok(Turn::Full);
    };
    let angle = context.eval_angle("extent.expr", expr)?;
    let range = |bound| check_range(context, "extent.expr", FieldUnit::Angle, angle, bound, None);
    range(Bound::GreaterThan(0.0))?;
    if angle > TAU + ANGULAR_TOLERANCE {
        range(Bound::AtMost(TAU))?;
    }
    if (angle - TAU).abs() <= ANGULAR_TOLERANCE {
        return Ok(Turn::Full);
    }
    Ok(Turn::Partial(angle))
}

/// True when the axis lies in the sketch plane: perpendicular to its normal within the angular
/// tolerance, with its origin within the modeling tolerance of the plane (features/revolve.md
/// section 3.3 step 2).
fn lies_in_plane(axis: &Axis, placement: &Placement) -> bool {
    let plane = placement.to_kernel();
    let height = offset(axis.origin, plane.origin).dot(plane.normal);
    axis.direction.dot(plane.normal).abs() <= ANGULAR_TOLERANCE && height.abs() <= LINEAR_TOLERANCE
}

/// True when the resolved regions have boundary points strictly on both sides of the axis line,
/// beyond the modeling tolerance (features/revolve.md section 3.3 step 3). Their union has the same
/// extreme distances as their boundaries, so the regions are checked as they are, before any
/// kernel work.
fn crosses_axis(profile: &ResolvedProfile<'_>, axis: &Axis) -> bool {
    let (origin, perpendicular) = axis_line(axis, &profile.sketch.placement);
    let (low, high) = profile
        .regions
        .iter()
        .filter_map(|&index| profile.sketch.profile.regions.get(index))
        .flat_map(|region| std::iter::once(&region.outer).chain(&region.holes))
        .flat_map(|profile_loop| &profile_loop.edges)
        .map(|edge| distance_range(&edge.curve, origin, perpendicular))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), range| {
            (low.min(range.0), high.max(range.1))
        });
    low < -LINEAR_TOLERANCE && high > LINEAR_TOLERANCE
}

/// The axis line in sketch coordinates: a point on it, and the unit in-plane direction
/// `normal × direction` along which signed distances to it are measured.
fn axis_line(axis: &Axis, placement: &Placement) -> ([f64; 2], [f64; 2]) {
    let plane = placement.to_kernel();
    let y_axis = plane.normal.cross(plane.x_axis);
    let from_origin = offset(axis.origin, plane.origin);
    let origin = [from_origin.dot(plane.x_axis), from_origin.dot(y_axis)];
    let along = [axis.direction.dot(plane.x_axis), axis.direction.dot(y_axis)];
    // In sketch coordinates `normal × (u, v)` is `(-v, u)`.
    (origin, [-along[1], along[0]])
}

/// The smallest and largest signed distance of a curve's points from the line through `origin`
/// with unit normal `perpendicular`.
fn distance_range(curve: &ProfileCurve, origin: [f64; 2], perpendicular: [f64; 2]) -> (f64, f64) {
    let distance = |point: [f64; 2]| {
        (point[0] - origin[0]) * perpendicular[0] + (point[1] - origin[1]) * perpendicular[1]
    };
    match *curve {
        ProfileCurve::Line { start, end } => {
            let (first, second) = (distance(start), distance(end));
            (first.min(second), first.max(second))
        }
        ProfileCurve::Circle { center, radius, .. } => {
            let middle = distance(center);
            (middle - radius, middle + radius)
        }
        ProfileCurve::Arc {
            center,
            radius,
            start,
            end,
            ccw,
        } => {
            // The counterclockwise sweep from `from` to `to` covers the arc in either sense.
            let (from, to) = if ccw { (start, end) } else { (end, start) };
            let angle = |point: [f64; 2]| (point[1] - center[1]).atan2(point[0] - center[0]);
            let first = angle(from);
            let mut sweep = (angle(to) - first).rem_euclid(TAU);
            if sweep == 0.0 {
                // Ends that meet close the circle.
                sweep = TAU;
            }
            let contains = |direction: f64| (direction - first).rem_euclid(TAU) <= sweep;
            let toward = perpendicular[1].atan2(perpendicular[0]);
            let middle = distance(center);
            let (at_from, at_to) = (distance(from), distance(to));
            let low = if contains(toward + PI) {
                middle - radius
            } else {
                at_from.min(at_to)
            };
            let high = if contains(toward) {
                middle + radius
            } else {
                at_from.max(at_to)
            };
            (low, high)
        }
    }
}

/// The vector from `origin` to `point`.
fn offset(point: Point, origin: Point) -> Vector {
    Vector::new(point.x - origin.x, point.y - origin.y, point.z - origin.z)
}

#[cfg(test)]
mod tests {
    use partsmith_sketch::{OriginPlane, RegionRef};
    use serde_json::json;

    use super::*;
    use crate::registry::Registry;
    use crate::test_support::{ScriptNames, id};

    /// `REV(axis, extent, op)` with `P-RING`, plus `extra` members.
    fn record(
        axis: serde_json::Value,
        extent: serde_json::Value,
        op: &str,
        extra: serde_json::Value,
    ) -> serde_json::Value {
        let mut record = json!({
            "id": "f-rev1",
            "type": "revolve",
            "body": "b-1",
            "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=15,15|a=300"}]},
            "axis": axis,
            "extent": extent,
            "op": op
        });
        if let (Some(target), Some(source)) = (record.as_object_mut(), extra.as_object()) {
            target.extend(source.clone());
        }
        record
    }

    fn params(record: serde_json::Value) -> RevolveParams {
        serde_json::from_value(record).expect("the revolve params are valid")
    }

    fn axis_z() -> serde_json::Value {
        json!({"kind": "origin", "axis": "Z"})
    }

    /// Script names with `e-9` of `f-sk1` as `line_1` and every region of `f-sk1` being `P-RING`.
    fn names() -> ScriptNames {
        let mut names = ScriptNames::default();
        names
            .entities
            .insert((id("f-sk1"), id("e-9")), "line_1".to_string());
        let ring: RegionRef =
            serde_json::from_value(json!({"key": "e-1,e-2,e-3,e-4", "fp": "c=15,15|a=300"}))
                .expect("a region ref");
        names.regions.insert(id("f-sk1"), vec![ring]);
        names
    }

    fn python(record: serde_json::Value) -> Option<String> {
        Revolve::python(&params(record), &names()).map(|call| call.render(0))
    }

    fn origin_axis(direction: [f64; 3]) -> Axis {
        Axis {
            origin: Point::new(0.0, 0.0, 0.0),
            direction: Vector::new(direction[0], direction[1], direction[2]),
        }
    }

    #[test]
    fn registers_through_the_public_registry() {
        let mut registry = Registry::new();

        assert_eq!(registry.register::<Revolve>(), Ok(()));
    }

    #[test]
    fn dependencies_read_an_axis_body_other_than_the_target() {
        let edge = json!({"kind": "edge", "body": "b-2", "edge": {"q": "edge(f-box1/front|f-box1/left)", "fp": "line|n=0,0,1"}});

        let dependencies = Revolve::dependencies(&params(record(
            edge,
            json!({"kind": "full"}),
            "new",
            json!({}),
        )));

        assert_eq!(dependencies.bodies_written[0].id, id("b-1"));
        assert_eq!(dependencies.bodies_read.len(), 1);
        assert_eq!(dependencies.bodies_read[0].id, id("b-2"));
        assert!(dependencies.expressions.is_empty());
    }

    #[test]
    fn dependencies_do_not_read_the_target_as_the_axis_body() {
        let edge = json!({"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-box1/front|f-box1/left)", "fp": "line|n=0,0,1"}});

        let dependencies = Revolve::dependencies(&params(record(
            edge,
            json!({"kind": "full"}),
            "join",
            json!({}),
        )));

        assert!(dependencies.bodies_read.is_empty());
        assert_eq!(dependencies.bodies_written.len(), 1);
    }

    #[test]
    fn dependencies_declare_the_profile_sketch_once_for_its_own_axis_line() {
        let line = json!({"kind": "sketch_line", "sketch": "f-sk1", "line": "e-9"});

        let dependencies = Revolve::dependencies(&params(record(
            line,
            json!({"kind": "angle", "expr": "90 deg"}),
            "new",
            json!({}),
        )));

        assert_eq!(dependencies.features.len(), 1);
        assert_eq!(dependencies.features[0].field, "profile.sketch");
        assert_eq!(dependencies.expressions[0].kind, FieldUnit::Angle);
    }

    #[test]
    fn validate_rejects_an_empty_profile() {
        let mut revolve = params(record(axis_z(), json!({"kind": "full"}), "new", json!({})));
        revolve.profile.regions.clear();

        let error = Revolve::validate(&revolve).expect_err("a profile needs a region");

        assert_eq!(error.to_string(), "Select at least one region for Profile.");
    }

    #[test]
    fn the_z_axis_lies_in_the_xz_plane_but_not_in_xy() {
        let axis = origin_axis([0.0, 0.0, 1.0]);

        assert!(lies_in_plane(&axis, &OriginPlane::Xz.placement()));
        assert!(!lies_in_plane(&axis, &OriginPlane::Xy.placement()));
    }

    #[test]
    fn an_axis_parallel_to_the_plane_but_off_it_is_not_in_it() {
        let axis = Axis {
            origin: Point::new(0.0, 0.0, 5.0),
            direction: Vector::new(1.0, 0.0, 0.0),
        };

        assert!(!lies_in_plane(&axis, &OriginPlane::Xy.placement()));
    }

    #[test]
    fn axis_line_measures_distances_along_normal_cross_direction() {
        // On XZ the sketch v axis is world +Z, so the Z axis is the sketch line u = 0, and
        // `normal × Z` is world -X, sketch -u.
        let (origin, perpendicular) =
            axis_line(&origin_axis([0.0, 0.0, 1.0]), &OriginPlane::Xz.placement());

        assert_eq!(origin, [0.0, 0.0]);
        assert_eq!(perpendicular, [-1.0, 0.0]);
    }

    #[test]
    fn a_line_ranges_between_its_ends() {
        let line = ProfileCurve::Line {
            start: [10.0, 0.0],
            end: [20.0, 30.0],
        };

        assert_eq!(distance_range(&line, [0.0, 0.0], [1.0, 0.0]), (10.0, 20.0));
    }

    #[test]
    fn a_circle_ranges_one_radius_around_its_center() {
        let circle = ProfileCurve::Circle {
            center: [5.0, 0.0],
            radius: 2.0,
            ccw: true,
        };

        assert_eq!(distance_range(&circle, [0.0, 0.0], [1.0, 0.0]), (3.0, 7.0));
    }

    #[test]
    fn an_arc_reaches_its_extreme_where_it_passes_it() {
        // The right half of the unit circle, from the bottom to the top, counterclockwise.
        let arc = ProfileCurve::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start: [0.0, -1.0],
            end: [0.0, 1.0],
            ccw: true,
        };

        let (low, high) = distance_range(&arc, [0.0, 0.0], [1.0, 0.0]);

        assert!(
            low.abs() < 1e-12 && (high - 1.0).abs() < 1e-12,
            "{low} {high}"
        );
    }

    #[test]
    fn a_clockwise_arc_covers_the_other_half() {
        // From the bottom to the top clockwise: the left half of the unit circle.
        let arc = ProfileCurve::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start: [0.0, -1.0],
            end: [0.0, 1.0],
            ccw: false,
        };

        let (low, high) = distance_range(&arc, [0.0, 0.0], [1.0, 0.0]);

        assert!(
            (low + 1.0).abs() < 1e-12 && high.abs() < 1e-12,
            "{low} {high}"
        );
    }

    #[test]
    fn a_full_turn_ignores_reverse() {
        assert_eq!(Turn::Full.sweep_angle(false), TAU);
        assert_eq!(Turn::Full.sweep_angle(true), TAU);
    }

    #[test]
    fn a_reversed_partial_turn_is_negative() {
        assert_eq!(Turn::Partial(1.5).sweep_angle(false), 1.5);
        assert_eq!(Turn::Partial(1.5).sweep_angle(true), -1.5);
    }

    #[test]
    fn python_writes_a_partial_turn_about_an_origin_axis() {
        assert_eq!(
            python(record(
                axis_z(),
                json!({"kind": "angle", "expr": "90 deg"}),
                "new",
                json!({})
            )),
            Some("revolve(sketch_1, Axis.Z, 90*deg)".to_string())
        );
    }

    #[test]
    fn python_omits_the_angle_of_a_full_turn() {
        assert_eq!(
            python(record(
                json!({"kind": "sketch_line", "sketch": "f-sk1", "line": "e-9"}),
                json!({"kind": "full"}),
                "join",
                json!({"reverse": true}),
            )),
            Some("revolve(sketch_1, line_1, reverse=True, op=\"join\")".to_string())
        );
    }

    #[test]
    fn python_writes_an_edge_axis_with_its_part() {
        let edge = json!({"kind": "edge", "body": "b-2", "edge": {"q": "edge(f-box1/front|f-box1/left)", "fp": "line|n=0,0,1"}});

        assert_eq!(
            python(record(edge, json!({"kind": "full"}), "new", json!({}))),
            Some(
                "revolve(sketch_1, Ref(\"edge(f-box1/front|f-box1/left)\", \"line|n=0,0,1\", part=part_2))"
                    .to_string()
            )
        );
    }

    #[test]
    fn python_needs_the_axis_variable() {
        let line = json!({"kind": "sketch_line", "sketch": "f-sk1", "line": "e-7"});

        assert_eq!(
            python(record(line, json!({"kind": "full"}), "new", json!({}))),
            None
        );
    }
}
