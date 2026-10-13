# partsmith-budgets

The harness of the performance budget benchmarks (`crates/*/benches/budgets.rs`): selecting operations by words of their names, median timing with `std::time::Instant`, and the report that prints every row and fails when one is over its budget. Dev-only (ADR 0027): no dependencies, `publish = false`, and only benchmarks depend on it, as a dev-dependency.

## Public interface (crate root)

- `Report`: `from_arguments` (the words on the command line select the operations whose names contain one of them; none selects all; flags starting with `--` are ignored), `includes`, `measure(operation, budget, time)` (runs `time` only for a selected operation), `record_time`, `record_bytes` (a memory row, shown in GB), `record_unmeasured` (a selected operation that could not be measured), and `finish() -> ExitCode`, which prints every row with the operations padded to one width, then `Over budget:` and `Not measured:` lines, and fails when there is any.
- Timing: `median(runs, sample)` (the median of `runs` samples, each a time the closure takes itself, given its sample number), `median_time(runs, setup, operation)` (a fresh untimed input per run), `median_time_of(runs, operation)` and `median_mean_time(runs, repeats, operation)` (the mean of `repeats` calls per run, for operations of microseconds). Inputs and results pass through `black_box`; results are dropped outside the timed span, except in `median_mean_time`, where each drop is timed with its call.

## Rules

- No dependencies; the crate is a dev-dependency of benchmarks only, never of a library, binary or test.
- Each benchmark keeps its own `RUNS`, workloads and budgets; the timed closures are the benchmark's, the harness only times and reports them.
- The desktop benchmark (`desktop/src-tauri/benches`, warm-up and 21 runs of desktop.md) and the sketch solver benchmark (`crates/partsmith-sketch/benches/solver.rs`, the scheme of sketch.md section 5) keep their own harnesses, which their specs define.

## Commands

```sh
cargo test -p partsmith-budgets
cargo clippy -p partsmith-budgets --all-targets -- -D warnings
cargo fmt -p partsmith-budgets
```
