//! Element descriptors and their geometry (engine.md sections 3.9 and 3.11).

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::BodyState;
use partsmith_model::Id;
use partsmith_naming::{ElementInfo, ElementKind, ElementName, NamingMap, Reference};
use partsmith_occt_sys::{ElementGeometry, GeometryKind, SubShapeKind};
use partsmith_protocol::{Axis, CurveKind, ElementDescriptor, SurfaceKind, Vec3};
use partsmith_sketch::{OriginPlane, Placement};

use crate::display::{ElementTable, TableEntry, kernel_failure, slot};
use crate::error::EngineError;

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
/// among its faces' features.
pub(crate) fn describe(
    table: &ElementTable,
    state: &BodyState,
    body: &Id,
    timeline: &BTreeMap<Id, usize>,
) -> Result<Vec<ElementDescriptor>, EngineError> {
    let shape = shape_of(state)?;
    let infos = element_infos(state)?;
    // The first info of each name, as `describe_element` finds it.
    let mut by_name: BTreeMap<&ElementName, &ElementInfo> = BTreeMap::new();
    for info in infos.iter() {
        by_name.entry(&info.name).or_insert(info);
    }
    let describer = Describer {
        table,
        naming: state.naming(),
        body,
        timeline,
    };
    let mut descriptors = Vec::new();
    for kind in [ElementKind::Face, ElementKind::Edge, ElementKind::Vertex] {
        let geometries = shape
            .elements(sub_shape_kind(kind))
            .map_err(kernel_failure)?;
        for entry in table.entries(kind) {
            let geometry = slot(entry.index)
                .and_then(|position| geometries.get(position))
                .ok_or_else(|| EngineError::internal("a displayed element has no geometry"))?;
            let info = by_name
                .get(&entry.name)
                .ok_or_else(|| EngineError::internal("a displayed element has no element info"))?;
            descriptors.push(describer.descriptor(kind, entry, geometry, info)?);
        }
    }
    Ok(descriptors)
}

/// Builds the descriptor of the displayed element of `kind` with display id `display_id`, equal
/// to its entry in `describe`. It queries the kernel for that element only.
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
        .map_err(kernel_failure)?;
    let infos = element_infos(state)?;
    let info = infos
        .iter()
        .find(|info| info.name == entry.name)
        .ok_or_else(|| EngineError::internal("a displayed element has no element info"))?;
    let describer = Describer {
        table,
        naming: state.naming(),
        body,
        timeline,
    };
    describer.descriptor(kind, entry, &geometry, info)
}

/// The shape of a body state that has an element table.
fn shape_of(state: &BodyState) -> Result<&partsmith_occt_sys::Shape, EngineError> {
    state
        .shape()
        .ok_or_else(|| EngineError::internal("descriptors need a shape"))
}

/// The element infos of a body state, which the state computes once and shares.
fn element_infos(state: &BodyState) -> Result<Arc<[ElementInfo]>, EngineError> {
    state
        .elements()
        .map_err(|error| EngineError::internal(error.to_string()))
}

/// What building the descriptors of one body state reads.
struct Describer<'a> {
    /// The body state's element table, with its kernel topology.
    table: &'a ElementTable,
    /// The body state's names.
    naming: &'a NamingMap,
    /// The body.
    body: &'a Id,
    /// The timeline position of every feature.
    timeline: &'a BTreeMap<Id, usize>,
}

impl Describer<'_> {
    /// The descriptor of one displayed element from its kernel geometry and element info.
    fn descriptor(
        &self,
        kind: ElementKind,
        entry: &TableEntry,
        geometry: &ElementGeometry,
        info: &ElementInfo,
    ) -> Result<ElementDescriptor, EngineError> {
        let (faces, adjacent) = self
            .adjacency(kind, entry.index)
            .ok_or_else(|| EngineError::internal("a displayed element has no topology"))?;
        let feature = match kind {
            ElementKind::Face => {
                face_feature(entry.name.as_str()).and_then(|feature| Id::new(feature).ok())
            }
            ElementKind::Edge | ElementKind::Vertex => self.feature_of_faces(&faces),
        }
        .ok_or_else(|| EngineError::internal("an element name names no feature"))?;
        let fields = element_fields(kind, geometry);
        Ok(ElementDescriptor {
            name: entry.name.to_string(),
            kind,
            body: self.body.clone(),
            feature,
            reference: Reference::from_info(info),
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
            adjacent,
        })
    }

    /// The sorted, distinct names of some kernel indices of a kind, only displayed ones when
    /// `displayed` is true.
    fn names(&self, kind: ElementKind, indices: &[u32], displayed: bool) -> Vec<String> {
        let mut names: Vec<String> = indices
            .iter()
            .filter(|&&index| !displayed || self.table.display_id(kind, index).is_some())
            .filter_map(|&index| self.naming.name(kind, index))
            .map(ToString::to_string)
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// The latest feature in timeline order among the features of some faces (kernel indices).
    fn feature_of_faces(&self, faces: &[u32]) -> Option<Id> {
        faces
            .iter()
            .filter_map(|&face| self.naming.name(ElementKind::Face, face))
            .filter_map(|name| face_feature(name.as_str()))
            .filter_map(|feature| Id::new(feature).ok())
            .max_by_key(|feature| self.timeline.get(feature).copied())
    }

    /// The adjacent faces (kernel indices) and the adjacent names of a descriptor (section 3.9):
    /// faces list their displayed edges; edges their faces then their displayed vertices;
    /// vertices their displayed edges.
    fn adjacency(&self, kind: ElementKind, index: u32) -> Option<(Vec<u32>, Vec<String>)> {
        let topology = self.table.topology();
        let position = slot(index)?;
        match kind {
            ElementKind::Face => {
                let face = topology.faces.get(position)?;
                Some((vec![index], self.names(ElementKind::Edge, &face.edges, true)))
            }
            ElementKind::Edge => {
                let edge = topology.edges.get(position)?;
                let mut adjacent = self.names(ElementKind::Face, &edge.faces, false);
                adjacent.extend(self.names(ElementKind::Vertex, &edge.vertices, true));
                Some((edge.faces.clone(), adjacent))
            }
            ElementKind::Vertex => {
                let vertex = topology.vertices.get(position)?;
                Some((
                    vertex.faces.clone(),
                    self.names(ElementKind::Edge, &vertex.edges, true),
                ))
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
    fn other_kinds_map_to_other() {
        assert_eq!(surface_kind(GeometryKind::Revolution), SurfaceKind::Other);
        assert_eq!(curve_kind(GeometryKind::Hyperbola), CurveKind::Other);
        assert_eq!(
            surface_kind(GeometryKind::BsplineSurface),
            SurfaceKind::Bspline
        );
    }
}
