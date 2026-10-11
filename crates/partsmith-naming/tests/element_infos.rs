//! Acceptance tests for element infos (spec section 6, tests 56 to 58).

mod common;

use common::{
    BOX_EDGE_NAMES, BOX_FACE_NAMES, BOX_VERTEX_NAMES, box_map, box_scope, box_topology,
    cylinder_map, cylinder_topology, fingerprint, info_named, name,
};
use partsmith_naming::{ElementInfo, ElementKind, NamingError};
use serde_json::json;

/// The CYL element infos.
fn cylinder_scope() -> Vec<ElementInfo> {
    cylinder_map()
        .element_infos(&cylinder_topology())
        .unwrap_or_else(|error| panic!("the CYL map matches the CYL topology, got {error:?}"))
}

/// The adjacency counts of the info named `text`.
fn adjacency_counts(scope: &[ElementInfo], text: &str) -> Option<[u32; 2]> {
    info_named(scope, text).fingerprint.adjacency
}

#[test]
fn t056_box_scope_lists_faces_edges_then_vertices_in_index_order() {
    let expected: Vec<(ElementKind, &str)> = BOX_FACE_NAMES
        .iter()
        .map(|text| (ElementKind::Face, *text))
        .chain(BOX_EDGE_NAMES.iter().map(|text| (ElementKind::Edge, *text)))
        .chain(
            BOX_VERTEX_NAMES
                .iter()
                .map(|text| (ElementKind::Vertex, *text)),
        )
        .collect();

    let scope = box_scope();
    let actual: Vec<(ElementKind, &str)> = scope
        .iter()
        .map(|info| (info.kind, info.name.as_str()))
        .collect();

    assert_eq!(actual.len(), 26);
    assert_eq!(actual, expected);
}

#[test]
fn t056_top_face_info_serializes_to_spec_json() {
    let scope = box_scope();

    let value =
        serde_json::to_value(info_named(&scope, "f-box1/top")).expect("an element info serializes");

    assert_eq!(
        value,
        json!({
            "name": "f-box1/top",
            "kind": "face",
            "fingerprint": "plane|n=0,0,1|c=5,10,30|a=200|adj=4,4",
            "adjacency": ["f-box1/back", "f-box1/front", "f-box1/left", "f-box1/right"]
        })
    );
}

#[test]
fn t056_spec_json_deserializes_to_top_face_info() {
    let scope = box_scope();
    let text = r#"{"name": "f-box1/top", "kind": "face", "fingerprint": "plane|n=0,0,1|c=5,10,30|a=200|adj=4,4", "adjacency": ["f-box1/back", "f-box1/front", "f-box1/left", "f-box1/right"]}"#;

    let parsed: ElementInfo = serde_json::from_str(text).expect("the spec example deserializes");

    assert_eq!(&parsed, info_named(&scope, "f-box1/top"));
}

#[test]
fn t056_edge_fingerprint_and_adjacency() {
    let scope = box_scope();

    let edge = info_named(&scope, "edge(f-box1/front|f-box1/top)");

    assert_eq!(
        edge.fingerprint,
        fingerprint("line|n=1,0,0|c=5,0,30|l=10|adj=2,2")
    );
    assert_eq!(
        edge.adjacency,
        vec![name("f-box1/front"), name("f-box1/top")]
    );
}

#[test]
fn t056_vertex_fingerprint() {
    let scope = box_scope();

    let vertex = info_named(&scope, "vertex(f-box1/back|f-box1/right|f-box1/top)");

    assert_eq!(vertex.fingerprint, fingerprint("point|c=10,20,30|adj=3,3"));
}

#[test]
fn t057_cylinder_face_adjacency_counts() {
    let scope = cylinder_scope();

    assert_eq!(adjacency_counts(&scope, "f-cyl1/side"), Some([2, 3]));
    assert_eq!(adjacency_counts(&scope, "f-cyl1/top"), Some([1, 1]));
}

#[test]
fn t057_cylinder_edge_adjacency_counts() {
    let scope = cylinder_scope();

    assert_eq!(
        adjacency_counts(&scope, "edge(f-cyl1/side|f-cyl1/top)"),
        Some([2, 1])
    );
    assert_eq!(adjacency_counts(&scope, "edge(f-cyl1/side)"), Some([1, 2]));
}

#[test]
fn t057_cylinder_vertex_adjacency_counts() {
    let scope = cylinder_scope();

    assert_eq!(
        adjacency_counts(&scope, "vertex(f-cyl1/side|f-cyl1/top)"),
        Some([2, 2])
    );
    assert_eq!(
        adjacency_counts(&scope, "vertex(f-cyl1/bottom|f-cyl1/side)"),
        Some([2, 2])
    );
}

#[test]
fn t057_seam_adjacency_is_its_one_face() {
    let scope = cylinder_scope();

    assert_eq!(
        info_named(&scope, "edge(f-cyl1/side)").adjacency,
        vec![name("f-cyl1/side")]
    );
}

#[test]
fn t058_topology_with_other_counts_fails() {
    // BOX topology without the top face (face 6), with face 6 removed from every list so the
    // topology itself stays valid.
    let mut topology = box_topology();
    topology.faces.pop();
    for edge in &mut topology.edges {
        edge.faces.retain(|face| *face != 6);
    }
    for vertex in &mut topology.vertices {
        vertex.faces.retain(|face| *face != 6);
    }

    let result = box_map().element_infos(&topology);

    assert_eq!(result, Err(NamingError::TopologyMismatch));
}
