# partsmith-mesh-fixtures

The mesh fixtures of `docs/spec/mesh.md` section 6.1, shared by the acceptance tests of `partsmith-mesh`, `partsmith-io` and `partsmith-print` (each `tests/common` re-exports the crate) and by the unit tests of `partsmith-print`. Dev-only (ADR 0027): depends on `partsmith-mesh` only, `publish = false`, used only as a dev-dependency.

## Public interface (crate root)

- Box data: `BOX_TRIANGLES`, `BOX_FACE_IDS`, `box_positions(min, max)`, `mesh_with_face_ids(positions, triangles, face_ids)`.
- Meshes: `box_mesh(min, max)`, `cube(size)`, `cube_with(extra_positions, extra_triangles)` (each extra triangle with its face id), `faceted_box_data` and `faceted_box` (24 vertices, 4 per side), `open_box`, `flipped_one`, `reverse_triangles`, `reversed_box`, `inside_out`, `void_box`, `fin`, `sphere(radius, segments, rings)`, `standard_sphere()` (`sphere(10, 64, 32)`), `boxes(corners)` (several boxes in one mesh).

## Rules

- Each fixture gives exactly the mesh mesh.md section 6.1 defines; the acceptance tests of three crates depend on it, so a change to a fixture is a spec change.
- Built only through the public `partsmith_mesh` API; a fixture mistake panics.
- `partsmith-mesh` may use the crate in its integration tests and benchmark only: its unit tests would see a second copy of its own types.
- Fixtures one crate alone uses stay in that crate's `tests/common`.

## Commands

```sh
cargo test -p partsmith-mesh-fixtures
cargo clippy -p partsmith-mesh-fixtures --all-targets -- -D warnings
cargo fmt -p partsmith-mesh-fixtures
```
