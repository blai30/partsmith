//! The `torus` feature: a ring torus centered on the placement origin, its axis along the plane
//! normal (docs/spec/features/torus.md).

use partsmith_model::{Expr, Id};
use partsmith_occt_sys::make_torus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::primitives::{self, SizeField};
use crate::{
    BodyOp, Bound, BuildContext, Dependencies, Feature, FeatureError, FeatureManifest,
    FeatureOutput, PlacementExt, PlaneRef, PrimitivePlacement, PythonCall, PythonNames,
    default_position, is_default_position,
};

/// The ring radius field.
const MAJOR_RADIUS: SizeField = SizeField {
    path: "major_radius",
    label: "Ring radius",
};

/// The tube radius field.
const MINOR_RADIUS: SizeField = SizeField {
    path: "minor_radius",
    label: "Tube radius",
};

/// A ring torus.
#[derive(Clone, Copy, Debug, Default)]
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
    /// The shared placement params.
    fn placement(&self) -> PrimitivePlacement<'_> {
        PrimitivePlacement {
            plane: &self.plane,
            position: &self.position,
        }
    }
}

impl Feature for Torus {
    const TYPE: &'static str = "torus";
    type Params = TorusParams;

    fn manifest() -> FeatureManifest {
        primitives::manifest("Torus", "torus", &[MAJOR_RADIUS, MINOR_RADIUS])
    }

    fn dependencies(params: &TorusParams) -> Dependencies {
        primitives::dependencies(
            &params.body,
            &[
                (MAJOR_RADIUS, &params.major_radius),
                (MINOR_RADIUS, &params.minor_radius),
            ],
            params.placement(),
        )
    }

    fn build(
        params: &TorusParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let major_radius =
            primitives::positive_length(context, MAJOR_RADIUS.path, &params.major_radius)?;
        let minor_radius =
            primitives::positive_length(context, MINOR_RADIUS.path, &params.minor_radius)?;
        if minor_radius >= major_radius {
            return Err(primitives::out_of_range(
                context,
                MINOR_RADIUS.path,
                minor_radius,
                Bound::LessThan(major_radius),
                Some("the ring radius"),
            ));
        }
        let frame = params.placement().frame(context)?;
        let primitive = make_torus(&frame.to_kernel(), major_radius, minor_radius)
            .map_err(primitives::kernel)?;
        primitives::combine(context, &params.body, params.op, primitive)
    }

    fn python(params: &TorusParams, names: &dyn PythonNames) -> Option<PythonCall> {
        primitives::python(
            "torus",
            &[&params.major_radius, &params.minor_radius],
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

    /// The spec's `TOR` record with this tube radius.
    fn torus(minor_radius: &str) -> TorusParams {
        params::<Torus>(json!({
            "id": "f-tor1", "type": "torus", "body": "b-1", "major_radius": "20",
            "minor_radius": minor_radius, "op": "new"
        }))
    }

    #[test]
    fn registers_with_its_manifest() {
        let mut registry = Registry::new();

        registry
            .register::<Torus>()
            .expect("the torus passes the registration checks");

        assert_eq!(registry.types().collect::<Vec<_>>(), ["torus"]);
    }

    #[test]
    fn manifest_labels_ring_and_tube() {
        let manifest = Torus::manifest();

        assert_eq!(manifest.fields[0].label, "Ring radius");
        assert_eq!(manifest.fields[1].label, "Tube radius");
    }

    #[test]
    fn builds_one_surface() {
        let output = build::<Torus>(&torus("5"), Vec::new()).expect("the torus builds");

        let OutputData::Bodies(updates) = &output.data else {
            panic!("a torus outputs body updates");
        };
        assert_eq!(face_names(&updates[0].naming), ["f-prim1/surface"]);
    }

    #[test]
    fn a_tube_as_large_as_the_ring_is_out_of_range() {
        let error = build::<Torus>(&torus("20"), Vec::new()).expect_err("a horn torus fails");

        assert_eq!(error.field().expect("names its field").path, "minor_radius");
        assert_eq!(
            error.to_string(),
            "Tube radius must be less than the ring radius (20 mm), but it is 20 mm. Use a value less than 20 mm."
        );
    }

    #[test]
    fn a_zero_tube_is_greater_than_zero_out_of_range() {
        let error = build::<Torus>(&torus("0"), Vec::new()).expect_err("no tube fails");

        assert!(
            matches!(
                error,
                FeatureError::OutOfRange {
                    bound: Bound::GreaterThan(0.0),
                    ..
                }
            ),
            "{error:?}"
        );
    }

    #[test]
    fn a_zero_ring_is_checked_first() {
        let mut params = torus("0");
        params.major_radius = Expr::new("0");

        let error = build::<Torus>(&params, Vec::new()).expect_err("no ring fails");

        assert_eq!(error.field().expect("names its field").path, "major_radius");
    }

    #[test]
    fn python_writes_ring_and_tube() {
        assert_eq!(
            python::<Torus>(&torus("5"), BodyOp::New).as_deref(),
            Some("torus(20, 5)")
        );
    }

    #[test]
    fn python_writes_the_op_when_it_is_not_the_default() {
        assert_eq!(
            python::<Torus>(&torus("5"), BodyOp::Join).as_deref(),
            Some(r#"torus(20, 5, op="new")"#)
        );
    }
}
