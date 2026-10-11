# partsmith-recompute

Dependency DAG over the timeline, dirty propagation, result cache keyed by input hash, rollback, per-feature status.

Spec: `docs/spec/recompute.md`.

## Commands

```sh
cargo test -p partsmith-recompute
cargo clippy -p partsmith-recompute --all-targets -- -D warnings
```
