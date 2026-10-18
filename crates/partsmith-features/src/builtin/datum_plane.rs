//! The `datum_plane` feature: a construction plane offset from an origin plane, a datum plane or a
//! flat face (features/datum_plane.md).

use partsmith_model::{Category, Expr, FeatureManifest, FieldUnit, SelectionKind};
use partsmith_naming::Motion;
use partsmith_sketch::{OriginPlane, Placement};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    call, essential, flag, hint, is_false, manifest, selection_hint, signed_length, unit_hint,
};
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
        let offset = signed_length(context, "offset", &params.offset)?;
        let motion = base_motion(context, &params.base, &base.value)?;
        let mut placement = base.value.offset(offset);
        if params.flip {
            placement = placement.flipped();
        }
        Ok(FeatureOutput::new(
            OutputData::Datum(DatumResult::Plane { placement, motion }),
            base.warnings,
        ))
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

/// The motion a datum passes on (datum_plane.md section 3.2 step 4): the identity for an origin base, the base
/// datum's own motion, and for a face base the rigid motion that takes `P0` to `placement`, the base placement of
/// step 1, with `P0` the face placement rule on the face's plane mapped back by `m`, the motion of the stored
/// query's face just before the datum. That is the part of the face's motion the datum's frame follows: none for a
/// slide along the face, a turn for a face turned over. `placement` lies in the face's plane with its normal, so
/// mapping it back gives that plane as the face's centroid and normal would.
fn base_motion(
    context: &BuildContext<'_>,
    base: &PlaneRef,
    placement: &Placement,
) -> Result<Motion, FeatureError> {
    let (body, face) = match base {
        PlaneRef::Origin { .. } => return Ok(Motion::IDENTITY),
        PlaneRef::Datum { feature } => {
            return match context.datum("base.feature", feature)? {
                DatumResult::Plane { motion, .. } => Ok(*motion),
            };
        }
        PlaneRef::Face { body, face } => (body, face),
    };
    let motion = context.body(body)?.motion(&face.query);
    if motion == Motion::IDENTITY {
        return Ok(Motion::IDENTITY);
    }
    let back = motion.inverse();
    let before = Placement::on_plane(
        back.apply_point(placement.origin),
        back.apply_direction(placement.normal),
        &OriginPlane::Xy.placement(),
    )
    .map_err(|error| FeatureError::internal(format!("placing a datum plane failed: {error}")))?;
    if before == *placement {
        return Ok(Motion::IDENTITY);
    }
    Ok(frame(&before).inverse().then(&frame(placement)))
}

/// The rigid motion that takes the world frame to `placement`'s: +X to its x axis, +Y to its y axis, +Z to its
/// normal and the world origin to its origin.
fn frame(placement: &Placement) -> Motion {
    let y_axis = placement.y_axis();
    Motion::new(
        [0, 1, 2].map(|row| [placement.x_axis[row], y_axis[row], placement.normal[row]]),
        placement.origin,
    )
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
    fn a_frame_takes_the_world_axes_to_the_placement_axes() {
        // Origin (0, -5, 0), x axis +X, y axis +Z, normal -Y.
        let placement = OriginPlane::Xz.placement().offset(5.0);

        let motion = frame(&placement);

        assert_eq!(motion.apply_point([0.0, 0.0, 0.0]), placement.origin);
        assert_eq!(motion.apply_point([1.0, 2.0, 3.0]), [1.0, -8.0, 2.0]);
        assert!(motion.is_proper());
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
