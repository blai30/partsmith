//! The edits of the frame fixes (engine.md section 3.22, ADR 0023): `Attach` puts a sketch on a
//! face where it stands now, and `TurnDrawingBack` mirrors the drawing of a sketch whose face
//! turned over and re-anchors the regions and points that features use of it. Both store where
//! the sketch stands in the coordinates that keep its motion (`kept_face_plane`): body
//! coordinates, unless that would give the faces built from the sketch another motion.

use partsmith_features::SketchResult;
use partsmith_model::{Edit, FeatureRecord, Id};
use partsmith_naming::{Motion, Reference};
use partsmith_sketch::{Placement, PointRef, RegionRef, SketchParams, SketchPlane};
use serde_json::{Map, Value};

use super::edits::{update_feature, updated};
use super::renames::PointRenames;
use crate::error::EngineError;
use crate::frames::{face_placement, keeping_frame, kept_face_plane};
use crate::params::{set, stored_references, visit_sketch_uses};
use crate::run::FeatureView;
use crate::sketch::sketch_params;
use crate::state::built_sketch;

impl FeatureView<'_> {
    /// The edits of `Attach` (section 3.22): one `update_feature` of `sketch` with its plane on
    /// `face` of `body`, whose fallback is where the sketch stands now projected onto the face
    /// (the face placement rule with that placement as the hint), in the coordinates that keep
    /// the sketch's motion.
    pub(super) fn attached(
        &self,
        sketch: &Id,
        body: &Id,
        face: &Reference,
    ) -> Result<Vec<Edit>, EngineError> {
        let view = self.sketch_view(sketch)?;
        let standing = standing(&view)?;
        let state = view.state_before(body);
        let placement = face_placement(&state, face, &standing.placement)
            .ok_or_else(|| EngineError::internal("an attach names no flat face"))?;
        let plane = kept_plane_params(
            body,
            face,
            &placement,
            &state.motion(&face.query),
            &standing.motion,
        )?;
        Ok(vec![updated(view.record, |params| {
            set(params, "/plane", plane)
        })?])
    }

    /// True when some coordinates keep the sketch's motion on `face` of `body`, so that `Attach`
    /// is offered (section 3.22, `keeping_frame`). A sketch that did not build offers no frame
    /// fix.
    pub(super) fn attach_keeps_motion(&self, sketch: &Id, body: &Id, face: &Reference) -> bool {
        let Ok(view) = self.sketch_view(sketch) else {
            return false;
        };
        standing(&view).is_ok_and(|standing| {
            let face_motion = view.state_before(body).motion(&face.query);
            keeping_frame(&face_motion, &standing.motion).is_some()
        })
    }

    /// The edits of `TurnDrawingBack` (section 3.22, features/sketch.md section 3.5): one
    /// `update_feature` of `sketch` with its entities and constraints mirrored
    /// (`SketchParams::mirrored`) and where it stands now as its fallback, in the coordinates that
    /// keep its motion, then one of each feature whose regions or points of the sketch read
    /// differently on the mirrored drawing (`mirrored_region`, `mirrored_point`), or that stores
    /// a reference to a face made from a point that reads differently (`PointRenames`).
    pub(super) fn turned_back(&self, sketch: &Id) -> Result<Vec<Edit>, EngineError> {
        let view = self.sketch_view(sketch)?;
        let drawing = sketch_params(self.run.registry, self.run.document, view.record)?;
        let SketchPlane::Face { body, face, .. } = &drawing.plane else {
            return Err(EngineError::internal("a drawing turns back only on a face"));
        };
        let standing = standing(&view)?;
        let motion = view.state_before(body).motion(&face.query);
        let plane = kept_plane_params(body, face, &standing.placement, &motion, &standing.motion)?;
        let mirrored = drawing.mirrored();
        let entities = json(serde_json::to_value(&mirrored.entities))?;
        let constraints = json(serde_json::to_value(&mirrored.constraints))?;
        let mut edits = vec![updated(view.record, |params| {
            set(params, "/entities", entities)
                && set(params, "/constraints", constraints)
                && set(params, "/plane", plane)
        })?];
        // A reference names faces of earlier features only, so the renames of the features before
        // each one are known when it is read.
        let mut renames = PointRenames::default();
        for record in &self.run.document.features {
            if record.id != *sketch
                && let Some(edit) = turned_back_record(record, sketch, &drawing, &mut renames)?
            {
                edits.push(edit);
            }
        }
        Ok(edits)
    }

    /// The view of the sketch a frame fix names.
    fn sketch_view(&self, sketch: &Id) -> Result<FeatureView<'_>, EngineError> {
        self.run
            .feature(sketch)
            .ok_or_else(|| EngineError::internal("a frame fix names a sketch that is gone"))
    }
}

/// Where a sketch stands in the run: the placement it built with and its motion, which the faces
/// built from it inherit. A frame fix is offered only by a message of a run in which its sketch
/// built.
fn standing<'v>(sketch: &FeatureView<'v>) -> Result<&'v SketchResult, EngineError> {
    built_sketch(sketch.report)
        .ok_or_else(|| EngineError::internal("a frame fix names a sketch that did not build"))
}

/// The plane a frame fix stores, as params JSON: `placement` on `face` of `body`, whose motion is
/// `face_motion`, in the coordinates that keep the sketch's motion `kept`.
fn kept_plane_params(
    body: &Id,
    face: &Reference,
    placement: &Placement,
    face_motion: &Motion,
    kept: &Motion,
) -> Result<Value, EngineError> {
    let plane = kept_face_plane(body, face, placement, face_motion, kept)
        .ok_or_else(|| EngineError::internal("a frame fix changes its sketch's motion"))?;
    json(serde_json::to_value(plane))
}

/// One `update_feature` of `record` as it reads on the mirrored `drawing` of `sketch`: with the
/// regions and points it uses of that sketch mirrored, and the references it stores to faces that
/// earlier features made from points they now write differently renamed (`renames`). The points
/// `record` itself now writes differently go into `renames`, since the faces it makes from them
/// change names too. `None` when nothing of it reads differently.
fn turned_back_record(
    record: &FeatureRecord,
    sketch: &Id,
    drawing: &SketchParams,
    renames: &mut PointRenames,
) -> Result<Option<Edit>, EngineError> {
    let mut params = Value::Object(record.params.clone());
    for stored in stored_references(&record.params) {
        if let Some(renamed) = renames.reference(&stored.reference)?
            && !set(
                &mut params,
                &stored.pointer,
                json(serde_json::to_value(renamed))?,
            )
        {
            return Err(EngineError::internal("a stored reference has no place"));
        }
    }
    let mut swapped = Vec::new();
    visit_sketch_uses(&mut params, sketch, &mut |uses| {
        mirror_entries(uses, "regions", |entry| {
            serde_json::from_value::<RegionRef>(entry.clone())
                .ok()
                .map(|region| json(serde_json::to_value(drawing.mirrored_region(&region))))
                .transpose()
        })?;
        mirror_entries(uses, "points", |entry| {
            let Ok(point) = serde_json::from_value::<PointRef>(entry.clone()) else {
                return Ok(None);
            };
            let mirrored = drawing.mirrored_point(&point);
            if mirrored != point {
                swapped.push((point.to_string(), mirrored.to_string()));
            }
            json(serde_json::to_value(mirrored)).map(Some)
        })
    })?;
    for (from, to) in swapped {
        renames.add(&record.id, from, to);
    }
    if params.as_object() == Some(&record.params) {
        return Ok(None);
    }
    update_feature(record, params).map(Some)
}

/// Rewrites each entry of the list at `key` of `members` that `mirror` gives a mirrored value;
/// entries it gives none (those that are no reference of the kind it mirrors) stay as they are.
fn mirror_entries(
    members: &mut Map<String, Value>,
    key: &str,
    mut mirror: impl FnMut(&Value) -> Result<Option<Value>, EngineError>,
) -> Result<(), EngineError> {
    let Some(Value::Array(entries)) = members.get_mut(key) else {
        return Ok(());
    };
    for entry in entries {
        if let Some(mirrored) = mirror(entry)? {
            *entry = mirrored;
        }
    }
    Ok(())
}

/// A value serialized as the JSON a record's params store.
fn json(value: serde_json::Result<Value>) -> Result<Value, EngineError> {
    value.map_err(EngineError::unexpected)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::id;

    /// A sketch `f-sk1` on XY with a rectangle `e-1` to `e-4` from (0, 0) to (20, 10), an arc
    /// `e-5` and a point `e-6`.
    fn drawing() -> SketchParams {
        serde_json::from_value(json!({
            "plane": {"kind": "origin", "plane": "XY"},
            "entities": [
                {"id": "e-1", "type": "line", "start": [0, 0], "end": [20, 0]},
                {"id": "e-2", "type": "line", "start": [20, 0], "end": [20, 10]},
                {"id": "e-3", "type": "line", "start": [20, 10], "end": [0, 10]},
                {"id": "e-4", "type": "line", "start": [0, 10], "end": [0, 0]},
                {"id": "e-5", "type": "arc", "center": [40, 0], "radius": 5, "start_angle": 0, "end_angle": 1.5},
                {"id": "e-6", "type": "point", "position": [30, 5]}
            ],
            "constraints": []
        }))
        .expect("valid sketch params")
    }

    /// A record `f-x1` of type `kind` with these params.
    fn record(kind: &str, params: Value) -> FeatureRecord {
        record_of("f-x1", kind, params)
    }

    /// A record `feature` of type `kind` with these params.
    fn record_of(feature: &str, kind: &str, params: Value) -> FeatureRecord {
        let Value::Object(params) = params else {
            panic!("params are an object");
        };
        FeatureRecord {
            id: id(feature),
            kind: kind.to_string(),
            name: None,
            suppressed: false,
            params,
        }
    }

    /// The edit `turned_back_record` gives `record` when `f-sk1` turns back, with no renames of
    /// earlier features.
    fn mirrored_uses(record: &FeatureRecord) -> Option<Edit> {
        turned_back_record(
            record,
            &id("f-sk1"),
            &drawing(),
            &mut PointRenames::default(),
        )
        .expect("mirrors")
    }

    /// The params of the record an edit updates.
    fn updated_params(edit: Option<Edit>) -> Value {
        match edit {
            Some(Edit::UpdateFeature { feature }) => Value::Object(feature.params),
            other => panic!("one update_feature, not {other:?}"),
        }
    }

    #[test]
    fn the_regions_a_feature_uses_read_as_on_the_mirrored_drawing() {
        let region = json!({"key": "e-1,e-2,e-3,e-4", "fp": "c=10,5|a=200"});
        let extrude = record(
            "extrude",
            json!({"body": "b-1", "profile": {"sketch": "f-sk1", "regions": [region.clone()]}}),
        );
        let stored: RegionRef = serde_json::from_value(region).expect("a region reference");

        let edit = mirrored_uses(&extrude);

        let mirrored = serde_json::to_value(drawing().mirrored_region(&stored))
            .expect("region references serialize");
        assert_eq!(
            updated_params(edit),
            json!({"body": "b-1", "profile": {"sketch": "f-sk1", "regions": [mirrored]}})
        );
    }

    #[test]
    fn the_ends_of_an_arc_a_feature_uses_swap_on_the_mirrored_drawing() {
        let hole = record(
            "hole",
            json!({"sketch": "f-sk1", "points": ["e-5.start", "e-6", "e-5.center"]}),
        );

        let edit = mirrored_uses(&hole);

        assert_eq!(
            updated_params(edit),
            json!({"sketch": "f-sk1", "points": ["e-5.end", "e-6", "e-5.center"]})
        );
    }

    #[test]
    fn a_feature_whose_uses_read_the_same_on_the_mirrored_drawing_is_not_edited() {
        let hole = record(
            "hole",
            json!({"sketch": "f-sk1", "points": ["e-6", "e-1.start"]}),
        );
        let other = record(
            "extrude",
            json!({"profile": {"sketch": "f-sk2", "regions": [{"key": "e-1,e-2,e-3,e-4"}]}}),
        );

        assert_eq!(mirrored_uses(&hole), None);
        assert_eq!(mirrored_uses(&other), None);
    }

    #[test]
    fn entries_that_are_no_reference_stay_as_they_are() {
        let odd = record(
            "plugin",
            json!({"sketch": "f-sk1", "regions": [7, {"key": 3}], "points": [false]}),
        );

        assert_eq!(mirrored_uses(&odd), None);
    }

    #[test]
    fn a_later_reference_to_a_face_made_from_a_swapped_point_is_renamed() {
        // The hole names its bore after the point it drills at (features/hole.md section 3.4), so
        // the mirrored drawing renames the bore, and the chamfer that follows names it anew.
        let hole = record_of(
            "f-hole1",
            "hole",
            json!({"body": "b-1", "sketch": "f-sk1", "points": ["e-5.start"]}),
        );
        let edge = |point: &str| json!({"q": format!("edge(f-box1/top|f-hole1/bore/{point})"), "fp": "circle|n=0,0,1"});
        let chamfer = record_of(
            "f-cha1",
            "chamfer",
            json!({"body": "b-1", "edges": [edge("e-5.start"), edge("e-6")]}),
        );
        let mut renames = PointRenames::default();

        let hole_edit =
            turned_back_record(&hole, &id("f-sk1"), &drawing(), &mut renames).expect("mirrors");
        let chamfer_edit =
            turned_back_record(&chamfer, &id("f-sk1"), &drawing(), &mut renames).expect("mirrors");

        assert_eq!(updated_params(hole_edit)["points"], json!(["e-5.end"]));
        assert_eq!(
            updated_params(chamfer_edit),
            json!({"body": "b-1", "edges": [edge("e-5.end"), edge("e-6")]})
        );
    }
}
