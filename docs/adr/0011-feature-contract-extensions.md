# 0011: Feature contract extensions and manifests in the model crate

Status: Accepted, needs human review

## Context

The architecture fixes the `Feature` trait (`TYPE`, `Params`, `manifest`, `dependencies`, `build`), the `Dependencies` it declares, the body updates in `FeatureOutput` (body, shape, naming map and "the boolean op applied") and puts `FeatureManifest` in `partsmith-features`. Writing the feature, model, protocol and Python specs showed gaps:

- Some params rules cannot be expressed by serde (non-empty selections, a `target` required by op `new`, distinct bodies) and must fail before any build.
- Python export must produce readable build123d-style calls (`box(20*mm, 30*mm, height)`), not only the generic `feature("box", ...)` form.
- The model must check that ids declared inside params (sketch entities and constraints) are unique in the document, but cannot read params.
- The `import` feature reads a file from the document's asset store (file-format.md section 3.11).
- Fillet, chamfer, shell, move and split change a body in place, and a consumed boolean tool empties its body; neither is a boolean op.
- `partsmith-protocol` does not depend on `partsmith-features`, so it could carry manifests only as untyped JSON, and the UI had to mirror them by hand.

## Decision

- The `Feature` trait gains two provided methods. `validate(params) -> Result<(), FeatureError>` (default `Ok(())`) checks rules serde cannot express; the registry runs it when it validates and canonicalizes params. `python(params, names: &dyn PythonNames) -> Option<PythonCall>` (default `None`) returns the feature's dedicated build123d-style call. `PythonNames` gives the document units and the script variables of bodies, features, sketch entities and parameters. `Registry::python(record, names)` returns the dedicated call when the type is registered, its params validate and the hook returns `Some`; otherwise `Registry::python_generic(record)`, the generic `feature("<type>", ...)` form that plugins without a hook also get. `PythonValue` gains `Name`, `Call` and `Tuple` for variables, quantity literals and nested calls. The engine's script generator owns sketch `with` blocks, variable assignment and the `name=`, `suppressed=` and `id=` keywords (features.md section 2.10, python-api.md section 3.8).
- `Dependencies` gains `declared_ids` (ids the params define, in params order; the registry reports them to the model through `FeatureInspector`) and `assets` (asset file names the feature reads, each with the field that names it). The build context hands declared assets to `build`; an undeclared read fails like any other undeclared dependency.
- `BodyChange`, the architecture's "boolean op applied", has the variants `New`, `Join`, `Cut`, `Intersect`, `Modify` (the body was changed in place) and `Clear` (the body was emptied).
- `FeatureManifest`, `Category`, `FieldHint`, `FieldUnit`, `SelectionKind` and `OptionHint` are pure data defined in `partsmith-model` (model.md section 2.12) and re-exported by `partsmith-features`. `partsmith-protocol` carries manifests typed, and the UI uses the generated TypeScript types. `FieldUnit` also replaces the features crate's `ValueKind`, which had the same variants.

## Consequences

Built-in and plugin features still go through the same trait and registry (ADR 0005); every new method has a default, so a minimal feature implements only the original five items. The model crate grows by a few plain data types and no dependency. Manifests, catalogs and inspector hints have one typed definition from Rust to TypeScript. Python export reads like hand-written build123d code for built-in features and stays complete for plugins through the generic form.
