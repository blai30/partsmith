//! Sketch commands: candidate solves, drag frames and committing a drag (engine.md section 3.6).

use std::sync::Arc;

use partsmith_model::{Document, Edit, FeatureRecord, Id, canonical};
use partsmith_protocol::{DocHandle, SketchSolution};
use partsmith_sketch::{PointRef, SketchParams, SketchPlane};
use serde_json::Value;

use crate::error::EngineError;
use crate::session::{Session, evaluate_in};
use crate::sketch::{
    Drag, Solved, dimension_values, has_point, placement, sketch_params, sketch_record, solve,
};
use crate::state::{Computed, feature_label};

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
        let values = evaluate_in(&open.document, &open.computed, |scope| {
            dimension_values(&params, scope)
        })?;
        let placement = placement(&params.plane, &open.computed, position);
        let drag = Drag::begin(params, values, placement, point.clone(), to)?;
        let solved = drag.solved(feature);
        open.drags.insert(feature.clone(), drag);
        Ok(solved)
    }
}

/// Solves `params` of the sketch at timeline position `position` the way `solve_sketch` does:
/// driving dimensions evaluated in the document's context, the plane resolved against the state
/// before the sketch.
pub(crate) fn solve_in(
    document: &Document,
    computed: &Computed,
    position: usize,
    feature: &Id,
    params: &SketchParams,
) -> Result<Solved, EngineError> {
    let values = evaluate_in(document, computed, |scope| dimension_values(params, scope))?;
    let placement = placement(&params.plane, computed, position);
    solve(feature, params, &values, placement)
}

/// Sketch params as a feature record's params.
fn params_map(params: &SketchParams) -> Result<serde_json::Map<String, Value>, EngineError> {
    match serde_json::to_value(params) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err(EngineError::internal(
            "sketch params serialize to an object",
        )),
        Err(error) => Err(EngineError::InvalidSketch {
            message: error.to_string(),
        }),
    }
}

/// The record a committed drag writes: the stored record with the last frame's entity geometry
/// and, for a sketch on a face, the resolved placement as its fallback. None when that equals
/// the stored geometry exactly.
fn dragged_record(
    record: &FeatureRecord,
    drag: &Drag,
) -> Result<Option<FeatureRecord>, EngineError> {
    let mut plane = drag.params.plane.clone();
    if let SketchPlane::Face { fallback, .. } = &mut plane {
        *fallback = drag.placement;
    }
    if drag.last.entities == drag.params.entities && plane == drag.params.plane {
        return Ok(None);
    }
    let mut updated = record.clone();
    let mut set = |key: &str, value: Result<Value, serde_json::Error>| {
        let mut value = value.map_err(|error| EngineError::internal(error.to_string()))?;
        canonical::normalize_numbers(&mut value);
        updated.params.insert(key.to_string(), value);
        Ok::<(), EngineError>(())
    };
    set("entities", serde_json::to_value(&drag.last.entities))?;
    set("plane", serde_json::to_value(&plane))?;
    Ok(Some(updated))
}
