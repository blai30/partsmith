//! Sketch commands: candidate solves, drag frames and committing a drag (engine.md section 3.6),
//! and the context a stored sketch is solved in: its dimension values and its placement.

use std::sync::Arc;

use partsmith_features::{DatumResult, OutputData, Registry};
use partsmith_model::{Document, Edit, FeatureRecord, Id};
use partsmith_protocol::{DocHandle, SketchSolution};
use partsmith_sketch::{
    DimensionValues, NativeSolver, OriginPlane, Placement, PointRef, Region, RegionRef,
    SketchParams, SketchPlane, Solver, build_profile,
};
use serde_json::Value;

use crate::error::EngineError;
use crate::frames::{face_placement, kept_face_plane, placement_hint, sketch_motion};
use crate::session::{Session, evaluate_in};
use crate::sketch::{
    Drag, Solved, dimension_values, has_point, sketch_params, sketch_record, solve,
};
use crate::state::{Computed, built_output, built_sketch, feature_label, state_before};

impl Session {
    /// Solves the stored sketch or a candidate in its place; records nothing.
    pub fn solve_sketch(
        &mut self,
        doc: DocHandle,
        feature: &Id,
        candidate: Option<SketchParams>,
    ) -> Result<SketchSolution, EngineError> {
        Ok(self.solve_sketch_command(doc, feature, candidate)?.solution)
    }

    /// One drag frame; records nothing.
    pub fn drag_sketch(
        &mut self,
        doc: DocHandle,
        feature: &Id,
        point: &PointRef,
        to: [f64; 2],
    ) -> Result<SketchSolution, EngineError> {
        Ok(self.drag_sketch_command(doc, feature, point, to)?.solution)
    }

    /// Ends a drag, optionally writing its geometry into the sketch; returns the revision.
    pub fn end_sketch_drag(
        &mut self,
        doc: DocHandle,
        feature: &Id,
        commit: bool,
    ) -> Result<u64, EngineError> {
        let open = self.open(doc)?;
        let (_, record) = sketch_record(&open.document, feature)?;
        let Some(drag) = open.drags.get(feature) else {
            return Err(EngineError::NoDrag {
                name: feature_label(&self.registry, &open.document, feature),
            });
        };
        let revision = open.revision;
        let update = if commit {
            dragged_record(record, drag)?
        } else {
            None
        };
        let Some(record) = update else {
            self.open_mut(doc)?.drags.remove(feature);
            return Ok(revision);
        };
        self.apply_batch(doc, "Drag", vec![Edit::UpdateFeature { feature: record }])
    }

    /// `solve_sketch` with the regions its display fills come from.
    pub(crate) fn solve_sketch_command(
        &self,
        doc: DocHandle,
        feature: &Id,
        candidate: Option<SketchParams>,
    ) -> Result<Solved, EngineError> {
        let open = self.open(doc)?;
        let (position, record) = sketch_record(&open.document, feature)?;
        let params = match candidate {
            Some(candidate) => {
                let mut record = record.clone();
                record.params = params_map(&candidate)?;
                sketch_params(&self.registry, &open.document, &record)?
            }
            None => sketch_params(&self.registry, &open.document, record)?,
        };
        solve_in(&open.document, &open.computed, position, feature, &params)
    }

    /// `drag_sketch` with the regions its display fills come from.
    pub(crate) fn drag_sketch_command(
        &mut self,
        doc: DocHandle,
        feature: &Id,
        point: &PointRef,
        to: [f64; 2],
    ) -> Result<Solved, EngineError> {
        let registry = Arc::clone(&self.registry);
        let open = self.open_mut(doc)?;
        let (position, record) = sketch_record(&open.document, feature)?;
        if let Some(drag) = open.drags.get_mut(feature)
            && drag.point == *point
        {
            drag.update(to)?;
            return Ok(drag.solved(feature));
        }
        let params = sketch_params(&registry, &open.document, record)?;
        if !has_point(&params, point) {
            return Err(EngineError::UnknownPoint {
                point: point.to_string(),
            });
        }
        let (values, placement) = sketch_inputs(&open.document, &open.computed, position, &params)?;
        let commit_plane = commit_plane(&params.plane, &placement, &open.computed, position)?;
        let drag = Drag::begin(params, values, placement, commit_plane, point.clone(), to)?;
        let solved = drag.solved(feature);
        open.drags.insert(feature.clone(), drag);
        Ok(solved)
    }
}

/// Solves `params` of the sketch at timeline position `position` the way `solve_sketch` does,
/// with the dimension values and placement `sketch_inputs` gives.
pub(crate) fn solve_in(
    document: &Document,
    computed: &Computed,
    position: usize,
    feature: &Id,
    params: &SketchParams,
) -> Result<Solved, EngineError> {
    let (values, placement) = sketch_inputs(document, computed, position, params)?;
    solve(feature, params, &values, placement)
}

/// What the sketch at timeline position `position` is solved with: its driving dimensions
/// evaluated in the document's context, and its placement, where the latest run placed it when
/// it built there on the same plane, else its plane resolved against the state before it.
fn sketch_inputs(
    document: &Document,
    computed: &Computed,
    position: usize,
    params: &SketchParams,
) -> Result<(DimensionValues, Placement), EngineError> {
    let values = evaluate_in(document, computed, |scope| dimension_values(params, scope))?;
    let placement = built_placement(document, computed, position, &params.plane)
        .unwrap_or_else(|| placement(&params.plane, computed, position));
    Ok((values, placement))
}

/// Where the sketch at timeline position `position` stands in the run of `computed`, when it
/// built there on `plane`: its record's plane is `plane`. That is where the run placed it, by
/// every step of features/sketch.md section 3.3 (a face that a later feature removed included).
fn built_placement(
    document: &Document,
    computed: &Computed,
    position: usize,
    plane: &SketchPlane,
) -> Option<Placement> {
    let record = document.features.get(position)?;
    let report = computed
        .report
        .features
        .get(position)
        .filter(|report| report.feature == record.id)?;
    let result = built_sketch(report)?;
    let stored: SketchPlane = serde_json::from_value(record.params.get("plane")?.clone()).ok()?;
    (stored == *plane).then_some(result.placement)
}

/// The plane a drag commit of the sketch at timeline position `position` on `plane` writes when
/// the drag is placed at `placement` (section 3.6): for a face sketch, `placement` as its
/// fallback in the coordinates that keep the sketch's motion (`kept_face_plane`, with the motion
/// of the stored face in the state just before the sketch, `BodyState::motion` of its query):
/// body coordinates, unless the fallback was in world coordinates on a face that moved. `None`
/// for other planes, which a commit keeps.
fn commit_plane(
    plane: &SketchPlane,
    placement: &Placement,
    computed: &Computed,
    position: usize,
) -> Result<Option<SketchPlane>, EngineError> {
    let SketchPlane::Face {
        body,
        face,
        fallback_frame,
        ..
    } = plane
    else {
        return Ok(None);
    };
    let face_motion = state_before(&computed.report, position, body).motion(&face.query);
    let kept = sketch_motion(*fallback_frame, &face_motion);
    kept_face_plane(body, face, placement, &face_motion, &kept)
        .map(Some)
        .ok_or_else(|| EngineError::internal("a drag commit keeps its sketch's motion"))
}

/// The placement of a sketch at timeline position `position` on `plane`, resolved against the
/// state before it (features/sketch.md section 3.3): an origin plane's placement, the datum's
/// plane (the XY plane when the datum has no result), or the face's plane with the fallback as
/// the hint, moved with the face when it is in body coordinates (that hint itself when the face
/// does not resolve to one flat face).
fn placement(plane: &SketchPlane, computed: &Computed, position: usize) -> Placement {
    match plane {
        SketchPlane::Origin { plane } => plane.placement(),
        SketchPlane::Datum { feature } => {
            datum_placement(computed, feature).unwrap_or_else(|| OriginPlane::Xy.placement())
        }
        SketchPlane::Face {
            body,
            face,
            fallback,
            fallback_frame,
        } => {
            let state = state_before(&computed.report, position, body);
            let hint = placement_hint(&state, face, fallback, *fallback_frame);
            face_placement(&state, face, &hint).unwrap_or(hint)
        }
    }
}

/// The plane a datum feature built, if it built one.
pub(crate) fn datum_placement(computed: &Computed, feature: &Id) -> Option<Placement> {
    let (_, report) = computed.feature(feature)?;
    match &built_output(report)?.data {
        OutputData::Datum(DatumResult::Plane { placement, .. }) => Some(*placement),
        _ => None,
    }
}

/// Every region of a stored sketch in solved order, or `None` when its params are invalid, a
/// dimension does not evaluate, or the solve fails or does not converge.
pub(crate) fn solved_regions(
    registry: &Registry,
    document: &Document,
    computed: &Computed,
    sketch: &Id,
) -> Option<Vec<RegionRef>> {
    let (_, record) = sketch_record(document, sketch).ok()?;
    let params = sketch_params(registry, document, record).ok()?;
    let values = evaluate_in(document, computed, |scope| dimension_values(&params, scope)).ok()?;
    let result = NativeSolver.solve(&params, &values).ok()?;
    if result.failed {
        return None;
    }
    Some(
        build_profile(&result.entities)
            .regions
            .iter()
            .map(Region::reference)
            .collect(),
    )
}

/// Sketch params as a feature record's params.
fn params_map(params: &SketchParams) -> Result<serde_json::Map<String, Value>, EngineError> {
    match serde_json::to_value(params) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err(EngineError::internal(
            "sketch params serialize to an object",
        )),
        Err(error) => Err(EngineError::invalid_sketch(error)),
    }
}

/// The record a committed drag writes (section 3.6): the stored record with the last frame's
/// entity geometry and, for a sketch on a face, the drag's commit plane (`commit_plane`). None
/// when the geometry equals the stored geometry exactly, so nothing rewrites a fallback on its
/// own. Applying the edit normalizes its numbers.
fn dragged_record(
    record: &FeatureRecord,
    drag: &Drag,
) -> Result<Option<FeatureRecord>, EngineError> {
    if drag.last.entities == drag.params.entities {
        return Ok(None);
    }
    let mut updated = record.clone();
    updated.params.insert(
        "entities".to_string(),
        serde_json::to_value(&drag.last.entities).map_err(EngineError::unexpected)?,
    );
    if let Some(plane) = &drag.commit_plane {
        updated.params.insert(
            "plane".to_string(),
            serde_json::to_value(plane).map_err(EngineError::unexpected)?,
        );
    }
    Ok(Some(updated))
}

#[cfg(test)]
mod tests {
    use partsmith_sketch::HintFrame;
    use serde_json::json;

    use super::*;
    use crate::SessionOptions;
    use crate::test_support::id;

    /// `box.ptsm` (a 20 x 30 x 40 box on XY) with `features` after its box.
    fn box_then(features: Vec<Value>) -> String {
        let mut document: Value = serde_json::from_str(include_str!("../tests/fixtures/box.ptsm"))
            .expect("box.ptsm is JSON");
        document["features"]
            .as_array_mut()
            .expect("documents list their features")
            .extend(features);
        document.to_string()
    }

    /// A face sketch `f-sk1` on the box's top holding one line, whose fallback has `origin`, x
    /// axis (1, 0, 0) and normal (0, 0, 1) in body coordinates.
    fn sketch_on_the_top(origin: [f64; 3]) -> Value {
        json!({
            "id": "f-sk1", "type": "sketch",
            "plane": {
                "kind": "face", "body": "b-1",
                "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"},
                "fallback": {"origin": origin, "x_axis": [1, 0, 0], "normal": [0, 0, 1]},
                "fallback_frame": "body"
            },
            "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [10, 0]}]
        })
    }

    /// A session with `text` open.
    fn opened(text: &str) -> (Session, DocHandle) {
        let mut session = Session::new(SessionOptions::headless());
        let doc = session
            .open_document(text, Vec::new())
            .expect("the document opens")
            .doc;
        (session, doc)
    }

    /// The placement `solve_sketch` gives `f-sk1` with its fallback at `origin` in `frame`, a
    /// candidate whose plane differs from the stored one, so the engine places it itself.
    fn candidate_placement(text: &str, origin: [f64; 3], frame: HintFrame) -> Placement {
        let (mut session, doc) = opened(text);
        let mut candidate = {
            let open = session.open(doc).expect("the document is open");
            let (_, record) = sketch_record(&open.document, &id("f-sk1")).expect("a sketch");
            sketch_params(&session.registry, &open.document, record).expect("valid params")
        };
        let SketchPlane::Face {
            fallback,
            fallback_frame,
            ..
        } = &mut candidate.plane
        else {
            panic!("f-sk1 is on a face");
        };
        fallback.origin = origin;
        *fallback_frame = frame;

        session
            .solve_sketch(doc, &id("f-sk1"), Some(candidate))
            .expect("the candidate solves")
            .placement
    }

    /// The box moved by (100, 0, 0), then the face sketch on its top.
    fn sketch_on_the_moved_top() -> String {
        box_then(vec![
            json!({
                "id": "f-mv1", "type": "move_body", "body": "b-1",
                "translation": ["100 mm", "0 mm", "0 mm"]
            }),
            sketch_on_the_top([0.0, 0.0, 40.0]),
        ])
    }

    #[test]
    fn a_candidate_hint_in_body_coordinates_moves_with_the_face() {
        let placement =
            candidate_placement(&sketch_on_the_moved_top(), [0.0, 5.0, 0.0], HintFrame::Body);

        assert_eq!(placement.origin, [100.0, 5.0, 40.0]);
        assert_eq!(placement.x_axis, [1.0, 0.0, 0.0]);
        assert_eq!(placement.normal, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn a_candidate_hint_in_world_coordinates_stays_where_it_is() {
        let placement = candidate_placement(
            &sketch_on_the_moved_top(),
            [0.0, 5.0, 0.0],
            HintFrame::World,
        );

        assert_eq!(placement.origin, [0.0, 5.0, 40.0]);
    }

    #[test]
    fn a_stored_sketch_is_solved_where_the_run_placed_it() {
        // A cut from z = 35 removes the top, so the run places the sketch where the top was just
        // before the cut (z = 40), not at its saved position (z = 10), which a candidate on a
        // face that does not resolve uses.
        let text = box_then(vec![
            json!({
                "id": "f-box2", "type": "box", "body": "b-1",
                "size": ["40 mm", "40 mm", "10 mm"], "position": ["0 mm", "0 mm", "35 mm"],
                "op": "cut"
            }),
            sketch_on_the_top([0.0, 0.0, 10.0]),
        ]);
        let (mut session, doc) = opened(&text);

        let solved = session
            .solve_sketch(doc, &id("f-sk1"), None)
            .expect("the sketch solves");

        assert_eq!(solved.placement.origin, [0.0, 0.0, 40.0]);
        assert_eq!(
            candidate_placement(&text, [0.0, 0.0, 10.0], HintFrame::World).origin,
            [0.0, 0.0, 10.0]
        );
    }
}
