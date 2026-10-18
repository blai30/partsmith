//! The `circular_pattern` feature: copies a body around an axis, evenly around a full turn or
//! spread across a partial angle, and fuses the copies into the source or puts them into
//! another, empty body (docs/spec/features/circular_pattern.md).

use std::f64::consts::TAU;

use partsmith_model::{
    ANGULAR_TOLERANCE, Category, Expr, FeatureManifest, FieldUnit, Id, SelectionKind,
};
use partsmith_naming::Instance;
use partsmith_occt_sys::Transform;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    angle_in_range, check_canceled, essential, flag, hint, into_keyword, is_false, manifest,
    out_of_range, selection_hint, target_hint, unit_hint,
};
use super::copy::{self, Source};
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::{Bound, FeatureError};
use crate::feature::Feature;
use crate::output::FeatureOutput;
use crate::params::{AxisRef, CopyOp};
use crate::python::{PythonCall, PythonNames, PythonValue};

/// The stored text of the default total angle, a full turn.
const FULL_TURN: &str = "360 deg";

/// Copies a body around an axis.
#[derive(Clone, Copy, Debug, Default)]
pub struct CircularPattern;

/// Params of a circular pattern feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CircularPatternParams {
    /// The body to copy.
    pub body: Id,
    /// The rotation axis.
    pub axis: AxisRef,
    /// Number of instances, including the original. Count, at least 1.
    pub count: Expr,
    /// Total angle. Angle, greater than 0 and at most 360 deg. Default "360 deg", omitted when
    /// default.
    #[serde(default = "full_turn", skip_serializing_if = "is_full_turn")]
    pub angle: Expr,
    /// Turn in the negative sense about the axis direction. Default false, omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub reverse: bool,
    /// Where the copies go. Default join, omitted when join.
    #[serde(default, skip_serializing_if = "CopyOp::is_default")]
    pub op: CopyOp,
    /// The empty body that receives the copies. Required with op new, not allowed with op join.
    /// Omitted when None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Id>,
}

/// The default total angle, `"360 deg"`.
fn full_turn() -> Expr {
    Expr::new(FULL_TURN)
}

/// True for the default total angle; for `skip_serializing_if`.
fn is_full_turn(angle: &Expr) -> bool {
    angle.as_str() == FULL_TURN
}

impl Feature for CircularPattern {
    const TYPE: &'static str = "circular_pattern";
    type Params = CircularPatternParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Circular pattern",
            Category::Pattern,
            "circular-pattern",
            vec![
                essential(selection_hint("body", "Body", SelectionKind::Body)),
                essential(selection_hint("axis", "Axis", SelectionKind::Axis)),
                essential(unit_hint("count", "Count", FieldUnit::Count)),
                essential(unit_hint("angle", "Total angle", FieldUnit::Angle)),
                hint("reverse", "Reverse"),
                copy::op_hint(),
                target_hint(),
            ],
        )
    }

    fn validate(params: &CircularPatternParams) -> Result<(), FeatureError> {
        copy::validate_target(
            params.op,
            &params.body,
            params.target.as_ref(),
            &CircularPattern::manifest(),
        )
    }

    /// The source body, what the axis reads, the count and the total angle, then the target of
    /// op new (circular_pattern.md section 3.2).
    fn dependencies(params: &CircularPatternParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        copy::add_source(&mut dependencies, params.op, &params.body);
        dependencies.add_axis("axis", &params.axis);
        dependencies.add_expression("count", &params.count, FieldUnit::Count);
        dependencies.add_expression("angle", &params.angle, FieldUnit::Angle);
        copy::add_target(&mut dependencies, params.op, params.target.as_ref());
        dependencies
    }

    /// Instance k is the source rotated about the axis by k steps, named `@<id>:<k>`; instance 0
    /// is the original. A full turn spaces n instances by 360 deg / n, a partial angle puts the
    /// first and last instances at its ends. The copies are combined with one kernel boolean
    /// (circular_pattern.md section 3.3).
    fn build(
        params: &CircularPatternParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let source = Source::new(context, &params.body)?;
        let count = context.eval_count("count", &params.count, 1)?;
        copy::check_instance_limit(u64::from(count))?;
        let angle = angle_in_range(context, "angle", &params.angle, Bound::GreaterThan(0.0))?;
        // A total angle within the tolerance above 360 deg counts as a full turn.
        if angle > TAU + ANGULAR_TOLERANCE {
            return Err(out_of_range(
                context,
                "angle",
                FieldUnit::Angle,
                angle,
                Bound::AtMost(TAU),
                None,
            ));
        }
        let axis = context.axis("axis", &params.axis)?;
        let step = step_angle(angle, count) * if params.reverse { -1.0 } else { 1.0 };
        let mut copies = Vec::new();
        for instance in 1..count {
            check_canceled(context)?;
            let rotation = Transform::rotation(&axis.value, step * f64::from(instance));
            let suffix = Instance {
                first: instance,
                second: 0,
            };
            copies.push(source.copy(context, &rotation, suffix)?);
        }
        source.place(
            context,
            params.op,
            params.target.as_ref(),
            copies,
            axis.warnings,
            vec![context.field("count")],
        )
    }

    /// `circular_pattern(axis, count, angle, reverse=True, into=...)` (circular_pattern.md
    /// section 2): the angle only when the stored params set it. None when the axis or the target
    /// has no variable.
    fn python(params: &CircularPatternParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let mut call = PythonCall::new("circular_pattern");
        call.positional = vec![
            PythonValue::axis(&params.axis, names)?,
            PythonValue::expression(&params.count, names),
        ];
        if !is_full_turn(&params.angle) {
            call.positional
                .push(PythonValue::expression(&params.angle, names));
        }
        if params.reverse {
            call.keywords.push(flag("reverse"));
        }
        call.keywords
            .extend(into_keyword(names, params.op, params.target.as_ref())?);
        Some(call)
    }
}

/// The angle between neighboring instances of `count` instances over a total `angle` in
/// (0, 2π]: 2π / n for a full turn, so the last instance does not land on the first, else
/// angle / (n - 1), so the first and last instances sit at the ends of the angle.
fn step_angle(angle: f64, count: u32) -> f64 {
    if (angle - TAU).abs() <= ANGULAR_TOLERANCE {
        TAU / f64::from(count)
    } else if count >= 2 {
        angle / f64::from(count - 1)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use partsmith_occt_sys::SubShapeKind;
    use serde_json::{Value, json};

    use super::*;
    use crate::context::BodyState;
    use crate::error::FieldRef;
    use crate::output::BodyChange;
    use crate::test_support::{
        assert_bounds, assert_volume, box_body, build, face_names, id, named_primitive,
        params_with, python, update, updated_shape,
    };

    fn pattern(extra: Value) -> CircularPatternParams {
        params_with::<CircularPattern>(
            json!({
                "id": "f-cp1", "type": "circular_pattern", "body": "b-1",
                "axis": {"kind": "origin", "axis": "Z"}
            }),
            extra,
        )
    }

    /// A cylinder of radius 2 and height 5 in `b-1` with its axis at x = 20, y = 0.
    fn peg() -> Vec<(&'static str, BodyState)> {
        let placement = partsmith_occt_sys::Plane {
            origin: partsmith_occt_sys::Point::new(20.0, 0.0, 0.0),
            ..partsmith_occt_sys::Plane::XY
        };
        let cylinder =
            partsmith_occt_sys::make_cylinder(&placement, 2.0, 5.0).expect("the cylinder builds");
        let tool = named_primitive("f-cyl1", cylinder);
        vec![(
            "b-1",
            BodyState::new(tool.shape, std::sync::Arc::new(tool.naming)),
        )]
    }

    fn run(params: &CircularPatternParams) -> Result<FeatureOutput, FeatureError> {
        build::<CircularPattern>("f-cp1", params, peg(), Vec::new())
    }

    #[test]
    fn canonical_params_omit_the_full_turn() {
        let params = pattern(json!({"count": "6", "angle": "360 deg", "reverse": false}));

        assert_eq!(
            serde_json::to_string(&params).expect("serializes"),
            r#"{"body":"b-1","axis":{"kind":"origin","axis":"Z"},"count":"6"}"#
        );
        assert_eq!(params.angle, Expr::new("360 deg"));
    }

    #[test]
    fn dependencies_declare_count_then_angle() {
        let params = pattern(json!({"count": "6"}));

        let dependencies = CircularPattern::dependencies(&params);

        let uses: Vec<(&str, &str, FieldUnit)> = dependencies
            .expressions
            .iter()
            .map(|usage| (usage.field.as_str(), usage.expr.as_str(), usage.kind))
            .collect();
        assert_eq!(
            uses,
            [
                ("count", "6", FieldUnit::Count),
                ("angle", "360 deg", FieldUnit::Angle)
            ]
        );
    }

    #[test]
    fn an_axis_face_reads_its_body() {
        let params = pattern(json!({
            "count": "6",
            "axis": {"kind": "face", "body": "b-2", "face": {"q": "f-cyl2/side", "fp": "cylinder|n=0,0,1"}}
        }));

        let dependencies = CircularPattern::dependencies(&params);

        assert_eq!(dependencies.bodies_read[0].id, id("b-2"));
        assert_eq!(dependencies.bodies_read[0].field, "axis.body");
    }

    #[test]
    fn a_full_turn_divides_the_circle_evenly() {
        assert!((step_angle(TAU, 4) - FRAC_PI_2).abs() < 1e-15);
    }

    #[test]
    fn a_partial_angle_ends_at_its_last_instance() {
        assert!((step_angle(FRAC_PI_2, 3) - FRAC_PI_2 / 2.0).abs() < 1e-15);
        assert_eq!(step_angle(FRAC_PI_2, 1), 0.0);
    }

    #[test]
    fn six_around_make_six_pegs() {
        let output = run(&pattern(json!({"count": "6"}))).expect("the pattern builds");

        let shape = updated_shape(&output, "b-1");
        assert_volume(shape, 6.0 * 20.0 * std::f64::consts::PI);
        assert_eq!(shape.count(SubShapeKind::Solid), 6);
        assert_eq!(shape.count(SubShapeKind::Face), 18);
        assert_eq!(update(&output, "b-1").change, BodyChange::Join);
        assert!(
            face_names(&update(&output, "b-1").naming).contains(&"f-cyl1/top@f-cp1:3".to_string())
        );
    }

    #[test]
    fn a_partial_angle_turns_counterclockwise_about_z() {
        let output =
            run(&pattern(json!({"count": "3", "angle": "90 deg"}))).expect("the pattern builds");

        assert_bounds(
            updated_shape(&output, "b-1"),
            [-2.0, -2.0, 0.0],
            [22.0, 22.0, 5.0],
        );
    }

    #[test]
    fn reverse_turns_the_other_way() {
        let output = run(&pattern(
            json!({"count": "3", "angle": "90 deg", "reverse": true}),
        ))
        .expect("the pattern builds");

        assert_bounds(
            updated_shape(&output, "b-1"),
            [-2.0, -22.0, 0.0],
            [22.0, 2.0, 5.0],
        );
    }

    #[test]
    fn a_zero_angle_is_out_of_range() {
        let error =
            run(&pattern(json!({"count": "3", "angle": "0"}))).expect_err("0 deg turns nothing");

        assert_eq!(
            error.to_string(),
            "Total angle must be greater than 0 deg, but it is 0 deg. Use a value greater than 0 deg."
        );
    }

    #[test]
    fn a_zero_count_is_out_of_range() {
        let error = run(&pattern(json!({"count": "0"}))).expect_err("count 0");

        assert_eq!(
            error.to_string(),
            "Count must be at least 1, but it is 0. Use 1 or more."
        );
    }

    #[test]
    fn count_one_with_op_new_needs_an_empty_target() {
        let params = pattern(json!({"count": "1", "op": "new", "target": "b-2"}));
        let mut bodies = peg();
        bodies.push(("b-2", box_body("f-box2", [50.0, 0.0, 0.0], 1.0)));

        let result = build::<CircularPattern>("f-cp1", &params, bodies, Vec::new());

        assert_eq!(
            result.map(|_| ()),
            Err(FeatureError::BodyNotEmpty {
                body: id("b-2"),
                target: Some(FieldRef::new("target", &CircularPattern::manifest())),
                new_body: false,
            })
        );
    }

    #[test]
    fn too_many_instances_fail_before_any_copy() {
        let error = run(&pattern(json!({"count": "20000"}))).expect_err("20000 instances");

        assert_eq!(
            error,
            FeatureError::TooManyInstances {
                count: 20_000,
                limit: 10_000
            }
        );
    }

    #[test]
    fn op_new_puts_the_copies_into_the_target() {
        let output = run(&pattern(
            json!({"count": "6", "op": "new", "target": "b-2"}),
        ))
        .expect("the pattern builds");

        assert_eq!(update(&output, "b-2").change, BodyChange::New);
        assert_eq!(updated_shape(&output, "b-2").count(SubShapeKind::Face), 15);
    }

    #[test]
    fn python_omits_the_full_turn_and_writes_a_partial_angle() {
        assert_eq!(
            python::<CircularPattern>(&pattern(json!({"count": "6"}))).as_deref(),
            Some("circular_pattern(Axis.Z, 6)")
        );
        assert_eq!(
            python::<CircularPattern>(&pattern(
                json!({"count": "3", "angle": "90 deg", "reverse": true})
            ))
            .as_deref(),
            Some("circular_pattern(Axis.Z, 3, 90*deg, reverse=True)")
        );
        assert_eq!(
            python::<CircularPattern>(&pattern(
                json!({"count": "6", "op": "new", "target": "b-2"})
            ))
            .as_deref(),
            Some("circular_pattern(Axis.Z, 6, into=part_2)")
        );
    }
}
