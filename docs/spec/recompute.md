# Recompute: dependency graph, dirty tracking and result cache

Status: M1 spec. Crate: `partsmith-recompute`. Read with [architecture.md](../design/architecture.md) (section "Recompute"), technical design section 14, [features.md](features.md) (the `Feature` contract this crate drives, its messages and their `Diagnosis`), [naming.md](naming.md) (element names, naming maps and the names-only `NameSet` the loss diagnosis reads), [model.md](model.md) and [expressions.md](expressions.md) (documents and parameter evaluation), and [engine.md](engine.md) (the only production caller). How losses are explained follows [ADR 0021](../adr/0021-resolve-references-by-meaning.md) (keep and explain every loss) and [ADR 0024](../adr/0024-diagnostics-with-actions-causes-and-ghosts.md) (fixes, details, roots), and every message follows [the error guidelines](../guidelines/errors.md).

## 1. Purpose and scope

`partsmith-recompute` runs a document's timeline: it builds the dependency graph, evaluates parameters and feature expressions, computes a content hash for every feature, reuses cached results for unchanged hashes, builds the rest through the feature registry, isolates failures, and reports per-feature status, timing, outputs and body states. It is pure Rust over `partsmith-model` and `partsmith-features`, holds no global state, and is deterministic.

### In M1

- Dependency DAG over the timeline: feature outputs, body state chains, parameters and context values.
- Dirty tracking through content-hash cache keys (BLAKE3 of canonical params, evaluated values, upstream keys and the versions of `partsmith-features` and OCCT), plus graph queries for "what does this change affect".
- An in-memory result cache, shared by all documents of a session, with LRU eviction by entry count.
- Rollback marker (the first rolled-back feature) and an explicit stop point for previews.
- Asset bytes supplied by the host for features that read imported files.
- Suppression, failure isolation, panic isolation.
- Per-feature status, warnings, info notices and errors with rendered messages, severities, one-click fixes, details and repair data, timing, outputs, and body states before each feature.
- Per-body writer chains passed to builds (section 3.5).
- Loss diagnosis: why each lost reference was lost, which feature a followed reference followed, the root of failure chains and why a body is empty, computed from names only, memoized, and attached to the rendered message (section 3.10).
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
pub struct Recompute { /* registry, cache, memoized loss reasons (section 3.10), run counter */ }

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
    /// Warnings and info notices, from the output first, then those recompute adds (`UnknownParams`).
    pub warnings: Vec<FeatureWarning>,
    /// The error, warnings and notices rendered with current names, units and the loss diagnosis (section 3.6):
    /// error first, then warnings of severity warning, then info notices, each group in the order of `warnings`.
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
    /// Built or reused with no warnings, or with info notices only.
    Ok,
    /// Built or reused with at least one warning of severity warning.
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
    /// Info, warning or error.
    pub severity: Severity,
    /// Stable snake_case code, for example "reference_missing".
    pub code: &'static str,
    /// JSON Pointer of the field it is about, relative to the feature record, for example "/edges/1".
    pub pointer: Option<String>,
    /// The message text, rendered with current names and units and, for losses, the diagnosed reason (section 3.10).
    pub text: String,
    /// Replacement expression for the field at `pointer`, in display units, when the feature computed a limit (features.md section 4.4).
    pub suggestion: Option<String>,
    /// Repair data for lost, guessed or widened references and regions: one per loss of `ReferencesNotFound` and per
    /// region of `ProfileNotFound`, else at most one (features.md section 4.2, `repairs`).
    pub repairs: Vec<Repair>,
    /// One-click fixes in the order to offer them: the error's or warning's `fixes`, which merge the fixes of the
    /// diagnosis (features.md section 4.4).
    pub fixes: Vec<Fix>,
    /// Technical text for reports (features.md `detail`), never shown as the message.
    pub detail: Option<String>,
    /// The first failing feature of an `upstream_failed` chain, of a loss whose reason is `UpstreamFailed`, or of the
    /// feature a `body_empty` or `tool_empty` names as failed; or the sketch that lost its face under a `no_effect`,
    /// `hole_misses_body` or `separate_solid`. Hosts group one root cause as one problem (section 3.10).
    pub root: Option<Id>,
    /// Why the reference the text names (its first loss) was lost, or which feature a followed or widened reference
    /// followed (section 3.10). None for messages without a diagnosed loss or follow, and for an ambiguous loss.
    pub reason: Option<LossReason>,
}

/// Severity of a message: `Info`, `Warning` or `Error` (features.md section 2.8).
pub use partsmith_features::Severity;
```

`LossReason` and `Removal` are defined in section 3.10. `CancelToken`, `FeatureOutput`, `BodyState`, `FeatureError`, `FeatureWarning`, `Repair`, `Fix` and `Severity` come from `partsmith-features` and are re-exported. `Report` and its parts derive `Debug` and `Clone`; none is serialized (the engine maps them to protocol types). `Recompute` is `Send`.

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
- A build that reads a declared body's history (section 3.5, features.md `BuildContext::body_history`) is still a function of its key: the body's provider key hashes the provider key of every earlier writer of the body (item 8, recursively), and each state in the chain is the output of one of those writers. No key changes.
- The loss diagnosis (section 3.10) reads records outside a feature's key, so its explanations are never part of a cached result.

### 3.5 Running

Features run sequentially in timeline order. A running state maps each body to its current `BodyState` (all start empty) and its provider key. For each record:

1. `RolledBack`, `Suppressed`: status as named; nothing else. `Invalid`: status `Error` with the validation error.
2. If `cancel.is_canceled()`: this and every later `Active` record get `NotRun`; the run stops and `canceled` is true.
3. Evaluate the declared expressions (`Values::evaluate`) and compute the key (section 3.4).
4. Pre-build checks (section 3.3). On failure: status `Error`, no output, bodies unchanged; the key is kept (this feature still provides the bodies it declares).
5. Record `bodies_before` from the running state.
6. **Cache.** If the key is cached, reuse the result (`from_cache: true`, duration zero).
7. **Build.** Otherwise build through `Registry::build` with a `BuildContext` holding only the declared bodies' states, the declared bodies' writer chains (`BuildInputs::history`, below), the declared features' outputs, the declared assets' bytes and the run's `CancelToken`, inside `catch_unwind`. A panic becomes `Internal { message }` with the panic message (or `unknown panic`). The duration is measured around the build.
8. **Check the output.** A `BodyUpdate` for a body not in `bodies_written`, or two updates for one body, turns the result into `UndeclaredDependency`; nothing from that output is applied.
9. **Store.** The result (output or error) is cached under the key, except `Canceled` and `Internal`. A build that returns `Canceled` gives status `NotRun` and stops the run as in step 2.
10. **Apply.** On success, apply each `BodyUpdate` to the running state: the body's state becomes `BodyState::from_update` (its shape and naming map, with the topology and volume the feature measured, features.md section 2.7, so later features and hosts reuse them) and its provider key this key. On error, bodies are unchanged; the feature is still the provider of the bodies it declares in `bodies_written`, so later keys stay a function of the timeline.
11. **Status.** `Ok` without warnings or with info notices only, `Warning` with a warning of severity `Warning` (including `UnknownParams`, added here and not cached), `Error` with an error. Severities are the warnings' `severity()` (features.md section 2.8): `ReferenceFollowed` and `RegionSplit` are info notices.

After the loop, `bodies` are the running states (the states at the stop point or before the rollback marker, or after a cancel the states after the last finished feature). Then the loss diagnosis runs (section 3.10) and the messages are rendered (section 3.6).

**Writer chains.** The running state also keeps, per body, its writer chain: for each `Active` record whose `bodies_written` contains the body and that ran, was reused, or failed (in its pre-build checks or its build), in timeline order, the record's id and the body's state in the running state just before it. The entry is appended once the record is done (after step 10, or after a failure in step 4), so a build sees only earlier writers. Step 7 passes each declared body's chain to the build as `BuildInputs::history` (features.md section 2.6). `Suppressed`, `RolledBack`, `Invalid` and `NotRun` records are never in a chain. The chains hold `Arc`s of states the running state and the cache already keep, so they cost no copies. The chains of every body are kept until the run ends: with `Report::bodies` they give every body's state just before any feature (the state just before the body's first writer at or after that feature, else its final state), and with the reports' outputs the naming maps the loss diagnosis walks (section 3.10). `plan` is unchanged.

Failure isolation: a failing feature never stops the run. Features that read a failed feature's bodies build on the states before it; features that read its output report `UpstreamFailed`.

### 3.6 Messages

`FeatureReport::messages` are rendered at report time, after the loss diagnosis (section 3.10), also for cached results: the error first (severity error), then the warnings of severity warning, then the info notices, each group in the order of `FeatureReport::warnings`. Each error, warning or notice is rendered with its own `RenderContext` (features.md section 2.8): the feature, `DocumentNames { document, registry, outputs }` with the outputs of this run, the document's units, the feature's `bodies_before` as a map, and the `Diagnosis` that section 3.10 computes for it (`Diagnosis::default()` when it has nothing to explain). The `Message` has its `severity()`, `code()`, the JSON Pointer of its field (`FieldRef::pointer`), `render(&context)` as its text, `suggestion(units)`, its repair data (`repairs()` for an error, `repair()` for a warning or notice), `fixes(&context)` (the order of features.md section 4.4, with the fixes of the diagnosis merged in), `detail()`, and `root` and `reason` from section 3.10. A rename or a units change re-renders every message, explanations included, without building anything.

### 3.7 The cache

- Entries map a `CacheKey` to the cached result: the output (`Arc<FeatureOutput>`) or the error. Body states derived from a cached output (`BodyState::from_update`) are cached with it, so the topology, element infos and volume computed once (`BodyState::topology`, `elements`, `volume`) are shared by later runs.
- One `Recompute` serves every document of a session; identical features in different documents share entries (their keys are equal only when their ids, params, values and upstream keys are).
- Each run records which entries it used. After a run, while there are more entries than the limit, the entry whose last use is oldest (ties: smaller key first) and that the latest run did not use is evicted. Entries the latest run used are never evicted, so a run can exceed the limit.
- Memoized loss reasons (section 3.10) belong to the cache entry of their feature's key: they are evicted with it, never counted as entries of their own, and `clear_cache` drops them.
- `plan` and `graph` never modify the cache.

### 3.8 Plan

`plan(input)` performs sections 3.1 to 3.4 without building, using cached errors to predict failures: `Fail` for invalid nodes, nodes that fail pre-build checks, and nodes whose feature links point at records whose cached result is an error; `Reuse` when the key is cached; `Build` otherwise. Since a node marked `Build` may fail when built, a later `Reuse` or `Build` prediction can turn into `UpstreamFailed` at run time; `plan` and `run` agree on every key.

### 3.9 Determinism

The same `Recompute` inputs give the same report except `Timing`, `elapsed` and `from_cache`: equal statuses, keys, messages (texts, fixes, details, roots and reasons), outputs (equal names and kernel results) and body states, in every session and on every platform built from the same sources. A run whose loss reasons all come from the memo gives the same messages as a cold run. Iteration never depends on hash-map order. Parallel execution, when added, must keep this.

### 3.10 Loss diagnosis

After the run (also a canceled one, for the features that finished), recompute explains what a feature cannot know from its own inputs: every `Loss` in the reports (`ReferencesNotFound`, `ReferenceNotFound`, `ReferenceLost`, `PlaneFallback`); every `ReferenceFollowed` and `ReferenceWidened` (`by`); every `BodyEmpty` and `ToolEmpty` (`because`); every `UpstreamFailed` (`root`); and every `NoEffect`, `HoleMissesBody` or `SeparateSolid` of a feature whose profile or point sketch reports `PlaneFallback` in the same run (`lost_face`: the root is that sketch, and the `NoEffect` and `HoleMissesBody` texts say so, features.md section 3.15). The explanation of one error, warning or notice is a features.md `Diagnosis` (section 2.8), which section 3.6 renders it with, plus the message's `root` and `reason`. It goes on the rendered `Message` only, never into the feature's error or warning: those are cached under the feature's key, while the explanation reads records outside it (section 3.4).

**What it reads.** The document (records, their states, timeline order and the rollback marker), the graph, the reports of this run (statuses, errors, outputs: their `absent` faces, sketch results and body updates), the writer chains of section 3.5 and the naming maps of their states. Faces are matched by name through `NameSet::new` of each naming map it walks (naming.md section 2.7: families through canonical names, aliases, successions and merged names, and which faces touch, where `edge(a|b)` makes `a` and `b` touch). It never reads fingerprints, element infos, topology or the kernel, so it computes no `BodyState::elements` or `BodyState::topology` for any state.

**The lost name.** For a `Loss` of feature `F` in body `B` (`Loss::body`), let `t` be the first name of `lost.parts`, else the first of `lost.pieces`, else, when `lost.unmet`, the query itself (`Reference::query`). Let `g` be the feature id that starts `t`, or, for a name with `@` groups, the feature of its last `@` group (`f-box1/top@f-lp1:2` gives `f-lp1`); for `unmet`, the latest in timeline order of the features found that way for the query's face terms. A loss whose `lost` is empty and that is not ambiguous gets `Unknown`. An ambiguous loss gets no reason (below).

**Reason.** The first rule that applies gives the reason. Rules 1 to 6 are skipped for `unmet`, whose parts are all present.

1. `g` names no record of the document: `NotInDocument { feature: g }`. Whether it was deleted is session knowledge, which the engine's cause line adds (engine.md section 3.21).
2. `g` is suppressed: `Suppressed { feature: g }`. Else `g` is at or after the document's rollback marker (`Document::rollback`; a preview's stop point is not a marker): `RolledBack { feature: g }`. Else `g` comes after `F` in timeline order: `ComesLater { feature: g }`. Else `g` is `Invalid` or ended with an error: `UpstreamFailed { feature: g, root }`, with `root` the first feature of `g`'s chain whose error is its own: from `g`, an `UpstreamFailed { feature: h }` error moves on to `h`, so `root` is `g` when `g`'s error is its own.
3. Every part of the query is present by name (its family is not empty) in the naming map of another body `C`'s state just before `F` (section 3.5, writer chains), and, for an edge or vertex query, that map names an edge or vertex whose face terms pair one to one with members of the parts' families: `OtherBody { body: C, by, by_suppressed }`, with `by` the last record before `F` whose dependencies write `B` and read or write `C` (a split or a boolean), suppressed or not, `by_suppressed` true when that record is suppressed, and `by` None when there is no such record. Of several such bodies, the first in `Document::bodies` order.
4. `g` is a pattern or mirror and no body update of its output names a face with the last `@` group of `t` (`@<g>:<instance>`): `CopyGone { feature: g, instance }`.
5. `t` has a sketch generator (`<feature>/<role>/<generator>` whose generator is a sketch entity id or point reference, naming.md section 3.1), with `S` the sketch `g` reads (its first feature link whose record has type `sketch`): when `g` is a hole and `S`'s result in this run has no such point (`SketchResult::point`), `PointGone { feature: g, sketch: S, point: generator, deleted: true }`; when `g` is a hole, `S` has the point, and `g`'s `points` param no longer lists it, `PointGone { ..., deleted: false }`; when `g` is not a hole and `S`'s result has no entity with that id, `EntityDeleted { feature: g, sketch: S, entity: generator }`.
6. `g` is an `import` and `t` is a fallback name (naming.md section 3.1): `NotInFile { feature: g }`.
7. **The remover.** Walk `B`'s writer chain up to `F` (section 3.5). A writer's *input* is the naming maps of its `bodies_before` (every body it declares, so a boolean's tool bodies count); its *output* is the naming map of `B` after it (`B`'s state just before the next writer of `B`, or just before `F`). A map *has* `t` when it names a face in the family of `t`; for a part of `lost.pieces` that has anchors in the reference (`nb`), only a member that touches every anchor whose family is not empty in that map counts (the anchor test of naming.md section 3.9, from names); the query's own `#n` is matched without its anchors, whose test needs vertex infos (naming.md section 2.7). For `unmet`, a map has the query when it names an edge or vertex whose face terms pair one to one with members of the parts' families. The *remover* `W` is the last writer whose input has `t` and whose output does not; when there is none and `g` is a writer of `B` whose output does not have `t`, `W` is `g`. With a remover, the reason is `Removed { by: W, how }`, with `how` from the first of these that applies:
   1. `W` is `g`: `NoLongerMeet { between }` for `unmet`, else `NoLongerMade { absence }`, with `absence` the `why` of the entry of `g`'s `FeatureOutput::absent` whose name `t` extends, if any.
   2. `W` is a fillet or a chamfer whose output names a face generated from the query (`<W>/fillet/<n>` or `<W>/chamfer/<n>` with `n`'s forms meeting the canonical set of the query): `Rounded` or `Beveled`.
   3. `W` is a shell: `Hollowed`.
   4. `t` is in `lost.pieces`: `PieceGone`.
   5. `W`'s update of `B` has change `Cut` or `Intersect`, or `W` is a split body: `CutAway`.
   6. It has change `Join`: `Covered`.
   7. `unmet`: `NoLongerMeet { between }`.
   8. `W` is a fillet or a chamfer: `Rounded` or `Beveled`.
   When none applies, the reason is `Unknown`. `between` is the face that now runs where the edge was: among the faces of `W`'s output that touch a member of the family of every part, the first in byte order that also touches every other such face (so not a cap at either end of the old edge, which touch the parts but not each other), or None when no face does.
8. `g` is `Active`, comes before `F`, and its dependencies write other bodies but not `B` (its operation or target changed): `OtherBody { body: the first body g writes, by: Some(g), by_suppressed: false }`.
9. Otherwise: `Unknown`.

**Ambiguous losses** get no reason. Their `after` (features.md `Explanation::after`) is the feature `g` that starts the first part of the query, in query order, that has a sketch generator and has relatives in `B`'s state just before `F` (faces of the same feature, sketch generator and `@` groups under another role, naming.md section 3.9 step 4), when `g` is `Active`, comes before `F` and did not fail; otherwise None. So a hole whose type changed names the hole (corpus R63a), and an imported file's fallback names, which have no relatives, name nothing (corpus R46b).

**Follows.** A `ReferenceFollowed` or `ReferenceWidened` gets the reason `Followed { by }` and `Diagnosis::by`, with `by`:

- `Replaced` or `Extended`, and every widening: the first writer of `B` whose output map records a succession whose key is in `via` (`NamingMap::successions`). A succession is recorded by the extrude or revolve that read the edited sketch, and `by` is that sketch (the writer's first feature link whose record has type `sketch`), because the sketch edit is what the user did.
- `Merged`: the first writer of `B` whose output map records a merge alias whose key is in `via` (`NamingMap::aliases`).
- `Continued`: the feature that starts the name of the continuing rim (`<feature>/rim/<s>`) among the faces of the elements the reference now resolves to.
- `Related`: the feature that starts the query's part that is not present, whose relative is used.

When none is found, `by` is None, the message has no reason, and features render their words without one (features.md section 4.2).

**Empty bodies, roots and lost faces.**

- `because` (`Diagnosis::because`) of a `BodyEmpty` or `ToolEmpty` on body `X` of `F`: walk the writer chain of `X` up to `F`, newest first. A writer whose error is `BodyEmpty` or `ToolEmpty` on `X` is passed over (it failed because `X` was already empty); one that ended with another error gives `Failed(it)`; one whose update emptied `X` (change `Clear`) gives `UsedAsTool { feature: it, target: the first body of its bodies_written that is not X }`; any other ends the walk. When the walk gives nothing, the last `Suppressed` record before `F` whose dependencies write `X` gives `Suppressed(it)`; else the first record after `F` whose dependencies write `X` gives `ComesLater(it)`; else there is none.
- `root` (`Message::root`): for an `UpstreamFailed { feature: h }` that recompute raised (section 3.3), the first feature of `h`'s chain whose error is its own, as in rule 2 (`Diagnosis::root` is set only when that feature is not `h`, so the text names it); for a message whose first loss has the reason `UpstreamFailed`, that reason's `root`; for a `BodyEmpty` or `ToolEmpty` whose `because` is `Failed(h)`, the first feature of `h`'s chain whose error is its own; for a message with `lost_face`, that sketch. Otherwise None.
- `lost_face` (`Diagnosis::lost_face`): for a `NoEffect`, `HoleMissesBody` or `SeparateSolid` of `F`, the sketch named by `F`'s first feature link whose record has type `sketch`, when that sketch's report in this run has a `PlaneFallback` warning.

```rust
/// Why a reference was lost, or which feature a followed reference followed. Holds ids, never display names, so
/// it renders with current labels.
#[derive(Debug, Clone, PartialEq)]
pub enum LossReason {
    /// Not diagnosed: no rule explains it.
    Unknown,
    /// The feature that made it is not in the document.
    NotInDocument { feature: Id },
    /// The feature that made it is suppressed.
    Suppressed { feature: Id },
    /// The feature that made it comes after this one in the timeline.
    ComesLater { feature: Id },
    /// The feature that made it is at or after the rollback marker.
    RolledBack { feature: Id },
    /// The feature that made it failed; `root` is the first feature of its chain whose error is its own.
    UpstreamFailed { feature: Id, root: Id },
    /// It is now in another body, put there by `by` when known.
    OtherBody { body: Id, by: Option<Id>, by_suppressed: bool },
    /// A feature between the one that made it and this one removed it.
    Removed { by: Id, how: Removal },
    /// The sketch entity that made its face was deleted.
    EntityDeleted { feature: Id, sketch: Id, entity: Id },
    /// A hole no longer uses the point that made it; `deleted` when the sketch no longer has the point.
    PointGone { feature: Id, sketch: Id, point: String, deleted: bool },
    /// A pattern or mirror no longer makes the copy it was on.
    CopyGone { feature: Id, instance: Instance },
    /// An imported file no longer has it.
    NotInFile { feature: Id },
    /// For a followed or widened reference: the feature whose change it followed.
    Followed { by: Id },
}

/// How a feature removed an element.
#[derive(Debug, Clone, PartialEq)]
pub enum Removal {
    /// A cut, an intersect or a split took it away.
    CutAway,
    /// A join covered it, so it is inside the part.
    Covered,
    /// A fillet rounded it.
    Rounded,
    /// A chamfer beveled it.
    Beveled,
    /// A shell removed it.
    Hollowed,
    /// The piece of a split face it was on is gone.
    PieceGone,
    /// The feature that made it no longer makes it; `absence` says why when the feature recorded it.
    NoLongerMade { absence: Option<Absence> },
    /// Its faces are still there but no longer meet; `between` is a face that now lies between them.
    NoLongerMeet { between: Option<ElementName> },
}
```

`Instance` comes from partsmith-naming, `Absence` from partsmith-features.

**Rendering.** Each loss of the message becomes an `Explanation { text, after, fixes, upstream }` in `Diagnosis::losses` under its `Loss::position`. For an ambiguous loss it carries only `after`, with an empty `text`, because features render `now matches {n} {kind}s{after}` in its place (features.md section 4.2). Otherwise `text` is the reason clause below, rendered at report time with current labels and units; `{this}` is `this` followed by `F`'s manifest label in lowercase (features.md section 3.12), and descriptions of faces follow features.md section 3.12.

| Reason | Text |
| --- | --- |
| `NotInDocument` | `is gone` when the element's description is already the description of features.md section 3.12 for a feature that is not in the document (`an edge of a feature that is not in this document`), else `is made by a feature that is not in this document` |
| `Suppressed` | `belongs to '{feature}', which is suppressed` |
| `ComesLater` | `is made by '{feature}', which now comes after {this}` |
| `RolledBack` | `is made by '{feature}', which is after the rollback marker` |
| `UpstreamFailed` | root is the feature: `is missing because '{feature}' failed`; otherwise `is missing because '{feature}' cannot be built while '{root}' has an error` |
| `OtherBody` | `is now in body '{body}'`, followed by `, put there by '{by}'` when `by` is set and not suppressed, or ` while '{by}', which joins it to this body, is suppressed` when `by_suppressed` |
| `Removed`, `CutAway` | `was cut away by '{by}', which comes before {this}` |
| `Removed`, `Covered` | `is now inside the part, covered by '{by}'` |
| `Removed`, `Rounded` | `was rounded by '{by}', which comes before {this}` |
| `Removed`, `Beveled` | `was beveled by '{by}', which comes before {this}` |
| `Removed`, `Hollowed` | `was removed by '{by}', which hollows the part` |
| `Removed`, `PieceGone` | `no longer exists: that part of {face} was removed by '{by}'`, with `{face}` the description of `t` without its `#n` groups |
| `Removed`, `NoLongerMade` | `is no longer made by '{by}'`, followed by `, which goes all the way through the body` (`Through`) or `, which turns all the way round` (`FullTurn`) |
| `Removed`, `NoLongerMeet` | `no longer exists, because its faces no longer meet`, followed by `: {between} now lies between them` when `between` is set; when `between` is a round face made from a sketch arc, `is already round: '{sketch}' now rounds that corner` instead |
| `EntityDeleted` | `is gone because that {line or curve} was deleted from '{sketch}'` |
| `PointGone` | deleted: `is gone because '{feature}' no longer makes a hole at that point, which was deleted from '{sketch}'`; not deleted: `is gone because '{feature}' no longer makes the hole at ({u}, {v}): that point was removed from its Points` |
| `CopyGone` | `is gone because '{feature}' no longer makes its {copy}` |
| `NotInFile` | `is not in the new version of the file imported by '{feature}'` |
| `Unknown` | `can no longer be found, and Partsmith could not tell why` |

- `{between}` is described in `B`'s state just before `F`. It is a *round face made from a sketch arc* when its name, or the face name its generator leads to through face-name generators (a shell's `inner` and `rim` faces), is a lateral face `<f>/lateral/<e>` of an extrude or revolve `f` whose sketch's result in this run has `e` as an arc or circle; `{sketch}` is that sketch.
- `{line or curve}` is `line` when the stored fingerprint shows the deleted entity was a line: for a face query, an extrude's lateral face whose fingerprint is a `plane`; for an edge query, an edge between the lateral face and a cap (`start-cap` or `end-cap`) of the same feature whose fingerprint is a `line`. Otherwise `curve`, which is true of every entity.
- `({u}, {v})` are the point's coordinates in `S`'s result, in document units, written as features.md section 3.12 writes sketch coordinates.
- `{copy}` names the copy as features.md section 3.12 does: `third copy` for instance 2 (the ordinal of the first index plus 1), `copy in row 2, column 3` for two indices, `mirror image` for a mirror.

`fixes` (`Explanation::fixes`), in the order features.md section 4.4 places them among the timeline fixes: `Suppressed` gives `Unsuppress { feature }`; `ComesLater` gives `Move { feature: F, place: Below(feature) }`; `OtherBody` from rule 3 gives `Unsuppress { feature: by }` when `by_suppressed`, then `UseBody { field, body }` with `field` the body field next to the reference (the sibling `body` of a `face` or `edge` field, such as `base.body` for `base.face`, else the feature's own `body`); `UpstreamFailed` gives `Show { feature: root, entity: None }` and sets `upstream`, so no `Pick` is offered, because re-picking would destroy a reference that is still right. `RolledBack` gives none: moving `F` below a feature after the marker would roll `F` back too. Every other reason gives none; reverting the edit that caused it belongs to the engine's cause line (engine.md section 3.21).

**Memoization.** The reasons, `after` and `by` of a feature's losses and follows are memoized in the `Recompute`, found by the feature's cache key, the message's code, field and loss position, and the stored reference. An entry holds the result (ids, never text) and the observations it was computed from: for each record it consulted (`g`, the remover, `by`, the records of a root chain, the sketch of rule 5), whether it is in the document, its state (`Active`, `Suppressed`, `RolledBack` or `Invalid`), whether it ended with an error and the target of an `UpstreamFailed` error, and whether it comes before the feature; the document's rollback marker; the provider key of every body whose state it read other than the feature's declared bodies (rules 3 and 7); and the cache key of every record whose output it read (a sketch's result, `absent` faces). The feature's own key already covers its declared bodies' writer chains, their naming maps and their writers' outputs (section 3.4). A later run reuses an entry when the feature has the same key and every observation still holds, and computes it again otherwise, so suppressing the remover, or moving the feature that made the element after this one, changes the explanation of a result reused from the cache. `root`, `because` and `lost_face` read only statuses and dependencies and are computed in every run. A run with the same losses reuses every reason, and a rename re-renders their texts without computing them again (section 3.6).

## 4. Errors

Recompute has no error type of its own: every problem is a feature's status (`FeatureError` and `FeatureWarning`, features.md section 4) or a document-level `Message`. Recompute raises these `FeatureError` variants itself: `UnknownFeature`, `ForwardReference`, `UpstreamFailed`, `UpstreamSuppressed`, `UnknownBody`, `AssetMissing` (section 3.3), `UndeclaredDependency` (section 3.5 step 8), `Internal` (panics), and it adds the `UnknownParams` warning. The texts of all of them are features.md section 4's; recompute adds only the explanations of section 3.10, which features render. Document-level messages have no repairs, fixes, detail, root or reason:

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
| Diagnosis of one lost reference on `bench50`, nothing memoized, the remover found by walking the whole writer chain of its body (section 3.10, rule 7) | 1 ms |
| `run` of `bench50` with every result cached and 5 lost references | 5 ms (the same as without losses: the reasons are memoized) |

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
15. Unknown keys: `DOC-A` with `"taper": "2 deg"` added to f-ext1: f-ext1 status `Warning` (`unknown_params`, text `This extrude has settings Partsmith does not know ('taper'). They are kept in the file and ignored.`); every key equal to test 6; changing `taper` to `"3 deg"` keeps every key.
16. Messages re-render: `DOC-A` plus `{"id": "f-box9", "type": "box", "body": "b-2", "size": ["1", "1", "1"], "position": ["500", "0", "0"], "op": "cut"}`: f-box9 status `Warning` with the message `This cut does not touch body 'Body 2': it lies beside it.` (features.md section 3.15, a primitive has no direction); after renaming b-2 to `Lid`, the next run has `from_cache` true for f-box9 and the message is `This cut does not touch body 'Lid': it lies beside it.`

**Stop point, suppression, failures**

17. Rollback: `DOC-A` with `"rollback": "f-box2"` (f-box2 is the first rolled-back feature): f-box2, f-fil1 and f-dp1 are `RolledBack` with no key; `bodies` has b-1 at volume 40000 with 6 faces and `last_feature == f-ext1`, and b-2 empty. Clearing the rollback after test 6 on the same `Recompute` gives `from_cache` true for every feature.
18. Stop point: `DOC-A` with `stop_at = f-fil1`: f-dp1 is `RolledBack`; f-fil1's `bodies_before` holds b-1 with 6 faces; its output holds b-1 with 7 faces; `bodies` b-1 has 7 faces. `stop_at` overrides a document rollback of `f-sk1`.
19. Unknown stop: `stop_at = f-nope`: every feature runs; `messages` holds `unknown_stop`.
20. Failure isolation: bodies b-1; `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "2.4"], "op": "new"}`; f-fil1 filleting the four top edges with radius 3; `{"id": "f-cha1", "type": "chamfer", "body": "b-1", "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,2.4|l=20"}], "distance": {"kind": "equal", "expr": "1"}}`. Expected: f-fil1 `Error` (`fillet_too_large`), and its error message has `pointer == Some("/radius")` and a `suggestion` equal to the `{max}` text of its message (for example `2.3 mm`); f-cha1 `Ok`, its `bodies_before` b-1 has volume 960; final b-1 volume 950, 7 faces; `bodies` b-1 `last_feature == f-cha1`.
21. Upstream failed: `DOC-A` with f-sk1 given the constraints `[{"id": "c-1", "type": "distance", "line": "e-1", "expr": "80 mm"}, {"id": "c-2", "type": "distance", "line": "e-1", "expr": "50 mm"}]`: f-sk1 `Error` (`sketch_conflict`); f-ext1 `Error` (`upstream_failed`, text `'Sketch 1' has an error, so this extrude cannot use it. Fix 'Sketch 1' first.`, fixes `[Show { feature: f-sk1 }]`, `root` f-sk1); f-fil1 `Error` (`body_empty`: f-ext1 failed, so b-1 is still empty; text `Body 'Body 1' is empty here because 'Extrude 1', which makes it, failed. Fix 'Extrude 1' first.`, `root` f-sk1, the first feature of the chain whose error is its own); f-box2 and f-dp1 `Ok`.
22. Upstream suppressed: `DOC-A` with f-sk1 suppressed: f-sk1 `Suppressed`; f-ext1 `Error` (`upstream_suppressed`); f-box2 `Ok`.
23. Unknown type: a record `{"id": "f-g1", "type": "gear_demo", "teeth": 12}` inserted before f-box2 in `DOC-A`: f-g1 `Error` (`unknown_type`) with no key; f-box2 `Ok` with the same key as in test 6.
24. Unknown body: `{"id": "f-fil9", "type": "fillet", "body": "b-9", "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-5,10|l=10"}], "radius": "1"}`: `Error` (`unknown_body`, pointer `/body`).
25. Forward reference: `DOC-A` with f-sk1 moved after f-ext1: f-ext1 `Error` (`forward_reference`, pointer `/profile/sketch`); f-sk1 `Ok`.
26. Undeclared read: `{"id": "f-t1", "type": "test_peek", "body": "b-2"}` after f-box2 in `DOC-A`: `Error` (`undeclared_dependency`); the following features run normally.
27. Undeclared update: `{"id": "f-t2", "type": "test_stray", "body": "b-2"}` after f-box2: `Error` (`undeclared_dependency`); b-2 unchanged (volume 1000).
28. Panic: `{"id": "f-t3", "type": "test_panic"}` after f-box2: `Error` (`internal`) whose message's `detail` contains `boom` and whose text does not (features.md section 4.1); later features run; a second run builds f-t3 again (`from_cache` false) and reports the same error.

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
36. Repair data: `DOC-A` with f-fil1's edge query changed to `edge(f-ext1/end-cap|f-ext1/nothing)` (face terms in byte order, naming.md section 3.1) and a far fingerprint: f-fil1 `Error` (`references_not_found`); with a second, valid edge added the status is `Warning` and its message has pointer `/edges/0` and `repairs` holding one `Repair::Element` with body `b-1`.

**Performance**

37. The budgets of section 5 hold in the benchmarks.

**Rollback, links and assets**

38. Rollback before the first feature: `DOC-A` with `"rollback": "f-sk1"`: every feature is `RolledBack` with no key, and every body is empty.
39. Unknown feature link: `DOC-A` with f-ext1's `profile.sketch` set to `f-nope`: f-ext1 `Error` (`unknown_feature`, pointer `/profile/sketch`); f-fil1 `Error` (`body_empty`); f-box2 and f-dp1 `Ok`.
40. Missing asset: bodies b-1; the record `{"id": "f-imp1", "type": "import", "body": "b-1", "asset": "<64 zeros>.step", "op": "new"}` ([features/import.md](features/import.md)) with an empty asset map: `plan` reports `Fail`; the run gives `Error` (`asset_missing`, pointer `/asset`) and caches nothing for it. With the asset map of features/import.md test 1, `plan` reports `Build` and the run is `Ok`.

**Statuses, writer chains and the diagnosis**

Tests 41 to 68 and 71 live in `crates/partsmith-recompute/tests/diagnosis.rs`, tests 69 and 70 in the files they name. Each runs the scenario of the reference stability corpus or the newcomer error corpus it names ([reference-stability-corpus.md](../design/reference-stability-corpus.md), [newcomer-error-corpus.md](../design/newcomer-error-corpus.md)), after its edit, as a document of features.md section 6.1, with its references picked as the reference stability corpus's "Picked references" rule says. Labels follow timeline order (features.md section 3.12). "Reason" is `Message::reason` and "root" `Message::root`; "ends with" compares the end of `Message::text`, whose start, the element's description, is features.md's to test (its section 6.5, test 15); fixes are `Fix` values in `Message::fixes` order.

41. Info notices leave the status ok (section 3.5 step 11). BLOCK and `f-fil1` fillet radius 2 on the picked right top edge `edge(f-ext1/end-cap|f-ext1/lateral/e-2)`, after the edit of corpus R06 (a) (`e-2` deleted, `e-9` (80,0)-(80,50) drawn in its place). Expected: f-ext1 `Ok` with no messages; f-fil1 `Ok` with exactly one message: severity `Info`, code `reference_followed`, pointer `/edges/0`, reason `Followed { by: f-sk1 }`, text ending `, because its line was replaced in 'Sketch 1'.`, fixes `[KeepMatch { pointer: "/edges/0" }, Pick { pointer: "/edges/0" }]`, no repairs. With a second entry `{q: edge(f-ext1/end-cap|f-gone/x), fp: line|n=0,0,1|c=500,500,500|l=1}` in f-fil1's `edges`: f-fil1 `Warning`, and its messages are the `reference_missing` warning at `/edges/1` first, then the notice.
42. Writer chains reach the build (section 3.5). The document of [features/sketch.md](features/sketch.md) test 22 (corpus R33): f-sk2's placement origin is (0,0,40), where the top was just before 'Extrude 1' removed it, which the build reads from b-1's writer chain `[(f-box1, empty), (f-ext3, the box f-box1 left)]`. After adding body b-2 and inserting `{"id": "f-box9", "type": "box", "body": "b-2", "size": ["1", "1", "1"], "op": "new"}` before f-sk2, the next run on the same `Recompute` reuses f-sk2 (`from_cache` true, the same key and placement). After changing f-box1's size to `["20", "20", "38"]` instead, f-sk2 is built again (`from_cache` false), its placement origin is (0,0,38), and its warning is still `plane_fallback` placed before removal by f-ext3.
43. Memoized reasons follow the records they read (section 3.10). Corpus R24 (a) on a fresh `Recompute`: f-fil1 `Error` (`references_not_found`), reason `Suppressed { feature: f-box2 }`, text ending `belongs to 'Box 2', which is suppressed.`, fixes holding `Unsuppress { feature: f-box2 }` and ending with `Pick { pointer: "/edges/0" }`; b-1 volume 8000. The same document again on the same `Recompute`: every active feature `from_cache` true and `messages` equal to the first run's. Then corpus R24 (b) on the same `Recompute`: f-fil1 has the key it had in (a) and is reused (`from_cache` true), yet its reason is `ComesLater { feature: f-box2 }`, its text ends `is made by 'Box 2', which now comes after this fillet.` and its fixes hold `Move { feature: f-fil1, place: Below(f-box2) }`, because 'Box 2' is no longer suppressed and no longer before the fillet; b-1 volume 7600. (b) on a fresh `Recompute` gives equal messages.
44. Names only (section 3.10). Corpus R16 on a fresh `Recompute`: after the run, the state of b-1 in f-ext2's `bodies_before` (the cube f-box1 left), whose naming map the diagnosis walked, has `measurements().elements == false`, while the state of b-1 in f-fil1's `bodies_before`, which f-fil1's resolve read, has `elements == true`. A second run on the same `Recompute`, every result reused, leaves the first still false.
45. `NotInDocument` (corpus R23 (b)). f-fil1 `Error` (`references_not_found`), reason `NotInDocument { feature: f-ext1 }`, text `An edge of a feature that is not in this document selected for Edges is gone.`; no `Unsuppress`, `Move` or `Show` among the fixes, and `Pick` last; peg B's top rim stays sharp.
46. One root for a chain (corpus R54). f-dp1 `Error` (`reference_not_found`), reason `NotInDocument { feature: f-ext2 }`, text `A face of a feature that is not in this document selected for Base plane is gone.`, root None. f-sk3 `Error` (`upstream_failed`), text `'Datum plane 1' has an error, so this sketch cannot use it. Fix 'Datum plane 1' first.`, fixes `[Show { feature: f-dp1, entity: None }]`, root f-dp1. f-ext3 `Error` (`upstream_failed`), text `'Sketch 2' cannot be built because 'Datum plane 1' has an error, so this extrude cannot use it either. Fix 'Datum plane 1' first.`, fixes `[Show { feature: f-dp1, entity: None }]`, root f-dp1. b-1 volume 120000.
47. `Suppressed` (corpus R25). While f-cha1 is suppressed: f-dp1 `Error` (`reference_not_found`), reason `Suppressed { feature: f-cha1 }`, text ending ` selected for Base plane belongs to 'Chamfer 1', which is suppressed.`, fixes holding `Unsuppress { feature: f-cha1 }` before the last, `Pick { pointer: "/base/face" }`; f-sk1 and f-ext1 `Error` (`upstream_failed`) with root f-dp1. After unsuppressing f-cha1 on the same `Recompute`: every status `Ok` with no messages, and the document's canonical text equals its text before the suppress.
48. `ComesLater` and the drag back (corpus R28). With f-fil1 dragged directly after f-ext1: f-fil1 `Error` (`references_not_found`), reason `ComesLater { feature: f-ext2 }`, text ending `is made by 'Extrude 2', which now comes after this fillet.`, first fix `Move { feature: f-fil1, place: Below(f-ext2) }` (no candidate keeps a face of the lost edge, so the timeline fix comes first, features.md section 4.4), last fix `Pick`; b-1 volume 123141.592654. The variant reference (the boss's base ring) gets the same reason. After dragging f-fil1 back, f-fil1 is `Ok` and the canonical text equals the original.
49. `RolledBack`. The dragged document of test 48 with `"rollback": "f-dp1"`: f-dp1, f-sk2 and f-ext2 `RolledBack`; f-fil1 reason `RolledBack { feature: f-ext2 }`, text ending `is made by 'Extrude 2', which is after the rollback marker.`, and no `Move` among its fixes. Without the marker and with `stop_at` f-fil1: reason `ComesLater { feature: f-ext2 }`, because a stop point is not the marker.
50. `UpstreamFailed` (corpus R47). f-fil1 `Error` (`references_not_found`), reason `UpstreamFailed { feature: f-ext3, root: f-sk3 }`, text ending `is missing because 'Extrude 3' cannot be built while 'Sketch 3' has an error.`, fixes holding `Show { feature: f-sk3, entity: None }` and no `Pick`, root f-sk3; f-ext3 `Error` (`upstream_failed`) with root f-sk3; b-1 volume 110575.222039. After removing `c-2`, f-fil1 is `Ok` with the original result. With `c-2` removed and f-ext3's distance set to `"0"` instead (its own `out_of_range` error): reason `UpstreamFailed { feature: f-ext3, root: f-ext3 }`, text ending `is missing because 'Extrude 3' failed.`, root f-ext3.
51. `OtherBody` (corpus R31 and R56). R31 (a): f-cha1 `Error`, reason `OtherBody { body: b-2, by: Some(f-split1), by_suppressed: false }`, text ending `is now in body 'Body 2', put there by 'Split body 1'.`, fixes holding `UseBody { field: body, body: b-2 }`; b-1 volume 6000. R31 (b): f-fil1 the same reason and `UseBody` fix; applying it (f-fil1's `body` set to b-2) gives f-fil1 `Ok` with its fillet face in b-2; b-1 volume 4000. R56 (a): f-fil1 reason `OtherBody { body: b-2, by: Some(f-bool1), by_suppressed: true }`, text ending `is now in body 'Body 2' while 'Boolean 1', which joins it to this body, is suppressed.`, fixes holding `Unsuppress { feature: f-bool1 }` and then `UseBody { field: body, body: b-2 }`; b-1 volume 8000. R56 (c): f-fil1 reason `OtherBody { body: b-2, by: Some(f-ext1), by_suppressed: false }` (rule 8: 'Extrude 1' now writes only 'Body 2'), text ending `is now in body 'Body 2', put there by 'Extrude 1'.`, and no `UseBody`, because the ring's other face is not in 'Body 2'.
52. `CutAway` (corpus R16 and R30 (b)). R16: f-fil1 `Error`, reason `Removed { by: f-ext2, how: CutAway }`, text `The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet.`; the first two fixes are `UseReferences` of the rabbet's two outer edges (y=-5 at z=20 and y=-10 at z=15, each 5 mm from where the edge was, `meaning` true), the last is `Pick`; b-1 volume 7500. R30 (b): f-fil1 reason `Removed { by: f-bool1, how: CutAway }`, text ending `was cut away by 'Boolean 1', which comes before this fillet.`: the boolean is the last writer of 'Body 1' whose input (its tool body 'Body 2') has the face, so rule 7 names it, and rule 8 never blames 'Box 2', which writes only 'Body 2'; b-1 volume 7450.
53. `Covered` (corpus R13 and R21). R13: f-fil1 `Error`, reason `Removed { by: f-ext2, how: Covered }`, text ending `is now inside the part, covered by 'Extrude 2'.` R21 (a) and (b): f-fil1 `Error`, reason `Removed { by: f-box2, how: Covered }`, text ending `is now inside the part, covered by 'Box 2'.`; in (b) the first fix is `UseReferences` of the inside corner `edge(f-box1/top|f-box2/left)` (`meaning` true, distance 0), never applied; b-1 volumes 8000 and 10000.
54. `Rounded` and `Beveled` (corpus R18 and R59). R18 (a): f-cha1 `Error`, reason `Removed { by: f-fil1, how: Rounded }`, text ending `was rounded by 'Fillet 1', which comes before this chamfer.` R18 (b): f-fil1 `Error`, reason `Removed { by: f-cha1, how: Beveled }`, text ending `was beveled by 'Chamfer 1', which comes before this fillet.`; b-1 volume 7960. R59 (a): f-rev1 `Error` (`reference_not_found`), reason `Removed { by: f-fil1, how: Rounded }`, text ending `was rounded by 'Fillet 1', which comes before this revolve.`, and the fillet's two straight tangent edges among its `UseReferences` fixes.
55. `Hollowed`. CUBE20; `f-dp1` datum_plane on the picked face `f-box1/top`, offset 0; inserted before it, `{"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "2", "remove_faces": [...]}` with the picked `f-box1/top`, `f-box1/front`, `f-box1/back`, `f-box1/left` and `f-box1/right`. Expected: b-1 is the 2 mm bottom plate (volume 800), so nothing is left in the top's plane and no rim continues the top; f-dp1 `Error` (`reference_not_found`), reason `Removed { by: f-sh1, how: Hollowed }`, text `The top of 'Box 1' selected for Base plane was removed by 'Shell 1', which hollows the part.`
56. `PieceGone` (corpus R10 (a)). f-fil1 `Error`, reason `Removed { by: f-ext2, how: PieceGone }`, text ending `no longer exists: that part of the top of 'Extrude 1' was removed by 'Extrude 2'.`; b-1 volume 112500; the left part's front top edge stays sharp.
57. `NoLongerMade` (corpus R12 and R57). R12 at distances 30 and 40, in both variants: f-fil1 `Error`, reason `Removed { by: f-ext2, how: NoLongerMade { absence: Some(Through) } }`, text ending `is no longer made by 'Extrude 2', which goes all the way through the body.`; the rims are offered as `UseReferences`, never applied; b-1 volume 110575.222039 (square variant 108000). R57 at a full turn: f-cha1 `Error`, reason `Removed { by: f-rev1, how: NoLongerMade { absence: Some(FullTurn) } }`, text ending `is no longer made by 'Revolve 1', which turns all the way round.`; f-sk2's `plane_fallback` has the same reason and renders as [features/sketch.md](features/sketch.md) test 29 states; with 270 deg again, every feature `Ok`. A face its feature no longer makes: CUBE20, `f-cha1` chamfer equal 2 on the picked `edge(f-box1/front|f-box1/top)`, `f-dp1` datum_plane on the picked face `f-cha1/chamfer/edge(f-box1/front|f-box1/top)`, offset 0; edit: f-cha1's `edges` become the picked `edge(f-box1/back|f-box1/top)`. Expected: f-dp1 `Error` (`reference_not_found`), reason `Removed { by: f-cha1, how: NoLongerMade { absence: None } }`, text `The beveled face of 'Chamfer 1' selected for Base plane is no longer made by 'Chamfer 1'.`
58. `NoLongerMeet` (corpus R01 and R64). BLOCK and `f-cha1` chamfer equal 1 on the picked front right vertical edge `edge(f-ext1/lateral/e-1|f-ext1/lateral/e-2)`, after the 5 mm edit of corpus R01 (line `e-6` from (75,0) to (80,5)): f-cha1 `Error`, reason `Removed { by: f-ext1, how: NoLongerMeet { between: Some(f-ext1/lateral/e-6) } }` (the caps also touch both sides, but not each other, so they are not `between`), text `The front right edge of 'Extrude 1' selected for Edges no longer exists, because its faces no longer meet: the side of 'Extrude 1' made from the line from (75, 0) to (80, 5) in 'Sketch 1' now lies between them.` R64: f-fil1 `Error`, reason `Removed { by: f-sh1, how: NoLongerMeet { between: Some(f-sh1/inner/f-ext1/lateral/e-5) } }`, text ending `is already round: 'Sketch 1' now rounds that corner.`; no fillet face.
59. `EntityDeleted` (corpus R03 and R51). R03: f-fil1 `Warning` (`reference_missing`, pointer `/edges/0`), reason `EntityDeleted { feature: f-ext1, sketch: f-sk1, entity: e-5 }`, text ending `is gone because that line was deleted from 'Sketch 1', so it was skipped.`, fixes ending with `RemoveEntry { pointer: "/edges/0" }` and `Pick { pointer: "/edges/0" }` and holding no fix of the reason; b-1 volume 119931.327412. R51: f-fil1 `Error`, reason `EntityDeleted { feature: f-ext1, sketch: f-sk1, entity: e-6 }`, text ending `is gone because that curve was deleted from 'Sketch 1'.`
60. `PointGone` (corpus R43). (a): f-cha1 `Error`, reason `PointGone { feature: f-hole1, sketch: f-sk1, point: "e-p2", deleted: false }`, text ending `is gone because 'Hole 1' no longer makes the hole at (7, 0): that point was removed from its Points.` (b): f-hole1 `Warning` (`point_missing`); f-cha1 `Error`, reason `PointGone { ..., deleted: true }`, text ending `is gone because 'Hole 1' no longer makes a hole at that point, which was deleted from 'Sketch 1'.` b-1 volume 15717.256661 in both.
61. `CopyGone` (corpus R41). f-fil1 `Warning` (`reference_missing`, pointer `/edges/1`), reason `CopyGone { feature: f-lp1, instance: Instance { first: 2, second: 0 } }`, text ending `is gone because 'Linear pattern 1' no longer makes its third copy, so it was skipped.`; b-1 volume 1997.853982. The variant with only the second reference: `Error` (`references_not_found`), text ending `is gone because 'Linear pattern 1' no longer makes its third copy.`, b-1 volume 2000.
62. `NotInFile` (corpus R46 (a)), with the corpus's assets in `Input::assets`: f-cha1 `Error`, reason `NotInFile { feature: f-imp1 }`, text ending `is not in the new version of the file imported by 'Import 1'.`; b-1 volume 4000.
63. `Unknown`. CUBE20; `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "5"}`; `{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{"q": "edge(f-box1/top|f-dp1/plane)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}], "radius": "1"}` (written by hand: a datum makes no faces). Expected: f-fil1 `Error` (`references_not_found`), reason `Unknown`, text ending `can no longer be found, and Partsmith could not tell why.`
64. Follows (corpus R19, R61 (a), R63 (b) and R66). R19: f-fil1 `Ok` with the notice `reference_followed`, reason `Followed { by: f-sh1 }`, text ending `was removed by 'Shell 1', which left its rim in the same place, so the rim is used.` R61 (a) with spacing 10: notice, reason `Followed { by: f-lp1 }`, text ending `merged with neighboring parts when 'Linear pattern 1' joined them, so all of it is used.` R63 (b): notice, reason `Followed { by: f-hole1 }`, text containing ` changed when 'Hole 1' changed, so `. R66: f-fil1 `Warning` (`reference_widened`, pointer `/edges/0`), reason `Followed { by: f-sk1 }`, text `The back top edge of 'Extrude 1' selected for Edges now covers 5 edges, because the line it was made from was replaced by 5 lines in 'Sketch 1'. Keep all of them, or use only one.`
65. Empty bodies (section 3.10, `because`). NC11: f-ext1 `Error` (`profile_not_found`); f-fil1 `Error` (`body_empty`), text `Body 'Body 1' is empty here because 'Extrude 1', which makes it, failed. Fix 'Extrude 1' first.`, fixes `[Show { feature: f-ext1, entity: None }]`, root f-ext1. BLOCK with f-ext1 suppressed and a fillet `f-fil1` of radius 1 on the front top edge: text `Body 'Body 1' is empty here because 'Extrude 1', which makes it, is suppressed.`, fixes `[Unsuppress { feature: f-ext1 }]`, root None. The same fillet moved above f-ext1: text `Body 'Body 1' is empty here because 'Extrude 1', which makes it, now comes after this fillet.`, fixes `[Move { feature: f-fil1, place: Below(f-ext1) }]`. Corpus R56 (b): text `Body 'Body 1' is empty here because 'Boolean 1' uses it as a tool, which empties it.`, fixes `[UseBody { field: body, body: b-2 }]`. Bodies `[b-1]` with `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new", "suppressed": true}`, `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["5", "5", "5"], "op": "join"}` and a fillet `f-fil1` on b-1: f-box2 and f-fil1 `Error` (`body_empty`), both with the text `Body 'Body 1' is empty here because 'Box 1', which makes it, is suppressed.`, since the walk passes over f-box2, which failed only because the body was empty. Bodies `[b-1, b-2]`, CUBE20 in b-1, `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["5", "5", "5"], "op": "new", "suppressed": true}` and `{"id": "f-bool1", "type": "boolean", "target": "b-1", "tools": ["b-2"], "op": "cut"}`: f-bool1 `Error` (`tool_empty`), text `Body 'Body 2' is empty here because 'Box 2', which makes it, is suppressed, so it cannot be used as a tool.`, first fix `Unsuppress { feature: f-box2 }`. A fillet on a body no feature writes: `Body 'Body 2' has no solid at this point, so this fillet has nothing to work on. Pick a body that has a solid.`
66. Lost faces (corpus R26 and R33, `lost_face`). The document of [features/sketch.md](features/sketch.md) test 23: f-ext2 `Warning` (`no_effect`), text `This extrude adds nothing to body 'Body 1' because 'Sketch 2' lost the face it was on. Fix 'Sketch 2' first.`, fixes `[Show { feature: f-sk2, entity: None }]`, root f-sk2; f-sk2's `plane_fallback` has the reason `NotInDocument { feature: f-ext1 }`. The document of [features/sketch.md](features/sketch.md) test 22: f-ext2 `Warning` (`separate_solid`) with its own text and root f-sk2; f-sk2's reason is `Removed { by: f-ext3, how: CutAway }`, its text ending `was cut away by 'Extrude 1', which comes before this sketch, so the sketch stays where that face was.`
67. Ambiguous losses (corpus R63 (a) and R46 (b)). R63 (a): f-cha1 `Error` (`references_not_found`), text ending ` now matches 2 edges after 'Hole 1' changed. Pick the one you mean.`, reason None, and the new mouth `edge(f-box1/top|f-hole1/counterbore/e-p2)` first among its `UseReferences` fixes. R46 (b): text ending ` now matches 2 edges. Pick the one you mean.`, reason None.
68. Every loss is kept (section 2, `Message::repairs`). CUBE20 and f-fil1 with the two references of features.md section 6.5 test 4: f-fil1 `Error` (`references_not_found`) whose message has `repairs` holding two `Repair::Element`, at `/edges/0` and `/edges/1`, reason `NotInDocument { feature: f-gone }` (its first loss), and the fixes of both losses in loss order, each loss's `Pick` after its own candidates.
69. The reference stability corpus ([reference-stability-corpus.md](../design/reference-stability-corpus.md), "Where the assertions run"). `reference_stability_1.rs` (R01 to R22), `reference_stability_2.rs` (R23 to R44) and `reference_stability_3.rs` (R45 to R66) run every scenario, one test per variant and size, with each setup's references picked as the corpus's "Picked references" rule says, and check what the document alone determines: geometry, statuses, codes, pointers, `Message::text` (the structural message, which says "a feature that is not in this document" where a scenario says "deleted"), `Message::fixes`, `root` and repair data. The cause lines are checked by the engine's twins (engine.md). The three files are behind the crate feature `stability-study` (`cargo test -p partsmith-recompute --features stability-study --test reference_stability_1`, and `_2`, `_3`) until every scenario passes; then the feature is removed and they run in the normal suite.
70. The newcomer error corpus ([newcomer-error-corpus.md](../design/newcomer-error-corpus.md)). `newcomer_errors.rs` runs NC01 to NC12 from the document alone and checks every code, `Message::text`, `Message::fixes` (the scenario's bracketed actions in their order, as `Fix` values; the labels are engine.md's) and `root`. NC01's outline finding and its `ConnectEnds` fix come from the sketch's result (sketch.md section 4.3), and its Extrude disabled reason is the UI's; NC08 checks the sketch's error without its cause line, and NC11 its two errors without the notification and cause line, which are the engine's; NC09 runs the document as `Document::units_edits` (model.md) leaves it with and without Keep sizes and checks that the volume is unchanged with it.
71. Explanations re-render with current names (sections 3.6 and 3.10). Corpus R16 as in test 52 on a fresh `Recompute`, then the same document with f-ext2 named `Rabbet`: every feature is reused (`from_cache` true), f-fil1's reason is still `Removed { by: f-ext2, how: CutAway }`, and its text ends `was cut away by 'Rabbet', which comes before this fillet.`

## Open issues

1. **`NotRun` status.** Protocol's feature states are ok, warning, error, suppressed and rolled back. Cancellation adds `NotRun`; engine.md lists cancellation as later, so the engine never sees it in M1, but the protocol will need it when cancellation ships.
2. **Failed writers stay providers.** A feature that fails still counts as the provider of the bodies it declares, so downstream keys depend on it even though it changed nothing. This keeps keys a pure function of the timeline at the cost of rebuilding downstream features when a failing feature's params change.
3. **Test 30** depends on wall-clock waits (50 ms and 100 ms against a 10 s ceiling). The margins are wide, but CI on slow runners may need them scaled.
4. **Cold run of `bench50` over budget (needs a budget decision).** `run` of `bench50` from an empty cache (section 5, 1 s) takes 2.10 s, against 2.26 s before October 13 (release builds on the 4-core reference container at load 1 to 3, medians of three alternating benchmark runs before and after, each the median of 7 runs); the edit of the last 5 features went from 34 to 30 ms. Earlier measurements: 2.56 s once body updates carried their measurements (features.md open issue 6), 2.96 s before that, 4.37 s before the kernel checked only what an operation changed (kernel.md section 3.5). This issue once said nearly all of the time was OCCT's booleans and blends on the large merged bodies (b-1 ends with 426 faces). A callgrind profile of eight cold runs showed otherwise: about half was the kernel's and the features' own measuring and checking, 22% the whole-body volume (`BRepGProp` after each boolean, and the volume of bodies whose feature carried none), 13% fingerprints (12 672 of 56 360 face queries fell back to `BRepGProp` surface integration, mostly faces an operation kept) and 13% the validity check, while OCCT's boolean core with the unification of coplanar faces was about 27% and its fillets and chamfers 14.5%, which puts OCCT's own floor near 1 s. The kernel now integrates volumes exactly on planes and cylinders (kernel.md section 3.6 item 1; a body's volume query went from 1.5 to 0.05 ms), stores the curves of edges on planes before checking (section 3.5 item 5) and trusts a sweep's caps (item 4); booleans now run on copies of their inputs (section 3.1 item 3), which costs about 5% of the run and was needed for correctness. What remains of Partsmith's share is mostly the fingerprints of faces an operation kept unchanged, which the naming bridge measures again (`topology_info` reads every face of the result): reusing the kept faces' fingerprints needs the kernel's history to say which result elements are an input's own unchanged elements, a kernel and features API change left for a later pass, and faces whose boundary curves are not lines or arcs fall back to numeric integration. With OCCT near 1 s on its own, the budget needs a decision against these measurements. The other budgets of section 5 held on October 12: `plan` of 300 features 3.2 ms (10 ms), overhead per feature 0.006 ms cold and 0.003 ms cached (0.1 ms), `bench50` cached 0.39 ms (5 ms), the edit of the last 5 features 36 ms (60 ms), and cancel latency 0.8 to 1.1 ms (5 ms).
5. **Every repair on the message (needs review with ADR 0024).** The reference stability study (section 3.6, RC3) gives `Message` one `repair: Option<Repair>`. features.md section 2.8 gives an error one repair per loss (`repairs`), and ADR 0021 keeps every loss, so `Message::repairs` holds them all, and engine.md section 3.4 should project each into `FeatureStatus::repairs` (the study's E1). `Message::reason` stays one value, the reason of the loss the text names; the reasons of a message's other losses reach hosts only through their fixes.
6. **Diagnosis rules sharpened from the study (needs review with ADR 0021).** Section 3.10 follows the study's RC4 with four changes, each to keep a statement true. The study's second `OtherBody` case (the feature that made the element writes other bodies but not this one) comes after the remover walk (rule 8), because a tool body joined by a boolean is written by a feature that writes only the tool body, and the boolean that later cut its face away is the remover (corpus R30 (b)). `W` being the feature that made the element is checked before the change of `W`'s update, because a cut that no longer makes its floor has change `Cut` (corpus R12, R30 (a)), and a reason from a fillet or chamfer that made no face from the query falls back to `Rounded` or `Beveled` instead of `Unknown`. The remover is the last writer that took the element away rather than the first, which differs only when a later writer made it again. `RolledBack` is checked before `ComesLater`, which every rolled-back feature also satisfies, and only for the document's rollback marker, so a preview's stop point never reads "after the rollback marker".
7. **Why a body is empty (needs review).** The study names "the last record before the feature that writes the body and failed, is suppressed, or emptied it". With a failed new body followed by a suppressed join, that names the join, whose unsuppressing changes nothing. Section 3.10 walks the active writers first, passes over writers that failed only because the body was already empty, and names a suppressed writer only when no active writer explains the emptiness. `root` is also set for an empty body whose maker failed, so the Problems panel groups it under that failure (the error guidelines, "One root cause, one message").
8. **`UpstreamFailed` text.** The study writes `is missing because '{feature}' failed ('{root}' has an error)`, a sentence in parentheses, which the error guidelines forbid. Section 3.10 writes `is missing because '{feature}' cannot be built while '{root}' has an error`, in the words features.md's `upstream_failed` uses.
9. **"Goes all the way through" (needs review with features/extrude.md).** `Through` is recorded for every cut whose end face is not in the result (features/extrude.md section 3.3 step 6), and renders `which goes all the way through the body`. That is true for corpus R12, but a symmetric cut that only exits the top (corpus R30 (a): a pocket from z 10 to the top at z 30) does not go all the way through. A wording true in both cases, for review: `which now ends outside the body`.
10. **Ambiguous losses name the change by meaning.** The study gives `{after}` (` after '{feature}' changed`) without saying how the diagnosis finds the feature. Section 3.10 names it only when the ambiguity comes from relatives (a hole whose type changed, corpus R63 (a)), the one case where names alone show that the feature now makes something else; an imported file's fallback names say nothing (corpus R46 (b)).
11. **Reasons in hosts.** `LossReason` is not serialized here (section 2). cli.md section 3.3 prints `reason` in its JSON output, and protocol.md's `Diagnostic` (the study's PR1) has no field for it yet; the engine's projection needs one (engine.md section 3.4) before the CLI can print it.
12. **`Hollowed` is rare.** A shell leaves a rim in the plane of every removed face that has material around it, and the resolver follows the rim as a continuation (naming.md section 3.9 step 3), so a loss is explained as `Hollowed` only when nothing is left in the face's plane (test 55: a shell that removes the top and every side).
