# partsmith-print

Printer profiles (FDM and resin), fit classes, and printability checks on meshes: overhangs, thin walls, small features, bridges, bed fit, islands, first-layer contact, trapped volumes, mesh validity.

Spec: `docs/spec/print.md`.

## Commands

```sh
cargo test -p partsmith-print
cargo clippy -p partsmith-print --all-targets -- -D warnings
```

## Rules

- Pure Rust on `partsmith_mesh::Mesh`; no kernel dependency.
