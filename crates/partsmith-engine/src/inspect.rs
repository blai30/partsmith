//! Element listing, picks, reference resolution and measurement at the display state (engine.md
//! sections 3.9 and 3.11).

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_model::{Document, Id};
use partsmith_naming::{ElementKind, ElementName, Reference, Resolution as Resolved, resolve};
use partsmith_protocol::{
    DocHandle, ElementDescriptor, ElementRef, MeasureTarget, Measurement, MeshElement, Pick,
    Resolution, ResolutionStatus,
};

use crate::display::{ElementTable, kernel_failure};
use crate::document::OpenDocument;
use crate::elements::{describe, describe_element, element_fields, sub_shape_kind};
use crate::error::EngineError;
use crate::session::Session;
use crate::state::{BodyView, display_body};

impl Session {
    /// Elements of a body at the display state.
    pub fn list_elements(
        &mut self,
        doc: DocHandle,
        body: &Id,
        kind: Option<ElementKind>,
    ) -> Result<Vec<ElementDescriptor>, EngineError> {
        let (view, timeline) = {
            let open = self.open(doc)?;
            (body_with_geometry(open, body)?, timeline(&open.document))
        };
        let t0 = std::time::Instant::now();
        let table = self.table_of(&view)?;
        let descriptors = descriptors(&table, &view, &timeline)?;
        let t1 = std::time::Instant::now();
        let result = descriptors
            .iter()
            .filter(|descriptor| kind.is_none_or(|kind| descriptor.kind == kind))
            .cloned()
            .collect();
        eprintln!("TIMING list describe {:?} clone {:?}", t1 - t0, t1.elapsed());
        Ok(result)
    }

    /// Converts a pick in a current model-layer mesh into an element descriptor.
    pub fn pick_reference(
        &mut self,
        doc: DocHandle,
        pick: Pick,
    ) -> Result<ElementDescriptor, EngineError> {
        let t0 = std::time::Instant::now();
        let open = self.open(doc)?;
        let Some((body, shown)) = open
            .emitted
            .model
            .iter()
            .find(|(_, shown)| shown.mesh_id == pick.mesh_id)
        else {
            let preview = open
                .emitted
                .preview
                .values()
                .any(|shown| shown.mesh_id == pick.mesh_id);
            return Err(if preview {
                EngineError::PreviewNotPickable
            } else {
                EngineError::StalePick
            });
        };
        let table = &shown.mesh.table;
        if pick.index >= table.count(pick.kind) {
            return Err(EngineError::InvalidPick { kind: pick.kind });
        }
        let view = body_with_geometry(open, body)?;
        eprintln!("TIMING before describe {:?}", t0.elapsed());
        // Only the picked element is described, so a pick in a new body state does not query
        // the geometry of every element (section 5).
        describe_element(
            table,
            &view.state,
            &view.body,
            &timeline(&open.document),
            pick.kind,
            pick.index,
        )
    }

    /// Resolves references, each within its body, against the display state.
    pub fn resolve_references(
        &mut self,
        doc: DocHandle,
        references: &[ElementRef],
    ) -> Result<Vec<Resolution>, EngineError> {
        let mut views = Vec::with_capacity(references.len());
        {
            let open = self.open(doc)?;
            for element in references {
                if open.document.body(&element.body).is_none() {
                    return Err(EngineError::UnknownBody {
                        id: element.body.clone(),
                    });
                }
                let mesh_id = open
                    .emitted
                    .model
                    .get(&element.body)
                    .map_or(0, |shown| shown.mesh_id);
                let view = display_body(&open.document, &open.computed, &element.body)
                    .filter(BodyView::has_geometry);
                views.push((view, mesh_id));
            }
        }
        let mut resolutions = Vec::with_capacity(references.len());
        for (element, (view, mesh_id)) in references.iter().zip(views) {
            let Some(view) = view else {
                resolutions.push(Resolution {
                    status: ResolutionStatus::Missing,
                    matches: Vec::new(),
                });
                continue;
            };
            let (status, names) = resolve_in(&view, &element.reference)?;
            let table = self.table_of(&view)?;
            let mut matches: Vec<MeshElement> = names
                .iter()
                .filter_map(|name| {
                    Some(MeshElement {
                        body: element.body.clone(),
                        mesh_id,
                        kind: name.kind(),
                        index: table.find(name)?,
                    })
                })
                .collect();
            matches.sort_by_key(|found| (found.kind, found.index));
            resolutions.push(Resolution { status, matches });
        }
        Ok(resolutions)
    }

    /// Measures exactly one target at the display state.
    pub fn measure(
        &mut self,
        doc: DocHandle,
        targets: &[MeasureTarget],
    ) -> Result<Measurement, EngineError> {
        let [target] = targets else {
            return Err(EngineError::InvalidMeasure);
        };
        match target {
            MeasureTarget::Body { body } => {
                let view = body_with_geometry(self.open(doc)?, body)?;
                let table = self.table_of(&view)?;
                let shape = shape_of(&view)?;
                let center = shape.center_of_mass().map_err(kernel_failure)?;
                Ok(Measurement {
                    volume: Some(shape.volume().map_err(kernel_failure)?),
                    area: Some(shape.area().map_err(kernel_failure)?),
                    center_of_mass: Some([center.x, center.y, center.z]),
                    bounding_box: Some(shape.bounding_box().map_err(kernel_failure)?),
                    face_count: Some(table.count(ElementKind::Face)),
                    edge_count: Some(table.count(ElementKind::Edge)),
                    vertex_count: Some(table.count(ElementKind::Vertex)),
                    ..Measurement::default()
                })
            }
            MeasureTarget::Element { body, reference } => {
                let view = body_with_geometry(self.open(doc)?, body)?;
                let (kind, index) = unique_element(&view, reference)?;
                let geometry = shape_of(&view)?
                    .element(sub_shape_kind(kind), index)
                    .map_err(kernel_failure)?;
                let fields = element_fields(kind, &geometry);
                Ok(Measurement {
                    area: fields.area,
                    length: fields.length,
                    centroid: Some(fields.centroid),
                    surface: fields.surface,
                    curve: fields.curve,
                    normal: fields.normal,
                    direction: fields.direction,
                    axis: fields.axis,
                    radius: fields.radius,
                    ..Measurement::default()
                })
            }
        }
    }

    /// The element table of a body view with geometry.
    pub(crate) fn table_of(&mut self, view: &BodyView) -> Result<Arc<ElementTable>, EngineError> {
        let identity = view
            .identity
            .as_ref()
            .ok_or_else(|| EngineError::internal("an element table needs geometry"))?;
        self.cache.table(identity, &view.state)
    }
}

/// The display state of a body with geometry: `unknown_body` or `body_empty` otherwise.
pub(crate) fn body_with_geometry(open: &OpenDocument, body: &Id) -> Result<BodyView, EngineError> {
    let record = open
        .document
        .body(body)
        .ok_or_else(|| EngineError::UnknownBody { id: body.clone() })?;
    display_body(&open.document, &open.computed, body)
        .filter(BodyView::has_geometry)
        .ok_or_else(|| EngineError::BodyEmpty {
            name: record.name.clone(),
        })
}

/// The shape of a body view with geometry.
pub(crate) fn shape_of(view: &BodyView) -> Result<&partsmith_occt_sys::Shape, EngineError> {
    view.state
        .shape()
        .ok_or_else(|| EngineError::internal("a body with geometry has no shape"))
}

/// The timeline position of every feature, for the features of edges and vertices.
pub(crate) fn timeline(document: &Document) -> BTreeMap<Id, usize> {
    document
        .features
        .iter()
        .enumerate()
        .map(|(position, record)| (record.id.clone(), position))
        .collect()
}

/// The descriptors of a body's table, computed once per table.
fn descriptors(
    table: &Arc<ElementTable>,
    view: &BodyView,
    timeline: &BTreeMap<Id, usize>,
) -> Result<Arc<[ElementDescriptor]>, EngineError> {
    table.descriptors(|table| describe(table, &view.state, &view.body, timeline))
}

/// Resolves a reference in a body view: the status and the denoted element names.
fn resolve_in(
    view: &BodyView,
    reference: &Reference,
) -> Result<(ResolutionStatus, Vec<ElementName>), EngineError> {
    let elements = view
        .state
        .elements()
        .map_err(|error| EngineError::internal(error.to_string()))?;
    Ok(
        match resolve(reference, &elements, view.state.naming().aliases()) {
            Resolved::Resolved { elements, .. } => (ResolutionStatus::Resolved, elements),
            Resolved::Ambiguous { candidates } => (
                ResolutionStatus::Ambiguous,
                candidates
                    .into_iter()
                    .map(|candidate| candidate.name)
                    .collect(),
            ),
            Resolved::Missing { .. } => (ResolutionStatus::Missing, Vec::new()),
        },
    )
}

/// The kind and kernel index of the one element a reference resolves to
/// (`reference_unresolved` otherwise).
pub(crate) fn unique_element(
    view: &BodyView,
    reference: &Reference,
) -> Result<(ElementKind, u32), EngineError> {
    let unresolved = || EngineError::ReferenceUnresolved {
        kind: reference.query.kind().to_string(),
    };
    let (status, names) = resolve_in(view, reference)?;
    match (status, names.as_slice()) {
        (ResolutionStatus::Resolved, [name]) => {
            let index = view.state.naming().index(name).ok_or_else(unresolved)?;
            Ok((name.kind(), index))
        }
        _ => Err(unresolved()),
    }
}
