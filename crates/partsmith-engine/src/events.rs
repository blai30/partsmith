//! Event emission: what changed since the last events of a document (engine.md sections 3.4
//! and 3.8).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use partsmith_features::Registry;
use partsmith_model::{Document, Id};
use partsmith_naming::{ElementKind, ElementName};
use partsmith_protocol::{
    Buffer, BufferIndex, DocHandle, Event, Layer, MeshBuffers, MeshElement, SketchDisplay,
    SketchSolution,
};
use partsmith_sketch::Placement;

use crate::counts::saturating_u32;
use crate::display::{DisplayCache, DisplayError, DisplayMesh, MeshKey};
use crate::document::OpenDocument;
use crate::error::EngineError;
use crate::fixes::offer_finding_fixes;
use crate::inspections::Inspections;
use crate::session::Session;
use crate::sketch::{DATUM_PLANE_TYPE, SKETCH_TYPE};
use crate::sketching::{datum_placement, stored_solution};
use crate::state::{BodyView, display_bodies, preview_bodies, ran};
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
    /// Send `recomputed` again with the latest recompute's statuses, which changed without a
    /// recompute: a commit made `Undo` available, or display failures appeared or cleared
    /// (section 3.4).
    restate: bool,
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
    /// Sends `recomputed` again with the next events, with the statuses of the latest recompute.
    pub(crate) fn restate_statuses(&mut self) {
        self.restate = true;
    }

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

/// A mesh that could not be built: the body state and tolerance that failed, and the failure.
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
    /// How many expressions and parameters are numbers without a unit (section 3.25).
    bare_numbers: u32,
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
            inspections,
            ..
        } = self;
        let open = documents
            .get_mut(&doc)
            .ok_or(EngineError::UnknownDocument { doc })?;
        let revision = open.revision;
        // The one step that can fail comes before anything is recorded as emitted, so a failed
        // pass leaves every layer as the host last saw it.
        document_changed(registry, inspections, open, doc, output)?;
        // Meshes are built and given their ids before the statuses, so `recomputed` can report
        // the bodies that could not be displayed and name the mesh elements of candidates.
        let meshes = display.then(|| {
            let (model, preview) = mesh_changes(open, cache);
            let model = record_layer(&mut open.emitted.model, model, next_mesh_id);
            let preview = record_layer(&mut open.emitted.preview, preview, next_mesh_id);
            (model, preview)
        });
        // `restate_statuses` asks for the statuses again within the same run, once.
        let restate = std::mem::take(&mut open.emitted.restate);
        if open.computed.run != open.emitted.run || restate {
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
            let mut features = feature_statuses(open, registry, display.then_some(&element));
            if *display {
                open.ghosts.add_to(&mut features, &open.causes, output);
            }
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
    registry: &Registry,
    inspections: &mut Inspections,
    open: &mut OpenDocument,
    doc: DocHandle,
    output: &mut Output,
) -> Result<(), EngineError> {
    let changed = Changed {
        bare_numbers: open.bare_numbers(registry, inspections),
        revision: open.revision,
        dirty: open.dirty()?,
        undo: open.history.undo_label().map(str::to_string),
        redo: open.history.redo_label().map(str::to_string),
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
        bare_numbers: changed.bare_numbers,
    });
    open.emitted.changed = Some(changed);
    Ok(())
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
        let Some(mut solved) = stored_solution(registry, &open.document, &open.computed, position)
        else {
            continue;
        };
        offer_finding_fixes(&mut solved.solution, revision, open.document.units);
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
        if !ran(report.status) {
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

/// A body whose shown mesh changes: its new mesh with its key, or none.
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
    let failures = emitted.display_failures();
    let mut planner = MeshPlanner { cache, refresh };
    let model = planner.changes(&mut emitted.model, model);
    let preview = planner.changes(&mut emitted.preview, preview);
    // The statuses report display failures, so they are sent again when these change.
    if emitted.display_failures() != failures {
        emitted.restate_statuses();
    }
    (model, preview)
}

/// What deciding one layer's changes reads and writes besides the layer.
struct MeshPlanner<'a> {
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
                    let failure = display_failure(&view, error);
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

/// The failure of a body whose display mesh could not be built (section 3.8).
fn display_failure(view: &BodyView, error: DisplayError) -> DisplayFailure {
    DisplayFailure {
        feature: view.provider.clone(),
        body: view.body.clone(),
        text: error.text,
    }
}

/// One `body_mesh` event to emit: the body, the id of its event and its mesh, if it has one.
struct NumberedMesh {
    /// The body.
    body: Id,
    /// The mesh id its `body_mesh` event carries.
    mesh_id: u64,
    /// The mesh; absent when the body shows none.
    mesh: Option<Arc<DisplayMesh>>,
}

/// Gives each of one layer's changes the next mesh id and records what the layer shows.
fn record_layer(
    state: &mut LayerState,
    changes: Vec<MeshChange>,
    next_mesh_id: &mut u64,
) -> Vec<NumberedMesh> {
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
            NumberedMesh {
                body: change.body,
                mesh_id,
                mesh,
            }
        })
        .collect()
}

/// Emits the `body_mesh` events of one layer's numbered meshes.
fn emit_layer(
    layer: Layer,
    numbered: Vec<NumberedMesh>,
    doc: DocHandle,
    revision: u64,
    output: &mut Output,
) {
    for change in numbered {
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
    use partsmith_protocol::{
        Diagnostic, FeatureState, FeatureStatus, Pick, Severity, TessellationQuality,
    };
    use partsmith_recompute::{CacheKey, Message, Report};

    use super::*;
    use crate::test_support::box_session;

    /// The `display_failed` message of `b-1` in `box.ptsm`, named `name`, when its body state has
    /// an empty naming map (engine.md section 3.8, test 159).
    fn display_failed(name: &str) -> Diagnostic {
        Diagnostic {
            detail: Some(
                "body b-1: naming the result failed: The topology does not match the naming map's element counts."
                    .to_string(),
            ),
            ..Diagnostic::new(
                Severity::Warning,
                "display_failed",
                format!(
                    "Body '{name}' cannot be shown, and Partsmith could not tell why. Report this with the details."
                ),
            )
        }
    }

    /// The document's latest report.
    fn latest_report(session: &Session, doc: DocHandle) -> Arc<Report> {
        Arc::clone(
            &session
                .open(doc)
                .expect("the document is open")
                .computed
                .report,
        )
    }

    /// A copy of `report` in which `b-1` holds its shape with an empty naming map, which no
    /// element table accepts, as a state written under a cache key of its own.
    fn broken(report: &Report) -> Report {
        let mut report = report.clone();
        let shape = report.bodies[0]
            .state
            .shape()
            .cloned()
            .expect("the box has a shape");
        report.bodies[0].state = BodyState::new(shape, Arc::clone(BodyState::empty().naming()));
        report.features[0].key = Some(CacheKey([7; 32]));
        report
    }

    /// Makes `report` the document's latest recompute: a new run when `new_run`, else the same
    /// run, as a change of the display without a recompute.
    fn replace_report(session: &mut Session, doc: DocHandle, report: Arc<Report>, new_run: bool) {
        let open = session.open_mut(doc).expect("the document is open");
        open.computed.report = report;
        if new_run {
            open.computed.run += 1;
        }
    }

    /// Replaces the document's latest recompute by a new run in which `b-1` cannot be displayed.
    fn break_display(session: &mut Session, doc: DocHandle) {
        let report = broken(&latest_report(session, doc));
        replace_report(session, doc, Arc::new(report), true);
    }

    /// The events of the document's next emission.
    fn events(session: &mut Session, doc: DocHandle) -> Vec<Event> {
        let mut output = Output::default();
        session
            .collect_events(doc, &mut output)
            .expect("events never fail because of one body");
        output.events
    }

    /// The revision and the status of `f-box1` of each `recomputed` event.
    fn recomputed(events: &[Event]) -> Vec<(u64, FeatureStatus)> {
        events
            .iter()
            .filter_map(|event| match event {
                Event::Recomputed {
                    revision, features, ..
                } => Some((
                    *revision,
                    features.first().cloned().expect("box.ptsm has f-box1"),
                )),
                _ => None,
            })
            .collect()
    }

    /// The revision and the status of `f-box1` of the one `recomputed` event.
    fn only_recomputed(events: &[Event]) -> (u64, FeatureStatus) {
        match recomputed(events).as_slice() {
            [only] => only.clone(),
            other => panic!("expected one recomputed event, got {other:?}"),
        }
    }

    /// The status of `f-box1` in the one `recomputed` event.
    fn box_status(events: &[Event]) -> FeatureStatus {
        only_recomputed(events).1
    }

    /// The `body_mesh` events: `(layer, body, has a mesh)`.
    fn meshes(events: &[Event]) -> Vec<(Layer, String, bool)> {
        events
            .iter()
            .filter_map(|event| match event {
                Event::BodyMesh {
                    body, layer, mesh, ..
                } => Some((*layer, body.to_string(), mesh.is_some())),
                _ => None,
            })
            .collect()
    }

    /// The model-layer `body_mesh` events: `(body, has a mesh)`.
    fn model_meshes(events: &[Event]) -> Vec<(String, bool)> {
        meshes(events)
            .into_iter()
            .filter(|(layer, _, _)| *layer == Layer::Model)
            .map(|(_, body, has_mesh)| (body, has_mesh))
            .collect()
    }

    /// The status `feature_statuses` reports for `f-box1`.
    fn queried_box_status(session: &Session, doc: DocHandle) -> FeatureStatus {
        session
            .feature_statuses(doc)
            .expect("the document is open")
            .first()
            .cloned()
            .expect("box.ptsm has f-box1")
    }

    #[test]
    fn t159_a_a_body_that_cannot_be_displayed_is_a_warning_of_its_feature() {
        let (mut session, doc) = box_session(true);
        break_display(&mut session, doc);

        let first = events(&mut session, doc);
        let status = box_status(&first);

        assert_eq!(meshes(&first), []);
        assert_eq!(status.status, FeatureState::Warning);
        assert_eq!(status.messages, [display_failed("Block")]);
        assert_eq!(queried_box_status(&session, doc), status);
    }

    #[test]
    fn t159_b_a_shown_body_that_can_no_longer_be_displayed_loses_its_mesh() {
        let (mut session, doc) = box_session(true);
        let opened = events(&mut session, doc);
        assert_eq!(model_meshes(&opened), [("b-1".to_string(), true)]);
        let shown = session.open(doc).expect("open").emitted.model.shown["b-1"].mesh_id;

        break_display(&mut session, doc);
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
        assert_eq!(box_status(&broken).messages, [display_failed("Block")]);
        assert_eq!(pick, Err(EngineError::StalePick));
    }

    #[test]
    fn t159_c_meshes_sent_again_at_the_same_quality_keep_the_failure() {
        let (mut session, doc) = box_session(true);
        break_display(&mut session, doc);
        events(&mut session, doc);

        session
            .set_tessellation(doc, TessellationQuality::Standard)
            .expect("the document is open");
        let again = events(&mut session, doc);

        assert_eq!(meshes(&again), []);
        assert_eq!(recomputed(&again), []);
        assert_eq!(
            queried_box_status(&session, doc).messages,
            [display_failed("Block")]
        );
    }

    #[test]
    fn t159_d_a_display_failure_clears_when_the_body_displays_again() {
        let (mut session, doc) = box_session(true);
        let good = latest_report(&session, doc);
        break_display(&mut session, doc);
        events(&mut session, doc);

        replace_report(&mut session, doc, good, true);
        let repaired = events(&mut session, doc);
        let status = box_status(&repaired);

        assert_eq!(model_meshes(&repaired), [("b-1".to_string(), true)]);
        assert_eq!(status.status, FeatureState::Ok);
        assert_eq!(status.messages, []);
    }

    #[test]
    fn t159_e_a_display_that_recovers_without_a_recompute_sends_the_statuses_again() {
        let (mut session, doc) = box_session(true);
        let good = latest_report(&session, doc);
        break_display(&mut session, doc);
        events(&mut session, doc);
        let revision = session.revision(doc).expect("the document is open");

        replace_report(&mut session, doc, good, false);
        let repaired = events(&mut session, doc);

        let (sent_revision, status) = only_recomputed(&repaired);

        assert_eq!(model_meshes(&repaired), [("b-1".to_string(), true)]);
        assert_eq!(sent_revision, revision);
        assert_eq!(status.status, FeatureState::Ok);
        assert_eq!(status.messages, []);
    }

    #[test]
    fn t159_e_a_display_that_fails_without_a_recompute_sends_the_statuses_again() {
        let (mut session, doc) = box_session(true);
        events(&mut session, doc);
        let revision = session.revision(doc).expect("the document is open");

        let report = broken(&latest_report(&session, doc));
        replace_report(&mut session, doc, Arc::new(report), false);
        let broken_events = events(&mut session, doc);

        let (sent_revision, status) = only_recomputed(&broken_events);

        assert_eq!(model_meshes(&broken_events), [("b-1".to_string(), false)]);
        assert_eq!(sent_revision, revision);
        assert_eq!(status.status, FeatureState::Warning);
        assert_eq!(status.messages, [display_failed("Block")]);
    }

    #[test]
    fn t159_f_a_renamed_body_that_cannot_be_displayed_is_named_by_its_new_name() {
        let (mut session, doc) = box_session(true);
        let report = Arc::new(broken(&latest_report(&session, doc)));
        replace_report(&mut session, doc, Arc::clone(&report), true);
        events(&mut session, doc);

        let open = session.open_mut(doc).expect("the document is open");
        let mut renamed = Document::clone(&open.document);
        renamed.bodies[0].name = "Lid".to_string();
        open.change(renamed);
        replace_report(&mut session, doc, report, true);
        let after_rename = events(&mut session, doc);
        let status = box_status(&after_rename);

        assert_eq!(meshes(&after_rename), []);
        assert_eq!(status.messages, [display_failed("Lid")]);
        assert_eq!(queried_box_status(&session, doc), status);
    }

    #[test]
    fn display_failed_comes_before_the_features_own_messages_and_raises_ok_to_warning() {
        let (mut session, doc) = box_session(true);
        let mut report = broken(&latest_report(&session, doc));
        report.features[0].messages.push(Message {
            severity: partsmith_recompute::Severity::Info,
            code: "test_notice",
            pointer: None,
            text: "This box followed a change on its own.".to_string(),
            suggestion: None,
            repairs: Vec::new(),
            fixes: Vec::new(),
            detail: None,
            root: None,
            reason: None,
        });
        replace_report(&mut session, doc, Arc::new(report), true);

        let status = box_status(&events(&mut session, doc));
        let codes: Vec<&str> = status
            .messages
            .iter()
            .map(|message| message.code.as_str())
            .collect();

        assert_eq!(status.status, FeatureState::Warning);
        assert_eq!(codes, ["display_failed", "test_notice"]);
    }

    #[test]
    fn headless_sessions_report_no_display_failure() {
        let (mut session, doc) = box_session(false);
        break_display(&mut session, doc);

        let status = box_status(&events(&mut session, doc));

        assert_eq!(status.status, FeatureState::Ok);
        assert_eq!(status.messages, []);
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
