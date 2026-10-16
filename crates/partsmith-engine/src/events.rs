//! Event emission: what changed since the last events of a document (engine.md sections 3.4
//! and 3.8).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use partsmith_features::{OutputData, Registry};
use partsmith_model::{Document, Id};
use partsmith_naming::{ElementKind, ElementName};
use partsmith_protocol::{
    Buffer, BufferIndex, DocHandle, Event, Layer, MeshBuffers, MeshElement, SketchDisplay,
    SketchSolution,
};
use partsmith_recompute::Status;
use partsmith_sketch::Placement;

use crate::counts::saturating_u32;
use crate::display::{DisplayCache, DisplayMesh, MeshKey};
use crate::document::OpenDocument;
use crate::error::EngineError;
use crate::session::Session;
use crate::sketch::{DATUM_PLANE_TYPE, SKETCH_TYPE, Solved, built, sketch_params};
use crate::sketching::{datum_placement, solve_in};
use crate::state::{BodyView, built_output, display_bodies, preview_bodies};
use crate::status::{DisplayFailure, document_diagnostics, feature_statuses, parameter_values};
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
        let index = BufferIndex(saturating_u32(self.buffers.len()));
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
    /// What the model layer shows.
    pub(crate) model: LayerState,
    /// What the preview layer shows.
    pub(crate) preview: LayerState,
    /// Re-send every mesh with a new id (after `set_tessellation`).
    pub(crate) refresh_meshes: bool,
}

impl Emitted {
    /// The current display failures, model layer first, each layer in body id order.
    pub(crate) fn display_failures(&self) -> Vec<DisplayFailure> {
        self.model
            .failures
            .values()
            .chain(self.preview.failures.values())
            .map(|failed| failed.failure.clone())
            .collect()
    }
}

/// What one display layer shows: the mesh of each body that has one, and the bodies whose mesh
/// could not be built.
#[derive(Debug, Default)]
pub(crate) struct LayerState {
    /// The mesh of each body that has one.
    pub(crate) shown: BTreeMap<Id, Shown>,
    /// The bodies whose mesh could not be built.
    failures: BTreeMap<Id, Failed>,
}

/// A mesh that could not be built: the body state and tolerance that failed, and its diagnostic.
#[derive(Debug, Clone)]
struct Failed {
    /// The body state and tolerance.
    key: MeshKey,
    /// What the feature statuses report.
    failure: DisplayFailure,
}

/// The fields of a `document_changed` event besides the handle and the recipe.
#[derive(Debug, Clone, PartialEq)]
struct Changed {
    /// The revision, which identifies the recipe: every recipe change increments it.
    revision: u64,
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
        // The one step that can fail comes before anything is recorded as emitted, so a failed
        // pass leaves every layer as the host last saw it.
        document_changed(open, doc, output)?;
        // Meshes are built and given their ids before the statuses, so `recomputed` can report
        // the bodies that could not be displayed and name the mesh elements of candidates.
        let meshes = display.then(|| {
            let (model, preview) = mesh_changes(open, cache);
            let model = record_layer(&mut open.emitted.model, model, next_mesh_id);
            let preview = record_layer(&mut open.emitted.preview, preview, next_mesh_id);
            (model, preview)
        });
        if open.computed.run != open.emitted.run {
            open.emitted.run = open.computed.run;
            let shown = &open.emitted.model.shown;
            let element = |body: &Id, name: &ElementName| {
                let shown = shown.get(body)?;
                Some(MeshElement {
                    body: body.clone(),
                    mesh_id: shown.mesh_id,
                    kind: name.kind(),
                    index: shown.mesh.table.find(name)?,
                })
            };
            let features = feature_statuses(
                &open.document,
                registry,
                &open.computed,
                &open.emitted.display_failures(),
                revision,
                display.then_some(&element),
            );
            output.events.push(Event::Recomputed {
                doc,
                revision,
                features,
                parameters: parameter_values(&open.computed),
                diagnostics: document_diagnostics(&open.computed),
            });
        }
        sketch_events(registry, open, doc, *display, output);
        datum_events(open, doc, output);
        if let Some((model, preview)) = meshes {
            emit_layer(Layer::Model, model, doc, revision, output);
            emit_layer(Layer::Preview, preview, doc, revision, output);
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
        document: Document::clone(&open.document),
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
fn event_solution(registry: &Registry, open: &OpenDocument, position: usize) -> Option<Solved> {
    let record = open.document.features.get(position)?;
    let report = open.computed.report.features.get(position)?;
    match report.status {
        Status::Ok | Status::Warning => {
            let OutputData::Sketch(result) = &built_output(report)?.data else {
                return None;
            };
            let params = sketch_params(registry, &open.document, record).ok()?;
            Some(built(&record.id, &params, result))
        }
        Status::Error => {
            let params = sketch_params(registry, &open.document, record).ok()?;
            solve_in(
                &open.document,
                &open.computed,
                position,
                &record.id,
                &params,
            )
            .ok()
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
    let mut present = BTreeSet::new();
    for (position, record) in open.document.features.iter().enumerate() {
        if record.kind != SKETCH_TYPE {
            continue;
        }
        present.insert(record.id.clone());
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
    let mut present = BTreeSet::new();
    for (record, report) in open
        .document
        .features
        .iter()
        .zip(&open.computed.report.features)
    {
        if record.kind != DATUM_PLANE_TYPE {
            continue;
        }
        present.insert(record.id.clone());
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

/// One `body_mesh` event to emit: the body and its new mesh with its key, or no mesh.
struct MeshChange {
    /// The body.
    body: Id,
    /// The mesh and the body state and tolerance it shows.
    mesh: Option<(MeshKey, Arc<DisplayMesh>)>,
}

/// One layer's wanted meshes: each body in document order with the key and state of the mesh it
/// should show, or `None` when it shows nothing.
type Wanted = Vec<(Id, Option<(MeshKey, BodyView)>)>;

/// Builds the meshes whose model or preview display changed (section 3.4, items 5 and 6) and
/// returns each layer's `body_mesh` changes in emission order. A body whose mesh cannot be built
/// shows no mesh, and its failure is recorded for the statuses of its provider feature.
fn mesh_changes(
    open: &mut OpenDocument,
    cache: &mut DisplayCache,
) -> (Vec<MeshChange>, Vec<MeshChange>) {
    let tolerance = open.display_tolerance();
    let refresh = std::mem::take(&mut open.emitted.refresh_meshes);
    let key = |view: &BodyView| {
        view.identity
            .clone()
            .map(|identity| MeshKey::new(identity, &tolerance))
    };
    let model: Wanted = display_bodies(&open.document, &open.computed)
        .into_iter()
        .map(|view| (view.body.clone(), key(&view).map(|key| (key, view))))
        .collect();
    let previews = preview_bodies(&open.computed);
    let preview: Wanted = open
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
    let emitted = &mut open.emitted;
    let mut planner = MeshPlanner {
        document: &open.document,
        cache,
        refresh,
    };
    let model = planner.changes(&mut emitted.model, model);
    let preview = planner.changes(&mut emitted.preview, preview);
    (model, preview)
}

/// What deciding one layer's changes reads and writes besides the layer.
struct MeshPlanner<'a> {
    /// The recipe, for body names.
    document: &'a Document,
    /// The display cache.
    cache: &'a mut DisplayCache,
    /// Re-send every shown mesh with a new id.
    refresh: bool,
}

impl MeshPlanner<'_> {
    /// The changes of one layer: for each body in document order whose wanted mesh differs from
    /// the shown one (or every shown mesh when refreshing), then for shown bodies that are no
    /// longer in the document. The layer's failures are updated with the meshes that cannot be
    /// built.
    fn changes(&mut self, layer: &mut LayerState, wanted: Wanted) -> Vec<MeshChange> {
        let LayerState { shown, failures } = layer;
        let bodies: Vec<Id> = wanted.iter().map(|(body, _)| body.clone()).collect();
        let mut changes = Vec::new();
        for (body, wanted) in wanted {
            let current = shown.get(&body).map(|shown| &shown.key);
            let Some((key, view)) = wanted else {
                failures.remove(&body);
                if current.is_some() {
                    changes.push(MeshChange { body, mesh: None });
                }
                continue;
            };
            let unchanged = current == Some(&key) && !self.refresh;
            let still_failing = failures.get(&body).is_some_and(|failed| failed.key == key);
            if unchanged || still_failing {
                continue;
            }
            match self.cache.mesh(&key, &view.state) {
                Ok(mesh) => {
                    failures.remove(&body);
                    changes.push(MeshChange {
                        body,
                        mesh: Some((key, mesh)),
                    });
                }
                Err(error) => {
                    let failure = display_failure(self.document, &view, &error);
                    failures.insert(body.clone(), Failed { key, failure });
                    if current.is_some() {
                        changes.push(MeshChange { body, mesh: None });
                    }
                }
            }
        }
        failures.retain(|body, _| bodies.contains(body));
        changes.extend(
            shown
                .keys()
                .filter(|body| !bodies.contains(body))
                .map(|body| MeshChange {
                    body: body.clone(),
                    mesh: None,
                }),
        );
        changes
    }
}

/// The diagnostic of a body whose display mesh could not be built.
fn display_failure(document: &Document, view: &BodyView, error: &EngineError) -> DisplayFailure {
    let name = document
        .body(&view.body)
        .map_or_else(|| view.body.to_string(), |body| body.name.clone());
    let detail = match error {
        EngineError::Internal { detail, .. } => detail.clone(),
        other => other.to_string(),
    };
    DisplayFailure {
        feature: view.provider.clone(),
        message: format!(
            "Partsmith could not display body \"{name}\" ({detail}). Please report this."
        ),
    }
}

/// One `body_mesh` event to emit: the body, the id of its event and its mesh, if it has one.
struct Recorded {
    /// The body.
    body: Id,
    /// The id of the event.
    mesh_id: u64,
    /// The mesh; absent when the body shows none.
    mesh: Option<Arc<DisplayMesh>>,
}

/// Gives each of one layer's changes the next mesh id and records what the layer shows.
fn record_layer(
    state: &mut LayerState,
    changes: Vec<MeshChange>,
    next_mesh_id: &mut u64,
) -> Vec<Recorded> {
    changes
        .into_iter()
        .map(|change| {
            let mesh_id = next_id(next_mesh_id);
            let mesh = match change.mesh {
                Some((key, mesh)) => {
                    let shown = Shown {
                        mesh_id,
                        key,
                        mesh: Arc::clone(&mesh),
                    };
                    state.shown.insert(change.body.clone(), shown);
                    Some(mesh)
                }
                None => {
                    state.shown.remove(&change.body);
                    None
                }
            };
            Recorded {
                body: change.body,
                mesh_id,
                mesh,
            }
        })
        .collect()
}

/// Emits the `body_mesh` events of one layer's recorded changes.
fn emit_layer(
    layer: Layer,
    recorded: Vec<Recorded>,
    doc: DocHandle,
    revision: u64,
    output: &mut Output,
) {
    for change in recorded {
        let mesh = change.mesh.map(|mesh| mesh_buffers(&mesh, output));
        output.events.push(Event::BodyMesh {
            doc,
            revision,
            body: change.body,
            layer,
            mesh_id: change.mesh_id,
            mesh,
        });
    }
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
    use partsmith_features::BodyState;
    use partsmith_protocol::{FeatureState, Pick};
    use partsmith_recompute::{CacheKey, Report};

    use super::*;
    use crate::test_support::box_session;

    /// Replaces the document's latest recompute by a new run in which `b-1` holds its shape with
    /// an empty naming map, which no element table accepts, as a state written under `key`.
    fn break_display(session: &mut Session, doc: DocHandle, key: u8) {
        let open = session.open_mut(doc).expect("the document is open");
        let mut report = Report::clone(&open.computed.report);
        let shape = report.bodies[0]
            .state
            .shape()
            .cloned()
            .expect("the box has a shape");
        report.bodies[0].state = BodyState::new(shape, Arc::clone(BodyState::empty().naming()));
        report.features[0].key = Some(CacheKey([key; 32]));
        open.computed.report = Arc::new(report);
        open.computed.run += 1;
    }

    /// The events of the document's next emission.
    fn events(session: &mut Session, doc: DocHandle) -> Vec<Event> {
        let mut output = Output::default();
        session
            .collect_events(doc, &mut output)
            .expect("events never fail because of one body");
        output.events
    }

    /// The status of `f-box1` in a `recomputed` event.
    fn box_status(events: &[Event]) -> partsmith_protocol::FeatureStatus {
        events
            .iter()
            .find_map(|event| match event {
                Event::Recomputed { features, .. } => features.first().cloned(),
                _ => None,
            })
            .expect("a recomputed event with f-box1")
    }

    /// The model-layer `body_mesh` events: `(body, has a mesh)`.
    fn model_meshes(events: &[Event]) -> Vec<(String, bool)> {
        events
            .iter()
            .filter_map(|event| match event {
                Event::BodyMesh {
                    body,
                    layer: Layer::Model,
                    mesh,
                    ..
                } => Some((body.to_string(), mesh.is_some())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_body_that_cannot_be_displayed_is_an_error_of_its_feature() {
        let (mut session, doc) = box_session(true);
        break_display(&mut session, doc, 7);

        let first = events(&mut session, doc);
        let status = box_status(&first);

        assert_eq!(status.status, FeatureState::Error);
        assert_eq!(status.messages[0].code, "display_failed");
        assert!(
            status.messages[0].message.contains("\"Block\""),
            "{}",
            status.messages[0].message
        );
        assert_eq!(model_meshes(&first), []);
        assert_eq!(
            session.feature_statuses(doc).expect("the document is open")[0],
            status
        );
    }

    #[test]
    fn a_failed_display_is_not_retried_until_the_state_changes() {
        let (mut session, doc) = box_session(true);
        break_display(&mut session, doc, 7);
        events(&mut session, doc);
        session.open_mut(doc).expect("open").emitted.refresh_meshes = true;

        let again = events(&mut session, doc);

        assert_eq!(model_meshes(&again), []);
    }

    #[test]
    fn a_shown_body_that_can_no_longer_be_displayed_loses_its_mesh() {
        let (mut session, doc) = box_session(true);
        let opened = events(&mut session, doc);
        assert_eq!(model_meshes(&opened), [("b-1".to_string(), true)]);
        let shown = session.open(doc).expect("open").emitted.model.shown["b-1"].mesh_id;

        break_display(&mut session, doc, 7);
        let broken = events(&mut session, doc);
        let pick = session.pick_reference(
            doc,
            Pick {
                mesh_id: shown,
                kind: ElementKind::Face,
                index: 0,
                body: None,
            },
        );

        assert_eq!(model_meshes(&broken), [("b-1".to_string(), false)]);
        assert_eq!(box_status(&broken).status, FeatureState::Error);
        assert_eq!(pick, Err(EngineError::StalePick));
    }

    #[test]
    fn a_display_failure_clears_when_the_body_displays_again() {
        let (mut session, doc) = box_session(true);
        let good = Arc::clone(&session.open(doc).expect("open").computed.report);
        break_display(&mut session, doc, 7);
        events(&mut session, doc);

        let open = session.open_mut(doc).expect("open");
        open.computed.report = good;
        open.computed.run += 1;
        let repaired = events(&mut session, doc);

        assert_eq!(box_status(&repaired).status, FeatureState::Ok);
        assert_eq!(model_meshes(&repaired), [("b-1".to_string(), true)]);
    }

    #[test]
    fn a_failed_pass_sends_its_meshes_with_the_next_one() {
        let (mut session, doc) = box_session(true);
        let open = session.open_mut(doc).expect("open");
        let readable = Arc::clone(&open.document);
        let mut unsaveable = Document::clone(&readable);
        unsaveable.bodies.push(unsaveable.bodies[0].clone());
        open.change(unsaveable);
        let mut output = Output::default();
        assert!(session.collect_events(doc, &mut output).is_err());

        session
            .open_mut(doc)
            .expect("open")
            .change(Document::clone(&readable));
        let next = events(&mut session, doc);

        assert_eq!(model_meshes(&next), [("b-1".to_string(), true)]);
    }

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
