# partsmith-mesh

Indexed triangle mesh with per-triangle face ids, edge polylines and vertex markers, plus metrics (volume, area, bounding box, center of mass), validity (watertight, manifold, orientation, degeneracy, shells), welding, a BVH for ray queries, planar slicing and the `f32` render buffers the protocol ships.

Spec: `docs/spec/mesh.md` (the definition of done). Acceptance tests in `tests/` come from its section 6; do not weaken them.

## Public interface (crate root)

- Scalars and constants: `Point3`, `Vector3`, `Point2` (`f64` arrays in mm), `WELD_TOLERANCE` (1e-6), `DEGENERATE_HEIGHT` (1e-9), `MAX_ELEMENTS` (`u32::MAX`).
- `Mesh`: immutable; `new` (validates, section 3.1 order), `from_triangles`, `empty`; accessors (`positions`, `triangles`, `face_ids`, `normals`, `edges`, `vertex_markers`, counts); per-triangle `triangle_points`, `triangle_normal`, `triangle_area`, `is_degenerate` (panic on out-of-range index); metrics `signed_volume`, `area`, `bounding_box`, `center_of_mass`; `transformed`, `concat`, `weld` (returns `Welded { mesh, vertex_map, triangle_map }`), `remap_ids`, `with_vertex_normals`; `validity` (`ValidityReport`), `shells` (`Vec<Shell>`); `slice`, `slice_many` (`Slice` of `SliceLoop`s); `render_buffers` (`RenderBuffers`).
- `EdgePolyline { edge_id, points }`, `VertexMarker { vertex_id, position }`.
- `BoundingVolumeHierarchy<'mesh>`: `new`, `mesh`, `first_hit`, `all_hits`; `Ray` (`new`, `origin`, `direction`) and `RayHit`. Results equal a brute-force Moller-Trumbore loop exactly (spec 3.10).
- Value types with serde and JSON Schema: `BoundingBox`, `Transform` (`identity`, `translation`, `rotation`, `then`, `inverse`, `determinant`, `apply_point`, `apply_vector`, `apply_normal`, `to_column_major`), `TessellationTolerance` (`new`, `Default`).
- Errors: `MeshError` (thiserror, messages in spec section 4) and `IdKind`.

## Internals

- `src/mesh.rs` type and invariants; `metrics.rs` (compensated sums); `transform.rs`; `combine.rs` (concat, remap, normals); `weld.rs` (spatial hash grid, provably the same result as the sequential procedure); `validity.rs` (half-edges bucketed by low vertex, disjoint sets for shells); `ray.rs` (triangle test); `bounding_volume_hierarchy.rs` (median split, 32-byte nodes with `f32` boxes rounded outward, sibling pairs adjacent, boxes padded per query so pruning never drops an accepted hit); `slice.rs` (segments keyed by mesh edge, chaining per spec 3.9, `slice_many` buckets triangles by height once); `render.rs`.
- `ValidityReport` has one private field (`has_triangles`) because `is_watertight` needs "at least one triangle"; it cannot be built with a struct literal outside the crate.
- No hash-map iteration anywhere; every result is deterministic.

## Commands

```sh
cargo test -p partsmith-mesh
cargo clippy -p partsmith-mesh --all-targets -- -D warnings
cargo fmt -p partsmith-mesh
```

`tests/ray_casting.rs` checks 10,000 random rays against brute force and takes about 10 s in a debug build.

## Rules

- Pure Rust, no kernel dependency, dependencies limited to `serde`, `schemars` and `thiserror`.
- No `unsafe`, no global state, no panics on user input (only the documented per-triangle accessors, and `render_buffers` beyond `u32::MAX` render vertices).
