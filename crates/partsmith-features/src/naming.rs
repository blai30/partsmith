//! Naming kernel shapes: topology and fingerprints for `partsmith_naming`, and the history rules
//! shared by features (features.md section 3.9, naming.md sections 3.3, 3.6 and 3.7).

use std::sync::Arc;

use partsmith_model::Id;
use partsmith_naming::{
    EdgeInfo, ElementKind, ElementName, FaceInfo, FaceSource, Fingerprint, GeometryKind, Motion,
    NamingMap, Successions, TopologyInfo, VertexInfo, roles,
};
use partsmith_occt_sys::{
    ElementGeometry, GeometryKind as KernelGeometryKind, OpResult, Shape, SubShapeKind,
};

use crate::error::FeatureError;

/// Topology and fingerprints of every face, edge and vertex of `shape`, in the form
/// `NamingMap::build` and `NamingMap::element_infos` take (naming.md section 3.7).
pub fn topology_info(shape: &Shape) -> Result<TopologyInfo, FeatureError> {
    let topology = shape.topology()?;
    let faces = shape.elements(SubShapeKind::Face)?;
    let edges = shape.elements(SubShapeKind::Edge)?;
    let vertices = shape.elements(SubShapeKind::Vertex)?;
    if faces.len() != topology.faces.len()
        || edges.len() != topology.edges.len()
        || vertices.len() != topology.vertices.len()
    {
        return Err(FeatureError::internal(
            "the kernel's element geometry and topology disagree on the element counts",
        ));
    }
    Ok(TopologyInfo {
        faces: topology
            .faces
            .into_iter()
            .zip(&faces)
            .map(|(face, geometry)| {
                Ok(FaceInfo {
                    edges: face.edges,
                    fingerprint: fingerprint(geometry, ElementKind::Face)?,
                })
            })
            .collect::<Result<_, FeatureError>>()?,
        edges: topology
            .edges
            .into_iter()
            .zip(&edges)
            .map(|(edge, geometry)| {
                Ok(EdgeInfo {
                    faces: edge.faces,
                    vertices: edge.vertices,
                    degenerate: edge.degenerate,
                    fingerprint: fingerprint(geometry, ElementKind::Edge)?,
                })
            })
            .collect::<Result<_, FeatureError>>()?,
        vertices: topology
            .vertices
            .into_iter()
            .zip(&vertices)
            .map(|(vertex, geometry)| {
                Ok(VertexInfo {
                    edges: vertex.edges,
                    faces: vertex.faces,
                    fingerprint: fingerprint(geometry, ElementKind::Vertex)?,
                })
            })
            .collect::<Result<_, FeatureError>>()?,
    })
}

/// A naming map with the topology and fingerprints it was built from, which a feature passes on
/// in `Measurements::topology` so the body's next state does not read them from the kernel again
/// (features.md section 2.7).
#[derive(Clone, Debug)]
pub struct Named {
    /// Names of every face, edge and vertex of the shape.
    pub naming: NamingMap,
    /// Topology and fingerprints of the shape (`topology_info`).
    pub topology: Arc<TopologyInfo>,
}

/// Names every element of `shape` from one source per face, in face index order (naming.md
/// section 3.3). `inherited` are the naming maps of every input shape, the body the feature writes
/// first; each `FaceSource::From` source names its map by its index there.
pub fn name_shape(
    shape: &Shape,
    faces: Vec<FaceSource>,
    inherited: &[&NamingMap],
) -> Result<Named, FeatureError> {
    let topology = topology_info(shape)?;
    let naming = NamingMap::build(
        faces,
        &topology,
        inherited,
        &Successions::default(),
        &Motion::IDENTITY,
    )
    .map_err(naming_failed)?;
    Ok(Named {
        naming,
        topology: Arc::new(topology),
    })
}

/// For each face of `result.shape`, in index order, the input faces whose `modified` list contains
/// it (naming.md `modified_sources`), in input order, each as the index of its input in `inputs`
/// and its name there. `inputs` names the operation's history inputs in order.
pub fn modified_sources(
    result: &OpResult,
    inputs: &[&NamingMap],
) -> Result<Vec<Vec<(u32, ElementName)>>, FeatureError> {
    if inputs.len() != result.history.inputs.len() {
        return Err(FeatureError::internal(format!(
            "the kernel history has {} inputs but {} naming maps were given",
            result.history.inputs.len(),
            inputs.len()
        )));
    }
    let face_count = result.shape.count(SubShapeKind::Face) as usize;
    let mut sources = vec![Vec::new(); face_count];
    for ((map, history), naming) in (0..).zip(&result.history.inputs).zip(inputs) {
        for (input_index, fate) in (1..).zip(&history.faces) {
            let name = naming.name(ElementKind::Face, input_index).ok_or_else(|| {
                FeatureError::internal(format!(
                    "the kernel history names input face {input_index}, which the naming map does not have"
                ))
            })?;
            for &output in &fate.modified {
                let names = output
                    .checked_sub(1)
                    .and_then(|position| sources.get_mut(position as usize))
                    .ok_or_else(|| {
                        FeatureError::internal(format!(
                            "the kernel history names output face {output} of a shape with {face_count} faces"
                        ))
                    })?;
                names.push((map, name.clone()));
            }
        }
    }
    Ok(sources)
}

/// Names a kernel result by the boolean rule (naming.md section 3.6): each face descends from the
/// input faces whose `modified` lists contain it, and a face without one gets the fallback name
/// `<feature>/face`. `inputs` names the operation's history inputs in order; their aliases are
/// inherited.
pub fn name_by_history(
    feature: &Id,
    result: &OpResult,
    inputs: &[&NamingMap],
) -> Result<Named, FeatureError> {
    let fallback = ElementName::generated(feature, roles::FACE, None).map_err(naming_failed)?;
    let faces = modified_sources(result, inputs)?
        .into_iter()
        .map(|names| {
            if names.is_empty() {
                FaceSource::New(fallback.clone())
            } else {
                FaceSource::From(names)
            }
        })
        .collect();
    name_shape(&result.shape, faces, inputs)
}

/// True when `shape` has exactly these numbers of faces, edges and vertices, compared in that
/// order.
pub(crate) fn has_element_counts(
    shape: &Shape,
    faces: usize,
    edges: usize,
    vertices: usize,
) -> bool {
    [
        (faces, SubShapeKind::Face),
        (edges, SubShapeKind::Edge),
        (vertices, SubShapeKind::Vertex),
    ]
    .into_iter()
    .all(|(entries, kind)| u32::try_from(entries).is_ok_and(|entries| entries == shape.count(kind)))
}

/// The kernel sub-shape kind of an element kind.
pub(crate) fn sub_shape_kind(kind: ElementKind) -> SubShapeKind {
    match kind {
        ElementKind::Face => SubShapeKind::Face,
        ElementKind::Edge => SubShapeKind::Edge,
        ElementKind::Vertex => SubShapeKind::Vertex,
    }
}

/// A naming failure, which valid kernel output cannot cause, as an internal error.
pub(crate) fn naming_failed(error: partsmith_naming::NamingError) -> FeatureError {
    FeatureError::internal(format!("naming the result failed: {error}"))
}

/// The fingerprint of one element from its kernel geometry (naming.md section 3.7.1): faces and
/// edges carry their area or length, vertices only their position.
fn fingerprint(geometry: &ElementGeometry, kind: ElementKind) -> Result<Fingerprint, FeatureError> {
    let size = (kind != ElementKind::Vertex).then_some(geometry.measure);
    Fingerprint::new(
        geometry_kind(geometry.kind),
        geometry
            .direction
            .map(|direction| [direction.x, direction.y, direction.z]),
        [
            geometry.centroid.x,
            geometry.centroid.y,
            geometry.centroid.z,
        ],
        size,
        geometry.radius,
    )
    .map_err(naming_failed)
}

/// The naming crate's geometry kind of a kernel geometry kind; the two enums mirror each other.
fn geometry_kind(kind: KernelGeometryKind) -> GeometryKind {
    match kind {
        KernelGeometryKind::Plane => GeometryKind::Plane,
        KernelGeometryKind::Cylinder => GeometryKind::Cylinder,
        KernelGeometryKind::Cone => GeometryKind::Cone,
        KernelGeometryKind::Sphere => GeometryKind::Sphere,
        KernelGeometryKind::Torus => GeometryKind::Torus,
        KernelGeometryKind::Revolution => GeometryKind::Revolution,
        KernelGeometryKind::Extrusion => GeometryKind::Extrusion,
        KernelGeometryKind::BsplineSurface => GeometryKind::BsplineSurface,
        KernelGeometryKind::BezierSurface => GeometryKind::BezierSurface,
        KernelGeometryKind::OffsetSurface => GeometryKind::OffsetSurface,
        KernelGeometryKind::OtherSurface => GeometryKind::OtherSurface,
        KernelGeometryKind::Line => GeometryKind::Line,
        KernelGeometryKind::Circle => GeometryKind::Circle,
        KernelGeometryKind::Ellipse => GeometryKind::Ellipse,
        KernelGeometryKind::Hyperbola => GeometryKind::Hyperbola,
        KernelGeometryKind::Parabola => GeometryKind::Parabola,
        KernelGeometryKind::BsplineCurve => GeometryKind::BsplineCurve,
        KernelGeometryKind::BezierCurve => GeometryKind::BezierCurve,
        KernelGeometryKind::OffsetCurve => GeometryKind::OffsetCurve,
        KernelGeometryKind::OtherCurve => GeometryKind::OtherCurve,
        KernelGeometryKind::Point => GeometryKind::Point,
    }
}

#[cfg(test)]
mod tests {
    use partsmith_naming::ElementInfo;
    use partsmith_occt_sys::fuse;

    use super::*;
    use crate::test_support::{face_names, id, named_box};

    fn infos(shape: &Shape, naming: &NamingMap) -> Vec<ElementInfo> {
        naming
            .element_infos(&topology_info(shape).expect("topology"))
            .expect("element infos")
    }

    #[test]
    fn box_topology_has_six_faces_twelve_edges_and_eight_vertices() {
        let block = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);

        let topology = topology_info(&block.shape).expect("the topology is read");

        assert_eq!(topology.faces.len(), 6);
        assert_eq!(topology.edges.len(), 12);
        assert_eq!(topology.vertices.len(), 8);
    }

    #[test]
    fn box_face_fingerprints_follow_naming_md() {
        let block = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);

        let top = infos(&block.shape, &block.naming)
            .into_iter()
            .find(|info| info.name.as_str() == "f-box1/top")
            .expect("the box has a top face");

        assert_eq!(
            top.fingerprint.to_string(),
            "plane|n=0,0,1|c=5,5,10|a=100|adj=4,4"
        );
    }

    #[test]
    fn box_edges_are_named_by_their_faces() {
        let block = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);

        let edge = infos(&block.shape, &block.naming)
            .into_iter()
            .find(|info| info.name.as_str() == "edge(f-box1/front|f-box1/top)")
            .expect("the box has a top front edge");

        assert_eq!(
            edge.fingerprint.to_string(),
            "line|n=1,0,0|c=5,0,10|l=10|adj=2,2"
        );
    }

    #[test]
    fn box_corners_have_three_faces_and_no_size() {
        let block = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);

        let corner = infos(&block.shape, &block.naming)
            .into_iter()
            .find(|info| info.kind == ElementKind::Vertex)
            .expect("the box has vertices");

        assert!(corner.fingerprint.to_string().starts_with("point|c="));
        assert!(corner.fingerprint.to_string().ends_with("|adj=3,3"));
    }

    #[test]
    fn fused_disjoint_boxes_keep_every_face_name() {
        let first = named_box("f-a", [0.0, 0.0, 0.0], 10.0);
        let second = named_box("f-b", [20.0, 0.0, 0.0], 10.0);
        let result = fuse(&first.shape, std::slice::from_ref(&second.shape)).expect("fuses");

        let naming = name_by_history(&id("f-c"), &result, &[&first.naming, &second.naming])
            .expect("the result is named")
            .naming;

        let names = face_names(&naming);
        assert_eq!(names.len(), 12);
        assert!(names.contains(&"f-a/back".to_string()));
        assert!(names.contains(&"f-b/top".to_string()));
    }

    #[test]
    fn fused_touching_boxes_merge_their_tops_with_an_alias() {
        let first = named_box("f-a", [0.0, 0.0, 0.0], 10.0);
        let second = named_box("f-b", [10.0, 0.0, 0.0], 10.0);
        let result = fuse(&first.shape, std::slice::from_ref(&second.shape)).expect("fuses");

        let naming = name_by_history(&id("f-c"), &result, &[&first.naming, &second.naming])
            .expect("the result is named")
            .naming;

        let top = ElementName::parse("f-a/top").expect("valid name");
        let merged = ElementName::parse("f-b/top").expect("valid name");
        assert!(naming.index(&top).is_some());
        assert_eq!(naming.aliases().chase(&merged), [top]);
        assert!(!face_names(&naming).contains(&"f-c/face".to_string()));
    }

    #[test]
    fn modified_sources_list_each_output_face_once_per_input() {
        let first = named_box("f-a", [0.0, 0.0, 0.0], 10.0);
        let second = named_box("f-b", [20.0, 0.0, 0.0], 10.0);
        let result = fuse(&first.shape, std::slice::from_ref(&second.shape)).expect("fuses");

        let sources = modified_sources(&result, &[&first.naming, &second.naming]).expect("sources");

        assert_eq!(sources.len(), 12);
        assert!(sources.iter().all(|names| names.len() == 1));
    }

    #[test]
    fn history_with_the_wrong_number_of_maps_is_internal() {
        let first = named_box("f-a", [0.0, 0.0, 0.0], 10.0);
        let second = named_box("f-b", [20.0, 0.0, 0.0], 10.0);
        let result = fuse(&first.shape, std::slice::from_ref(&second.shape)).expect("fuses");

        let error = modified_sources(&result, &[&first.naming]).expect_err("one map is missing");

        assert_eq!(error.code(), "internal");
    }

    #[test]
    fn element_kinds_map_to_kernel_kinds() {
        assert_eq!(sub_shape_kind(ElementKind::Face), SubShapeKind::Face);
        assert_eq!(sub_shape_kind(ElementKind::Edge), SubShapeKind::Edge);
        assert_eq!(sub_shape_kind(ElementKind::Vertex), SubShapeKind::Vertex);
    }
}
