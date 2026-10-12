//! The `move_body` feature: rotates a body about an axis, then translates it, keeping every element
//! name (docs/spec/features/move_body.md).

use std::f64::consts::TAU;
use std::sync::Arc;

use partsmith_model::{Category, Expr, FeatureManifest, FieldUnit, Id, SelectionKind};
use partsmith_occt_sys::{Transform, Vector, transform};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    bodies, essential, manifest, selection_hint, solid_body, unchanged, unit_hint,
};
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::{FeatureError, FeatureWarning};
use crate::feature::Feature;
use crate::naming::kernel;
use crate::output::{BodyChange, BodyUpdate, FeatureOutput};
use crate::params::AxisRef;
use crate::python::{PythonCall, PythonNames, PythonValue};
use crate::{default_position, is_default_position};

/// Field paths of the three translation components.
const TRANSLATION_FIELDS: [&str; 3] = ["translation[0]", "translation[1]", "translation[2]"];

/// A translation component at most this large in millimeters, or a rotation within this many
/// radians of a whole number of turns, moves nothing (move_body.md section 3.2).
const NOTHING_MOVES: f64 = 1e-12;

/// Rotates and then translates a body.
#[derive(Clone, Copy, Debug, Default)]
pub struct MoveBody;

/// Params of a move body feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MoveBodyParams {
    /// The body to move.
    pub body: Id,
    /// Translation along world X, Y and Z, applied after the rotation. Lengths. Default
    /// ["0", "0", "0"], omitted when default.
    #[serde(
        default = "default_position",
        skip_serializing_if = "is_default_position"
    )]
    pub translation: [Expr; 3],
    /// Rotation applied first. Omitted when None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<MoveBodyRotation>,
}

/// A rotation about an axis. Named with the feature prefix so its schema definition is unique
/// (features.md section 2.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveBodyRotation {
    /// The axis, resolved before the move.
    pub axis: AxisRef,
    /// Angle, positive by the right-hand rule about the axis direction. Any value.
    pub angle: Expr,
}

impl Feature for MoveBody {
    const TYPE: &'static str = "move_body";
    type Params = MoveBodyParams;

    fn manifest() -> FeatureManifest {
        let mut fields = vec![essential(selection_hint(
            "body",
            "Body",
            SelectionKind::Body,
        ))];
        fields.extend(
            TRANSLATION_FIELDS
                .into_iter()
                .zip(["Move X", "Move Y", "Move Z"])
                .map(|(path, label)| essential(unit_hint(path, label, FieldUnit::Length))),
        );
        fields.push(selection_hint(
            "rotation.axis",
            "Rotation axis",
            SelectionKind::Axis,
        ));
        fields.push(unit_hint(
            "rotation.angle",
            "Rotation angle",
            FieldUnit::Angle,
        ));
        manifest("Move body", Category::Modify, "move-body", fields)
    }

    /// Writes the body; reads the translation lengths, then what the rotation axis reads and the
    /// rotation angle (move_body.md section 3.1).
    fn dependencies(params: &MoveBodyParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_body_written(&params.body, "body");
        for (field, expr) in TRANSLATION_FIELDS.iter().zip(&params.translation) {
            dependencies.add_expression(field, expr, FieldUnit::Length);
        }
        if let Some(rotation) = &params.rotation {
            dependencies.add_axis("rotation.axis", &rotation.axis);
            dependencies.add_expression("rotation.angle", &rotation.angle, FieldUnit::Angle);
        }
        dependencies
    }

    /// The kernel transform of the body by the rotation, then the translation. The naming map is
    /// carried over unchanged, not rebuilt, so no `#n` group is renumbered by the motion
    /// (move_body.md section 3.2, naming.md section 3.6.12).
    fn build(
        params: &MoveBodyParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let (state, shape) = solid_body(context, &params.body)?;
        let mut translation = [0.0; 3];
        for ((value, field), expr) in translation
            .iter_mut()
            .zip(TRANSLATION_FIELDS)
            .zip(&params.translation)
        {
            *value = context.eval_length(field, expr)?;
        }
        let rotation = match &params.rotation {
            Some(rotation) => {
                let angle = context.eval_angle("rotation.angle", &rotation.angle)?;
                let axis = context.axis("rotation.axis", &rotation.axis)?;
                Some((axis, angle))
            }
            None => None,
        };
        let turns_nothing = rotation
            .as_ref()
            .is_none_or(|(_, angle)| is_whole_turns(*angle));
        if turns_nothing && translation.iter().all(|value| value.abs() <= NOTHING_MOVES) {
            let warning = FeatureWarning::NoEffect {
                body: params.body.clone(),
            };
            return Ok(bodies(
                vec![unchanged(&params.body, state, BodyChange::Modify)],
                vec![warning],
            ));
        }
        let [x, y, z] = translation;
        let motion = rotation
            .map_or(Transform::IDENTITY, |(axis, angle)| {
                Transform::rotation(&axis, angle)
            })
            .then(&Transform::translation(Vector::new(x, y, z)));
        let moved = transform(shape, &motion).map_err(kernel)?;
        let update = BodyUpdate {
            body: params.body.clone(),
            shape: Some(moved.shape),
            naming: Arc::clone(state.naming()),
            change: BodyChange::Modify,
        };
        Ok(bodies(vec![update], Vec::new()))
    }

    /// `move(translate=(x, y, z), rotate=(axis, angle))` (move_body.md section 2): `translate`
    /// when it is not the default, `rotate` when a rotation is set. None when the axis has no
    /// variable.
    fn python(params: &MoveBodyParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let mut call = PythonCall::new("move");
        if !is_default_position(&params.translation) {
            let components = params
                .translation
                .iter()
                .map(|expr| PythonValue::expression(expr, names))
                .collect();
            call.keywords
                .push(("translate".to_string(), PythonValue::Tuple(components)));
        }
        if let Some(rotation) = &params.rotation {
            let rotate = PythonValue::Tuple(vec![
                PythonValue::axis(&rotation.axis, names)?,
                PythonValue::expression(&rotation.angle, names),
            ]);
            call.keywords.push(("rotate".to_string(), rotate));
        }
        Some(call)
    }
}

/// True when `angle` in radians is a whole number of turns within `NOTHING_MOVES`.
fn is_whole_turns(angle: f64) -> bool {
    let remainder = angle.rem_euclid(TAU);
    remainder <= NOTHING_MOVES || TAU - remainder <= NOTHING_MOVES
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use partsmith_naming::ElementName;
    use partsmith_occt_sys::{GeometryKind, SubShapeKind};
    use serde_json::{Value, json};

    use super::*;
    use crate::builtin::copy::testing::{
        assert_bounds, assert_volume, build, params, python, update, updated_shape, updates,
    };
    use crate::context::BodyState;
    use crate::registry::Registry;
    use crate::test_support::{box_body, id};

    fn move_body(extra: Value) -> MoveBodyParams {
        let mut record = json!({"id": "f-mv1", "type": "move_body", "body": "b-1"});
        if let Value::Object(members) = extra {
            for (key, value) in members {
                record[key] = value;
            }
        }
        params::<MoveBody>(record)
    }

    fn about_z(angle: &str) -> Value {
        json!({"axis": {"kind": "origin", "axis": "Z"}, "angle": angle})
    }

    /// A 10 mm cube in `b-1` spanning [-5,-5,0]-[5,5,10].
    fn cube() -> Vec<(&'static str, BodyState)> {
        vec![("b-1", box_body("f-box1", [-5.0, -5.0, 0.0], 10.0))]
    }

    fn run(params: &MoveBodyParams) -> Result<FeatureOutput, FeatureError> {
        build::<MoveBody>("f-mv1", params, cube(), Vec::new())
    }

    #[test]
    fn registers_with_its_manifest() {
        let mut registry = Registry::new();

        registry
            .register::<MoveBody>()
            .expect("the move passes the registration checks");

        assert_eq!(registry.types().collect::<Vec<_>>(), ["move_body"]);
    }

    #[test]
    fn canonical_params_omit_a_zero_translation() {
        let params =
            move_body(json!({"rotation": about_z("90 deg"), "translation": ["0", "0", "0"]}));

        assert_eq!(
            serde_json::to_string(&params).expect("serializes"),
            r#"{"body":"b-1","rotation":{"axis":{"kind":"origin","axis":"Z"},"angle":"90 deg"}}"#
        );
    }

    #[test]
    fn dependencies_declare_translation_then_angle() {
        let params =
            move_body(json!({"translation": ["5", "0", "0"], "rotation": about_z("90 deg")}));

        let dependencies = MoveBody::dependencies(&params);

        let fields: Vec<(&str, FieldUnit)> = dependencies
            .expressions
            .iter()
            .map(|usage| (usage.field.as_str(), usage.kind))
            .collect();
        assert_eq!(
            fields,
            [
                ("translation[0]", FieldUnit::Length),
                ("translation[1]", FieldUnit::Length),
                ("translation[2]", FieldUnit::Length),
                ("rotation.angle", FieldUnit::Angle)
            ]
        );
        assert_eq!(dependencies.bodies_written[0].id, id("b-1"));
    }

    #[test]
    fn an_edge_of_the_moved_body_keeps_it_written() {
        let params = move_body(json!({"rotation": {
            "axis": {"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-box1/right|f-box1/top)", "fp": "line|n=0,1,0"}},
            "angle": "180 deg"
        }}));

        let dependencies = MoveBody::dependencies(&params);

        assert_eq!(dependencies.bodies_written.len(), 1);
        assert!(dependencies.bodies_read.is_empty());
    }

    #[test]
    fn a_translation_keeps_every_name() {
        let before = cube();
        let output =
            run(&move_body(json!({"translation": ["5", "0", "0"]}))).expect("the move builds");

        let shape = updated_shape(&output, "b-1");
        assert_volume(shape, 1000.0);
        assert_bounds(shape, [0.0, -5.0, 0.0], [10.0, 5.0, 10.0]);
        let moved = update(&output, "b-1");
        assert_eq!(moved.change, BodyChange::Modify);
        assert_eq!(*moved.naming, **before[0].1.naming());
    }

    #[test]
    fn the_rotation_comes_before_the_translation() {
        let params =
            move_body(json!({"translation": ["5", "0", "0"], "rotation": about_z("90 deg")}));
        // The cube spans [-5,-10,0]-[5,0,10]: turned a quarter about Z it spans x 0..10 and
        // y -5..5, then moves 5 along X. Translating first would end at x 0..10, y 0..10.
        let bodies = vec![("b-1", box_body("f-box1", [-5.0, -10.0, 0.0], 10.0))];

        let output =
            build::<MoveBody>("f-mv1", &params, bodies, Vec::new()).expect("the move builds");

        assert_bounds(
            updated_shape(&output, "b-1"),
            [5.0, -5.0, 0.0],
            [15.0, 5.0, 10.0],
        );
    }

    #[test]
    fn a_rotation_turns_the_front_face_to_plus_x() {
        let output =
            run(&move_body(json!({"rotation": about_z("90 deg")}))).expect("the move builds");

        let name = ElementName::parse("f-box1/front").expect("valid name");
        let index = update(&output, "b-1")
            .naming
            .index(&name)
            .expect("the front face keeps its name");
        let front = updated_shape(&output, "b-1")
            .element(SubShapeKind::Face, index)
            .expect("the face has geometry");
        assert_eq!(front.kind, GeometryKind::Plane);
        let normal = front.direction.expect("a plane has a normal");
        assert!((normal.x - 1.0).abs() < 1e-9, "normal {normal:?}");
    }

    #[test]
    fn a_whole_turn_without_translation_moves_nothing() {
        let state = cube();
        let params =
            move_body(json!({"translation": ["0", "0", "0"], "rotation": about_z("360 deg")}));

        let output =
            build::<MoveBody>("f-mv1", &params, state.clone(), Vec::new()).expect("nothing moves");

        assert_eq!(
            output.warnings,
            [FeatureWarning::NoEffect { body: id("b-1") }]
        );
        let kept = &updates(&output)[0];
        assert_eq!(kept.change, BodyChange::Modify);
        assert!(Arc::ptr_eq(&kept.naming, state[0].1.naming()));
    }

    #[test]
    fn whole_turns_are_recognized_in_both_senses() {
        assert!(is_whole_turns(0.0));
        assert!(is_whole_turns(2.0 * TAU));
        assert!(is_whole_turns(-TAU));
        assert!(!is_whole_turns(PI));
        assert!(!is_whole_turns(1e-9));
    }

    #[test]
    fn an_empty_body_has_nothing_to_move() {
        let error = build::<MoveBody>(
            "f-mv1",
            &move_body(json!({"translation": ["5", "0", "0"]})),
            Vec::new(),
            Vec::new(),
        )
        .expect_err("b-1 is empty");

        assert_eq!(error.code(), "body_empty");
    }

    #[test]
    fn a_length_as_the_angle_is_an_expression_error() {
        let error =
            run(&move_body(json!({"rotation": about_z("5 mm")}))).expect_err("5 mm is no angle");

        assert_eq!(error.field().expect("names a field").path, "rotation.angle");
        assert_eq!(
            error.to_string(),
            "Rotation angle: Expected an angle, but this expression gives a length."
        );
    }

    #[test]
    fn python_writes_translate_and_rotate() {
        let params =
            move_body(json!({"translation": ["5", "0", "0"], "rotation": about_z("90 deg")}));

        assert_eq!(
            python::<MoveBody>(&params).as_deref(),
            Some("move(translate=(5, 0, 0), rotate=(Axis.Z, 90*deg))")
        );
        assert_eq!(
            python::<MoveBody>(&move_body(json!({"translation": ["0", "0", "3 mm"]}))).as_deref(),
            Some("move(translate=(0, 0, 3*mm))")
        );
    }
}
