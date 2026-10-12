//! The `datum_plane` feature: a construction plane offset from an origin plane, a datum plane or a
//! flat face (features/datum_plane.md).

use partsmith_model::{Category, Expr, FeatureManifest, FieldUnit, SelectionKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{call, essential, flag, hint, is_false, manifest, selection_hint, unit_hint};
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::output::{DatumResult, FeatureOutput, OutputData};
use crate::params::{PlacementExt, PlaneRef};
use crate::python::{PythonCall, PythonNames, PythonValue};

/// A construction plane offset from another plane.
#[derive(Clone, Copy, Debug)]
pub struct DatumPlane;

/// Params of a datum plane feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DatumPlaneParams {
    /// The plane to offset from.
    pub base: PlaneRef,
    /// Distance along the base normal. Length, any sign. Default "0", omitted when "0".
    #[serde(default = "zero", skip_serializing_if = "is_zero")]
    pub offset: Expr,
    /// Reverse the normal (the x-axis stays). Default false, omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip: bool,
}

/// The default offset, "0".
fn zero() -> Expr {
    Expr::new("0")
}

/// True for the default offset; for `skip_serializing_if`.
fn is_zero(offset: &Expr) -> bool {
    *offset == zero()
}

impl Feature for DatumPlane {
    const TYPE: &'static str = "datum_plane";
    type Params = DatumPlaneParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Datum plane",
            Category::Construct,
            "datum-plane",
            vec![
                essential(selection_hint("base", "Base plane", SelectionKind::Plane)),
                essential(unit_hint("offset", "Offset", FieldUnit::Length)),
                hint("flip", "Flip"),
            ],
        )
    }

    fn dependencies(params: &DatumPlaneParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_plane("base", &params.base);
        dependencies.add_expression("offset", &params.offset, FieldUnit::Length);
        dependencies
    }

    fn build(
        params: &DatumPlaneParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let base = context.plane("base", &params.base)?;
        let offset = context.eval_length("offset", &params.offset)?;
        let mut placement = base.offset(offset);
        if params.flip {
            placement = placement.flipped();
        }
        Ok(FeatureOutput {
            data: OutputData::Datum(DatumResult::Plane(placement)),
            warnings: Vec::new(),
        })
    }

    fn python(params: &DatumPlaneParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let mut positional = vec![PythonValue::plane(&params.base, names)?];
        if !is_zero(&params.offset) {
            positional.push(PythonValue::expression(&params.offset, names));
        }
        let keywords = if params.flip {
            vec![flag("flip")]
        } else {
            Vec::new()
        };
        Some(call("datum_plane", positional, keywords))
    }
}

#[cfg(test)]
mod tests {
    use partsmith_model::FeatureRecord;
    use serde_json::json;

    use super::*;
    use crate::registry::Registry;
    use crate::test_support::{ScriptNames, id};

    /// The Python of a datum plane record, with the record keywords the registry appends.
    fn python(record: serde_json::Value) -> String {
        let record: FeatureRecord = serde_json::from_value(record).expect("a valid record");
        Registry::with_builtins()
            .python(&record, &ScriptNames::default())
            .render(0)
    }

    #[test]
    fn dependencies_read_the_face_body_and_the_offset() {
        let params: DatumPlaneParams = serde_json::from_value(json!({
            "base": {"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1"}},
            "offset": "5"
        }))
        .expect("valid params");

        let dependencies = DatumPlane::dependencies(&params);

        assert!(dependencies.features.is_empty());
        assert_eq!(dependencies.bodies_read[0].id, id("b-1"));
        assert_eq!(dependencies.bodies_read[0].field, "base.body");
        assert!(dependencies.bodies_written.is_empty());
        assert_eq!(dependencies.expressions[0].field, "offset");
    }

    #[test]
    fn python_writes_the_spec_examples() {
        let on_origin_plane = python(
            json!({"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "10 mm"}),
        );
        let on_face = python(json!({
            "id": "f-dp2", "type": "datum_plane",
            "base": {"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=3,4,10|a=100"}},
            "offset": "5", "flip": true
        }));

        assert_eq!(
            on_origin_plane,
            r#"datum_plane(Plane.XY, 10*mm, id="f-dp1")"#
        );
        // Longer than 88 characters on one line, so one argument per line.
        assert_eq!(
            on_face,
            [
                "datum_plane(",
                r#"    Ref("f-box1/top", "plane|n=0,0,1|c=3,4,10|a=100", part=block),"#,
                "    5,",
                "    flip=True,",
                r#"    id="f-dp2","#,
                ")",
            ]
            .join("\n")
        );
    }

    #[test]
    fn the_default_offset_is_omitted_in_python() {
        let names = ScriptNames::default();
        let plain: DatumPlaneParams =
            serde_json::from_value(json!({"base": {"kind": "origin", "plane": "XY"}}))
                .expect("valid params");
        let offset: DatumPlaneParams = serde_json::from_value(
            json!({"base": {"kind": "origin", "plane": "XY"}, "offset": "10 mm", "flip": true}),
        )
        .expect("valid params");

        assert_eq!(
            DatumPlane::python(&plain, &names)
                .expect("a dedicated form")
                .render(0),
            "datum_plane(Plane.XY)"
        );
        assert_eq!(
            DatumPlane::python(&offset, &names)
                .expect("a dedicated form")
                .render(0),
            "datum_plane(Plane.XY, 10*mm, flip=True)"
        );
    }
}
