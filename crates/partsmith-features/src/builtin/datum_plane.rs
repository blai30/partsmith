//! The `datum_plane` feature: a construction plane offset from an origin plane, a datum plane or
//! a flat face (docs/spec/features/datum_plane.md).

use partsmith_model::{Category, Expr, FeatureManifest, FieldHint, FieldUnit, SelectionKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    BuildContext, DatumResult, Dependencies, Feature, FeatureError, FeatureOutput, OutputData,
    PlacementExt, PlaneRef, PythonCall, PythonNames, PythonValue,
};

/// The stored offset when none is given: a datum on the base plane itself.
const DEFAULT_OFFSET: &str = "0";

/// A construction plane offset from another plane.
pub struct DatumPlane;

/// Params of a datum plane feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DatumPlaneParams {
    /// The plane to offset from.
    pub base: PlaneRef,
    /// Distance along the base normal. Length, any sign. Default "0", omitted when "0".
    #[serde(default = "default_offset", skip_serializing_if = "is_default_offset")]
    pub offset: Expr,
    /// Reverse the normal (the x-axis stays). Default false, omitted when false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flip: bool,
}

/// The default `offset`, "0".
fn default_offset() -> Expr {
    Expr::new(DEFAULT_OFFSET)
}

/// True for the default `offset`; for `skip_serializing_if`.
fn is_default_offset(offset: &Expr) -> bool {
    offset.as_str() == DEFAULT_OFFSET
}

impl Feature for DatumPlane {
    const TYPE: &'static str = "datum_plane";
    type Params = DatumPlaneParams;

    fn manifest() -> FeatureManifest {
        FeatureManifest {
            label: "Datum plane".to_string(),
            category: Category::Construct,
            icon: "datum-plane".to_string(),
            version: 1,
            fields: vec![
                FieldHint {
                    selection: Some(SelectionKind::Plane),
                    ..hint("base", "Base plane", true)
                },
                FieldHint {
                    unit: Some(FieldUnit::Length),
                    ..hint("offset", "Offset", true)
                },
                hint("flip", "Flip", false),
            ],
        }
    }

    /// The base plane's datum feature or face body, and the offset length (spec section 3.1).
    fn dependencies(params: &DatumPlaneParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_plane("base", &params.base);
        dependencies.add_expression("offset", &params.offset, FieldUnit::Length);
        dependencies
    }

    /// The base placement moved by the offset along its normal, then flipped when asked (spec
    /// section 3.2).
    fn build(
        params: &DatumPlaneParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let base = context.plane("base", &params.base)?;
        let offset = context.eval_length("offset", &params.offset)?;
        let moved = base.offset(offset);
        let placement = if params.flip { moved.flipped() } else { moved };
        Ok(FeatureOutput {
            data: OutputData::Datum(DatumResult::Plane(placement)),
            warnings: Vec::new(),
        })
    }

    /// `datum_plane(base, offset, flip=True)`, with `offset` only when it is set and `flip` only
    /// when true (spec section 2, Python).
    fn python(params: &DatumPlaneParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let mut call = PythonCall::new("datum_plane");
        call.positional.push(PythonValue::plane(&params.base, names)?);
        if !is_default_offset(&params.offset) {
            call.positional
                .push(PythonValue::expression(&params.offset, names));
        }
        if params.flip {
            call.keywords
                .push(("flip".to_string(), PythonValue::Bool(true)));
        }
        Some(call)
    }
}

/// A manifest hint without unit, selection or options.
fn hint(path: &str, label: &str, essential: bool) -> FieldHint {
    FieldHint {
        path: path.to_string(),
        label: label.to_string(),
        essential,
        unit: None,
        selection: None,
        options: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use partsmith_model::{EmptyContext, Id, ParameterValues, Scope, Units};
    use partsmith_sketch::{OriginPlane, Placement, RegionRef};
    use serde_json::json;

    use super::*;
    use crate::test_support::Fixture;
    use crate::{BodyOp, Registry, Values};

    /// Script names with `b-1` as `block`, `f-dp0` as `plane_0` and the parameter `wall`.
    struct Names;

    impl PythonNames for Names {
        fn units(&self) -> Units {
            Units::Mm
        }

        fn body(&self, body: &Id) -> Option<String> {
            (body.as_str() == "b-1").then(|| "block".to_string())
        }

        fn feature(&self, feature: &Id) -> Option<String> {
            (feature.as_str() == "f-dp0").then(|| "plane_0".to_string())
        }

        fn entity(&self, _sketch: &Id, _entity: &Id) -> Option<String> {
            None
        }

        fn parameter(&self, name: &str) -> Option<String> {
            (name == "wall").then(|| "wall".to_string())
        }

        fn regions(&self, _sketch: &Id) -> Option<Vec<RegionRef>> {
            None
        }

        fn default_op(&self, _body: &Id) -> BodyOp {
            BodyOp::New
        }
    }

    fn params(value: serde_json::Value) -> DatumPlaneParams {
        serde_json::from_value(value).expect("the params are valid")
    }

    fn python(value: serde_json::Value) -> Option<String> {
        DatumPlane::python(&params(value), &Names).map(|call| call.render(0))
    }

    #[test]
    fn registers_through_the_public_registry() {
        let mut registry = Registry::new();

        assert_eq!(registry.register::<DatumPlane>(), Ok(()));
    }

    #[test]
    fn offset_defaults_to_zero_and_is_omitted() {
        let parsed = params(json!({"base": {"kind": "origin", "plane": "XY"}}));

        assert_eq!(parsed.offset.as_str(), "0");
        assert!(!parsed.flip);
        assert_eq!(
            serde_json::to_value(&parsed).expect("serializes"),
            json!({"base": {"kind": "origin", "plane": "XY"}})
        );
    }

    #[test]
    fn dependencies_declare_the_datum_base_and_the_offset() {
        let dependencies = DatumPlane::dependencies(&params(json!({
            "base": {"kind": "datum", "feature": "f-dp0"},
            "offset": "5"
        })));

        assert_eq!(dependencies.features.len(), 1);
        assert_eq!(dependencies.features[0].id.as_str(), "f-dp0");
        assert_eq!(dependencies.expressions.len(), 1);
        assert_eq!(dependencies.expressions[0].field, "offset");
        assert_eq!(dependencies.expressions[0].kind, FieldUnit::Length);
    }

    #[test]
    fn dependencies_read_the_body_of_a_face_base() {
        let dependencies = DatumPlane::dependencies(&params(json!({
            "base": {"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1"}}
        })));

        assert_eq!(dependencies.bodies_read.len(), 1);
        assert_eq!(dependencies.bodies_read[0].id.as_str(), "b-1");
        assert!(dependencies.bodies_written.is_empty());
    }

    #[test]
    fn python_writes_a_set_offset_as_the_second_argument() {
        assert_eq!(
            python(json!({"base": {"kind": "origin", "plane": "XY"}, "offset": "10 mm"})),
            Some("datum_plane(Plane.XY, 10*mm)".to_string())
        );
    }

    #[test]
    fn python_omits_the_default_offset() {
        assert_eq!(
            python(json!({"base": {"kind": "datum", "feature": "f-dp0"}})),
            Some("datum_plane(plane_0)".to_string())
        );
    }

    #[test]
    fn python_writes_a_face_base_with_its_part_and_flip() {
        assert_eq!(
            python(json!({
                "base": {"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=3,4,10|a=100"}},
                "offset": "5",
                "flip": true
            })),
            Some(
                "datum_plane(Ref(\"f-box1/top\", \"plane|n=0,0,1|c=3,4,10|a=100\", part=block), 5, flip=True)"
                    .to_string()
            )
        );
    }

    #[test]
    fn python_needs_the_base_variable() {
        assert_eq!(
            python(json!({"base": {"kind": "datum", "feature": "f-dp9"}})),
            None
        );
    }

    #[test]
    fn python_writes_a_parameter_offset_as_its_variable() {
        assert_eq!(
            python(json!({"base": {"kind": "origin", "plane": "YZ"}, "offset": "wall"})),
            Some("datum_plane(Plane.YZ, wall)".to_string())
        );
    }

    /// The datum placement `params` build in a millimeter document without bodies.
    fn built(params: &DatumPlaneParams) -> Result<FeatureOutput, FeatureError> {
        let mut fixture = Fixture::new("f-dp1");
        fixture.manifest = DatumPlane::manifest();
        let parameters = ParameterValues::default();
        let scope = Scope {
            units: Units::Mm,
            parameters: &parameters,
            context: &EmptyContext,
        };
        fixture.values = Values::evaluate(
            &DatumPlane::dependencies(params).expressions,
            &scope,
            &fixture.manifest,
        );
        DatumPlane::build(params, &fixture.context())
    }

    fn placement(output: &FeatureOutput) -> Placement {
        match &output.data {
            OutputData::Datum(DatumResult::Plane(placement)) => *placement,
            other => panic!("expected a datum plane, got {other:?}"),
        }
    }

    #[test]
    fn build_offsets_along_the_base_normal() {
        let output = built(&params(json!({
            "base": {"kind": "origin", "plane": "XZ"},
            "offset": "10"
        })))
        .expect("the datum builds");

        assert_eq!(placement(&output).origin, [0.0, -10.0, 0.0]);
        assert_eq!(placement(&output).normal, OriginPlane::Xz.placement().normal);
        assert!(output.warnings.is_empty());
    }

    #[test]
    fn build_flips_the_normal_after_the_offset() {
        let output = built(&params(json!({
            "base": {"kind": "origin", "plane": "XY"},
            "offset": "10",
            "flip": true
        })))
        .expect("the datum builds");

        assert_eq!(placement(&output).origin, [0.0, 0.0, 10.0]);
        assert_eq!(placement(&output).x_axis, [1.0, 0.0, 0.0]);
        assert_eq!(placement(&output).normal, [0.0, 0.0, -1.0]);
    }

    #[test]
    fn build_reports_an_offset_that_is_not_a_length() {
        let error = built(&params(json!({
            "base": {"kind": "origin", "plane": "XY"},
            "offset": "10 deg"
        })))
        .expect_err("an angle is not an offset");

        assert_eq!(error.code(), "expression");
    }
}
