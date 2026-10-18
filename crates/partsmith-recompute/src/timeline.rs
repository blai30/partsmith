//! Preparing a timeline for a graph, plan or run: validation, node states, providers, parameters
//! and edges (recompute.md sections 3.1 to 3.3).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use partsmith_features::{DynFeature, FeatureError, FieldRef, Link, Registry};
use partsmith_model::{Document, FeatureRecord, Id, Parameter};

use crate::graph::{Edge, EdgeKind, Graph, Node, NodeState, first_positions};
use crate::report::Message;
use crate::validation::{Valid, Validations};

/// A document's timeline, ready to plan or run.
pub(crate) struct Timeline<'a> {
    /// The document.
    pub(crate) document: &'a Document,
    /// One step per feature record, in timeline order.
    pub(crate) steps: Vec<Step<'a>>,
    /// The dependency graph.
    pub(crate) graph: Graph,
    /// Document-level messages (an unknown stop point or rollback marker).
    pub(crate) messages: Vec<Message>,
    /// The ids of the document's bodies.
    bodies: BTreeSet<&'a Id>,
}

/// One feature record of a timeline.
pub(crate) struct Step<'a> {
    /// The record.
    pub(crate) record: &'a FeatureRecord,
    /// What a run does with it.
    pub(crate) kind: StepKind<'a>,
}

impl Step<'_> {
    /// Its canonical params and dependencies when it validates, whatever its state, so the loss
    /// diagnosis knows what suppressed and rolled-back records write (section 3.10).
    pub(crate) fn valid(&self) -> Option<&Valid> {
        match &self.kind {
            StepKind::Active(active) => Some(&active.valid),
            StepKind::RolledBack(valid) | StepKind::Suppressed(valid) => valid.as_deref(),
            StepKind::Invalid(_) => None,
        }
    }
}

/// What a run does with a record. A record that runs nothing keeps its canonical params and
/// dependencies when it validates.
pub(crate) enum StepKind<'a> {
    /// Nothing: it is at or after the rollback marker, or after the stop point.
    RolledBack(Option<Arc<Valid>>),
    /// Nothing: the user suppressed it.
    Suppressed(Option<Arc<Valid>>),
    /// Reports its validation error. Boxed because an error is much larger than an active step.
    Invalid(Box<FeatureError>),
    /// Evaluates, checks and builds it.
    Active(ActiveStep<'a>),
}

impl StepKind<'_> {
    /// The graph state of a record with this kind.
    pub(crate) fn state(&self) -> NodeState {
        match self {
            StepKind::RolledBack(_) => NodeState::RolledBack,
            StepKind::Suppressed(_) => NodeState::Suppressed,
            StepKind::Invalid(_) => NodeState::Invalid,
            StepKind::Active(_) => NodeState::Active,
        }
    }
}

/// A record that runs.
pub(crate) struct ActiveStep<'a> {
    /// Its registered feature type.
    pub(crate) feature: &'a dyn DynFeature,
    /// Its canonical params and dependencies.
    pub(crate) valid: Arc<Valid>,
    /// For each body link (read, then written), the position of the body's provider, or None
    /// when the body is empty at this point.
    pub(crate) providers: Vec<Option<usize>>,
}

impl ActiveStep<'_> {
    /// The body links in key order: read, then written.
    pub(crate) fn body_links(&self) -> impl Iterator<Item = &Link> {
        self.valid.body_links()
    }
}

impl<'a> Timeline<'a> {
    /// Validates every record (through the memo), resolves the stop point and rollback marker,
    /// gives each record its state, and builds the graph.
    pub(crate) fn prepare(
        registry: &'a Registry,
        validations: &mut Validations,
        document: &'a Document,
        stop_at: Option<&Id>,
    ) -> Timeline<'a> {
        let positions = first_positions(document.features.iter().map(|record| &record.id));
        let mut linker = Linker::new(document, &positions);
        let (end, messages) = active_end(document, &positions, stop_at);
        let mut steps = Vec::with_capacity(document.features.len());
        let mut nodes = Vec::with_capacity(document.features.len());
        for (position, record) in document.features.iter().enumerate() {
            let validation = validations.check(registry, record).map(|valid| {
                let feature = registry.get(&record.kind).expect(
                    "validation passed for this type on this registry, which never changes, so the type is registered",
                );
                (feature, valid)
            });
            let mut node = Node {
                feature: record.id.clone(),
                state: NodeState::Invalid,
                upstream: Vec::new(),
                parameters: Vec::new(),
                context_names: Vec::new(),
                uses_expressions: false,
            };
            // Records whose dependencies are known have edges whatever their state.
            let providers = match &validation {
                Ok((_, valid)) => linker.link(&mut node, valid, position),
                Err(_) => Vec::new(),
            };
            let kind = if position >= end {
                StepKind::RolledBack(validation.ok().map(|(_, valid)| valid))
            } else if record.suppressed {
                StepKind::Suppressed(validation.ok().map(|(_, valid)| valid))
            } else {
                match validation {
                    Ok((feature, valid)) => {
                        linker.write(&valid, position);
                        StepKind::Active(ActiveStep {
                            feature,
                            valid,
                            providers,
                        })
                    }
                    Err(error) => StepKind::Invalid(Box::new(error)),
                }
            };
            node.state = kind.state();
            nodes.push(node);
            steps.push(Step { record, kind });
        }
        Timeline {
            document,
            steps,
            graph: Graph::with_positions(nodes, positions),
            messages,
            bodies: document.bodies.iter().map(|body| &body.id).collect(),
        }
    }

    /// The pre-build checks of the active step at `position` (recompute.md section 3.3): its
    /// links in declaration order, each of which must name what exists, is earlier and did not
    /// fail. `failed[i]` is true when the record at position `i` is invalid or ended with an error.
    pub(crate) fn check_links(
        &self,
        position: usize,
        step: &ActiveStep<'_>,
        failed: &[bool],
        assets: &BTreeMap<String, Arc<[u8]>>,
    ) -> Result<(), FeatureError> {
        let dependencies = &step.valid.dependencies;
        let field = |path: &str| FieldRef::new(path, step.feature.manifest());
        for link in &dependencies.features {
            let upstream =
                self.graph
                    .position(&link.id)
                    .ok_or_else(|| FeatureError::UnknownFeature {
                        field: field(&link.field),
                        feature: link.id.clone(),
                    })?;
            if upstream >= position {
                return Err(FeatureError::ForwardReference {
                    field: field(&link.field),
                    feature: link.id.clone(),
                });
            }
            if matches!(self.steps[upstream].kind, StepKind::Suppressed(_)) {
                return Err(FeatureError::UpstreamSuppressed {
                    field: field(&link.field),
                    feature: link.id.clone(),
                });
            }
            if failed[upstream] {
                return Err(FeatureError::UpstreamFailed {
                    feature: link.id.clone(),
                });
            }
        }
        for link in step.body_links() {
            if !self.bodies.contains(&link.id) {
                return Err(FeatureError::UnknownBody {
                    field: field(&link.field),
                    body: link.id.clone(),
                });
            }
        }
        for asset in &dependencies.assets {
            if !assets.contains_key(&asset.name) {
                return Err(FeatureError::AssetMissing {
                    field: field(&asset.field),
                    asset: asset.name.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Links records to what they depend on while the timeline is walked in order.
struct Linker<'a> {
    /// The document.
    document: &'a Document,
    /// Timeline position of the first record with each id.
    positions: &'a BTreeMap<Id, usize>,
    /// The document's parameters and what they name.
    parameters: ParameterTable<'a>,
    /// The last active writer of each body among the records walked so far.
    writers: BTreeMap<Id, usize>,
}

impl<'a> Linker<'a> {
    /// A linker at the start of `document`'s timeline, whose first record with each id is at
    /// `positions`.
    fn new(document: &'a Document, positions: &'a BTreeMap<Id, usize>) -> Linker<'a> {
        Linker {
            document,
            positions,
            parameters: ParameterTable::new(&document.parameters),
            writers: BTreeMap::new(),
        }
    }

    /// Fills the edges, parameters, context names and expression flag of the node of the record
    /// at `position`, and returns the provider of each of its body links (read, then written).
    fn link(&self, node: &mut Node, valid: &Valid, position: usize) -> Vec<Option<usize>> {
        let dependencies = &valid.dependencies;
        for link in &dependencies.features {
            if self
                .positions
                .get(&link.id)
                .is_some_and(|&upstream| upstream < position)
            {
                node.upstream.push(Edge {
                    feature: link.id.clone(),
                    via: EdgeKind::Output,
                });
            }
        }
        let mut providers = Vec::new();
        for link in valid.body_links() {
            let provider = self.writers.get(&link.id).copied();
            if let Some(provider) = provider {
                node.upstream.push(Edge {
                    feature: self.document.features[provider].id.clone(),
                    via: EdgeKind::Body(link.id.clone()),
                });
            }
            providers.push(provider);
        }
        let (parameters, context_names) = self
            .parameters
            .closure(&valid.parameter_names, &valid.value_names);
        node.parameters = parameters;
        node.context_names = context_names;
        node.uses_expressions = !dependencies.expressions.is_empty();
        providers
    }

    /// Records the active record at `position` as the provider of the bodies it writes.
    fn write(&mut self, valid: &Valid, position: usize) {
        for link in &valid.dependencies.bodies_written {
            self.writers.insert(link.id.clone(), position);
        }
    }
}

/// The number of active candidates (recompute.md section 3.1 step 3): the records up to and
/// including the stop point when it names a record, else the records before the rollback marker
/// when it names a record, else all records; with an `unknown_stop` message for each of the two
/// that is set but names no record, worded after where the run then stops.
fn active_end(
    document: &Document,
    positions: &BTreeMap<Id, usize>,
    stop_at: Option<&Id>,
) -> (usize, Vec<Message>) {
    let rollback_position = document
        .rollback
        .as_ref()
        .and_then(|rollback| positions.get(rollback).copied());
    let mut messages = Vec::new();
    if let Some(stop_at) = stop_at {
        match (positions.get(stop_at), rollback_position) {
            (Some(&position), _) => return (position + 1, messages),
            (None, Some(_)) => messages.push(Message::unknown_stop_before_rollback()),
            (None, None) => messages.push(Message::unknown_stop()),
        }
    }
    if document.rollback.is_some() {
        match rollback_position {
            Some(position) => return (position, messages),
            None => messages.push(Message::unknown_stop()),
        }
    }
    (document.features.len(), messages)
}

/// The parameters of a document and what each one's expression names.
struct ParameterTable<'a> {
    /// The parameters in document order.
    parameters: &'a [Parameter],
    /// Position of the first parameter with each name (later ones are never found by name).
    by_name: BTreeMap<&'a str, usize>,
    /// For each parameter, the positions of the parameters its expression names.
    references: Vec<Vec<usize>>,
    /// For each parameter, the dotted names its expression uses.
    values: Vec<Vec<String>>,
}

impl<'a> ParameterTable<'a> {
    /// Parses each parameter's expression; one that does not parse names nothing.
    fn new(parameters: &'a [Parameter]) -> ParameterTable<'a> {
        let mut by_name = BTreeMap::new();
        for (position, parameter) in parameters.iter().enumerate() {
            by_name.entry(parameter.name.as_str()).or_insert(position);
        }
        let mut references = Vec::with_capacity(parameters.len());
        let mut values = Vec::with_capacity(parameters.len());
        for parameter in parameters {
            let named = parameter.expr.references().unwrap_or_default();
            references.push(
                named
                    .parameters
                    .iter()
                    .filter_map(|name| by_name.get(name.as_str()).copied())
                    .collect(),
            );
            values.push(named.values);
        }
        ParameterTable {
            parameters,
            by_name,
            references,
            values,
        }
    }

    /// The parameters that expressions naming `parameter_names` and `value_names` use, directly
    /// or through other parameters, in document order, and every dotted name they use, sorted.
    fn closure(
        &self,
        parameter_names: &[String],
        value_names: &[String],
    ) -> (Vec<Id>, Vec<String>) {
        let mut used = BTreeSet::new();
        let mut pending: Vec<usize> = parameter_names
            .iter()
            .filter_map(|name| self.by_name.get(name.as_str()).copied())
            .collect();
        while let Some(position) = pending.pop() {
            if used.insert(position) {
                pending.extend(&self.references[position]);
            }
        }
        let mut context_names: BTreeSet<&str> = value_names.iter().map(String::as_str).collect();
        for &position in &used {
            context_names.extend(self.values[position].iter().map(String::as_str));
        }
        (
            used.into_iter()
                .map(|position| self.parameters[position].id.clone())
                .collect(),
            context_names.into_iter().map(str::to_string).collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::{
        datum, document, document_with, id, prepared, prepared_until, writer,
    };

    fn kinds(timeline: &Timeline<'_>) -> Vec<&'static str> {
        timeline
            .steps
            .iter()
            .map(|step| match step.kind {
                StepKind::RolledBack(_) => "rolled_back",
                StepKind::Suppressed(_) => "suppressed",
                StepKind::Invalid(_) => "invalid",
                StepKind::Active(_) => "active",
            })
            .collect()
    }

    fn active<'t, 'a>(timeline: &'t Timeline<'a>, position: usize) -> &'t ActiveStep<'a> {
        match &timeline.steps[position].kind {
            StepKind::Active(step) => step,
            _ => panic!("step {position} is not active"),
        }
    }

    #[test]
    fn records_get_their_states_with_rollback_first() {
        let document = document_with(json!({
            "features": [
                datum("f-1", "1"),
                {"id": "f-2", "type": "test_datum", "suppressed": true, "offset": "1"},
                {"id": "f-3", "type": "gear_demo"},
                datum("f-4", "1"),
                {"id": "f-5", "type": "gear_demo", "suppressed": true}
            ],
            "rollback": "f-4"
        }));

        let timeline = prepared(&document);

        assert_eq!(
            kinds(&timeline),
            [
                "active",
                "suppressed",
                "invalid",
                "rolled_back",
                "rolled_back"
            ]
        );
        assert!(timeline.messages.is_empty());
    }

    #[test]
    fn records_that_validate_keep_their_dependencies_whatever_their_state() {
        let document = document_with(json!({
            "features": [
                writer("f-1", "b-1"),
                {"id": "f-2", "type": "test_writer", "suppressed": true, "body": "b-2"},
                {"id": "f-3", "type": "gear_demo"},
                writer("f-4", "b-3"),
                {"id": "f-5", "type": "gear_demo"}
            ],
            "rollback": "f-4"
        }));

        let timeline = prepared(&document);

        let written: Vec<Option<Vec<String>>> = timeline
            .steps
            .iter()
            .map(|step| {
                let valid = step.valid()?;
                let bodies = &valid.dependencies.bodies_written;
                Some(bodies.iter().map(|link| link.id.to_string()).collect())
            })
            .collect();
        let bodies = |body: &str| Some(vec![body.to_string()]);
        assert_eq!(
            written,
            [bodies("b-1"), bodies("b-2"), None, bodies("b-3"), None]
        );
    }

    #[test]
    fn a_stop_point_includes_its_record_and_overrides_the_rollback() {
        let document = document_with(json!({
            "features": [datum("f-1", "1"), datum("f-2", "1"), datum("f-3", "1")],
            "rollback": "f-1"
        }));
        let stop = id("f-2");

        let timeline = prepared_until(&document, &stop);

        assert_eq!(kinds(&timeline), ["active", "active", "rolled_back"]);
    }

    #[test]
    fn an_unknown_stop_point_falls_back_to_the_rollback_with_a_warning() {
        let document = document_with(json!({
            "features": [datum("f-1", "1"), datum("f-2", "1")],
            "rollback": "f-2"
        }));
        let stop = id("f-nope");

        let timeline = prepared_until(&document, &stop);

        assert_eq!(kinds(&timeline), ["active", "rolled_back"]);
        let messages: Vec<(&str, &str)> = timeline
            .messages
            .iter()
            .map(|message| (message.code, message.text.as_str()))
            .collect();
        assert_eq!(
            messages,
            [(
                "unknown_stop",
                "The preview point is a feature that is not in this document, so the timeline was rebuilt up to the rollback marker."
            )]
        );
    }

    #[test]
    fn an_unknown_stop_point_without_a_rollback_runs_every_record() {
        let document = document(json!([datum("f-1", "1"), datum("f-2", "1")]));
        let stop = id("f-nope");

        let timeline = prepared_until(&document, &stop);

        assert_eq!(kinds(&timeline), ["active", "active"]);
        let texts: Vec<&str> = timeline
            .messages
            .iter()
            .map(|message| message.text.as_str())
            .collect();
        assert_eq!(
            texts,
            [
                "The preview or rollback point is a feature that is not in this document, so the whole timeline was rebuilt."
            ]
        );
    }

    #[test]
    fn an_unknown_rollback_runs_every_record_with_a_warning() {
        let mut document = document(json!([datum("f-1", "1")]));
        document.rollback = Some(id("f-gone"));

        let timeline = prepared(&document);

        assert_eq!(kinds(&timeline), ["active"]);
        let texts: Vec<&str> = timeline
            .messages
            .iter()
            .map(|message| message.text.as_str())
            .collect();
        assert_eq!(
            texts,
            [
                "The preview or rollback point is a feature that is not in this document, so the whole timeline was rebuilt."
            ]
        );
    }

    #[test]
    fn providers_are_the_last_active_writers() {
        let document = document(json!([
            writer("f-1", "b-1"),
            writer("f-2", "b-1"),
            {"id": "f-3", "type": "test_writer", "suppressed": true, "body": "b-1"},
            {"id": "f-4", "type": "test_reader", "body": "b-1"},
            {"id": "f-5", "type": "test_reader", "body": "b-2"}
        ]));

        let timeline = prepared(&document);

        assert_eq!(active(&timeline, 0).providers, [None]);
        assert_eq!(active(&timeline, 1).providers, [Some(0)]);
        assert_eq!(active(&timeline, 3).providers, [Some(1)]);
        assert_eq!(active(&timeline, 4).providers, [None]);
        let reader = timeline.graph.node(&id("f-4")).expect("a node");
        assert_eq!(
            reader.upstream,
            [Edge {
                feature: id("f-2"),
                via: EdgeKind::Body(id("b-1"))
            }]
        );
    }

    #[test]
    fn output_edges_need_an_earlier_record() {
        let document = document(json!([
            datum("f-1", "1"),
            {"id": "f-2", "type": "test_datum", "base": "f-1", "offset": "1"},
            {"id": "f-3", "type": "test_datum", "base": "f-4", "offset": "1"},
            datum("f-4", "1")
        ]));

        let timeline = prepared(&document);

        let edges = |feature: &str| {
            timeline
                .graph
                .node(&id(feature))
                .expect("a node")
                .upstream
                .len()
        };
        assert_eq!(edges("f-2"), 1);
        assert_eq!(edges("f-3"), 0);
    }

    #[test]
    fn parameters_and_context_names_include_those_of_used_parameters() {
        let document = document_with(json!({
            "parameters": [
                {"id": "p-h", "name": "h", "expr": "printer.fit.slip * 2"},
                {"id": "p-w", "name": "w", "expr": "h + 1 mm"},
                {"id": "p-u", "name": "u", "expr": "1 mm"}
            ],
            "features": [datum("f-1", "w + printer.layer.height")]
        }));

        let timeline = prepared(&document);

        let node = timeline.graph.node(&id("f-1")).expect("a node");
        assert_eq!(node.parameters, [id("p-h"), id("p-w")]);
        assert_eq!(
            node.context_names,
            ["printer.fit.slip", "printer.layer.height"]
        );
        assert!(node.uses_expressions);
    }

    #[test]
    fn parameter_cycles_end() {
        let table_parameters = [
            Parameter::new(id("p-a"), "a", "b"),
            Parameter::new(id("p-b"), "b", "a"),
        ];
        let table = ParameterTable::new(&table_parameters);

        let (used, names) = table.closure(&["a".to_string()], &[]);

        assert_eq!(used, [id("p-a"), id("p-b")]);
        assert!(names.is_empty());
    }

    #[test]
    fn unknown_parameter_names_are_left_out() {
        let table_parameters = [Parameter::new(id("p-a"), "a", "1 mm")];
        let table = ParameterTable::new(&table_parameters);

        let (used, _) = table.closure(&["nope".to_string()], &[]);

        assert!(used.is_empty());
    }

    #[test]
    fn links_are_checked_in_declaration_order() {
        let document = document(json!([
            datum("f-1", "1"),
            {"id": "f-2", "type": "test_datum", "suppressed": true, "offset": "1"},
            {"id": "f-3", "type": "test_datum", "base": "f-nope", "offset": "1"},
            {"id": "f-4", "type": "test_datum", "base": "f-4", "offset": "1"},
            {"id": "f-5", "type": "test_datum", "base": "f-2", "offset": "1"},
            {"id": "f-6", "type": "test_datum", "base": "f-1", "offset": "1"},
            {"id": "f-7", "type": "test_reader", "body": "b-9"},
            {"id": "f-8", "type": "test_asset", "asset": "a.step"}
        ]));
        let timeline = prepared(&document);
        let assets = BTreeMap::new();
        let mut failed = vec![false; 8];
        let code = |position: usize, failed: &[bool]| {
            timeline
                .check_links(position, active(&timeline, position), failed, &assets)
                .err()
                .map(|error| error.code())
        };

        assert_eq!(code(2, &failed), Some("unknown_feature"));
        assert_eq!(code(3, &failed), Some("forward_reference"));
        assert_eq!(code(4, &failed), Some("upstream_suppressed"));
        assert_eq!(code(5, &failed), None);
        failed[0] = true;
        assert_eq!(code(5, &failed), Some("upstream_failed"));
        assert_eq!(code(6, &failed), Some("unknown_body"));
        assert_eq!(code(7, &failed), Some("asset_missing"));
    }

    #[test]
    fn link_errors_name_the_field_with_its_label() {
        let document = document(json!([
            {"id": "f-1", "type": "test_datum", "base": "f-nope", "offset": "1"}
        ]));
        let timeline = prepared(&document);

        let error = timeline
            .check_links(0, active(&timeline, 0), &[false], &BTreeMap::new())
            .expect_err("the base is unknown");

        let field = error.field().expect("a field");
        assert_eq!(field.path, "base");
        assert_eq!(field.label, "Base");
    }

    #[test]
    fn an_asset_that_is_supplied_passes() {
        let document = document(json!([{"id": "f-1", "type": "test_asset", "asset": "a.step"}]));
        let timeline = prepared(&document);
        let assets = BTreeMap::from([("a.step".to_string(), Arc::from(&b"x"[..]))]);

        assert!(
            timeline
                .check_links(0, active(&timeline, 0), &[false], &assets)
                .is_ok()
        );
    }
}
