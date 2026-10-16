//! A small timeline runner for build context and message tests. It builds feature records of the built-in registry in
//! order, the way recompute does (recompute.md section 3.5): each feature gets the states of its declared bodies, their
//! writer chains (`BuildInputs::history`) and the outputs of its declared features, and its body updates become the
//! next states through `BodyState::from_update`. There is no cache, no graph and no loss diagnosis, so tests pass the
//! `Diagnosis` a scenario implies.

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::{
    BodyState, BuildContext, BuildInputs, CancelToken, Diagnosis, DocumentNames, FeatureError,
    FeatureOutput, FeatureWarning, Fix, Names, OutputData, Registry, RenderContext, SketchResult,
};
use partsmith_model::{Document, Id, Units};
use partsmith_naming::{ElementKind, ElementName, ElementSet, Reference};
use partsmith_occt_sys::{SubShape, SubShapeKind};
use serde_json::{Value, json};

use super::{ContextFixture, evaluate, id};

/// The outcome of building feature records in timeline order.
pub struct Timeline {
    /// The built-in registry the records were built with.
    pub registry: Registry,
    /// The document the records form (for `DocumentNames`).
    pub document: Document,
    states: BTreeMap<Id, BodyState>,
    chains: BTreeMap<Id, Vec<(Id, BodyState)>>,
    outputs: BTreeMap<Id, Arc<FeatureOutput>>,
    errors: BTreeMap<Id, FeatureError>,
    before: BTreeMap<Id, BTreeMap<Id, BodyState>>,
}

impl Timeline {
    /// Builds `features` (record JSON) in a millimeter document with the bodies `b-<n>` named `Body <n>`. Suppressed
    /// records are skipped; a record that fails keeps its error and changes no body.
    pub fn run(bodies: &[&str], features: &[Value]) -> Timeline {
        let bodies: Vec<Value> = bodies
            .iter()
            .map(|body| {
                let number = body
                    .strip_prefix("b-")
                    .expect("test body ids look like b-1");
                json!({"id": body, "name": format!("Body {number}")})
            })
            .collect();
        let text = json!({
            "format": "partsmith",
            "format_version": 2,
            "units": "mm",
            "parameters": [],
            "bodies": bodies,
            "features": features,
        })
        .to_string();
        let document = partsmith_model::load(&text)
            .unwrap_or_else(|error| panic!("the test document does not load: {error}\n{text}"))
            .document;
        let mut timeline = Timeline {
            registry: Registry::with_builtins(),
            document,
            states: BTreeMap::new(),
            chains: BTreeMap::new(),
            outputs: BTreeMap::new(),
            errors: BTreeMap::new(),
            before: BTreeMap::new(),
        };
        let records = timeline.document.features.clone();
        for record in records.iter().filter(|record| !record.suppressed) {
            timeline.build(record);
        }
        timeline
    }

    /// Builds one record against the states so far and applies its body updates.
    fn build(&mut self, record: &partsmith_model::FeatureRecord) {
        let validated = match self.registry.validate_and_canonicalize(record) {
            Ok(validated) => validated,
            Err(error) => {
                self.errors.insert(record.id.clone(), error);
                return;
            }
        };
        let record = validated.record;
        let dependencies = self
            .registry
            .dependencies(&record)
            .expect("valid records have dependencies");
        let manifest = self
            .registry
            .get(&record.kind)
            .expect("the type is built in")
            .manifest()
            .clone();
        let values = evaluate(&dependencies.expressions, Units::Mm, &manifest);
        let declared: Vec<Id> = dependencies
            .bodies_read
            .iter()
            .chain(&dependencies.bodies_written)
            .map(|link| link.id.clone())
            .collect();
        let bodies: BTreeMap<Id, BodyState> = declared
            .iter()
            .map(|body| (body.clone(), self.state_of(body)))
            .collect();
        let history: BTreeMap<Id, Vec<(Id, BodyState)>> = declared
            .iter()
            .map(|body| {
                (
                    body.clone(),
                    self.chains.get(body).cloned().unwrap_or_default(),
                )
            })
            .collect();
        let upstream: BTreeMap<Id, Arc<FeatureOutput>> = dependencies
            .features
            .iter()
            .filter_map(|link| {
                self.outputs
                    .get(&link.id)
                    .map(|output| (link.id.clone(), Arc::clone(output)))
            })
            .collect();
        let assets = BTreeMap::new();
        let cancel = CancelToken::new();
        let result = self.registry.build(
            &record,
            &BuildContext::new(BuildInputs {
                feature: &record.id,
                manifest: &manifest,
                units: Units::Mm,
                values: &values,
                bodies: &bodies,
                history: &history,
                upstream: &upstream,
                assets: &assets,
                cancel: &cancel,
            }),
        );
        for link in &dependencies.bodies_written {
            self.chains
                .entry(link.id.clone())
                .or_default()
                .push((record.id.clone(), bodies[&link.id].clone()));
        }
        self.before.insert(record.id.clone(), bodies);
        match result {
            Ok(output) => {
                if let OutputData::Bodies(updates) = &output.data {
                    for update in updates {
                        self.states
                            .insert(update.body.clone(), BodyState::from_update(update));
                    }
                }
                self.outputs.insert(record.id.clone(), Arc::new(output));
            }
            Err(error) => {
                self.errors.insert(record.id.clone(), error);
            }
        }
    }

    /// The state of `body` at the end of the timeline (empty when nothing wrote it).
    fn state_of(&self, body: &Id) -> BodyState {
        self.states
            .get(body)
            .cloned()
            .unwrap_or_else(BodyState::empty)
    }

    /// The state of `body` at the end of the timeline.
    pub fn state(&self, body: &str) -> BodyState {
        self.state_of(&id(body))
    }

    /// The writer chain of `body` at the end of the timeline: each writer and the state just before it, oldest first.
    pub fn chain(&self, body: &str) -> Vec<(Id, BodyState)> {
        self.chains.get(&id(body)).cloned().unwrap_or_default()
    }

    /// The output of `feature`; fails the test when it failed.
    pub fn output(&self, feature: &str) -> &FeatureOutput {
        self.outputs.get(&id(feature)).unwrap_or_else(|| {
            panic!(
                "{feature} has no output; its error: {:?}",
                self.errors.get(&id(feature))
            )
        })
    }

    /// The warnings and notices of `feature`'s output.
    pub fn warnings(&self, feature: &str) -> &[FeatureWarning] {
        &self.output(feature).warnings
    }

    /// The error of `feature`; fails the test when it built.
    pub fn error(&self, feature: &str) -> &FeatureError {
        self.errors
            .get(&id(feature))
            .unwrap_or_else(|| panic!("{feature} built without an error"))
    }

    /// The sketch result of `sketch`.
    pub fn sketch(&self, sketch: &str) -> &SketchResult {
        match &self.output(sketch).data {
            OutputData::Sketch(result) => result,
            _ => panic!("{sketch} is not a sketch"),
        }
    }

    /// The states of `feature`'s declared bodies just before it.
    pub fn before(&self, feature: &str) -> &BTreeMap<Id, BodyState> {
        self.before
            .get(&id(feature))
            .unwrap_or_else(|| panic!("{feature} did not run"))
    }

    /// The resolver's view of `body` at the end of the timeline.
    pub fn element_set(&self, body: &str) -> Arc<ElementSet> {
        self.state(body)
            .element_set()
            .expect("the element set of a built body")
    }

    /// `Reference::from_element` of the element named `name` in `body` at the end of the timeline: what a pick
    /// stores.
    pub fn picked(&self, body: &str, name: &str) -> Reference {
        picked_in(&self.element_set(body), name)
    }

    /// `Reference::from_element` of the element named `name` in `body` just before `feature`: what a pick made there
    /// stores, and what a candidate of that feature's losses holds.
    pub fn picked_before(&self, feature: &str, body: &str, name: &str) -> Reference {
        let state = self
            .before(feature)
            .get(&id(body))
            .unwrap_or_else(|| panic!("{feature} does not declare {body}"));
        picked_in(
            &state
                .element_set()
                .expect("the element set of a built body"),
            name,
        )
    }

    /// A context fixture for a feature `feature` of the built-in type `type_name`, declaring every body with its
    /// state and writer chain at the end of the timeline and every output as an upstream feature.
    pub fn fixture(&self, feature: &str, type_name: &str) -> ContextFixture {
        let manifest = self
            .registry
            .get(type_name)
            .expect("the type is built in")
            .manifest()
            .clone();
        let mut fixture = ContextFixture::with_manifest(id(feature), manifest, Units::Mm, &[]);
        fixture.bodies = self.states.clone();
        fixture.history = self.chains.clone();
        fixture.upstream = self.outputs.clone();
        fixture
    }

    /// Calls `use_context` with the render context of `feature`: the document's names with the outputs of this
    /// timeline, millimeters, the states of its declared bodies just before it (none when it did not run) and
    /// `diagnosis`.
    pub fn with_context<T>(
        &self,
        feature: &str,
        diagnosis: &Diagnosis,
        use_context: impl FnOnce(&RenderContext<'_>) -> T,
    ) -> T {
        let feature = id(feature);
        let names = DocumentNames {
            document: &self.document,
            registry: &self.registry,
            outputs: &self.outputs,
        };
        let empty = BTreeMap::new();
        let bodies = self.before.get(&feature).unwrap_or(&empty);
        use_context(&RenderContext {
            feature: &feature,
            names: &names,
            units: Units::Mm,
            bodies,
            diagnosis,
        })
    }

    /// Calls `use_names` with the document's names and the outputs of this timeline, as messages read them.
    pub fn with_names<T>(&self, use_names: impl FnOnce(&dyn Names) -> T) -> T {
        use_names(&DocumentNames {
            document: &self.document,
            registry: &self.registry,
            outputs: &self.outputs,
        })
    }

    /// Calls `use_context` with the render context of `feature` over the states at the end of the timeline: for a
    /// feature that did not run (one that is suppressed, so that its label and type are known), whose warnings a test
    /// resolves through `fixture`.
    pub fn with_end_context<T>(
        &self,
        feature: &str,
        diagnosis: &Diagnosis,
        use_context: impl FnOnce(&RenderContext<'_>) -> T,
    ) -> T {
        let feature = id(feature);
        let names = DocumentNames {
            document: &self.document,
            registry: &self.registry,
            outputs: &self.outputs,
        };
        use_context(&RenderContext {
            feature: &feature,
            names: &names,
            units: Units::Mm,
            bodies: &self.states,
            diagnosis,
        })
    }

    /// `feature`'s error rendered with `diagnosis`.
    pub fn render_error(&self, feature: &str, diagnosis: &Diagnosis) -> String {
        let error = self.error(feature);
        self.with_context(feature, diagnosis, |context| error.render(context))
    }

    /// The fixes of `feature`'s error with `diagnosis`.
    pub fn error_fixes(&self, feature: &str, diagnosis: &Diagnosis) -> Vec<Fix> {
        let error = self.error(feature);
        self.with_context(feature, diagnosis, |context| error.fixes(context))
    }

    /// The warning of `feature` at `index` rendered with `diagnosis`.
    pub fn render_warning(&self, feature: &str, index: usize, diagnosis: &Diagnosis) -> String {
        let warning = &self.warnings(feature)[index];
        self.with_context(feature, diagnosis, |context| warning.render(context))
    }

    /// The fixes of the warning of `feature` at `index` with `diagnosis`.
    pub fn warning_fixes(&self, feature: &str, index: usize, diagnosis: &Diagnosis) -> Vec<Fix> {
        let warning = &self.warnings(feature)[index];
        self.with_context(feature, diagnosis, |context| warning.fixes(context))
    }
}

/// `Reference::from_element` of the element named `name` in `set`.
pub fn picked_in(set: &ElementSet, name: &str) -> Reference {
    let name = ElementName::parse(name).expect("element names written in tests are valid");
    let info = set
        .elements()
        .iter()
        .find(|info| info.name == name)
        .unwrap_or_else(|| panic!("the body has no element {name}"));
    Reference::from_element(info, set)
}

/// The names of resolved sub-shapes of `state`, in order.
pub fn sub_shape_names(state: &BodyState, sub_shapes: &[SubShape]) -> Vec<String> {
    sub_shapes
        .iter()
        .map(|sub_shape| {
            let kind = match sub_shape.kind {
                SubShapeKind::Face => ElementKind::Face,
                SubShapeKind::Edge => ElementKind::Edge,
                SubShapeKind::Vertex => ElementKind::Vertex,
                other => panic!("resolved sub-shapes are faces, edges or vertices, not {other:?}"),
            };
            state
                .naming()
                .name(kind, sub_shape.index)
                .unwrap_or_else(|| panic!("no {kind:?} with index {}", sub_shape.index))
                .to_string()
        })
        .collect()
}

/// A stored reference from its JSON text form.
pub fn stored(query: &str, fingerprint: &str) -> Reference {
    serde_json::from_value(json!({"q": query, "fp": fingerprint}))
        .expect("references written in tests are valid")
}
