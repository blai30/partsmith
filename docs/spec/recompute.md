# Recompute: dependency graph, dirty tracking and result cache

Status: M1 spec. Crate: `partsmith-recompute`. Read with [architecture.md](../design/architecture.md) (section "Recompute"), technical design section 14, [features.md](features.md) (the `Feature` contract this crate drives), [model.md](model.md) and [expressions.md](expressions.md) (documents and parameter evaluation), and [engine.md](engine.md) (the only production caller).

## 1. Purpose and scope

`partsmith-recompute` runs a document's timeline: it builds the dependency graph, evaluates parameters and feature expressions, computes a content hash for every feature, reuses cached results for unchanged hashes, builds the rest through the feature registry, isolates failures, and reports per-feature status, timing, outputs and body states. It is pure Rust over `partsmith-model` and `partsmith-features`, holds no global state, and is deterministic.

### In M1

- Dependency DAG over the timeline: feature outputs, body state chains, parameters and context values.
- Dirty tracking through content-hash cache keys (BLAKE3 of canonical params, evaluated values, upstream keys and the versions of `partsmith-features` and OCCT), plus graph queries for "what does this change affect".
- An in-memory result cache, shared by all documents of a session, with LRU eviction by entry count.
- Rollback marker (the first rolled-back feature) and an explicit stop point for previews.
- Asset bytes supplied by the host for features that read imported files.
- Suppression, failure isolation, panic isolation.
- Per-feature status, warnings and errors with rendered messages and repair data, timing, outputs, and body states before each feature.
- A cancellation token checked between features and exposed to long builds.
- A plan (which features would build and which would come from the cache) without building.
- Sequential execution in timeline order.

### Later

- Parallel execution of independent branches on desktop (the graph already exposes independence; results must not change).
- A persistent `.ptsm-cache/` disk cache of B-rep and naming maps keyed by the same hashes.
- Fast proxy previews of expensive features during drags.
- Cancellation inside a kernel call (OCCT progress indicators); M1 cancels between features and between steps of features that check the token.
- Eviction by memory size instead of entry count.
- Cross-document links (components) and script features.

## 2. Public API

```rust
/// Default maximum number of cached results.
pub const DEFAULT_CACHE_ENTRIES: usize = 4096;

/// Runs timelines with a result cache. One per session, shared by its documents.
pub struct Recompute { /* registry, cache, run counter */ }

impl Recompute {
    /// A recompute with the default cache size.
    pub fn new(registry: Arc<Registry>) -> Self;
    /// A recompute whose cache keeps at most `max_entries` results after each run. Results the
    /// latest run used are never evicted, so a run can leave more (section 3.7).
    pub fn with_cache_limit(registry: Arc<Registry>, max_entries: usize) -> Self;
    /// The feature registry.
    pub fn registry(&self) -> &Arc<Registry>;
    /// The dependency graph of a document (no evaluation, no building).
    pub fn graph(&self, input: &Input<'_>) -> Graph;
    /// What a run would build or reuse, with every key (evaluates, never builds).
    pub fn plan(&self, input: &Input<'_>) -> Plan;
    /// Runs the timeline up to the stop point.
    pub fn run(&mut self, input: &Input<'_>, cancel: &CancelToken) -> Report;
    /// Number of cached results.
    pub fn cache_len(&self) -> usize;
    /// Drops every cached result.
    pub fn clear_cache(&mut self);
}

/// What to recompute.
pub struct Input<'a> {
    /// The document (structurally valid, as `partsmith_model` loads and edits keep it).
    pub document: &'a Document,
    /// Resolves dotted names such as `printer.fit.slip` (expressions.md `EvaluationContext`).
    pub context: &'a dyn EvaluationContext,
    /// Stop after this feature (inclusive) instead of at the document's rollback marker. Used for previews.
    pub stop_at: Option<&'a Id>,
    /// Asset bytes by name (the document's asset store).
    pub assets: &'a BTreeMap<String, Arc<[u8]>>,
}

/// The dependency graph of one document.
pub struct Graph { /* private */ }

impl Graph {
    /// One node per feature record, in timeline order.
    pub fn nodes(&self) -> &[Node];
    /// The node of a feature.
    pub fn node(&self, feature: &Id) -> Option<&Node>;
    /// Features with a direct edge from `feature`, in timeline order.
    pub fn dependents(&self, feature: &Id) -> Vec<Id>;
    /// Every active feature that a change can affect (directly or transitively), in timeline order.
    pub fn affected_by(&self, changes: &[Change]) -> Vec<Id>;
    /// True when neither feature can reach the other through edges.
    pub fn independent(&self, a: &Id, b: &Id) -> bool;
}

/// One feature in the graph.
pub struct Node {
    /// The feature.
    pub feature: Id,
    /// Whether it runs.
    pub state: NodeState,
    /// Direct upstream edges, features first in declaration order, then bodies (read, then written) in declaration order.
    pub upstream: Vec<Edge>,
    /// Parameters its expressions use, directly or through other parameters, in document order.
    pub parameters: Vec<Id>,
    /// Dotted names its expressions use (directly or through parameters), sorted.
    pub context_names: Vec<String>,
    /// True when it evaluates at least one expression.
    pub uses_expressions: bool,
}

/// Whether a node runs.
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
pub struct Edge {
    /// The upstream feature.
    pub feature: Id,
    /// Why the node depends on it.
    pub via: EdgeKind,
}

/// Why a node depends on an upstream feature.
pub enum EdgeKind {
    /// The node reads the upstream feature's output (a sketch or datum).
    Output,
    /// The upstream feature is the last active writer of this body before the node.
    Body(Id),
}

/// A change to a document, for `Graph::affected_by`.
pub enum Change {
    /// The feature's own record changed (params, suppression).
    Feature(Id),
    /// A parameter's expression changed.
    Parameter(Id),
    /// A context value changed, for example "printer.fit.slip".
    ContextName(String),
    /// The document units changed.
    Units,
}

/// What a run would do.
pub struct Plan {
    /// One step per feature record, in timeline order.
    pub steps: Vec<PlanStep>,
}

/// What a run would do with one feature.
pub struct PlanStep {
    /// The feature.
    pub feature: Id,
    /// The predicted action.
    pub action: Action,
    /// Its cache key, when it has one.
    pub key: Option<CacheKey>,
}

/// Predicted action for a feature.
pub enum Action {
    /// No cached result for its key: it will be built.
    Build,
    /// Its key is cached: the result will be reused.
    Reuse,
    /// It fails before building (invalid, unknown references, unavailable upstream).
    Fail,
    /// Suppressed.
    Suppressed,
    /// At or after the rollback marker, or after the stop point.
    RolledBack,
}

/// A 32-byte BLAKE3 hash. `Display` writes 64 lowercase hex characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CacheKey(pub [u8; 32]);

/// The result of a run.
pub struct Report {
    /// One report per feature record, in timeline order.
    pub features: Vec<FeatureReport>,
    /// One report per body in `Document::bodies` order: the state at the end of the run (after the stop point, or before the rollback marker).
    pub bodies: Vec<BodyReport>,
    /// One report per parameter in document order.
    pub parameters: Vec<ParameterReport>,
    /// Document-level messages (for example an unknown stop point).
    pub messages: Vec<Message>,
    /// True when the run was canceled; `bodies` are then the states after the last feature that finished.
    pub canceled: bool,
    /// Wall time of the whole run.
    pub elapsed: Duration,
}

/// The status of one feature after a run.
pub struct FeatureReport {
    /// The feature.
    pub feature: Id,
    /// Overall status.
    pub status: Status,
    /// The error, when status is `Error`.
    pub error: Option<FeatureError>,
    /// Warnings, from the output first, then those recompute adds (`UnknownParams`).
    pub warnings: Vec<FeatureWarning>,
    /// The error and warnings rendered with current names and units: error first, then warnings in order.
    pub messages: Vec<Message>,
    /// Its cache key, when it has one.
    pub key: Option<CacheKey>,
    /// How long it took and whether it came from the cache.
    pub timing: Timing,
    /// Its output, when status is `Ok` or `Warning`.
    pub output: Option<Arc<FeatureOutput>>,
    /// States of the bodies it declared (read, then written, in declaration order) just before it. Empty unless it ran or was reused. Undeclared bodies are unchanged by the feature, so every body's state just before the stop feature is `bodies_before` for its declared bodies and `Report::bodies` for the rest (what the engine's focus preview needs).
    pub bodies_before: Vec<(Id, BodyState)>,
}

/// Status of one feature.
pub enum Status {
    /// Built or reused with no warnings.
    Ok,
    /// Built or reused with warnings.
    Warning,
    /// Failed; its bodies are unchanged.
    Error,
    /// Suppressed by the user.
    Suppressed,
    /// At or after the rollback marker, or after the stop point.
    RolledBack,
    /// Not run because the run was canceled.
    NotRun,
}

/// Timing of one feature.
pub struct Timing {
    /// Build time; zero when the result came from the cache or the feature did not run.
    pub duration: Duration,
    /// True when the result came from the cache.
    pub from_cache: bool,
}

/// The state of one body at the end of the run.
pub struct BodyReport {
    /// The body.
    pub body: Id,
    /// Its state (shape, naming map, element infos on demand).
    pub state: BodyState,
    /// The key of the feature that last wrote it (its provider), or None when no feature wrote it.
    pub key: Option<CacheKey>,
    /// That feature.
    pub last_feature: Option<Id>,
}

/// The value of one parameter.
pub struct ParameterReport {
    /// The parameter.
    pub parameter: Id,
    /// Its value in internal units, or the evaluation error.
    pub value: Result<Quantity, ExprError>,
}

/// A rendered message.
pub struct Message {
    /// Warning or error.
    pub severity: Severity,
    /// Stable snake_case code, for example "reference_missing".
    pub code: &'static str,
    /// JSON Pointer of the field it is about, relative to the feature record, for example "/edges/1".
    pub pointer: Option<String>,
    /// The message text, rendered with current names and units.
    pub text: String,
    /// Replacement expression for the field at `pointer`, in display units, when the feature computed a limit (features.md section 4.4).
    pub suggestion: Option<String>,
    /// Repair data for lost references and regions.
    pub repair: Option<Repair>,
}

/// Severity of a message.
pub enum Severity { Warning, Error }
```

`CancelToken`, `FeatureOutput`, `BodyState`, `FeatureError`, `FeatureWarning` and `Repair` come from `partsmith-features` and are re-exported. `Report` and its parts derive `Debug` and `Clone`; none is serialized (the engine maps them to protocol types). `Recompute` is `Send`.

## 3. Behavior

### 3.1 Preparing a run

For every feature record, in timeline order:

1. **Validation.** `Registry::validate_and_canonicalize`. Failure (unknown type, invalid params, a sketch that fails its own validation) makes the node `Invalid` with that error. Results are memoized per record content within the `Recompute`, so unchanged records are not reparsed.
2. **Dependencies.** `Registry::dependencies` of the canonical record.
3. **State.** Active candidates are the records up to and including `input.stop_at` when it is set and names a record; otherwise the records before `document.rollback` (the first rolled-back record) when it is set and names a record; otherwise all records. Every other record is `RolledBack`: with a stop point, the records after it; with the rollback marker, the marker record and every later one. A `stop_at` that names no record is ignored with a document-level warning `unknown_stop`, and the rollback rule applies as if it were absent; a `rollback` that names no record is ignored the same way (the model rejects such documents at load, so this only guards in-memory edits). When nothing valid is left the whole timeline runs and the warning reads `The preview or rollback point '{id}' is not in this document, so the whole timeline was rebuilt.`; when an unknown `stop_at` falls back to a valid rollback marker it reads `The preview point '{id}' is not in this document, so the timeline was rebuilt up to the rollback marker.` Among the candidates, suppressed records are `Suppressed`, invalid ones `Invalid`, the rest `Active`. Rollback is checked first: a suppressed record that is not a candidate is `RolledBack`.
4. **Parameters.** `Document::evaluate_parameters(context)` (expressions.md section 3.13) gives `ParameterValues`; the report's `parameters` list them.

### 3.2 The graph

Edges go only from earlier to later records, so the graph is acyclic by construction.

- **Output edges.** For each `Link` in a node's `features`, an `Output` edge from that feature, if it exists and comes earlier in the timeline.
- **Body edges.** For each `Link` in `bodies_read` and `bodies_written`, a `Body(b)` edge from the body's **provider**: the last `Active` record before the node whose `bodies_written` contains `b`. `Suppressed`, `RolledBack` and `Invalid` records are never providers. A body with no provider has no edge (it is empty at that point).
- **Assets** create no edges: an asset name is a content hash stored in the params, so it is part of the cache key (section 3.4).
- **Parameters.** A node's `parameters` are the parameters named in its expressions (`Expr::references().parameters` resolved by name), plus every parameter those reference, transitively, in document order. Its `context_names` are the dotted names (`references().values`) of its expressions and of those parameters.
- `affected_by(changes)`: the seed set is each `Feature(id)` itself; each active node whose `parameters` contain a changed `Parameter`; each active node whose `context_names` contain a changed `ContextName`; for `Units`, every active node with `uses_expressions`. The result is the seeds that are active plus every active node reachable from them along edges, in timeline order. Query the graph in which the changed feature is active: the graph after the edit for adding or unsuppressing a feature, the graph before it for removing or suppressing one, and the union of both results when an edit changes what a feature reads or writes.
- `independent(a, b)` is true when neither node is reachable from the other.

### 3.3 Pre-build checks

Before an `Active` node is built, recompute checks its links in declaration order and fails the node without building when one does not hold (these errors are not cached; they are cheap):

| Check | Error |
| --- | --- |
| A feature link names no record | `UnknownFeature { field }` |
| A feature link names the node itself or a later record | `ForwardReference { field }` |
| A feature link's record is `Suppressed` | `UpstreamSuppressed { feature }` |
| A feature link's record is `Invalid` or ended with `Error` | `UpstreamFailed { feature }` |
| A body link names no body of the document | `UnknownBody { field }` |
| An asset link names no entry of `input.assets` | `AssetMissing { field, asset }` |

### 3.4 Cache keys

Every `Active` node gets a key; `Suppressed`, `RolledBack` and `Invalid` nodes have none. The key is the BLAKE3 hash of this byte sequence, each item written as UTF-8 followed by one line feed (no item contains a line feed):

1. `partsmith-recompute 1`
2. The engine signature: `partsmith-features {partsmith_features::VERSION} occt {partsmith_features::OCCT_VERSION}`, for example `partsmith-features 0.1.0 occt 7.9.3`.
3. `type <type name>` and `version <manifest version>`.
4. `id <feature id>`.
5. `params <canonical params JSON>`: the known keys of the canonical params (unknown keys excluded), written compactly (no spaces) with keys in canonical order and numbers in canonical form.
6. For each use of `Values::iter` (declaration order): `expr <field> <unit>` (the `FieldUnit` in snake_case, for example `length`) and then `value <hex of f64::to_bits>` for a converted value, or `error <code> <Display text>` for an error.
7. For each feature link in order: `feature <id> <upstream key hex>`, or `feature <id> none` when the upstream has no key.
8. For each body link (read, then written) in order: `body <id> <provider key hex>`, or `body <id> empty` when it has no provider.

Consequences:

- An edit marks dirty exactly the edited feature and its transitive dependents, because a dependent's key contains its upstream keys.
- A parameter edit that leaves a value unchanged (`2 mm` to `1 mm + 1 mm`) rebuilds nothing; a units change rebuilds only features whose converted values changed.
- Names (`Body::name`, `FeatureRecord::name`), body colors and materials are not in keys; messages render names at report time.
- Key order in the stored params does not matter (canonicalization comes first); unknown keys do not affect the key.
- Values from the evaluation context (printer values) are in the keys through evaluated values.
- Asset bytes are in the keys through the asset name in the params, which is their BLAKE3 hash.
- A different `partsmith-features` or OCCT version gives different keys, so a later disk cache never reuses results across upgrades.

### 3.5 Running

Features run sequentially in timeline order. A running state maps each body to its current `BodyState` (all start empty) and its provider key. For each record:

1. `RolledBack`, `Suppressed`: status as named; nothing else. `Invalid`: status `Error` with the validation error.
2. If `cancel.is_canceled()`: this and every later `Active` record get `NotRun`; the run stops and `canceled` is true.
3. Evaluate the declared expressions (`Values::evaluate`) and compute the key (section 3.4).
4. Pre-build checks (section 3.3). On failure: status `Error`, no output, bodies unchanged; the key is kept (this feature still provides the bodies it declares).
5. Record `bodies_before` from the running state.
6. **Cache.** If the key is cached, reuse the result (`from_cache: true`, duration zero).
7. **Build.** Otherwise build through `Registry::build` with a `BuildContext` holding only the declared bodies' states, the declared features' outputs, the declared assets' bytes and the run's `CancelToken`, inside `catch_unwind`. A panic becomes `Internal { message }` with the panic message (or `unknown panic`). The duration is measured around the build.
8. **Check the output.** A `BodyUpdate` for a body not in `bodies_written`, or two updates for one body, turns the result into `UndeclaredDependency`; nothing from that output is applied.
9. **Store.** The result (output or error) is cached under the key, except `Canceled` and `Internal`. A build that returns `Canceled` gives status `NotRun` and stops the run as in step 2.
10. **Apply.** On success, apply each `BodyUpdate` to the running state: the body's state becomes `BodyState::from_update` (its shape and naming map, with the topology and volume the feature measured, features.md section 2.7, so later features and hosts reuse them) and its provider key this key. On error, bodies are unchanged; the feature is still the provider of the bodies it declares in `bodies_written`, so later keys stay a function of the timeline.
11. **Status.** `Ok` without warnings, `Warning` with warnings (including `UnknownParams` for unknown keys, added here and not cached), `Error` with an error.

After the loop, `bodies` are the running states (the states at the stop point or before the rollback marker, or after a cancel the states after the last finished feature).

Failure isolation: a failing feature never stops the run. Features that read a failed feature's bodies build on the states before it; features that read its output report `UpstreamFailed`.

### 3.6 Messages

`FeatureReport::messages` are rendered at report time, also for cached results, with `DocumentNames { document, registry }` and the document's units: the error first (severity error), then warnings (severity warning). Each message has the error's or warning's `code()`, the JSON Pointer of its field (`FieldRef::pointer`), the rendered text, `suggestion(units)`, and `repair()`.

### 3.7 The cache

- Entries map a `CacheKey` to the cached result: the output (`Arc<FeatureOutput>`) or the error. Body states derived from a cached output (`BodyState::from_update`) are cached with it, so the topology, element infos and volume computed once (`BodyState::topology`, `elements`, `volume`) are shared by later runs.
- One `Recompute` serves every document of a session; identical features in different documents share entries (their keys are equal only when their ids, params, values and upstream keys are).
- Each run records which entries it used. After a run, while there are more entries than the limit, the entry whose last use is oldest (ties: smaller key first) and that the latest run did not use is evicted. Entries the latest run used are never evicted, so a run can exceed the limit.
- `plan` and `graph` never modify the cache.

### 3.8 Plan

`plan(input)` performs sections 3.1 to 3.4 without building, using cached errors to predict failures: `Fail` for invalid nodes, nodes that fail pre-build checks, and nodes whose feature links point at records whose cached result is an error; `Reuse` when the key is cached; `Build` otherwise. Since a node marked `Build` may fail when built, a later `Reuse` or `Build` prediction can turn into `UpstreamFailed` at run time; `plan` and `run` agree on every key.

### 3.9 Determinism

The same `Recompute` inputs give the same report except `Timing`, `elapsed` and `from_cache`: equal statuses, keys, messages, outputs (equal names and kernel results) and body states, in every session and on every platform built from the same sources. Iteration never depends on hash-map order. Parallel execution, when added, must keep this.

## 4. Errors

Recompute has no error type of its own: every problem is a feature's status (`FeatureError` and `FeatureWarning`, features.md section 4) or a document-level `Message`. Recompute raises these `FeatureError` variants itself: `UnknownFeature`, `ForwardReference`, `UpstreamFailed`, `UpstreamSuppressed`, `UnknownBody`, `AssetMissing` (section 3.3), `UndeclaredDependency` (section 3.5 step 8), `Internal` (panics), and it adds the `UnknownParams` warning. Document-level messages:

| Code | Severity | Text |
| --- | --- | --- |
| `unknown_stop` | warning | `The preview or rollback point '{id}' is not in this document, so the whole timeline was rebuilt.`, or `The preview point '{id}' is not in this document, so the timeline was rebuilt up to the rollback marker.` when a valid rollback marker remains (section 3.1) |

## 5. Performance budgets

Release builds on the reference machine (technical design section 14), single thread; web budgets are 3 times these. Measured by benchmarks in `crates/partsmith-recompute/benches/` (`harness = false`, `std::time::Instant`), on the one 50-feature benchmark document `examples/bench/fifty-features/model.ptsm` (engine.md section 5; `bench50` below), made of: 2 datum planes, 10 sketches (rectangles with circles), 10 extrudes (new, join and cut), 8 fillets of 4 edges, 6 chamfers of 4 edges, 4 hole features of 4 holes, 2 shells, 4 linear patterns of 3, 2 mirrors and 2 booleans, over 4 bodies, with a parameter `base` that drives the first sketch.

| Operation | Budget (desktop) |
| --- | --- |
| `plan` of a 300-feature document (validation, dependencies, parameters, keys) | 10 ms |
| Overhead per feature, excluding build and element infos | 0.1 ms |
| `run` of `bench50` with every result cached | 5 ms |
| `run` of `bench50` from an empty cache | 1 s |
| `run` after a parameter edit that affects the last 5 features of `bench50` | 60 ms |
| Latency from `cancel()` to the end of `run`, between features | the remaining time of the feature being built |

## 6. Acceptance tests

Tests live in `crates/partsmith-recompute/tests/` and use only public APIs. Conventions from features.md section 6.1 (documents, fixtures, tolerances); tests pass an empty asset map unless stated. Test-only features are registered through `Registry::register` on `Registry::with_builtins()`:

- `test_panic`: params `{}`; no dependencies; `build` panics with the message `boom`.
- `test_wait`: params `{}`; no dependencies; `build` polls `ctx.is_canceled()` every 1 ms and returns `Err(Canceled)` once it is set (or `Ok(Nothing)` after 10 s).
- `test_peek`: params `{"body": Id}`; declares nothing; `build` calls `ctx.body(body)`.
- `test_stray`: params `{"body": Id}`; declares `bodies_read [body]`; `build` returns a `BodyUpdate` for `body`.

Document `DOC-A` (bodies `b-1` named Body 1, `b-2` named Body 2; parameters `{"id": "p-h", "name": "h", "expr": "10 mm"}` and `{"id": "p-w", "name": "w", "expr": "5 mm"}`), features in order:

1. `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": <RECT(0, 0, 80, 50) lines e-1..e-4>}`
2. `{"id": "f-ext1", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]}, "extent": {"kind": "distance", "expr": "h"}, "op": "new"}`
3. `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["10", "10", "10"], "position": ["100", "0", "0"], "op": "new"}`
4. `{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-1)", "fp": "line|n=1,0,0|c=40,0,10|l=80"}], "radius": "1"}`
5. `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "w"}`

**Graph**

1. `graph(DOC-A)`: `f-ext1.upstream == [Output f-sk1]`; `f-fil1.upstream == [Body(b-1) f-ext1]`; `f-box2.upstream` and `f-dp1.upstream` are empty; `f-ext1.parameters == [p-h]`; `f-dp1.parameters == [p-w]`; `dependents(f-sk1) == [f-ext1]`; `dependents(f-ext1) == [f-fil1]`; `independent(f-box2, f-fil1)` is true; `independent(f-sk1, f-fil1)` is false.
2. `affected_by([Parameter(p-h)]) == [f-ext1, f-fil1]`; `affected_by([Feature(f-sk1)]) == [f-sk1, f-ext1, f-fil1]`; `affected_by([Parameter(p-w)]) == [f-dp1]`; `affected_by([Units]) == [f-ext1, f-box2, f-fil1, f-dp1]` (f-sk1 has no expressions).
3. Body chain: features `f-box1` (b-1, new), `f-box2` (b-1, size 5,5,5, position [20,0,0], join), `f-box3` (b-1, size 2,2,2, cut), then a fillet `f-fil1` on b-1. `f-fil1.upstream == [Body(b-1) f-box3]` and `f-box3.upstream == [Body(b-1) f-box2]`. With `f-box3` suppressed, `f-fil1.upstream == [Body(b-1) f-box2]` and f-box3's state is `Suppressed`.
4. Parameter chain: parameters `a = 2 mm` (`p-a`) and `b = a * 2` (`p-b`); a box with size `["b", "1", "1"]` has `parameters == [p-a, p-b]`; `affected_by([Parameter(p-a)])` contains it.
5. Context names: a box with size `["printer.fit.slip * 10", "1", "1"]` has `context_names == ["printer.fit.slip"]`.

**Runs and the cache**

6. First run of `DOC-A`: every status `Ok`; every `from_cache` false; every active feature has a key; `b-1` volume 40000 - (1 - π/4) × 80 = 39982.831853 with 7 faces; `b-2` volume 1000; `parameters` p-h = 10 mm, p-w = 5 mm; f-dp1's output placement origin (0,0,5).
7. A second run of `DOC-A` on the same `Recompute`: `plan` beforehand reports `Reuse` for every active feature; every `from_cache` true with zero duration; keys equal to test 6.
8. Edit `f-ext1`'s extent to `"12"`: `plan` reports `Build` for exactly f-ext1 and f-fil1 and `Reuse` for f-sk1, f-box2, f-dp1; the run has `from_cache` false for exactly those two.
9. Edit `p-h` to `"5 mm + 5 mm"`: `plan` reports `Reuse` for every active feature.
10. Edit `p-h` to `"12 mm"`: `plan` reports `Build` for exactly f-ext1 and f-fil1.
11. Rename body `b-1` to `Bracket` and give f-ext1 the name `Base`: every key unchanged, every `from_cache` true.
12. Undo: run `DOC-A`, run test 10's document, run `DOC-A` again: the third run has every `from_cache` true.
13. Shared cache: run `DOC-A`, then run a second `Document` value parsed from the same text on the same `Recompute`: every `from_cache` true.
14. Key independence: a copy of `DOC-A` whose f-ext1 params are written in the key order `op`, `extent`, `profile`, `body` has the same keys.
15. Unknown keys: `DOC-A` with `"taper": "2 deg"` added to f-ext1: f-ext1 status `Warning` (`unknown_params`, text `This feature has settings this version of Partsmith does not know ('taper'). They are ignored and kept in the file.`); every key equal to test 6; changing `taper` to `"3 deg"` keeps every key.
16. Messages re-render: `DOC-A` plus `{"id": "f-box9", "type": "box", "body": "b-2", "size": ["1", "1", "1"], "position": ["500", "0", "0"], "op": "cut"}`: f-box9 status `Warning` with the message `This feature does not change body 'Body 2'. Check its position, size and direction.`; after renaming b-2 to `Lid`, the next run has `from_cache` true for f-box9 and the message is `This feature does not change body 'Lid'. Check its position, size and direction.`

**Stop point, suppression, failures**

17. Rollback: `DOC-A` with `"rollback": "f-box2"` (f-box2 is the first rolled-back feature): f-box2, f-fil1 and f-dp1 are `RolledBack` with no key; `bodies` has b-1 at volume 40000 with 6 faces and `last_feature == f-ext1`, and b-2 empty. Clearing the rollback after test 6 on the same `Recompute` gives `from_cache` true for every feature.
18. Stop point: `DOC-A` with `stop_at = f-fil1`: f-dp1 is `RolledBack`; f-fil1's `bodies_before` holds b-1 with 6 faces; its output holds b-1 with 7 faces; `bodies` b-1 has 7 faces. `stop_at` overrides a document rollback of `f-sk1`.
19. Unknown stop: `stop_at = f-nope`: every feature runs; `messages` holds `unknown_stop`.
20. Failure isolation: bodies b-1; `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "2.4"], "op": "new"}`; f-fil1 filleting the four top edges with radius 3; `{"id": "f-cha1", "type": "chamfer", "body": "b-1", "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,2.4|l=20"}], "distance": {"kind": "equal", "expr": "1"}}`. Expected: f-fil1 `Error` (`fillet_too_large`), and its error message has `pointer == Some("/radius")` and a `suggestion` equal to the `{max}` text of its message (for example `2.3 mm`); f-cha1 `Ok`, its `bodies_before` b-1 has volume 960; final b-1 volume 950, 7 faces; `bodies` b-1 `last_feature == f-cha1`.
21. Upstream failed: `DOC-A` with f-sk1 given the constraints `[{"id": "c-1", "type": "distance", "line": "e-1", "expr": "80 mm"}, {"id": "c-2", "type": "distance", "line": "e-1", "expr": "50 mm"}]`: f-sk1 `Error` (`sketch_conflict`); f-ext1 `Error` (`upstream_failed`, text `'Sketch 1' has an error, so this feature cannot use it. Fix 'Sketch 1' first.`); f-fil1 `Error` (`body_empty`: f-ext1 failed, so b-1 is still empty); f-box2 and f-dp1 `Ok`.
22. Upstream suppressed: `DOC-A` with f-sk1 suppressed: f-sk1 `Suppressed`; f-ext1 `Error` (`upstream_suppressed`); f-box2 `Ok`.
23. Unknown type: a record `{"id": "f-g1", "type": "gear_demo", "teeth": 12}` inserted before f-box2 in `DOC-A`: f-g1 `Error` (`unknown_type`) with no key; f-box2 `Ok` with the same key as in test 6.
24. Unknown body: `{"id": "f-fil9", "type": "fillet", "body": "b-9", "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-5,10|l=10"}], "radius": "1"}`: `Error` (`unknown_body`, pointer `/body`).
25. Forward reference: `DOC-A` with f-sk1 moved after f-ext1: f-ext1 `Error` (`forward_reference`, pointer `/profile/sketch`); f-sk1 `Ok`.
26. Undeclared read: `{"id": "f-t1", "type": "test_peek", "body": "b-2"}` after f-box2 in `DOC-A`: `Error` (`undeclared_dependency`); the following features run normally.
27. Undeclared update: `{"id": "f-t2", "type": "test_stray", "body": "b-2"}` after f-box2: `Error` (`undeclared_dependency`); b-2 unchanged (volume 1000).
28. Panic: `{"id": "f-t3", "type": "test_panic"}` after f-box2: `Error` (`internal`, text containing `boom`); later features run; a second run builds f-t3 again (`from_cache` false) and reports the same error.

**Cancellation**

29. Canceled before the run: a canceled token: every active feature `NotRun`, `canceled` true, every body empty, cache unchanged.
30. Canceled during the run: features `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`, `{"id": "f-w1", "type": "test_wait"}`, `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["10", "10", "10"], "op": "new"}`; another thread cancels 50 ms after `run` starts. Expected: f-box1 `Ok`, f-w1 `NotRun`, f-box2 `NotRun`, `canceled` true, b-1 volume 1000, b-2 empty. A following run with a new token that another thread cancels after 100 ms has `from_cache` true for f-box1 (its result was cached), and f-w1 and f-box2 `NotRun` again (a canceled build is never cached).

**Values, units and context**

31. Context values: a box with size `["printer.fit.slip * 100", "10", "10"]` and a context map `{"printer.fit.slip": 0.2 mm}`: the box is 20 × 10 × 10, volume 2000. With 0.3 mm the box is rebuilt (volume 3000); with 0.2 mm again it is reused. Without the name in the context: `Error` (`expression`, rendered `Length: Unknown value 'printer.fit.slip'. Check the spelling; printer values look like printer.fit.slip.`).
32. Units: boxes `f-box1` with size `["10", "10", "10"]` and `f-box2` (in b-2) with size `["10 mm", "10 mm", "10 mm"]` in an `mm` document; after changing the units to `in`, `plan` reports `Build` for f-box1 and `Reuse` for f-box2.
33. Parameter report: a parameter `bad` with expression `"2 +"`: its `ParameterReport` value is the parse error; a box using `bad` has `Error` (`expression`).

**Cache limit and determinism**

34. Eviction: `with_cache_limit(2)`; run a document with three boxes in three bodies (3 entries, all used, none evicted: `cache_len() == 3`); then run a document with one different box: `cache_len() == 2`, holding the new box's entry and one entry of the first run.
35. Determinism: two fresh `Recompute`s running `DOC-A` give identical keys, statuses, messages, volumes and element names; `CacheKey` display is 64 lowercase hex characters.
36. Repair data: `DOC-A` with f-fil1's edge query changed to `edge(f-ext1/end-cap|f-ext1/nothing)` (face terms in byte order, naming.md section 3.1) and a far fingerprint: f-fil1 `Error` (`references_not_found`); with a second, valid edge added the status is `Warning` and its message has pointer `/edges/0` and a `Repair::Element` with body `b-1`.

**Performance**

37. The budgets of section 5 hold in the benchmarks.

**Rollback, links and assets**

38. Rollback before the first feature: `DOC-A` with `"rollback": "f-sk1"`: every feature is `RolledBack` with no key, and every body is empty.
39. Unknown feature link: `DOC-A` with f-ext1's `profile.sketch` set to `f-nope`: f-ext1 `Error` (`unknown_feature`, pointer `/profile/sketch`); f-fil1 `Error` (`body_empty`); f-box2 and f-dp1 `Ok`.
40. Missing asset: bodies b-1; the record `{"id": "f-imp1", "type": "import", "body": "b-1", "asset": "<64 zeros>.step", "op": "new"}` ([features/import.md](features/import.md)) with an empty asset map: `plan` reports `Fail`; the run gives `Error` (`asset_missing`, pointer `/asset`) and caches nothing for it. With the asset map of features/import.md test 1, `plan` reports `Build` and the run is `Ok`.

## Open issues

1. **`NotRun` status.** Protocol's feature states are ok, warning, error, suppressed and rolled back. Cancellation adds `NotRun`; engine.md lists cancellation as later, so the engine never sees it in M1, but the protocol will need it when cancellation ships.
2. **Failed writers stay providers.** A feature that fails still counts as the provider of the bodies it declares, so downstream keys depend on it even though it changed nothing. This keeps keys a pure function of the timeline at the cost of rebuilding downstream features when a failing feature's params change.
3. **Test 30** depends on wall-clock waits (50 ms and 100 ms against a 10 s ceiling). The margins are wide, but CI on slow runners may need them scaled.
4. **Cold run of `bench50` over budget.** Measured in release builds on the 4-core reference container (load average 3 to 16), October 12 2026, alternating builds before and after the change in one session: `run` of `bench50` from an empty cache (section 5, 1 s) takes 2.56 s (median of six benchmark runs, each the median of 7 runs: 2.41 to 2.70 s), against 2.96 s (2.82 to 3.18 s) before the body updates carried the topology and volume their features measured and hole read overlaps from its cut's history (features.md open issue 6), and 4.37 s in the previous measurement, before the kernel checked the validity of only what an operation changed (kernel.md section 3.5). Recompute's own overhead is a few milliseconds; per feature (medians of 5 cold runs, 2.56 s in all), the five cut extrudes f-ext5 to f-ext9 take 0.94 s (143 to 226 ms each), holes f-hole3 and f-hole4 0.36 s, the booleans 0.34 s, fillets f-fil5 to f-fil7 0.29 s, chamfers f-cha3 to f-cha6 0.27 s and the mirrors 0.15 s; the other 32 features take 0.22 s together. Nearly all of it is OCCT's booleans and blends on the large merged bodies (b-1 ends with 426 faces): each of them now reads the kernel topology only of its result (to name it) and no longer measures its input body again. Meeting the budget needs faster kernel operations on large bodies and a measurement on an idle reference machine; if OCCT's operations alone stay over 1 s, the budget is revisited in a new decision. The other budgets of section 5 hold in the same session: `plan` of 300 features 3.2 ms (10 ms), overhead per feature 0.006 ms cold and 0.003 ms cached (0.1 ms), `bench50` cached 0.39 ms (5 ms), the edit of the last 5 features 36 ms (60 ms), and cancel latency 0.8 to 1.1 ms (5 ms).
