# partsmith-mesh

Indexed triangle mesh with per-triangle face ids and edge polylines, plus metrics (volume, area, bounding box), validity (watertight, manifold, orientation), a BVH for ray queries, and planar slicing.

Spec: `docs/spec/mesh.md`.

## Commands

```sh
cargo test -p partsmith-mesh
cargo clippy -p partsmith-mesh --all-targets -- -D warnings
```

## Rules

- Pure Rust, no kernel dependency.
