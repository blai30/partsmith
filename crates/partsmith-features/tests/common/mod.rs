//! Test features, names and context fixtures shared by the acceptance tests of `docs/spec/features.md` sections 6.2 and 6.3.
//!
//! `Probe` and `Slab` are small features registered only by these tests. They let the registry and
//! context tests run before the built-in feature types exist: `Probe` is the "test feature registered
//! for the purpose" of section 6.3, and `Slab` has the same params shape as extrude.

// Each test binary uses only some of these helpers.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::{
    BodyOp, BodyState, BuildContext, BuildInputs, CancelToken, Category, Dependencies, ExprUse,
    Feature, FeatureError, FeatureManifest, FeatureOutput, FieldHint, FieldRef, FieldUnit, Link,
    Names, Noun, OptionHint, OutputData, ProfileRef, PythonCall, PythonNames, PythonValue,
    Registry, SelectionKind, SketchResult, Values,
};
use partsmith_model::{EmptyContext, Expr, FeatureRecord, Id, ParameterValues, Scope, Units};
use partsmith_sketch::{OriginPlane, RegionRef, SolveResult, build_profile};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Absolute tolerance for evaluated values, placements and axes (features.md section 6.1).
pub const TOLERANCE: f64 = 1e-9;

/// `id("x")` of the specs: a valid id written in a test.
pub fn id(text: &str) -> Id {
    Id::new(text).expect("ids written in tests are valid")
}

/// A feature record from its JSON form, keeping the key order of its params.
pub fn record(value: Value) -> FeatureRecord {
    serde_json::from_value(value).expect("records written in tests are well-formed")
}

/// The params of a record as compact JSON text, so comparisons see the key order.
pub fn params_text(record: &FeatureRecord) -> String {
    serde_json::to_string(&record.params).expect("params serialize")
}

/// A whole record as compact JSON text, so comparisons see the key order.
pub fn record_text(record: &FeatureRecord) -> String {
    serde_json::to_string(record).expect("records serialize")
}

/// A field hint with a path and a label and nothing else set.
pub fn hint(path: &str, label: &str) -> FieldHint {
    FieldHint {
        path: path.to_string(),
        label: label.to_string(),
        essential: false,
        unit: None,
        selection: None,
        options: Vec::new(),
    }
}

/// An essential hint for an expression field.
pub fn expression_hint(path: &str, label: &str, unit: FieldUnit) -> FieldHint {
    FieldHint {
        essential: true,
        unit: Some(unit),
        ..hint(path, label)
    }
}

/// A hint for a reference field.
pub fn selection_hint(path: &str, label: &str, selection: SelectionKind) -> FieldHint {
    FieldHint {
        selection: Some(selection),
        ..hint(path, label)
    }
}

/// An essential hint for an enumeration or tag field with (value, label) options.
pub fn options_hint(path: &str, label: &str, options: &[(&str, &str)]) -> FieldHint {
    FieldHint {
        essential: true,
        options: options
            .iter()
            .map(|(value, label)| OptionHint {
                value: value.to_string(),
                label: label.to_string(),
            })
            .collect(),
        ..hint(path, label)
    }
}

/// A version 1 manifest in the `create` category with the given label and fields.
pub fn manifest(label: &str, fields: Vec<FieldHint>) -> FeatureManifest {
    FeatureManifest {
        label: label.to_string(),
        category: Category::Create,
        icon: "test".to_string(),
        version: 1,
        fields,
    }
}

/// A field reference with an explicit label.
pub fn field(path: &str, label: &str) -> FieldRef {
    FieldRef {
        path: path.to_string(),
        label: label.to_string(),
    }
}

/// A declared expression use.
pub fn expression_use(field: &str, expr: &str, kind: FieldUnit) -> ExprUse {
    ExprUse {
        field: field.to_string(),
        expr: Expr::new(expr),
        kind,
    }
}

/// A link to `id` named at `field`.
pub fn link(id: &Id, field: &str) -> Link {
    Link {
        id: id.clone(),
        field: field.to_string(),
    }
}

/// Dependencies with every list empty.
pub fn no_dependencies() -> Dependencies {
    Dependencies {
        features: Vec::new(),
        bodies_read: Vec::new(),
        bodies_written: Vec::new(),
        expressions: Vec::new(),
        declared_ids: Vec::new(),
        assets: Vec::new(),
    }
}

/// An output with no data and no warnings.
pub fn nothing() -> FeatureOutput {
    FeatureOutput {
        data: OutputData::Nothing,
        warnings: Vec::new(),
    }
}

/// The links of a dependency list as (id, field) pairs, for comparisons.
pub fn link_pairs(links: &[Link]) -> Vec<(String, String)> {
    links
        .iter()
        .map(|link| (link.id.to_string(), link.field.clone()))
        .collect()
}

/// Expression uses as (field, expression, unit) triples, for comparisons.
pub fn use_triples(uses: &[ExprUse]) -> Vec<(String, String, FieldUnit)> {
    uses.iter()
        .map(|expression| {
            (
                expression.field.clone(),
                expression.expr.as_str().to_string(),
                expression.kind,
            )
        })
        .collect()
}

/// `Names` that knows no body and no feature, so messages show ids.
pub struct NoNames;

impl Names for NoNames {
    fn body_name(&self, _body: &Id) -> Option<String> {
        None
    }

    fn feature_label(&self, _feature: &Id) -> Option<String> {
        None
    }
}

/// The `PythonNames` of features.md section 6.2 test 22: units mm, body `b-1` as `block`,
/// parameter `height` as `height`, and default op new. Every other lookup finds nothing.
pub struct ScriptNames;

impl PythonNames for ScriptNames {
    fn units(&self) -> Units {
        Units::Mm
    }

    fn body(&self, body: &Id) -> Option<String> {
        (body.as_str() == "b-1").then(|| "block".to_string())
    }

    fn feature(&self, _feature: &Id) -> Option<String> {
        None
    }

    fn entity(&self, _sketch: &Id, _entity: &Id) -> Option<String> {
        None
    }

    fn parameter(&self, name: &str) -> Option<String> {
        (name == "height").then(|| "height".to_string())
    }

    fn regions(&self, _sketch: &Id) -> Option<Vec<RegionRef>> {
        None
    }

    fn default_op(&self, _body: &Id) -> BodyOp {
        BodyOp::New
    }
}

/// Params of the probe test feature: one expression of each unit the context tests evaluate.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProbeParams {
    /// A length.
    pub distance: Expr,
    /// An angle.
    pub angle: Expr,
    /// A whole number.
    pub count: Expr,
}

/// Test feature whose manifest labels its fields Distance, Angle and Count (features.md section 6.3).
/// Its Python hook always returns `probe(<distance>, <angle>, <count>)`.
pub struct Probe;

impl Feature for Probe {
    const TYPE: &'static str = "probe";
    type Params = ProbeParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Probe",
            vec![
                expression_hint("distance", "Distance", FieldUnit::Length),
                expression_hint("angle", "Angle", FieldUnit::Angle),
                expression_hint("count", "Count", FieldUnit::Count),
            ],
        )
    }

    fn dependencies(params: &ProbeParams) -> Dependencies {
        Dependencies {
            expressions: vec![
                expression_use("distance", params.distance.as_str(), FieldUnit::Length),
                expression_use("angle", params.angle.as_str(), FieldUnit::Angle),
                expression_use("count", params.count.as_str(), FieldUnit::Count),
            ],
            ..no_dependencies()
        }
    }

    fn build(
        _params: &ProbeParams,
        _context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        Ok(nothing())
    }

    fn python(params: &ProbeParams, names: &dyn PythonNames) -> Option<PythonCall> {
        Some(PythonCall {
            function: "probe".to_string(),
            positional: vec![
                PythonValue::expression(&params.distance, names),
                PythonValue::expression(&params.angle, names),
                PythonValue::expression(&params.count, names),
            ],
            keywords: Vec::new(),
            extra: Vec::new(),
        })
    }
}

/// Params of the slab test feature: the shape of extrude's params (features/extrude.md) with two extents.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SlabParams {
    /// Target body.
    pub body: Id,
    /// Closed regions of one sketch.
    pub profile: ProfileRef,
    /// How far the slab goes.
    pub extent: SlabExtent,
    /// Go against the sketch normal. Default false, omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub reverse: bool,
    /// How the slab combines with the body.
    pub op: BodyOp,
}

/// How far a slab goes.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlabExtent {
    /// One side.
    Distance {
        /// The distance.
        expr: Expr,
    },
    /// Both sides.
    TwoSides {
        /// The distance along the direction.
        expr: Expr,
        /// The distance against the direction.
        opposite_expr: Expr,
    },
}

/// Default test of `SlabParams::reverse`.
fn is_false(value: &bool) -> bool {
    !*value
}

/// The manifest of the slab test feature.
pub fn slab_manifest() -> FeatureManifest {
    manifest(
        "Slab",
        vec![
            FieldHint {
                essential: true,
                ..selection_hint("profile", "Profile", SelectionKind::SketchRegion)
            },
            options_hint(
                "extent.kind",
                "Extent",
                &[("distance", "Distance"), ("two_sides", "Two sides")],
            ),
            expression_hint("extent.expr", "Distance", FieldUnit::Length),
            expression_hint(
                "extent.opposite_expr",
                "Opposite distance",
                FieldUnit::Length,
            ),
            options_hint(
                "op",
                "Operation",
                &[
                    ("new", "New body"),
                    ("join", "Join"),
                    ("cut", "Cut"),
                    ("intersect", "Intersect"),
                ],
            ),
            hint("reverse", "Reverse"),
            selection_hint("body", "Body", SelectionKind::Body),
        ],
    )
}

/// Test feature with extrude's params shape. It rejects an empty profile in `validate`, declares
/// its sketch, body and expressions like extrude, and has a Python hook only for the distance extent.
pub struct Slab;

impl Feature for Slab {
    const TYPE: &'static str = "slab";
    type Params = SlabParams;

    fn manifest() -> FeatureManifest {
        slab_manifest()
    }

    fn dependencies(params: &SlabParams) -> Dependencies {
        let expressions = match &params.extent {
            SlabExtent::Distance { expr } => {
                vec![expression_use(
                    "extent.expr",
                    expr.as_str(),
                    FieldUnit::Length,
                )]
            }
            SlabExtent::TwoSides {
                expr,
                opposite_expr,
            } => vec![
                expression_use("extent.expr", expr.as_str(), FieldUnit::Length),
                expression_use(
                    "extent.opposite_expr",
                    opposite_expr.as_str(),
                    FieldUnit::Length,
                ),
            ],
        };
        Dependencies {
            features: vec![link(&params.profile.sketch, "profile.sketch")],
            bodies_written: vec![link(&params.body, "body")],
            expressions,
            ..no_dependencies()
        }
    }

    fn build(
        _params: &SlabParams,
        _context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        Ok(nothing())
    }

    fn validate(params: &SlabParams) -> Result<(), FeatureError> {
        if params.profile.regions.is_empty() {
            return Err(FeatureError::EmptySelection {
                field: FieldRef::new("profile", &slab_manifest()),
                noun: Noun::Region,
            });
        }
        Ok(())
    }

    fn python(params: &SlabParams, names: &dyn PythonNames) -> Option<PythonCall> {
        match &params.extent {
            SlabExtent::Distance { expr } => Some(PythonCall {
                function: "slab".to_string(),
                positional: vec![PythonValue::expression(expr, names)],
                keywords: Vec::new(),
                extra: Vec::new(),
            }),
            SlabExtent::TwoSides { .. } => None,
        }
    }
}

/// `P-RECT` of features/extrude.md section 6: the rectangle region of sketch `f-sk1`.
pub fn rectangle_profile() -> Value {
    json!({"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]})
}

/// A canonical slab record `f-slab1` on body `b-1` with a distance extent of 30 mm and op new.
pub fn slab_record() -> FeatureRecord {
    record(json!({
        "id": "f-slab1",
        "type": "slab",
        "body": "b-1",
        "profile": rectangle_profile(),
        "extent": {"kind": "distance", "expr": "30 mm"},
        "op": "new"
    }))
}

/// The record of features/extrude.md test 1: `EXT(P-RECT, {"kind": "distance", "expr": "30 mm"}, "new")`.
pub fn extrude_record() -> FeatureRecord {
    record(json!({
        "id": "f-ext1",
        "type": "extrude",
        "body": "b-1",
        "profile": rectangle_profile(),
        "extent": {"kind": "distance", "expr": "30 mm"},
        "op": "new"
    }))
}

/// `SK-RECT` of features/extrude.md section 6, the sketch of features/sketch.md test 1.
pub fn rectangle_sketch_record() -> FeatureRecord {
    record(json!({
        "id": "f-sk1",
        "type": "sketch",
        "plane": {"kind": "origin", "plane": "XY"},
        "entities": [
            {"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]},
            {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]},
            {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]},
            {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}
        ]
    }))
}

/// The output of a solved sketch on the XY plane with no entities.
pub fn empty_sketch_output() -> FeatureOutput {
    let solved: SolveResult = serde_json::from_value(json!({
        "status": "fully_constrained",
        "failed": false,
        "entities": [],
        "dof": 0,
        "redundant": [],
        "conflicting": [],
        "entity_states": {},
        "measured": {}
    }))
    .expect("an empty solve result deserializes");
    FeatureOutput {
        data: OutputData::Sketch(SketchResult {
            placement: OriginPlane::Xy.placement(),
            solved,
            profile: build_profile(&[]),
        }),
        warnings: Vec::new(),
    }
}

/// Owns everything a `BuildContext` borrows. Tests change the public fields, then call `context`.
pub struct ContextFixture {
    /// The feature being built.
    pub feature: Id,
    /// Its manifest.
    pub manifest: FeatureManifest,
    /// Document units.
    pub units: Units,
    /// Values of the declared expressions.
    pub values: Values,
    /// Declared bodies.
    pub bodies: BTreeMap<Id, BodyState>,
    /// Outputs of declared features.
    pub upstream: BTreeMap<Id, Arc<FeatureOutput>>,
    /// Bytes of declared assets.
    pub assets: BTreeMap<String, Arc<[u8]>>,
    /// Cancellation flag.
    pub cancel: CancelToken,
}

impl ContextFixture {
    /// A fixture for the probe feature `f-probe1` in a document with `units`, declaring `uses`
    /// as (field, expression, unit), evaluated without parameters or printer values.
    pub fn new(units: Units, uses: &[(&str, &str, FieldUnit)]) -> ContextFixture {
        let uses: Vec<ExprUse> = uses
            .iter()
            .map(|(field, expr, kind)| expression_use(field, expr, *kind))
            .collect();
        ContextFixture::with_manifest(id("f-probe1"), Probe::manifest(), units, &uses)
    }

    /// A fixture for any feature and manifest, declaring `uses`.
    pub fn with_manifest(
        feature: Id,
        manifest: FeatureManifest,
        units: Units,
        uses: &[ExprUse],
    ) -> ContextFixture {
        let values = evaluate(uses, units, &manifest);
        ContextFixture {
            feature,
            manifest,
            units,
            values,
            bodies: BTreeMap::new(),
            upstream: BTreeMap::new(),
            assets: BTreeMap::new(),
            cancel: CancelToken::new(),
        }
    }

    /// The context over this fixture's inputs.
    pub fn context(&self) -> BuildContext<'_> {
        BuildContext::new(BuildInputs {
            feature: &self.feature,
            manifest: &self.manifest,
            units: self.units,
            values: &self.values,
            bodies: &self.bodies,
            upstream: &self.upstream,
            assets: &self.assets,
            cancel: &self.cancel,
        })
    }
}

/// Evaluates `uses` in a document with `units`, no parameters and no printer values.
pub fn evaluate(uses: &[ExprUse], units: Units, manifest: &FeatureManifest) -> Values {
    let parameters = ParameterValues::default();
    let scope = Scope {
        units,
        parameters: &parameters,
        context: &EmptyContext,
    };
    Values::evaluate(uses, &scope, manifest)
}

/// Asserts that two vectors are equal within `TOLERANCE` per component.
#[track_caller]
pub fn assert_vector_close(actual: [f64; 3], expected: [f64; 3], what: &str) {
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(value, wanted)| (value - wanted).abs() <= TOLERANCE),
        "{what}: expected {expected:?} within {TOLERANCE}, got {actual:?}"
    );
}

/// The state that a solid feature record of the built-in registry, such as a box or a cylinder
/// with op new, gives its empty body in a millimeter document without parameters.
pub fn built_body(value: Value) -> BodyState {
    let registry = Registry::with_builtins();
    let validated = registry
        .validate_and_canonicalize(&record(value))
        .expect("records written in tests are valid");
    let dependencies = registry
        .dependencies(&validated.record)
        .expect("valid records have dependencies");
    let manifest = registry
        .get(&validated.record.kind)
        .expect("the type is built in")
        .manifest()
        .clone();
    let mut fixture = ContextFixture::with_manifest(
        validated.record.id.clone(),
        manifest,
        Units::Mm,
        &dependencies.expressions,
    );
    for link in &dependencies.bodies_written {
        fixture.bodies.insert(link.id.clone(), BodyState::empty());
    }
    let output = registry
        .build(&validated.record, &fixture.context())
        .expect("the record builds");
    let OutputData::Bodies(updates) = output.data else {
        panic!("a solid feature updates bodies");
    };
    let update = updates.into_iter().next().expect("one body update");
    BodyState::new(update.shape.expect("the body holds a solid"), update.naming)
}
