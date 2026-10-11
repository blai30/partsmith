# partsmith-features

`Feature` trait, `Registry`, `BuildContext`, `FeatureOutput`, and the built-in features (sketch, extrude, revolve, fillet, chamfer, shell, hole, primitives, booleans, patterns, mirror, datums, and more).

Spec: `docs/spec/features.md` and `docs/spec/features/<type>.md`.

## Commands

```sh
cargo test -p partsmith-features
cargo clippy -p partsmith-features --all-targets -- -D warnings
```

## Rules

- Built-in features use only the public registry API, like a plugin would.
- Every registered feature needs an example in `examples/`.
