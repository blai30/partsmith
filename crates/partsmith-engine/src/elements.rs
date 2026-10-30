//! Element descriptors and their geometry (engine.md sections 3.9 and 3.11).

use std::collections::BTreeMap;

use partsmith_features::{BodyState, PlacementExt};
use partsmith_model::Id;
use partsmith_naming::{ElementKind, ElementName, ElementSet, Motion, Reference};
use partsmith_occt_sys::{ElementGeometry, GeometryKind, SubShapeKind};
use partsmith_protocol::{Axis, CurveKind, ElementDescriptor, SurfaceKind, Vec3};
use partsmith_sketch::{OriginPlane, Placement};

use crate::display::{ElementTable, TableEntry, slot};
use crate::error::EngineError;
use crate::frames::undoing;

/// The geometry fields of one element (protocol.md section 2.4), as descriptors and
/// measurements report them.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct ElementFields {
    /// Surface type (faces).
    pub(crate) surface: Option<SurfaceKind>,
    /// Curve type (edges).
    pub(crate) curve: Option<CurveKind>,
    /// Area centroid, length centroid or position.
    pub(crate) centroid: Vec3,
    /// Area (faces).
    pub(crate) area: Option<f64>,
    /// Length (edges).
    pub(crate) length: Option<f64>,
    /// Outward unit normal (planar faces).
    pub(crate) normal: Option<Vec3>,
    /// Unit direction (line edges).
    pub(crate) direction: Option<Vec3>,
    /// Axis (cylinder, cone and torus faces; circle edges).
    pub(crate) axis: Option<Axis>,
    /// Radius (cylinder and sphere faces; circle edges).
    pub(crate) radius: Option<f64>,
    /// The sketch placement on a planar face.
    pub(crate) placement: Option<Placement>,
}

impl ElementFields {
    /// True for the elements whose descriptor has an `axis_direction` (section 3.9): line edges,
    /// and cylinder, cone and torus faces.
    fn has_axis_direction(&self) -> bool {
        self.curve == Some(CurveKind::Line)
            || matches!(
                self.surface,
                Some(SurfaceKind::Cylinder | SurfaceKind::Cone | SurfaceKind::Torus)
            )
    }
}

/// The geometry fields of an element of `kind` with kernel geometry `geometry`.
pub(crate) fn element_fields(kind: ElementKind, geometry: &ElementGeometry) -> ElementFields {
    let centroid = [
        geometry.centroid.x,
        geometry.centroid.y,
        geometry.centroid.z,
    ];
    let direction = geometry
        .direction
        .map(|direction| [direction.x, direction.y, direction.z]);
    let axis = match (geometry.location, direction) {
        (Some(location), Some(direction)) => Some(Axis {
            origin: [location.x, location.y, location.z],
            direction,
        }),
        _ => None,
    };
    let mut fields = ElementFields {
        centroid,
        ..ElementFields::default()
    };
    match kind {
        ElementKind::Face => {
            fields.surface = Some(surface_kind(geometry.kind));
            fields.area = Some(geometry.measure);
            match geometry.kind {
                GeometryKind::Plane => {
                    fields.normal = direction;
                    fields.placement = direction.and_then(|normal| {
                        Placement::on_plane(centroid, normal, &OriginPlane::Xy.placement()).ok()
                    });
                }
                GeometryKind::Cylinder => {
                    fields.axis = axis;
                    fields.radius = geometry.radius;
                }
                GeometryKind::Cone | GeometryKind::Torus => fields.axis = axis,
                GeometryKind::Sphere => fields.radius = geometry.radius,
                _ => {}
            }
        }
        ElementKind::Edge => {
            fields.curve = Some(curve_kind(geometry.kind));
            fields.length = Some(geometry.measure);
            match geometry.kind {
                GeometryKind::Line => fields.direction = direction,
                GeometryKind::Circle => {
                    fields.axis = axis;
                    fields.radius = geometry.radius;
                }
                _ => {}
            }
        }
        ElementKind::Vertex => {}
    }
    fields
}

/// The protocol surface type of a kernel surface kind.
fn surface_kind(kind: GeometryKind) -> SurfaceKind {
    match kind {
        GeometryKind::Plane => SurfaceKind::Plane,
        GeometryKind::Cylinder => SurfaceKind::Cylinder,
        GeometryKind::Cone => SurfaceKind::Cone,
        GeometryKind::Sphere => SurfaceKind::Sphere,
        GeometryKind::Torus => SurfaceKind::Torus,
        GeometryKind::BsplineSurface => SurfaceKind::Bspline,
        _ => SurfaceKind::Other,
    }
}

/// The protocol curve type of a kernel curve kind.
fn curve_kind(kind: GeometryKind) -> CurveKind {
    match kind {
        GeometryKind::Line => CurveKind::Line,
        GeometryKind::Circle => CurveKind::Circle,
        GeometryKind::Ellipse => CurveKind::Ellipse,
        GeometryKind::BsplineCurve => CurveKind::Bspline,
        _ => CurveKind::Other,
    }
}

/// The kernel sub-shape kind of an element kind.
pub(crate) fn sub_shape_kind(kind: ElementKind) -> SubShapeKind {
    match kind {
        ElementKind::Face => SubShapeKind::Face,
        ElementKind::Edge => SubShapeKind::Edge,
        ElementKind::Vertex => SubShapeKind::Vertex,
    }
}

/// The feature that created a face (section 3.9): the feature of the last `@<feature>:<n>`
/// instance suffix of its name when it has one, else the feature id that starts its name.
pub(crate) fn face_feature(name: &str) -> Option<&str> {
    let mut depth = 0_usize;
    let mut last_instance = None;
    for (position, character) in name.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            '@' if depth == 0 => last_instance = Some(position),
            _ => {}
        }
    }
    match last_instance {
        Some(position) => name[position + 1..].split(':').next(),
        None => name.split('/').next(),
    }
}

/// Builds the descriptors of every element of `table`, in table order (faces, edges, vertices).
/// `timeline` gives each feature's position, for the feature of an edge or vertex: the latest
/// among its faces' features. The kernel is asked for the geometry of each kind once; names,
/// adjacency and references come from the table and its topology.
pub(crate) fn describe(
    table: &ElementTable,
    state: &BodyState,
    body: &Id,
    timeline: &BTreeMap<Id, usize>,
) -> Result<Vec<ElementDescriptor>, EngineError> {
    let shape = shape_of(state)?;
    let describer = Describer::for_every_face(table, state, body, timeline);
    let kinds = [ElementKind::Face, ElementKind::Edge, ElementKind::Vertex];
    let count = kinds.iter().map(|&kind| table.entries(kind).len()).sum();
    let mut descriptors = Vec::with_capacity(count);
    for kind in kinds {
        let geometries = shape
            .elements(sub_shape_kind(kind))
            .map_err(EngineError::unexpected)?;
        for entry in table.entries(kind) {
            let geometry = slot(entry.index)
                .and_then(|position| geometries.get(position))
                .ok_or_else(|| EngineError::internal("a displayed element has no geometry"))?;
            descriptors.push(describer.descriptor(kind, entry, geometry)?);
        }
    }
    Ok(descriptors)
}

/// Builds the descriptor of the displayed element of `kind` with display id `display_id`, equal
/// to its entry in `describe`. It queries the kernel for that element only and reads only the
/// topology around it.
pub(crate) fn describe_element(
    table: &ElementTable,
    state: &BodyState,
    body: &Id,
    timeline: &BTreeMap<Id, usize>,
    kind: ElementKind,
    display_id: u32,
) -> Result<ElementDescriptor, EngineError> {
    let entry = usize::try_from(display_id)
        .ok()
        .and_then(|position| table.entries(kind).get(position))
        .ok_or_else(|| EngineError::internal("a display id names no displayed element"))?;
    let geometry = shape_of(state)?
        .element(sub_shape_kind(kind), entry.index)
        .map_err(EngineError::unexpected)?;
    Describer::for_one_element(table, state, body, timeline).descriptor(kind, entry, &geometry)
}

/// The full reference a pick of the element named `name` stores: `Reference::from_element`
/// against the element set of its state, with anchors and merged names (section 3.9). `None`
/// when the set has no such element.
pub(crate) fn full_reference(set: &ElementSet, name: &ElementName) -> Option<Reference> {
    set.element(name)
        .map(|info| Reference::from_element(info, set))
}

/// The shape of a body state that has an element table.
fn shape_of(state: &BodyState) -> Result<&partsmith_occt_sys::Shape, EngineError> {
    state
        .shape()
        .ok_or_else(|| EngineError::internal("descriptors need a shape"))
}

/// The feature that created a face, with its timeline position.
#[derive(Debug, Clone)]
struct FaceFeature {
    /// The feature id.
    feature: Id,
    /// Its position in the timeline, `None` when the timeline does not have it.
    position: Option<usize>,
}

/// What building the descriptors of one body state reads.
struct Describer<'a> {
    /// The body state's element table, with its topology.
    table: &'a ElementTable,
    /// The body state, whose naming map records how far its faces moved.
    state: &'a BodyState,
    /// The body.
    body: &'a Id,
    /// The timeline position of every feature.
    timeline: &'a BTreeMap<Id, usize>,
    /// The feature of each face by display id, when every descriptor is built; empty when one
    /// element is described, which reads the features of its own faces only.
    face_features: Vec<Option<FaceFeature>>,
    /// The motion that undoes how far each face moved, by display id (`None` for a face that did
    /// not move), when every descriptor is built, so the edges of a face reuse it; empty when one
    /// element is described.
    face_undoings: Vec<Option<Motion>>,
}

impl<'a> Describer<'a> {
    /// A describer for every element, which finds the feature and the motion of each face once.
    fn for_every_face(
        table: &'a ElementTable,
        state: &'a BodyState,
        body: &'a Id,
        timeline: &'a BTreeMap<Id, usize>,
    ) -> Describer<'a> {
        let mut describer = Describer::for_one_element(table, state, body, timeline);
        let faces = table.entries(ElementKind::Face);
        describer.face_features = faces
            .iter()
            .map(|entry| describer.face_feature_of(&entry.name))
            .collect();
        describer.face_undoings = faces
            .iter()
            .map(|entry| undoing(&state.motion(&entry.name)))
            .collect();
        describer
    }

    /// A describer for single elements.
    fn for_one_element(
        table: &'a ElementTable,
        state: &'a BodyState,
        body: &'a Id,
        timeline: &'a BTreeMap<Id, usize>,
    ) -> Describer<'a> {
        Describer {
            table,
            state,
            body,
            timeline,
            face_features: Vec::new(),
            face_undoings: Vec::new(),
        }
    }

    /// The descriptor of one displayed element from its kernel geometry; its reference is its
    /// name with its fingerprint at this state, and its `fallback` and `axis_direction` are in
    /// body coordinates (section 3.9).
    fn descriptor(
        &self,
        kind: ElementKind,
        entry: &TableEntry,
        geometry: &ElementGeometry,
    ) -> Result<ElementDescriptor, EngineError> {
        let (faces, adjacent) = self
            .adjacency(kind, entry.index)
            .ok_or_else(|| EngineError::internal("a displayed element has no topology"))?;
        let feature = match kind {
            ElementKind::Face => self.face_feature(entry.index).map(|found| found.feature),
            ElementKind::Edge | ElementKind::Vertex => self.feature_of_faces(faces),
        }
        .ok_or_else(|| EngineError::internal("an element name names no feature"))?;
        let fingerprint = self
            .table
            .topology()
            .fingerprint(kind, entry.index)
            .map_err(EngineError::unexpected)?;
        let fields = element_fields(kind, geometry);
        let (fallback, axis_direction) =
            self.body_frames(kind, entry, faces, &fields, fingerprint.direction);
        let reference =
            Reference::new(entry.name.clone(), fingerprint).map_err(EngineError::unexpected)?;
        Ok(ElementDescriptor {
            name: entry.name.as_str().to_owned(),
            kind,
            body: self.body.clone(),
            feature,
            reference,
            surface: fields.surface,
            curve: fields.curve,
            centroid: fields.centroid,
            area: fields.area,
            length: fields.length,
            normal: fields.normal,
            direction: fields.direction,
            axis: fields.axis,
            radius: fields.radius,
            placement: fields.placement,
            fallback,
            axis_direction,
            adjacent,
        })
    }

    /// The `fallback` of a flat face and the `axis_direction` of a line edge or a cylinder, cone
    /// or torus face (section 3.9): its placement and its fingerprint's `direction` in body
    /// coordinates, the motion of the element undone. `faces` are the kernel indices of the
    /// faces of an edge.
    fn body_frames(
        &self,
        kind: ElementKind,
        entry: &TableEntry,
        faces: &[u32],
        fields: &ElementFields,
        direction: Option<[f64; 3]>,
    ) -> (Option<Placement>, Option<Vec3>) {
        let axis = direction.filter(|_| fields.has_axis_direction());
        if fields.placement.is_none() && axis.is_none() {
            return (None, None);
        }
        let Some(undo) = self.undoing_of(kind, entry, faces) else {
            return (fields.placement, axis);
        };
        (
            fields.placement.map(|placement| placement.moved(&undo)),
            axis.map(|axis| undo.apply_direction(axis)),
        )
    }

    /// The motion that undoes how far an element moved (section 3.9), `None` when it did not
    /// move: a face's own, and for an edge or vertex that of its first face term. Edge and vertex
    /// names list the names of their adjacent faces in byte order (naming.md section 3.3, step 4)
    /// and display ids follow that order, so the first term is the adjacent face with the
    /// smallest display id; `faces` are the kernel indices of those faces.
    fn undoing_of(&self, kind: ElementKind, entry: &TableEntry, faces: &[u32]) -> Option<Motion> {
        let face = match kind {
            ElementKind::Face => self.table.display_id(ElementKind::Face, entry.index),
            ElementKind::Edge | ElementKind::Vertex => faces
                .iter()
                .filter_map(|&face| self.table.display_id(ElementKind::Face, face))
                .min(),
        };
        if let Some(undo) = face.and_then(|id| self.face_undoings.get(id as usize)) {
            return *undo;
        }
        let name = face
            .and_then(|id| self.table.entries(ElementKind::Face).get(id as usize))
            .map_or(&entry.name, |first| &first.name);
        undoing(&self.state.motion(name))
    }

    /// The names of the displayed elements of a kind among some kernel indices, sorted and
    /// distinct. Display ids follow the name order, so the ids are sorted instead of the names.
    fn names(&self, kind: ElementKind, indices: &[u32]) -> Vec<String> {
        let mut ids: Vec<u32> = indices
            .iter()
            .filter_map(|&index| self.table.display_id(kind, index))
            .collect();
        ids.sort_unstable();
        ids.dedup();
        let entries = self.table.entries(kind);
        ids.iter()
            .filter_map(|&id| entries.get(id as usize))
            .map(|entry| entry.name.as_str().to_owned())
            .collect()
    }

    /// The feature of the face with kernel index `face` and its timeline position.
    fn face_feature(&self, face: u32) -> Option<FaceFeature> {
        let id = self.table.display_id(ElementKind::Face, face)?;
        match self.face_features.get(id as usize) {
            Some(found) => found.clone(),
            None => {
                let entry = self.table.entries(ElementKind::Face).get(id as usize)?;
                self.face_feature_of(&entry.name)
            }
        }
    }

    /// The feature a face name names (`face_feature`) and its timeline position.
    fn face_feature_of(&self, name: &ElementName) -> Option<FaceFeature> {
        let feature = Id::new(face_feature(name.as_str())?).ok()?;
        let position = self.timeline.get(&feature).copied();
        Some(FaceFeature { feature, position })
    }

    /// The latest feature in timeline order among the features of some faces (kernel indices).
    fn feature_of_faces(&self, faces: &[u32]) -> Option<Id> {
        faces
            .iter()
            .filter_map(|&face| self.face_feature(face))
            .max_by_key(|found| found.position)
            .map(|found| found.feature)
    }

    /// The faces (kernel indices) an edge or vertex takes its feature from, and the adjacent
    /// names of a descriptor (section 3.9): faces list their displayed edges; edges their faces
    /// then their displayed vertices; vertices their displayed edges. A face takes its feature
    /// from its own name, so its list of faces is empty.
    fn adjacency(&self, kind: ElementKind, index: u32) -> Option<(&[u32], Vec<String>)> {
        let topology = self.table.topology();
        let position = slot(index)?;
        match kind {
            ElementKind::Face => {
                let face = topology.faces.get(position)?;
                Some((&[], self.names(ElementKind::Edge, &face.edges)))
            }
            ElementKind::Edge => {
                let edge = topology.edges.get(position)?;
                let mut adjacent = self.names(ElementKind::Face, &edge.faces);
                adjacent.extend(self.names(ElementKind::Vertex, &edge.vertices));
                Some((&edge.faces, adjacent))
            }
            ElementKind::Vertex => {
                let vertex = topology.vertices.get(position)?;
                Some((&vertex.faces, self.names(ElementKind::Edge, &vertex.edges)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use partsmith_occt_sys::{Point, Vector};

    use super::*;

    fn geometry(kind: GeometryKind) -> ElementGeometry {
        ElementGeometry {
            kind,
            direction: Some(Vector::new(0.0, 0.0, 1.0)),
            location: Some(Point::new(1.0, 2.0, 3.0)),
            radius: Some(5.0),
            centroid: Point::new(0.0, 0.0, 40.0),
            measure: 600.0,
        }
    }

    #[test]
    fn instance_suffixes_name_the_copying_feature() {
        assert_eq!(face_feature("f-box1/top"), Some("f-box1"));
        assert_eq!(face_feature("f-box1/top@f-pat1:2"), Some("f-pat1"));
        assert_eq!(
            face_feature("f-box1/top@f-pat1:2@f-mir1:1#2"),
            Some("f-mir1")
        );
        assert_eq!(
            face_feature("f-fil1/fillet/edge(f-box1/front@f-pat1:1|f-box1/left)"),
            Some("f-fil1")
        );
    }

    #[test]
    fn planar_faces_have_a_normal_and_a_placement() {
        let fields = element_fields(ElementKind::Face, &geometry(GeometryKind::Plane));

        assert_eq!(fields.surface, Some(SurfaceKind::Plane));
        assert_eq!(fields.area, Some(600.0));
        assert_eq!(fields.normal, Some([0.0, 0.0, 1.0]));
        let placement = fields.placement.expect("a planar face has a placement");
        assert_eq!(placement.origin, [0.0, 0.0, 40.0]);
        assert_eq!(placement.x_axis, [1.0, 0.0, 0.0]);
        assert_eq!(fields.radius, None);
        assert_eq!(fields.axis, None);
    }

    #[test]
    fn cylinders_have_an_axis_and_a_radius() {
        let fields = element_fields(ElementKind::Face, &geometry(GeometryKind::Cylinder));

        assert_eq!(fields.surface, Some(SurfaceKind::Cylinder));
        assert_eq!(fields.radius, Some(5.0));
        assert_eq!(
            fields.axis,
            Some(Axis {
                origin: [1.0, 2.0, 3.0],
                direction: [0.0, 0.0, 1.0]
            })
        );
        assert_eq!(fields.normal, None);
    }

    #[test]
    fn tori_have_an_axis_but_no_radius() {
        let fields = element_fields(ElementKind::Face, &geometry(GeometryKind::Torus));

        assert!(fields.axis.is_some());
        assert_eq!(fields.radius, None);
    }

    #[test]
    fn line_edges_have_a_direction_and_a_length() {
        let fields = element_fields(ElementKind::Edge, &geometry(GeometryKind::Line));

        assert_eq!(fields.curve, Some(CurveKind::Line));
        assert_eq!(fields.length, Some(600.0));
        assert_eq!(fields.direction, Some([0.0, 0.0, 1.0]));
        assert_eq!(fields.area, None);
    }

    #[test]
    fn circle_edges_have_an_axis_and_a_radius() {
        let fields = element_fields(ElementKind::Edge, &geometry(GeometryKind::Circle));

        assert_eq!(fields.curve, Some(CurveKind::Circle));
        assert_eq!(fields.radius, Some(5.0));
        assert!(fields.axis.is_some());
        assert_eq!(fields.direction, None);
    }

    #[test]
    fn vertices_have_only_a_centroid() {
        let fields = element_fields(ElementKind::Vertex, &geometry(GeometryKind::Point));

        assert_eq!(
            fields,
            ElementFields {
                centroid: [0.0, 0.0, 40.0],
                ..ElementFields::default()
            }
        );
    }

    #[test]
    fn only_line_edges_and_cylinder_cone_and_torus_faces_have_an_axis_direction() {
        let has = |kind, geometry_kind| {
            element_fields(kind, &geometry(geometry_kind)).has_axis_direction()
        };

        assert!(has(ElementKind::Edge, GeometryKind::Line));
        assert!(has(ElementKind::Face, GeometryKind::Cylinder));
        assert!(has(ElementKind::Face, GeometryKind::Cone));
        assert!(has(ElementKind::Face, GeometryKind::Torus));
        assert!(!has(ElementKind::Edge, GeometryKind::Circle));
        assert!(!has(ElementKind::Face, GeometryKind::Plane));
        assert!(!has(ElementKind::Face, GeometryKind::Sphere));
        assert!(!has(ElementKind::Vertex, GeometryKind::Point));
    }

    #[test]
    fn other_kinds_map_to_other() {
        assert_eq!(surface_kind(GeometryKind::Revolution), SurfaceKind::Other);
        assert_eq!(curve_kind(GeometryKind::Hyperbola), CurveKind::Other);
        assert_eq!(
            surface_kind(GeometryKind::BsplineSurface),
            SurfaceKind::Bspline
        );
    }
}
