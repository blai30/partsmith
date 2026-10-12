//! Naming kernel shapes: topology and fingerprints for `partsmith_naming`, and the history rules
//! shared by features (features.md section 3.9, naming.md sections 3.3, 3.6 and 3.7).

use partsmith_model::Id;
use partsmith_naming::{
    Aliases, EdgeInfo, ElementKind, ElementName, FaceInfo, FaceSource, Fingerprint, GeometryKind,
    NamingMap, TopologyInfo, VertexInfo, roles,
};
use partsmith_occt_sys::{
    ElementGeometry, GeometryKind as KernelGeometryKind, OpResult, Shape, SubShapeKind,
};

use crate::error::FeatureError;

/// Topology and fingerprints of every face, edge and vertex of `shape`, in the form
/// `NamingMap::build` and `NamingMap::element_infos` take (naming.md section 3.7).
pub fn topology_info(shape: &Shape) -> Result<TopologyInfo, FeatureError> {
    let topology = shape.topology().map_err(kernel)?;
    let faces = shape.elements(SubShapeKind::Face).map_err(kernel)?;
    let edges = shape.elements(SubShapeKind::Edge).map_err(kernel)?;
    let vertices = shape.elements(SubShapeKind::Vertex).map_err(kernel)?;
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

/// Names every element of `shape` from one source per face, in face index order (naming.md
/// section 3.3). `inherited_aliases` are the aliases of every input naming map.
pub fn name_shape(
    shape: &Shape,
    faces: Vec<FaceSource>,
    inherited_aliases: &[&Aliases],
) -> Result<NamingMap, FeatureError> {
    let topology = topology_info(shape)?;
    NamingMap::build(faces, &topology, inherited_aliases).map_err(naming)
}

/// For each face of `result.shape`, in index order, the names of the input faces whose `modified`
/// list contains it (naming.md `modified_sources`), in input order. `inputs` names the
/// operation's history inputs in order.
pub fn modified_sources(
    result: &OpResult,
    inputs: &[&NamingMap],
) -> Result<Vec<Vec<ElementName>>, FeatureError> {
    if inputs.len() != result.history.inputs.len() {
        return Err(FeatureError::internal(format!(
            "the kernel history has {} inputs but {} naming maps were given",
            result.history.inputs.len(),
            inputs.len()
        )));
    }
    let face_count = result.shape.count(SubShapeKind::Face) as usize;
    let mut sources = vec![Vec::new(); face_count];
    for (history, naming) in result.history.inputs.iter().zip(inputs) {
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
                names.push(name.clone());
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
) -> Result<NamingMap, FeatureError> {
    let fallback = ElementName::generated(feature, roles::FACE, None).map_err(naming)?;
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
    let aliases: Vec<&Aliases> = inputs.iter().map(|naming| naming.aliases()).collect();
    name_shape(&result.shape, faces, &aliases)
}

/// The kernel sub-shape kind of an element kind.
pub(crate) fn sub_shape_kind(kind: ElementKind) -> SubShapeKind {
    match kind {
        ElementKind::Face => SubShapeKind::Face,
        ElementKind::Edge => SubShapeKind::Edge,
        ElementKind::Vertex => SubShapeKind::Vertex,
    }
}

/// A kernel failure as a feature error.
pub(crate) fn kernel(error: partsmith_occt_sys::KernelError) -> FeatureError {
    FeatureError::Kernel { error }
}

/// A naming failure, which valid kernel output cannot cause, as an internal error.
pub(crate) fn naming(error: partsmith_naming::NamingError) -> FeatureError {
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
    .map_err(naming)
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
            .expect("the result is named");

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
            .expect("the result is named");

        let top = ElementName::parse("f-a/top").expect("valid name");
        let merged = ElementName::parse("f-b/top").expect("valid name");
        assert!(naming.index(&top).is_some());
        assert_eq!(naming.aliases().chase(&merged), top);
        assert!(!face_names(&naming).contains(&"f-c/face".to_string()));
    }

    #[test]
    fn modified_sources_list_each_output_face_once_per_input() {
        let first = named_box("f-a", [0.0, 0.0, 0.0], 10.0);
        let second = named_box("f-b", [20.0, 0.0, 0.0], 10.0);
        let result = fuse(&first.shape, std::slice::from_ref(&second.shape)).expect("fuses");

        let sources =
            modified_sources(&result, &[&first.naming, &second.naming]).expect("sources");

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
