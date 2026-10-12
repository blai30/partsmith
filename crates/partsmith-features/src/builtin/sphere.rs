//! The `sphere` feature: a sphere centered on the placement origin (docs/spec/features/sphere.md).

use partsmith_model::{Expr, Id};
use partsmith_occt_sys::make_sphere;
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

/// A sphere.
#[derive(Clone, Copy, Debug, Default)]
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
        primitives::manifest("Sphere", "sphere", &[RADIUS])
    }

    fn dependencies(params: &SphereParams) -> Dependencies {
        primitives::dependencies(
            &params.body,
            &[(RADIUS, &params.radius)],
            params.placement(),
        )
    }

    fn build(
        params: &SphereParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let radius = primitives::positive_length(context, RADIUS.path, &params.radius)?;
        let frame = params.placement().frame(context)?;
        let primitive = make_sphere(&frame.to_kernel(), radius).map_err(primitives::kernel)?;
        primitives::combine(context, &params.body, params.op, primitive)
    }

    fn python(params: &SphereParams, names: &dyn PythonNames) -> Option<PythonCall> {
        primitives::python(
            "sphere",
            &[&params.radius],
            &params.body,
            params.op,
            params.placement(),
            names,
        )
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::builtin::primitives::testing::{build, params, python};
    use crate::test_support::face_names;
    use crate::{OutputData, Registry};

    /// The sphere of sphere.md section 2: radius 10 resting on XY.
    fn sphere() -> SphereParams {
        params::<Sphere>(json!({
            "id": "f-sph1", "type": "sphere", "body": "b-1", "radius": "10",
            "position": ["0", "0", "10"], "op": "new"
        }))
    }

    #[test]
    fn registers_with_its_manifest() {
        let mut registry = Registry::new();

        registry
            .register::<Sphere>()
            .expect("the sphere passes the registration checks");

        assert_eq!(registry.types().collect::<Vec<_>>(), ["sphere"]);
    }

    #[test]
    fn builds_one_surface_around_the_position() {
        let output = build::<Sphere>(&sphere(), Vec::new()).expect("the sphere builds");

        let OutputData::Bodies(updates) = &output.data else {
            panic!("a sphere outputs body updates");
        };
        assert_eq!(face_names(&updates[0].naming), ["f-prim1/surface"]);
        let bounds = updates[0]
            .shape
            .as_ref()
            .expect("the body holds the sphere")
            .bounding_box()
            .expect("the sphere has bounds");
        assert!((bounds.min[2] - 0.0).abs() < 1e-9);
        assert!((bounds.max[2] - 20.0).abs() < 1e-9);
    }

    #[test]
    fn a_zero_radius_is_out_of_range() {
        let mut params = sphere();
        params.radius = Expr::new("0");

        let error = build::<Sphere>(&params, Vec::new()).expect_err("no radius fails");

        assert_eq!(error.code(), "out_of_range");
        assert_eq!(error.field().expect("names its field").path, "radius");
    }

    #[test]
    fn python_writes_radius_and_position() {
        assert_eq!(
            python::<Sphere>(&sphere(), BodyOp::New).as_deref(),
            Some("sphere(10, position=(0, 0, 10))")
        );
    }
}
