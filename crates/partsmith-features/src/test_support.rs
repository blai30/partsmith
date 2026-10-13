//! Fixtures shared by the unit tests: ids, a build context over owned inputs, named boxes, script
//! names for Python hooks, and features built from JSON params without a timeline. Documents of
//! built-in features are tested in `partsmith-recompute`.

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_model::{
    Category, EmptyContext, Expr, FeatureManifest, FeatureRecord, FieldHint, FieldUnit, Id,
    ParameterValues, Scope, Units,
};
use partsmith_naming::{ElementKind, NamingMap, Reference};
use partsmith_occt_sys::{Plane, Point, Primitive, Shape, make_box};
use partsmith_sketch::RegionRef;
use serde_json::Value;

use crate::builtin::common::primitive_tool;
use crate::context::{BodyState, BuildContext, BuildInputs, CancelToken, Values};
use crate::deps::ExprUse;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::output::{BodyUpdate, FeatureOutput, OutputData, Tool};
use crate::params::BodyOp;
use crate::python::PythonNames;

/// A valid id written in a test.
pub(crate) fn id(text: &str) -> Id {
    Id::new(text).expect("ids written in tests are valid")
}

/// The values of these `(field, expression, unit)` uses in a millimeter document without
/// parameters.
pub(crate) fn values(uses: &[(&str, &str, FieldUnit)]) -> Values {
    let uses: Vec<ExprUse> = uses
        .iter()
        .map(|(field, expr, kind)| ExprUse {
            field: field.to_string(),
            expr: Expr::new(*expr),
            kind: *kind,
        })
        .collect();
    let parameters = ParameterValues::default();
    let scope = Scope {
        units: Units::Mm,
        parameters: &parameters,
        context: &EmptyContext,
    };
    Values::evaluate(&uses, &scope, &manifest(Vec::new()))
}

/// A stored reference parsed from its JSON form.
pub(crate) fn reference(json: &str) -> Reference {
    serde_json::from_str(json).expect("references written in tests are valid")
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

/// The typed params of a feature record written as JSON.
pub(crate) fn params<F: Feature>(record: Value) -> F::Params {
    let record: FeatureRecord =
        serde_json::from_value(record).expect("records written in tests are well-formed");
    serde_json::from_value(Value::Object(record.params))
        .expect("params written in tests deserialize")
}

/// The typed params of a feature record written as JSON, with the members of the JSON object
/// `extra` added to it or replacing its own.
pub(crate) fn params_with<F: Feature>(mut record: Value, extra: Value) -> F::Params {
    let Value::Object(members) = extra else {
        panic!("extra members written in tests are a JSON object, not {extra}");
    };
    for (key, value) in members {
        record[key] = value;
    }
    params::<F>(record)
}

/// The fixture of feature `feature` with `params` in a millimeter document without parameters:
/// its declared expressions evaluated, every declared body empty unless `bodies` gives its state,
/// and `upstream` as the outputs of the features it reads.
pub(crate) fn fixture<F: Feature>(
    feature: &str,
    params: &F::Params,
    bodies: Vec<(&str, BodyState)>,
    upstream: Vec<(&str, FeatureOutput)>,
) -> Fixture {
    let dependencies = F::dependencies(params);
    let mut fixture = Fixture::new(feature);
    fixture.manifest = F::manifest();
    let parameters = ParameterValues::default();
    let scope = Scope {
        units: Units::Mm,
        parameters: &parameters,
        context: &EmptyContext,
    };
    fixture.values = Values::evaluate(&dependencies.expressions, &scope, &fixture.manifest);
    for link in dependencies
        .bodies_written
        .iter()
        .chain(&dependencies.bodies_read)
    {
        fixture.bodies.insert(link.id.clone(), BodyState::empty());
    }
    for (body, state) in bodies {
        fixture.bodies.insert(id(body), state);
    }
    for (feature, output) in upstream {
        fixture.upstream.insert(id(feature), Arc::new(output));
    }
    fixture
}

/// Builds `params` as feature `feature` over the fixture that `fixture` makes for them.
pub(crate) fn build<F: Feature>(
    feature: &str,
    params: &F::Params,
    bodies: Vec<(&str, BodyState)>,
    upstream: Vec<(&str, FeatureOutput)>,
) -> Result<FeatureOutput, FeatureError> {
    let fixture = fixture::<F>(feature, params, bodies, upstream);
    F::build(params, &fixture.context())
}

/// The body updates of an output.
pub(crate) fn updates(output: &FeatureOutput) -> &[BodyUpdate] {
    match &output.data {
        OutputData::Bodies(updates) => updates,
        other => panic!("expected body updates, got {other:?}"),
    }
}

/// The update of `body` in an output.
pub(crate) fn update<'a>(output: &'a FeatureOutput, body: &str) -> &'a BodyUpdate {
    updates(output)
        .iter()
        .find(|update| update.body.as_str() == body)
        .unwrap_or_else(|| panic!("no update of {body}"))
}

/// The shape the update of `body` gives it; fails the test when it empties the body.
pub(crate) fn updated_shape<'a>(output: &'a FeatureOutput, body: &str) -> &'a Shape {
    update(output, body)
        .shape
        .as_ref()
        .unwrap_or_else(|| panic!("{body} is empty after the feature"))
}

/// Asserts a shape's volume within the relative tolerance of features.md section 6.1.
pub(crate) fn assert_volume(shape: &Shape, expected: f64) {
    assert_relative("volume", volume(shape), expected);
}

/// Asserts a shape's tight bounding box within the 1e-5 mm per coordinate of features.md section
/// 6.1.
pub(crate) fn assert_bounds(shape: &Shape, min: [f64; 3], max: [f64; 3]) {
    let bounds = shape.bounding_box().expect("the shape has bounds");
    let corner = |[x, y, z]: [f64; 3]| Point::new(x, y, z);
    assert_point("bounding box minimum", corner(bounds.min), min);
    assert_point("bounding box maximum", corner(bounds.max), max);
}

/// The dedicated Python call of `params` with the default `ScriptNames`, rendered at indentation
/// 0, or None.
pub(crate) fn python<F: Feature>(params: &F::Params) -> Option<String> {
    python_with::<F>(params, &ScriptNames::default())
}

/// The dedicated Python call of `params` with `names`, rendered at indentation 0, or None.
pub(crate) fn python_with<F: Feature>(params: &F::Params, names: &ScriptNames) -> Option<String> {
    F::python(params, names).map(|call| call.render(0))
}

/// Relative tolerance of volumes and areas (features.md section 6.1).
const MEASURE_TOLERANCE: f64 = 1e-6;

/// Absolute tolerance of bounding box coordinates and positions in millimeters.
const LENGTH_TOLERANCE: f64 = 1e-5;

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
