# partsmith-recompute

Runs a document's timeline: validation, dependency graph, parameter and expression evaluation, content-hash cache keys, the shared result cache with LRU eviction, rollback and stop points, failure, panic and cancellation isolation, and per-feature reports.

Spec (the definition of done): `docs/spec/recompute.md`. Acceptance tests in `tests/` come from recompute.md section 6, features.md section 6.4 (`tests/body_semantics.rs`) and naming.md tests 81 to 90 (`tests/naming_conformance.rs`); test 37 (the section 5 budgets) is the benchmark `benches/budgets.rs`. The per-feature document tests `tests/feature_<type>.rs` and their helpers `tests/feature_support/mod.rs` belong to the built-in feature pipelines (features.md section 6.1). Do not weaken them.

## Public interface (crate root)

- `Recompute` (`new`, `with_cache_limit`, `registry`, `graph`, `plan`, `run`, `cache_len`, `clear_cache`), `DEFAULT_CACHE_ENTRIES`, and `Input { document, context, stop_at, assets }`. `Recompute` is `Send + Sync`; `graph` and `plan` take `&self` and never change the result cache.
- Graph: `Graph` (`nodes`, `node`, `dependents`, `affected_by`, `independent`), `Node`, `NodeState`, `Edge`, `EdgeKind`, `Change`.
- Plan: `Plan`, `PlanStep`, `Action`.
- Keys: `CacheKey` (32 bytes, `Display` is 64 lowercase hex characters).
- Report: `Report`, `FeatureReport`, `Status`, `Timing`, `BodyReport`, `ParameterReport`, `Message`, `Severity`.
- Re-exported from `partsmith-features`: `CancelToken`, `FeatureOutput`, `BodyState`, `FeatureError`, `FeatureWarning`, `Repair`.

## Internals

- `validation.rs`: `Validations` memoizes `validate_and_canonicalize` plus `dependencies`, the compact key params and the names expressions use, per record content (the BLAKE3 hash of id, type and params in stored order), so documents that share ids (`SequentialIds`) keep their own outcomes. Each `graph`, `plan` or `run` is one pass; after it the memo is pruned to the cache limit by last use, never dropping what the pass used.
- `timeline.rs`: `Timeline::prepare` gives every record its state (stop point, then rollback, then suppression, then validity), computes providers, edges, parameters and context names, and builds the `Graph`. `check_links` holds the pre-build checks. A stop point that names no record falls back to the rollback marker (section 3.1 step 3); its `unknown_stop` text then says the run stopped at the marker, because section 4's text ("the whole timeline was rebuilt") would be false.
- `graph.rs`: inactive nodes provide no bodies, so `affected_by` of a feature change is queried on the graph in which that feature is active (after unsuppressing or adding it, before suppressing or removing it).
- `evaluate.rs`: values and the key of one active step, from the keys of earlier steps. `key.rs`: the key preimage (`KeyInput`) and the compact canonical params text.
- `plan.rs` and `run.rs` walk the steps in order with the same key computation and checks; `cache.rs` holds outcomes (`Built` output with its body states, or the error) and evicts by last-use run, then key.
- `test_support.rs` (tests only): small test features (`test_datum`, `test_writer`, `test_reader`, `test_asset`, `test_panic`) and document helpers.
- `tests/common` holds the `Harness`, the spec's test features and report readers, and re-exports the tolerances and measurement helpers of `tests/feature_support`, so every test measures the same way. `benches/budgets.rs` includes it with `#[path]`.

## Commands

```sh
export OCCT_ROOT=/opt/occt/7.9.3/native
cargo test -p partsmith-recompute
cargo clippy -p partsmith-recompute --all-targets -- -D warnings
cargo fmt -p partsmith-recompute
# timing budgets (recompute.md section 5, test 37), all or by words of their names
cargo bench -p partsmith-recompute --bench budgets
cargo bench -p partsmith-recompute --bench budgets -- plan cancel
```

Every test runs by default against `Registry::with_builtins()` (all 20 types are registered). The only ignored tests are the release-only feature budgets in `tests/feature_<type>.rs`.
