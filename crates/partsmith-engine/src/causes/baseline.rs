//! What one recompute of a document read (engine.md section 3.21, "Snapshots"), shared through
//! `Arc` by every feature that ran in it, so a feature's snapshot is its part of the baseline of
//! the last recompute in which it ran; and, made the first time a cause compares the baseline and
//! then kept with it, the dependency graph of its recipe (recompute.md section 3.2), which says
//! what reaches a feature, and its parameters as the recompute evaluated them. So a recompute
//! that makes many causes validates each record of each recipe it compares once.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use partsmith_features::{
    Dependencies, ExprUse, FeatureManifest, FeatureOutput, FieldUnit, OutputData, Registry,
    SketchResult, Values,
};
use partsmith_model::{Document, Expr, FeatureRecord, Id, ParameterValues, Quantity, Scope, Units};
use partsmith_print::PrinterValues;

use crate::params::declared_dependencies;
use crate::state::{Computed, ran};

/// What one recompute of a document read: the recipe and the registry that validates it, which
/// features ran, the printer values its expressions were evaluated with, each built sketch's
/// solution and placement, and how many commands had changed the recipe.
#[derive(Debug)]
pub(crate) struct Baseline {
    /// The registry the session validates records with.
    registry: Arc<Registry>,
    /// The recipe the recompute ran.
    pub(crate) document: Arc<Document>,
    /// The features that ran (built or failed), rather than being suppressed or rolled back.
    ran: BTreeSet<Id>,
    /// The `printer.*` values of the profile the recompute evaluated expressions with.
    printer: PrinterValues,
    /// The output of each sketch that built.
    sketches: BTreeMap<Id, Arc<FeatureOutput>>,
    /// How many commands had changed the recipe when the recompute ran (`History::commands`).
    commands: u64,
    /// The timeline position of each feature, indexed the first time one is looked up.
    positions: OnceLock<BTreeMap<Id, usize>>,
    /// The dependencies of the recipe, found the first time a cause compares the baseline.
    graph: OnceLock<GraphData>,
    /// The recipe's parameters evaluated with `printer`, the first time a cause reads a value.
    parameters: OnceLock<ParameterValues>,
}

impl Baseline {
    /// The baseline of the recompute `computed` of `document`, after `commands` commands changed
    /// the recipe.
    pub(crate) fn of(
        registry: Arc<Registry>,
        document: Arc<Document>,
        computed: &Computed,
        commands: u64,
    ) -> Baseline {
        let features = &computed.report.features;
        let sketches = features
            .iter()
            .filter_map(|feature| {
                let output = feature.output.as_ref()?;
                matches!(output.data, OutputData::Sketch(_))
                    .then(|| (feature.feature.clone(), Arc::clone(output)))
            })
            .collect();
        Baseline {
            registry,
            document,
            ran: features
                .iter()
                .filter(|feature| ran(feature.status))
                .map(|feature| feature.feature.clone())
                .collect(),
            printer: PrinterValues::new(&computed.profile.profile),
            sketches,
            commands,
            positions: OnceLock::new(),
            graph: OnceLock::new(),
            parameters: OnceLock::new(),
        }
    }

    /// The registry the session validates records with.
    pub(crate) fn registry(&self) -> &Registry {
        &self.registry
    }

    /// The record of a feature, when the recipe has it.
    pub(crate) fn record(&self, feature: &Id) -> Option<&FeatureRecord> {
        self.document.features.get(self.position(feature)?)
    }

    /// The timeline position of a feature, when the recipe has it.
    pub(crate) fn position(&self, feature: &Id) -> Option<usize> {
        let positions = self.positions.get_or_init(|| {
            self.document
                .features
                .iter()
                .enumerate()
                .map(|(position, record)| (record.id.clone(), position))
                .collect()
        });
        positions.get(feature).copied()
    }

    /// The document units.
    pub(crate) fn units(&self) -> Units {
        self.document.units
    }

    /// The solution and placement of a sketch that built in this recompute.
    pub(crate) fn sketch(&self, feature: &Id) -> Option<&SketchResult> {
        match &self.sketches.get(feature)?.data {
            OutputData::Sketch(result) => Some(result),
            _ => None,
        }
    }

    /// How many commands changed the recipe between `earlier`'s recompute and this one.
    pub(super) fn commands_since(&self, earlier: &Baseline) -> u64 {
        self.commands.saturating_sub(earlier.commands)
    }

    /// The dependency graph of the recipe.
    pub(super) fn graph(&self) -> Graph<'_> {
        Graph {
            baseline: self,
            data: self.graph.get_or_init(|| GraphData::of(self)),
        }
    }

    /// The recipe's expressions as this recompute evaluated them.
    pub(super) fn evaluation(&self) -> Evaluation<'_> {
        Evaluation {
            baseline: self,
            parameters: self
                .parameters
                .get_or_init(|| self.document.evaluate_parameters(&self.printer)),
        }
    }
}

/// The parameters of a baseline's recipe, evaluated with its printer values, to evaluate its
/// expressions the way its recompute did.
pub(super) struct Evaluation<'b> {
    /// The baseline.
    baseline: &'b Baseline,
    /// Its evaluated parameters.
    parameters: &'b ParameterValues,
}

impl Evaluation<'_> {
    /// The value of `expr` in a field of `kind` of a feature whose type has `manifest`, as the
    /// feature evaluates it (features.md section 3.4); `None` when it does not evaluate.
    pub(super) fn value(
        &self,
        manifest: &FeatureManifest,
        expr: &Expr,
        kind: FieldUnit,
    ) -> Option<f64> {
        let used = ExprUse {
            field: String::new(),
            expr: expr.clone(),
            kind,
        };
        let values = Values::evaluate(std::slice::from_ref(&used), &self.scope(), manifest);
        let (_, value) = values.iter().next()?;
        value.as_ref().ok().copied()
    }

    /// The value of the parameter named `name`, when it evaluates.
    pub(super) fn parameter(&self, name: &str) -> Option<Quantity> {
        self.parameters.get(name)?.as_ref().ok().copied()
    }

    /// The scope of the baseline's expressions.
    fn scope(&self) -> Scope<'_> {
        Scope {
            units: self.baseline.units(),
            parameters: self.parameters,
            context: &self.baseline.printer,
        }
    }
}

/// What the graph of a recipe holds for each of its records, in timeline order.
#[derive(Debug)]
struct GraphData {
    /// Each record's declared dependencies; `None` for a record whose params fail validation,
    /// which links to nothing and provides no body.
    dependencies: Vec<Option<Dependencies>>,
    /// Each record's body providers: for each body it declares, the last record before it that
    /// is active (it ran, with valid params) and declares writing that body.
    providers: Vec<Vec<Id>>,
    /// The positions of the records each record reads directly: its feature links the recipe
    /// has, and its body providers.
    reads: Vec<Vec<usize>>,
    /// The parameters each record's expressions use, found the first time they are asked for.
    parameters: Vec<OnceLock<UsedParameters>>,
}

impl GraphData {
    /// The graph of `baseline`'s recipe: each record validated once, and its providers found in
    /// one pass over the timeline.
    fn of(baseline: &Baseline) -> GraphData {
        let records = &baseline.document.features;
        let dependencies: Vec<Option<Dependencies>> = records
            .iter()
            .map(|record| declared_dependencies(&baseline.registry, record))
            .collect();
        let mut writers: BTreeMap<&Id, &Id> = BTreeMap::new();
        let mut providers = Vec::with_capacity(records.len());
        for (record, declared) in records.iter().zip(&dependencies) {
            let mut found: Vec<Id> = Vec::new();
            if let Some(declared) = declared {
                for body in declared.bodies_read.iter().chain(&declared.bodies_written) {
                    if let Some(&writer) = writers.get(&body.id)
                        && !found.contains(writer)
                    {
                        found.push(writer.clone());
                    }
                }
                if baseline.ran.contains(&record.id) {
                    for body in &declared.bodies_written {
                        writers.insert(&body.id, &record.id);
                    }
                }
            }
            providers.push(found);
        }
        let reads = dependencies
            .iter()
            .zip(&providers)
            .map(|(declared, found)| {
                let links = declared.iter().flat_map(|declared| &declared.features);
                let mut read: Vec<usize> = links
                    .map(|link| &link.id)
                    .chain(found)
                    .filter_map(|feature| baseline.position(feature))
                    .collect();
                read.sort_unstable();
                read.dedup();
                read
            })
            .collect();
        GraphData {
            parameters: records.iter().map(|_| OnceLock::new()).collect(),
            dependencies,
            providers,
            reads,
        }
    }
}

/// The dependency graph of a baseline's recipe (recompute.md section 3.2): each record's
/// declared dependencies, and through them its feature links and body providers.
#[derive(Clone, Copy)]
pub(super) struct Graph<'b> {
    /// The baseline.
    baseline: &'b Baseline,
    /// What the graph holds for each record.
    data: &'b GraphData,
}

impl<'b> Graph<'b> {
    /// The baseline.
    pub(super) fn baseline(&self) -> &'b Baseline {
        self.baseline
    }

    /// The declared dependencies of a feature; `None` when the recipe does not have it or its
    /// params fail validation.
    pub(super) fn dependencies(&self, feature: &Id) -> Option<&'b Dependencies> {
        self.data
            .dependencies
            .get(self.baseline.position(feature)?)?
            .as_ref()
    }

    /// The features a feature links to (its feature links, `Dependencies::features`).
    pub(super) fn links(&self, feature: &Id) -> Vec<Id> {
        self.dependencies(feature)
            .map(|dependencies| {
                dependencies
                    .features
                    .iter()
                    .map(|link| link.id.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The body providers of a feature: for each body it declares, the last record before it
    /// that is active (it ran, with valid params) and declares writing that body.
    pub(super) fn providers(&self, feature: &Id) -> &'b [Id] {
        self.baseline
            .position(feature)
            .and_then(|position| self.data.providers.get(position))
            .map_or(&[], Vec::as_slice)
    }

    /// The features of the recipe upstream of `feature`: what it reads directly (its feature
    /// links and body providers), and what those read, at any depth, without passing through
    /// `avoided`.
    pub(super) fn upstream(&self, feature: &Id, avoided: Option<&Id>) -> BTreeSet<&'b Id> {
        let records = &self.baseline.document.features;
        let avoided = avoided.and_then(|avoided| self.baseline.position(avoided));
        let mut reached = vec![false; records.len()];
        let mut waiting: Vec<usize> = self.baseline.position(feature).into_iter().collect();
        while let Some(reader) = waiting.pop() {
            for &read in &self.data.reads[reader] {
                if Some(read) != avoided && !reached[read] {
                    reached[read] = true;
                    waiting.push(read);
                }
            }
        }
        records
            .iter()
            .zip(reached)
            .filter_map(|(record, reached)| reached.then_some(&record.id))
            .collect()
    }

    /// The expressions a feature declares.
    pub(super) fn expressions(&self, feature: &Id) -> &'b [ExprUse] {
        self.dependencies(feature)
            .map_or(&[], |dependencies| dependencies.expressions.as_slice())
    }

    /// True when the document units decide a size of a feature (engine.md section 3.21, the
    /// units change): one of its length expressions is a number without a unit other than zero,
    /// or names in a promoting position (expressions.md section 3.19) a parameter whose
    /// expression is one, directly or through other parameters so named.
    pub(super) fn relies_on_units(&self, feature: &Id) -> bool {
        let document = &self.baseline.document;
        self.expressions(feature)
            .iter()
            .filter(|used| used.kind == FieldUnit::Length)
            .any(|used| {
                let mut followed: BTreeSet<String> = BTreeSet::new();
                let mut waiting = vec![&used.expr];
                while let Some(expr) = waiting.pop() {
                    if is_bare_size(expr) {
                        return true;
                    }
                    for name in expr.promoted_names().unwrap_or_default() {
                        if let Some(parameter) = document.parameter_by_name(&name)
                            && followed.insert(name)
                        {
                            waiting.push(&parameter.expr);
                        }
                    }
                }
                false
            })
    }

    /// The parameters a feature's expressions use, found once per feature; `None` when the
    /// recipe does not have it.
    pub(super) fn parameters(&self, feature: &Id) -> Option<&'b UsedParameters> {
        let memo = self.data.parameters.get(self.baseline.position(feature)?)?;
        Some(memo.get_or_init(|| self.used_parameters(feature)))
    }

    /// The parameters a feature's expressions use, found from its expressions.
    fn used_parameters(&self, feature: &Id) -> UsedParameters {
        let document = &self.baseline.document;
        let names_in = |expr: &Expr| expr.references().map(|found| found.parameters).ok();
        let direct: BTreeSet<Id> = self
            .expressions(feature)
            .iter()
            .filter_map(|used| names_in(&used.expr))
            .flatten()
            .filter_map(|name| {
                document
                    .parameter_by_name(&name)
                    .map(|found| found.id.clone())
            })
            .collect();
        let mut all = direct.clone();
        let mut waiting: Vec<Id> = direct.iter().cloned().collect();
        while let Some(parameter) = waiting.pop() {
            let Some(expr) = document.parameter(&parameter).map(|found| &found.expr) else {
                continue;
            };
            for name in names_in(expr).unwrap_or_default() {
                if let Some(used) = document.parameter_by_name(&name)
                    && all.insert(used.id.clone())
                {
                    waiting.push(used.id.clone());
                }
            }
        }
        UsedParameters { direct, all }
    }
}

/// True when `expr` is a number without a unit other than zero, which the document units give
/// its size; a zero, such as a primitive's default position, is the same in every unit.
fn is_bare_size(expr: &Expr) -> bool {
    let zero = expr
        .as_str()
        .trim_start_matches(['-', ' '])
        .trim()
        .parse::<f64>()
        .is_ok_and(|number| number == 0.0);
    expr.is_bare_number() && !zero
}

/// The parameters a feature uses: `direct` those its expressions name, `all` those and the
/// parameters these use, at any depth.
#[derive(Debug)]
pub(super) struct UsedParameters {
    /// Those its own expressions name.
    pub(super) direct: BTreeSet<Id>,
    /// Those and the parameters they use, at any depth.
    pub(super) all: BTreeSet<Id>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::id;
    use crate::{Session, SessionOptions};

    /// `fillet.ptsm`: a box sized by the parameter `height`, and a fillet on one of its edges.
    const FILLET: &str = include_str!("../../tests/fixtures/fillet.ptsm");

    /// The baseline of `text` as a headless session opened it.
    fn baseline_of(text: &str) -> Baseline {
        let mut session = Session::new(SessionOptions::headless());
        let doc = session
            .open_document(text, Vec::new())
            .expect("the document opens")
            .doc;
        let open = &session.documents[&doc];
        Baseline::of(
            Arc::clone(&session.registry),
            Arc::clone(&open.document),
            &open.computed,
            0,
        )
    }

    #[test]
    fn the_graph_and_the_parameter_values_are_made_once_per_baseline() {
        let baseline = baseline_of(FILLET);

        assert!(std::ptr::eq(baseline.graph().data, baseline.graph().data));
        assert!(std::ptr::eq(
            baseline.evaluation().parameters,
            baseline.evaluation().parameters
        ));
    }

    #[test]
    fn upstream_follows_body_providers_and_stops_at_the_avoided_feature() {
        let baseline = baseline_of(FILLET);
        let graph = baseline.graph();
        let block = id("f-box1");

        assert_eq!(
            graph.upstream(&id("f-fil1"), None),
            BTreeSet::from([&block])
        );
        assert_eq!(graph.upstream(&id("f-fil1"), Some(&block)), BTreeSet::new());
        assert_eq!(graph.upstream(&block, None), BTreeSet::new());
    }

    #[test]
    fn a_length_relies_on_the_units_through_a_parameter_without_a_unit() {
        // The box's position is left out, so it is at the bare zeros of its default.
        let with_unit = baseline_of(FILLET);
        let bare = baseline_of(&FILLET.replace(r#""expr": "40 mm""#, r#""expr": "40""#));

        assert!(!with_unit.graph().relies_on_units(&id("f-box1")));
        assert!(bare.graph().relies_on_units(&id("f-box1")));
        assert!(!bare.graph().relies_on_units(&id("f-fil1")));
    }

    #[test]
    fn only_a_number_without_a_unit_other_than_zero_is_a_size_the_units_decide() {
        let sizes = ["2", "-2", "0.5", " 2 "];
        let others = ["0", "-0", "0.0", "2 mm", "height", "2 * height"];

        for size in sizes {
            assert!(is_bare_size(&Expr::new(size)), "{size}");
        }
        for other in others {
            assert!(!is_bare_size(&Expr::new(other)), "{other}");
        }
    }
}
