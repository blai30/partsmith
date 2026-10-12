//! The `cone` feature: a right circular cone or frustum standing on a plane (features/cone.md).

use partsmith_model::{Expr, FeatureManifest, FieldUnit, Id, LINEAR_TOLERANCE};
use partsmith_occt_sys::{KernelError, Plane, Primitive, make_cone, make_cylinder};
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

/// A right circular cone or frustum.
#[derive(Clone, Copy, Debug)]
pub struct Cone;

/// Params of a cone feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ConeParams {
    /// Target body.
    pub body: Id,
    /// Radius at z = 0. Length, at least 0.
    pub bottom_radius: Expr,
    /// Radius at z = height. Length, at least 0. Not both radii 0.
    pub top_radius: Expr,
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
    /// How the cone combines with the body. Required.
    pub op: BodyOp,
}

impl ConeParams {
    /// The params as the steps all primitives share read them.
    fn primitive(&self) -> PrimitiveParams<'_> {
        PrimitiveParams {
            body: &self.body,
            sizes: vec![
                ("bottom_radius", &self.bottom_radius),
                ("top_radius", &self.top_radius),
                ("height", &self.height),
            ],
            placement: PrimitivePlacement {
                plane: &self.plane,
                position: &self.position,
            },
            op: self.op,
        }
    }
}

/// A radius that must be at least 0, as the kernel builds it (`kernel_radius`).
fn radius(context: &BuildContext<'_>, field: &str, expr: &Expr) -> Result<f64, FeatureError> {
    let value = context.eval_length(field, expr)?;
    check_range(
        context,
        field,
        FieldUnit::Length,
        value,
        Bound::AtLeast(0.0),
        None,
    )
    .map(kernel_radius)
}

/// A radius of at least 0 as the kernel builds it: one below the kernel's modeling tolerance is a
/// point (an apex), so it counts as 0, because the kernel builds no circle that small.
fn kernel_radius(radius: f64) -> f64 {
    if radius < LINEAR_TOLERANCE {
        0.0
    } else {
        radius
    }
}

/// The kernel solid of checked radii and height in the local frame. The kernel's cone needs radii
/// that differ by at least its modeling tolerance, so closer radii make a cylinder.
fn solid(frame: &Plane, bottom: f64, top: f64, height: f64) -> Result<Primitive, KernelError> {
    if (bottom - top).abs() < LINEAR_TOLERANCE {
        make_cylinder(frame, bottom, height)
    } else {
        make_cone(frame, bottom, top, height)
    }
}

impl Feature for Cone {
    const TYPE: &'static str = "cone";
    type Params = ConeParams;

    fn manifest() -> FeatureManifest {
        primitive_manifest(
            "Cone",
            "cone",
            &[
                ("bottom_radius", "Bottom radius"),
                ("top_radius", "Top radius"),
                ("height", "Height"),
            ],
        )
    }

    fn dependencies(params: &ConeParams) -> Dependencies {
        params.primitive().dependencies()
    }

    fn build(
        params: &ConeParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let bottom = radius(context, "bottom_radius", &params.bottom_radius)?;
        let top = radius(context, "top_radius", &params.top_radius)?;
        if bottom == 0.0 && top == 0.0 {
            return Err(FeatureError::ConeRadiiZero);
        }
        let height = positive_length(context, "height", &params.height)?;
        params.primitive().build(context, |frame| {
            solid(&frame.to_kernel(), bottom, top, height)
        })
    }

    fn python(params: &ConeParams, names: &dyn PythonNames) -> Option<PythonCall> {
        params.primitive().python("cone", names)
    }
}

#[cfg(test)]
mod tests {
    use partsmith_occt_sys::{GeometryKind, PrimitiveFace, SubShapeKind};
    use serde_json::json;

    use super::*;
    use crate::test_support::ScriptNames;

    #[test]
    fn a_radius_below_the_tolerance_is_an_apex() {
        assert_eq!(kernel_radius(0.0), 0.0);
        assert_eq!(kernel_radius(LINEAR_TOLERANCE / 2.0), 0.0);
        assert_eq!(kernel_radius(LINEAR_TOLERANCE), LINEAR_TOLERANCE);
        assert_eq!(kernel_radius(5.0), 5.0);
    }

    #[test]
    fn radii_the_feature_accepts_always_build_in_the_kernel() {
        let tolerance = LINEAR_TOLERANCE;
        let radii = [
            0.0,
            tolerance / 2.0,
            tolerance,
            1.5 * tolerance,
            5.0,
            5.0 + tolerance / 2.0,
            5.0 + tolerance,
            5.0 + 2.0 * tolerance,
        ];
        for bottom in radii.map(kernel_radius) {
            for top in radii.map(kernel_radius) {
                if bottom == 0.0 && top == 0.0 {
                    continue;
                }

                let built = solid(&Plane::XY, bottom, top, 10.0);

                assert!(
                    built.is_ok(),
                    "radii {bottom} and {top} pass the feature checks but fail in the kernel: {built:?}"
                );
            }
        }
    }

    #[test]
    fn radii_closer_than_the_tolerance_make_a_cylinder() {
        let primitive = solid(&Plane::XY, 5.0, 5.0000005, 10.0).expect("the cylinder builds");

        let side = primitive
            .faces
            .iter()
            .position(|face| *face == PrimitiveFace::Side)
            .expect("the solid has a side");
        let index = u32::try_from(side + 1).expect("a primitive has few faces");
        let geometry = primitive
            .shape
            .element(SubShapeKind::Face, index)
            .expect("the side has geometry");
        assert_eq!(geometry.kind, GeometryKind::Cylinder);
        assert_eq!(primitive.faces.len(), 3);
    }

    #[test]
    fn python_writes_the_three_sizes() {
        let params: ConeParams = serde_json::from_value(json!({"body": "b-1", "bottom_radius": "10", "top_radius": "5", "height": "10", "op": "new"})).expect("valid params");

        let python = Cone::python(&params, &ScriptNames::default()).expect("a dedicated form");

        assert_eq!(python.render(0), "cone(10, 5, 10)");
    }
}
