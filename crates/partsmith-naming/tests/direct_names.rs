//! Acceptance tests for direct-generator names, the naming maps that hold them, their element infos
//! and element sets (spec section 6, tests 144 to 175 and 198 to 200; sections 2.2, 2.5 to 2.7,
//! 3.1, 3.2, 3.3 step 9, 3.5 and 3.12).

use crate::common;

use std::sync::Arc;

use common::{
    ScopeElement, assert_resolved, box_infos, box_map, box_sources, box_topology, build,
    build_direct, cylinder_map, cylinder_sources, cylinder_topology, direct_of, direct_sources,
    element_named, element_set_of_map, from_direct, id, merged_wire_map, name, names_of,
    new_direct, new_source, numbered_pair_map, pair_map, pair_sources, pair_topology,
    scope_element, single_pair_edge_map, sphere_topology, split_wire_map, split_wire_topology,
    translation, try_build_direct, wire_map, wire_map_holding, wire_topology,
};
use partsmith_naming::{
    Aliases, CopyGroup, DirectSource, DirectSources, ElementInfo, ElementKind, ElementName,
    ElementSet, GeneratorUse, Instance, MatchMethod, NamingError, NamingMap, Reference,
    Successions, resolve,
};
use schemars::generate::SchemaSettings;
use serde_json::{Value, json};

/// The instance `(1, 0)`, used by mirrors and the first pattern copy.
const FIRST_INSTANCE: Instance = Instance {
    first: 1,
    second: 0,
};

/// The direct-generator names of spec test 144 with their kinds.
const DIRECT_EXAMPLES: [(&str, ElementKind); 6] = [
    ("edge:f-hel1/helix", ElementKind::Edge),
    ("vertex:f-hel1/start", ElementKind::Vertex),
    ("edge:f-swp1/rail/e-1.end", ElementKind::Edge),
    ("edge:f-swp1/rail/e-1.end#2", ElementKind::Edge),
    ("edge:f-hel1/helix@f-lp1:1", ElementKind::Edge),
    ("vertex:f-hel1/end#1@f-lp1:0.2", ElementKind::Vertex),
];

/// The names the schema of spec test 153 must not change.
const SCHEMA_NAMES: [&str; 5] = [
    "ElementName",
    "Reference",
    "Anchors",
    "Merged",
    "MergedName",
];

/// Asserts that parsing `text` fails with `InvalidName`.
fn assert_invalid_name(text: &str) {
    let result = ElementName::parse(text);
    assert!(
        matches!(result, Err(NamingError::InvalidName { .. })),
        "{text:?} must be rejected with InvalidName, got {result:?}"
    );
}

/// An `InvalidName` error with this text and reason (spec section 4).
fn invalid_name(text: &str, reason: &str) -> NamingError {
    NamingError::InvalidName {
        text: text.to_owned(),
        reason: reason.to_owned(),
    }
}

/// A feature with an optional sketch generator, as `ElementName::generators` lists it.
fn generator_use(feature: &str, generator: Option<&str>) -> GeneratorUse {
    GeneratorUse {
        feature: id(feature),
        generator: generator.map(str::to_owned),
    }
}

/// Test 158's map: SPLIT WIRE topology with both edges made from one generator.
fn numbered_wire_map() -> NamingMap {
    build_direct(
        Vec::new(),
        direct_sources(vec![new_direct("edge:f-hel1/helix"); 2], Vec::new()),
        &split_wire_topology(),
        &[],
    )
}

/// Test 159's map: WIRE topology carried from the WIRE map by history.
fn carried_wire_map() -> NamingMap {
    build_direct(
        Vec::new(),
        direct_sources(
            vec![from_direct(&[(0, 1)])],
            vec![from_direct(&[(0, 1)]), from_direct(&[(0, 2)])],
        ),
        &wire_topology(),
        &[&wire_map()],
    )
}

/// Test 162's map: one WIRE-shaped edge from the second piece of test 161's map.
fn piece_wire_map() -> NamingMap {
    build_direct(
        Vec::new(),
        direct_sources(vec![from_direct(&[(0, 2)])], Vec::new()),
        &wire_topology(),
        &[&split_wire_map()],
    )
}

/// Test 163's map: the edges of two maps holding `edge:f-a/rail/e-1` and `edge:f-b/rail/e-2`
/// merged into one.
fn merged_rails_map() -> NamingMap {
    merged_wire_map("edge:f-a/rail/e-1", "edge:f-b/rail/e-2")
}

/// Test 199's map: the edges of two maps holding `edge:f-a/rail/e-1` and `edge:f-a/rail/e-10`
/// merged into one.
fn merged_tens_map() -> NamingMap {
    merged_wire_map("edge:f-a/rail/e-1", "edge:f-a/rail/e-10")
}

/// The maps of tests 154 to 163.
fn direct_maps() -> Vec<NamingMap> {
    vec![
        wire_map(),
        pair_map(),
        numbered_wire_map(),
        carried_wire_map(),
        single_pair_edge_map(),
        split_wire_map(),
        piece_wire_map(),
        merged_rails_map(),
    ]
}

/// The element infos of the WIRE map.
fn wire_infos() -> Vec<ElementInfo> {
    wire_map()
        .element_infos(&wire_topology())
        .unwrap_or_else(|error| panic!("the WIRE map matches the WIRE topology, got {error:?}"))
}

/// An element set of `infos` without aliases or successions, or the error it fails with.
fn try_element_set(infos: Vec<ElementInfo>) -> Result<ElementSet, NamingError> {
    ElementSet::new(Arc::from(infos), Aliases::default(), Successions::default())
}

/// The WIRE infos with the edge holding `direct`.
fn wire_infos_with_edge_direct(direct: &str) -> Vec<ElementInfo> {
    let mut infos = wire_infos();
    infos[0] = infos[0].clone().with_direct(&[direct]);
    infos
}

/// The JSON of `map` with its `direct` key replaced by `direct`.
fn json_with_direct(map: &NamingMap, direct: Value) -> Value {
    let mut value = serde_json::to_value(map).expect("a naming map serializes");
    value["direct"] = direct;
    value
}

/// Deserializes `value` as a naming map, or the error message.
fn read_map(value: &Value) -> Result<NamingMap, String> {
    serde_json::from_value(value.clone()).map_err(|error| error.to_string())
}

/// Asserts that every edge and vertex of `actual` has the name and the direct-generator names of
/// the element with its index in `expected`.
fn assert_same_edge_and_vertex_names(actual: &NamingMap, expected: &NamingMap) {
    for kind in [ElementKind::Edge, ElementKind::Vertex] {
        assert_eq!(names_of(actual, kind), names_of(expected, kind), "{kind:?}");
        for index in 1..=expected.count(kind) {
            assert_eq!(
                direct_of(actual, kind, index),
                direct_of(expected, kind, index),
                "{kind:?} {index}"
            );
        }
    }
}

/// The `$defs` of a checked-in JSON Schema.
fn checked_in_definitions(text: &str) -> serde_json::Map<String, Value> {
    let schema: Value = serde_json::from_str(text).expect("a checked-in schema is JSON");
    schema["$defs"]
        .as_object()
        .expect("a checked-in schema has $defs")
        .clone()
}

#[test]
fn t144_parse_accepts_direct_generator_names_unchanged() {
    for (text, _) in DIRECT_EXAMPLES {
        let parsed = ElementName::parse(text).map(|parsed| parsed.as_str().to_owned());

        assert_eq!(parsed, Ok(text.to_owned()));
    }
}

#[test]
fn t144_direct_generator_names_are_direct_and_take_their_kind_from_their_prefix() {
    for (text, kind) in DIRECT_EXAMPLES {
        let parsed = name(text);

        assert!(parsed.is_direct(), "{text} is a direct-generator name");
        assert_eq!(parsed.kind(), kind, "{text}");
    }
}

#[test]
fn t144_face_names_and_face_rule_names_are_not_direct() {
    for text in [
        "f-hel1/helix",
        "edge(f-a/x|f-b/y)#2",
        "vertex(f-a/x)",
        "edge/x",
    ] {
        assert!(
            !name(text).is_direct(),
            "{text} is not a direct-generator name"
        );
    }
}

#[test]
fn t145_parse_rejects_broken_direct_generator_names() {
    for text in [
        "edge:",
        "edge:f-hel1",
        "edge:f-hel1/",
        "edge:f-hel1/Helix",
        "vertex:f-x/start#0",
        "edge:f-x/rail@f-p:0",
        "edge:f-x/rail/e-1.End",
        "edge:f-x/rail/f-a/top",
        "edge:edge:f-x/rail",
        "edge:(f-a/x)",
    ] {
        assert_invalid_name(text);
    }
}

#[test]
fn t145_parse_rejects_a_feature_id_of_65_characters() {
    assert_invalid_name(&format!("edge:{}/rail", "a".repeat(65)));
}

#[test]
fn t145_parse_rejects_a_direct_generator_name_inside_another_name() {
    assert_invalid_name("f-swp1/lateral/edge:f-hel1/helix");
    assert_invalid_name("edge(edge:f-a/x)");
}

#[test]
fn t146_direct_builds_an_edge_name_without_generator() {
    assert_eq!(
        ElementName::direct(ElementKind::Edge, &id("f-hel1"), "helix", None),
        Ok(name("edge:f-hel1/helix"))
    );
}

#[test]
fn t146_direct_builds_a_vertex_name_with_a_sketch_generator() {
    assert_eq!(
        ElementName::direct(ElementKind::Vertex, &id("f-swp1"), "start", Some("e-1.end")),
        Ok(name("vertex:f-swp1/start/e-1.end"))
    );
}

#[test]
fn t146_direct_of_a_face_fails() {
    assert_eq!(
        ElementName::direct(ElementKind::Face, &id("f-hel1"), "helix", None),
        Err(invalid_name(
            "f-hel1/helix",
            "it is not an edge or vertex name"
        ))
    );
}

#[test]
fn t146_direct_rejects_a_role_with_uppercase() {
    assert_eq!(
        ElementName::direct(ElementKind::Edge, &id("f-hel1"), "Helix", None),
        Err(NamingError::InvalidRole {
            role: "Helix".to_owned()
        })
    );
}

#[test]
fn t146_direct_rejects_an_invalid_sketch_reference() {
    assert_eq!(
        ElementName::direct(ElementKind::Edge, &id("f-hel1"), "helix", Some("e 1")),
        Err(NamingError::InvalidGenerator {
            text: "e 1".to_owned()
        })
    );
}

#[test]
fn direct_builds_a_vertex_name_without_generator_and_an_edge_name_with_one() {
    assert_eq!(
        ElementName::direct(ElementKind::Vertex, &id("f-hel1"), "end", None),
        Ok(name("vertex:f-hel1/end"))
    );
    assert_eq!(
        ElementName::direct(ElementKind::Edge, &id("f-swp1"), "rail", Some("e-1.end")),
        Ok(name("edge:f-swp1/rail/e-1.end"))
    );
}

#[test]
fn t147_split_base_of_a_direct_generator_name() {
    assert_eq!(
        name("edge:f-h/helix#2#1").split_base(),
        name("edge:f-h/helix")
    );
}

#[test]
fn t147_piece_free_form_of_a_direct_generator_name() {
    assert_eq!(
        name("edge:f-h/helix#2@f-p:1#1").piece_free(),
        Some(name("edge:f-h/helix@f-p:1"))
    );
}

#[test]
fn t147_split_descendants_of_a_direct_generator_name() {
    let helix = name("edge:f-h/helix");

    assert!(name("edge:f-h/helix#1#2").is_split_descendant_of(&helix));
    assert!(!name("edge:f-h/helix@f-p:1").is_split_descendant_of(&helix));
}

#[test]
fn t147_numbered_direct_generator_names_are_order_dependent() {
    assert!(name("edge:f-h/helix#1").is_order_dependent());
    assert!(!name("edge:f-h/helix@f-p:1").is_order_dependent());
}

#[test]
fn t148_a_direct_generator_name_has_no_face_list() {
    let helix = name("edge:f-h/helix");

    assert!(helix.face_terms().is_empty());
    assert!(helix.face_parts().is_empty());
    assert!(helix.face_name().is_none());
}

#[test]
fn t149_direct_parts_read_the_body_as_a_face_name() {
    let rail = name("edge:f-swp1/rail/e-1.end#2@f-lp1:2");

    let parts = rail
        .direct_parts()
        .expect("a direct-generator name has parts");

    assert_eq!(parts.feature(), "f-swp1");
    assert_eq!(parts.role(), "rail");
    assert_eq!(parts.sketch_generator(), Some("e-1.end"));
    assert_eq!(parts.groups(), "#2@f-lp1:2");
    assert_eq!(
        parts.copies(),
        [CopyGroup {
            feature: "f-lp1",
            instance: Instance {
                first: 2,
                second: 0
            },
            text: "@f-lp1:2",
        }]
    );
    assert_eq!(parts.maker(), "f-lp1");
}

#[test]
fn t149_face_names_and_face_rule_names_have_no_direct_parts() {
    for text in ["f-box1/top", "edge(f-box1/front|f-box1/top)"] {
        assert!(name(text).direct_parts().is_none(), "{text}");
    }
}

#[test]
fn t150_with_split_appends_to_a_direct_generator_name() {
    assert_eq!(
        name("edge:f-h/helix").with_split(2),
        Ok(name("edge:f-h/helix#2"))
    );
}

#[test]
fn t150_with_instance_appends_after_the_own_numbers() {
    assert_eq!(
        name("edge:f-h/helix#2").with_instance(&id("f-p"), FIRST_INSTANCE),
        Ok(name("edge:f-h/helix#2@f-p:1"))
    );
    assert_eq!(
        name("vertex:f-h/start").with_instance(&id("f-p"), FIRST_INSTANCE),
        Ok(name("vertex:f-h/start@f-p:1"))
    );
}

#[test]
fn t151_generators_of_a_copied_rail() {
    assert_eq!(
        name("edge:f-swp1/rail/e-1.end@f-lp1:1").generators(),
        vec![
            generator_use("f-lp1", None),
            generator_use("f-swp1", Some("e-1.end")),
        ]
    );
}

#[test]
fn t151_generators_of_a_vertex_without_generator() {
    assert_eq!(
        name("vertex:f-hel1/start").generators(),
        vec![generator_use("f-hel1", None)]
    );
}

#[test]
fn t152_direct_generator_names_serialize_as_json_strings_and_back() {
    for (text, _) in DIRECT_EXAMPLES {
        let value = serde_json::to_value(name(text)).expect("a name serializes");

        let back: ElementName =
            serde_json::from_value(value.clone()).expect("a serialized name deserializes");

        assert_eq!(value, json!(text));
        assert_eq!(back, name(text));
    }
}

#[test]
fn t152_direct_generator_names_sort_after_face_rule_names_and_before_face_names() {
    assert!(name("edge(f-a/x)") < name("edge:f-a/x"));
    assert!(name("edge:f-a/x") < name("f-a/x"));
}

#[test]
fn t153_the_schemas_of_names_and_references_do_not_change() {
    let mut generator = SchemaSettings::draft2020_12().into_generator();
    generator.subschema_for::<ElementName>();
    generator.subschema_for::<Reference>();
    let generated = generator.definitions();
    let files = [
        (
            "docs/format/ptsm.schema.json",
            include_str!("../../../docs/format/ptsm.schema.json"),
        ),
        (
            "docs/format/protocol.schema.json",
            include_str!("../../../docs/format/protocol.schema.json"),
        ),
    ];

    for (file, text) in files {
        let checked_in = checked_in_definitions(text);
        for type_name in SCHEMA_NAMES {
            assert!(
                generated.contains_key(type_name),
                "the crate generates {type_name}"
            );
            assert_eq!(
                generated.get(type_name),
                checked_in.get(type_name),
                "{type_name} in {file}"
            );
        }
    }
}

#[test]
fn t154_the_wire_map_names_its_edge_and_vertices_by_their_direct_generator_names() {
    let map = wire_map();

    assert_eq!(
        names_of(&map, ElementKind::Edge),
        [Some("edge:f-hel1/helix")]
    );
    assert_eq!(
        names_of(&map, ElementKind::Vertex),
        [Some("vertex:f-hel1/start"), Some("vertex:f-hel1/end")]
    );
}

#[test]
fn t154_the_wire_edge_holds_its_name_and_index_finds_it() {
    let map = wire_map();

    assert_eq!(direct_of(&map, ElementKind::Edge, 1), ["edge:f-hel1/helix"]);
    assert_eq!(map.index(&name("edge:f-hel1/helix")), Some(1));
}

#[test]
fn t155_build_leaves_a_wire_unnamed() {
    let map = build(Vec::new(), &wire_topology());

    assert_eq!(names_of(&map, ElementKind::Edge), [None]);
    assert_eq!(names_of(&map, ElementKind::Vertex), [None, None]);
}

#[test]
fn t155_build_with_direct_without_new_sources_leaves_a_wire_unnamed() {
    let none_lists = direct_sources(vec![DirectSource::None], vec![DirectSource::None; 2]);

    for direct in [DirectSources::default(), none_lists] {
        let map = build_direct(Vec::new(), direct, &wire_topology(), &[]);

        assert_eq!(names_of(&map, ElementKind::Edge), [None]);
        assert_eq!(names_of(&map, ElementKind::Vertex), [None, None]);
        assert!(map.direct(ElementKind::Edge, 1).is_empty());
    }
}

#[test]
fn t156_a_face_rule_name_without_number_gets_no_direct_generator_name() {
    let mut edges = vec![DirectSource::None; 12];
    edges[1] = new_direct("edge:f-box1/rail/e-1");

    let map = build_direct(
        box_sources(),
        direct_sources(edges, vec![DirectSource::None; 8]),
        &box_topology(),
        &[],
    );

    assert_eq!(map, box_map());
    assert!(map.direct(ElementKind::Edge, 2).is_empty());
}

#[test]
fn t157_order_dependent_face_rule_names_stay_as_without_direct_sources() {
    let map = pair_map();
    let without = build(pair_sources(), &pair_topology());

    assert_eq!(
        names_of(&map, ElementKind::Edge),
        [Some("edge(f-a/x|f-b/y)#2"), Some("edge(f-a/x|f-b/y)#1")]
    );
    for kind in [ElementKind::Face, ElementKind::Edge, ElementKind::Vertex] {
        assert_eq!(names_of(&map, kind), names_of(&without, kind), "{kind:?}");
    }
}

#[test]
fn t157_order_dependent_edges_gain_their_direct_generator_names_as_aliases() {
    let map = pair_map();

    assert_eq!(
        direct_of(&map, ElementKind::Edge, 1),
        ["edge:f-a/side/e-1.end"]
    );
    assert_eq!(
        direct_of(&map, ElementKind::Edge, 2),
        ["edge:f-a/side/e-1.start"]
    );
}

#[test]
fn t158_two_elements_made_from_one_generator_are_numbered_by_geometric_order() {
    let map = numbered_wire_map();

    assert_eq!(
        names_of(&map, ElementKind::Edge),
        [Some("edge:f-hel1/helix#1"), Some("edge:f-hel1/helix#2")]
    );
    assert_eq!(
        direct_of(&map, ElementKind::Edge, 1),
        ["edge:f-hel1/helix#1"]
    );
    assert_eq!(
        direct_of(&map, ElementKind::Edge, 2),
        ["edge:f-hel1/helix#2"]
    );
}

#[test]
fn t159_history_carries_the_names_and_direct_generator_names() {
    assert_same_edge_and_vertex_names(&carried_wire_map(), &wire_map());
}

#[test]
fn t160_a_direct_generator_name_is_carried_whatever_the_face_rule_name_becomes() {
    let map = single_pair_edge_map();

    assert_eq!(
        names_of(&map, ElementKind::Edge),
        [Some("edge(f-a/x|f-b/y)")]
    );
    assert_eq!(
        direct_of(&map, ElementKind::Edge, 1),
        ["edge:f-a/side/e-1.end"]
    );
}

#[test]
fn t161_split_pieces_are_numbered_and_a_new_vertex_stays_unnamed() {
    let map = split_wire_map();

    assert_eq!(
        names_of(&map, ElementKind::Edge),
        [Some("edge:f-hel1/helix#1"), Some("edge:f-hel1/helix#2")]
    );
    assert_eq!(
        names_of(&map, ElementKind::Vertex),
        [Some("vertex:f-hel1/start"), None, Some("vertex:f-hel1/end")]
    );
    assert!(map.direct(ElementKind::Vertex, 2).is_empty());
}

#[test]
fn t162_an_inherited_number_is_kept() {
    let map = piece_wire_map();

    assert_eq!(
        names_of(&map, ElementKind::Edge),
        [Some("edge:f-hel1/helix#2")]
    );
}

#[test]
fn t163_a_merged_edge_holds_every_source_name_and_is_named_by_the_first() {
    let map = merged_rails_map();

    assert_eq!(
        direct_of(&map, ElementKind::Edge, 1),
        ["edge:f-a/rail/e-1", "edge:f-b/rail/e-2"]
    );
    assert_eq!(
        names_of(&map, ElementKind::Edge),
        [Some("edge:f-a/rail/e-1")]
    );
}

#[test]
fn t163_index_finds_a_merged_edge_by_either_name() {
    let map = merged_rails_map();

    assert_eq!(map.index(&name("edge:f-a/rail/e-1")), Some(1));
    assert_eq!(map.index(&name("edge:f-b/rail/e-2")), Some(1));
}

#[test]
fn t164_a_degenerated_edge_gets_no_direct_generator_name() {
    let map = build_direct(
        vec![new_source("f-sph1/surface")],
        direct_sources(
            vec![
                new_direct("edge:f-sph1/pole"),
                DirectSource::None,
                DirectSource::None,
            ],
            Vec::new(),
        ),
        &sphere_topology(),
        &[],
    );

    assert_eq!(map.name(ElementKind::Edge, 1), None);
    assert!(map.direct(ElementKind::Edge, 1).is_empty());
}

#[test]
fn a_degenerated_edge_carries_no_direct_generator_name() {
    let map = build_direct(
        vec![new_source("f-sph1/surface")],
        direct_sources(
            vec![
                from_direct(&[(0, 1)]),
                DirectSource::None,
                DirectSource::None,
            ],
            Vec::new(),
        ),
        &sphere_topology(),
        &[&wire_map()],
    );

    assert_eq!(map.name(ElementKind::Edge, 1), None);
    assert!(map.direct(ElementKind::Edge, 1).is_empty());
}

#[test]
fn t165_build_with_direct_fails_on_a_direct_source_count_mismatch() {
    let result = try_build_direct(
        cylinder_sources(),
        direct_sources(vec![DirectSource::None; 2], Vec::new()),
        &cylinder_topology(),
        &[],
    );

    assert_eq!(
        result,
        Err(NamingError::DirectSourceCountMismatch {
            kind: ElementKind::Edge,
            sources: 2,
            elements: 3
        })
    );
}

#[test]
fn t165_build_with_direct_fails_on_empty_direct_sources() {
    let result = try_build_direct(
        cylinder_sources(),
        direct_sources(Vec::new(), vec![from_direct(&[]), DirectSource::None]),
        &cylinder_topology(),
        &[],
    );

    assert_eq!(
        result,
        Err(NamingError::EmptyDirectSources {
            kind: ElementKind::Vertex,
            index: 1
        })
    );
}

#[test]
fn t166_build_with_direct_fails_on_a_direct_source_beyond_the_inherited_maps() {
    let result = try_build_direct(
        cylinder_sources(),
        direct_sources(
            vec![
                from_direct(&[(1, 1)]),
                DirectSource::None,
                DirectSource::None,
            ],
            Vec::new(),
        ),
        &cylinder_topology(),
        &[&cylinder_map()],
    );

    assert_eq!(
        result,
        Err(NamingError::DirectSourceOutOfRange {
            kind: ElementKind::Edge,
            index: 1,
            map: 1,
            count: 1
        })
    );
}

#[test]
fn t166_build_with_direct_fails_on_an_element_index_beyond_the_inherited_map() {
    let result = try_build_direct(
        cylinder_sources(),
        direct_sources(
            vec![
                from_direct(&[(0, 4)]),
                DirectSource::None,
                DirectSource::None,
            ],
            Vec::new(),
        ),
        &cylinder_topology(),
        &[&cylinder_map()],
    );

    assert_eq!(
        result,
        Err(NamingError::IndexOutOfRange {
            kind: ElementKind::Edge,
            index: 4,
            count: 3
        })
    );
}

#[test]
fn t167_a_new_face_rule_name_on_an_edge_fails() {
    let result = try_build_direct(
        cylinder_sources(),
        direct_sources(
            vec![
                new_direct("edge(f-a/x)"),
                DirectSource::None,
                DirectSource::None,
            ],
            Vec::new(),
        ),
        &cylinder_topology(),
        &[],
    );

    assert_eq!(
        result,
        Err(invalid_name(
            "edge(f-a/x)",
            "it is not a direct-generator name"
        ))
    );
}

#[test]
fn t167_a_new_vertex_name_on_an_edge_fails() {
    let result = try_build_direct(
        cylinder_sources(),
        direct_sources(
            vec![
                new_direct("vertex:f-h/start"),
                DirectSource::None,
                DirectSource::None,
            ],
            Vec::new(),
        ),
        &cylinder_topology(),
        &[],
    );

    assert_eq!(
        result,
        Err(invalid_name("vertex:f-h/start", "it is not an edge name"))
    );
}

#[test]
fn t167_a_new_face_name_on_a_vertex_fails() {
    let result = try_build_direct(
        cylinder_sources(),
        direct_sources(Vec::new(), vec![new_direct("f-a/x"), DirectSource::None]),
        &cylinder_topology(),
        &[],
    );

    assert_eq!(
        result,
        Err(invalid_name("f-a/x", "it is not a vertex name"))
    );
}

#[test]
fn t169_the_wire_map_serializes_its_edge_name_and_direct_lists() {
    let json = serde_json::to_string(&wire_map()).expect("a naming map serializes");

    assert_eq!(
        json,
        concat!(
            r#"{"faces":[],"edges":["edge:f-hel1/helix"],"#,
            r#""vertices":["vertex:f-hel1/start","vertex:f-hel1/end"],"#,
            r#""direct":{"edges":[["edge:f-hel1/helix"]],"#,
            r#""vertices":[["vertex:f-hel1/start"],["vertex:f-hel1/end"]]}}"#
        )
    );
}

#[test]
fn t169_the_pair_map_serializes_only_its_edge_lists() {
    let value = serde_json::to_value(pair_map()).expect("a naming map serializes");

    assert_eq!(
        value["direct"],
        json!({"edges": [["edge:f-a/side/e-1.end"], ["edge:f-a/side/e-1.start"]]})
    );
}

#[test]
fn t169_maps_with_direct_generator_names_survive_a_json_round_trip() {
    for map in direct_maps() {
        let json = serde_json::to_string(&map).expect("a naming map serializes");

        let back: NamingMap = serde_json::from_str(&json).expect("a serialized map deserializes");

        assert_eq!(back, map, "{json}");
    }
}

#[test]
fn t169_the_direct_key_comes_after_the_motions() {
    let moved = wire_map().moved(&translation(100.0, 0.0, 0.0));

    let json = serde_json::to_string(&moved).expect("a naming map serializes");

    let motions = json.find(r#""motions""#).expect("a moved map has motions");
    let direct = json
        .find(r#""direct""#)
        .expect("the WIRE map has direct lists");
    assert!(motions < direct, "{json}");
}

#[test]
fn t170_broken_direct_lists_fail() {
    let map = pair_map();
    let broken = [
        json!({"edges": [["edge:f-a/side/e-1.end"]]}),
        json!({"edges": [[], ["edge:f-a/side/e-1.start"]]}),
        json!({"edges": [["edge:f-a/side/e-1.start", "edge:f-a/side/e-1.end"], null]}),
        json!({"edges": [["edge:f-a/side/e-1.end", "edge:f-a/side/e-1.end"], null]}),
        json!({"edges": [["f-a/x"], ["edge:f-a/side/e-1.start"]]}),
        json!({"edges": [["vertex:f-a/side/e-1.end"], ["edge:f-a/side/e-1.start"]]}),
        json!({"edges": [["edge(f-a/x)"], ["edge:f-a/side/e-1.start"]]}),
    ];

    for direct in broken {
        let value = json_with_direct(&map, direct);

        assert!(
            read_map(&value).is_err(),
            "{value} must not deserialize as a naming map"
        );
    }
}

#[test]
fn t170_a_direct_generator_name_held_by_two_edges_fails_with_duplicate_name() {
    let expected = NamingError::DuplicateName {
        name: "edge:f-a/side/e-1.end".to_owned(),
    };
    let value = json_with_direct(
        &pair_map(),
        json!({"edges": [["edge:f-a/side/e-1.end"], ["edge:f-a/side/e-1.end"]]}),
    );

    let error = read_map(&value).expect_err("a duplicate direct-generator name must be rejected");

    assert!(
        error.contains(&expected.to_string()),
        "the error must be DuplicateName ({expected}), got: {error}"
    );
}

#[test]
fn t170_an_element_named_by_a_direct_generator_name_holds_it_first() {
    let value = json_with_direct(
        &wire_map(),
        json!({"edges": [["edge:f-a/rail"]], "vertices": [["vertex:f-hel1/start"], ["vertex:f-hel1/end"]]}),
    );

    assert!(
        read_map(&value).is_err(),
        "{value} must not deserialize as a naming map"
    );
}

#[test]
fn t170_an_element_named_by_a_direct_generator_name_needs_a_list_of_its_kind() {
    // Section 2.6: the element must hold its name first, also when its kind has no written lists.
    let mut without_direct = serde_json::to_value(wire_map()).expect("a naming map serializes");
    without_direct
        .as_object_mut()
        .expect("a naming map is a JSON object")
        .remove("direct");
    let vertices_only = json_with_direct(
        &wire_map(),
        json!({"vertices": [["vertex:f-hel1/start"], ["vertex:f-hel1/end"]]}),
    );

    for value in [without_direct, vertices_only] {
        assert!(
            read_map(&value).is_err(),
            "{value} must not deserialize as a naming map"
        );
    }
}

#[test]
fn t170_an_unnamed_element_holds_no_direct_generator_name() {
    let value = json_with_direct(
        &split_wire_map(),
        json!({
            "edges": [["edge:f-hel1/helix#1"], ["edge:f-hel1/helix#2"]],
            "vertices": [["vertex:f-hel1/start"], ["vertex:f-hel1/middle"], ["vertex:f-hel1/end"]]
        }),
    );

    assert!(
        read_map(&value).is_err(),
        "{value} must not deserialize as a naming map"
    );
}

#[test]
fn t171_with_instance_renames_the_wire_names_and_direct_lists() {
    let copy = wire_map()
        .with_instance(&id("f-lp1"), FIRST_INSTANCE, &translation(40.0, 0.0, 0.0))
        .expect("the WIRE map has an instance copy");

    assert_eq!(
        names_of(&copy, ElementKind::Edge),
        [Some("edge:f-hel1/helix@f-lp1:1")]
    );
    assert_eq!(
        names_of(&copy, ElementKind::Vertex),
        [
            Some("vertex:f-hel1/start@f-lp1:1"),
            Some("vertex:f-hel1/end@f-lp1:1")
        ]
    );
    assert_eq!(
        direct_of(&copy, ElementKind::Edge, 1),
        ["edge:f-hel1/helix@f-lp1:1"]
    );
    assert_eq!(
        direct_of(&copy, ElementKind::Vertex, 1),
        ["vertex:f-hel1/start@f-lp1:1"]
    );
    assert_eq!(
        direct_of(&copy, ElementKind::Vertex, 2),
        ["vertex:f-hel1/end@f-lp1:1"]
    );
}

#[test]
fn t171_with_instance_renames_a_face_rule_name_and_its_alias_alike() {
    let copy = pair_map()
        .with_instance(&id("f-lp1"), FIRST_INSTANCE, &translation(40.0, 0.0, 0.0))
        .expect("the PAIR map has an instance copy");

    assert_eq!(
        copy.name(ElementKind::Edge, 1),
        Some(&name("edge(f-a/x@f-lp1:1|f-b/y@f-lp1:1)#2"))
    );
    assert_eq!(
        direct_of(&copy, ElementKind::Edge, 1),
        ["edge:f-a/side/e-1.end@f-lp1:1"]
    );
}

#[test]
fn t172_moved_keeps_every_name_and_direct_generator_name() {
    let map = wire_map();

    let moved = map.moved(&translation(100.0, 0.0, 0.0));

    assert_same_edge_and_vertex_names(&moved, &map);
}

#[test]
fn t172_project_copies_the_direct_lists() {
    let projected = wire_map()
        .project(&[], &[1], &[2])
        .expect("the indices are in range");

    assert_eq!(
        names_of(&projected, ElementKind::Edge),
        [Some("edge:f-hel1/helix")]
    );
    assert_eq!(
        names_of(&projected, ElementKind::Vertex),
        [Some("vertex:f-hel1/end")]
    );
    assert_eq!(
        direct_of(&projected, ElementKind::Edge, 1),
        ["edge:f-hel1/helix"]
    );
    assert_eq!(
        direct_of(&projected, ElementKind::Vertex, 1),
        ["vertex:f-hel1/end"]
    );
}

#[test]
fn t173_building_twice_gives_identical_json() {
    for (first, second) in direct_maps().iter().zip(direct_maps().iter()) {
        let first_json = serde_json::to_string(first).expect("a naming map serializes");
        let second_json = serde_json::to_string(second).expect("a naming map serializes");

        assert_eq!(first_json, second_json);
    }
}

#[test]
fn t173_the_order_of_the_inherited_maps_does_not_matter() {
    let first = wire_map_holding("edge:f-a/rail/e-1");
    let second = wire_map_holding("edge:f-b/rail/e-2");

    let reordered = build_direct(
        Vec::new(),
        direct_sources(vec![from_direct(&[(1, 1), (0, 1)])], Vec::new()),
        &wire_topology(),
        &[&second, &first],
    );

    assert_eq!(reordered, merged_rails_map());
}

#[test]
fn an_element_holding_several_names_is_numbered_in_the_group_of_each() {
    let map = build_direct(
        Vec::new(),
        direct_sources(
            vec![from_direct(&[(0, 1), (1, 1)]), from_direct(&[(0, 1)])],
            Vec::new(),
        ),
        &split_wire_topology(),
        &[
            &wire_map_holding("edge:f-a/rail"),
            &wire_map_holding("edge:f-b/rail"),
        ],
    );

    assert_eq!(
        direct_of(&map, ElementKind::Edge, 1),
        ["edge:f-a/rail#1", "edge:f-b/rail"]
    );
    assert_eq!(direct_of(&map, ElementKind::Edge, 2), ["edge:f-a/rail#2"]);
    assert_eq!(
        names_of(&map, ElementKind::Edge),
        [Some("edge:f-a/rail#1"), Some("edge:f-a/rail#2")]
    );
}

#[test]
fn direct_of_a_face_and_of_an_index_out_of_range_is_empty() {
    let map = pair_map();

    assert!(map.direct(ElementKind::Face, 1).is_empty());
    assert!(map.direct(ElementKind::Edge, 0).is_empty());
    assert!(map.direct(ElementKind::Edge, 3).is_empty());
}

#[test]
fn t174_the_wire_infos_are_the_edge_then_its_vertices() {
    let infos = wire_infos();

    let listed: Vec<(ElementKind, &str)> = infos
        .iter()
        .map(|info| (info.kind, info.name.as_str()))
        .collect();

    assert_eq!(
        listed,
        [
            (ElementKind::Edge, "edge:f-hel1/helix"),
            (ElementKind::Vertex, "vertex:f-hel1/start"),
            (ElementKind::Vertex, "vertex:f-hel1/end"),
        ]
    );
}

#[test]
fn t174_the_wire_edge_info_serializes_to_spec_json() {
    let value = serde_json::to_value(&wire_infos()[0]).expect("an element info serializes");

    assert_eq!(
        value,
        json!({
            "name": "edge:f-hel1/helix",
            "kind": "edge",
            "fingerprint": "line|n=1,0,0|c=5,0,0|l=10|adj=0,2",
            "adjacency": [],
            "vertices": ["vertex:f-hel1/end", "vertex:f-hel1/start"],
            "direct": ["edge:f-hel1/helix"]
        })
    );
}

#[test]
fn t174_a_wire_vertex_counts_its_edge_and_no_face() {
    assert_eq!(
        wire_infos()[1].fingerprint.to_string(),
        "point|c=0,0,0|adj=0,1"
    );
}

#[test]
fn t174_box_infos_have_no_direct_key() {
    for info in box_infos() {
        let value = serde_json::to_value(&info).expect("an element info serializes");

        assert!(
            value.get("direct").is_none(),
            "an info without direct-generator names has no direct key, got {value}"
        );
    }
}

#[test]
fn t174_an_info_without_a_direct_key_holds_none() {
    let info: ElementInfo = serde_json::from_value(json!({
        "name": "edge(f-box1/front|f-box1/top)",
        "kind": "edge",
        "fingerprint": "line|n=1,0,0|c=5,0,30|l=10|adj=2,2",
        "adjacency": ["f-box1/front", "f-box1/top"]
    }))
    .expect("an info without direct key deserializes");

    assert!(info.direct.is_empty());
}

#[test]
fn infos_of_an_order_dependent_edge_list_its_aliases() {
    let infos = pair_map()
        .element_infos(&pair_topology())
        .expect("the PAIR map matches the PAIR topology");

    for info in &infos {
        let expected: &[&str] = match info.name.as_str() {
            "edge(f-a/x|f-b/y)#2" => &["edge:f-a/side/e-1.end"],
            "edge(f-a/x|f-b/y)#1" => &["edge:f-a/side/e-1.start"],
            _ => &[],
        };
        let direct: Vec<&str> = info.direct.iter().map(ElementName::as_str).collect();
        assert_eq!(direct, expected, "{}", info.name);
    }
}

#[test]
fn t175_the_reference_name_of_a_wire_edge_is_its_name() {
    assert_eq!(wire_infos()[0].reference_name(), &name("edge:f-hel1/helix"));
}

#[test]
fn t175_the_reference_name_of_an_order_dependent_edge_is_its_first_direct_generator_name() {
    let set = element_set_of_map(&pair_map(), &pair_topology());

    let edge = element_named(&set, "edge(f-a/x|f-b/y)#2");

    assert_eq!(edge.reference_name(), &name("edge:f-a/side/e-1.end"));
}

#[test]
fn t175_the_reference_name_of_an_edge_without_number_is_its_name() {
    let map = single_pair_edge_map();
    let infos = map
        .element_infos(&common::single_pair_edge_topology())
        .expect("test 160's map matches its topology");

    let edge = infos
        .iter()
        .find(|info| info.kind == ElementKind::Edge)
        .expect("test 160's map has an edge");

    assert_eq!(edge.reference_name(), &name("edge(f-a/x|f-b/y)"));
}

#[test]
fn t175_the_reference_name_of_every_box_element_is_its_name() {
    for info in box_infos() {
        assert_eq!(info.reference_name(), &info.name);
    }
}

#[test]
fn the_reference_name_stays_the_name_when_every_direct_generator_name_is_order_dependent() {
    let set = element_set_of_map(&numbered_pair_map(), &pair_topology());

    let edge = element_named(&set, "edge(f-a/x|f-b/y)#2");

    assert_eq!(edge.reference_name(), &name("edge(f-a/x|f-b/y)#2"));
}

#[test]
fn the_reference_name_takes_the_smallest_direct_generator_name() {
    let edge =
        scope_element("edge(f-a/x|f-b/y)#1").with_direct(&["edge:f-z/rail", "edge:f-a/rail"]);

    assert_eq!(edge.reference_name(), &name("edge:f-a/rail"));
}

#[test]
fn t198_two_edges_holding_one_direct_generator_name_fail() {
    let mut infos = wire_infos();
    infos.push(scope_element("edge:f-x/rail").with_direct(&["edge:f-hel1/helix", "edge:f-x/rail"]));

    let result = try_element_set(infos);

    assert_eq!(
        result.err(),
        Some(NamingError::DuplicateName {
            name: "edge:f-hel1/helix".to_owned()
        })
    );
}

#[test]
fn t198_a_vertex_name_in_the_direct_list_of_an_edge_fails() {
    let result = try_element_set(wire_infos_with_edge_direct("vertex:f-hel1/middle"));

    assert_eq!(
        result.err(),
        Some(invalid_name(
            "vertex:f-hel1/middle",
            "it is not an edge name"
        ))
    );
}

#[test]
fn t198_a_face_rule_name_in_a_direct_list_fails() {
    let result = try_element_set(wire_infos_with_edge_direct("edge(f-a/x)"));

    assert_eq!(
        result.err(),
        Some(invalid_name(
            "edge(f-a/x)",
            "it is not a direct-generator name"
        ))
    );
}

#[test]
fn two_vertices_holding_one_direct_generator_name_fail() {
    let mut infos = wire_infos();
    infos.push(scope_element("vertex(f-a/x)").with_direct(&["vertex:f-hel1/start"]));

    let result = try_element_set(infos);

    assert_eq!(
        result.err(),
        Some(NamingError::DuplicateName {
            name: "vertex:f-hel1/start".to_owned()
        })
    );
}

#[test]
fn t199_a_merged_edge_is_named_by_the_first_of_its_direct_generator_names() {
    let map = merged_tens_map();

    assert_eq!(
        direct_of(&map, ElementKind::Edge, 1),
        ["edge:f-a/rail/e-1", "edge:f-a/rail/e-10"]
    );
    assert_eq!(
        names_of(&map, ElementKind::Edge),
        [Some("edge:f-a/rail/e-1")]
    );
}

#[test]
fn t199_with_instance_sorts_each_list_again_and_names_the_element_by_its_first_entry() {
    let copy = merged_tens_map()
        .with_instance(&id("f-lp1"), FIRST_INSTANCE, &translation(40.0, 0.0, 0.0))
        .expect("the map has an instance copy");

    assert_eq!(
        direct_of(&copy, ElementKind::Edge, 1),
        ["edge:f-a/rail/e-10@f-lp1:1", "edge:f-a/rail/e-1@f-lp1:1"]
    );
    assert_eq!(
        names_of(&copy, ElementKind::Edge),
        [Some("edge:f-a/rail/e-10@f-lp1:1")]
    );
}

#[test]
fn t199_the_copy_survives_a_json_round_trip() {
    let copy = merged_tens_map()
        .with_instance(&id("f-lp1"), FIRST_INSTANCE, &translation(40.0, 0.0, 0.0))
        .expect("the map has an instance copy");

    let json = serde_json::to_string(&copy).expect("a naming map serializes");
    let back: NamingMap = serde_json::from_str(&json).expect("a serialized map deserializes");

    assert_eq!(back, copy, "{json}");
}

#[test]
fn t199_a_pick_of_the_copied_edge_resolves_exactly() {
    let copy = merged_tens_map()
        .with_instance(&id("f-lp1"), FIRST_INSTANCE, &translation(40.0, 0.0, 0.0))
        .expect("the map has an instance copy");
    let set = element_set_of_map(&copy, &wire_topology());

    let picked = Reference::from_element(element_named(&set, "edge:f-a/rail/e-10@f-lp1:1"), &set);

    assert_resolved(
        &resolve(&picked, &set),
        &["edge:f-a/rail/e-10@f-lp1:1"],
        MatchMethod::Exact,
        1.0,
    );
}

#[test]
fn t200_index_finds_an_edge_by_a_direct_generator_name_it_holds() {
    let map = pair_map();

    assert_eq!(map.index(&name("edge:f-a/side/e-1.end")), Some(1));
    assert_eq!(map.index(&name("edge:f-a/side/e-1.start")), Some(2));
    assert_eq!(
        map.name(ElementKind::Edge, 1),
        Some(&name("edge(f-a/x|f-b/y)#2"))
    );
    assert_eq!(map.index(&name("edge:f-a/side/e-2")), None);
}

#[test]
fn t200_index_still_finds_an_edge_by_its_face_rule_name() {
    assert_eq!(pair_map().index(&name("edge(f-a/x|f-b/y)#2")), Some(1));
}
