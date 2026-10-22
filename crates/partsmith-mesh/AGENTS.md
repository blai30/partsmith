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

- `src/mesh.rs` type and invariants; `metrics.rs` (compensated sums); `transform.rs`; `combine.rs` (concat, remap, normals); `weld.rs` (uniform grid with cells 64 tolerances wide, independent of the mesh's extent, so a far-away vertex cannot crowd the cells; provably the same result as the sequential procedure); `validity.rs` (half-edges bucketed by low vertex, disjoint sets for shells); `ray.rs` (triangle test); `bounding_volume_hierarchy/` (48-byte nodes with `f32` boxes rounded outward and a cone of the directions that may graze their triangles, sibling pairs adjacent; boxes are padded per query by a bound on the triangle test's rounding, wide only for nearly grazing rays, so pruning never drops an accepted hit: `mod.rs` has the error analysis, the type and the queries, `build.rs` the binned surface area heuristic, `cone.rs` the grazing cones, `probe.rs` the padded box test); `slice.rs` (segments keyed by mesh edge, chaining per spec 3.9, `slice_many` buckets triangles by height once); `render.rs` (latest copy per mesh vertex, plus a map by (mesh vertex, face id) for vertices shared by several face ids); `hash.rs` (a fast deterministic hasher for maps that are only looked up).
- `ValidityReport` carries the public `triangle_count` field (spec 2.5), which `is_watertight` uses for its "at least one triangle" rule.
- No hash-map iteration anywhere; every result is deterministic.
- `tests/common` re-exports the section 6.1 fixtures of `partsmith-mesh-fixtures` (a dev-dependency, ADR 0027), which the io and print tests share, and keeps the fixtures and assertions only these tests use.

## Commands

```sh
cargo test -p partsmith-mesh
cargo test -p partsmith-mesh --test integration ray_casting::   # one file of tests/, a module of tests/integration.rs
cargo clippy -p partsmith-mesh --all-targets -- -D warnings
cargo fmt -p partsmith-mesh
cargo bench -p partsmith-mesh --bench budgets            # spec section 5 budgets, all of them
cargo bench -p partsmith-mesh --bench budgets -- weld    # only the operations whose names contain a word
```

`tests/ray_casting.rs` checks 10,000 random rays and 2,000 grazing rays from up to 1 km away against brute force and takes about 10 s in a debug build. The benchmark exits with a failure when a median time is over its budget.

## Rules

- Pure Rust, no kernel dependency, dependencies limited to `serde`, `schemars` and `thiserror`.
- No `unsafe`, no global state, no panics on user input (only the documented per-triangle accessors, and `render_buffers` beyond `u32::MAX` render vertices, which spec section 3.11 documents).
