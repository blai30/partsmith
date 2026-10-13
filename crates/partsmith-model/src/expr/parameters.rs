//! Evaluated parameters, the parameter dependency graph and its cycles (expressions.md section 3.13).

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use super::{EvaluationContext, ExprError, ExprErrorKind, ParsedExpr, Quantity, Scope, Span};
use crate::{Parameter, Units};

/// Evaluated parameters by name, in document order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParameterValues {
    /// Each name with its result, in insertion order; names are unique.
    entries: Vec<(String, Result<Quantity, ExprError>)>,
    /// Position of each name in `entries`.
    positions: BTreeMap<String, usize>,
}

impl ParameterValues {
    /// The result for a parameter name.
    pub fn get(&self, name: &str) -> Option<&Result<Quantity, ExprError>> {
        self.positions
            .get(name)
            .map(|&position| &self.entries[position].1)
    }

    /// Every (name, result) in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Result<Quantity, ExprError>)> {
        self.entries
            .iter()
            .map(|(name, result)| (name.as_str(), result))
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Adds an entry unless the name is already present, in which case the first entry stays.
    fn insert(&mut self, name: String, result: Result<Quantity, ExprError>) {
        if !self.positions.contains_key(&name) {
            self.positions.insert(name.clone(), self.entries.len());
            self.entries.push((name, result));
        }
    }
}

impl FromIterator<(String, Result<Quantity, ExprError>)> for ParameterValues {
    /// Keeps insertion order; when a name repeats, the first entry wins.
    fn from_iter<I: IntoIterator<Item = (String, Result<Quantity, ExprError>)>>(
        iter: I,
    ) -> ParameterValues {
        let mut values = ParameterValues::default();
        for (name, result) in iter {
            values.insert(name, result);
        }
        values
    }
}

/// The parameters of a document with their parsed expressions and the parameters each one names.
pub(crate) struct ParameterGraph<'a> {
    /// Position of the first parameter with each name; later parameters with the same name lose lookups.
    positions: BTreeMap<&'a str, usize>,
    /// Each parameter's parse result, in document order.
    parsed: Vec<Result<ParsedExpr, ExprError>>,
    /// For each parameter, the positions of the parameters its expression names, in first-use
    /// order without repeats. A name used by several parameters refers to the first of them.
    dependencies: Vec<Vec<usize>>,
}

impl<'a> ParameterGraph<'a> {
    /// Parses every parameter and links each to the parameters it names.
    pub(crate) fn new(parameters: &'a [Parameter]) -> ParameterGraph<'a> {
        let mut positions = BTreeMap::new();
        for (position, parameter) in parameters.iter().enumerate() {
            positions.entry(parameter.name.as_str()).or_insert(position);
        }
        let parsed: Vec<_> = parameters
            .iter()
            .map(|parameter| parameter.expr.parse())
            .collect();
        let dependencies = parsed
            .iter()
            .map(|result| match result {
                Ok(tree) => tree
                    .references()
                    .parameters
                    .iter()
                    .filter_map(|name| positions.get(name.as_str()).copied())
                    .collect(),
                Err(_) => Vec::new(),
            })
            .collect();
        ParameterGraph {
            positions,
            parsed,
            dependencies,
        }
    }

    /// Each parameter's parse result, in document order.
    pub(crate) fn parsed(&self) -> &[Result<ParsedExpr, ExprError>] {
        &self.parsed
    }

    /// True when a parameter has this name.
    pub(crate) fn has_parameter(&self, name: &str) -> bool {
        self.positions.contains_key(name)
    }

    /// True when the parameter at `position` is the first with its name, so lookups find it.
    fn is_first_with_name(&self, parameters: &[Parameter], position: usize) -> bool {
        self.positions.get(parameters[position].name.as_str()) == Some(&position)
    }

    /// Parameters that depend on each other in a loop: every strongly connected component with
    /// two or more members or with an edge to itself, each as ascending positions, ordered by
    /// their first position.
    pub(crate) fn cycles(&self) -> Vec<Vec<usize>> {
        let mut cycles: Vec<Vec<usize>> = strongly_connected_components(&self.dependencies)
            .into_iter()
            .filter(|component| match component.as_slice() {
                [single] => self.dependencies[*single].contains(single),
                _ => true,
            })
            .map(|mut component| {
                component.sort_unstable();
                component
            })
            .collect();
        cycles.sort_unstable_by_key(|cycle| cycle[0]);
        cycles
    }
}

/// Where the evaluation of one parameter stands.
enum Slot {
    /// Parsed, waiting for its turn in dependency order.
    Pending(ParsedExpr),
    /// Its final result: a parse or cycle error, or what it evaluated to.
    Done(Result<Quantity, ExprError>),
}

impl Slot {
    /// True while the parameter waits to be evaluated.
    fn is_pending(&self) -> bool {
        matches!(self, Slot::Pending(_))
    }
}

/// Evaluates every parameter: parse errors, then cycles, then the rest in dependency order with
/// ties broken by document order (expressions.md section 3.13). Entries are in document order.
pub(crate) fn evaluate_parameters(
    parameters: &[Parameter],
    units: Units,
    context: &dyn EvaluationContext,
) -> ParameterValues {
    let mut graph = ParameterGraph::new(parameters);
    let cycles = graph.cycles();

    let mut slots: Vec<Slot> = std::mem::take(&mut graph.parsed)
        .into_iter()
        .map(|parsed| match parsed {
            Ok(tree) => Slot::Pending(tree),
            Err(error) => Slot::Done(Err(error)),
        })
        .collect();
    for cycle in cycles {
        let names: Vec<String> = cycle
            .iter()
            .map(|&position| parameters[position].name.clone())
            .collect();
        for &position in &cycle {
            let span = Span::whole(parameters[position].expr.as_str());
            slots[position] = Slot::Done(Err(ExprError::new(
                ExprErrorKind::Cycle {
                    names: names.clone(),
                },
                span,
            )));
        }
    }

    // Lookups see only the first parameter with each name.
    let mut known: ParameterValues = slots
        .iter()
        .enumerate()
        .filter(|&(position, _)| graph.is_first_with_name(parameters, position))
        .filter_map(|(position, slot)| match slot {
            Slot::Done(result) => Some((parameters[position].name.clone(), result.clone())),
            Slot::Pending(_) => None,
        })
        .collect();
    for position in dependency_order(&graph.dependencies, |node| slots[node].is_pending()) {
        let Slot::Pending(tree) = &slots[position] else {
            continue;
        };
        let scope = Scope {
            units,
            parameters: &known,
            context,
        };
        let result = tree.evaluate(&scope, None);
        if graph.is_first_with_name(parameters, position) {
            known.insert(parameters[position].name.clone(), result.clone());
        }
        slots[position] = Slot::Done(result);
    }

    parameters
        .iter()
        .zip(slots)
        .map(|(parameter, slot)| match slot {
            Slot::Done(result) => (parameter.name.clone(), result),
            Slot::Pending(_) => unreachable!(
                "every parameter failed to parse, is in a cycle, or was evaluated in dependency order"
            ),
        })
        .collect()
}

/// The pending nodes in dependency order (each after every pending node it depends on), ties
/// broken by position. Pending nodes form no cycle, so every one of them is in the order.
fn dependency_order(dependencies: &[Vec<usize>], is_pending: impl Fn(usize) -> bool) -> Vec<usize> {
    let mut waiting_on = vec![0_usize; dependencies.len()];
    let mut dependents = vec![Vec::new(); dependencies.len()];
    for (node, node_dependencies) in dependencies.iter().enumerate() {
        if !is_pending(node) {
            continue;
        }
        for &dependency in node_dependencies {
            if is_pending(dependency) {
                waiting_on[node] += 1;
                dependents[dependency].push(node);
            }
        }
    }
    let mut ready: BinaryHeap<Reverse<usize>> = (0..dependencies.len())
        .filter(|&node| is_pending(node) && waiting_on[node] == 0)
        .map(Reverse)
        .collect();
    let mut order = Vec::new();
    while let Some(Reverse(node)) = ready.pop() {
        order.push(node);
        for &dependent in &dependents[node] {
            waiting_on[dependent] -= 1;
            if waiting_on[dependent] == 0 {
                ready.push(Reverse(dependent));
            }
        }
    }
    order
}

/// Tarjan's strongly connected components of the graph whose node `i` has edges to
/// `edges[i]`, iteratively so long chains cannot exhaust the call stack.
fn strongly_connected_components(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let count = edges.len();
    let mut discovery: Vec<Option<usize>> = vec![None; count];
    let mut low_link = vec![0_usize; count];
    let mut on_stack = vec![false; count];
    let mut stack = Vec::new();
    let mut components = Vec::new();
    let mut next_discovery = 0;

    for root in 0..count {
        if discovery[root].is_some() {
            continue;
        }
        // Each frame is a node and the index of its next edge to follow.
        let mut frames = vec![(root, 0_usize)];
        discovery[root] = Some(next_discovery);
        low_link[root] = next_discovery;
        next_discovery += 1;
        stack.push(root);
        on_stack[root] = true;

        while let Some(frame) = frames.last_mut() {
            let (node, next_edge) = *frame;
            if let Some(&target) = edges[node].get(next_edge) {
                frame.1 += 1;
                match discovery[target] {
                    None => {
                        discovery[target] = Some(next_discovery);
                        low_link[target] = next_discovery;
                        next_discovery += 1;
                        stack.push(target);
                        on_stack[target] = true;
                        frames.push((target, 0));
                    }
                    Some(target_discovery) if on_stack[target] => {
                        low_link[node] = low_link[node].min(target_discovery);
                    }
                    Some(_) => {}
                }
                continue;
            }
            frames.pop();
            if let Some(&(parent, _)) = frames.last() {
                low_link[parent] = low_link[parent].min(low_link[node]);
            }
            if Some(low_link[node]) == discovery[node] {
                let mut component = Vec::new();
                while let Some(member) = stack.pop() {
                    on_stack[member] = false;
                    component.push(member);
                    if member == node {
                        break;
                    }
                }
                components.push(component);
            }
        }
    }
    components
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EmptyContext, Id};

    fn parameter(name: &str, source: &str) -> Parameter {
        Parameter::new(
            Id::new(format!("p-{name}")).expect("valid id"),
            name,
            source,
        )
    }

    fn sorted(mut components: Vec<Vec<usize>>) -> Vec<Vec<usize>> {
        for component in &mut components {
            component.sort_unstable();
        }
        components.sort();
        components
    }

    #[test]
    fn components_of_a_chain_are_single_nodes() {
        let edges = vec![vec![1], vec![2], vec![]];

        assert_eq!(
            sorted(strongly_connected_components(&edges)),
            [vec![0], vec![1], vec![2]]
        );
    }

    #[test]
    fn components_find_loops() {
        let edges = vec![vec![1], vec![0, 2], vec![3], vec![2], vec![]];

        assert_eq!(
            sorted(strongly_connected_components(&edges)),
            [vec![0, 1], vec![2, 3], vec![4]]
        );
    }

    #[test]
    fn long_chains_do_not_recurse() {
        let edges: Vec<Vec<usize>> = (0..100_000)
            .map(|node| vec![node + 1])
            .chain([vec![0]])
            .collect();

        assert_eq!(strongly_connected_components(&edges).len(), 1);
    }

    #[test]
    fn cycles_include_self_loops_and_skip_single_nodes() {
        let parameters = [
            parameter("a", "a"),
            parameter("b", "1"),
            parameter("c", "d"),
            parameter("d", "c + b"),
        ];
        let graph = ParameterGraph::new(&parameters);

        assert_eq!(graph.cycles(), [vec![0], vec![2, 3]]);
    }

    #[test]
    fn names_refer_to_the_first_parameter_with_that_name() {
        let parameters = [
            parameter("a", "1"),
            parameter("a", "2"),
            parameter("b", "a"),
        ];
        let graph = ParameterGraph::new(&parameters);

        assert_eq!(graph.dependencies[2], [0]);
        assert!(graph.is_first_with_name(&parameters, 0));
        assert!(!graph.is_first_with_name(&parameters, 1));
        assert!(graph.has_parameter("b"));
        assert!(!graph.has_parameter("c"));
    }

    #[test]
    fn dependency_order_breaks_ties_by_position() {
        let dependencies = vec![vec![2], vec![], vec![], vec![1]];

        assert_eq!(dependency_order(&dependencies, |_| true), [1, 2, 0, 3]);
    }

    #[test]
    fn dependencies_that_are_not_pending_do_not_block() {
        let dependencies = vec![vec![1], vec![]];

        assert_eq!(dependency_order(&dependencies, |node| node == 0), [0]);
    }

    #[test]
    fn duplicate_names_keep_the_first_value() {
        let values = evaluate_parameters(
            &[
                parameter("a", "1"),
                parameter("a", "2"),
                parameter("b", "a * 10"),
            ],
            Units::Mm,
            &EmptyContext,
        );

        assert_eq!(values.len(), 2);
        assert_eq!(values.get("b"), Some(&Ok(Quantity::plain(10.0))));
    }

    #[test]
    fn a_later_duplicate_that_fails_does_not_hide_the_first() {
        let values = evaluate_parameters(
            &[
                parameter("b", "a"),
                parameter("a", "2 mm"),
                parameter("a", "1 +"),
            ],
            Units::Mm,
            &EmptyContext,
        );

        assert_eq!(values.get("b"), Some(&Ok(Quantity::length(2.0))));
        assert_eq!(values.get("a"), Some(&Ok(Quantity::length(2.0))));
    }

    #[test]
    fn values_iterate_in_insertion_order() {
        let values: ParameterValues = [
            ("z".to_string(), Ok(Quantity::plain(1.0))),
            ("a".to_string(), Ok(Quantity::plain(2.0))),
        ]
        .into_iter()
        .collect();

        assert_eq!(
            values.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            ["z", "a"]
        );
        assert!(!values.is_empty());
        assert_eq!(values.get("q"), None);
    }
}
