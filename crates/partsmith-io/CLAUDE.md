# partsmith-io

STL (binary and ASCII), 3MF (with materials, colors and modifiers), OBJ writers, mesh import, and STEP via `partsmith-occt-sys`.

Spec: `docs/spec/io.md`.

## Commands

```sh
cargo test -p partsmith-io
cargo clippy -p partsmith-io --all-targets -- -D warnings
```

## Rules

- Outputs are deterministic so golden-file tests can compare bytes.
