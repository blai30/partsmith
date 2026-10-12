//! The `linear_pattern` feature: copies a body at equal spacing along one direction, or along two
//! as a grid, and fuses the copies into the source or puts them into another, empty body
//! (docs/spec/features/linear_pattern.md).

use partsmith_model::{Category, Expr, FeatureManifest, FieldUnit, Id, SelectionKind};
use partsmith_naming::Instance;
use partsmith_occt_sys::{Transform, Vector};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    COPY_OP_OPTIONS, check_range, essential, hint, into_keyword, is_false, manifest, options_hint,
    selection_hint, unit_hint,
};
use super::copy::{self, INSTANCE_LIMIT, Source};
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::{Bound, FeatureError};
use crate::feature::Feature;
use crate::output::FeatureOutput;
use crate::params::{AxisRef, CopyOp};
use crate::python::{PythonCall, PythonNames, PythonValue};

/// Copies a body along one or two directions.
#[derive(Clone, Copy, Debug, Default)]
pub struct LinearPattern;

/// Params of a linear pattern feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LinearPatternParams {
    /// The body to copy.
    pub body: Id,
    /// The first direction.
    pub direction: LinearPatternDirection,
    /// An optional second direction for a grid. Omitted when None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second_direction: Option<LinearPatternDirection>,
    /// Where the copies go. Default join, omitted when join.
    #[serde(default, skip_serializing_if = "CopyOp::is_default")]
    pub op: CopyOp,
    /// The empty body that receives the copies. Required with op new, not allowed with op join.
    /// Omitted when None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Id>,
}

/// One direction of a linear pattern. Named with the feature prefix so its schema definition is
/// unique (features.md section 2.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinearPatternDirection {
    /// The direction (only its direction is used).
    pub axis: AxisRef,
    /// Number of instances along this direction, including the original. Count, at least 1.
    pub count: Expr,
    /// Distance between neighboring instances. Length, greater than 0.
    pub spacing: Expr,
    /// Go against the axis direction. Default false, omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub reverse: bool,
}

/// The params fields of the two directions, in params order.
const DIRECTION_FIELDS: [&str; 2] = ["direction", "second_direction"];

/// One evaluated direction: how many instances it makes and how far apart.
#[derive(Clone, Copy, Debug)]
struct Step {
    /// Instances along the direction, the original included.
    count: u32,
    /// The displacement between neighboring instances.
    offset: Vector,
}

impl Step {
    /// The direction at `field`: its count (at least 1), its spacing (greater than 0) and its
    /// resolved axis direction, reversed when asked (linear_pattern.md section 3.3 step 2).
    fn new(
        context: &BuildContext<'_>,
        field: &str,
        direction: &LinearPatternDirection,
    ) -> Result<Step, FeatureError> {
        let count_field = format!("{field}.count");
        let count = context.eval_count(&count_field, &direction.count)?;
        check_range(
            context,
            &count_field,
            FieldUnit::Count,
            f64::from(count),
            Bound::AtLeast(1.0),
            None,
        )?;
        let spacing_field = format!("{field}.spacing");
        let spacing = context.eval_length(&spacing_field, &direction.spacing)?;
        check_range(
            context,
            &spacing_field,
            FieldUnit::Length,
            spacing,
            Bound::GreaterThan(0.0),
            None,
        )?;
        let axis = context.axis(&format!("{field}.axis"), &direction.axis)?;
        let signed = if direction.reverse { -spacing } else { spacing };
        Ok(Step {
            count,
            offset: axis.direction.scaled(signed),
        })
    }

    /// The single instance of a missing second direction.
    const NONE: Step = Step {
        count: 1,
        offset: Vector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
    };
}

impl Feature for LinearPattern {
    const TYPE: &'static str = "linear_pattern";
    type Params = LinearPatternParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Linear pattern",
            Category::Pattern,
            "linear-pattern",
            vec![
                essential(selection_hint("body", "Body", SelectionKind::Body)),
                essential(selection_hint(
                    "direction.axis",
                    "Direction",
                    SelectionKind::Axis,
                )),
                essential(unit_hint("direction.count", "Count", FieldUnit::Count)),
                essential(unit_hint(
                    "direction.spacing",
                    "Spacing",
                    FieldUnit::Length,
                )),
                hint("direction.reverse", "Reverse"),
                selection_hint(
                    "second_direction.axis",
                    "Second direction",
                    SelectionKind::Axis,
                ),
                unit_hint("second_direction.count", "Second count", FieldUnit::Count),
                unit_hint(
                    "second_direction.spacing",
                    "Second spacing",
                    FieldUnit::Length,
                ),
                hint("second_direction.reverse", "Second reverse"),
                options_hint("op", "Operation", &COPY_OP_OPTIONS),
                selection_hint("target", "Target body", SelectionKind::Body),
            ],
        )
    }

    fn validate(params: &LinearPatternParams) -> Result<(), FeatureError> {
        copy::validate_target(
            params.op,
            &params.body,
            params.target.as_ref(),
            &LinearPattern::manifest(),
        )
    }

    /// The source body, each direction's axis, count and spacing, then the target of op new
    /// (linear_pattern.md section 3.2).
    fn dependencies(params: &LinearPatternParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        copy::add_source(&mut dependencies, params.op, &params.body);
        for (field, direction) in directions(params) {
            dependencies.add_axis(&format!("{field}.axis"), &direction.axis);
            dependencies.add_expression(
                &format!("{field}.count"),
                &direction.count,
                FieldUnit::Count,
            );
            dependencies.add_expression(
                &format!("{field}.spacing"),
                &direction.spacing,
                FieldUnit::Length,
            );
        }
        copy::add_target(&mut dependencies, params.op, params.target.as_ref());
        dependencies
    }

    /// Instance (i, j) is the source translated by i steps along the first direction and j along
    /// the second, named `@<id>:<i>` or `@<id>:<i>.<j>`, built in order of j, then i; (0, 0) is
    /// the original. The copies are combined with one kernel boolean (linear_pattern.md section
    /// 3.3).
    fn build(
        params: &LinearPatternParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let source = Source::new(context, &params.body)?;
        let first = Step::new(context, DIRECTION_FIELDS[0], &params.direction)?;
        let second = match &params.second_direction {
            Some(direction) => Step::new(context, DIRECTION_FIELDS[1], direction)?,
            None => Step::NONE,
        };
        let total = u64::from(first.count) * u64::from(second.count);
        if total > INSTANCE_LIMIT {
            return Err(FeatureError::TooManyInstances {
                count: total,
                limit: INSTANCE_LIMIT,
            });
        }
        let mut copies = Vec::new();
        for j in 0..second.count {
            for i in 0..first.count {
                if (i, j) == (0, 0) {
                    continue;
                }
                copy::check_canceled(context)?;
                let along_first = first.offset.scaled(f64::from(i));
                let along_second = second.offset.scaled(f64::from(j));
                let offset = Vector::new(
                    along_first.x + along_second.x,
                    along_first.y + along_second.y,
                    along_first.z + along_second.z,
                );
                let instance = Instance {
                    first: i,
                    second: j,
                };
                copies.push(source.copy(context, &Transform::translation(offset), instance)?);
            }
        }
        source.place(context, params.op, params.target.as_ref(), copies)
    }

    /// `linear_pattern(direction, count, spacing, second=(axis, count, spacing), into=...)`
    /// (linear_pattern.md section 2). A reversed direction has no dedicated form. None when an
    /// axis or the target has no variable.
    fn python(params: &LinearPatternParams, names: &dyn PythonNames) -> Option<PythonCall> {
        if directions(params).any(|(_, direction)| direction.reverse) {
            return None;
        }
        let mut call = PythonCall::new("linear_pattern");
        call.positional = direction_values(&params.direction, names)?;
        if let Some(second) = &params.second_direction {
            call.keywords.push((
                "second".to_string(),
                PythonValue::Tuple(direction_values(second, names)?),
            ));
        }
        call.keywords
            .extend(into_keyword(names, params.op, params.target.as_ref())?);
        Some(call)
    }
}

/// The directions that are set, with their params fields, in params order.
fn directions(
    params: &LinearPatternParams,
) -> impl Iterator<Item = (&'static str, &LinearPatternDirection)> {
    DIRECTION_FIELDS
        .into_iter()
        .zip([Some(&params.direction), params.second_direction.as_ref()])
        .filter_map(|(field, direction)| Some((field, direction?)))
}

/// A direction's axis, count and spacing as Python values. None when the axis has no variable.
fn direction_values(
    direction: &LinearPatternDirection,
    names: &dyn PythonNames,
) -> Option<Vec<PythonValue>> {
    Some(vec![
        PythonValue::axis(&direction.axis, names)?,
        PythonValue::expression(&direction.count, names),
        PythonValue::expression(&direction.spacing, names),
    ])
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::builtin::copy::testing::{
        assert_bounds, assert_volume, build, fixture, params, python, python_with, update,
        updated_shape, updates,
    };
    use crate::context::BodyState;
    use crate::output::BodyChange;
    use crate::registry::Registry;
    use crate::test_support::{ScriptNames, box_body, face_names, id};

    const X: &str = r#"{"kind": "origin", "axis": "X"}"#;
    const Y: &str = r#"{"kind": "origin", "axis": "Y"}"#;

    fn direction(axis: &str, count: &str, spacing: &str) -> Value {
        json!({
            "axis": serde_json::from_str::<Value>(axis).expect("valid axis JSON"),
            "count": count,
            "spacing": spacing
        })
    }

    fn pattern(direction: Value, extra: Value) -> LinearPatternParams {
        let mut record = json!({
            "id": "f-lp1", "type": "linear_pattern", "body": "b-1", "direction": direction
        });
        if let Value::Object(members) = extra {
            for (key, value) in members {
                record[key] = value;
            }
        }
        params::<LinearPattern>(record)
    }

    /// A 10 mm cube in `b-1` spanning [-5,-5,0]-[5,5,10].
    fn cube() -> Vec<(&'static str, BodyState)> {
        vec![("b-1", box_body("f-box1", [-5.0, -5.0, 0.0], 10.0))]
    }

    fn run(params: &LinearPatternParams) -> Result<FeatureOutput, FeatureError> {
        build::<LinearPattern>("f-lp1", params, cube(), Vec::new())
    }

    #[test]
    fn registers_with_its_manifest() {
        let mut registry = Registry::new();

        registry
            .register::<LinearPattern>()
            .expect("the linear pattern passes the registration checks");

        assert_eq!(registry.types().collect::<Vec<_>>(), ["linear_pattern"]);
    }

    #[test]
    fn canonical_params_omit_defaults_and_keep_the_direction_order() {
        let params = pattern(
            json!({"spacing": "20 mm", "reverse": false, "count": "3", "axis": {"axis": "X", "kind": "origin"}}),
            json!({"op": "join"}),
        );

        assert_eq!(
            serde_json::to_string(&params).expect("serializes"),
            r#"{"body":"b-1","direction":{"axis":{"kind":"origin","axis":"X"},"count":"3","spacing":"20 mm"}}"#
        );
    }

    #[test]
    fn a_direction_rejects_unknown_keys() {
        let record = json!({
            "id": "f-lp1", "type": "linear_pattern", "body": "b-1",
            "direction": {"axis": {"kind": "origin", "axis": "X"}, "count": "3", "spacing": "20", "skip": [1]}
        });
        let record: partsmith_model::FeatureRecord =
            serde_json::from_value(record).expect("well-formed");
        let mut registry = Registry::new();
        registry.register::<LinearPattern>().expect("registers");

        let error = registry
            .validate_and_canonicalize(&record)
            .expect_err("skip is not a direction key");

        assert_eq!(error.code(), "invalid_params");
    }

    #[test]
    fn dependencies_declare_both_directions_in_order() {
        let params = pattern(
            direction(X, "3", "20"),
            json!({"second_direction": direction(r#"{"kind": "sketch_line", "sketch": "f-sk1", "line": "e-1"}"#, "2", "15")}),
        );

        let dependencies = LinearPattern::dependencies(&params);

        let uses: Vec<(&str, &str, FieldUnit)> = dependencies
            .expressions
            .iter()
            .map(|usage| (usage.field.as_str(), usage.expr.as_str(), usage.kind))
            .collect();
        assert_eq!(
            uses,
            [
                ("direction.count", "3", FieldUnit::Count),
                ("direction.spacing", "20", FieldUnit::Length),
                ("second_direction.count", "2", FieldUnit::Count),
                ("second_direction.spacing", "15", FieldUnit::Length),
            ]
        );
        assert_eq!(dependencies.features[0].field, "second_direction.axis.sketch");
        assert_eq!(dependencies.bodies_written[0].id, id("b-1"));
    }

    #[test]
    fn three_apart_make_three_solids_named_by_instance() {
        let output = run(&pattern(direction(X, "3", "20"), json!({}))).expect("the pattern builds");

        let shape = updated_shape(&output, "b-1");
        assert_volume(shape, 3000.0);
        assert_bounds(shape, [-5.0, -5.0, 0.0], [45.0, 5.0, 10.0]);
        assert_eq!(shape.count(partsmith_occt_sys::SubShapeKind::Solid), 3);
        let names = face_names(&update(&output, "b-1").naming);
        assert!(names.contains(&"f-box1/top@f-lp1:1".to_string()));
        assert!(names.contains(&"f-box1/right@f-lp1:2".to_string()));
        assert!(names.contains(&"f-box1/right".to_string()));
        assert_eq!(update(&output, "b-1").change, BodyChange::Join);
    }

    #[test]
    fn a_grid_names_instances_with_both_indices() {
        let params = pattern(
            direction(X, "3", "20"),
            json!({"second_direction": direction(Y, "2", "15")}),
        );

        let output = run(&params).expect("the grid builds");

        let shape = updated_shape(&output, "b-1");
        assert_volume(shape, 6000.0);
        assert_bounds(shape, [-5.0, -5.0, 0.0], [45.0, 20.0, 10.0]);
        let names = face_names(&update(&output, "b-1").naming);
        for name in ["f-box1/top@f-lp1:1", "f-box1/top@f-lp1:0.1", "f-box1/top@f-lp1:2.1"] {
            assert!(names.contains(&name.to_string()), "{name} exists");
        }
    }

    #[test]
    fn reverse_goes_against_the_axis() {
        let mut direction = direction(X, "3", "20");
        direction["reverse"] = json!(true);

        let output = run(&pattern(direction, json!({}))).expect("the pattern builds");

        assert_bounds(
            updated_shape(&output, "b-1"),
            [-45.0, -5.0, 0.0],
            [5.0, 5.0, 10.0],
        );
    }

    #[test]
    fn op_new_fuses_the_copies_into_the_target() {
        let params = pattern(direction(X, "3", "20"), json!({"op": "new", "target": "b-2"}));

        let output = run(&params).expect("the pattern builds");

        assert_eq!(updates(&output).len(), 1);
        assert_eq!(update(&output, "b-2").change, BodyChange::New);
        let copies = updated_shape(&output, "b-2");
        assert_volume(copies, 2000.0);
        assert_bounds(copies, [15.0, -5.0, 0.0], [45.0, 5.0, 10.0]);
    }

    #[test]
    fn count_one_has_no_effect() {
        let output = run(&pattern(direction(X, "1", "20"), json!({}))).expect("no copies is fine");

        assert_eq!(output.warnings[0].code(), "no_effect");
    }

    #[test]
    fn a_zero_count_is_out_of_range() {
        let error = run(&pattern(direction(X, "0", "20"), json!({}))).expect_err("count 0");

        assert_eq!(error.field().expect("names a field").path, "direction.count");
        assert_eq!(
            error.to_string(),
            "Count must be at least 1, but it is 0. Use 1 or more."
        );
    }

    #[test]
    fn a_zero_spacing_is_out_of_range() {
        let error = run(&pattern(direction(X, "3", "0"), json!({}))).expect_err("spacing 0");

        assert_eq!(error.code(), "out_of_range");
        assert_eq!(error.field().expect("names a field").path, "direction.spacing");
    }

    #[test]
    fn more_than_ten_thousand_instances_are_too_many() {
        let params = pattern(
            direction(X, "200", "20"),
            json!({"second_direction": direction(Y, "100", "20")}),
        );

        let error = run(&params).expect_err("20000 instances");

        assert_eq!(
            error,
            FeatureError::TooManyInstances {
                count: 20_000,
                limit: 10_000
            }
        );
    }

    #[test]
    fn a_canceled_run_stops_before_the_first_copy() {
        let params = pattern(direction(X, "3", "20"), json!({}));
        let fixture = fixture::<LinearPattern>("f-lp1", &params, cube(), Vec::new());
        fixture.cancel.cancel();

        let result = LinearPattern::build(&params, &fixture.context());

        assert_eq!(result.map(|_| ()), Err(FeatureError::Canceled));
    }

    #[test]
    fn python_writes_the_directions_and_into() {
        let params = pattern(
            direction(X, "3", "20 mm"),
            json!({"second_direction": direction(Y, "2", "15 mm")}),
        );

        assert_eq!(
            python::<LinearPattern>(&params).as_deref(),
            Some("linear_pattern(Axis.X, 3, 20*mm, second=(Axis.Y, 2, 15*mm))")
        );
        let into = pattern(direction(X, "3", "20"), json!({"op": "new", "target": "b-2"}));
        assert_eq!(
            python::<LinearPattern>(&into).as_deref(),
            Some("linear_pattern(Axis.X, 3, 20, into=part_2)")
        );
    }

    #[test]
    fn python_writes_a_sketch_line_by_its_variable() {
        let mut names = ScriptNames::default();
        names
            .entities
            .insert((id("f-sk1"), id("e-1")), "line_1".to_string());
        let params = pattern(
            direction(r#"{"kind": "sketch_line", "sketch": "f-sk1", "line": "e-1"}"#, "2", "50"),
            json!({}),
        );

        assert_eq!(
            python_with::<LinearPattern>(&params, &names).as_deref(),
            Some("linear_pattern(line_1, 2, 50)")
        );
    }

    #[test]
    fn a_reversed_direction_has_no_dedicated_form() {
        let mut second = direction(Y, "2", "15");
        second["reverse"] = json!(true);
        let params = pattern(direction(X, "3", "20"), json!({"second_direction": second}));

        assert_eq!(python::<LinearPattern>(&params), None);
    }
}
