//! The `cylinder` feature: a right circular cylinder standing on a plane (features/cylinder.md).

use partsmith_model::{Category, Expr, FeatureManifest, FieldUnit, Id};
use partsmith_occt_sys::make_cylinder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    apply_tool, body_hint, body_op_hint, call, essential, manifest, op_keyword, positive_length,
    primitive_tool, unit_hint,
};
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::naming::kernel;
use crate::output::FeatureOutput;
use crate::params::{BodyOp, PlacementExt, PlaneRef};
use crate::primitive::{PrimitivePlacement, default_position, is_default_position};
use crate::python::{PythonCall, PythonNames, PythonValue};

/// A right circular cylinder.
#[derive(Clone, Copy, Debug)]
pub struct Cylinder;

/// Params of a cylinder feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CylinderParams {
    /// Target body.
    pub body: Id,
    /// Radius. Length, greater than 0.
    pub radius: Expr,
    /// Height along the placement normal. Length, greater than 0.
    pub height: Expr,
    /// Placement plane. Default origin XY, omitted when default.
    #[serde(default, skip_serializing_if = "PlaneRef::is_default")]
    pub plane: PlaneRef,
    /// Placement offset. Default ["0", "0", "0"], omitted when default.
    #[serde(
        default = "default_position",
        skip_serializing_if = "is_default_position"
    )]
    pub position: [Expr; 3],
    /// How the cylinder combines with the body. Required.
    pub op: BodyOp,
}

impl CylinderParams {
    /// The shared placement params.
    fn placement(&self) -> PrimitivePlacement<'_> {
        PrimitivePlacement {
            plane: &self.plane,
            position: &self.position,
        }
    }
}

impl Feature for Cylinder {
    const TYPE: &'static str = "cylinder";
    type Params = CylinderParams;

    fn manifest() -> FeatureManifest {
        let mut fields = vec![
            essential(unit_hint("radius", "Radius", FieldUnit::Length)),
            essential(unit_hint("height", "Height", FieldUnit::Length)),
            body_op_hint(),
        ];
        fields.extend(PrimitivePlacement::hints());
        fields.push(body_hint());
        manifest("Cylinder", Category::Create, "cylinder", fields)
    }

    fn dependencies(params: &CylinderParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_body_written(&params.body, "body");
        dependencies.add_expression("radius", &params.radius, FieldUnit::Length);
        dependencies.add_expression("height", &params.height, FieldUnit::Length);
        params.placement().add_dependencies(&mut dependencies);
        dependencies
    }

    fn build(
        params: &CylinderParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let radius = positive_length(context, "radius", &params.radius)?;
        let height = positive_length(context, "height", &params.height)?;
        let frame = params.placement().frame(context)?;
        let primitive = make_cylinder(&frame.to_kernel(), radius, height).map_err(kernel)?;
        let tool = primitive_tool(context.feature(), primitive)?;
        apply_tool(context, &params.body, params.op, tool, Vec::new())
    }

    fn python(params: &CylinderParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let sizes = vec![
            PythonValue::expression(&params.radius, names),
            PythonValue::expression(&params.height, names),
        ];
        let mut keywords = params.placement().python_keywords(names)?;
        keywords.extend(op_keyword(names, &params.body, params.op));
        Some(call("cylinder", sizes, keywords))
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use partsmith_occt_sys::{GeometryKind, Vector};
    use serde_json::json;

    use super::*;
    use crate::test_support::{ScriptNames, assert_point, run};

    fn cylinder(extra: serde_json::Value) -> serde_json::Value {
        let mut record = json!({"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "5", "height": "10", "op": "new"});
        if let (Some(record), Some(extra)) = (record.as_object_mut(), extra.as_object()) {
            record.extend(extra.clone());
        }
        record
    }

    #[test]
    fn a_default_cylinder_stands_on_xy() {
        let run = run(&["b-1"], &[cylinder(json!({}))]);

        run.assert_ok("f-cyl1");
        run.assert_volume("b-1", 250.0 * PI);
        run.assert_faces("b-1", 3);
        run.assert_bounding_box("b-1", [-5.0, -5.0, 0.0], [5.0, 5.0, 10.0]);
        let side = run.face("b-1", "f-cyl1/side");
        assert_eq!(side.kind, GeometryKind::Cylinder);
        assert_eq!(side.radius, Some(5.0));
        assert_eq!(
            side.direction.map(|direction| direction.z.abs()),
            Some(1.0)
        );
        run.assert_face_area("b-1", "f-cyl1/top", 25.0 * PI);
        assert_point("top", run.face("b-1", "f-cyl1/top").centroid, [0.0, 0.0, 10.0]);
    }

    #[test]
    fn a_cylinder_on_yz_points_along_x() {
        let run = run(
            &["b-1"],
            &[cylinder(json!({"plane": {"kind": "origin", "plane": "YZ"}}))],
        );

        run.assert_bounding_box("b-1", [0.0, -5.0, -5.0], [10.0, 5.0, 5.0]);
        assert_eq!(
            run.face("b-1", "f-cyl1/top").direction,
            Some(Vector::new(1.0, 0.0, 0.0))
        );
    }

    #[test]
    fn the_height_must_be_positive() {
        let run = run(&["b-1"], &[cylinder(json!({"height": "-2"}))]);

        assert_eq!(
            run.messages("f-cyl1"),
            ["Height must be greater than 0 mm, but it is -2 mm. Use a value greater than 0 mm."]
        );
    }

    #[test]
    fn the_radius_is_checked_first() {
        let run = run(&["b-1"], &[cylinder(json!({"radius": "0", "height": "0"}))]);

        assert_eq!(
            run.error("f-cyl1").field().map(|field| field.path.as_str()),
            Some("radius")
        );
    }

    #[test]
    fn a_round_hole_keeps_the_cylinder_side_name() {
        let run = run(
            &["b-1"],
            &[
                json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "position": ["0", "0", "-10"], "op": "new"}),
                json!({"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "3", "height": "10", "position": ["0", "0", "-10"], "op": "cut"}),
            ],
        );

        run.assert_volume("b-1", 15717.256661);
        run.assert_faces("b-1", 7);
        assert!(
            run.face_names("b-1")
                .contains(&"f-cyl1/side".to_string())
        );
    }

    #[test]
    fn python_writes_radius_and_height() {
        let params: CylinderParams =
            serde_json::from_value(json!({"body": "b-1", "radius": "5", "height": "10", "op": "new"}))
                .expect("valid params");

        let python = Cylinder::python(&params, &ScriptNames::default()).expect("a dedicated form");

        assert_eq!(python.render(0), "cylinder(5, 10)");
    }
}
