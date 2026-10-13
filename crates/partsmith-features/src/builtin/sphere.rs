//! The `sphere` feature: a sphere centered on the placement origin (features/sphere.md).

use partsmith_model::{Expr, FeatureManifest, Id};
use partsmith_occt_sys::make_sphere;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::primitives::{PrimitiveParams, primitive_manifest};
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::output::FeatureOutput;
use crate::params::{BodyOp, PlacementExt, PlaneRef};
use crate::python::{PythonCall, PythonNames};
use crate::{PrimitivePlacement, add_position_default, default_position, is_default_position};

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
    #[schemars(transform = add_position_default)]
    pub position: [Expr; 3],
    /// How the sphere combines with the body. Required.
    pub op: BodyOp,
}

impl SphereParams {
    /// The params as the steps all primitives share read them.
    fn primitive(&self) -> PrimitiveParams<'_> {
        PrimitiveParams {
            body: &self.body,
            sizes: vec![("radius", &self.radius)],
            placement: PrimitivePlacement {
                plane: &self.plane,
                position: &self.position,
            },
            op: self.op,
        }
    }
}

impl Feature for Sphere {
    const TYPE: &'static str = "sphere";
    type Params = SphereParams;

    fn manifest() -> FeatureManifest {
        primitive_manifest("Sphere", "sphere", &[("radius", "Radius")])
    }

    fn dependencies(params: &SphereParams) -> Dependencies {
        params.primitive().dependencies()
    }

    fn build(
        params: &SphereParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let primitive = params.primitive();
        let [radius] = primitive.positive_sizes(context)?;
        primitive.build(context, |frame| make_sphere(&frame.to_kernel(), radius))
    }

    fn python(params: &SphereParams, names: &dyn PythonNames) -> Option<PythonCall> {
        params.primitive().python("sphere", names)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::ScriptNames;

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
