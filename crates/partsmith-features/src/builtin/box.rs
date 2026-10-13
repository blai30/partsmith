//! The `box` feature: a rectangular block standing on a plane, centered on the placement origin in
//! x and y (features/box.md).

use partsmith_model::{Expr, FeatureManifest, Id};
use partsmith_occt_sys::make_box;
use partsmith_sketch::Placement;
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

/// Field paths of the three sizes.
const SIZE_FIELDS: [&str; 3] = ["size[0]", "size[1]", "size[2]"];

/// A rectangular block. The type name in files is `box`; `Box` is the standard library's pointer.
#[derive(Clone, Copy, Debug)]
pub struct BoxFeature;

/// Params of a box feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BoxParams {
    /// Target body.
    pub body: Id,
    /// Size along the placement x, y and z axes. Lengths, greater than 0.
    pub size: [Expr; 3],
    /// Placement plane. Default origin XY, omitted when default.
    #[serde(default, skip_serializing_if = "PlaneRef::is_default")]
    pub plane: PlaneRef,
    /// Placement offset. Default ["0", "0", "0"], omitted when default.
    #[serde(
        default = "default_position",
        skip_serializing_if = "is_default_position"
    )]
    #[schemars(transform = add_position_default)]
    pub position: [Expr; 3],
    /// How the box combines with the body. Required.
    pub op: BodyOp,
}

impl BoxParams {
    /// The params as the steps all primitives share read them.
    fn primitive(&self) -> PrimitiveParams<'_> {
        PrimitiveParams {
            body: &self.body,
            sizes: SIZE_FIELDS.into_iter().zip(&self.size).collect(),
            placement: PrimitivePlacement {
                plane: &self.plane,
                position: &self.position,
            },
            op: self.op,
        }
    }
}

impl Feature for BoxFeature {
    const TYPE: &'static str = "box";
    type Params = BoxParams;

    fn manifest() -> FeatureManifest {
        let sizes: Vec<_> = SIZE_FIELDS
            .into_iter()
            .zip(["Length", "Width", "Height"])
            .collect();
        primitive_manifest("Box", "box", &sizes)
    }

    fn dependencies(params: &BoxParams) -> Dependencies {
        params.primitive().dependencies()
    }

    fn build(
        params: &BoxParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let primitive = params.primitive();
        let [length, width, height] = primitive.positive_sizes(context)?;
        primitive.build(context, |frame| {
            // The kernel's box starts at its placement origin; center it in x and y.
            let corner = Placement {
                origin: frame.local_to_world([-length / 2.0, -width / 2.0, 0.0]),
                ..*frame
            };
            make_box(&corner.to_kernel(), length, width, height)
        })
    }

    fn python(params: &BoxParams, names: &dyn PythonNames) -> Option<PythonCall> {
        params.primitive().python("box", names)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::ScriptNames;

    #[test]
    fn python_writes_sizes_then_placement_then_op() {
        let names = ScriptNames::default();
        let params: BoxParams = serde_json::from_value(json!({"body": "b-1", "size": ["10", "10", "5 mm"], "plane": {"kind": "datum", "feature": "f-dp1"}, "position": ["20", "0", "0"], "op": "cut"})).expect("valid params");

        let python = BoxFeature::python(&params, &names).expect("a dedicated form");

        assert_eq!(
            python.render(0),
            r#"box(10, 10, 5*mm, plane=plane_1, position=(20, 0, 0), op="cut")"#
        );
    }
}
