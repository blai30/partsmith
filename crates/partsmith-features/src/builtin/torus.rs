//! The `torus` feature: a ring torus centered on the placement origin (features/torus.md).

use partsmith_model::{Expr, FeatureManifest, FieldUnit, Id};
use partsmith_occt_sys::make_torus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{check_range, positive_length};
use super::primitives::{PrimitiveParams, primitive_manifest};
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::{Bound, FeatureError};
use crate::feature::Feature;
use crate::output::FeatureOutput;
use crate::params::{BodyOp, PlacementExt, PlaneRef};
use crate::primitive::{PrimitivePlacement, default_position, is_default_position};
use crate::python::{PythonCall, PythonNames};

/// A ring torus.
#[derive(Clone, Copy, Debug)]
pub struct Torus;

/// Params of a torus feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TorusParams {
    /// Target body.
    pub body: Id,
    /// Distance from the axis to the tube center. Length, greater than `minor_radius`.
    pub major_radius: Expr,
    /// Tube radius. Length, greater than 0.
    pub minor_radius: Expr,
    /// Placement plane. Default origin XY, omitted when default.
    #[serde(default, skip_serializing_if = "PlaneRef::is_default")]
    pub plane: PlaneRef,
    /// Placement offset (the center). Default ["0", "0", "0"], omitted when default.
    #[serde(
        default = "default_position",
        skip_serializing_if = "is_default_position"
    )]
    pub position: [Expr; 3],
    /// How the torus combines with the body. Required.
    pub op: BodyOp,
}

impl TorusParams {
    /// The params as the steps all primitives share read them.
    fn primitive(&self) -> PrimitiveParams<'_> {
        PrimitiveParams {
            body: &self.body,
            sizes: vec![
                ("major_radius", &self.major_radius),
                ("minor_radius", &self.minor_radius),
            ],
            placement: PrimitivePlacement {
                plane: &self.plane,
                position: &self.position,
            },
            op: self.op,
        }
    }
}

impl Feature for Torus {
    const TYPE: &'static str = "torus";
    type Params = TorusParams;

    fn manifest() -> FeatureManifest {
        primitive_manifest(
            "Torus",
            "torus",
            &[
                ("major_radius", "Ring radius"),
                ("minor_radius", "Tube radius"),
            ],
        )
    }

    fn dependencies(params: &TorusParams) -> Dependencies {
        params.primitive().dependencies()
    }

    fn build(
        params: &TorusParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let major = positive_length(context, "major_radius", &params.major_radius)?;
        let minor = positive_length(context, "minor_radius", &params.minor_radius)?;
        check_range(
            context,
            "minor_radius",
            FieldUnit::Length,
            minor,
            Bound::LessThan(major),
            Some("the ring radius"),
        )?;
        params.primitive().build(context, |frame| {
            make_torus(&frame.to_kernel(), major, minor)
        })
    }

    fn python(params: &TorusParams, names: &dyn PythonNames) -> Option<PythonCall> {
        params.primitive().python("torus", names)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::ScriptNames;

    #[test]
    fn python_writes_both_radii() {
        let params: TorusParams = serde_json::from_value(
            json!({"body": "b-1", "major_radius": "20", "minor_radius": "5", "op": "join"}),
        )
        .expect("valid params");

        let python = Torus::python(&params, &ScriptNames::default()).expect("a dedicated form");

        assert_eq!(python.render(0), r#"torus(20, 5, op="join")"#);
    }
}
