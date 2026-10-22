//! Acceptance tests for element infos and element sets (spec section 6, tests 56 to 58 and 124).

use crate::common;

use std::sync::Arc;

use common::{
    BOX_EDGE_NAMES, BOX_FACE_NAMES, BOX_VERTEX_NAMES, aliases, box_infos, box_map, box_topology,
    cylinder_map, cylinder_topology, fingerprint, info_named, names, scope_element, successions,
};
use partsmith_naming::{Aliases, ElementInfo, ElementKind, ElementSet, NamingError, Successions};
use serde_json::json;

/// The CYL element infos.
fn cylinder_infos() -> Vec<ElementInfo> {
    cylinder_map()
        .element_infos(&cylinder_topology())
        .unwrap_or_else(|error| panic!("the CYL map matches the CYL topology, got {error:?}"))
}

/// The adjacency counts of the info named `text`.
fn adjacency_counts(infos: &[ElementInfo], text: &str) -> Option<[u32; 2]> {
    info_named(infos, text).fingerprint.adjacency
}

/// The kind and name of every element of a set, in its order.
fn kinds_and_names(set: &ElementSet) -> Vec<(ElementKind, &str)> {
    set.elements()
        .iter()
        .map(|info| (info.kind, info.name.as_str()))
        .collect()
}

#[test]
fn t056_box_infos_list_faces_edges_then_vertices_in_index_order() {
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

    let infos = box_infos();
    let actual: Vec<(ElementKind, &str)> = infos
        .iter()
        .map(|info| (info.kind, info.name.as_str()))
        .collect();

    assert_eq!(actual.len(), 26);
    assert_eq!(actual, expected);
}

#[test]
fn t056_top_face_info_serializes_to_spec_json() {
    let infos = box_infos();

    let value =
        serde_json::to_value(info_named(&infos, "f-box1/top")).expect("an element info serializes");

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
    let infos = box_infos();
    let text = r#"{"name": "f-box1/top", "kind": "face", "fingerprint": "plane|n=0,0,1|c=5,10,30|a=200|adj=4,4", "adjacency": ["f-box1/back", "f-box1/front", "f-box1/left", "f-box1/right"]}"#;

    let parsed: ElementInfo = serde_json::from_str(text).expect("the spec example deserializes");

    assert_eq!(&parsed, info_named(&infos, "f-box1/top"));
}

#[test]
fn t056_edge_fingerprint_and_adjacency() {
    let infos = box_infos();

    let edge = info_named(&infos, "edge(f-box1/front|f-box1/top)");

    assert_eq!(
        edge.fingerprint,
        fingerprint("line|n=1,0,0|c=5,0,30|l=10|adj=2,2")
    );
    assert_eq!(edge.adjacency, names(&["f-box1/front", "f-box1/top"]));
}

#[test]
fn t056_edge_lists_its_named_vertices() {
    let infos = box_infos();

    let edge = info_named(&infos, "edge(f-box1/front|f-box1/top)");

    assert_eq!(
        edge.vertices,
        names(&[
            "vertex(f-box1/front|f-box1/left|f-box1/top)",
            "vertex(f-box1/front|f-box1/right|f-box1/top)",
        ])
    );
}

#[test]
fn t056_faces_and_vertices_have_no_vertices() {
    for info in box_infos() {
        if info.kind != ElementKind::Edge {
            assert!(info.vertices.is_empty(), "{info:?}");
        }
    }
}

#[test]
fn t056_vertex_fingerprint() {
    let infos = box_infos();

    let vertex = info_named(&infos, "vertex(f-box1/back|f-box1/right|f-box1/top)");

    assert_eq!(vertex.fingerprint, fingerprint("point|c=10,20,30|adj=3,3"));
}

#[test]
fn t056_edge_info_serializes_its_vertices() {
    let infos = box_infos();

    let value = serde_json::to_value(info_named(&infos, "edge(f-box1/front|f-box1/top)"))
        .expect("an element info serializes");

    assert_eq!(
        value,
        json!({
            "name": "edge(f-box1/front|f-box1/top)",
            "kind": "edge",
            "fingerprint": "line|n=1,0,0|c=5,0,30|l=10|adj=2,2",
            "adjacency": ["f-box1/front", "f-box1/top"],
            "vertices": [
                "vertex(f-box1/front|f-box1/left|f-box1/top)",
                "vertex(f-box1/front|f-box1/right|f-box1/top)"
            ]
        })
    );
}

#[test]
fn t056_edge_info_without_vertices_key_reads_none() {
    let text = r#"{"name": "edge(f-a/x|f-b/y)", "kind": "edge", "fingerprint": "line|n=1,0,0|c=0,0,0|l=1|adj=2,2", "adjacency": ["f-a/x", "f-b/y"]}"#;

    let parsed: ElementInfo = serde_json::from_str(text).expect("an info without vertices reads");

    assert!(parsed.vertices.is_empty());
}

#[test]
fn t057_cylinder_face_adjacency_counts() {
    let infos = cylinder_infos();

    assert_eq!(adjacency_counts(&infos, "f-cyl1/side"), Some([2, 3]));
    assert_eq!(adjacency_counts(&infos, "f-cyl1/top"), Some([1, 1]));
}

#[test]
fn t057_cylinder_edge_adjacency_counts() {
    let infos = cylinder_infos();

    assert_eq!(
        adjacency_counts(&infos, "edge(f-cyl1/side|f-cyl1/top)"),
        Some([2, 1])
    );
    assert_eq!(adjacency_counts(&infos, "edge(f-cyl1/side)"), Some([1, 2]));
}

#[test]
fn t057_cylinder_vertex_adjacency_counts() {
    let infos = cylinder_infos();

    assert_eq!(
        adjacency_counts(&infos, "vertex(f-cyl1/side|f-cyl1/top)"),
        Some([2, 2])
    );
    assert_eq!(
        adjacency_counts(&infos, "vertex(f-cyl1/bottom|f-cyl1/side)"),
        Some([2, 2])
    );
}

#[test]
fn t057_seam_adjacency_is_its_one_face() {
    let infos = cylinder_infos();

    assert_eq!(
        info_named(&infos, "edge(f-cyl1/side)").adjacency,
        names(&["f-cyl1/side"])
    );
}

#[test]
fn t057_seam_lists_both_of_its_vertices() {
    let infos = cylinder_infos();

    assert_eq!(
        info_named(&infos, "edge(f-cyl1/side)").vertices,
        names(&[
            "vertex(f-cyl1/bottom|f-cyl1/side)",
            "vertex(f-cyl1/side|f-cyl1/top)",
        ])
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

#[test]
fn t124_element_set_rejects_two_faces_with_one_name() {
    let result = ElementSet::new(
        Arc::from(vec![scope_element("f-a/x"), scope_element("f-a/x")]),
        Aliases::default(),
        Successions::default(),
    );

    assert_eq!(
        result.err(),
        Some(NamingError::DuplicateName {
            name: "f-a/x".to_owned()
        })
    );
}

#[test]
fn t124_element_set_lists_faces_then_edges_then_vertices() {
    let set = ElementSet::new(
        Arc::from(vec![
            scope_element("edge(f-a/x|f-b/y)"),
            scope_element("vertex(f-a/x|f-b/y|f-c/z)"),
            scope_element("f-a/x"),
        ]),
        Aliases::default(),
        Successions::default(),
    )
    .expect("the names are distinct");

    assert_eq!(
        kinds_and_names(&set),
        [
            (ElementKind::Face, "f-a/x"),
            (ElementKind::Edge, "edge(f-a/x|f-b/y)"),
            (ElementKind::Vertex, "vertex(f-a/x|f-b/y|f-c/z)"),
        ]
    );
}

#[test]
fn t124_element_set_keeps_the_order_within_each_kind() {
    let set = ElementSet::new(
        Arc::from(vec![
            scope_element("edge(f-b/y|f-c/z)"),
            scope_element("f-b/y"),
            scope_element("edge(f-a/x|f-b/y)"),
            scope_element("f-a/x"),
        ]),
        Aliases::default(),
        Successions::default(),
    )
    .expect("the names are distinct");

    assert_eq!(
        kinds_and_names(&set),
        [
            (ElementKind::Face, "f-b/y"),
            (ElementKind::Face, "f-a/x"),
            (ElementKind::Edge, "edge(f-b/y|f-c/z)"),
            (ElementKind::Edge, "edge(f-a/x|f-b/y)"),
        ]
    );
}

#[test]
fn t124_element_set_of_box_infos_keeps_every_element() {
    let set = ElementSet::new(
        Arc::from(box_infos()),
        Aliases::default(),
        Successions::default(),
    )
    .expect("the BOX names are distinct");

    assert_eq!(set.elements(), box_infos().as_slice());
}

#[test]
fn t124_element_set_names_keep_the_aliases_and_successions() {
    let merged = aliases(&[("f-ext2/end-cap", &["f-ext1/end-cap"])]);
    let replaced = successions(&[("f-ext1/lateral/e-3", &["f-ext1/lateral/e-9"])]);

    let set = ElementSet::new(
        Arc::from(vec![scope_element("f-ext1/end-cap")]),
        merged.clone(),
        replaced.clone(),
    )
    .expect("one face");

    assert_eq!(set.names().aliases(), &merged);
    assert_eq!(set.names().successions(), &replaced);
}
