//! Sketch commands: candidate solves, drag frames and committing a drag (engine.md section 3.6),
//! and the context a stored sketch is solved in: its dimension values and its placement.

use std::sync::Arc;

use partsmith_features::{BodyState, DatumResult, OutputData, Registry};
use partsmith_model::{Document, Edit, FeatureRecord, Id, canonical};
use partsmith_naming::{Reference, Resolution, resolve};
use partsmith_occt_sys::{GeometryKind, SubShapeKind};
use partsmith_protocol::{DocHandle, SketchSolution};
use partsmith_sketch::{
    DimensionValues, NativeSolver, OriginPlane, Placement, PointRef, Region, RegionRef,
    SketchParams, SketchPlane, Solver, build_profile,
};
use serde_json::Value;

use crate::error::EngineError;
use crate::session::{Session, evaluate_in};
use crate::sketch::{
    Drag, Solved, dimension_values, has_point, sketch_params, sketch_record, solve,
};
use crate::state::{Computed, built_output, feature_label, state_before};

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
    let (values, placement) = sketch_inputs(document, computed, position, params)?;
    solve(feature, params, &values, placement)
}

/// What the sketch at timeline position `position` is solved with: its driving dimensions
/// evaluated in the document's context, and its plane resolved against the state before it.
fn sketch_inputs(
    document: &Document,
    computed: &Computed,
    position: usize,
    params: &SketchParams,
) -> Result<(DimensionValues, Placement), EngineError> {
    let values = evaluate_in(document, computed, |scope| dimension_values(params, scope))?;
    Ok((values, placement(&params.plane, computed, position)))
}

/// The placement of a sketch at timeline position `position`, resolved against the state
/// before it (features/sketch.md section 3.3): an origin plane's placement, the datum's plane
/// (the XY plane when the datum has no result), or the face's plane near the fallback (the
/// fallback when the face does not resolve to one flat face).
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
            ..
        } => {
            let state = state_before(&computed.report, position, body);
            face_placement(&state, face, fallback).unwrap_or(*fallback)
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

/// The placement on the face `face` of `state` near `fallback`, when the reference resolves and
/// its first face is flat.
fn face_placement(state: &BodyState, face: &Reference, fallback: &Placement) -> Option<Placement> {
    let shape = state.shape()?;
    let set = state.element_set().ok()?;
    let Resolution::Resolved {
        elements: names, ..
    } = resolve(face, &set)
    else {
        return None;
    };
    let index = state.naming().index(names.first()?)?;
    let geometry = shape.element(SubShapeKind::Face, index).ok()?;
    if geometry.kind != GeometryKind::Plane {
        return None;
    }
    let normal = geometry.direction?;
    let centroid = geometry.centroid;
    Placement::on_plane(
        [centroid.x, centroid.y, centroid.z],
        [normal.x, normal.y, normal.z],
        fallback,
    )
    .ok()
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
        let mut value = value.map_err(EngineError::unexpected)?;
        canonical::normalize_numbers(&mut value);
        updated.params.insert(key.to_string(), value);
        Ok::<(), EngineError>(())
    };
    set("entities", serde_json::to_value(&drag.last.entities))?;
    set("plane", serde_json::to_value(&plane))?;
    Ok(Some(updated))
}
