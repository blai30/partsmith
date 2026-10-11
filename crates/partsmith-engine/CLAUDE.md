# partsmith-engine

`Session`: open documents, apply commands, undo and redo, recompute, tessellate, export, analyze, generate Python.

Spec: `docs/spec/engine.md`.

## Commands

```sh
cargo test -p partsmith-engine
cargo clippy -p partsmith-engine --all-targets -- -D warnings
```

## Rules

- The single entry point for every host; hosts contain no modeling logic.
