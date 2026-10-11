# partsmith-naming

Element names, `NamingMap`, references (`{q, fp}`), fingerprints and the resolver.

Spec: `docs/spec/naming.md`.

## Commands

```sh
cargo test -p partsmith-naming
cargo clippy -p partsmith-naming --all-targets -- -D warnings
```

## Rules

- Pure Rust, no kernel dependency. Works on `ElementInfo` data, never on shapes.
- Changing the naming algorithm needs an ADR and human review.
