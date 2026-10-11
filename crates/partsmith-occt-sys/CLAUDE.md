# partsmith-occt-sys

C++17 shim over OCCT 7.9.3 exposed with `cxx`, plus the safe facade: `Shape`, primitives, sketch-curve faces, prism, revolve, sweep, loft, booleans, fillet, chamfer, shell, draft, transforms, measurement, element geometry queries, tessellation to `partsmith_mesh::Mesh`, STEP I/O. Every operation returns history.

Spec: `docs/spec/kernel.md`. Requires `OCCT_ROOT` pointing at an install from `scripts/build-occt.sh`.

## Commands

```sh
cargo test -p partsmith-occt-sys
cargo clippy -p partsmith-occt-sys --all-targets -- -D warnings
```

## Rules

- The only crate with C++ and `unsafe`. Every `unsafe` block has a `// SAFETY:` comment.
- Every shim function catches all exceptions; nothing throws across the boundary.
- No C++ type appears in the public API.
