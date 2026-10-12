//! The `cone` feature: a right circular cone or frustum standing on a plane, its base centered on
//! the placement origin and its axis along the plane normal (docs/spec/features/cone.md).

use partsmith_model::{Expr, Id};
use partsmith_occt_sys::{make_cone, make_cylinder};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::primitives::{self, SizeField};
use crate::{
    BodyOp, BuildContext, Dependencies, Feature, FeatureError, FeatureManifest, FeatureOutput,
    PlacementExt, PlaneRef, PrimitivePlacement, PythonCall, PythonNames, default_position,
    is_default_position,
};

/// The bottom radius field.
const BOTTOM_RADIUS: SizeField = SizeField {
    path: "bottom_radius",
    label: "Bottom radius",
};

/// The top radius field.
const TOP_RADIUS: SizeField = SizeField {
    path: "top_radius",
    label: "Top radius",
};

/// The height field.
const HEIGHT: SizeField = SizeField {
    path: "height",
    label: "Height",
};

/// Largest difference in millimeters between radii that build a cylinder rather than a cone
/// (cone.md section 3.2), because the kernel's cone needs different radii.
const EQUAL_RADII: f64 = 1e-9;

/// A right circular cone or frustum.
#[derive(Clone, Copy, Debug, Default)]
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
    /// The shared placement params.
    fn placement(&self) -> PrimitivePlacement<'_> {
        PrimitivePlacement {
            plane: &self.plane,
            position: &self.position,
        }
    }
}

impl Feature for Cone {
    const TYPE: &'static str = "cone";
    type Params = ConeParams;

    fn manifest() -> FeatureManifest {
        primitives::manifest("Cone", "cone", &[BOTTOM_RADIUS, TOP_RADIUS, HEIGHT])
    }

    fn dependencies(params: &ConeParams) -> Dependencies {
        primitives::dependencies(
            &params.body,
            &[
                (BOTTOM_RADIUS, &params.bottom_radius),
                (TOP_RADIUS, &params.top_radius),
                (HEIGHT, &params.height),
            ],
            params.placement(),
        )
    }

    fn build(
        params: &ConeParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let bottom_radius =
            primitives::non_negative_length(context, BOTTOM_RADIUS.path, &params.bottom_radius)?;
        let top_radius =
            primitives::non_negative_length(context, TOP_RADIUS.path, &params.top_radius)?;
        if bottom_radius == 0.0 && top_radius == 0.0 {
            return Err(FeatureError::ConeRadiiZero);
        }
        let height = primitives::positive_length(context, HEIGHT.path, &params.height)?;
        let placement = params.placement().frame(context)?.to_kernel();
        let primitive = if (bottom_radius - top_radius).abs() <= EQUAL_RADII {
            make_cylinder(&placement, bottom_radius, height)
        } else {
            make_cone(&placement, bottom_radius, top_radius, height)
        }
        .map_err(primitives::kernel)?;
        primitives::combine(context, &params.body, params.op, primitive)
    }

    fn python(params: &ConeParams, names: &dyn PythonNames) -> Option<PythonCall> {
        primitives::python(
            "cone",
            &[&params.bottom_radius, &params.top_radius, &params.height],
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

    /// The spec's `CONE(r1, r2, h)` record.
    fn cone(bottom_radius: &str, top_radius: &str, height: &str) -> ConeParams {
        params::<Cone>(json!({
            "id": "f-cone1", "type": "cone", "body": "b-1", "bottom_radius": bottom_radius,
            "top_radius": top_radius, "height": height, "op": "new"
        }))
    }

    fn update(output: &FeatureOutput) -> &BodyUpdate {
        let OutputData::Bodies(updates) = &output.data else {
            panic!("a cone outputs body updates");
        };
        &updates[0]
    }

    #[test]
    fn registers_with_its_manifest() {
        let mut registry = Registry::new();

        registry
            .register::<Cone>()
            .expect("the cone passes the registration checks");

        assert_eq!(registry.types().collect::<Vec<_>>(), ["cone"]);
    }

    #[test]
    fn a_frustum_has_side_top_and_bottom() {
        let output = build::<Cone>(&cone("10", "5", "10"), Vec::new()).expect("the frustum builds");

        assert_eq!(
            face_names(&update(&output).naming),
            ["f-prim1/bottom", "f-prim1/side", "f-prim1/top"]
        );
    }

    #[test]
    fn a_pointed_cone_has_no_top() {
        let output = build::<Cone>(&cone("10", "0", "10"), Vec::new()).expect("the cone builds");

        assert_eq!(
            face_names(&update(&output).naming),
            ["f-prim1/bottom", "f-prim1/side"]
        );
    }

    #[test]
    fn equal_radii_build_a_cylindrical_side() {
        let output = build::<Cone>(&cone("5", "5", "10"), Vec::new()).expect("the cone builds");

        let update = update(&output);
        let side = ElementName::parse("f-prim1/side").expect("valid name");
        let index = update.naming.index(&side).expect("the side is named");
        let geometry = update
            .shape
            .as_ref()
            .expect("the body holds the cone")
            .element(SubShapeKind::Face, index)
            .expect("the side has geometry");
        assert_eq!(geometry.kind, GeometryKind::Cylinder);
    }

    #[test]
    fn a_negative_radius_is_below_zero() {
        let error = build::<Cone>(&cone("10", "-1", "10"), Vec::new())
            .expect_err("a negative radius fails");

        assert_eq!(error.field().expect("names its field").path, "top_radius");
        assert_eq!(
            error.to_string(),
            "Top radius must be at least 0 mm, but it is -1 mm. Use 0 mm or more."
        );
    }

    #[test]
    fn both_radii_zero_is_its_own_error() {
        let error =
            build::<Cone>(&cone("0", "0", "10"), Vec::new()).expect_err("no radius fails");

        assert_eq!(error, FeatureError::ConeRadiiZero);
    }

    #[test]
    fn radii_are_checked_before_the_height() {
        let error =
            build::<Cone>(&cone("0", "0", "0"), Vec::new()).expect_err("nothing is sized");

        assert_eq!(error.code(), "cone_radii_zero");
    }

    #[test]
    fn a_zero_height_is_out_of_range() {
        let error =
            build::<Cone>(&cone("10", "5", "0"), Vec::new()).expect_err("a flat cone fails");

        assert_eq!(error.code(), "out_of_range");
        assert_eq!(error.field().expect("names its field").path, "height");
    }

    #[test]
    fn python_writes_both_radii_and_the_height() {
        assert_eq!(
            python::<Cone>(&cone("10", "5", "10"), BodyOp::New).as_deref(),
            Some("cone(10, 5, 10)")
        );
    }
}
