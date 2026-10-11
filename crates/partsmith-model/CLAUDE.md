# partsmith-model

Document recipe model: `Id`, `Units`, `Expr` and its evaluator, `Document`, `Parameter`, `Body`, `FeatureRecord`, `Edit` with inverses, `UndoLog`, canonical `.ptsm` JSON, migrations.

Spec: `docs/spec/model.md`, `docs/spec/expressions.md`, `docs/spec/file-format.md`.

## Commands

```sh
cargo test -p partsmith-model
cargo clippy -p partsmith-model --all-targets -- -D warnings
```

## Rules

- Pure Rust, no kernel dependency.
- Canonical serialization must stay byte-stable; any change to output needs a format ADR.
