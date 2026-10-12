//! The `sphere` feature: a sphere centered on the placement origin (features/sphere.md).

use partsmith_model::{Category, Expr, FeatureManifest, FieldUnit, Id};
use partsmith_occt_sys::make_sphere;
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

/// A sphere.
#[derive(Clone, Copy, Debug)]
pub struct Sphere;

/// Params of a sphere feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SphereParams {
    /// Target body.
    pub body: Id,
    /// Radius. Length, greater than 0.
    pub radius: Expr,
    /// Placement plane. Default origin XY, omitted when default.
    #[serde(default, skip_serializing_if = "PlaneRef::is_default")]
    pub plane: PlaneRef,
    /// Placement offset (the center). Default ["0", "0", "0"], omitted when default.
    #[serde(
        default = "default_position",
        skip_serializing_if = "is_default_position"
    )]
    pub position: [Expr; 3],
    /// How the sphere combines with the body. Required.
    pub op: BodyOp,
}

impl SphereParams {
    /// The shared placement params.
    fn placement(&self) -> PrimitivePlacement<'_> {
        PrimitivePlacement {
            plane: &self.plane,
            position: &self.position,
        }
    }
}

impl Feature for Sphere {
    const TYPE: &'static str = "sphere";
    type Params = SphereParams;

    fn manifest() -> FeatureManifest {
        let mut fields = vec![
            essential(unit_hint("radius", "Radius", FieldUnit::Length)),
            body_op_hint(),
        ];
        fields.extend(PrimitivePlacement::hints());
        fields.push(body_hint());
        manifest("Sphere", Category::Create, "sphere", fields)
    }

    fn dependencies(params: &SphereParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_body_written(&params.body, "body");
        dependencies.add_expression("radius", &params.radius, FieldUnit::Length);
        params.placement().add_dependencies(&mut dependencies);
        dependencies
    }

    fn build(
        params: &SphereParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let radius = positive_length(context, "radius", &params.radius)?;
        let frame = params.placement().frame(context)?;
        let primitive = make_sphere(&frame.to_kernel(), radius).map_err(kernel)?;
        let tool = primitive_tool(context.feature(), primitive)?;
        apply_tool(context, &params.body, params.op, tool, Vec::new())
    }

    fn python(params: &SphereParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let sizes = vec![PythonValue::expression(&params.radius, names)];
        let mut keywords = params.placement().python_keywords(names)?;
        keywords.extend(op_keyword(names, &params.body, params.op));
        Some(call("sphere", sizes, keywords))
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use partsmith_occt_sys::GeometryKind;
    use serde_json::json;

    use super::*;
    use crate::test_support::{ScriptNames, run};

    /// A 20 mm cube standing on XY.
    fn cube() -> serde_json::Value {
        json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"})
    }

    #[test]
    fn a_default_sphere_is_centered_on_the_origin() {
        let run = run(
            &["b-1"],
            &[
                json!({"id": "f-sph1", "type": "sphere", "body": "b-1", "radius": "10", "op": "new"}),
            ],
        );

        run.assert_ok("f-sph1");
        run.assert_volume("b-1", 4000.0 * PI / 3.0);
        run.assert_faces("b-1", 1);
        run.assert_bounding_box("b-1", [-10.0; 3], [10.0; 3]);
        assert_eq!(run.face("b-1", "f-sph1/surface").kind, GeometryKind::Sphere);
        run.assert_face_area("b-1", "f-sph1/surface", 400.0 * PI);
    }

    #[test]
    fn a_dome_joins_a_box() {
        let run = run(
            &["b-1"],
            &[
                cube(),
                json!({"id": "f-sph1", "type": "sphere", "body": "b-1", "radius": "5", "position": ["0", "0", "20"], "op": "join"}),
            ],
        );

        run.assert_volume("b-1", 8261.799388);
        run.assert_faces("b-1", 7);
        run.assert_bounding_box("b-1", [-10.0, -10.0, 0.0], [10.0, 10.0, 25.0]);
    }

    #[test]
    fn a_dimple_cuts_a_box() {
        let run = run(
            &["b-1"],
            &[
                cube(),
                json!({"id": "f-sph1", "type": "sphere", "body": "b-1", "radius": "5", "position": ["0", "0", "20"], "op": "cut"}),
            ],
        );

        run.assert_volume("b-1", 7738.200612);
        run.assert_faces("b-1", 7);
    }

    #[test]
    fn a_zero_radius_is_out_of_range() {
        let run = run(
            &["b-1"],
            &[json!({"id": "f-sph1", "type": "sphere", "body": "b-1", "radius": "0", "op": "new"})],
        );

        assert_eq!(run.error_code("f-sph1"), "out_of_range");
    }

    #[test]
    fn python_writes_the_center() {
        let params: SphereParams = serde_json::from_value(
            json!({"body": "b-1", "radius": "10", "position": ["0", "0", "10"], "op": "new"}),
        )
        .expect("valid params");

        let python = Sphere::python(&params, &ScriptNames::default()).expect("a dedicated form");

        assert_eq!(python.render(0), "sphere(10, position=(0, 0, 10))");
    }
}
