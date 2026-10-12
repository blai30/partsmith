//! Fixtures shared by the unit tests: ids, a build context over owned inputs, named boxes, script
//! names for Python hooks, and a small timeline runner for documents of built-in features.

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_mesh::BoundingBox;
use partsmith_model::{
    Category, Document, EmptyContext, FeatureManifest, FieldHint, Id, Scope, Units,
};
use partsmith_naming::{ElementKind, ElementName, NamingMap};
use partsmith_occt_sys::{ElementGeometry, Plane, Point, Primitive, Shape, SubShapeKind, make_box};
use partsmith_sketch::RegionRef;
use serde_json::Value;

use crate::builtin::common::primitive_tool;
use crate::context::{BodyState, BuildContext, BuildInputs, CancelToken, Values};
use crate::error::{DocumentNames, FeatureError, FeatureWarning};
use crate::output::{FeatureOutput, OutputData, SketchResult, Tool};
use crate::params::BodyOp;
use crate::python::PythonNames;
use crate::registry::Registry;

/// A valid id written in a test.
pub(crate) fn id(text: &str) -> Id {
    Id::new(text).expect("ids written in tests are valid")
}

/// A version 1 manifest labeled "Test" with the given fields.
pub(crate) fn manifest(fields: Vec<FieldHint>) -> FeatureManifest {
    FeatureManifest {
        label: "Test".to_string(),
        category: Category::Create,
        icon: "test".to_string(),
        version: 1,
        fields,
    }
}

/// Owns everything a `BuildContext` borrows. Tests fill the public fields, then call `context`.
pub(crate) struct Fixture {
    /// The feature being built.
    pub(crate) feature: Id,
    /// Its manifest.
    pub(crate) manifest: FeatureManifest,
    /// Values of the declared expressions.
    pub(crate) values: Values,
    /// Declared bodies.
    pub(crate) bodies: BTreeMap<Id, BodyState>,
    /// Outputs of declared features.
    pub(crate) upstream: BTreeMap<Id, Arc<FeatureOutput>>,
    /// Bytes of declared assets.
    pub(crate) assets: BTreeMap<String, Arc<[u8]>>,
    /// Cancellation flag.
    pub(crate) cancel: CancelToken,
}

impl Fixture {
    /// A fixture for feature `feature` with an empty manifest and no inputs.
    pub(crate) fn new(feature: &str) -> Fixture {
        Fixture {
            feature: id(feature),
            manifest: manifest(Vec::new()),
            values: Values::default(),
            bodies: BTreeMap::new(),
            upstream: BTreeMap::new(),
            assets: BTreeMap::new(),
            cancel: CancelToken::new(),
        }
    }

    /// Declares `body` holding `state`.
    pub(crate) fn with_body(mut self, body: &str, state: BodyState) -> Fixture {
        self.bodies.insert(id(body), state);
        self
    }

    /// The context over this fixture's inputs, in a millimeter document.
    pub(crate) fn context(&self) -> BuildContext<'_> {
        BuildContext::new(BuildInputs {
            feature: &self.feature,
            manifest: &self.manifest,
            units: Units::Mm,
            values: &self.values,
            bodies: &self.bodies,
            upstream: &self.upstream,
            assets: &self.assets,
            cancel: &self.cancel,
        })
    }
}

/// A primitive with its faces named `<feature>/<role>` from the kernel's face roles.
pub(crate) fn named_primitive(feature: &str, primitive: Primitive) -> Tool {
    primitive_tool(&id(feature), primitive).expect("the primitive is named")
}

/// An axis-aligned box from `corner` with edge length `size`, named like `named_primitive`.
pub(crate) fn named_box(feature: &str, corner: [f64; 3], size: f64) -> Tool {
    let placement = Plane {
        origin: Point::new(corner[0], corner[1], corner[2]),
        ..Plane::XY
    };
    named_primitive(
        feature,
        make_box(&placement, size, size, size).expect("the box builds"),
    )
}

/// A body state holding a named box.
pub(crate) fn box_body(feature: &str, corner: [f64; 3], size: f64) -> BodyState {
    let tool = named_box(feature, corner, size);
    BodyState::new(tool.shape, Arc::new(tool.naming))
}

/// The face names of a naming map, sorted.
pub(crate) fn face_names(naming: &NamingMap) -> Vec<String> {
    let mut names: Vec<String> = naming
        .names(ElementKind::Face)
        .map(|(_, name)| name.to_string())
        .collect();
    names.sort_unstable();
    names
}

/// The volume of a shape.
pub(crate) fn volume(shape: &Shape) -> f64 {
    shape.volume().expect("the shape has a volume")
}

/// Script variables for Python hooks. By default: units mm, `b-1` is `block`, `b-2` is `part_2`,
/// `b-3` is `part_3`, `f-sk1` is `sketch_1`, `f-sk2` is `sketch_2`, `f-dp1` is `plane_1`, no
/// entities, parameters or solved regions, and default op new for every body.
pub(crate) struct ScriptNames {
    /// The document units.
    pub(crate) units: Units,
    /// Part variables by body id.
    pub(crate) bodies: BTreeMap<Id, String>,
    /// Feature variables by feature id.
    pub(crate) features: BTreeMap<Id, String>,
    /// Entity variables by sketch and entity id.
    pub(crate) entities: BTreeMap<(Id, Id), String>,
    /// Parameter variables by parameter name.
    pub(crate) parameters: BTreeMap<String, String>,
    /// Every region of each solved sketch, in solved order.
    pub(crate) regions: BTreeMap<Id, Vec<RegionRef>>,
    /// Default ops that are not new, by body id.
    pub(crate) default_ops: BTreeMap<Id, BodyOp>,
}

impl Default for ScriptNames {
    fn default() -> ScriptNames {
        let variables = |pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .map(|(key, variable)| (id(key), variable.to_string()))
                .collect()
        };
        ScriptNames {
            units: Units::Mm,
            bodies: variables(&[("b-1", "block"), ("b-2", "part_2"), ("b-3", "part_3")]),
            features: variables(&[
                ("f-sk1", "sketch_1"),
                ("f-sk2", "sketch_2"),
                ("f-dp1", "plane_1"),
            ]),
            entities: BTreeMap::new(),
            parameters: BTreeMap::new(),
            regions: BTreeMap::new(),
            default_ops: BTreeMap::new(),
        }
    }
}

impl PythonNames for ScriptNames {
    fn units(&self) -> Units {
        self.units
    }

    fn body(&self, body: &Id) -> Option<String> {
        self.bodies.get(body).cloned()
    }

    fn feature(&self, feature: &Id) -> Option<String> {
        self.features.get(feature).cloned()
    }

    fn entity(&self, sketch: &Id, entity: &Id) -> Option<String> {
        self.entities
            .get(&(sketch.clone(), entity.clone()))
            .cloned()
    }

    fn parameter(&self, name: &str) -> Option<String> {
        self.parameters.get(name).cloned()
    }

    fn regions(&self, sketch: &Id) -> Option<Vec<RegionRef>> {
        self.regions.get(sketch).cloned()
    }

    fn default_op(&self, body: &Id) -> BodyOp {
        self.default_ops.get(body).copied().unwrap_or(BodyOp::New)
    }
}

/// Relative tolerance of volumes and areas (features.md section 6.1).
const MEASURE_TOLERANCE: f64 = 1e-6;

/// Absolute tolerance of bounding box coordinates and positions in millimeters.
const LENGTH_TOLERANCE: f64 = 1e-5;

/// A document run through the built-in registry feature by feature, as recompute runs a
/// timeline: suppressed features are skipped, a feature whose declared feature failed reports
/// `UpstreamFailed`, and the body updates of each successful build are applied in order.
pub(crate) struct DocumentRun {
    /// The document.
    pub(crate) document: Document,
    /// The registry with every built-in feature.
    pub(crate) registry: Registry,
    /// The result of every feature that ran.
    pub(crate) results: BTreeMap<Id, Result<Arc<FeatureOutput>, FeatureError>>,
    /// The state of every body at the end.
    pub(crate) bodies: BTreeMap<Id, BodyState>,
    /// The states of the declared bodies just before each feature.
    pub(crate) before: BTreeMap<Id, BTreeMap<Id, BodyState>>,
}

/// A millimeter document with these test bodies (`b-<n>` named `Body <n>`) and feature records.
pub(crate) fn run(bodies: &[&str], features: &[Value]) -> DocumentRun {
    run_document("mm", &[], bodies, features, &BTreeMap::new())
}

/// A document in `units` with these parameter records, test bodies, feature records and assets.
pub(crate) fn run_document(
    units: &str,
    parameters: &[Value],
    bodies: &[&str],
    features: &[Value],
    assets: &BTreeMap<String, Arc<[u8]>>,
) -> DocumentRun {
    let bodies: Vec<Value> = bodies
        .iter()
        .map(|body| {
            let number = body.strip_prefix("b-").expect("test bodies look like b-1");
            serde_json::json!({"id": body, "name": format!("Body {number}")})
        })
        .collect();
    let text = serde_json::json!({
        "format": "partsmith",
        "format_version": 1,
        "units": units,
        "parameters": parameters,
        "bodies": bodies,
        "features": features,
    })
    .to_string();
    let document = partsmith_model::load(&text)
        .unwrap_or_else(|error| panic!("the test document does not load: {error}"))
        .document;
    DocumentRun::new(document, assets)
}

impl DocumentRun {
    /// Runs every feature of `document` in timeline order.
    fn new(document: Document, assets: &BTreeMap<String, Arc<[u8]>>) -> DocumentRun {
        let registry = Registry::with_builtins();
        let parameters = document.evaluate_parameters(&EmptyContext);
        let scope = Scope {
            units: document.units,
            parameters: &parameters,
            context: &EmptyContext,
        };
        let cancel = CancelToken::new();
        let mut run = DocumentRun {
            bodies: document
                .bodies
                .iter()
                .map(|body| (body.id.clone(), BodyState::empty()))
                .collect(),
            document: document.clone(),
            registry,
            results: BTreeMap::new(),
            before: BTreeMap::new(),
        };
        for record in document.features.iter().filter(|record| !record.suppressed) {
            let result = run.build(record, &scope, assets, &cancel);
            if let Ok(output) = &result
                && let OutputData::Bodies(updates) = &output.data
            {
                for update in updates {
                    let state = match &update.shape {
                        Some(shape) => BodyState::new(shape.clone(), update.naming.clone()),
                        None => BodyState::empty(),
                    };
                    run.bodies.insert(update.body.clone(), state);
                }
            }
            run.results.insert(record.id.clone(), result.map(Arc::new));
        }
        run
    }

    /// Validates, evaluates and builds one record with the states and outputs so far.
    fn build(
        &mut self,
        record: &partsmith_model::FeatureRecord,
        scope: &Scope<'_>,
        assets: &BTreeMap<String, Arc<[u8]>>,
        cancel: &CancelToken,
    ) -> Result<FeatureOutput, FeatureError> {
        let validated = self.registry.validate_and_canonicalize(record)?;
        let dependencies = self.registry.dependencies(&validated.record)?;
        let manifest = self
            .registry
            .get(&record.kind)
            .expect("a validated record has a registered type")
            .manifest()
            .clone();
        let values = Values::evaluate(&dependencies.expressions, scope, &manifest);
        let mut upstream = BTreeMap::new();
        for link in &dependencies.features {
            match self.results.get(&link.id) {
                Some(Ok(output)) => {
                    upstream.insert(link.id.clone(), Arc::clone(output));
                }
                _ => {
                    return Err(FeatureError::UpstreamFailed {
                        feature: link.id.clone(),
                    });
                }
            }
        }
        let mut bodies = BTreeMap::new();
        for link in dependencies
            .bodies_written
            .iter()
            .chain(&dependencies.bodies_read)
        {
            let state = self
                .bodies
                .get(&link.id)
                .ok_or_else(|| FeatureError::UnknownBody {
                    field: crate::error::FieldRef::new(&link.field, &manifest),
                    body: link.id.clone(),
                })?;
            bodies.insert(link.id.clone(), state.clone());
        }
        self.before.insert(record.id.clone(), bodies.clone());
        let assets: BTreeMap<String, Arc<[u8]>> = dependencies
            .assets
            .iter()
            .filter_map(|asset| {
                let bytes = assets.get(&asset.name)?;
                Some((asset.name.clone(), Arc::clone(bytes)))
            })
            .collect();
        let context = BuildContext::new(BuildInputs {
            feature: &record.id,
            manifest: &manifest,
            units: scope.units,
            values: &values,
            bodies: &bodies,
            upstream: &upstream,
            assets: &assets,
            cancel,
        });
        self.registry.build(&validated.record, &context)
    }

    /// The output of a feature that built; fails the test otherwise.
    pub(crate) fn output(&self, feature: &str) -> &FeatureOutput {
        match self.results.get(&id(feature)) {
            Some(Ok(output)) => output,
            Some(Err(error)) => panic!("{feature} failed: {error}"),
            None => panic!("{feature} did not run"),
        }
    }

    /// The error of a feature that failed; fails the test otherwise.
    pub(crate) fn error(&self, feature: &str) -> &FeatureError {
        match self.results.get(&id(feature)) {
            Some(Err(error)) => error,
            Some(Ok(output)) => panic!("{feature} built with warnings {:?}", output.warnings),
            None => panic!("{feature} did not run"),
        }
    }

    /// Asserts that a feature built without warnings.
    pub(crate) fn assert_ok(&self, feature: &str) {
        let output = self.output(feature);
        assert!(
            output.warnings.is_empty(),
            "{feature} warned: {:?}",
            self.messages(feature)
        );
    }

    /// The error code of a failed feature.
    pub(crate) fn error_code(&self, feature: &str) -> &'static str {
        self.error(feature).code()
    }

    /// The warning codes of a feature that built.
    pub(crate) fn warning_codes(&self, feature: &str) -> Vec<&'static str> {
        self.output(feature)
            .warnings
            .iter()
            .map(FeatureWarning::code)
            .collect()
    }

    /// The rendered messages of a feature: its error, or its warnings.
    pub(crate) fn messages(&self, feature: &str) -> Vec<String> {
        let names = DocumentNames {
            document: &self.document,
            registry: &self.registry,
        };
        let units = self.document.units;
        match self.results.get(&id(feature)) {
            Some(Ok(output)) => output
                .warnings
                .iter()
                .map(|warning| warning.render(&names, units))
                .collect(),
            Some(Err(error)) => vec![error.render(&names, units)],
            None => Vec::new(),
        }
    }

    /// The sketch result of a sketch feature.
    pub(crate) fn sketch(&self, feature: &str) -> &SketchResult {
        match &self.output(feature).data {
            OutputData::Sketch(sketch) => sketch,
            _ => panic!("{feature} is not a sketch"),
        }
    }

    /// The final state of a body.
    pub(crate) fn state(&self, body: &str) -> &BodyState {
        self.bodies
            .get(&id(body))
            .unwrap_or_else(|| panic!("the document has no body {body}"))
    }

    /// The final shape of a body; fails the test when it is empty.
    pub(crate) fn shape(&self, body: &str) -> &Shape {
        self.state(body)
            .shape()
            .unwrap_or_else(|| panic!("body {body} is empty"))
    }

    /// True when a body is empty at the end.
    pub(crate) fn is_empty(&self, body: &str) -> bool {
        self.state(body).is_empty()
    }

    /// The face names of a body, sorted.
    pub(crate) fn face_names(&self, body: &str) -> Vec<String> {
        face_names(self.state(body).naming())
    }

    /// The names of a body's faces that start with `prefix`, sorted.
    pub(crate) fn faces_named(&self, body: &str, prefix: &str) -> Vec<String> {
        self.face_names(body)
            .into_iter()
            .filter(|name| name.starts_with(prefix))
            .collect()
    }

    /// The geometry of the face of a body named `name`.
    pub(crate) fn face(&self, body: &str, name: &str) -> ElementGeometry {
        self.element(body, name, SubShapeKind::Face)
    }

    /// The geometry of the element of a body named `name`.
    pub(crate) fn element(&self, body: &str, name: &str, kind: SubShapeKind) -> ElementGeometry {
        let element = ElementName::parse(name).expect("valid element name");
        let index = self
            .state(body)
            .naming()
            .index(&element)
            .unwrap_or_else(|| {
                panic!(
                    "body {body} has no element {name}; faces {:?}",
                    self.face_names(body)
                )
            });
        self.shape(body)
            .element(kind, index)
            .expect("the kernel reads element geometry")
    }

    /// Asserts a body's volume within the relative tolerance.
    pub(crate) fn assert_volume(&self, body: &str, expected: f64) {
        assert_relative("volume", volume(self.shape(body)), expected);
    }

    /// Asserts the number of faces of a body.
    pub(crate) fn assert_faces(&self, body: &str, expected: u32) {
        assert_eq!(
            self.shape(body).count(SubShapeKind::Face),
            expected,
            "face count of {body}: {:?}",
            self.face_names(body)
        );
    }

    /// Asserts a body's tight bounding box within 1e-5 mm per coordinate.
    pub(crate) fn assert_bounding_box(&self, body: &str, min: [f64; 3], max: [f64; 3]) {
        let BoundingBox {
            min: actual_min,
            max: actual_max,
        } = self
            .shape(body)
            .bounding_box()
            .expect("the body has bounds");
        for (actual, expected) in actual_min
            .iter()
            .chain(&actual_max)
            .zip(min.iter().chain(&max))
        {
            assert!(
                (actual - expected).abs() <= LENGTH_TOLERANCE,
                "bounding box of {body}: {actual_min:?}-{actual_max:?}, expected {min:?}-{max:?}"
            );
        }
    }

    /// Asserts the area of a body's face within the relative tolerance.
    pub(crate) fn assert_face_area(&self, body: &str, face: &str, expected: f64) {
        assert_relative(face, self.face(body, face).measure, expected);
    }

    /// The state of `body` just before `feature` ran.
    pub(crate) fn before(&self, feature: &str, body: &str) -> &BodyState {
        &self.before[&id(feature)][&id(body)]
    }

    /// Asserts that a body is the same at the end as just before `feature`: same shape volume, face
    /// count and element names.
    pub(crate) fn assert_unchanged_by(&self, feature: &str, body: &str) {
        let before = self.before(feature, body);
        let after = self.state(body);
        match (before.shape(), after.shape()) {
            (Some(before_shape), Some(after_shape)) => {
                assert_relative("volume", volume(after_shape), volume(before_shape));
                assert_eq!(
                    after_shape.count(SubShapeKind::Face),
                    before_shape.count(SubShapeKind::Face)
                );
            }
            (None, None) => {}
            _ => panic!("{body} changed between empty and solid"),
        }
        for kind in [ElementKind::Face, ElementKind::Edge, ElementKind::Vertex] {
            let names = |state: &BodyState| -> Vec<String> {
                let mut names: Vec<String> = state
                    .naming()
                    .names(kind)
                    .map(|(_, name)| name.to_string())
                    .collect();
                names.sort();
                names
            };
            assert_eq!(names(after), names(before), "{kind:?} names of {body}");
        }
    }
}

/// Asserts `actual` equals `expected` within the relative tolerance (absolute near zero).
pub(crate) fn assert_relative(what: &str, actual: f64, expected: f64) {
    let difference = (actual - expected).abs();
    assert!(
        difference <= MEASURE_TOLERANCE * expected.abs().max(1.0),
        "{what}: expected {expected}, got {actual}"
    );
}

/// Asserts that a point is within 1e-5 mm of the expected coordinates.
pub(crate) fn assert_point(what: &str, actual: Point, expected: [f64; 3]) {
    let coordinates = [actual.x, actual.y, actual.z];
    for (component, wanted) in coordinates.iter().zip(expected) {
        assert!(
            (component - wanted).abs() <= LENGTH_TOLERANCE,
            "{what}: expected {expected:?}, got {coordinates:?}"
        );
    }
}
