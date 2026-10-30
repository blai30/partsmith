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

use crate::display::ElementTable;
use crate::document::OpenDocument;
use crate::elements::{describe, describe_element, element_fields, full_reference, sub_shape_kind};
use crate::error::EngineError;
use crate::session::Session;
use crate::state::{BodyView, display_body};

/// The `mesh_id` of a headless pick, which names its element by body and position (section 3.9).
const HEADLESS_MESH_ID: u64 = 0;

/// The largest volume of a body without volume, as a fraction of its area to the power 3/2
/// (section 3.11). A ball, the most volume an area bounds, has about 0.094.
const NO_VOLUME_RATIO: f64 = 1e-9;

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
        let table = self.table_of(&view)?;
        let descriptors = descriptors(&table, &view, &timeline)?;
        Ok(descriptors
            .iter()
            .filter(|descriptor| kind.is_none_or(|kind| descriptor.kind == kind))
            .cloned()
            .collect())
    }

    /// Converts a pick in a current model-layer mesh into an element descriptor: the entry of
    /// `list_elements` with the full reference (section 3.9). A headless pick names the element by
    /// `mesh_id` 0, its body, and its position among the elements of its kind in `list_elements`
    /// order.
    pub fn pick_reference(
        &mut self,
        doc: DocHandle,
        pick: Pick,
    ) -> Result<ElementDescriptor, EngineError> {
        let (view, timeline) = {
            let open = self.open(doc)?;
            let body = if self.display {
                displayed_body(open, &pick)?
            } else {
                match &pick.body {
                    Some(body) if pick.mesh_id == HEADLESS_MESH_ID => body.clone(),
                    _ => return Err(EngineError::StalePick),
                }
            };
            (body_with_geometry(open, &body)?, timeline(&open.document))
        };
        let table = self.table_of(&view)?;
        if pick.index >= table.count(pick.kind) {
            return Err(EngineError::InvalidPick { kind: pick.kind });
        }
        // Only the picked element is described, so a pick in a new body state does not query
        // the geometry of every element (section 5).
        let mut descriptor = describe_element(
            &table,
            &view.state,
            &view.body,
            &timeline,
            pick.kind,
            pick.index,
        )?;
        descriptor.reference = full_reference(table.element_set(), &descriptor.reference.query)
            .ok_or_else(|| {
                EngineError::internal("a displayed element is not in its element set")
            })?;
        Ok(descriptor)
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
                    .shown
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
                let shape = view.shape()?;
                // The body state's volume, which the feature that made it usually carries.
                let volume = view.state.volume().map_err(EngineError::unexpected)?;
                let area = shape.area().map_err(EngineError::unexpected)?;
                // A body without volume has no center of mass, and the kernel's point for it
                // would mean nothing (section 3.11).
                let center_of_mass = if has_volume(volume, area) {
                    let center = shape.center_of_mass().map_err(EngineError::unexpected)?;
                    Some([center.x, center.y, center.z])
                } else {
                    None
                };
                Ok(Measurement {
                    volume: Some(volume),
                    area: Some(area),
                    center_of_mass,
                    bounding_box: Some(shape.bounding_box().map_err(EngineError::unexpected)?),
                    face_count: Some(table.count(ElementKind::Face)),
                    edge_count: Some(table.count(ElementKind::Edge)),
                    vertex_count: Some(table.count(ElementKind::Vertex)),
                    ..Measurement::default()
                })
            }
            MeasureTarget::Element { body, reference } => {
                let view = body_with_geometry(self.open(doc)?, body)?;
                let (kind, index) = unique_element(&view, reference)?;
                let geometry = view
                    .shape()?
                    .element(sub_shape_kind(kind), index)
                    .map_err(EngineError::unexpected)?;
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
        self.cache
            .table(identity, &view.state)
            .map_err(EngineError::from)
    }
}

/// The body of the model-layer mesh a display session's pick names: `preview_not_pickable` for
/// a preview mesh, `stale_pick` for a mesh that is not the latest model-layer mesh of its body or
/// a `body` other than the mesh's.
fn displayed_body(open: &OpenDocument, pick: &Pick) -> Result<Id, EngineError> {
    let Some((body, _)) = open
        .emitted
        .model
        .shown
        .iter()
        .find(|(_, shown)| shown.mesh_id == pick.mesh_id)
    else {
        let preview = open
            .emitted
            .preview
            .shown
            .values()
            .any(|shown| shown.mesh_id == pick.mesh_id);
        return Err(if preview {
            EngineError::PreviewNotPickable
        } else {
            EngineError::StalePick
        });
    };
    if pick.body.as_ref().is_some_and(|picked| picked != body) {
        return Err(EngineError::StalePick);
    }
    Ok(body.clone())
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
    let set = view.state.element_set().map_err(EngineError::unexpected)?;
    Ok(match resolve(reference, &set) {
        Resolved::Resolved { elements, .. } => (ResolutionStatus::Resolved, elements),
        Resolved::Ambiguous { candidates } => (
            ResolutionStatus::Ambiguous,
            candidates
                .into_iter()
                .map(|candidate| candidate.name)
                .collect(),
        ),
        Resolved::Missing { .. } => (ResolutionStatus::Missing, Vec::new()),
    })
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

/// Whether a body with this volume (mm³) and area (mm²), both from one measurement, bounds
/// material: a body whose faces bound none, such as a solid whose void is its own outer shell, has
/// a volume of 0 up to a rounding that grows with its size (section 3.11).
fn has_volume(volume: f64, area: f64) -> bool {
    volume.abs() > NO_VOLUME_RATIO * area.powf(1.5)
}

#[cfg(test)]
mod tests {
    use super::has_volume;

    #[test]
    fn t164_a_volume_of_zero_is_no_volume() {
        assert!(!has_volume(0.0, 5200.0));
        assert!(!has_volume(-0.0, 5200.0));
    }

    #[test]
    fn t164_the_rounding_left_by_a_shell_and_its_void_is_no_volume() {
        // SELF_VOID_FILLET, the 213.7 x 177.1 x 93.3 mm box, the cylinder of radius 137.3 mm and
        // height 191.7 mm, and the box's rounding with the opposite sign.
        for (volume, area) in [
            (1.8189894035458565e-12, 5163.946891450772),
            (1.1641532182693481e-10, 148615.82),
            (2.3283064365386963e-10, 283822.16174033505),
            (-1.1641532182693481e-10, 148615.82),
        ] {
            assert!(
                !has_volume(volume, area),
                "volume {volume} with area {area} is no volume"
            );
        }
    }

    #[test]
    fn t164_bodies_that_bound_material_have_volume() {
        // box.ptsm, the same solid turned inside out, a 0.01 mm cube and a 100 x 100 x 0.001 mm
        // sheet.
        for (volume, area) in [
            (24000.0, 5200.0),
            (-24000.0, 5200.0),
            (1e-6, 6e-4),
            (10.0, 20000.4),
        ] {
            assert!(
                has_volume(volume, area),
                "volume {volume} with area {area} is a volume"
            );
        }
    }

    #[test]
    fn t164_the_limit_grows_with_the_area() {
        // Area 10000 puts the limit at 1e-3 mm³.
        assert!(!has_volume(5e-4, 10000.0));
        assert!(has_volume(2e-3, 10000.0));
    }
}
