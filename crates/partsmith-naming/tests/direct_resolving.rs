//! Acceptance tests for resolving direct-generator names, the references that picks write and the
//! names-only view (spec section 6, tests 176 to 191; sections 2.4, 2.7, 3.12.4 and 3.12.5).

use crate::common;

use common::{
    ScopeElement, TOP_FRONT_EDGE_FINGERPRINT, assert_candidate, assert_candidate_place,
    assert_follow, assert_guess, assert_invalid_reference, assert_lost, assert_resolved,
    assert_via, box_scope, build, candidate_names, element_named, element_set, element_set_of_map,
    merged_wire_map, missing, name, numbered_pair_map, pair_map, pair_scope, pair_sources,
    pair_topology, reference, scope_element, single_pair_edge_map, single_pair_edge_topology,
    split_wire_map, split_wire_topology, wire_map, wire_scope, wire_topology,
};
use partsmith_naming::{
    ElementInfo, ElementKind, ElementSet, Follow, GeometryKind, MatchMethod, NameSet, NamingError,
    Reference, resolve,
};
use proptest::prelude::*;

/// The fingerprint of the WIRE edge with adjacency counts (spec test 174).
const WIRE_EDGE_FINGERPRINT: &str = "line|n=1,0,0|c=5,0,0|l=10|adj=0,2";

/// The fingerprint of edge 1 of the PAIR map, at x = 3 (spec test 187).
const PAIR_FIRST_EDGE_FINGERPRINT: &str = "line|n=0,0,1|c=3,0,5|l=10|adj=2,2";

/// The fingerprint of edge 2 of the PAIR map, at x = 1 (spec test 177).
const PAIR_SECOND_EDGE_FINGERPRINT: &str = "line|n=0,0,1|c=1,0,5|l=10|adj=2,2";

/// The fingerprint of the second piece of test 161's map, at x = 7.5 (spec tests 179 and 180).
const SECOND_PIECE_FINGERPRINT: &str = "line|n=1,0,0|c=7.5,0,0|l=5|adj=0,2";

/// The fingerprint of the first piece of test 161's map, at x = 2.5 (spec test 179).
const FIRST_PIECE_FINGERPRINT: &str = "line|n=1,0,0|c=2.5,0,0|l=5|adj=0,2";

/// The direct-generator name bodies the property scopes hold, before their `edge:` or `vertex:`
/// prefix: whole names, pieces of pieces, a copy and a name of another feature.
const PROPERTY_BODIES: [&str; 8] = [
    "f-a/rail",
    "f-a/rail#1",
    "f-a/rail#2",
    "f-a/rail#2#1",
    "f-a/rail/e-1",
    "f-a/rail/e-1@f-p:1",
    "f-b/crest",
    "f-b/crest#1",
];

/// Query bodies that no property scope holds: a lost name and a lost piece.
const PROPERTY_LOST_BODIES: [&str; 2] = ["f-c/gone", "f-a/rail#3"];

/// The most edges and the most vertices of a property scope.
const PROPERTY_ELEMENTS: usize = 5;

/// The element set of test 161's map.
fn split_wire_scope() -> ElementSet {
    element_set_of_map(&split_wire_map(), &split_wire_topology())
}

/// The element set of test 163's map.
fn merged_rails_scope() -> ElementSet {
    element_set_of_map(
        &merged_wire_map("edge:f-a/rail/e-1", "edge:f-b/rail/e-2"),
        &wire_topology(),
    )
}

/// The PAIR scope without direct sources (spec test 184).
fn pair_scope_without_direct_sources() -> ElementSet {
    element_set_of_map(&build(pair_sources(), &pair_topology()), &pair_topology())
}

/// The same set with its infos in reverse order.
fn reversed(set: &ElementSet) -> ElementSet {
    element_set(set.elements().iter().rev().cloned().collect())
}

/// The scopes and queries of tests 176 to 184.
fn resolver_cases() -> Vec<(ElementSet, Reference)> {
    vec![
        (
            wire_scope(),
            reference("edge:f-hel1/helix", WIRE_EDGE_FINGERPRINT),
        ),
        (
            pair_scope(),
            reference("edge:f-a/side/e-1.end", PAIR_SECOND_EDGE_FINGERPRINT),
        ),
        (
            split_wire_scope(),
            reference("edge:f-hel1/helix", WIRE_EDGE_FINGERPRINT),
        ),
        (
            split_wire_scope(),
            reference("edge:f-hel1/helix#1", SECOND_PIECE_FINGERPRINT),
        ),
        (
            split_wire_scope(),
            reference("edge:f-hel1/helix#1", FIRST_PIECE_FINGERPRINT),
        ),
        (
            wire_scope(),
            reference("edge:f-hel1/helix#2", SECOND_PIECE_FINGERPRINT),
        ),
        (
            wire_scope(),
            reference("edge:f-gone/helix", WIRE_EDGE_FINGERPRINT),
        ),
        (
            wire_scope(),
            reference("edge:f-hel1/coil", WIRE_EDGE_FINGERPRINT),
        ),
        (
            merged_rails_scope(),
            reference("edge:f-b/rail/e-2", WIRE_EDGE_FINGERPRINT),
        ),
        (
            merged_rails_scope(),
            reference("edge:f-a/rail/e-1", WIRE_EDGE_FINGERPRINT),
        ),
        (
            box_scope(),
            reference("edge:f-box1/front", TOP_FRONT_EDGE_FINGERPRINT),
        ),
        (
            pair_scope(),
            reference("edge(f-a/x|f-b/y)#2", PAIR_FIRST_EDGE_FINGERPRINT),
        ),
    ]
}

/// A scope of direct-named edges `edge:f-a/rail#1` at x = 0 and the pieces `#2#1` and `#2#2` of
/// `edge:f-a/rail#2` at x = 10 and 11, for the guess rules of spec section 3.12.4.
fn piece_scope() -> ElementSet {
    let edge = |text: &str, x: u32| {
        scope_element(text)
            .with_fingerprint(&format!("line|n=1,0,0|c={x},0,0|l=1"))
            .with_direct(&[text])
    };
    element_set(vec![
        edge("edge:f-a/rail#1", 0),
        edge("edge:f-a/rail#2#1", 10),
        edge("edge:f-a/rail#2#2", 11),
    ])
}

/// One random edge or vertex of a property scope: whether it is named by its first
/// direct-generator name when it holds one, and the x of its centroid.
#[derive(Debug, Clone)]
struct RandomElement {
    named_direct: bool,
    x: u32,
}

/// A random property scope: its edges and vertices, and for each property body of each kind the
/// element that holds it, if any (an index taken modulo the element count).
#[derive(Debug, Clone)]
struct RandomScope {
    edges: Vec<RandomElement>,
    vertices: Vec<RandomElement>,
    edge_holders: Vec<Option<usize>>,
    vertex_holders: Vec<Option<usize>>,
}

impl RandomScope {
    /// The element infos of the scope: two faces, then the edges, then the vertices. Each
    /// direct-generator name is held by at most one element of its kind, each list is sorted and
    /// distinct, and an element named by a direct-generator name holds it first.
    fn infos(&self) -> Vec<ElementInfo> {
        let faces = [
            scope_element("f-a/x").with_adjacency(&["f-b/y"]),
            scope_element("f-b/y").with_adjacency(&["f-a/x"]),
        ];
        faces
            .into_iter()
            .chain(Self::kind_infos(
                ElementKind::Edge,
                &self.edges,
                &self.edge_holders,
            ))
            .chain(Self::kind_infos(
                ElementKind::Vertex,
                &self.vertices,
                &self.vertex_holders,
            ))
            .collect()
    }

    /// The infos of the elements of one kind.
    fn kind_infos(
        kind: ElementKind,
        elements: &[RandomElement],
        holders: &[Option<usize>],
    ) -> Vec<ElementInfo> {
        (0..elements.len())
            .map(|position| {
                let element = &elements[position];
                let mut held: Vec<String> = PROPERTY_BODIES
                    .iter()
                    .zip(holders)
                    .filter(|(_, holder)| {
                        holder.is_some_and(|holder| holder % elements.len() == position)
                    })
                    .map(|(body, _)| property_name(kind, body))
                    .collect();
                held.sort();
                let face_rule_name = match kind {
                    ElementKind::Edge => format!("edge(f-a/x|f-b/y)#{}", position + 1),
                    _ => format!("vertex(f-a/x|f-b/y)#{}", position + 1),
                };
                let named_direct = element.named_direct && !held.is_empty();
                let info_name = if named_direct {
                    held[0].clone()
                } else {
                    face_rule_name
                };
                let held_texts: Vec<&str> = held.iter().map(String::as_str).collect();
                let info = scope_element(&info_name)
                    .with_fingerprint(&property_fingerprint(kind, element.x))
                    .with_direct(&held_texts);
                if named_direct {
                    info.with_adjacency(&[])
                } else {
                    info.with_adjacency(&["f-a/x", "f-b/y"])
                }
            })
            .collect()
    }
}

/// The direct-generator name of a property body for elements of `kind`.
fn property_name(kind: ElementKind, body: &str) -> String {
    match kind {
        ElementKind::Edge => format!("edge:{body}"),
        _ => format!("vertex:{body}"),
    }
}

/// The fingerprint of a property element of `kind` at x.
fn property_fingerprint(kind: ElementKind, x: u32) -> String {
    match kind {
        ElementKind::Edge => format!("line|n=1,0,0|c={x},0,0|l=1"),
        _ => format!("point|c={x},0,0"),
    }
}

/// Every direct-generator query of the property test, with a fingerprint at x.
fn property_queries(x: u32) -> Vec<Reference> {
    [ElementKind::Edge, ElementKind::Vertex]
        .into_iter()
        .flat_map(|kind| {
            PROPERTY_BODIES
                .iter()
                .chain(&PROPERTY_LOST_BODIES)
                .map(move |body| {
                    reference(&property_name(kind, body), &property_fingerprint(kind, x))
                })
        })
        .collect()
}

/// A random edge or vertex.
fn random_element() -> impl Strategy<Value = RandomElement> {
    (any::<bool>(), 0_u32..4).prop_map(|(named_direct, x)| RandomElement { named_direct, x })
}

/// A random property scope.
fn random_scope() -> impl Strategy<Value = RandomScope> {
    let holders = || prop::collection::vec(prop::option::of(0..PROPERTY_ELEMENTS), PROPERTY_BODIES.len());
    (
        prop::collection::vec(random_element(), 1..=PROPERTY_ELEMENTS),
        prop::collection::vec(random_element(), 1..=PROPERTY_ELEMENTS),
        holders(),
        holders(),
    )
        .prop_map(|(edges, vertices, edge_holders, vertex_holders)| RandomScope {
            edges,
            vertices,
            edge_holders,
            vertex_holders,
        })
}

/// The infos of a random scope in their order and in a shuffled order.
fn infos_in_two_orders() -> impl Strategy<Value = (Vec<ElementInfo>, Vec<ElementInfo>)> {
    random_scope().prop_flat_map(|scope| {
        let infos = scope.infos();
        (Just(infos.clone()), Just(infos).prop_shuffle())
    })
}

#[test]
fn t176_a_direct_generator_query_resolves_exactly_to_its_holder() {
    let resolution = resolve(
        &reference("edge:f-hel1/helix", WIRE_EDGE_FINGERPRINT),
        &wire_scope(),
    );

    assert_resolved(
        &resolution,
        &["edge:f-hel1/helix"],
        MatchMethod::Exact,
        1.0,
    );
    assert_follow(&resolution, None);
    assert_via(&resolution, &[]);
}

#[test]
fn t177_an_alias_resolves_exactly_whatever_its_fingerprint() {
    let resolution = resolve(
        &reference("edge:f-a/side/e-1.end", PAIR_SECOND_EDGE_FINGERPRINT),
        &pair_scope(),
    );

    assert_resolved(
        &resolution,
        &["edge(f-a/x|f-b/y)#2"],
        MatchMethod::Exact,
        1.0,
    );
}

#[test]
fn t178_a_split_name_resolves_to_its_pieces() {
    let resolution = resolve(
        &reference("edge:f-hel1/helix", WIRE_EDGE_FINGERPRINT),
        &split_wire_scope(),
    );

    assert_resolved(
        &resolution,
        &["edge:f-hel1/helix#1", "edge:f-hel1/helix#2"],
        MatchMethod::History,
        1.0,
    );
    assert_follow(&resolution, None);
    assert_via(&resolution, &[]);
}

#[test]
fn t179_a_numbered_query_is_a_guess_when_its_sibling_fits_clearly_better() {
    let resolution = resolve(
        &reference("edge:f-hel1/helix#1", SECOND_PIECE_FINGERPRINT),
        &split_wire_scope(),
    );

    assert_guess(
        &resolution,
        &["edge:f-hel1/helix#1"],
        0.754175,
        &["edge:f-hel1/helix#2"],
    );
}

#[test]
fn t179_a_numbered_query_that_fits_its_piece_resolves_exactly() {
    let resolution = resolve(
        &reference("edge:f-hel1/helix#1", FIRST_PIECE_FINGERPRINT),
        &split_wire_scope(),
    );

    assert_resolved(
        &resolution,
        &["edge:f-hel1/helix#1"],
        MatchMethod::Exact,
        1.0,
    );
}

#[test]
fn t180_a_lost_piece_below_the_threshold_is_missing() {
    let resolution = resolve(
        &reference("edge:f-hel1/helix#2", SECOND_PIECE_FINGERPRINT),
        &wire_scope(),
    );

    let (lost, candidates) = missing(&resolution);
    assert_lost(lost, &[], &["edge:f-hel1/helix#2"], false);
    assert_eq!(candidates.len(), 1, "{resolution:?}");
    assert_candidate(&candidates[0], "edge:f-hel1/helix", true, 2.5, 0.735873);
}

#[test]
fn t181_a_name_no_element_holds_is_lost() {
    let resolution = resolve(
        &reference("edge:f-gone/helix", WIRE_EDGE_FINGERPRINT),
        &wire_scope(),
    );

    let (lost, candidates) = missing(&resolution);
    assert_lost(lost, &["edge:f-gone/helix"], &[], false);
    assert_eq!(candidates.len(), 1, "{resolution:?}");
    assert_candidate(&candidates[0], "edge:f-hel1/helix", false, 0.0, 1.0);
}

#[test]
fn t182_a_candidate_of_the_same_feature_has_meaning() {
    let resolution = resolve(
        &reference("edge:f-hel1/coil", WIRE_EDGE_FINGERPRINT),
        &wire_scope(),
    );

    let (lost, candidates) = missing(&resolution);
    assert_lost(lost, &["edge:f-hel1/coil"], &[], false);
    assert_eq!(candidate_names(candidates), ["edge:f-hel1/helix"]);
    assert_candidate_place(&candidates[0], "edge:f-hel1/helix", true, 0.0);
}

#[test]
fn t183_a_name_merged_into_another_element_follows_with_merged() {
    let resolution = resolve(
        &reference("edge:f-b/rail/e-2", WIRE_EDGE_FINGERPRINT),
        &merged_rails_scope(),
    );

    assert_resolved(
        &resolution,
        &["edge:f-a/rail/e-1"],
        MatchMethod::History,
        1.0,
    );
    assert_follow(&resolution, Some(Follow::Merged));
    assert_via(&resolution, &[]);
}

#[test]
fn t183_the_first_name_of_a_merged_element_resolves_exactly() {
    let resolution = resolve(
        &reference("edge:f-a/rail/e-1", WIRE_EDGE_FINGERPRINT),
        &merged_rails_scope(),
    );

    assert_resolved(
        &resolution,
        &["edge:f-a/rail/e-1"],
        MatchMethod::Exact,
        1.0,
    );
    assert_follow(&resolution, None);
}

#[test]
fn t184_a_direct_generator_query_never_matches_a_face_rule_name() {
    let resolution = resolve(
        &reference("edge:f-box1/front", TOP_FRONT_EDGE_FINGERPRINT),
        &box_scope(),
    );

    let (lost, candidates) = missing(&resolution);
    assert_lost(lost, &["edge:f-box1/front"], &[], false);
    assert_candidate(
        &candidates[0],
        "edge(f-box1/front|f-box1/top)",
        true,
        0.0,
        1.0,
    );
}

#[test]
fn t184_a_face_rule_query_resolves_by_the_face_rule_beside_direct_generator_names() {
    let query = reference("edge(f-a/x|f-b/y)#2", PAIR_FIRST_EDGE_FINGERPRINT);

    for scope in [pair_scope(), pair_scope_without_direct_sources()] {
        assert_resolved(
            &resolve(&query, &scope),
            &["edge(f-a/x|f-b/y)#2"],
            MatchMethod::Exact,
            1.0,
        );
    }
}

#[test]
fn a_face_rule_query_never_matches_a_direct_generator_name() {
    let resolution = resolve(
        &reference("edge(f-hel1/helix)", WIRE_EDGE_FINGERPRINT),
        &wire_scope(),
    );

    let (lost, _) = missing(&resolution);
    assert_lost(lost, &["f-hel1/helix"], &[], false);
}

#[test]
fn pieces_of_a_numbered_query_are_a_guess_when_another_holder_fits_clearly_better() {
    let resolution = resolve(
        &reference("edge:f-a/rail#2", "line|n=1,0,0|c=0,0,0|l=1"),
        &piece_scope(),
    );

    assert_guess(
        &resolution,
        &["edge:f-a/rail#2#1", "edge:f-a/rail#2#2"],
        0.562520,
        &["edge:f-a/rail#1"],
    );
}

#[test]
fn a_lost_piece_above_the_threshold_is_a_guess_on_the_best_holder() {
    let resolution = resolve(
        &reference("edge:f-a/rail#3", "line|n=1,0,0|c=0,0,0|l=1"),
        &piece_scope(),
    );

    assert_guess(
        &resolution,
        &["edge:f-a/rail#1"],
        1.0,
        &["edge:f-a/rail#2#1", "edge:f-a/rail#2#2"],
    );
}

#[test]
fn t185_each_query_resolves_alike_against_reversed_infos() {
    for (set, query) in resolver_cases() {
        assert_eq!(
            resolve(&query, &set),
            resolve(&query, &reversed(&set)),
            "{query:?}"
        );
    }
}

proptest! {
    #[test]
    fn t185_direct_generator_queries_do_not_depend_on_the_order_of_the_infos(
        (infos, shuffled) in infos_in_two_orders(),
        x in 0_u32..4,
    ) {
        let set = element_set(infos);
        let shuffled_set = element_set(shuffled);

        for query in property_queries(x) {
            prop_assert_eq!(
                resolve(&query, &set),
                resolve(&query, &shuffled_set),
                "{:?}",
                query
            );
        }
    }
}

#[test]
fn t186_a_pick_of_the_wire_edge_is_its_name_and_fingerprint() {
    let scope = wire_scope();

    let picked = Reference::from_element(element_named(&scope, "edge:f-hel1/helix"), &scope);

    assert_eq!(
        serde_json::to_string(&picked).expect("a reference serializes"),
        r#"{"q":"edge:f-hel1/helix","fp":"line|n=1,0,0|c=5,0,0|l=10|adj=0,2"}"#
    );
}

#[test]
fn t187_a_pick_of_an_order_dependent_edge_writes_its_first_direct_generator_name() {
    let scope = pair_scope();

    let picked = Reference::from_element(element_named(&scope, "edge(f-a/x|f-b/y)#2"), &scope);

    assert_eq!(
        picked,
        reference("edge:f-a/side/e-1.end", PAIR_FIRST_EDGE_FINGERPRINT)
    );
    assert!(picked.anchors.is_empty());
    assert!(picked.merged.is_empty());
}

#[test]
fn t187_the_pick_resolves_exactly() {
    let scope = pair_scope();
    let picked = Reference::from_element(element_named(&scope, "edge(f-a/x|f-b/y)#2"), &scope);

    assert_resolved(
        &resolve(&picked, &scope),
        &["edge(f-a/x|f-b/y)#2"],
        MatchMethod::Exact,
        1.0,
    );
}

#[test]
fn t187_a_pick_of_an_edge_without_number_writes_its_face_rule_name() {
    let scope = element_set_of_map(&single_pair_edge_map(), &single_pair_edge_topology());

    let picked = Reference::from_element(element_named(&scope, "edge(f-a/x|f-b/y)"), &scope);

    assert_eq!(picked.query, name("edge(f-a/x|f-b/y)"));
}

#[test]
fn t188_every_direct_generator_name_of_the_pair_edges_is_numbered() {
    let map = numbered_pair_map();

    assert_eq!(
        map.direct(ElementKind::Edge, 1),
        [name("edge:f-a/side/e-1#2")]
    );
    assert_eq!(
        map.direct(ElementKind::Edge, 2),
        [name("edge:f-a/side/e-1#1")]
    );
}

#[test]
fn t188_a_pick_keeps_the_face_rule_name_when_every_direct_generator_name_is_order_dependent() {
    let scope = element_set_of_map(&numbered_pair_map(), &pair_topology());

    let picked = Reference::from_element(element_named(&scope, "edge(f-a/x|f-b/y)#2"), &scope);

    assert_eq!(
        picked,
        reference("edge(f-a/x|f-b/y)#2", PAIR_FIRST_EDGE_FINGERPRINT)
    );
}

#[test]
fn t189_a_direct_generator_query_takes_no_anchors() {
    assert_invalid_reference(
        r#"{"q": "edge:f-h/helix#2", "fp": "line|c=0,0,0", "nb": {"edge:f-h/helix#2": ["f-a/x"]}}"#,
        "edge:f-h/helix#2",
        "a direct-generator name takes no anchors",
    );
}

#[test]
fn t189_a_direct_generator_query_takes_no_merged_names() {
    assert_invalid_reference(
        r#"{"q": "edge:f-h/helix", "fp": "line|c=0,0,0", "merged": {"f-a/x": ["f-b/y"]}}"#,
        "edge:f-h/helix",
        "a direct-generator name takes no merged names",
    );
}

#[test]
fn t189_anchors_are_reported_before_merged_names() {
    assert_invalid_reference(
        r#"{"q": "edge:f-h/helix#2", "fp": "line|c=0,0,0", "nb": {"edge:f-h/helix#2": ["f-a/x"]}, "merged": {"f-a/x": ["f-b/y"]}}"#,
        "edge:f-h/helix#2",
        "a direct-generator name takes no anchors",
    );
}

#[test]
fn t190_an_edge_query_with_a_point_fingerprint_is_a_kind_mismatch() {
    let expected = NamingError::KindMismatch {
        name: "edge:f-hel1/helix".to_owned(),
        kind: GeometryKind::Point,
    };

    let error =
        serde_json::from_str::<Reference>(r#"{"q": "edge:f-hel1/helix", "fp": "point|c=0,0,0"}"#)
            .expect_err("an edge query with a point fingerprint must not deserialize");

    assert!(
        error.to_string().contains(&expected.to_string()),
        "the error must be KindMismatch ({expected}), got: {error}"
    );
}

#[test]
fn t190_a_vertex_reference_round_trips_byte_for_byte() {
    let text = r#"{"q":"vertex:f-hel1/start","fp":"point|c=0,0,0"}"#;

    let parsed: Reference = serde_json::from_str(text).expect("the reference deserializes");

    assert_eq!(
        serde_json::to_string(&parsed).expect("a reference serializes"),
        text
    );
}

#[test]
fn t190_a_vertex_reference_in_the_spec_spacing_reads_its_query_and_fingerprint() {
    let parsed: Reference =
        serde_json::from_str(r#"{"q": "vertex:f-hel1/start", "fp": "point|c=0,0,0"}"#)
            .expect("the reference deserializes");

    assert_eq!(parsed, reference("vertex:f-hel1/start", "point|c=0,0,0"));
}

#[test]
fn t191_the_wire_map_carries_its_edge_name_and_nothing_else() {
    let set = NameSet::new(&wire_map());

    assert!(set.carries(&name("edge:f-hel1/helix")));
    assert!(!set.carries(&name("edge:f-hel1/helix#1")));
    assert!(!set.carries(&name("edge:f-gone/helix")));
}

#[test]
fn t191_test_161s_map_carries_the_whole_name_and_its_pieces() {
    let set = NameSet::new(&split_wire_map());

    assert!(set.carries(&name("edge:f-hel1/helix")));
    assert!(set.carries(&name("edge:f-hel1/helix#2")));
}

#[test]
fn t191_the_pair_map_carries_an_alias() {
    assert!(NameSet::new(&pair_map()).carries(&name("edge:f-a/side/e-1.end")));
}

#[test]
fn carries_reads_vertices_and_never_matches_a_copy() {
    let set = NameSet::new(&wire_map());

    assert!(set.carries(&name("vertex:f-hel1/start")));
    assert!(!set.carries(&name("edge:f-hel1/helix@f-p:1")));
    assert!(!set.carries(&name("vertex:f-hel1/helix")));
}
