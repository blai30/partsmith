# partsmith-features

`Feature` trait, `Registry`, `BuildContext`, `FeatureOutput`, and the built-in features (sketch, extrude, revolve, fillet, chamfer, shell, hole, primitives, booleans, patterns, mirror, datums, and more).

Spec (the definition of done): `docs/spec/features.md` and `docs/spec/features/<type>.md`. Acceptance tests in `tests/` come from features.md sections 6.2 and 6.3; document tests of section 6.4 and of every feature spec live in `crates/partsmith-recompute/tests/`. Do not weaken them.

## Public interface

The modules of features.md section 2.1 are public: `feature`, `deps`, `params`, `context`, `output`, `error` (errors, warnings and message parts), `registry` (with `register_builtins`), `python` and `builtin` (one module per type). The crate root re-exports everything below.

- Constants: `VERSION`, `FEATURE_API_VERSION`, `OCCT_VERSION` (re-exported). Manifest types re-exported from `partsmith_model::manifest`: `FeatureManifest`, `Category`, `FieldHint`, `FieldUnit`, `SelectionKind`, `OptionHint`.
- Contract: `Feature` (with provided `validate` and `python`), `DynFeature`, `Dependencies` (`Link`, `AssetLink`, `ExprUse`; helpers `add_feature`, `add_body_read`, `add_body_written`, `add_expression`, `add_asset`, `add_plane`, `add_axis` keep the no-duplicate rules).
- Shared params: `BodyOp`, `CopyOp` (`is_default`), `ProfileRef`, `PlaneRef` (`Default` is origin XY, `is_default`), `AxisRef`, `OriginAxis`, and `PlacementExt` on `partsmith_sketch::Placement` (`offset`, `flipped`, `local_to_world`, `to_kernel`). Primitives share `PrimitivePlacement` (`add_dependencies`, `frame`, `python_keywords`, `hints`) with `default_position` and `is_default_position` (section 3.14).
- Context: `BuildContext::new(BuildInputs)` with `feature`, `units`, `field` (a `FieldRef` labeled from the manifest), `eval_length/angle/scalar/count`, `body`, `sketch`, `datum`, `asset`, `profile`, `resolve`, `resolve_all`, `plane`, `axis`, `is_canceled`. `Values::evaluate` and `iter`; `BodyState` (`empty`, `new`, `shape`, `naming`, `is_empty`, `elements` cached and shared by clones); `Resolved`; `ResolvedProfile` with `faces()` (the union of the resolved regions as kernel faces, `ProfileFaces { profile, entities }` where curve tag `i` is `entities[i]`); `CancelToken`.
- Output: `FeatureOutput`, `OutputData`, `BodyUpdate`, `BodyChange`, `SketchResult` (`point`, `to_world`), `DatumResult`, `Tool`, and `apply_op` (the body semantics of section 3.5 with the boolean naming rule).
- Naming bridge: `topology_info` (kernel topology and fingerprints for `partsmith_naming`), `name_shape`, `modified_sources`, `name_by_history` (the boolean rule with the `<feature>/face` fallback).
- Messages: `FeatureError` and `FeatureWarning` (`render`, `code`, `field`, `suggestion`, `repair`; `Display` renders with ids and millimeters), `FieldRef` (`new`, `pointer`), `Bound`, `Noun`, `IgnoredReason`, `FallbackReason`, `Repair`, `Names`, `DocumentNames`, `format_value`. `too_large_search` is the bisection of section 4.3.
- Registry: `Registry` (`new`, `with_builtins`, `register`, `register_dyn`, `get`, `types`, `validate_and_canonicalize`, `params_schemas`, `dependencies`, `build`, `python`, `python_generic`, `label`, and `FeatureInspector`), `Canonical`, `Validated`, `RegistryError`, `register_builtins`.
- Python: `PythonCall` (`new`, `render`), `PythonValue` (`from_json`, `expression`, `length`, `angle`, `reference`, `region`, `plane`, `axis`, `render`), `PythonNames`.

## Internals

- `deserializer.rs` reads typed params straight from the JSON map with its own serde error type, so `InvalidParams` details name the deepest field path without parsing serde messages. Unknown root keys are the input keys the params schema does not know: `schema::known_keys` collects the root `properties` and those of every root `$ref`, `anyOf`, `oneOf` and `allOf` branch, so the fields of a flattened enum are known keys, reserved-name checked and need hints.
- `schema.rs` holds the registration checks (open object root, reserved keys, `$defs` conflicts, manifest against schema through `$ref`, `anyOf`, `oneOf`, `allOf`, `items` and `prefixItems`).
- `profile.rs` builds the region union by dropping boundary pieces that two selected regions share (matched exactly, both directions) and chaining the rest through the arrangement's own loop successors; components (joined by shared pieces) give one outer loop each.
- `error/` holds `FeatureError` (`mod.rs`), `FeatureWarning` (`warning.rs`), `FieldRef` and field paths (`field.rs`), and the message parts (`message.rs`: names, `format_value`, and the suggested-limit text, which cuts two significant digits from the shortest decimal form so it never exceeds the value).
- `test_support.rs` (tests only) has a `Fixture` that owns a context's inputs, named box and primitive tools, `ScriptNames` for Python hooks, and `DocumentRun` (`run`, `run_document`), a small timeline runner for documents of built-in features.
- `tests/budgets.rs` times the framework budgets of features.md section 5 with `std::time::Instant` in release builds; the `BodyState::elements` budget of a 200-face body is ignored while the kernel's `Shape::elements(Face)` alone is over it.

## Built-in features

Each type lives in `src/builtin/<type>.rs` and is registered by adding `Registry::register::<Type>` to `BUILTINS` in `src/builtin/mod.rs`. All 20 types of features.md section 3.13 are registered, so the acceptance tests that need them run by default.

Registered types:

- Primitives (features.md section 3.14): `box` (`BoxFeature`, since `Box` is the standard library's pointer), `cylinder` (`Cylinder`), `sphere` (`Sphere`), `cone` (`Cone`), `torus` (`Torus`). Each params struct puts its sizes first, then the shared `plane` and `position` (`PrimitivePlacement`), then `op`; sizes are checked in field order before any kernel work, faces are named `<feature>/<role>` from the kernel's `PrimitiveFace` roles (`common::primitive_tool`), and the tool goes through `apply_op`. The box shifts the kernel's corner-anchored frame by half its length and width; a cone with radii within 1e-9 mm of each other builds a kernel cylinder.
- Dress-up (features/fillet.md, chamfer.md, shell.md): `fillet` (`Fillet`), `chamfer` (`Chamfer`, with `ChamferDistance` tagged `equal` or `two_distances`) and `shell` (`Shell`). They share `dressup.rs`: `select_edges` resolves `edges` and skips smooth edges with an `Ignored` warning (a seam, or two faces whose normals at the edge midpoint differ by at most 1e-6 rad; the normals come from the kernel's surface description, signed for flat faces and up to sign for cylinders and spheres, and any other surface or a curve other than a line or circle counts as a corner), `run_or_search` calls the kernel and runs the "too large" search on any failure except invalid input, and `add_origins` turns kernel history into the face origins that `name_blend` and the shell's naming map to names (naming.md sections 3.6.5 to 3.6.7). A two-distance chamfer measures `expr` on the face of each edge whose name sorts first (`flip` swaps the distances). Shell hollows each solid of the body with its own removed faces (a solid without any becomes a closed hollow) and combines several results with the kernel's `compound`, mapping each solid's history through `Shape::locate`.

## Commands

```sh
cargo test -p partsmith-features
cargo clippy -p partsmith-features --all-targets -- -D warnings
cargo fmt -p partsmith-features
# performance budgets (features.md section 5), release builds only
cargo test -p partsmith-features --release --test budgets
```

The kernel facade needs an OCCT install: `export OCCT_ROOT=/opt/occt/7.9.3/native`.

## Rules

- Built-in features use only the public registry API, like a plugin would.
- Every registered feature needs an example in `examples/`.
- `FeatureError` keeps the payload types the spec gives, so the crate expects `clippy::result_large_err` (see `lib.rs`).
