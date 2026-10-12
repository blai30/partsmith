//! The `cylinder` feature: a right circular cylinder standing on a plane (features/cylinder.md).

use partsmith_model::{Expr, FeatureManifest, Id};
use partsmith_occt_sys::make_cylinder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::positive_length;
use super::primitives::{PrimitiveParams, primitive_manifest};
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::output::FeatureOutput;
use crate::params::{BodyOp, PlacementExt, PlaneRef};
use crate::primitive::{PrimitivePlacement, default_position, is_default_position};
use crate::python::{PythonCall, PythonNames};

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
    /// The params as the steps all primitives share read them.
    fn primitive(&self) -> PrimitiveParams<'_> {
        PrimitiveParams {
            body: &self.body,
            sizes: vec![("radius", &self.radius), ("height", &self.height)],
            placement: PrimitivePlacement {
                plane: &self.plane,
                position: &self.position,
            },
            op: self.op,
        }
    }
}

impl Feature for Cylinder {
    const TYPE: &'static str = "cylinder";
    type Params = CylinderParams;

    fn manifest() -> FeatureManifest {
        primitive_manifest(
            "Cylinder",
            "cylinder",
            &[("radius", "Radius"), ("height", "Height")],
        )
    }

    fn dependencies(params: &CylinderParams) -> Dependencies {
        params.primitive().dependencies()
    }

    fn build(
        params: &CylinderParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let radius = positive_length(context, "radius", &params.radius)?;
        let height = positive_length(context, "height", &params.height)?;
        params.primitive().build(context, |frame| {
            make_cylinder(&frame.to_kernel(), radius, height)
        })
    }

    fn python(params: &CylinderParams, names: &dyn PythonNames) -> Option<PythonCall> {
        params.primitive().python("cylinder", names)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::{ScriptNames, run};

    #[test]
    fn the_radius_is_checked_first() {
        let run = run(
            &["b-1"],
            &[
                json!({"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "0", "height": "0", "op": "new"}),
            ],
        );

        assert_eq!(
            run.error("f-cyl1").field().map(|field| field.path.as_str()),
            Some("radius")
        );
    }

    #[test]
    fn python_writes_radius_and_height() {
        let params: CylinderParams = serde_json::from_value(
            json!({"body": "b-1", "radius": "5", "height": "10", "op": "new"}),
        )
        .expect("valid params");

        let python = Cylinder::python(&params, &ScriptNames::default()).expect("a dedicated form");

        assert_eq!(python.render(0), "cylinder(5, 10)");
    }
}
