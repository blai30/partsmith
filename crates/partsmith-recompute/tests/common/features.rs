//! Test-only feature types.
//!
//! The first four are the ones recompute.md section 6 defines (`test_panic`, `test_wait`,
//! `test_peek`, `test_stray`). The others are small features that let the runner be exercised
//! without the built-in feature pipelines: datum outputs (`test_datum`), bodies that hold a real
//! kernel solid (`test_block`), body readers (`test_probe`), a node with arbitrary links
//! (`test_node`), an asset reader (`test_asset`), a feature that updates one body twice
//! (`test_twice`) and a reader of a body's writer chain (`test_history`).

use std::sync::Arc;
use std::time::{Duration, Instant};

use partsmith_features::{
    AssetLink, BodyChange, BodyState, BodyUpdate, BuildContext, DatumResult, Dependencies, ExprUse,
    Feature, FeatureError, FeatureOutput, FeatureWarning, Link, Measurements, NoEffectHow,
    OutputData, Registry,
};
use partsmith_model::{
    Category, Expr, FeatureManifest, FieldHint, FieldUnit, Id, OptionHint, SelectionKind,
};
use partsmith_naming::{Motion, NamingMap};
use partsmith_occt_sys::{Plane, make_box};
use partsmith_sketch::Placement;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Names of a box's 6 faces, 12 edges and 8 vertices in kernel index order (naming.md test 33,
/// with the feature id `f-cube`). `test_block` gives every solid it makes this map.
const CUBE_NAMING: &str = r#"{
  "faces": ["f-cube/left", "f-cube/right", "f-cube/front", "f-cube/back", "f-cube/bottom", "f-cube/top"],
  "edges": [
    "edge(f-cube/bottom|f-cube/front)", "edge(f-cube/front|f-cube/top)",
    "edge(f-cube/back|f-cube/bottom)", "edge(f-cube/back|f-cube/top)",
    "edge(f-cube/bottom|f-cube/left)", "edge(f-cube/left|f-cube/top)",
    "edge(f-cube/bottom|f-cube/right)", "edge(f-cube/right|f-cube/top)",
    "edge(f-cube/front|f-cube/left)", "edge(f-cube/back|f-cube/left)",
    "edge(f-cube/front|f-cube/right)", "edge(f-cube/back|f-cube/right)"
  ],
  "vertices": [
    "vertex(f-cube/bottom|f-cube/front|f-cube/left)", "vertex(f-cube/front|f-cube/left|f-cube/top)",
    "vertex(f-cube/back|f-cube/bottom|f-cube/left)", "vertex(f-cube/back|f-cube/left|f-cube/top)",
    "vertex(f-cube/bottom|f-cube/front|f-cube/right)", "vertex(f-cube/front|f-cube/right|f-cube/top)",
    "vertex(f-cube/back|f-cube/bottom|f-cube/right)", "vertex(f-cube/back|f-cube/right|f-cube/top)"
  ]
}"#;

/// Longest time `test_wait` waits for cancellation (recompute.md section 6).
const WAIT_LIMIT: Duration = Duration::from_secs(10);

/// How often `test_wait` checks for cancellation (recompute.md section 6).
const WAIT_POLL: Duration = Duration::from_millis(1);

/// Registers the four test features of recompute.md section 6.
pub fn register_spec_features(registry: &mut Registry) {
    registry
        .register::<TestPanic>()
        .expect("test_panic registers");
    registry
        .register::<TestWait>()
        .expect("test_wait registers");
    registry
        .register::<TestPeek>()
        .expect("test_peek registers");
    registry
        .register::<TestStray>()
        .expect("test_stray registers");
}

/// Registers the runner test features (everything in this module except the spec's four).
pub fn register_runner_features(registry: &mut Registry) {
    registry
        .register::<TestDatum>()
        .expect("test_datum registers");
    registry
        .register::<TestBlock>()
        .expect("test_block registers");
    registry
        .register::<TestProbe>()
        .expect("test_probe registers");
    registry
        .register::<TestNode>()
        .expect("test_node registers");
    registry
        .register::<TestAsset>()
        .expect("test_asset registers");
    registry
        .register::<TestTwice>()
        .expect("test_twice registers");
    registry
        .register::<TestHistory>()
        .expect("test_history registers");
}

/// A test manifest with `label`, category create, version 1 and these field hints.
fn manifest(label: &str, icon: &str, fields: Vec<FieldHint>) -> FeatureManifest {
    FeatureManifest {
        label: label.to_string(),
        category: Category::Create,
        icon: icon.to_string(),
        version: 1,
        fields,
    }
}

/// A field hint.
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

/// The hint of a `body` field.
fn body_hint() -> FieldHint {
    hint("body", "Body", None, Some(SelectionKind::Body))
}

/// Dependencies that declare nothing.
fn no_dependencies() -> Dependencies {
    Dependencies {
        features: Vec::new(),
        bodies_read: Vec::new(),
        bodies_written: Vec::new(),
        expressions: Vec::new(),
        declared_ids: Vec::new(),
        assets: Vec::new(),
    }
}

/// A link to `id` named at `field`.
fn link(id: &Id, field: impl Into<String>) -> Link {
    Link {
        id: id.clone(),
        field: field.into(),
    }
}

/// A length expression use at `field`.
fn length_use(field: impl Into<String>, expr: &Expr) -> ExprUse {
    ExprUse {
        field: field.into(),
        expr: expr.clone(),
        kind: FieldUnit::Length,
    }
}

/// An output with no warnings.
fn output(data: OutputData) -> FeatureOutput {
    FeatureOutput::new(data, Vec::new())
}

/// A datum plane parallel to XY at height `z`.
fn datum_at(z: f64) -> OutputData {
    OutputData::Datum(DatumResult::Plane {
        placement: Placement {
            origin: [0.0, 0.0, z],
            x_axis: [1.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        },
        motion: Motion::IDENTITY,
    })
}

/// The naming map of an empty body.
fn empty_naming() -> Arc<NamingMap> {
    BodyState::empty().naming().clone()
}

/// The naming map `test_block` gives its cubes.
fn cube_naming() -> Arc<NamingMap> {
    Arc::new(serde_json::from_str(CUBE_NAMING).expect("the cube naming map is valid"))
}

/// Params with no fields.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TestNoParams {}

/// Params naming one body.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TestBodyParams {
    /// The body.
    pub body: Id,
}

/// `test_panic`: params `{}`; no dependencies; `build` panics with the message `boom`.
pub struct TestPanic;

impl Feature for TestPanic {
    const TYPE: &'static str = "test_panic";
    type Params = TestNoParams;

    fn manifest() -> FeatureManifest {
        manifest("Test panic", "test-panic", Vec::new())
    }

    fn dependencies(_params: &TestNoParams) -> Dependencies {
        no_dependencies()
    }

    fn build(
        _params: &TestNoParams,
        _context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        panic!("boom")
    }
}

/// `test_wait`: params `{}`; no dependencies; `build` polls `is_canceled` every 1 ms and returns
/// `Canceled` once it is set, or `Nothing` after 10 s.
pub struct TestWait;

impl Feature for TestWait {
    const TYPE: &'static str = "test_wait";
    type Params = TestNoParams;

    fn manifest() -> FeatureManifest {
        manifest("Test wait", "test-wait", Vec::new())
    }

    fn dependencies(_params: &TestNoParams) -> Dependencies {
        no_dependencies()
    }

    fn build(
        _params: &TestNoParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let start = Instant::now();
        while start.elapsed() < WAIT_LIMIT {
            if context.is_canceled() {
                return Err(FeatureError::Canceled);
            }
            std::thread::sleep(WAIT_POLL);
        }
        Ok(output(OutputData::Nothing))
    }
}

/// `test_peek`: params `{"body": Id}`; declares nothing; `build` calls `ctx.body(body)`.
pub struct TestPeek;

impl Feature for TestPeek {
    const TYPE: &'static str = "test_peek";
    type Params = TestBodyParams;

    fn manifest() -> FeatureManifest {
        manifest("Test peek", "test-peek", vec![body_hint()])
    }

    fn dependencies(_params: &TestBodyParams) -> Dependencies {
        no_dependencies()
    }

    fn build(
        params: &TestBodyParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        context.body(&params.body)?;
        Ok(output(OutputData::Nothing))
    }
}

/// `test_stray`: params `{"body": Id}`; declares `bodies_read [body]`; `build` returns a
/// `BodyUpdate` for `body`. The update empties the body, so applying it by mistake would show.
pub struct TestStray;

impl Feature for TestStray {
    const TYPE: &'static str = "test_stray";
    type Params = TestBodyParams;

    fn manifest() -> FeatureManifest {
        manifest("Test stray", "test-stray", vec![body_hint()])
    }

    fn dependencies(params: &TestBodyParams) -> Dependencies {
        Dependencies {
            bodies_read: vec![link(&params.body, "body")],
            ..no_dependencies()
        }
    }

    fn build(
        params: &TestBodyParams,
        _context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        Ok(output(OutputData::Bodies(vec![BodyUpdate {
            body: params.body.clone(),
            shape: None,
            naming: empty_naming(),
            change: BodyChange::Clear,
            measured: Measurements::default(),
        }])))
    }
}

/// Params of `test_datum`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TestDatumParams {
    /// An earlier `test_datum` (or other datum) this one is offset from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Id>,
    /// Offset along +Z. Length.
    pub offset: Expr,
}

/// `test_datum`: a datum plane parallel to XY at the height of its base datum (0 without one)
/// plus `offset`. Declares `features [base]` and the length use `offset`.
pub struct TestDatum;

impl Feature for TestDatum {
    const TYPE: &'static str = "test_datum";
    type Params = TestDatumParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Test datum",
            "test-datum",
            vec![
                hint("base", "Base", None, Some(SelectionKind::Plane)),
                hint("offset", "Offset", Some(FieldUnit::Length), None),
            ],
        )
    }

    fn dependencies(params: &TestDatumParams) -> Dependencies {
        Dependencies {
            features: params.base.iter().map(|base| link(base, "base")).collect(),
            expressions: vec![length_use("offset", &params.offset)],
            ..no_dependencies()
        }
    }

    fn build(
        params: &TestDatumParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let base_height = match &params.base {
            Some(base) => match context.datum("base", base)? {
                DatumResult::Plane { placement, .. } => placement.origin[2],
            },
            None => 0.0,
        };
        let offset = context.eval_length("offset", &params.offset)?;
        Ok(output(datum_at(base_height + offset)))
    }
}

/// How `test_block` combines its cube with the body.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TestBlockOp {
    /// Start the body with a cube of edge `size`; the body must be empty.
    New,
    /// Replace the body by a cube whose volume is the body's volume plus `size` cubed; the body
    /// must hold a solid.
    Join,
}

/// Params of `test_block`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TestBlockParams {
    /// Target body.
    pub body: Id,
    /// Edge of the added cube. Length.
    pub size: Expr,
    /// How the cube combines with the body.
    pub op: TestBlockOp,
}

/// `test_block`: writes `body` with a real kernel cube, so body states have volumes.
///
/// Op `new` makes a cube of edge `size` (error `BodyNotEmpty` when the body holds a solid). Op
/// `join` replaces the body with a cube whose volume is the old volume plus `size` cubed (error
/// `BodyEmpty` when the body is empty). Declares `bodies_written [body]` and the length use `size`.
pub struct TestBlock;

impl Feature for TestBlock {
    const TYPE: &'static str = "test_block";
    type Params = TestBlockParams;

    fn manifest() -> FeatureManifest {
        let mut operation = hint("op", "Operation", None, None);
        operation.options = vec![
            OptionHint {
                value: "new".to_string(),
                label: "New body".to_string(),
            },
            OptionHint {
                value: "join".to_string(),
                label: "Join".to_string(),
            },
        ];
        manifest(
            "Test block",
            "test-block",
            vec![
                hint("size", "Size", Some(FieldUnit::Length), None),
                operation,
                body_hint(),
            ],
        )
    }

    fn dependencies(params: &TestBlockParams) -> Dependencies {
        Dependencies {
            bodies_written: vec![link(&params.body, "body")],
            expressions: vec![length_use("size", &params.size)],
            ..no_dependencies()
        }
    }

    fn build(
        params: &TestBlockParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let size = context.eval_length("size", &params.size)?;
        let before = context.body(&params.body)?;
        let (volume, change) = match params.op {
            TestBlockOp::New => {
                if !before.is_empty() {
                    return Err(FeatureError::BodyNotEmpty {
                        body: params.body.clone(),
                        target: None,
                    });
                }
                (size.powi(3), BodyChange::New)
            }
            TestBlockOp::Join => {
                let Some(shape) = before.shape() else {
                    return Err(FeatureError::BodyEmpty {
                        field: context.field("body"),
                        body: params.body.clone(),
                        use_new: true,
                    });
                };
                let old_volume = shape
                    .volume()
                    .map_err(|error| FeatureError::Kernel { error })?;
                (old_volume + size.powi(3), BodyChange::Join)
            }
        };
        let edge = volume.cbrt();
        let cube = make_box(&Plane::XY, edge, edge, edge)
            .map_err(|error| FeatureError::Kernel { error })?;
        Ok(output(OutputData::Bodies(vec![BodyUpdate {
            body: params.body.clone(),
            shape: Some(cube.shape),
            naming: cube_naming(),
            change,
            measured: Measurements::default(),
        }])))
    }
}

/// `test_probe`: reads `body`. Fails with `BodyEmpty` when the body is empty, else outputs
/// nothing with a `NoEffect` warning (`Inside`) for the body. Declares `bodies_read [body]`.
pub struct TestProbe;

impl Feature for TestProbe {
    const TYPE: &'static str = "test_probe";
    type Params = TestBodyParams;

    fn manifest() -> FeatureManifest {
        manifest("Test probe", "test-probe", vec![body_hint()])
    }

    fn dependencies(params: &TestBodyParams) -> Dependencies {
        Dependencies {
            bodies_read: vec![link(&params.body, "body")],
            ..no_dependencies()
        }
    }

    fn build(
        params: &TestBodyParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        if context.body(&params.body)?.is_empty() {
            return Err(FeatureError::BodyEmpty {
                field: context.field("body"),
                body: params.body.clone(),
                use_new: false,
            });
        }
        Ok(FeatureOutput::new(
            OutputData::Nothing,
            vec![FeatureWarning::NoEffect {
                body: params.body.clone(),
                how: NoEffectHow::Inside,
                reverse_hits: false,
                attach: None,
            }],
        ))
    }
}

/// Params of `test_node`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TestNodeParams {
    /// Datum features whose output is read, at `uses[i]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uses: Vec<Id>,
    /// Bodies read, at `reads[i]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<Id>,
    /// Bodies written, at `writes[i]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writes: Vec<Id>,
    /// Length expressions, at `values[i]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<Expr>,
}

/// `test_node`: declares exactly the links of its params, in params order.
///
/// `build` reads every declared datum and body and evaluates every value. Without written bodies
/// it outputs a datum plane at the height of the sum of its values; otherwise it rewrites every
/// written body with its current state (change `Modify`).
pub struct TestNode;

impl Feature for TestNode {
    const TYPE: &'static str = "test_node";
    type Params = TestNodeParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Test node",
            "test-node",
            vec![
                hint("uses", "Uses", None, Some(SelectionKind::Plane)),
                hint("reads", "Reads", None, Some(SelectionKind::Body)),
                hint("writes", "Writes", None, Some(SelectionKind::Body)),
                hint("values[*]", "Value", Some(FieldUnit::Length), None),
            ],
        )
    }

    fn dependencies(params: &TestNodeParams) -> Dependencies {
        let links = |ids: &[Id], name: &str| -> Vec<Link> {
            ids.iter()
                .enumerate()
                .map(|(i, id)| link(id, format!("{name}[{i}]")))
                .collect()
        };
        Dependencies {
            features: links(&params.uses, "uses"),
            bodies_read: links(&params.reads, "reads"),
            bodies_written: links(&params.writes, "writes"),
            expressions: params
                .values
                .iter()
                .enumerate()
                .map(|(i, value)| length_use(format!("values[{i}]"), value))
                .collect(),
            ..no_dependencies()
        }
    }

    fn build(
        params: &TestNodeParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        for (i, feature) in params.uses.iter().enumerate() {
            context.datum(&format!("uses[{i}]"), feature)?;
        }
        for body in &params.reads {
            context.body(body)?;
        }
        let mut total = 0.0;
        for (i, value) in params.values.iter().enumerate() {
            total += context.eval_length(&format!("values[{i}]"), value)?;
        }
        if params.writes.is_empty() {
            return Ok(output(datum_at(total)));
        }
        let mut updates = Vec::new();
        for body in &params.writes {
            let state = context.body(body)?;
            updates.push(BodyUpdate {
                body: body.clone(),
                shape: state.shape().cloned(),
                naming: state.naming().clone(),
                change: BodyChange::Modify,
                measured: state.measurements(),
            });
        }
        Ok(output(OutputData::Bodies(updates)))
    }
}

/// Params of `test_asset`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TestAssetParams {
    /// The asset name.
    pub asset: String,
}

/// `test_asset`: declares the asset link `asset` and outputs a datum plane at the height of the
/// asset's length in bytes.
pub struct TestAsset;

impl Feature for TestAsset {
    const TYPE: &'static str = "test_asset";
    type Params = TestAssetParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Test asset",
            "test-asset",
            vec![hint("asset", "File", None, None)],
        )
    }

    fn dependencies(params: &TestAssetParams) -> Dependencies {
        Dependencies {
            assets: vec![AssetLink {
                name: params.asset.clone(),
                field: "asset".to_string(),
            }],
            ..no_dependencies()
        }
    }

    fn build(
        params: &TestAssetParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let bytes = context.asset("asset", &params.asset)?;
        let length = u32::try_from(bytes.len()).expect("test assets are small");
        Ok(output(datum_at(f64::from(length))))
    }
}

/// `test_twice`: declares `bodies_written [body]` and returns two updates for it, both emptying
/// it, so applying either by mistake would show.
pub struct TestTwice;

impl Feature for TestTwice {
    const TYPE: &'static str = "test_twice";
    type Params = TestBodyParams;

    fn manifest() -> FeatureManifest {
        manifest("Test twice", "test-twice", vec![body_hint()])
    }

    fn dependencies(params: &TestBodyParams) -> Dependencies {
        Dependencies {
            bodies_written: vec![link(&params.body, "body")],
            ..no_dependencies()
        }
    }

    fn build(
        params: &TestBodyParams,
        _context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let update = BodyUpdate {
            body: params.body.clone(),
            shape: None,
            naming: empty_naming(),
            change: BodyChange::Clear,
            measured: Measurements::default(),
        };
        Ok(output(OutputData::Bodies(vec![update.clone(), update])))
    }
}

/// Params of `test_history`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TestHistoryParams {
    /// The body whose writer chain is read.
    pub body: Id,
    /// The writers the chain must name, in timeline order.
    pub writers: Vec<Id>,
}

/// `test_history`: declares `bodies_read [body]` and reads the body's writer chain
/// (`BuildContext::body_history`, recompute.md section 3.5).
///
/// When the chain names exactly `writers`, in order, it outputs a datum plane at the height of the
/// sum of the body's volumes just before each writer (an empty body counts 0), so the states of
/// the chain show in the output; otherwise it fails with an `Internal` error that names the
/// chain's writers.
pub struct TestHistory;

impl Feature for TestHistory {
    const TYPE: &'static str = "test_history";
    type Params = TestHistoryParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Test history",
            "test-history",
            vec![body_hint(), hint("writers", "Writers", None, None)],
        )
    }

    fn dependencies(params: &TestHistoryParams) -> Dependencies {
        Dependencies {
            bodies_read: vec![link(&params.body, "body")],
            ..no_dependencies()
        }
    }

    fn build(
        params: &TestHistoryParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let chain = context.body_history(&params.body)?;
        let writers: Vec<&Id> = chain.iter().map(|(writer, _)| writer).collect();
        if writers.iter().copied().ne(params.writers.iter()) {
            return Err(FeatureError::internal(format!(
                "the writer chain of {} is {writers:?}",
                params.body
            )));
        }
        let mut height = 0.0;
        for (_, state) in chain {
            if let Some(shape) = state.shape() {
                height += shape
                    .volume()
                    .map_err(|error| FeatureError::Kernel { error })?;
            }
        }
        Ok(output(datum_at(height)))
    }
}
