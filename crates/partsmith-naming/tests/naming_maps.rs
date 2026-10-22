//! Acceptance tests for building, renaming, projecting and serializing naming maps (spec
//! section 6, tests 33 to 55, 115, 123, 131 and 139).

use crate::common;

use common::{
    BOX_EDGE_NAMES, BOX_FACE_NAMES, BOX_VERTEX_NAMES, aliases, box_map, box_sources, box_topology,
    build, build_on, build_with, cylinder_map, degenerate_edge_info, edge_info, face_info,
    face_row_topology, faces_only_topology, from_indexed_sources, from_sources, id, map_from_json,
    map_with_faces, name, names, names_of, new_source, sphere_map, successions, translation,
    vertex_info,
};
use partsmith_naming::{
    Aliases, ElementKind, FaceInfo, FaceSource, Fingerprint, GeometryKind, Instance, Motion,
    NamingError, NamingMap, Successions, TopologyInfo,
};
use proptest::prelude::*;
use serde_json::{Value, json};

/// The instance `(1, 0)`, used by mirrors and the first pattern copy.
const FIRST_INSTANCE: Instance = Instance {
    first: 1,
    second: 0,
};

/// The succession of spec test 131: a line replaced by three.
const REPLACED_LINE: (&str, &[&str]) = (
    "f-ext1/lateral/e-3",
    &[
        "f-ext1/lateral/e-9",
        "f-ext1/lateral/e-10",
        "f-ext1/lateral/e-11",
    ],
);

/// An inherited map with these faces and merge aliases, read from JSON.
fn inherited_with_aliases(face_names: &[&str], alias_entries: &[(&str, &[&str])]) -> NamingMap {
    map_from_json(json!({
        "faces": face_names,
        "edges": [],
        "vertices": [],
        "aliases": serde_json::to_value(aliases(alias_entries)).expect("aliases serialize"),
    }))
}

/// An inherited map with these faces and sketch successions, read from JSON.
fn inherited_with_successions(
    face_names: &[&str],
    succession_entries: &[(&str, &[&str])],
) -> NamingMap {
    map_from_json(json!({
        "faces": face_names,
        "edges": [],
        "vertices": [],
        "successions": serde_json::to_value(successions(succession_entries))
            .expect("successions serialize"),
    }))
}

/// Builds `sources` over a row of unit faces, every `From` source from `inherited`.
fn build_from(inherited: &NamingMap, sources: Vec<FaceSource>) -> NamingMap {
    let topology = face_row_topology(sources.len());
    build_on(sources, &topology, &[inherited])
}

/// Test 37: two pieces of one end cap, face 1 at x = 65 and face 2 at x = 15.
fn split_end_cap_map() -> NamingMap {
    let topology = faces_only_topology(&[
        "plane|n=0,0,1|c=65,25,30|a=1500",
        "plane|n=0,0,1|c=15,25,30|a=1500",
    ]);
    build_on(
        vec![
            from_sources(&["f-ext1/end-cap"]),
            from_sources(&["f-ext1/end-cap"]),
        ],
        &topology,
        &[&map_with_faces(&["f-ext1/end-cap"])],
    )
}

/// Two pieces of `f-a/x` with the given fingerprints.
fn two_pieces_map(first: &str, second: &str) -> NamingMap {
    build_on(
        vec![from_sources(&["f-a/x"]), from_sources(&["f-a/x"])],
        &faces_only_topology(&[first, second]),
        &[&map_with_faces(&["f-a/x"])],
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
    build_on(
        vec![from_sources(&["f-a/x"]), from_sources(&["f-a/x"])],
        &topology,
        &[&map_with_faces(&["f-a/x"])],
    )
}

/// Test 40: one face from an inherited piece.
fn inherited_suffix_map() -> NamingMap {
    build_from(
        &map_with_faces(&["f-ext1/end-cap#2"]),
        vec![from_sources(&["f-ext1/end-cap#2"])],
    )
}

/// Test 41: face 1 from `f-a/x#1`, faces 2 and 3 new `f-a/x`, at x = 0, 1 and 2.
fn second_pass_map() -> NamingMap {
    build_from(
        &map_with_faces(&["f-a/x#1"]),
        vec![
            from_sources(&["f-a/x#1"]),
            new_source("f-a/x"),
            new_source("f-a/x"),
        ],
    )
}

/// Test 42: one face merged from two end caps.
fn merge_map() -> NamingMap {
    build_from(
        &map_with_faces(&["f-ext1/end-cap", "f-ext2/end-cap"]),
        vec![from_sources(&["f-ext2/end-cap", "f-ext1/end-cap"])],
    )
}

/// Test 43: two pieces of one face rejoined.
fn rejoin_map() -> NamingMap {
    build_from(
        &map_with_faces(&["f-a/x#1", "f-a/x#2"]),
        vec![from_sources(&["f-a/x#1", "f-a/x#2"])],
    )
}

/// Test 44: pieces of two different faces merged.
fn merge_of_pieces_map() -> NamingMap {
    build_from(
        &map_with_faces(&["f-a/x#1", "f-b/y#2"]),
        vec![from_sources(&["f-b/y#2", "f-a/x#1"])],
    )
}

/// Test 45: an inherited alias to a face that merges into another.
fn alias_carry_map() -> NamingMap {
    build_from(
        &inherited_with_aliases(&["f-a/x", "f-b/y"], &[("f-c/z", &["f-b/y"])]),
        vec![from_sources(&["f-b/y", "f-a/x"])],
    )
}

/// Test 46: builds `outputs` (each from one inherited face) on a map holding them and `alias`.
fn pruning_map(alias: (&str, &[&str]), outputs: &[&str]) -> NamingMap {
    build_from(
        &inherited_with_aliases(outputs, &[alias]),
        outputs
            .iter()
            .map(|output| from_sources(&[output]))
            .collect(),
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
        pruning_map(("f-c/z", &["f-b/y"]), &["f-a/x"]),
        pruning_map(("f-c/z", &["f-b/y"]), &["f-b/y#2"]),
    ]
}

/// Test 131: an extrude's faces with the succession of a replaced line.
fn succession_map() -> NamingMap {
    build_with(
        vec![
            new_source("f-ext1/lateral/e-9"),
            new_source("f-ext1/lateral/e-10"),
            new_source("f-ext1/lateral/e-11"),
            new_source("f-ext1/end-cap"),
        ],
        &face_row_topology(4),
        &[],
        &successions(&[REPLACED_LINE]),
        &Motion::IDENTITY,
    )
}

/// Three tool maps for the order tests (spec tests 51 and 115): a target with `f-t/top`, tools
/// with aliases of one key to different faces, and a tool with a succession.
fn order_test_maps() -> [NamingMap; 4] {
    [
        map_with_faces(&["f-t/top"]),
        inherited_with_aliases(&["f-a/x"], &[("f-k/k", &["f-a/x"]), ("f-m/m", &["f-a/x"])]),
        inherited_with_aliases(&["f-b/y"], &[("f-k/k", &["f-b/y"])]),
        inherited_with_successions(&["f-e/v"], &[("f-e/u", &["f-e/v", "f-e/w"])]),
    ]
}

/// One output face per face of the order test maps, each from its map, listed for the order of
/// `maps` given as original map indices.
fn order_test_sources(order: &[u32]) -> Vec<FaceSource> {
    let position = |original: u32| {
        u32::try_from(
            order
                .iter()
                .position(|map| *map == original)
                .expect("every map is in the order"),
        )
        .expect("map positions fit in u32")
    };
    vec![
        from_indexed_sources(&[(position(0), "f-t/top")]),
        from_indexed_sources(&[(position(1), "f-a/x")]),
        from_indexed_sources(&[(position(2), "f-b/y")]),
        from_indexed_sources(&[(position(3), "f-e/v")]),
    ]
}

/// Builds the order test faces over `maps` taken in `order` (original indices).
fn build_in_order(maps: &[NamingMap], order: &[u32]) -> NamingMap {
    let inherited: Vec<&NamingMap> = order.iter().map(|map| &maps[*map as usize]).collect();
    build_on(order_test_sources(order), &face_row_topology(4), &inherited)
}

/// The 12 numbers of a JSON motion, or a panic.
fn motion_numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("a motion is a JSON array, got {value}"))
        .iter()
        .map(|number| number.as_f64().expect("motion components are numbers"))
        .collect()
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
fn t042_merge_records_alias_to_a_target_list() {
    assert_eq!(
        merge_map().aliases(),
        &aliases(&[("f-ext2/end-cap", &["f-ext1/end-cap"])])
    );
}

#[test]
fn t042_merge_records_no_succession() {
    assert!(merge_map().successions().is_empty());
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
fn t044_merge_of_pieces_aliases_full_names() {
    assert_eq!(
        merge_of_pieces_map().aliases(),
        &aliases(&[("f-b/y#2", &["f-a/x#1"])])
    );
}

#[test]
fn t045_inherited_aliases_are_carried_and_flattened() {
    assert_eq!(
        alias_carry_map().aliases(),
        &aliases(&[("f-b/y", &["f-a/x"]), ("f-c/z", &["f-a/x"])])
    );
}

#[test]
fn t046_alias_to_a_target_no_face_extends_is_dropped() {
    let map = pruning_map(("f-c/z", &["f-b/y"]), &["f-a/x"]);

    assert_eq!(map.aliases(), &Aliases::default());
}

#[test]
fn t046_alias_to_a_target_a_piece_extends_is_kept() {
    let map = pruning_map(("f-c/z", &["f-b/y"]), &["f-b/y#2"]);

    assert_eq!(map.aliases(), &aliases(&[("f-c/z", &["f-b/y"])]));
}

#[test]
fn t046_alias_whose_key_is_an_output_face_is_dropped() {
    let map = pruning_map(("f-c/z#2", &["f-a/x"]), &["f-a/x", "f-c/z#2"]);

    assert_eq!(map.aliases(), &Aliases::default());
}

#[test]
fn t046_alias_whose_key_is_a_sibling_of_an_output_face_is_kept() {
    let map = pruning_map(("f-c/z#2", &["f-a/x"]), &["f-c/z#1", "f-a/x"]);

    assert_eq!(map.aliases(), &aliases(&[("f-c/z#2", &["f-a/x"])]));
}

#[test]
fn t046_alias_to_a_piece_is_kept_while_that_piece_is_split_further() {
    let map = pruning_map(("f-c/z", &["f-a/x#2"]), &["f-a/x#2#1"]);

    assert_eq!(map.aliases(), &aliases(&[("f-c/z", &["f-a/x#2"])]));
}

#[test]
fn t046_alias_to_a_piece_is_dropped_when_only_a_sibling_is_left() {
    let map = pruning_map(("f-c/z", &["f-a/x#2"]), &["f-a/x#1"]);

    assert_eq!(map.aliases(), &Aliases::default());
}

#[test]
fn t046_pruning_drops_only_the_dead_targets_of_a_list() {
    let map = pruning_map(("f-c/z", &["f-a/x", "f-b/y"]), &["f-b/y"]);

    assert_eq!(map.aliases(), &aliases(&[("f-c/z", &["f-b/y"])]));
}

#[test]
fn t047_build_fails_on_face_count_mismatch() {
    let mut sources = box_sources();
    sources.pop();

    let result = NamingMap::build(
        sources,
        &box_topology(),
        &[],
        &Successions::default(),
        &Motion::IDENTITY,
    );

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
    let inherited = map_with_faces(&["f-a/x"]);

    let result = NamingMap::build(
        vec![FaceSource::From(Vec::new())],
        &face_row_topology(1),
        &[&inherited],
        &Successions::default(),
        &Motion::IDENTITY,
    );

    assert_eq!(result, Err(NamingError::EmptySources { face: 1 }));
}

#[test]
fn t047_build_fails_on_face_index_out_of_range() {
    let mut topology = box_topology();
    topology.edges[0].faces = vec![3, 7];

    let result = NamingMap::build(
        box_sources(),
        &topology,
        &[],
        &Successions::default(),
        &Motion::IDENTITY,
    );

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

    let result = NamingMap::build(
        box_sources(),
        &topology,
        &[],
        &Successions::default(),
        &Motion::IDENTITY,
    );

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
    let inherited = map_with_faces(&["f-a/x"]);

    let result = NamingMap::build(
        vec![from_sources(&["f-a/x"]), from_sources(&["f-a/x"])],
        &topology,
        &[&inherited],
        &Successions::default(),
        &Motion::IDENTITY,
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

#[test]
fn t051_order_of_the_maps_after_the_first_does_not_matter() {
    let maps = order_test_maps();

    let forward = build_in_order(&maps, &[0, 1, 2, 3]);
    let shuffled = build_in_order(&maps, &[0, 3, 1, 2]);

    assert_eq!(forward, shuffled);
}

#[test]
fn t051_order_of_moved_maps_after_the_first_does_not_matter() {
    let [target, first, second, third] = order_test_maps();
    let maps = [
        target.moved(&translation(0.0, 0.0, 1.0)),
        first.moved(&translation(1.0, 0.0, 0.0)),
        second.moved(&translation(0.0, 2.0, 0.0)),
        third,
    ];

    let forward = build_in_order(&maps, &[0, 1, 2, 3]);
    let shuffled = build_in_order(&maps, &[0, 2, 3, 1]);

    assert_eq!(forward, shuffled);
}

#[test]
fn t051_order_of_all_maps_without_motions_does_not_matter() {
    let maps = order_test_maps();

    let forward = build_in_order(&maps, &[0, 1, 2, 3]);
    let shuffled = build_in_order(&maps, &[2, 0, 3, 1]);

    assert_eq!(forward, shuffled);
}

#[test]
fn t051_inherited_alias_lists_of_one_key_are_unioned() {
    let map = build_in_order(&order_test_maps(), &[0, 1, 2, 3]);

    assert_eq!(
        map.aliases(),
        &aliases(&[("f-k/k", &["f-a/x", "f-b/y"]), ("f-m/m", &["f-a/x"])])
    );
}

#[test]
fn t051_inherited_successions_are_carried() {
    let map = build_in_order(&order_test_maps(), &[0, 1, 2, 3]);

    assert_eq!(
        map.successions(),
        &successions(&[("f-e/u", &["f-e/v", "f-e/w"])])
    );
}

#[test]
fn t051_conflicting_successions_keep_the_smallest_list() {
    let first = inherited_with_successions(&["f-a/x"], &[("f-k/k", &["f-c/z", "f-b/y"])]);
    let second = inherited_with_successions(&["f-b/y"], &[("f-k/k", &["f-b/y", "f-z/z"])]);
    let sources = |first_map: u32, second_map: u32| {
        vec![
            from_indexed_sources(&[(first_map, "f-a/x")]),
            from_indexed_sources(&[(second_map, "f-b/y")]),
        ]
    };

    let forward = build_on(sources(0, 1), &face_row_topology(2), &[&first, &second]);
    let backward = build_on(sources(1, 0), &face_row_topology(2), &[&second, &first]);

    assert_eq!(
        forward.successions(),
        &successions(&[("f-k/k", &["f-b/y", "f-z/z"])])
    );
    assert_eq!(backward.successions(), forward.successions());
}

#[test]
fn t052_with_instance_renames_faces() {
    let copy = box_map()
        .with_instance(&id("f-pat1"), FIRST_INSTANCE, &Motion::IDENTITY)
        .expect("the BOX map has an instance copy");

    assert_eq!(
        copy.name(ElementKind::Face, 6),
        Some(&name("f-box1/top@f-pat1:1"))
    );
}

#[test]
fn t052_with_instance_renames_edges() {
    let copy = box_map()
        .with_instance(&id("f-pat1"), FIRST_INSTANCE, &Motion::IDENTITY)
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
        .with_instance(&feature, FIRST_INSTANCE, &Motion::IDENTITY)
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
        .with_instance(&id("f-pat1"), FIRST_INSTANCE, &Motion::IDENTITY)
        .expect("the merge map has an instance copy");

    assert_eq!(
        copy.aliases(),
        &aliases(&[("f-ext2/end-cap@f-pat1:1", &["f-ext1/end-cap@f-pat1:1"])])
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
fn t053_project_copies_motions_unchanged() {
    let moved = box_map().moved(&translation(100.0, 0.0, 0.0));

    let projected = moved
        .project(&[6, 3], &[2], &[])
        .expect("the indices are in range");

    assert_eq!(
        projected.motion(&name("f-box1/top")),
        moved.motion(&name("f-box1/top"))
    );
    assert_eq!(
        projected.motion(&name("f-box1/left")),
        moved.motion(&name("f-box1/left"))
    );
}

#[test]
fn t053_project_prunes_aliases_against_the_projected_faces() {
    let inherited = inherited_with_aliases(
        &["f-a/x", "f-b/y"],
        &[("f-c/z", &["f-a/x"]), ("f-d/w", &["f-b/y"])],
    );
    let map = build_from(
        &inherited,
        vec![from_sources(&["f-a/x"]), from_sources(&["f-b/y"])],
    );

    let projected = map.project(&[2], &[], &[]).expect("the index is in range");

    assert_eq!(projected.aliases(), &aliases(&[("f-d/w", &["f-b/y"])]));
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

#[test]
fn t115_tool_maps_in_either_order_give_equal_maps() {
    let target = map_with_faces(&["f-t/top"]);
    let first = inherited_with_aliases(&["f-a/x"], &[("f-k/k", &["f-a/x"])]);
    let second = inherited_with_aliases(&["f-b/y"], &[("f-k/k", &["f-b/y"])]);
    let sources = |first_map: u32, second_map: u32| {
        vec![
            from_indexed_sources(&[(0, "f-t/top")]),
            from_indexed_sources(&[(first_map, "f-a/x"), (second_map, "f-b/y")]),
        ]
    };

    let forward = build_on(
        sources(1, 2),
        &face_row_topology(2),
        &[&target, &first, &second],
    );
    let backward = build_on(
        sources(2, 1),
        &face_row_topology(2),
        &[&target, &second, &first],
    );

    assert_eq!(forward, backward);
    assert_eq!(
        forward.aliases(),
        &aliases(&[("f-b/y", &["f-a/x"]), ("f-k/k", &["f-a/x"])])
    );
}

/// Test 115: the target `M0` and the tools `M1` and `M2`, moved by `T1` and `T2`, joined in the
/// order `order` of original map indices.
fn moved_tools_joined(order: &[u32]) -> NamingMap {
    let maps = [
        map_with_faces(&["f-x/top#1"]),
        map_with_faces(&["f-x/top#2"]).moved(&translation(10.0, 0.0, 0.0)),
        map_with_faces(&["f-x/top#3"]).moved(&translation(0.0, 10.0, 0.0)),
    ];
    let position = |original: u32| {
        u32::try_from(
            order
                .iter()
                .position(|map| *map == original)
                .expect("every map is in the order"),
        )
        .expect("map positions fit in u32")
    };
    let inherited: Vec<&NamingMap> = order.iter().map(|map| &maps[*map as usize]).collect();
    build_on(
        vec![
            from_indexed_sources(&[(position(0), "f-x/top#1")]),
            from_indexed_sources(&[(position(1), "f-x/top#2")]),
            from_indexed_sources(&[(position(2), "f-x/top#3")]),
        ],
        &face_row_topology(3),
        &inherited,
    )
}

#[test]
fn t115_moved_tool_maps_in_either_order_give_equal_maps() {
    assert_eq!(
        moved_tools_joined(&[0, 1, 2]),
        moved_tools_joined(&[0, 2, 1])
    );
}

#[test]
fn t115_each_piece_keeps_the_motion_of_its_tool() {
    let joined = moved_tools_joined(&[0, 2, 1]);

    assert_eq!(joined.motion(&name("f-x/top#1")), Motion::IDENTITY);
    assert_eq!(
        joined.motion(&name("f-x/top#2")),
        translation(10.0, 0.0, 0.0)
    );
    assert_eq!(
        joined.motion(&name("f-x/top#3")),
        translation(0.0, 10.0, 0.0)
    );
}

#[test]
fn t115_disagreeing_pieces_leave_the_piece_free_name_the_base() {
    let joined = moved_tools_joined(&[0, 1, 2]);

    assert_eq!(joined.motion(&name("f-x/top#4")), Motion::IDENTITY);
    assert_eq!(joined.motion(&name("f-x/top")), Motion::IDENTITY);
}

#[test]
fn t115_same_name_with_different_motions_has_the_base() {
    let target = map_with_faces(&["f-t/top"]).moved(&translation(0.0, 0.0, 5.0));
    let first = map_with_faces(&["f-k/top"]).moved(&translation(1.0, 0.0, 0.0));
    let second = map_with_faces(&["f-k/top"]).moved(&translation(2.0, 0.0, 0.0));

    let joined = build_on(
        vec![
            from_indexed_sources(&[(0, "f-t/top")]),
            from_indexed_sources(&[(1, "f-k/top"), (2, "f-k/top")]),
        ],
        &face_row_topology(2),
        &[&target, &first, &second],
    );

    assert_eq!(joined.motion(&name("f-k/top")), translation(0.0, 0.0, 5.0));
}

#[test]
fn t123_box_map_has_no_motions_key() {
    let value = serde_json::to_value(box_map()).expect("a naming map serializes");

    assert!(
        value.get("motions").is_none(),
        "a map without motions must omit the key, got {value}"
    );
}

#[test]
fn t123_moved_map_serializes_its_base_motion() {
    let value = serde_json::to_value(box_map().moved(&translation(100.0, 0.0, 0.0)))
        .expect("a naming map serializes");

    assert_eq!(
        motion_numbers(&value["motions"]["base"]),
        [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 100.0, 0.0, 0.0]
    );
    assert_eq!(value["motions"]["own"], json!({}));
}

#[test]
fn t123_moved_map_round_trips() {
    let moved = box_map().moved(&translation(100.0, 0.0, 0.0));

    let json = serde_json::to_string(&moved).expect("a naming map serializes");
    let back: NamingMap = serde_json::from_str(&json).expect("a serialized map deserializes");

    assert_eq!(back, moved);
}

#[test]
fn t123_own_motions_serialize_under_face_names() {
    let joined = moved_tools_joined(&[0, 1, 2]);

    let value = serde_json::to_value(&joined).expect("a naming map serializes");

    let own = value["motions"]["own"]
        .as_object()
        .unwrap_or_else(|| panic!("own motions are an object, got {value}"));
    let mut keys: Vec<&str> = own.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["f-x/top#2", "f-x/top#3"]);
    assert_eq!(
        motion_numbers(&own["f-x/top#2"]),
        [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 10.0, 0.0, 0.0]
    );
}

#[test]
fn t123_map_with_own_motions_round_trips() {
    let joined = moved_tools_joined(&[0, 1, 2]);

    let json = serde_json::to_string(&joined).expect("a naming map serializes");
    let back: NamingMap = serde_json::from_str(&json).expect("a serialized map deserializes");

    assert_eq!(back, joined);
}

#[test]
fn t123_merge_map_serializes_alias_target_lists() {
    let value = serde_json::to_value(merge_map()).expect("a naming map serializes");

    assert_eq!(
        value.get("aliases"),
        Some(&json!({"f-ext2/end-cap": ["f-ext1/end-cap"]}))
    );
}

#[test]
fn t123_broken_map_json_fails() {
    let broken = [
        json!({"faces": ["f-a/x"], "edges": [], "vertices": [], "aliases": {"f-b/y": "f-a/x"}}),
        json!({"faces": ["f-a/x"], "edges": [], "vertices": [], "aliases": {"f-b/y": []}}),
        json!({"faces": ["f-a/x"], "edges": [], "vertices": [], "motions": {"base": [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0], "own": {}}}),
    ];

    for value in broken {
        let result = serde_json::from_value::<NamingMap>(value.clone());

        assert!(result.is_err(), "{value} must not deserialize");
    }
}

#[test]
fn t123_alias_target_lists_must_be_sorted_and_distinct() {
    let broken = [
        json!({"faces": ["f-a/x"], "edges": [], "vertices": [], "aliases": {"f-c/z": ["f-b/y", "f-a/x"]}}),
        json!({"faces": ["f-a/x"], "edges": [], "vertices": [], "aliases": {"f-c/z": ["f-a/x", "f-a/x"]}}),
    ];

    for value in broken {
        let result = serde_json::from_value::<NamingMap>(value.clone());

        assert!(result.is_err(), "{value} must not deserialize");
    }
}

#[test]
fn t123_own_motion_keys_must_be_face_names() {
    let value = json!({
        "faces": ["f-a/x"],
        "edges": [],
        "vertices": [],
        "motions": {
            "base": [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0],
            "own": {"edge(f-a/x)": [1, 0, 0, 0, 1, 0, 0, 0, 1, 5, 0, 0]}
        }
    });

    let result = serde_json::from_value::<NamingMap>(value);

    assert!(result.is_err(), "an edge name is no face name");
}

#[test]
fn t123_own_motions_read_from_json_are_used() {
    let map = map_from_json(json!({
        "faces": ["f-a/x"],
        "edges": [],
        "vertices": [],
        "motions": {
            "base": [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0],
            "own": {"f-a/x": [1, 0, 0, 0, 1, 0, 0, 0, 1, 5, 0, 0]}
        }
    }));

    assert_eq!(map.motion(&name("f-a/x")), translation(5.0, 0.0, 0.0));
    assert_eq!(map.motion(&name("f-a/x#2")), translation(5.0, 0.0, 0.0));
    assert_eq!(map.motion(&name("f-b/y")), Motion::IDENTITY);
}

#[test]
fn t131_build_keeps_the_order_of_added_successions() {
    assert_eq!(
        succession_map().successions(),
        &successions(&[REPLACED_LINE])
    );
}

#[test]
fn t131_successions_serialize_in_order() {
    let value = serde_json::to_value(succession_map()).expect("a naming map serializes");

    assert_eq!(
        value.get("successions"),
        Some(
            &json!({"f-ext1/lateral/e-3": ["f-ext1/lateral/e-9", "f-ext1/lateral/e-10", "f-ext1/lateral/e-11"]})
        )
    );
}

#[test]
fn t131_map_with_successions_round_trips() {
    let map = succession_map();

    let json = serde_json::to_string(&map).expect("a naming map serializes");
    let back: NamingMap = serde_json::from_str(&json).expect("a serialized map deserializes");

    assert_eq!(back, map);
}

#[test]
fn t131_a_map_built_on_it_carries_the_successions() {
    let map = succession_map();

    let joined = build_from(
        &map,
        vec![
            from_sources(&["f-ext1/lateral/e-9"]),
            from_sources(&["f-ext1/lateral/e-10"]),
            from_sources(&["f-ext1/lateral/e-11"]),
            from_sources(&["f-ext1/end-cap"]),
        ],
    );

    assert_eq!(joined.successions(), &successions(&[REPLACED_LINE]));
}

#[test]
fn t131_successions_are_never_pruned() {
    let map = succession_map();

    let cut = build_from(&map, vec![from_sources(&["f-ext1/end-cap"])]);

    assert_eq!(cut.successions(), &successions(&[REPLACED_LINE]));
}

#[test]
fn t131_with_instance_renames_successions_in_order() {
    let copy = succession_map()
        .with_instance(&id("f-lp1"), FIRST_INSTANCE, &translation(0.0, 30.0, 0.0))
        .expect("the map has an instance copy");

    assert_eq!(
        copy.successions(),
        &successions(&[(
            "f-ext1/lateral/e-3@f-lp1:1",
            &[
                "f-ext1/lateral/e-9@f-lp1:1",
                "f-ext1/lateral/e-10@f-lp1:1",
                "f-ext1/lateral/e-11@f-lp1:1",
            ],
        )])
    );
}

#[test]
fn t131_project_copies_successions_unchanged() {
    let projected = succession_map()
        .project(&[4], &[], &[])
        .expect("the index is in range");

    assert_eq!(projected.successions(), &successions(&[REPLACED_LINE]));
}

#[test]
fn t131_map_without_successions_has_no_successions_key() {
    let value = serde_json::to_value(box_map()).expect("a naming map serializes");

    assert!(
        value.get("successions").is_none(),
        "a map without successions must omit the key, got {value}"
    );
}

#[test]
fn t131_succession_listing_a_target_twice_fails() {
    let value = json!({
        "faces": ["f-a/x"],
        "edges": [],
        "vertices": [],
        "successions": {"f-ext1/lateral/e-3": ["f-ext1/lateral/e-9", "f-ext1/lateral/e-9"]}
    });

    let result = serde_json::from_value::<NamingMap>(value);

    assert!(
        result.is_err(),
        "a succession target listed twice must be rejected"
    );
}

#[test]
fn t131_succession_targets_in_any_order_are_read_as_written() {
    let map = inherited_with_successions(
        &["f-a/x"],
        &[(
            "f-ext1/lateral/e-2",
            &["f-ext1/lateral/e-2", "f-ext1/lateral/e-10"],
        )],
    );

    assert_eq!(
        map.successions().0[&name("f-ext1/lateral/e-2")],
        names(&["f-ext1/lateral/e-2", "f-ext1/lateral/e-10"])
    );
}

#[test]
fn t139_source_beyond_the_inherited_maps_fails() {
    let inherited = map_with_faces(&["f-a/x"]);

    let result = NamingMap::build(
        vec![from_indexed_sources(&[(1, "f-a/x")])],
        &face_row_topology(1),
        &[&inherited],
        &Successions::default(),
        &Motion::IDENTITY,
    );

    assert_eq!(
        result,
        Err(NamingError::SourceOutOfRange {
            face: 1,
            map: 1,
            count: 1
        })
    );
}

#[test]
fn new_faces_have_the_new_motion_and_the_map_has_it_as_base() {
    let shift = translation(0.0, 0.0, 7.0);

    let map = build_with(
        vec![new_source("f-a/x")],
        &face_row_topology(1),
        &[],
        &Successions::default(),
        &shift,
    );

    assert_eq!(map.motion(&name("f-a/x")), shift);
    assert_eq!(map.motion(&name("f-never/seen")), shift);
}

#[test]
fn box_map_has_no_successions_and_identity_motions() {
    let map = box_map();

    assert!(map.successions().is_empty());
    assert_eq!(map.motion(&name("f-box1/top")), Motion::IDENTITY);
}

/// Face names the random maps of the order property tests draw from, in byte order.
const VOCABULARY: [&str; 8] = [
    "f-a/x", "f-a/x#1", "f-a/x#2", "f-b/y", "f-b/y#1", "f-c/z", "f-d/w", "f-e/v",
];

/// A random inherited map: some faces, merge aliases and successions keyed by other names, and a
/// translation.
#[derive(Debug, Clone)]
struct RandomMap {
    /// Indices of its faces in `VOCABULARY`, ascending.
    faces: Vec<usize>,
    /// Alias keys and their target lists, as indices in `VOCABULARY`.
    aliases: Vec<(usize, Vec<usize>)>,
    /// Succession keys and their lists, as indices in `VOCABULARY`.
    successions: Vec<(usize, Vec<usize>)>,
    /// Translation of the map, in mm along x, y and z.
    translation: [i8; 3],
}

impl RandomMap {
    /// The map, moved by its translation when `with_motion`.
    fn map(&self, with_motion: bool) -> NamingMap {
        let mut alias_entries = serde_json::Map::new();
        for (key, targets) in &self.aliases {
            if self.faces.contains(key) || alias_entries.contains_key(VOCABULARY[*key]) {
                continue;
            }
            // Indices ascend, so the targets are in byte order.
            let target_names: Vec<&str> =
                targets.iter().map(|target| VOCABULARY[*target]).collect();
            alias_entries.insert(VOCABULARY[*key].to_owned(), json!(target_names));
        }
        let mut succession_entries = serde_json::Map::new();
        for (key, targets) in &self.successions {
            if self.faces.contains(key) || succession_entries.contains_key(VOCABULARY[*key]) {
                continue;
            }
            let mut distinct: Vec<&str> = Vec::new();
            for target in targets {
                if !distinct.contains(&VOCABULARY[*target]) {
                    distinct.push(VOCABULARY[*target]);
                }
            }
            succession_entries.insert(VOCABULARY[*key].to_owned(), json!(distinct));
        }
        let faces: Vec<&str> = self.faces.iter().map(|face| VOCABULARY[*face]).collect();
        let mut value = json!({"faces": faces, "edges": [], "vertices": []});
        if !alias_entries.is_empty() {
            value["aliases"] = Value::Object(alias_entries);
        }
        if !succession_entries.is_empty() {
            value["successions"] = Value::Object(succession_entries);
        }
        let map = map_from_json(value);
        if with_motion {
            let [x, y, z] = self.translation.map(f64::from);
            map.moved(&translation(x, y, z))
        } else {
            map
        }
    }
}

/// One output face of the order property tests: new, or from faces picked in the inherited maps.
#[derive(Debug, Clone)]
enum RandomFace {
    /// A new face named `VOCABULARY[index]`.
    New(usize),
    /// Faces picked as (map pick, face pick), each taken modulo the counts.
    From(Vec<(usize, usize)>),
}

/// The picked inherited face: its map index and name.
fn picked(maps: &[RandomMap], (map_pick, face_pick): (usize, usize)) -> (usize, &'static str) {
    let map = map_pick % maps.len();
    let faces = &maps[map].faces;
    (map, VOCABULARY[faces[face_pick % faces.len()]])
}

/// The sources of `faces` over `maps` taken in `order` (original indices).
fn random_sources(maps: &[RandomMap], faces: &[RandomFace], order: &[usize]) -> Vec<FaceSource> {
    faces
        .iter()
        .map(|face| match face {
            RandomFace::New(index) => new_source(VOCABULARY[*index]),
            RandomFace::From(picks) => FaceSource::From(
                picks
                    .iter()
                    .map(|pick| {
                        let (map, face_name) = picked(maps, *pick);
                        let position = order
                            .iter()
                            .position(|original| *original == map)
                            .expect("every map is in the order");
                        (
                            u32::try_from(position).expect("map positions fit in u32"),
                            name(face_name),
                        )
                    })
                    .collect(),
            ),
        })
        .collect()
}

/// Builds `faces` over `maps` taken in `order` (original indices).
fn build_random(
    maps: &[RandomMap],
    faces: &[RandomFace],
    order: &[usize],
    with_motion: bool,
) -> NamingMap {
    let built: Vec<NamingMap> = order
        .iter()
        .map(|original| maps[*original].map(with_motion))
        .collect();
    let inherited: Vec<&NamingMap> = built.iter().collect();
    build_on(
        random_sources(maps, faces, order),
        &face_row_topology(faces.len()),
        &inherited,
    )
}

/// Indices into `VOCABULARY`, ascending and distinct.
fn vocabulary_subset(sizes: std::ops::RangeInclusive<usize>) -> impl Strategy<Value = Vec<usize>> {
    prop::sample::subsequence((0..VOCABULARY.len()).collect::<Vec<usize>>(), sizes)
}

/// A random inherited map.
fn random_map() -> impl Strategy<Value = RandomMap> {
    (
        vocabulary_subset(1..=3),
        prop::collection::vec((0..VOCABULARY.len(), vocabulary_subset(1..=2)), 0..=2),
        prop::collection::vec(
            (
                0..VOCABULARY.len(),
                prop::collection::vec(0..VOCABULARY.len(), 1..=3),
            ),
            0..=2,
        ),
        prop::array::uniform3(-2_i8..=2),
    )
        .prop_map(|(faces, aliases, successions, translation)| RandomMap {
            faces,
            aliases,
            successions,
            translation,
        })
}

/// A random output face.
fn random_face() -> impl Strategy<Value = RandomFace> {
    prop_oneof![
        (0..VOCABULARY.len()).prop_map(RandomFace::New),
        prop::collection::vec((0..8_usize, 0..8_usize), 1..=3).prop_map(RandomFace::From),
    ]
}

/// Random inherited maps, output faces, and a shuffled order of the maps after the first.
fn tools_in_two_orders() -> impl Strategy<Value = (Vec<RandomMap>, Vec<RandomFace>, Vec<usize>)> {
    prop::collection::vec(random_map(), 2..=4).prop_flat_map(|maps| {
        let tools: Vec<usize> = (1..maps.len()).collect();
        (
            Just(maps),
            prop::collection::vec(random_face(), 1..=5),
            Just(tools).prop_shuffle(),
        )
    })
}

/// Random inherited maps, output faces, and a shuffled order of every map.
fn maps_in_two_orders() -> impl Strategy<Value = (Vec<RandomMap>, Vec<RandomFace>, Vec<usize>)> {
    prop::collection::vec(random_map(), 1..=4).prop_flat_map(|maps| {
        let all: Vec<usize> = (0..maps.len()).collect();
        (
            Just(maps),
            prop::collection::vec(random_face(), 1..=5),
            Just(all).prop_shuffle(),
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn t115_building_does_not_depend_on_the_order_of_moved_tool_maps(
        (maps, faces, shuffled_tools) in tools_in_two_orders()
    ) {
        let forward: Vec<usize> = (0..maps.len()).collect();
        let shuffled: Vec<usize> = std::iter::once(0).chain(shuffled_tools).collect();

        let in_order = build_random(&maps, &faces, &forward, true);
        let reordered = build_random(&maps, &faces, &shuffled, true);

        prop_assert_eq!(in_order, reordered);
    }

    #[test]
    fn t051_building_does_not_depend_on_the_order_of_maps_without_motions(
        (maps, faces, shuffled) in maps_in_two_orders()
    ) {
        let forward: Vec<usize> = (0..maps.len()).collect();

        let in_order = build_random(&maps, &faces, &forward, false);
        let reordered = build_random(&maps, &faces, &shuffled, false);

        prop_assert_eq!(in_order, reordered);
    }
}
