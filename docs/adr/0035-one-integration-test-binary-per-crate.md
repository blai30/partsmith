# 0035: One integration test binary per crate

Status: Accepted

## Context

Cargo builds every `.rs` file directly in a crate's `tests/` as a test binary of its own. The workspace had 177 such targets (174 without the `stability-study` feature of `partsmith-recompute`), and 105 of them linked OCCT statically, 42 to 112 MB each without debug info. Together they made 7.5 GB of integration test executables in `target/debug` (7.2 GB the 105 that linked OCCT), with one link per file. With Cargo's default debug info they outgrew the free space of the CI runner (delivery.md 3.2, open issue 9), and a local `target/debug` grew to 15 GB.

## Decision

Each crate with integration tests builds them as one test binary named `integration`:

- `Cargo.toml` sets `autotests = false` in `[package]`, with a comment naming this ADR, and declares `[[test]] name = "integration"` (path inferred).
- `tests/integration.rs` declares every shared helper directory of `tests/` (`common`, and where a crate has them `corpus`, `oracle`, `feature_support` and `stability_common`) and then every other `.rs` file directly in `tests/` as a module, one `mod <file stem>;` each. It holds nothing else; its `//!` documentation says what it is.
- The files stay where they are, so every spec, AGENTS.md and comment that names a test file path stays true. A former file's `mod common;` (or `mod corpus;`, `mod oracle;`, `mod feature_support;`) becomes `use crate::common;`; test bodies and assertions do not change. Shared modules reach each other through `super::`, which a bench that includes one with `#[path]` reads as its own root.
- The root is not named `main.rs` or `lib.rs`, which would move proptest's regression files (`tests/properties.proptest-regressions` of `partsmith-sketch` keeps being read).
- Tests that run their own binary again in a child process (`partsmith-mesh`'s determinism probe and three probes of `partsmith-occt-sys`) select themselves with `--exact` and the name the harness gives them, built from `module_path!()` by `harness_test_name` in the crate's `tests/common`.

Three test targets stay separate, each with its reason in `Cargo.toml` and the crate's AGENTS.md:

- **`budgets` of `partsmith-features` and of `partsmith-recompute`** (`tests/budgets.rs` in each): the release-only budgets. As targets of their own they are timed beside only each other, never beside the tests of the other files. `partsmith-features`' are the framework budgets of features.md section 5, medians of 20 µs to 20 ms. `partsmith-recompute`'s are the 22 build-time budgets of the per-feature specs (features.md section 6.1), medians of 0.1 ms to 1.3 s, each in an inline module named after the file of the feature it times (`mod feature_box { ... }`), so its test id stays the one it had there (`feature_box::t010_default_box_builds_within_2_ms`); the records they share with that file's document tests are modules of `tests/feature_support` (`fillet.rs`, `hole.rs`, `import.rs`, `linear_pattern.rs` and `shell.rs`).
- **`partsmith-recompute`, `reference_stability`** (root `tests/reference_stability.rs`, `required-features = ["stability-study"]`): the reference stability corpus, its three thirds and their helpers as modules of one binary. As cfg-gated modules of `integration`, a run without the feature would pass with no test; `required-features` keeps Cargo's error instead. When the feature goes (recompute.md test 69), the corpus modules join `tests/integration.rs` and this target goes away.

`partsmith-recompute`'s budgets were first modules of `integration`, in the files of the features they time, timed beside the 890 other tests there. They held at the default thread count on an otherwise idle machine, but with twice as many test threads as cores (`RUST_TEST_THREADS=8` on 4 cores) `feature_shell::t012_performance` missed its 250 ms median in every run (f-sh1 318 and 364 ms), and on a loaded machine `feature_boolean::t016_join_builds_within_15_ms` missed its 15 ms. In their own target (release, 4 cores, otherwise idle, 2026-10-22), every budget but `feature_hole::t033` (features.md open issue 5) passed in 5 of 5 runs at the default thread count and in 2 of 2 runs one test at a time (`--test-threads=1`). At 8 threads `feature_shell::t012_performance` passed in 3 of 3 runs, but `feature_boolean::t016_join_builds_within_15_ms` missed its 15 ms in 3 of 3 (f-bool1 17.4 to 25.5 ms), since 8 budgets then share the 4 cores; alone at 8 threads it passed. A budget's median therefore holds when its target runs with no more test threads than cores, as the default thread count does, on an otherwise idle machine. `partsmith-features`' budgets passed 3 of 3 runs at 8 threads.

## Alternatives considered

- **Keep one binary per file.** Disk use and link time stay several times higher, locally and in CI.
- **`tests/<name>/main.rs` with the files moved under it.** Every path that names a test file changes, history follows the files only through renames, and `main.rs` moves proptest's regression files.
- **The corpus as cfg-gated modules of `integration`.** A command that names its modules without the feature selects nothing and passes.
- **The release-only budgets as modules of `integration`.** Their medians would be timed beside the crate's other tests on the same threads: the features budgets' microsecond medians beside 577 tests, recompute's beside 890, where `feature_shell::t012_performance` missed its 250 ms at 8 threads on 4 cores. Recompute's budgets were first kept there so that each stayed in the file of the feature it times.
- **Recompute's budgets kept in `integration` behind a lock.** Every helper that builds would take a shared lock and every budget an exclusive one: a measurement change in shared helpers for what a target of their own gets from Cargo.

## Consequences

- 177 integration targets became 18 (174 became 17 without the feature), and 105 OCCT-linking integration executables became 12. The integration test executables went from 7.5 GB to 1.03 GB, and linking those of `partsmith-recompute` (`integration` and `budgets`) from scratch took 7.8 to 8.0 s instead of 21.5 to 26.3 s (measured on 2026-10-22).
- A test's id starts with its file's name (`examples::examples_rebuild_as_expected`), and `cargo test -p <crate> --test integration <file>::` runs one file's tests. The files of the separate targets select nothing there: `budgets.rs` runs with `--test budgets` (in `partsmith-recompute`, `--test budgets feature_box::` runs one feature's budgets and `--test integration feature_box::` its document tests), and a corpus third with `--features stability-study --test reference_stability reference_stability_N::`. The filter is a substring match, so it also selects files whose names end in `_<file>`: in `partsmith-features`, `messages::` selects `context_messages`, `error_messages`, `followed_messages` and `warning_messages` too, and `-- --skip _messages::` narrows it. Without `--test integration`, a filter such as `diagnosis::` also selects the library's unit tests of a module of that name.
- A command that names a former target fails with Cargo's "no test target named" error instead of running nothing.
- A file in `tests/` that the root does not declare is never compiled, formatted or linted. A new test file is declared in `tests/integration.rs` (for example `crates/partsmith-engine/tests/reference_causes.rs`, which a later phase of the reference stability plan adds).
- Editing one test file recompiles the crate's whole test binary.
- Tests of different files of a crate share one process, so their OCCT calls overlap on the test threads; the locks of kernel.md 3.10 cover the shared OCCT state, and every merged binary that links OCCT passed repeated debug runs at the default thread count and at 8 or 16 threads. No release-only budget is in a merged binary: each `budgets` target times its medians beside only each other, with no more test threads than cores on an otherwise idle machine. A crash (a signal) ends every test of the crate's run.
- Helper modules share one name space: inherent methods that two modules add to one shared type clash, so such helpers are traits, each imported only where it is used (the corpus thirds' `PlacementChecks`).
- On `wasm32-unknown-emscripten`, all of a crate's integration tests run in one instance, one after another, and `partsmith-wasm`'s integration tests share one session there.
- A feature's release-only budgets are no longer in its `tests/feature_<type>.rs` but in the module of that name in `partsmith-recompute`'s `tests/budgets.rs` (features.md section 6.1).
- With the `stability-study` feature, Cargo builds `partsmith-recompute`'s library, `integration` and `budgets` a second time, as it did for the per-file binaries before.

## Amendments

| Date | Specs | Change |
| --- | --- | --- |
| 2026-10-22 | features.md section 6.1; delivery.md 3.2 and open issue 9 | **`partsmith-recompute`'s 22 release-only budgets are a test target of their own, `budgets`, as `partsmith-features`' are.** They were first modules of `integration`, in the files of the features they time, where `feature_shell::t012_performance` missed its 250 ms median in every run at 8 test threads on 4 cores. The measurements before and after are in the decision above. |
