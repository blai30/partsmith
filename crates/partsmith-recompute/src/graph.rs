//! The dependency graph of a timeline and its queries (recompute.md section 3.2).

use std::collections::BTreeMap;

use partsmith_model::Id;

/// The dependency graph of one document. Edges only go from earlier to later records, so it is
/// acyclic by construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Graph {
    /// One node per feature record, in timeline order.
    nodes: Vec<Node>,
    /// Timeline position of each feature id (the first record with that id).
    positions: BTreeMap<Id, usize>,
    /// Positions of the nodes with a direct edge from each node, ascending, without repeats.
    dependents: Vec<Vec<usize>>,
}

/// One feature in the graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    /// The feature.
    pub feature: Id,
    /// Whether it runs.
    pub state: NodeState,
    /// Direct upstream edges, features first in declaration order, then bodies (read, then
    /// written) in declaration order.
    pub upstream: Vec<Edge>,
    /// Parameters its expressions use, directly or through other parameters, in document order.
    pub parameters: Vec<Id>,
    /// Dotted names its expressions use (directly or through parameters), sorted.
    pub context_names: Vec<String>,
    /// True when it evaluates at least one expression.
    pub uses_expressions: bool,
}

/// Whether a node runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeState {
    /// Runs.
    Active,
    /// Suppressed by the user.
    Suppressed,
    /// At or after the rollback marker, or after the stop point.
    RolledBack,
    /// Unknown type or invalid params: its dependencies are unknown, it runs nothing and writes nothing.
    Invalid,
}

/// A direct dependency of a node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    /// The upstream feature.
    pub feature: Id,
    /// Why the node depends on it.
    pub via: EdgeKind,
}

/// Why a node depends on an upstream feature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    /// The node reads the upstream feature's output (a sketch or datum).
    Output,
    /// The upstream feature is the last active writer of this body before the node.
    Body(Id),
}

/// A change to a document, for `Graph::affected_by`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// The feature's own record changed (params, suppression). Query the graph in which the
    /// feature is active: after unsuppressing or adding it, before suppressing or removing it.
    Feature(Id),
    /// A parameter's expression changed.
    Parameter(Id),
    /// A context value changed, for example "printer.fit.slip".
    ContextName(String),
    /// The document units changed.
    Units,
}

impl Graph {
    /// A graph of `nodes` in timeline order. Every upstream edge must name an earlier node.
    #[cfg(test)]
    pub(crate) fn new(nodes: Vec<Node>) -> Graph {
        let positions = first_positions(nodes.iter().map(|node| &node.feature));
        Graph::with_positions(nodes, positions)
    }

    /// A graph of `nodes` in timeline order, where `positions` is the position of the first node
    /// with each feature id. Every upstream edge must name an earlier node.
    pub(crate) fn with_positions(nodes: Vec<Node>, positions: BTreeMap<Id, usize>) -> Graph {
        let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
        for (position, node) in nodes.iter().enumerate() {
            for edge in &node.upstream {
                if let Some(&upstream) = positions.get(&edge.feature) {
                    let list = &mut dependents[upstream];
                    // Positions arrive in ascending order, so a repeat is always the last entry.
                    if list.last() != Some(&position) {
                        list.push(position);
                    }
                }
            }
        }
        Graph {
            nodes,
            positions,
            dependents,
        }
    }

    /// One node per feature record, in timeline order.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// The node of a feature.
    pub fn node(&self, feature: &Id) -> Option<&Node> {
        self.position(feature).map(|position| &self.nodes[position])
    }

    /// Features with a direct edge from `feature`, in timeline order.
    pub fn dependents(&self, feature: &Id) -> Vec<Id> {
        self.position(feature)
            .map(|position| self.ids(&self.dependents[position]))
            .unwrap_or_default()
    }

    /// Every active feature that a change can affect (directly or transitively), in timeline order.
    ///
    /// The changed features themselves, the active nodes that use a changed parameter or context
    /// name, and for a units change every active node that evaluates an expression, are the
    /// seeds; the result is every active node among the seeds and the nodes reachable from them.
    ///
    /// A graph describes one version of the document, and a node that is not active provides no
    /// body, so later readers of its bodies are not its dependents. For an edit that makes a
    /// feature active (unsuppressing, adding, moving it before the rollback marker), query the
    /// graph after the edit; for one that makes it inactive, the graph before the edit; for an
    /// edit that changes what a feature reads or writes, take the union of both results.
    pub fn affected_by(&self, changes: &[Change]) -> Vec<Id> {
        let mut reached = vec![false; self.nodes.len()];
        let mut pending: Vec<usize> = (0..self.nodes.len())
            .filter(|&position| {
                changes
                    .iter()
                    .any(|change| is_seed(&self.nodes[position], change))
            })
            .collect();
        for &seed in &pending {
            reached[seed] = true;
        }
        while let Some(position) = pending.pop() {
            for &dependent in &self.dependents[position] {
                if !reached[dependent] {
                    reached[dependent] = true;
                    pending.push(dependent);
                }
            }
        }
        let affected: Vec<usize> = (0..self.nodes.len())
            .filter(|&position| {
                reached[position] && self.nodes[position].state == NodeState::Active
            })
            .collect();
        self.ids(&affected)
    }

    /// True when neither feature can reach the other through edges. A feature is never
    /// independent of itself; a feature that is not in the graph reaches nothing.
    pub fn independent(&self, first: &Id, second: &Id) -> bool {
        if first == second {
            return false;
        }
        match (self.position(first), self.position(second)) {
            (Some(one), Some(other)) => !self.reaches(one.min(other), one.max(other)),
            _ => true,
        }
    }

    /// Timeline position of a feature.
    pub(crate) fn position(&self, feature: &Id) -> Option<usize> {
        self.positions.get(feature).copied()
    }

    /// True when a path of edges leads from the node at `from` to the later node at `to`.
    fn reaches(&self, from: usize, to: usize) -> bool {
        let mut visited = vec![false; to + 1];
        let mut pending = vec![from];
        while let Some(position) = pending.pop() {
            for &dependent in &self.dependents[position] {
                if dependent == to {
                    return true;
                }
                // Edges only go forward, so nodes after `to` cannot lead back to it.
                if dependent < to && !visited[dependent] {
                    visited[dependent] = true;
                    pending.push(dependent);
                }
            }
        }
        false
    }

    /// The feature ids of nodes at these positions.
    fn ids(&self, positions: &[usize]) -> Vec<Id> {
        positions
            .iter()
            .map(|&position| self.nodes[position].feature.clone())
            .collect()
    }
}

/// The position of the first of `ids` with each id.
pub(crate) fn first_positions<'a>(ids: impl Iterator<Item = &'a Id>) -> BTreeMap<Id, usize> {
    let mut positions = BTreeMap::new();
    for (position, id) in ids.enumerate() {
        positions.entry(id.clone()).or_insert(position);
    }
    positions
}

/// True when `change` seeds `affected_by` at `node` (recompute.md section 3.2).
fn is_seed(node: &Node, change: &Change) -> bool {
    let active = node.state == NodeState::Active;
    match change {
        Change::Feature(feature) => node.feature == *feature,
        Change::Parameter(parameter) => active && node.parameters.contains(parameter),
        Change::ContextName(name) => active && node.context_names.contains(name),
        Change::Units => active && node.uses_expressions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::id;

    /// A node with these upstream edges (`"output f-x"` or `"body b-1 f-x"`), active unless stated.
    fn node(feature: &str, upstream: &[&str]) -> Node {
        Node {
            feature: id(feature),
            state: NodeState::Active,
            upstream: upstream
                .iter()
                .map(
                    |edge| match edge.split(' ').collect::<Vec<_>>().as_slice() {
                        ["output", feature] => Edge {
                            feature: id(feature),
                            via: EdgeKind::Output,
                        },
                        ["body", body, feature] => Edge {
                            feature: id(feature),
                            via: EdgeKind::Body(id(body)),
                        },
                        _ => panic!("bad edge {edge}"),
                    },
                )
                .collect(),
            parameters: Vec::new(),
            context_names: Vec::new(),
            uses_expressions: false,
        }
    }

    fn with_state(mut node: Node, state: NodeState) -> Node {
        node.state = state;
        node
    }

    fn names(ids: &[Id]) -> Vec<&str> {
        ids.iter().map(Id::as_str).collect()
    }

    /// f-a -> f-b -> f-d, f-a -> f-c (body), f-e alone.
    fn diamond() -> Graph {
        Graph::new(vec![
            node("f-a", &[]),
            node("f-b", &["output f-a"]),
            node("f-c", &["body b-1 f-a"]),
            node("f-d", &["output f-b", "body b-2 f-b"]),
            node("f-e", &[]),
        ])
    }

    #[test]
    fn nodes_are_found_by_feature() {
        let graph = diamond();

        assert_eq!(graph.nodes().len(), 5);
        assert_eq!(
            graph.node(&id("f-c")).map(|node| node.feature.as_str()),
            Some("f-c")
        );
        assert!(graph.node(&id("f-x")).is_none());
    }

    #[test]
    fn dependents_are_listed_once_in_timeline_order() {
        let graph = diamond();

        assert_eq!(names(&graph.dependents(&id("f-a"))), ["f-b", "f-c"]);
        assert_eq!(names(&graph.dependents(&id("f-b"))), ["f-d"]);
        assert!(graph.dependents(&id("f-e")).is_empty());
        assert!(graph.dependents(&id("f-x")).is_empty());
    }

    #[test]
    fn affected_by_a_feature_follows_edges_transitively() {
        let graph = diamond();

        assert_eq!(
            names(&graph.affected_by(&[Change::Feature(id("f-a"))])),
            ["f-a", "f-b", "f-c", "f-d"]
        );
        assert_eq!(
            names(&graph.affected_by(&[Change::Feature(id("f-b"))])),
            ["f-b", "f-d"]
        );
    }

    #[test]
    fn affected_by_nothing_is_empty() {
        assert!(diamond().affected_by(&[]).is_empty());
    }

    #[test]
    fn affected_by_reports_only_active_nodes_but_passes_through_inactive_ones() {
        let graph = Graph::new(vec![
            with_state(node("f-a", &[]), NodeState::Suppressed),
            node("f-b", &["output f-a"]),
            with_state(node("f-c", &["output f-b"]), NodeState::RolledBack),
        ]);

        assert_eq!(
            names(&graph.affected_by(&[Change::Feature(id("f-a"))])),
            ["f-b"]
        );
    }

    #[test]
    fn parameter_and_context_seeds_are_active_users() {
        let mut user = node("f-a", &[]);
        user.parameters = vec![id("p-h")];
        user.context_names = vec!["printer.fit.slip".to_string()];
        let mut inactive = with_state(node("f-b", &[]), NodeState::Suppressed);
        inactive.parameters = vec![id("p-h")];
        let mut downstream = node("f-c", &["output f-b"]);
        downstream.uses_expressions = true;
        let graph = Graph::new(vec![user, inactive, downstream]);

        assert_eq!(
            names(&graph.affected_by(&[Change::Parameter(id("p-h"))])),
            ["f-a"]
        );
        assert_eq!(
            names(&graph.affected_by(&[Change::ContextName("printer.fit.slip".to_string())])),
            ["f-a"]
        );
        assert_eq!(names(&graph.affected_by(&[Change::Units])), ["f-c"]);
    }

    #[test]
    fn independence_needs_no_path_either_way() {
        let graph = diamond();

        assert!(!graph.independent(&id("f-a"), &id("f-d")));
        assert!(!graph.independent(&id("f-d"), &id("f-a")));
        assert!(graph.independent(&id("f-c"), &id("f-d")));
        assert!(graph.independent(&id("f-b"), &id("f-c")));
        assert!(graph.independent(&id("f-e"), &id("f-a")));
    }

    #[test]
    fn a_feature_is_not_independent_of_itself() {
        assert!(!diamond().independent(&id("f-a"), &id("f-a")));
    }

    #[test]
    fn a_feature_outside_the_graph_is_independent_of_everything() {
        assert!(diamond().independent(&id("f-x"), &id("f-a")));
    }

    #[test]
    fn repeated_ids_resolve_to_the_first_record() {
        let graph = Graph::new(vec![
            node("f-a", &[]),
            node("f-a", &[]),
            node("f-b", &["output f-a"]),
        ]);

        assert_eq!(graph.position(&id("f-a")), Some(0));
        assert_eq!(names(&graph.dependents(&id("f-a"))), ["f-b"]);
    }
}
