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

Two test targets stay separate, each with its reason in `Cargo.toml` and the crate's AGENTS.md:

- **`partsmith-features`, `budgets`** (`tests/budgets.rs`): the release-only budgets of features.md section 5, medians of 20 µs to 20 ms. As a target of their own they are timed beside only each other, never beside the tests of the other files.
- **`partsmith-recompute`, `reference_stability`** (root `tests/reference_stability.rs`, `required-features = ["stability-study"]`): the reference stability corpus, its three thirds and their helpers as modules of one binary. As cfg-gated modules of `integration`, a run without the feature would pass with no test; `required-features` keeps Cargo's error instead. When the feature goes (recompute.md test 69), the corpus modules join `tests/integration.rs` and this target goes away.

`partsmith-recompute`'s 22 release-only budgets stay in `integration`: each is a test of the feature it times, in that feature's `tests/feature_<type>.rs` (features.md section 6.1), and a target of their own would move them out of those files. They are timed beside the tests of the other files, so they run at the default thread count on an otherwise idle machine (`cargo test -p partsmith-recompute --release`), where they give the results of the per-file binaries. With twice as many test threads as cores (`RUST_TEST_THREADS=8` on 4 cores), `feature_shell::t012_performance` misses its 250 ms median, which it met in its own binary, where the file's other tests finished early.

## Alternatives considered

- **Keep one binary per file.** Disk use and link time stay several times higher, locally and in CI.
- **`tests/<name>/main.rs` with the files moved under it.** Every path that names a test file changes, history follows the files only through renames, and `main.rs` moves proptest's regression files.
- **The corpus as cfg-gated modules of `integration`.** A command that names its modules without the feature selects nothing and passes.
- **The features budgets as a module.** Microsecond medians would be timed beside 577 other tests on the same threads.
- **`partsmith-recompute`'s release budgets as a target of their own.** They would leave the files of the features they time.

## Consequences

- 177 integration targets became 17 (174 became 16 without the feature), and 105 OCCT-linking integration executables became 11. The integration test executables went from 7.5 GB to 0.96 GB, and linking those of `partsmith-recompute` from scratch took 8.4 to 8.8 s instead of 21.5 to 26.3 s (measured on 2026-10-22).
- A test's id starts with its file's name (`examples::examples_rebuild_as_expected`), and `cargo test -p <crate> --test integration <file>::` runs one file's tests. The files of the two separate targets select nothing there: `budgets.rs` runs with `--test budgets`, and a corpus third with `--features stability-study --test reference_stability reference_stability_N::`. The filter is a substring match, so it also selects files whose names end in `_<file>`: in `partsmith-features`, `messages::` selects `context_messages`, `error_messages`, `followed_messages` and `warning_messages` too, and `-- --skip _messages::` narrows it. Without `--test integration`, a filter such as `diagnosis::` also selects the library's unit tests of a module of that name.
- A command that names a former target fails with Cargo's "no test target named" error instead of running nothing.
- A file in `tests/` that the root does not declare is never compiled, formatted or linted. A new test file is declared in `tests/integration.rs` (for example `crates/partsmith-engine/tests/reference_causes.rs`, which a later phase of the reference stability plan adds).
- Editing one test file recompiles the crate's whole test binary.
- Tests of different files of a crate share one process, so their OCCT calls overlap on the test threads; the locks of kernel.md 3.10 cover the shared OCCT state, and every merged binary that links OCCT passed repeated debug runs at the default thread count and at 8 or 16 threads. Release-only budgets in a merged binary (`partsmith-recompute`'s) are timed beside the other tests, so they keep their per-file results only at the default thread count on an otherwise idle machine. A crash (a signal) ends every test of the crate's run.
- Helper modules share one name space: inherent methods that two modules add to one shared type clash, so such helpers are traits, each imported only where it is used (the corpus thirds' `PlacementChecks`).
- On `wasm32-unknown-emscripten`, all of a crate's integration tests run in one instance, one after another, and `partsmith-wasm`'s integration tests share one session there.
- With the `stability-study` feature, Cargo builds `partsmith-recompute`'s library and `integration` a second time, as it did for the per-file binaries before.
