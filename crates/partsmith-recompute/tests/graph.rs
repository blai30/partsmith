//! The dependency graph (recompute.md sections 3.1 and 3.2): tests 1 to 5 of section 6, plus
//! runner tests over test-only features that exercise every graph rule without the built-in
//! feature pipelines.

mod common;

use common::{
    Harness, bodies, doc_a, document, graph_node, id, owned, parameter, state_name, strings,
    suppressed, test_block, test_datum, test_datum_on, test_node, test_probe, upstream,
};
use partsmith_model::Document;
use partsmith_recompute::{Change, Graph};
use serde_json::json;

/// The ids `affected_by` returns for these changes.
fn affected(graph: &Graph, changes: &[Change]) -> Vec<String> {
    strings(&graph.affected_by(changes))
}

/// The ids of the direct dependents of a feature.
fn dependents(graph: &Graph, feature: &str) -> Vec<String> {
    strings(&graph.dependents(&id(feature)))
}

/// The state name of a node.
fn node_state(graph: &Graph, feature: &str) -> &'static str {
    state_name(&graph_node(graph, feature).state)
}

/// Recompute test 3's body chain: three boxes on b-1, then a fillet on b-1.
fn body_chain(box3_suppressed: bool) -> Document {
    let box3 =
        json!({"id": "f-box3", "type": "box", "body": "b-1", "size": ["2", "2", "2"], "op": "cut"});
    let box3 = if box3_suppressed {
        suppressed(box3)
    } else {
        box3
    };
    document(json!({
        "bodies": bodies(1),
        "features": [
            {"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"},
            {"id": "f-box2", "type": "box", "body": "b-1", "size": ["5", "5", "5"], "position": ["20", "0", "0"], "op": "join"},
            box3,
            {
                "id": "f-fil1", "type": "fillet", "body": "b-1",
                "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-5,10|l=10"}],
                "radius": "1"
            }
        ]
    }))
}

#[test]
#[ignore = "needs built-in sketch, extrude, box, fillet, datum_plane"]
fn t001_graph_of_doc_a() {
    let harness = Harness::builtins();

    let graph = harness.graph(&doc_a());

    assert_eq!(upstream(&graph, "f-ext1"), owned(&["output f-sk1"]));
    assert_eq!(upstream(&graph, "f-fil1"), owned(&["body b-1 f-ext1"]));
    assert!(upstream(&graph, "f-box2").is_empty());
    assert!(upstream(&graph, "f-dp1").is_empty());
    assert_eq!(
        strings(&graph_node(&graph, "f-ext1").parameters),
        owned(&["p-h"])
    );
    assert_eq!(
        strings(&graph_node(&graph, "f-dp1").parameters),
        owned(&["p-w"])
    );
    assert_eq!(dependents(&graph, "f-sk1"), owned(&["f-ext1"]));
    assert_eq!(dependents(&graph, "f-ext1"), owned(&["f-fil1"]));
    assert!(graph.independent(&id("f-box2"), &id("f-fil1")));
    assert!(!graph.independent(&id("f-sk1"), &id("f-fil1")));
}

#[test]
#[ignore = "needs built-in sketch, extrude, box, fillet, datum_plane"]
fn t002_affected_by_in_doc_a() {
    let harness = Harness::builtins();

    let graph = harness.graph(&doc_a());

    assert_eq!(
        affected(&graph, &[Change::Parameter(id("p-h"))]),
        owned(&["f-ext1", "f-fil1"])
    );
    assert_eq!(
        affected(&graph, &[Change::Feature(id("f-sk1"))]),
        owned(&["f-sk1", "f-ext1", "f-fil1"])
    );
    assert_eq!(
        affected(&graph, &[Change::Parameter(id("p-w"))]),
        owned(&["f-dp1"])
    );
    assert_eq!(
        affected(&graph, &[Change::Units]),
        owned(&["f-ext1", "f-box2", "f-fil1", "f-dp1"])
    );
}

#[test]
#[ignore = "needs built-in box, fillet"]
fn t003_body_chain_follows_the_last_active_writer() {
    let harness = Harness::builtins();

    let graph = harness.graph(&body_chain(false));
    assert_eq!(upstream(&graph, "f-fil1"), owned(&["body b-1 f-box3"]));
    assert_eq!(upstream(&graph, "f-box3"), owned(&["body b-1 f-box2"]));

    let graph = harness.graph(&body_chain(true));
    assert_eq!(upstream(&graph, "f-fil1"), owned(&["body b-1 f-box2"]));
    assert_eq!(node_state(&graph, "f-box3"), "suppressed");
}

#[test]
#[ignore = "needs built-in box"]
fn t004_parameters_include_referenced_parameters() {
    let harness = Harness::builtins();
    let document = document(json!({
        "parameters": [parameter("p-a", "a", "2 mm"), parameter("p-b", "b", "a * 2")],
        "bodies": bodies(1),
        "features": [{"id": "f-box1", "type": "box", "body": "b-1", "size": ["b", "1", "1"], "op": "new"}]
    }));

    let graph = harness.graph(&document);

    assert_eq!(
        strings(&graph_node(&graph, "f-box1").parameters),
        owned(&["p-a", "p-b"])
    );
    assert!(affected(&graph, &[Change::Parameter(id("p-a"))]).contains(&"f-box1".to_string()));
}

#[test]
#[ignore = "needs built-in box"]
fn t005_context_names_of_expressions() {
    let harness = Harness::builtins();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [{
            "id": "f-box1", "type": "box", "body": "b-1",
            "size": ["printer.fit.slip * 10", "1", "1"], "op": "new"
        }]
    }));

    let graph = harness.graph(&document);

    assert_eq!(
        graph_node(&graph, "f-box1").context_names,
        owned(&["printer.fit.slip"])
    );
}

// Runner tests over test-only features.

/// A graph fixture with datum outputs, a body chain, parameters and context names:
///
/// - `f-d1` datum at 1; `f-d2` datum on f-d1 at `w`.
/// - `f-a` (new) and `f-b` (join) write b-1; `f-c` (new) writes b-2 with size `s`.
/// - `f-p` reads b-1; `f-n` uses f-d2 and f-d1, reads b-2 and b-3, writes b-1, with values `h`
///   and `printer.layer.height + s`; `f-q` reads b-3, which nothing writes.
fn graph_fixture() -> Document {
    document(json!({
        "parameters": [
            parameter("p-h", "h", "2 mm"),
            parameter("p-w", "w", "h + 1 mm"),
            parameter("p-s", "s", "printer.fit.slip * 2"),
            parameter("p-u", "u", "7 mm")
        ],
        "bodies": bodies(3),
        "features": [
            test_datum("f-d1", "1"),
            test_block("f-a", "b-1", "2", "new"),
            test_datum_on("f-d2", "f-d1", "w"),
            test_block("f-b", "b-1", "1", "join"),
            test_block("f-c", "b-2", "s", "new"),
            test_probe("f-p", "b-1"),
            test_node("f-n", &["f-d2", "f-d1"], &["b-2", "b-3"], &["b-1"], &["h", "printer.layer.height + s"]),
            test_probe("f-q", "b-3")
        ]
    }))
}

#[test]
fn graph_has_one_node_per_record_in_timeline_order() {
    let harness = Harness::test_features();

    let graph = harness.graph(&graph_fixture());

    let nodes: Vec<String> = graph
        .nodes()
        .iter()
        .map(|node| node.feature.to_string())
        .collect();
    assert_eq!(
        nodes,
        owned(&["f-d1", "f-a", "f-d2", "f-b", "f-c", "f-p", "f-n", "f-q"])
    );
    assert!(graph.node(&id("f-nope")).is_none());
}

#[test]
fn output_edges_come_from_the_features_a_node_reads() {
    let harness = Harness::test_features();

    let graph = harness.graph(&graph_fixture());

    assert_eq!(upstream(&graph, "f-d2"), owned(&["output f-d1"]));
    assert!(upstream(&graph, "f-d1").is_empty());
}

#[test]
fn body_edges_come_from_the_last_active_writer() {
    let harness = Harness::test_features();

    let graph = harness.graph(&graph_fixture());

    assert!(upstream(&graph, "f-a").is_empty());
    assert_eq!(upstream(&graph, "f-b"), owned(&["body b-1 f-a"]));
    assert_eq!(upstream(&graph, "f-p"), owned(&["body b-1 f-b"]));
}

#[test]
fn a_body_without_a_provider_has_no_edge() {
    let harness = Harness::test_features();

    let graph = harness.graph(&graph_fixture());

    assert!(upstream(&graph, "f-q").is_empty());
}

#[test]
fn upstream_lists_features_then_read_bodies_then_written_bodies_in_declaration_order() {
    let harness = Harness::test_features();

    let graph = harness.graph(&graph_fixture());

    assert_eq!(
        upstream(&graph, "f-n"),
        owned(&["output f-d2", "output f-d1", "body b-2 f-c", "body b-1 f-b"])
    );
}

#[test]
fn output_edges_only_come_from_earlier_records() {
    let harness = Harness::test_features();
    let document = document(json!({
        "features": [
            test_datum_on("f-d1", "f-nope", "1"),
            test_datum_on("f-d2", "f-d3", "1"),
            test_datum_on("f-d3", "f-d3", "1")
        ]
    }));

    let graph = harness.graph(&document);

    assert!(upstream(&graph, "f-d1").is_empty());
    assert!(upstream(&graph, "f-d2").is_empty());
    assert!(upstream(&graph, "f-d3").is_empty());
    assert!(dependents(&graph, "f-d3").is_empty());
}

#[test]
fn suppressed_invalid_and_rolled_back_writers_are_not_providers() {
    let harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            test_block("f-a", "b-1", "2", "new"),
            suppressed(test_block("f-b", "b-1", "1", "join")),
            {"id": "f-bad", "type": "test_block", "body": "b-1", "op": "join"},
            test_probe("f-p", "b-1"),
            test_block("f-c", "b-1", "1", "join"),
            test_probe("f-q", "b-1")
        ],
        "rollback": "f-c"
    }));

    let graph = harness.graph(&document);

    assert_eq!(upstream(&graph, "f-p"), owned(&["body b-1 f-a"]));
    assert_eq!(upstream(&graph, "f-q"), owned(&["body b-1 f-a"]));
}

#[test]
fn parameters_include_transitive_references_in_document_order() {
    let harness = Harness::test_features();

    let graph = harness.graph(&graph_fixture());

    assert_eq!(
        strings(&graph_node(&graph, "f-d2").parameters),
        owned(&["p-h", "p-w"])
    );
    assert_eq!(
        strings(&graph_node(&graph, "f-n").parameters),
        owned(&["p-h", "p-s"])
    );
    assert!(graph_node(&graph, "f-d1").parameters.is_empty());
}

#[test]
fn parameters_are_listed_in_document_order_not_reference_order() {
    let harness = Harness::test_features();
    let document = document(json!({
        "parameters": [
            parameter("p-c", "c", "b + a"),
            parameter("p-z", "z", "1 mm"),
            parameter("p-a", "a", "2 mm"),
            parameter("p-b", "b", "a * 2")
        ],
        "features": [test_datum("f-d1", "c")]
    }));

    let graph = harness.graph(&document);

    assert_eq!(
        strings(&graph_node(&graph, "f-d1").parameters),
        owned(&["p-c", "p-a", "p-b"])
    );
}

#[test]
fn context_names_are_sorted_and_include_those_of_parameters() {
    let harness = Harness::test_features();

    let graph = harness.graph(&graph_fixture());

    assert_eq!(
        graph_node(&graph, "f-n").context_names,
        owned(&["printer.fit.slip", "printer.layer.height"])
    );
    assert_eq!(
        graph_node(&graph, "f-c").context_names,
        owned(&["printer.fit.slip"])
    );
    assert!(graph_node(&graph, "f-d1").context_names.is_empty());
}

#[test]
fn uses_expressions_is_true_only_for_nodes_with_expressions() {
    let harness = Harness::test_features();

    let graph = harness.graph(&graph_fixture());

    assert!(graph_node(&graph, "f-d1").uses_expressions);
    assert!(graph_node(&graph, "f-n").uses_expressions);
    assert!(!graph_node(&graph, "f-p").uses_expressions);
    assert!(!graph_node(&graph, "f-q").uses_expressions);
}

#[test]
fn dependents_lists_direct_edges_in_timeline_order() {
    let harness = Harness::test_features();

    let graph = harness.graph(&graph_fixture());

    assert_eq!(dependents(&graph, "f-d1"), owned(&["f-d2", "f-n"]));
    assert_eq!(dependents(&graph, "f-b"), owned(&["f-p", "f-n"]));
    assert_eq!(dependents(&graph, "f-a"), owned(&["f-b"]));
    assert!(dependents(&graph, "f-n").is_empty());
}

#[test]
fn affected_by_a_feature_is_the_feature_and_everything_reachable_from_it() {
    let harness = Harness::test_features();

    let graph = harness.graph(&graph_fixture());

    assert_eq!(
        affected(&graph, &[Change::Feature(id("f-a"))]),
        owned(&["f-a", "f-b", "f-p", "f-n"])
    );
    assert_eq!(
        affected(&graph, &[Change::Feature(id("f-c"))]),
        owned(&["f-c", "f-n"])
    );
    assert_eq!(
        affected(&graph, &[Change::Feature(id("f-q"))]),
        owned(&["f-q"])
    );
}

#[test]
fn affected_by_a_parameter_seeds_every_node_that_uses_it_directly_or_transitively() {
    let harness = Harness::test_features();

    let graph = harness.graph(&graph_fixture());

    assert_eq!(
        affected(&graph, &[Change::Parameter(id("p-h"))]),
        owned(&["f-d2", "f-n"])
    );
    assert_eq!(
        affected(&graph, &[Change::Parameter(id("p-w"))]),
        owned(&["f-d2", "f-n"])
    );
    assert_eq!(
        affected(&graph, &[Change::Parameter(id("p-s"))]),
        owned(&["f-c", "f-n"])
    );
    assert!(affected(&graph, &[Change::Parameter(id("p-u"))]).is_empty());
}

#[test]
fn affected_by_a_context_name_seeds_every_node_that_uses_it() {
    let harness = Harness::test_features();

    let graph = harness.graph(&graph_fixture());

    assert_eq!(
        affected(
            &graph,
            &[Change::ContextName("printer.fit.slip".to_string())]
        ),
        owned(&["f-c", "f-n"])
    );
    assert_eq!(
        affected(
            &graph,
            &[Change::ContextName("printer.layer.height".to_string())]
        ),
        owned(&["f-n"])
    );
    assert!(affected(&graph, &[Change::ContextName("printer.nozzle".to_string())]).is_empty());
}

#[test]
fn affected_by_units_seeds_every_node_with_expressions() {
    let harness = Harness::test_features();

    let graph = harness.graph(&graph_fixture());

    // f-p has no expressions but is reachable from f-a; f-q has none and is reachable from nothing.
    assert_eq!(
        affected(&graph, &[Change::Units]),
        owned(&["f-d1", "f-a", "f-d2", "f-b", "f-c", "f-p", "f-n"])
    );
}

#[test]
fn affected_by_several_changes_is_their_union_in_timeline_order() {
    let harness = Harness::test_features();

    let graph = harness.graph(&graph_fixture());

    assert_eq!(
        affected(
            &graph,
            &[Change::Feature(id("f-q")), Change::Parameter(id("p-w"))]
        ),
        owned(&["f-d2", "f-n", "f-q"])
    );
}

#[test]
fn affected_by_returns_only_active_nodes() {
    let harness = Harness::test_features();
    let document = document(json!({
        "features": [
            test_datum("f-d1", "1"),
            suppressed(test_datum_on("f-d2", "f-d1", "1")),
            test_datum_on("f-d3", "f-d1", "1"),
            test_datum_on("f-d4", "f-d1", "1")
        ],
        "rollback": "f-d4"
    }));

    let graph = harness.graph(&document);

    assert_eq!(
        affected(&graph, &[Change::Feature(id("f-d1"))]),
        owned(&["f-d1", "f-d3"])
    );
    assert_eq!(affected(&graph, &[Change::Units]), owned(&["f-d1", "f-d3"]));
}

#[test]
fn independent_is_true_only_when_neither_node_reaches_the_other() {
    let harness = Harness::test_features();

    let graph = harness.graph(&graph_fixture());

    assert!(graph.independent(&id("f-c"), &id("f-b")));
    assert!(graph.independent(&id("f-d1"), &id("f-c")));
    assert!(graph.independent(&id("f-q"), &id("f-n")));
    assert!(!graph.independent(&id("f-a"), &id("f-n")));
    assert!(!graph.independent(&id("f-n"), &id("f-a")));
    assert!(!graph.independent(&id("f-d1"), &id("f-d2")));
}

/// Records in every node state, with the rollback marker at f-d2.
fn states_fixture() -> Document {
    document(json!({
        "bodies": bodies(1),
        "features": [
            test_datum("f-d1", "1"),
            suppressed(test_datum("f-x", "2")),
            {"id": "f-g", "type": "gear_demo", "teeth": 12},
            {"id": "f-bad", "type": "test_block", "body": "b-1", "op": "new"},
            test_datum("f-d2", "3"),
            suppressed(test_datum("f-d3", "4"))
        ],
        "rollback": "f-d2"
    }))
}

#[test]
fn node_states_follow_rollback_suppression_and_validity() {
    let harness = Harness::test_features();

    let graph = harness.graph(&states_fixture());

    assert_eq!(node_state(&graph, "f-d1"), "active");
    assert_eq!(node_state(&graph, "f-x"), "suppressed");
    assert_eq!(node_state(&graph, "f-g"), "invalid");
    assert_eq!(node_state(&graph, "f-bad"), "invalid");
    assert_eq!(node_state(&graph, "f-d2"), "rolled_back");
    // Rollback is checked first: a suppressed record after the marker is rolled back.
    assert_eq!(node_state(&graph, "f-d3"), "rolled_back");
}

#[test]
fn a_stop_point_overrides_the_rollback_marker() {
    let harness = Harness::test_features();

    let graph = harness.graph_until(&states_fixture(), "f-d2");

    assert_eq!(node_state(&graph, "f-d1"), "active");
    assert_eq!(node_state(&graph, "f-d2"), "active");
    assert_eq!(node_state(&graph, "f-d3"), "rolled_back");
}

#[test]
fn records_after_the_stop_point_are_rolled_back_whatever_their_own_state() {
    let harness = Harness::test_features();

    let graph = harness.graph_until(&states_fixture(), "f-d1");

    assert_eq!(node_state(&graph, "f-d1"), "active");
    assert_eq!(node_state(&graph, "f-x"), "rolled_back");
    assert_eq!(node_state(&graph, "f-g"), "rolled_back");
    assert_eq!(node_state(&graph, "f-bad"), "rolled_back");
    assert_eq!(node_state(&graph, "f-d2"), "rolled_back");
}

#[test]
fn invalid_nodes_have_no_dependencies() {
    let harness = Harness::test_features();

    let graph = harness.graph(&states_fixture());

    for feature in ["f-g", "f-bad"] {
        let node = graph_node(&graph, feature);
        assert!(node.upstream.is_empty(), "{feature} has upstream edges");
        assert!(node.parameters.is_empty(), "{feature} has parameters");
        assert!(node.context_names.is_empty(), "{feature} has context names");
        assert!(!node.uses_expressions, "{feature} uses expressions");
    }
}

#[test]
fn graph_and_plan_never_modify_the_cache() {
    let mut harness = Harness::test_features();
    harness.run(&graph_fixture());
    let cached = harness.cache_len();
    let other = document(json!({
        "bodies": bodies(1),
        "features": [test_block("f-z", "b-1", "9", "new")]
    }));

    harness.graph(&other);
    harness.plan(&other);

    assert_eq!(harness.cache_len(), cached);
}
