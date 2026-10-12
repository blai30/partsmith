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
    use partsmith_sketch::Placement;
    use serde_json::json;

    use super::*;
    use crate::registry::Registry;
    use crate::test_support::{DocumentRun, ScriptNames, run};

    fn placement(run: &DocumentRun, feature: &str) -> Placement {
        match &run.output(feature).data {
            OutputData::Datum(DatumResult::Plane(placement)) => *placement,
            other => panic!("{feature} output {other:?}"),
        }
    }

    fn assert_close(actual: [f64; 3], expected: [f64; 3]) {
        for (component, wanted) in actual.iter().zip(expected) {
            assert!(
                (component - wanted).abs() <= 1e-9,
                "expected {expected:?}, got {actual:?}"
            );
        }
    }

    fn datum(base: serde_json::Value, extra: serde_json::Value) -> serde_json::Value {
        let mut record = json!({"id": "f-dp1", "type": "datum_plane", "base": base});
        if let (Some(record), Some(extra)) = (record.as_object_mut(), extra.as_object()) {
            record.extend(extra.clone());
        }
        record
    }

    #[test]
    fn an_origin_plane_offset_moves_along_its_normal() {
        let run = run(
            &[],
            &[datum(
                json!({"kind": "origin", "plane": "XZ"}),
                json!({"offset": "10"}),
            )],
        );

        let plane = placement(&run, "f-dp1");
        assert_close(plane.origin, [0.0, -10.0, 0.0]);
        assert_close(plane.normal, [0.0, -1.0, 0.0]);
        assert_close(plane.y_axis(), [0.0, 0.0, 1.0]);
    }

    #[test]
    fn flip_reverses_the_normal_and_keeps_the_x_axis() {
        let run = run(
            &[],
            &[datum(
                json!({"kind": "origin", "plane": "XY"}),
                json!({"offset": "10", "flip": true}),
            )],
        );

        let plane = placement(&run, "f-dp1");
        assert_close(plane.origin, [0.0, 0.0, 10.0]);
        assert_close(plane.x_axis, [1.0, 0.0, 0.0]);
        assert_close(plane.normal, [0.0, 0.0, -1.0]);
    }

    #[test]
    fn a_side_face_takes_its_x_axis_from_world_y() {
        let run = run(
            &["b-1"],
            &[
                json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "position": ["3", "4", "0"], "op": "new"}),
                datum(
                    json!({"kind": "face", "body": "b-1", "face": {"q": "f-box1/right", "fp": "plane|n=1,0,0|c=8,4,5|a=100"}}),
                    json!({}),
                ),
            ],
        );

        let plane = placement(&run, "f-dp1");
        assert_close(plane.origin, [8.0, 0.0, 0.0]);
        assert_close(plane.normal, [1.0, 0.0, 0.0]);
        assert_close(plane.x_axis, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn chained_datums_add_their_offsets() {
        let run = run(
            &[],
            &[
                datum(
                    json!({"kind": "origin", "plane": "XY"}),
                    json!({"offset": "10"}),
                ),
                json!({"id": "f-dp2", "type": "datum_plane", "base": {"kind": "datum", "feature": "f-dp1"}, "offset": "5"}),
            ],
        );

        assert_close(placement(&run, "f-dp2").origin, [0.0, 0.0, 15.0]);
    }

    #[test]
    fn a_curved_face_is_not_a_base_plane() {
        let run = run(
            &["b-1"],
            &[
                json!({"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "5", "height": "10", "op": "new"}),
                datum(
                    json!({"kind": "face", "body": "b-1", "face": {"q": "f-cyl1/side", "fp": "cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5"}}),
                    json!({}),
                ),
            ],
        );

        assert_eq!(
            run.messages("f-dp1"),
            [
                "The face selected for Base plane is not flat. Pick a flat face, a datum plane or an origin plane."
            ]
        );
    }

    /// The Python of a datum plane record, with the record keywords the registry appends.
    fn python(record: serde_json::Value) -> String {
        let record: FeatureRecord = serde_json::from_value(record).expect("a valid record");
        Registry::with_builtins()
            .python(&record, &ScriptNames::default())
            .render(0)
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
