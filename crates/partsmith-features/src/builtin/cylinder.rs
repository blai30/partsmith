//! The `cylinder` feature: a right circular cylinder standing on a plane, its base centered on the
//! placement origin and its axis along the plane normal (docs/spec/features/cylinder.md).

use partsmith_model::{Expr, Id};
use partsmith_occt_sys::make_cylinder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::primitives::{self, SizeField};
use crate::{
    BodyOp, BuildContext, Dependencies, Feature, FeatureError, FeatureManifest, FeatureOutput,
    PlacementExt, PlaneRef, PrimitivePlacement, PythonCall, PythonNames, default_position,
    is_default_position,
};

/// The radius field.
const RADIUS: SizeField = SizeField {
    path: "radius",
    label: "Radius",
};

/// The height field.
const HEIGHT: SizeField = SizeField {
    path: "height",
    label: "Height",
};

/// A right circular cylinder.
#[derive(Clone, Copy, Debug, Default)]
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
        primitives::manifest("Cylinder", "cylinder", &[RADIUS, HEIGHT])
    }

    fn dependencies(params: &CylinderParams) -> Dependencies {
        primitives::dependencies(
            &params.body,
            &[(RADIUS, &params.radius), (HEIGHT, &params.height)],
            params.placement(),
        )
    }

    fn build(
        params: &CylinderParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let radius = primitives::positive_length(context, RADIUS.path, &params.radius)?;
        let height = primitives::positive_length(context, HEIGHT.path, &params.height)?;
        let frame = params.placement().frame(context)?;
        let primitive =
            make_cylinder(&frame.to_kernel(), radius, height).map_err(primitives::kernel)?;
        primitives::combine(context, &params.body, params.op, primitive)
    }

    fn python(params: &CylinderParams, names: &dyn PythonNames) -> Option<PythonCall> {
        primitives::python(
            "cylinder",
            &[&params.radius, &params.height],
            &params.body,
            params.op,
            params.placement(),
            names,
        )
    }
}

#[cfg(test)]
mod tests {
    use partsmith_naming::ElementName;
    use partsmith_occt_sys::{GeometryKind, SubShapeKind};
    use serde_json::json;

    use super::*;
    use crate::builtin::primitives::testing::{build, params, python};
    use crate::test_support::face_names;
    use crate::{BodyUpdate, OutputData, Registry};

    /// The `CYL` record of cylinder.md section 6.
    fn cylinder() -> CylinderParams {
        params::<Cylinder>(json!({
            "id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "5", "height": "10", "op": "new"
        }))
    }

    fn update(output: &FeatureOutput) -> &BodyUpdate {
        let OutputData::Bodies(updates) = &output.data else {
            panic!("a cylinder outputs body updates");
        };
        &updates[0]
    }

    #[test]
    fn registers_with_its_manifest() {
        let mut registry = Registry::new();

        registry
            .register::<Cylinder>()
            .expect("the cylinder passes the registration checks");

        assert_eq!(registry.types().collect::<Vec<_>>(), ["cylinder"]);
    }

    #[test]
    fn dependencies_read_radius_then_height() {
        let dependencies = Cylinder::dependencies(&cylinder());

        let fields: Vec<&str> = dependencies
            .expressions
            .iter()
            .map(|usage| usage.field.as_str())
            .collect();
        assert_eq!(fields[..2], ["radius", "height"]);
        assert_eq!(fields.len(), 5);
    }

    #[test]
    fn builds_side_top_and_bottom() {
        let output = build::<Cylinder>(&cylinder(), Vec::new()).expect("the cylinder builds");

        assert_eq!(
            face_names(&update(&output).naming),
            ["f-prim1/bottom", "f-prim1/side", "f-prim1/top"]
        );
    }

    #[test]
    fn the_side_has_the_radius() {
        let output = build::<Cylinder>(&cylinder(), Vec::new()).expect("the cylinder builds");

        let update = update(&output);
        let side = ElementName::parse("f-prim1/side").expect("valid name");
        let index = update.naming.index(&side).expect("the side is named");
        let geometry = update
            .shape
            .as_ref()
            .expect("the body holds the cylinder")
            .element(SubShapeKind::Face, index)
            .expect("the side has geometry");
        assert_eq!(geometry.kind, GeometryKind::Cylinder);
        assert!((geometry.radius.expect("a cylinder has a radius") - 5.0).abs() < 1e-9);
    }

    #[test]
    fn a_zero_radius_is_out_of_range() {
        let mut params = cylinder();
        params.radius = Expr::new("0");

        let error = build::<Cylinder>(&params, Vec::new()).expect_err("no radius fails");

        assert_eq!(error.field().expect("names its field").path, "radius");
        assert_eq!(
            error.to_string(),
            "Radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."
        );
    }

    #[test]
    fn a_negative_height_is_out_of_range() {
        let mut params = cylinder();
        params.height = Expr::new("-2");

        let error = build::<Cylinder>(&params, Vec::new()).expect_err("a negative height fails");

        assert_eq!(
            error.to_string(),
            "Height must be greater than 0 mm, but it is -2 mm. Use a value greater than 0 mm."
        );
    }

    #[test]
    fn python_writes_radius_and_height() {
        assert_eq!(
            python::<Cylinder>(&cylinder(), BodyOp::New).as_deref(),
            Some("cylinder(5, 10)")
        );
    }

    #[test]
    fn python_writes_a_plane_other_than_xy() {
        let mut params = cylinder();
        params.plane = serde_json::from_value(json!({"kind": "origin", "plane": "YZ"}))
            .expect("an origin plane deserializes");

        assert_eq!(
            python::<Cylinder>(&params, BodyOp::New).as_deref(),
            Some("cylinder(5, 10, plane=Plane.YZ)")
        );
    }
}
