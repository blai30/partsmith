//! Acceptance tests for building, renaming, projecting and serializing naming maps (spec
//! section 6, tests 33 to 55).

mod common;

use common::{
    BOX_EDGE_NAMES, BOX_FACE_NAMES, BOX_VERTEX_NAMES, aliases, box_map, box_sources, box_topology,
    build, build_with_aliases, cylinder_map, degenerate_edge_info, edge_info, face_info,
    faces_only_topology, from_sources, id, name, names_of, new_source, sphere_map, vertex_info,
};
use partsmith_naming::{
    Aliases, ElementKind, FaceInfo, FaceSource, Fingerprint, GeometryKind, Instance, NamingError,
    NamingMap, TopologyInfo,
};
use serde_json::json;

/// The instance `(1, 0)`, used by mirrors and the first pattern copy.
const FIRST_INSTANCE: Instance = Instance {
    first: 1,
    second: 0,
};

/// Test 37: two pieces of one end cap, face 1 at x = 65 and face 2 at x = 15.
fn split_end_cap_map() -> NamingMap {
    let topology = faces_only_topology(&[
        "plane|n=0,0,1|c=65,25,30|a=1500",
        "plane|n=0,0,1|c=15,25,30|a=1500",
    ]);
    build(
        vec![
            from_sources(&["f-ext1/end-cap"]),
            from_sources(&["f-ext1/end-cap"]),
        ],
        &topology,
    )
}

/// Two pieces of `f-a/x` with the given fingerprints.
fn two_pieces_map(first: &str, second: &str) -> NamingMap {
    build(
        vec![from_sources(&["f-a/x"]), from_sources(&["f-a/x"])],
        &faces_only_topology(&[first, second]),
    )
}

/// Test 38: equal centroids, face 1 with area 10 and face 2 with area 5.
fn equal_centroid_map() -> NamingMap {
    two_pieces_map("plane|n=0,0,1|c=0,0,0|a=10", "plane|n=0,0,1|c=0,0,0|a=5")
}

/// Test 38: equal centroids and equal areas.
fn equal_centroid_and_area_map() -> NamingMap {
    two_pieces_map("plane|n=0,0,1|c=0,0,0|a=5", "plane|n=0,0,1|c=0,0,0|a=5")
}

/// A plane fingerprint at `(x, 0, 0)` with the given area, not rounded to text precision.
fn unrounded_plane(x: f64, size: f64) -> Fingerprint {
    Fingerprint {
        kind: GeometryKind::Plane,
        direction: Some([0.0, 0.0, 1.0]),
        centroid: Some([x, 0.0, 0.0]),
        size: Some(size),
        radius: None,
        adjacency: None,
    }
}

/// Test 39: face 1 at x = 1.0000004 with size 1, face 2 at x = 1.0000001 with size 2. Unquantized,
/// face 2 would come first by x; quantized, x is equal and the smaller size (face 1) comes first.
fn quantization_map() -> NamingMap {
    let topology = TopologyInfo {
        faces: vec![
            FaceInfo {
                edges: Vec::new(),
                fingerprint: unrounded_plane(1.0000004, 1.0),
            },
            FaceInfo {
                edges: Vec::new(),
                fingerprint: unrounded_plane(1.0000001, 2.0),
            },
        ],
        edges: Vec::new(),
        vertices: Vec::new(),
    };
    build(
        vec![from_sources(&["f-a/x"]), from_sources(&["f-a/x"])],
        &topology,
    )
}

/// A one-face topology.
fn one_face_topology() -> TopologyInfo {
    faces_only_topology(&["plane|n=0,0,1|c=0,0,0|a=1"])
}

/// Test 40: one face from an inherited piece.
fn inherited_suffix_map() -> NamingMap {
    build(
        vec![from_sources(&["f-ext1/end-cap#2"])],
        &one_face_topology(),
    )
}

/// Test 41: face 1 from `f-a/x#1`, faces 2 and 3 new `f-a/x`, at x = 0, 1 and 2.
fn second_pass_map() -> NamingMap {
    let topology = faces_only_topology(&[
        "plane|n=0,0,1|c=0,0,0|a=1",
        "plane|n=0,0,1|c=1,0,0|a=1",
        "plane|n=0,0,1|c=2,0,0|a=1",
    ]);
    build(
        vec![
            from_sources(&["f-a/x#1"]),
            new_source("f-a/x"),
            new_source("f-a/x"),
        ],
        &topology,
    )
}

/// Test 42: one face merged from two end caps.
fn merge_map() -> NamingMap {
    build(
        vec![from_sources(&["f-ext2/end-cap", "f-ext1/end-cap"])],
        &one_face_topology(),
    )
}

/// Test 43: two pieces of one face rejoined.
fn rejoin_map() -> NamingMap {
    build(
        vec![from_sources(&["f-a/x#1", "f-a/x#2"])],
        &one_face_topology(),
    )
}

/// Test 44: pieces of two different faces merged.
fn merge_of_pieces_map() -> NamingMap {
    build(
        vec![from_sources(&["f-b/y#2", "f-a/x#1"])],
        &one_face_topology(),
    )
}

/// Test 45: an inherited alias to a face that merges into another.
fn alias_carry_map() -> NamingMap {
    let inherited = aliases(&[("f-c/z", "f-b/y")]);
    build_with_aliases(
        vec![from_sources(&["f-b/y", "f-a/x"])],
        &one_face_topology(),
        &[&inherited],
    )
}

/// Test 46: an inherited alias whose target no output face has as split base.
fn alias_without_target_map() -> NamingMap {
    let inherited = aliases(&[("f-c/z", "f-b/y")]);
    build_with_aliases(
        vec![from_sources(&["f-a/x"])],
        &one_face_topology(),
        &[&inherited],
    )
}

/// Test 46: an inherited alias whose key is the split base of an output face.
fn alias_with_live_key_map() -> NamingMap {
    let inherited = aliases(&[("f-c/z", "f-a/x")]);
    build_with_aliases(
        vec![from_sources(&["f-a/x"]), from_sources(&["f-c/z#1"])],
        &faces_only_topology(&["plane|n=0,0,1|c=0,0,0|a=1", "plane|n=0,0,1|c=1,0,0|a=1"]),
        &[&inherited],
    )
}

/// Test 48: face 1 with a degenerate edge 1, a free edge 2 without faces, vertex 1 on face 1 and
/// a free vertex 2 without faces.
fn unnamed_elements_map() -> NamingMap {
    let topology = TopologyInfo {
        faces: vec![face_info(&[1], "plane|n=0,0,1|c=0,0,0|a=1")],
        edges: vec![
            degenerate_edge_info(&[1], &[1], "other_curve|c=0,0,0|l=0"),
            edge_info(&[], &[2], "line|n=1,0,0|c=5,5,5|l=1"),
        ],
        vertices: vec![
            vertex_info(&[1], &[1], "point|c=0,0,0"),
            vertex_info(&[2], &[], "point|c=5,5,5"),
        ],
    };
    build(vec![new_source("f-a/x")], &topology)
}

/// Every map built by tests 33 to 46.
fn maps_of_tests_33_to_46() -> Vec<NamingMap> {
    vec![
        box_map(),
        cylinder_map(),
        sphere_map(),
        split_end_cap_map(),
        equal_centroid_map(),
        equal_centroid_and_area_map(),
        quantization_map(),
        inherited_suffix_map(),
        second_pass_map(),
        merge_map(),
        rejoin_map(),
        merge_of_pieces_map(),
        alias_carry_map(),
        alias_without_target_map(),
        alias_with_live_key_map(),
    ]
}

#[test]
fn t033_box_faces_are_named_as_sources() {
    assert_eq!(
        names_of(&box_map(), ElementKind::Face),
        BOX_FACE_NAMES.map(Some)
    );
}

#[test]
fn t033_box_edges_are_named_by_adjacent_faces() {
    assert_eq!(
        names_of(&box_map(), ElementKind::Edge),
        BOX_EDGE_NAMES.map(Some)
    );
}

#[test]
fn t033_box_vertices_are_named_by_adjacent_faces() {
    assert_eq!(
        names_of(&box_map(), ElementKind::Vertex),
        BOX_VERTEX_NAMES.map(Some)
    );
}

#[test]
fn t034_index_finds_face() {
    assert_eq!(box_map().index(&name("f-box1/top")), Some(6));
}

#[test]
fn t034_index_finds_edge() {
    assert_eq!(
        box_map().index(&name("edge(f-box1/front|f-box1/top)")),
        Some(2)
    );
}

#[test]
fn t034_index_of_unknown_name_is_none() {
    assert_eq!(box_map().index(&name("f-box1/nothing")), None);
}

#[test]
fn t034_name_out_of_range_is_none() {
    assert_eq!(box_map().name(ElementKind::Face, 7), None);
}

#[test]
fn t034_count_counts_edges() {
    assert_eq!(box_map().count(ElementKind::Edge), 12);
}

#[test]
fn t035_cylinder_names() {
    let map = cylinder_map();

    assert_eq!(
        names_of(&map, ElementKind::Face),
        [
            Some("f-cyl1/side"),
            Some("f-cyl1/top"),
            Some("f-cyl1/bottom")
        ]
    );
    assert_eq!(
        names_of(&map, ElementKind::Edge),
        [
            Some("edge(f-cyl1/side|f-cyl1/top)"),
            Some("edge(f-cyl1/side)"),
            Some("edge(f-cyl1/bottom|f-cyl1/side)"),
        ]
    );
    assert_eq!(
        names_of(&map, ElementKind::Vertex),
        [
            Some("vertex(f-cyl1/side|f-cyl1/top)"),
            Some("vertex(f-cyl1/bottom|f-cyl1/side)"),
        ]
    );
}

#[test]
fn t035_cylinder_map_serializes_to_spec_json() {
    let value = serde_json::to_value(cylinder_map()).expect("a naming map serializes");

    assert_eq!(
        value,
        json!({
            "faces": ["f-cyl1/side", "f-cyl1/top", "f-cyl1/bottom"],
            "edges": ["edge(f-cyl1/side|f-cyl1/top)", "edge(f-cyl1/side)", "edge(f-cyl1/bottom|f-cyl1/side)"],
            "vertices": ["vertex(f-cyl1/side|f-cyl1/top)", "vertex(f-cyl1/bottom|f-cyl1/side)"]
        })
    );
}

#[test]
fn t036_sphere_degenerate_edges_are_unnamed() {
    assert_eq!(
        names_of(&sphere_map(), ElementKind::Edge),
        [None, Some("edge(f-sph1/surface)"), None]
    );
}

#[test]
fn t036_sphere_poles_are_numbered_by_geometric_order() {
    assert_eq!(
        names_of(&sphere_map(), ElementKind::Vertex),
        [
            Some("vertex(f-sph1/surface)#2"),
            Some("vertex(f-sph1/surface)#1"),
        ]
    );
}

#[test]
fn t037_split_pieces_are_numbered_by_centroid_x() {
    assert_eq!(
        names_of(&split_end_cap_map(), ElementKind::Face),
        [Some("f-ext1/end-cap#2"), Some("f-ext1/end-cap#1")]
    );
}

#[test]
fn t038_equal_centroids_are_ordered_by_size() {
    assert_eq!(
        names_of(&equal_centroid_map(), ElementKind::Face),
        [Some("f-a/x#2"), Some("f-a/x#1")]
    );
}

#[test]
fn t038_equal_centroids_and_sizes_are_ordered_by_index() {
    assert_eq!(
        names_of(&equal_centroid_and_area_map(), ElementKind::Face),
        [Some("f-a/x#1"), Some("f-a/x#2")]
    );
}

#[test]
fn t039_centroids_are_quantized_so_size_decides() {
    assert_eq!(
        names_of(&quantization_map(), ElementKind::Face),
        [Some("f-a/x#1"), Some("f-a/x#2")]
    );
}

#[test]
fn t040_inherited_suffix_is_kept() {
    assert_eq!(
        names_of(&inherited_suffix_map(), ElementKind::Face),
        [Some("f-ext1/end-cap#2")]
    );
}

#[test]
fn t041_second_pass_resolves_collision_with_inherited_name() {
    assert_eq!(
        names_of(&second_pass_map(), ElementKind::Face),
        [Some("f-a/x#1#1"), Some("f-a/x#1#2"), Some("f-a/x#2")]
    );
}

#[test]
fn t042_merge_keeps_smallest_name() {
    assert_eq!(
        names_of(&merge_map(), ElementKind::Face),
        [Some("f-ext1/end-cap")]
    );
}

#[test]
fn t042_merge_records_alias() {
    assert_eq!(
        merge_map().aliases(),
        &aliases(&[("f-ext2/end-cap", "f-ext1/end-cap")])
    );
}

#[test]
fn t043_rejoined_pieces_take_split_base() {
    assert_eq!(names_of(&rejoin_map(), ElementKind::Face), [Some("f-a/x")]);
}

#[test]
fn t043_rejoin_records_no_alias() {
    assert!(rejoin_map().aliases().is_empty());
}

#[test]
fn t044_merge_of_pieces_keeps_smallest_piece_name() {
    assert_eq!(
        names_of(&merge_of_pieces_map(), ElementKind::Face),
        [Some("f-a/x#1")]
    );
}

#[test]
fn t044_merge_of_pieces_aliases_split_bases() {
    assert_eq!(
        merge_of_pieces_map().aliases(),
        &aliases(&[("f-b/y", "f-a/x")])
    );
}

#[test]
fn t045_inherited_aliases_are_carried_and_flattened() {
    assert_eq!(
        alias_carry_map().aliases(),
        &aliases(&[("f-b/y", "f-a/x"), ("f-c/z", "f-a/x")])
    );
}

#[test]
fn t046_alias_to_absent_target_is_pruned() {
    assert_eq!(alias_without_target_map().aliases(), &Aliases::default());
}

#[test]
fn t046_alias_from_live_name_is_pruned() {
    assert_eq!(alias_with_live_key_map().aliases(), &Aliases::default());
}

#[test]
fn t047_build_fails_on_face_count_mismatch() {
    let mut sources = box_sources();
    sources.pop();

    let result = NamingMap::build(sources, &box_topology(), &[]);

    assert_eq!(
        result,
        Err(NamingError::FaceCountMismatch {
            sources: 5,
            faces: 6
        })
    );
}

#[test]
fn t047_build_fails_on_empty_sources() {
    let result = NamingMap::build(
        vec![FaceSource::From(Vec::new())],
        &one_face_topology(),
        &[],
    );

    assert_eq!(result, Err(NamingError::EmptySources { face: 1 }));
}

#[test]
fn t047_build_fails_on_face_index_out_of_range() {
    let mut topology = box_topology();
    topology.edges[0].faces = vec![3, 7];

    let result = NamingMap::build(box_sources(), &topology, &[]);

    assert_eq!(
        result,
        Err(NamingError::IndexOutOfRange {
            kind: ElementKind::Face,
            index: 7,
            count: 6
        })
    );
}

#[test]
fn t047_build_fails_on_unsorted_indices() {
    let mut topology = box_topology();
    topology.edges[0].faces = vec![5, 3];

    let result = NamingMap::build(box_sources(), &topology, &[]);

    assert_eq!(
        result,
        Err(NamingError::UnsortedIndices {
            kind: ElementKind::Edge,
            index: 1
        })
    );
}

#[test]
fn t047_build_fails_on_missing_centroid_when_ordering() {
    let topology = faces_only_topology(&["plane|n=0,0,1|c=0,0,0|a=1", "plane|n=0,0,1|a=1"]);

    let result = NamingMap::build(
        vec![from_sources(&["f-a/x"]), from_sources(&["f-a/x"])],
        &topology,
        &[],
    );

    assert_eq!(
        result,
        Err(NamingError::MissingCentroid {
            kind: ElementKind::Face,
            index: 2
        })
    );
}

#[test]
fn t048_degenerate_edge_is_unnamed() {
    assert_eq!(unnamed_elements_map().name(ElementKind::Edge, 1), None);
}

#[test]
fn t048_edge_without_faces_is_unnamed() {
    assert_eq!(unnamed_elements_map().name(ElementKind::Edge, 2), None);
}

#[test]
fn t048_vertex_without_faces_is_unnamed() {
    let map = unnamed_elements_map();

    assert_eq!(
        names_of(&map, ElementKind::Vertex),
        [Some("vertex(f-a/x)"), None]
    );
}

#[test]
fn t049_non_manifold_edge_names_all_three_faces() {
    let topology = TopologyInfo {
        faces: vec![
            face_info(&[1], "plane|n=0,0,1|c=0,0,0|a=1"),
            face_info(&[1], "plane|n=0,1,0|c=0,0,0|a=1"),
            face_info(&[1], "plane|n=1,0,0|c=0,0,0|a=1"),
        ],
        edges: vec![edge_info(&[1, 2, 3], &[], "line|n=1,0,0|c=0,0,0|l=1")],
        vertices: Vec::new(),
    };

    let map = build(
        vec![
            new_source("f-c/z"),
            new_source("f-a/x"),
            new_source("f-b/y"),
        ],
        &topology,
    );

    assert_eq!(
        names_of(&map, ElementKind::Edge),
        [Some("edge(f-a/x|f-b/y|f-c/z)")]
    );
}

#[test]
fn t050_edges_sharing_face_pair_are_numbered_by_centroid() {
    let topology = TopologyInfo {
        faces: vec![
            face_info(&[1, 2], "plane|n=0,0,1|c=2,0,0|a=1"),
            face_info(&[1, 2], "plane|n=0,1,0|c=2,0,0|a=1"),
        ],
        edges: vec![
            edge_info(&[1, 2], &[], "line|n=1,0,0|c=3,0,0|l=1"),
            edge_info(&[1, 2], &[], "line|n=1,0,0|c=1,0,0|l=1"),
        ],
        vertices: Vec::new(),
    };

    let map = build(vec![new_source("f-a/x"), new_source("f-b/y")], &topology);

    assert_eq!(
        names_of(&map, ElementKind::Edge),
        [Some("edge(f-a/x|f-b/y)#2"), Some("edge(f-a/x|f-b/y)#1")]
    );
}

#[test]
fn t051_building_twice_gives_equal_maps() {
    assert_eq!(box_map(), box_map());
    assert_eq!(second_pass_map(), second_pass_map());
    assert_eq!(alias_carry_map(), alias_carry_map());
}

#[test]
fn t051_building_twice_gives_identical_json() {
    for (first, second) in maps_of_tests_33_to_46()
        .iter()
        .zip(maps_of_tests_33_to_46().iter())
    {
        let first_json = serde_json::to_string(first).expect("a naming map serializes");
        let second_json = serde_json::to_string(second).expect("a naming map serializes");
        assert_eq!(first_json, second_json);
    }
}

/// Faces `f-a/x` and `f-b/y` built with inherited aliases in the given order.
fn map_with_inherited(inherited: &[&Aliases]) -> NamingMap {
    build_with_aliases(
        vec![new_source("f-a/x"), new_source("f-b/y")],
        &faces_only_topology(&["plane|n=0,0,1|c=0,0,0|a=1", "plane|n=0,0,1|c=1,0,0|a=1"]),
        inherited,
    )
}

#[test]
fn t051_order_of_inherited_aliases_does_not_matter() {
    let first = aliases(&[("f-c/z", "f-b/y")]);
    let second = aliases(&[("f-c/z", "f-a/x")]);
    let third = aliases(&[("f-d/w", "f-b/y")]);

    let forward = map_with_inherited(&[&first, &second, &third]);
    let backward = map_with_inherited(&[&third, &second, &first]);

    assert_eq!(forward, backward);
}

#[test]
fn t051_inherited_key_conflict_keeps_smallest_target() {
    let first = aliases(&[("f-c/z", "f-b/y")]);
    let second = aliases(&[("f-c/z", "f-a/x")]);
    let third = aliases(&[("f-d/w", "f-b/y")]);

    let map = map_with_inherited(&[&first, &second, &third]);

    assert_eq!(
        map.aliases(),
        &aliases(&[("f-c/z", "f-a/x"), ("f-d/w", "f-b/y")])
    );
}

#[test]
fn t052_with_instance_renames_faces() {
    let copy = box_map()
        .with_instance(&id("f-pat1"), FIRST_INSTANCE)
        .expect("the BOX map has an instance copy");

    assert_eq!(
        copy.name(ElementKind::Face, 6),
        Some(&name("f-box1/top@f-pat1:1"))
    );
}

#[test]
fn t052_with_instance_renames_edges() {
    let copy = box_map()
        .with_instance(&id("f-pat1"), FIRST_INSTANCE)
        .expect("the BOX map has an instance copy");

    assert_eq!(
        copy.name(ElementKind::Edge, 2),
        Some(&name("edge(f-box1/front@f-pat1:1|f-box1/top@f-pat1:1)"))
    );
}

#[test]
fn t052_with_instance_keeps_every_index() {
    let source = box_map();
    let feature = id("f-pat1");

    let copy = source
        .with_instance(&feature, FIRST_INSTANCE)
        .expect("the BOX map has an instance copy");

    for kind in [ElementKind::Face, ElementKind::Edge, ElementKind::Vertex] {
        assert_eq!(copy.count(kind), source.count(kind));
        for index in 1..=source.count(kind) {
            let expected = source.name(kind, index).map(|original| {
                original
                    .with_instance(&feature, FIRST_INSTANCE)
                    .expect("every BOX name has an instance name")
            });
            assert_eq!(
                copy.name(kind, index),
                expected.as_ref(),
                "{kind:?} {index}"
            );
        }
    }
}

#[test]
fn t052_with_instance_renames_alias_keys_and_targets() {
    let copy = merge_map()
        .with_instance(&id("f-pat1"), FIRST_INSTANCE)
        .expect("the merge map has an instance copy");

    assert_eq!(
        copy.aliases(),
        &aliases(&[("f-ext2/end-cap@f-pat1:1", "f-ext1/end-cap@f-pat1:1")])
    );
}

#[test]
fn t053_project_copies_selected_names() {
    let projected = box_map()
        .project(&[6, 3], &[2], &[])
        .expect("the indices are in range");

    assert_eq!(
        names_of(&projected, ElementKind::Face),
        [Some("f-box1/top"), Some("f-box1/front")]
    );
    assert_eq!(
        names_of(&projected, ElementKind::Edge),
        [Some("edge(f-box1/front|f-box1/top)")]
    );
    assert_eq!(projected.count(ElementKind::Vertex), 0);
}

#[test]
fn t053_project_rejects_index_out_of_range() {
    let result = box_map().project(&[7], &[], &[]);

    assert_eq!(
        result,
        Err(NamingError::IndexOutOfRange {
            kind: ElementKind::Face,
            index: 7,
            count: 6
        })
    );
}

#[test]
fn t054_maps_survive_json_round_trip() {
    for map in maps_of_tests_33_to_46() {
        let json = serde_json::to_string(&map).expect("a naming map serializes");

        let back: NamingMap = serde_json::from_str(&json).expect("a serialized map deserializes");

        assert_eq!(back, map, "{json}");
    }
}

#[test]
fn t054_map_without_aliases_has_no_aliases_key() {
    let value = serde_json::to_value(box_map()).expect("a naming map serializes");

    assert!(
        value.get("aliases").is_none(),
        "a map without aliases must omit the key, got {value}"
    );
}

#[test]
fn t054_map_with_aliases_serializes_them() {
    let value = serde_json::to_value(merge_map()).expect("a naming map serializes");

    assert_eq!(
        value.get("aliases"),
        Some(&json!({"f-ext2/end-cap": "f-ext1/end-cap"}))
    );
}

#[test]
fn t055_deserializing_duplicate_face_name_fails() {
    let expected = NamingError::DuplicateName {
        name: "f-a/x".to_owned(),
    };

    let error = serde_json::from_str::<NamingMap>(
        r#"{"faces": ["f-a/x", "f-a/x"], "edges": [], "vertices": []}"#,
    )
    .expect_err("a duplicate face name must be rejected");

    assert!(
        error.to_string().contains(&expected.to_string()),
        "the error must be DuplicateName ({expected}), got: {error}"
    );
}

#[test]
fn t055_deserializing_face_name_in_edges_fails() {
    let error = serde_json::from_str::<NamingMap>(
        r#"{"faces": ["f-a/x"], "edges": ["f-a/x"], "vertices": []}"#,
    )
    .expect_err("a face name in the edge list must be rejected");

    assert!(
        error
            .to_string()
            .contains(r#""f-a/x" is not a valid element name"#),
        "the error must be InvalidName for \"f-a/x\", got: {error}"
    );
}
