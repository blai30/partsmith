# partsmith-sketch

Sketch params (plane, entities, constraints), the native solver (ADR 0004), DOF and redundancy diagnosis, drag solving, and profile regions with `RegionKey`.

Spec: `docs/spec/sketch.md`.

## Commands

```sh
cargo test -p partsmith-sketch
cargo clippy -p partsmith-sketch --all-targets -- -D warnings
```

## Rules

- Pure Rust, no kernel dependency.
- Solver must meet the 4 ms budget for 500 constraints; keep the benchmark green.
