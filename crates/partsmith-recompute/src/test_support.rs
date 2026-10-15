//! Test-only feature types and document helpers for the unit tests of this crate.
//!
//! The features are as small as the runner allows: datum planes with an offset and an optional
//! base datum, a body writer that keeps the body's state, a body reader, an asset reader and a
//! feature that panics. None of them calls the kernel.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use partsmith_features::{
    AssetLink, BodyChange, BodyUpdate, BuildContext, DatumResult, Dependencies, ExprUse, Feature,
    FeatureError, FeatureOutput, Link, OutputData, Registry,
};
use partsmith_model::{
    Category, Document, Expr, FeatureManifest, FeatureRecord, FieldHint, FieldUnit, Id,
    SelectionKind,
};
use partsmith_sketch::Placement;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::Input;
use crate::timeline::Timeline;
use crate::validation::Validations;

/// A registry with every test feature of this module.
pub(crate) fn registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .register::<TestDatum>()
        .expect("test_datum registers");
    registry
        .register::<TestWriter>()
        .expect("test_writer registers");
    registry
        .register::<TestReader>()
        .expect("test_reader registers");
    registry
        .register::<TestAsset>()
        .expect("test_asset registers");
    registry
        .register::<TestPanic>()
        .expect("test_panic registers");
    registry
}

/// The registry of [`registry`], built once for the helpers whose results borrow it.
fn shared_registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(registry)
}

/// The timeline of `document` prepared without a stop point, over a fresh validation memo.
pub(crate) fn prepared(document: &Document) -> Timeline<'_> {
    prepared_with(document, None)
}

/// The timeline of `document` prepared to stop after `stop`, over a fresh validation memo.
pub(crate) fn prepared_until<'a>(document: &'a Document, stop: &Id) -> Timeline<'a> {
    prepared_with(document, Some(stop))
}

/// The timeline of `document` prepared over the registry of [`registry`] and a fresh validation
/// memo, stopping after `stop` when it is set.
fn prepared_with<'a>(document: &'a Document, stop: Option<&Id>) -> Timeline<'a> {
    Timeline::prepare(
        shared_registry(),
        &mut Validations::default(),
        document,
        stop,
    )
}

/// A `test_datum` record offset by `offset`.
pub(crate) fn datum(feature: &str, offset: &str) -> Value {
    json!({"id": feature, "type": "test_datum", "offset": offset})
}

/// A `test_datum` record offset by 1 from the datum `base`.
pub(crate) fn datum_on(feature: &str, base: &str) -> Value {
    json!({"id": feature, "type": "test_datum", "base": base, "offset": "1"})
}

/// A `test_writer` record of `body`.
pub(crate) fn writer(feature: &str, body: &str) -> Value {
    json!({"id": feature, "type": "test_writer", "body": body})
}

/// A feature record from its JSON form.
pub(crate) fn record(value: Value) -> FeatureRecord {
    serde_json::from_value(value).expect("the test record deserializes")
}

/// A millimeter document with bodies `b-1` to `b-3`, parameters `h = 2 mm` and `w = h + 1 mm`,
/// and these feature records.
pub(crate) fn document(features: Value) -> Document {
    document_with(json!({"features": features}))
}

/// The document of [`document`] with `members` replacing or adding top-level members.
pub(crate) fn document_with(members: Value) -> Document {
    let mut text = json!({
        "format": "partsmith",
        "format_version": 1,
        "units": "mm",
        "parameters": [
            {"id": "p-h", "name": "h", "expr": "2 mm"},
            {"id": "p-w", "name": "w", "expr": "h + 1 mm"}
        ],
        "bodies": [
            {"id": "b-1", "name": "Body 1"},
            {"id": "b-2", "name": "Body 2"},
            {"id": "b-3", "name": "Body 3"}
        ],
        "features": []
    });
    for (key, value) in members.as_object().expect("members are an object") {
        text[key] = value.clone();
    }
    partsmith_model::load(&text.to_string())
        .expect("the test document loads")
        .document
}

/// The input of a whole-timeline run of `document` with no context values and these assets.
pub(crate) fn input<'a>(
    document: &'a Document,
    assets: &'a BTreeMap<String, Arc<[u8]>>,
) -> Input<'a> {
    Input {
        document,
        context: &partsmith_model::EmptyContext,
        stop_at: None,
        assets,
    }
}

/// A test id.
pub(crate) fn id(text: &str) -> Id {
    Id::new(text).expect("valid test id")
}

/// A manifest labeled `label` with these hints.
fn manifest(label: &str, fields: Vec<FieldHint>) -> FeatureManifest {
    FeatureManifest {
        label: label.to_string(),
        category: Category::Construct,
        icon: "test".to_string(),
        version: 1,
        fields,
    }
}

/// A hint for `path` labeled `label`.
fn hint(
    path: &str,
    label: &str,
    unit: Option<FieldUnit>,
    selection: Option<SelectionKind>,
) -> FieldHint {
    FieldHint {
        path: path.to_string(),
        label: label.to_string(),
        essential: true,
        unit,
        selection,
        options: Vec::new(),
    }
}

/// A link to `id` named at `field`.
fn link(id: &Id, field: &str) -> Link {
    Link {
        id: id.clone(),
        field: field.to_string(),
    }
}

/// An output with no warnings.
fn output(data: OutputData) -> FeatureOutput {
    FeatureOutput {
        data,
        warnings: Vec::new(),
        absent: Vec::new(),
    }
}

/// Params of `test_datum`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub(crate) struct TestDatumParams {
    /// A datum this one is offset from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    base: Option<Id>,
    /// Offset along +Z. Length.
    offset: Expr,
}

/// `test_datum`: a plane parallel to XY at the height of its base datum (0 without one) plus `offset`.
pub(crate) struct TestDatum;

impl Feature for TestDatum {
    const TYPE: &'static str = "test_datum";
    type Params = TestDatumParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Test datum",
            vec![
                hint("base", "Base", None, Some(SelectionKind::Plane)),
                hint("offset", "Offset", Some(FieldUnit::Length), None),
            ],
        )
    }

    fn dependencies(params: &TestDatumParams) -> Dependencies {
        Dependencies {
            features: params.base.iter().map(|base| link(base, "base")).collect(),
            expressions: vec![ExprUse {
                field: "offset".to_string(),
                expr: params.offset.clone(),
                kind: FieldUnit::Length,
            }],
            ..Dependencies::default()
        }
    }

    fn build(
        params: &TestDatumParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let base = match &params.base {
            Some(base) => match context.datum("base", base)? {
                DatumResult::Plane(placement) => placement.origin[2],
            },
            None => 0.0,
        };
        let offset = context.eval_length("offset", &params.offset)?;
        Ok(output(OutputData::Datum(DatumResult::Plane(Placement {
            origin: [0.0, 0.0, base + offset],
            x_axis: [1.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        }))))
    }
}

/// Params naming one body.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub(crate) struct TestBodyParams {
    /// The body.
    body: Id,
}

/// `test_writer`: declares `bodies_written [body]` and rewrites the body with its current state.
pub(crate) struct TestWriter;

impl Feature for TestWriter {
    const TYPE: &'static str = "test_writer";
    type Params = TestBodyParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Test writer",
            vec![hint("body", "Body", None, Some(SelectionKind::Body))],
        )
    }

    fn dependencies(params: &TestBodyParams) -> Dependencies {
        Dependencies {
            bodies_written: vec![link(&params.body, "body")],
            ..Dependencies::default()
        }
    }

    fn build(
        params: &TestBodyParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let state = context.body(&params.body)?;
        Ok(output(OutputData::Bodies(vec![BodyUpdate {
            body: params.body.clone(),
            shape: state.shape().cloned(),
            naming: state.naming().clone(),
            change: BodyChange::Modify,
            measured: state.measurements(),
        }])))
    }
}

/// `test_reader`: declares `bodies_read [body]`, reads it and outputs nothing.
pub(crate) struct TestReader;

impl Feature for TestReader {
    const TYPE: &'static str = "test_reader";
    type Params = TestBodyParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Test reader",
            vec![hint("body", "Body", None, Some(SelectionKind::Body))],
        )
    }

    fn dependencies(params: &TestBodyParams) -> Dependencies {
        Dependencies {
            bodies_read: vec![link(&params.body, "body")],
            ..Dependencies::default()
        }
    }

    fn build(
        params: &TestBodyParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        context.body(&params.body)?;
        Ok(output(OutputData::Nothing))
    }
}

/// Params of `test_asset`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub(crate) struct TestAssetParams {
    /// The asset name.
    asset: String,
}

/// `test_asset`: declares the asset `asset`, reads it and outputs nothing.
pub(crate) struct TestAsset;

impl Feature for TestAsset {
    const TYPE: &'static str = "test_asset";
    type Params = TestAssetParams;

    fn manifest() -> FeatureManifest {
        manifest("Test asset", vec![hint("asset", "File", None, None)])
    }

    fn dependencies(params: &TestAssetParams) -> Dependencies {
        Dependencies {
            assets: vec![AssetLink {
                name: params.asset.clone(),
                field: "asset".to_string(),
            }],
            ..Dependencies::default()
        }
    }

    fn build(
        params: &TestAssetParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        context.asset("asset", &params.asset)?;
        Ok(output(OutputData::Nothing))
    }
}

/// Params with no fields.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub(crate) struct TestNoParams {}

/// `test_panic`: declares nothing and panics with the message `boom`.
pub(crate) struct TestPanic;

impl Feature for TestPanic {
    const TYPE: &'static str = "test_panic";
    type Params = TestNoParams;

    fn manifest() -> FeatureManifest {
        manifest("Test panic", Vec::new())
    }

    fn dependencies(_params: &TestNoParams) -> Dependencies {
        Dependencies::default()
    }

    fn build(
        _params: &TestNoParams,
        _context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        panic!("boom")
    }
}
