//! Event emission: what changed since the last events of a document (engine.md sections 3.4
//! and 3.8).

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::{OutputData, Registry};
use partsmith_model::{Document, Id};
use partsmith_naming::ElementKind;
use partsmith_protocol::{
    Buffer, BufferIndex, DocHandle, Event, Layer, MeshBuffers, SketchDisplay, SketchSolution,
};
use partsmith_recompute::Status;
use partsmith_sketch::Placement;

use crate::display::{DisplayCache, DisplayMesh, MeshKey};
use crate::document::OpenDocument;
use crate::error::EngineError;
use crate::session::Session;
use crate::sketch::{DATUM_PLANE_TYPE, SKETCH_TYPE, Solved, built, datum_placement, sketch_params};
use crate::sketching::solve_in;
use crate::state::{BodyView, display_bodies, preview_bodies};
use crate::status::{document_diagnostics, feature_statuses, parameter_values};
use crate::triangulate::fill_regions;

/// Events and the buffers they reference, collected for one reply.
#[derive(Debug, Default)]
pub(crate) struct Output {
    /// Events in emission order.
    pub(crate) events: Vec<Event>,
    /// Buffers in order of first reference.
    pub(crate) buffers: Vec<Buffer>,
}

impl Output {
    /// Adds a buffer and returns its index.
    pub(crate) fn buffer(&mut self, buffer: Buffer) -> BufferIndex {
        let index = BufferIndex(u32::try_from(self.buffers.len()).unwrap_or(u32::MAX));
        self.buffers.push(buffer);
        index
    }
}

/// What the latest events of a document described.
#[derive(Debug, Default)]
pub(crate) struct Emitted {
    /// The last `document_changed` fields.
    changed: Option<Changed>,
    /// The recompute run the last `recomputed` described.
    run: u64,
    /// The last solution of each sketch.
    sketches: BTreeMap<Id, SketchSolution>,
    /// The last placement (or its absence) of each datum plane.
    datums: BTreeMap<Id, Option<Placement>>,
    /// The model-layer mesh of each body that has one.
    pub(crate) model: BTreeMap<Id, Shown>,
    /// The preview-layer mesh of each body that has one.
    pub(crate) preview: BTreeMap<Id, Shown>,
    /// Re-send every mesh with a new id (after `set_tessellation`).
    pub(crate) refresh_meshes: bool,
}

/// The fields of a `document_changed` event besides the handle.
#[derive(Debug, Clone, PartialEq)]
struct Changed {
    /// The revision.
    revision: u64,
    /// The recipe.
    document: Arc<Document>,
    /// Whether it differs from the baseline.
    dirty: bool,
    /// The undo label.
    undo: Option<String>,
    /// The redo label.
    redo: Option<String>,
    /// The open transaction's label.
    transaction: Option<String>,
    /// The focus feature.
    focus: Option<Id>,
}

/// A displayed mesh.
#[derive(Debug, Clone)]
pub(crate) struct Shown {
    /// The id of the event that sent it.
    pub(crate) mesh_id: u64,
    /// Its body state and tolerance.
    key: MeshKey,
    /// The mesh; holding it pins it in the cache.
    pub(crate) mesh: Arc<DisplayMesh>,
}

impl Session {
    /// Emits the events of a document that describe what changed since its last events, in the
    /// order of section 3.4. Headless sessions emit no meshes and no display buffers.
    pub(crate) fn collect_events(
        &mut self,
        doc: DocHandle,
        output: &mut Output,
    ) -> Result<(), EngineError> {
        let Session {
            documents,
            cache,
            registry,
            next_mesh_id,
            display,
            ..
        } = self;
        let open = documents
            .get_mut(&doc)
            .ok_or(EngineError::UnknownDocument { doc })?;
        let revision = open.revision;
        document_changed(open, doc, output)?;
        if open.computed.run != open.emitted.run {
            open.emitted.run = open.computed.run;
            output.events.push(Event::Recomputed {
                doc,
                revision,
                features: feature_statuses(&open.document, &open.computed),
                parameters: parameter_values(&open.computed),
                diagnostics: document_diagnostics(&open.computed),
            });
        }
        sketch_events(registry, open, doc, *display, output);
        datum_events(open, doc, output);
        if *display {
            let mut meshes = Meshes {
                cache,
                next_mesh_id,
                doc,
                revision,
                output,
            };
            mesh_events(open, &mut meshes)?;
        }
        Ok(())
    }
}

/// Emits `document_changed` when any of its fields changed.
fn document_changed(
    open: &mut OpenDocument,
    doc: DocHandle,
    output: &mut Output,
) -> Result<(), EngineError> {
    let changed = Changed {
        revision: open.revision,
        document: Arc::clone(&open.document),
        dirty: open.dirty()?,
        undo: open.undo.undo_label().map(str::to_string),
        redo: open.undo.redo_label().map(str::to_string),
        transaction: open
            .transaction
            .as_ref()
            .map(|transaction| transaction.label.clone()),
        focus: open.focus().cloned(),
    };
    if open.emitted.changed.as_ref() == Some(&changed) {
        return Ok(());
    }
    output.events.push(Event::DocumentChanged {
        doc,
        revision: changed.revision,
        document: Document::clone(&changed.document),
        dirty: changed.dirty,
        undo: changed.undo.clone(),
        redo: changed.redo.clone(),
        transaction_open: changed.transaction.is_some(),
        transaction: changed.transaction.clone(),
        focus: changed.focus.clone(),
    });
    open.emitted.changed = Some(changed);
    Ok(())
}

/// The solution `sketch_solved` reports for the sketch at `position`: its recompute output when
/// it built, the engine's own solve when it failed, nothing when it is suppressed or rolled back
/// or the solve fails.
fn event_solution(
    registry: &Registry,
    open: &OpenDocument,
    position: usize,
) -> Option<Solved> {
    let record = open.document.features.get(position)?;
    let report = open.computed.report.features.get(position)?;
    match report.status {
        Status::Ok | Status::Warning => {
            let OutputData::Sketch(result) = &report.output.as_ref()?.data else {
                return None;
            };
            let params = sketch_params(registry, &open.document, record).ok()?;
            Some(built(&record.id, &params, result))
        }
        Status::Error => {
            let params = sketch_params(registry, &open.document, record).ok()?;
            solve_in(&open.document, &open.computed, position, &record.id, &params).ok()
        }
        Status::Suppressed | Status::RolledBack | Status::NotRun => None,
    }
}

/// Emits `sketch_solved` for each active sketch whose solution changed, in timeline order.
fn sketch_events(
    registry: &Registry,
    open: &mut OpenDocument,
    doc: DocHandle,
    display: bool,
    output: &mut Output,
) {
    let revision = open.revision;
    let mut present = Vec::new();
    for (position, record) in open.document.features.iter().enumerate() {
        if record.kind != SKETCH_TYPE {
            continue;
        }
        present.push(record.id.clone());
        let Some(solved) = event_solution(registry, open, position) else {
            continue;
        };
        if open.emitted.sketches.get(&record.id) == Some(&solved.solution) {
            continue;
        }
        open.emitted
            .sketches
            .insert(record.id.clone(), solved.solution.clone());
        let mut solution = solved.solution;
        if display {
            solution.display = Some(sketch_display(&solved.regions, output));
        }
        output.events.push(Event::SketchSolved {
            doc,
            revision,
            solution,
        });
    }
    open.emitted.sketches.retain(|id, _| present.contains(id));
}

/// The region fill buffers of a sketch solution.
pub(crate) fn sketch_display(
    regions: &[partsmith_sketch::Region],
    output: &mut Output,
) -> SketchDisplay {
    let fill = fill_regions(regions);
    let positions = fill
        .positions
        .iter()
        .flat_map(|point| point.map(|value| value as f32))
        .collect();
    let indices = fill.triangles.iter().flatten().copied().collect();
    SketchDisplay {
        region_positions: output.buffer(Buffer::F32(positions)),
        region_indices: output.buffer(Buffer::U32(indices)),
        region_ids: output.buffer(Buffer::U32(fill.regions)),
    }
}

/// Emits `datum_plane` for each active datum plane whose placement changed, in timeline order.
fn datum_events(open: &mut OpenDocument, doc: DocHandle, output: &mut Output) {
    let revision = open.revision;
    let mut present = Vec::new();
    for (record, report) in open
        .document
        .features
        .iter()
        .zip(&open.computed.report.features)
    {
        if record.kind != DATUM_PLANE_TYPE {
            continue;
        }
        present.push(record.id.clone());
        if matches!(
            report.status,
            Status::Suppressed | Status::RolledBack | Status::NotRun
        ) {
            continue;
        }
        let placement = datum_placement(&open.computed, &record.id);
        if open.emitted.datums.get(&record.id) == Some(&placement) {
            continue;
        }
        open.emitted.datums.insert(record.id.clone(), placement);
        output.events.push(Event::DatumPlane {
            doc,
            revision,
            feature: record.id.clone(),
            placement,
        });
    }
    open.emitted.datums.retain(|id, _| present.contains(id));
}

/// What mesh emission writes to besides the document.
struct Meshes<'a> {
    /// The display cache.
    cache: &'a mut DisplayCache,
    /// The session's mesh id counter.
    next_mesh_id: &'a mut u64,
    /// The document.
    doc: DocHandle,
    /// Its revision.
    revision: u64,
    /// The reply.
    output: &'a mut Output,
}

/// Emits `body_mesh` for every body whose model or preview mesh changed (section 3.4, items 5
/// and 6), then for removed bodies.
fn mesh_events(open: &mut OpenDocument, meshes: &mut Meshes<'_>) -> Result<(), EngineError> {
    let tolerance = crate::session::Session::display_tolerance(open);
    let refresh = std::mem::take(&mut open.emitted.refresh_meshes);
    let key = |view: &BodyView| {
        view.identity
            .clone()
            .filter(|_| view.has_geometry())
            .map(|identity| MeshKey::new(identity, &tolerance))
    };
    let model: Vec<(Id, Option<(MeshKey, BodyView)>)> = display_bodies(&open.document, &open.computed)
        .into_iter()
        .map(|view| (view.body.clone(), key(&view).map(|key| (key, view))))
        .collect();
    let previews = preview_bodies(&open.computed);
    let preview: Vec<(Id, Option<(MeshKey, BodyView)>)> = open
        .document
        .bodies
        .iter()
        .map(|body| {
            let wanted = previews
                .get(&body.id)
                .and_then(|view| key(view).map(|key| (key, view.clone())));
            (body.id.clone(), wanted)
        })
        .collect();
    layer_events(&mut open.emitted.model, Layer::Model, model, refresh, meshes)?;
    layer_events(&mut open.emitted.preview, Layer::Preview, preview, refresh, meshes)
}

/// Emits the `body_mesh` events of one layer: for each body in document order whose wanted mesh
/// differs from the shown one (or every shown mesh when `refresh`), then for shown bodies that
/// are no longer in the document.
fn layer_events(
    shown: &mut BTreeMap<Id, Shown>,
    layer: Layer,
    wanted: Vec<(Id, Option<(MeshKey, BodyView)>)>,
    refresh: bool,
    meshes: &mut Meshes<'_>,
) -> Result<(), EngineError> {
    let bodies: Vec<Id> = wanted.iter().map(|(body, _)| body.clone()).collect();
    for (body, wanted) in wanted {
        let current = shown.get(&body).map(|shown| &shown.key);
        let unchanged = current == wanted.as_ref().map(|(key, _)| key);
        if unchanged && !(refresh && wanted.is_some()) {
            continue;
        }
        let mesh_id = next_id(meshes.next_mesh_id);
        let mesh = match wanted {
            Some((key, view)) => {
                let mesh = meshes.cache.mesh(&key, &view.state)?;
                let buffers = mesh_buffers(&mesh, meshes.output);
                shown.insert(
                    body.clone(),
                    Shown {
                        mesh_id,
                        key,
                        mesh,
                    },
                );
                Some(buffers)
            }
            None => {
                shown.remove(&body);
                None
            }
        };
        meshes.output.events.push(Event::BodyMesh {
            doc: meshes.doc,
            revision: meshes.revision,
            body,
            layer,
            mesh_id,
            mesh,
        });
    }
    let removed: Vec<Id> = shown
        .keys()
        .filter(|body| !bodies.contains(body))
        .cloned()
        .collect();
    for body in removed {
        shown.remove(&body);
        let mesh_id = next_id(meshes.next_mesh_id);
        meshes.output.events.push(Event::BodyMesh {
            doc: meshes.doc,
            revision: meshes.revision,
            body,
            layer,
            mesh_id,
            mesh: None,
        });
    }
    Ok(())
}

/// Takes the next mesh id.
fn next_id(counter: &mut u64) -> u64 {
    let id = *counter;
    *counter += 1;
    id
}

/// Adds the buffers of a display mesh to the reply.
fn mesh_buffers(mesh: &DisplayMesh, output: &mut Output) -> MeshBuffers {
    let buffers = &mesh.buffers;
    MeshBuffers {
        positions: output.buffer(Buffer::F32(buffers.positions.clone())),
        normals: output.buffer(Buffer::F32(buffers.normals.clone())),
        indices: output.buffer(Buffer::U32(buffers.indices.clone())),
        face_ids: output.buffer(Buffer::U32(buffers.face_ids.clone())),
        edge_positions: output.buffer(Buffer::F32(buffers.edge_positions.clone())),
        edge_ids: output.buffer(Buffer::U32(buffers.edge_ids.clone())),
        vertex_positions: output.buffer(Buffer::F32(buffers.vertex_positions.clone())),
        vertex_ids: output.buffer(Buffer::U32(buffers.vertex_ids.clone())),
        face_count: mesh.table.count(ElementKind::Face),
        edge_count: mesh.table.count(ElementKind::Edge),
        vertex_count: mesh.table.count(ElementKind::Vertex),
        bounding_box: mesh.bounding_box,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffers_are_numbered_in_order() {
        let mut output = Output::default();

        let first = output.buffer(Buffer::U8(vec![1]));
        let second = output.buffer(Buffer::U32(vec![2]));

        assert_eq!((first, second), (BufferIndex(0), BufferIndex(1)));
        assert_eq!(output.buffers.len(), 2);
    }

    #[test]
    fn mesh_ids_count_up() {
        let mut counter = 4;

        assert_eq!(next_id(&mut counter), 4);
        assert_eq!(next_id(&mut counter), 5);
        assert_eq!(counter, 6);
    }

    #[test]
    fn region_fills_flatten_into_three_buffers() {
        let mut output = Output::default();

        let display = sketch_display(&[], &mut output);

        assert_eq!(display.region_positions, BufferIndex(0));
        assert_eq!(display.region_ids, BufferIndex(2));
        assert_eq!(output.buffers[0], Buffer::F32(Vec::new()));
    }
}
