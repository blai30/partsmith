# partsmith-recompute

Runs a document's timeline: validation, dependency graph, parameter and expression evaluation, content-hash cache keys, the shared result cache with LRU eviction, rollback and stop points, failure, panic and cancellation isolation, and per-feature reports.

Spec (the definition of done): `docs/spec/recompute.md`. Acceptance tests in `tests/` come from recompute.md section 6, features.md section 6.4 (`tests/body_semantics.rs`) and naming.md tests 81 to 90 (`tests/naming_conformance.rs`); test 37 (the section 5 budgets) is the benchmark `benches/budgets.rs`; tests 41 to 71 (the statuses and the loss diagnosis) are `tests/diagnosis.rs`, as far as phase 1 of the reference stability plan builds them (its module documentation lists the tests and parts later phases add), and the writer chain tests of section 3.5 are in `tests/timeline.rs`. The per-feature document tests `tests/feature_<type>.rs` and their helpers `tests/feature_support/mod.rs` belong to the built-in feature pipelines (features.md section 6.1). Do not weaken them.

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
- `plan.rs` and `run.rs` walk the steps in order with the same key computation and checks; `cache.rs` holds outcomes (`Built` output with its body states, made with `BodyState::from_update` so they keep the topology and volume the feature measured, or the error) and evicts by last-use run, then key.
- `pass_lru.rs`: `PassLru`, the map behind the cache and the validation memo. It records the pass (run, plan or graph) that last used each entry and, beyond a limit, evicts the entries used longest ago, the smaller key first on ties, never one the current pass used.
- `test_support.rs` (tests only): small test features (`test_datum`, `test_writer`, `test_reader`, `test_asset`, `test_panic`), their records (`datum`, `datum_on`, `writer`) and document helpers (`document`, `prepared`, `prepared_until`, `id`); timelines are prepared over one shared registry.
- `tests/common` holds the `Harness`, the spec's test features (with `test_history`, which checks the writer chain `BuildContext::body_history` gives it and places its datum at the sum of the body's volumes before each writer) and the plan and graph readers, and re-exports the tolerances, measurement helpers and report readers of `tests/feature_support` (`feature_report`, `body_report`, `final_state`, `state_before`; `Run` methods use the same readers), so every test measures and reads reports the same way. `benches/budgets.rs` includes it with `#[path]`.
- `tests/diagnosis.rs` runs corpus scenarios through `tests/stability_common` (record builders, the STEP plates of the import scenarios, and the picks below), which it includes with `#[path]`; that module is shared with the corpus and is not behind `stability-study`. Picks follow the corpus's "Picked references" rule: `run_picked` runs a setup, re-picks every reference (`Reference::from_element`), region (`Region::reference()`), face sketch plane (`fallback_frame: body`) and edge or face axis (`direction`) in the state the setup builds, and runs the picked document; `with_picks` carries those picks into an edited document wherever the edit kept the stored query or region key; `stored` reads the pick a record stores for a query.
- Feature `stability-study`: the reference stability corpus (docs/design/reference-stability-corpus.md, R01 to R66) as `tests/reference_stability_{1,2,3}.rs`, with their helpers in `tests/stability_support_{1,2,3}/mod.rs` (directories, so Cargo builds no test target of their own); the record builders, picks, readers and checks they share live once in `tests/stability_common/mod.rs`, which all three include with `#[path]` and re-export (the first third keeps its own `Expectations` and `EXACT_POSITION_TOLERANCE`, the corpus's 1e-6 mm). Every setup runs picked (`run_picked`, through each third's `setup` or `run`), every edit keeps the picks (`with_picks`, through `edit`, `edit_document` or `run_edited`), and checks that resolve a reference resolve its stored pick. `Cargo.toml` gives the three tests `required-features`, so default runs build none of them; they record the study's expectations and fail until its plan is implemented, so CI only compiles and lints them.

## Commands

```sh
export OCCT_ROOT=/opt/occt/7.9.3/native
cargo test -p partsmith-recompute
cargo clippy -p partsmith-recompute --all-targets -- -D warnings
cargo fmt -p partsmith-recompute
# timing budgets (recompute.md section 5, test 37), all or by words of their names
cargo bench -p partsmith-recompute --bench budgets
cargo bench -p partsmith-recompute --bench budgets -- plan cancel
# the reference stability corpus (fails by design until the study's plan is implemented)
cargo clippy -p partsmith-recompute --features stability-study --all-targets -- -D warnings
cargo test -p partsmith-recompute --features stability-study --test reference_stability_1
```

Every test runs by default against `Registry::with_builtins()` (all 20 types are registered). The only ignored tests are the release-only feature budgets in `tests/feature_<type>.rs`.
