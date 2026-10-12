# partsmith-recompute

Runs a document's timeline: validation, dependency graph, parameter and expression evaluation, content-hash cache keys, the shared result cache with LRU eviction, rollback and stop points, failure, panic and cancellation isolation, and per-feature reports.

Spec (the definition of done): `docs/spec/recompute.md`. Acceptance tests in `tests/` come from recompute.md section 6, features.md section 6.4 (`tests/body_semantics.rs`) and naming.md tests 81 to 90 (`tests/naming_conformance.rs`); the per-feature document tests `tests/feature_<type>.rs` belong to the built-in feature pipelines. Do not weaken them.

## Public interface (crate root)

- `Recompute` (`new`, `with_cache_limit`, `registry`, `graph`, `plan`, `run`, `cache_len`, `clear_cache`), `DEFAULT_CACHE_ENTRIES`, and `Input { document, context, stop_at, assets }`. `Recompute` is `Send + Sync`; `graph` and `plan` take `&self` and never change the result cache.
- Graph: `Graph` (`nodes`, `node`, `dependents`, `affected_by`, `independent`), `Node`, `NodeState`, `Edge`, `EdgeKind`, `Change`.
- Plan: `Plan`, `PlanStep`, `Action`.
- Keys: `CacheKey` (32 bytes, `Display` is 64 lowercase hex characters).
- Report: `Report`, `FeatureReport`, `Status`, `Timing`, `BodyReport`, `ParameterReport`, `Message`, `Severity`.
- Re-exported from `partsmith-features`: `CancelToken`, `FeatureOutput`, `BodyState`, `FeatureError`, `FeatureWarning`, `Repair`.

## Internals

- `validation.rs`: `Validations` memoizes `validate_and_canonicalize` plus `dependencies`, the compact key params and the names expressions use, per feature id and record content (type and params in stored order). Pruned to the current document once it holds more entries than the cache limit.
- `timeline.rs`: `Timeline::prepare` gives every record its state (stop point, then rollback, then suppression, then validity), computes providers, edges, parameters and context names, and builds the `Graph`. `check_links` holds the pre-build checks.
- `evaluate.rs`: values and the key of one active step, from the keys of earlier steps. `key.rs`: the key preimage (`KeyInput`) and the compact canonical params text.
- `plan.rs` and `run.rs` walk the steps in order with the same key computation and checks; `cache.rs` holds outcomes (`Built` output with its body states, or the error) and evicts by last-use run, then key.
- `test_support.rs` (tests only): small test features (`test_datum`, `test_writer`, `test_reader`, `test_asset`, `test_panic`) and document helpers.

## Commands

```sh
export OCCT_ROOT=/opt/occt/7.9.3/native
cargo test -p partsmith-recompute
cargo clippy -p partsmith-recompute --all-targets -- -D warnings
cargo fmt -p partsmith-recompute
# tests that need the built-in features, once they are registered
cargo test -p partsmith-recompute -- --include-ignored
# timing budgets (recompute.md section 5)
cargo test -p partsmith-recompute --release --test performance -- --ignored
```

Tests that need built-in feature types are marked `#[ignore = "needs built-in <types>"]` until the feature pipelines register them.
