# partsmith-model

Document recipe model: ids and id generators, units, colors, `Document` and its records, structural rules and validation, edits with exact inverses, transactions and the undo log, changing units with Keep sizes, feature manifests, expressions (parse, dimension-checked evaluation, parameters, rename, display, bare numbers, suggestions for unknown names), canonical `.ptsm` JSON (load, save, migration, JSON Schema).

Spec (the definition of done): `docs/spec/model.md`, `docs/spec/expressions.md`, `docs/spec/file-format.md`. Acceptance tests in `tests/` come from their section 6; do not weaken them.

## Public interface (crate root)

- Constants: `LINEAR_TOLERANCE`, `ANGULAR_TOLERANCE`, `MAX_ID_ATTEMPTS`, `MAX_PARAMETER_NAME_LENGTH`, `MAX_LABEL_LENGTH`, `MAX_FEATURE_TYPE_LENGTH`, `RESERVED_NAMES`, `MAX_EXPRESSION_LENGTH`, `MAX_EXPRESSION_DEPTH`, `SUGGESTION_DISTANCE`, `FORMAT`, `FORMAT_VERSION`, `OLDEST_FORMAT_VERSION`, `FILE_EXTENSION`, `MIGRATIONS`.
- Ids: `Id` (`new`, `generated`, `as_str`), `IdKind`, `IdGenerator` with `RandomIdGenerator` and `SequentialIds`, `IdError`.
- Values: `Units`, `Color`, `BodyRole`, with `UnitsError`, `ColorError`.
- Records: `Document` (lookups, `active_features`, `contains_id`, `used_ids`, `check_structure`, `validate`, `apply`, `apply_all`, `rename_parameter_edits`, `units_edits`, `bare_numbers`, `evaluate_parameters`, `normalize`), `Parameter`, `Body`, `FeatureRecord`.
- Names: `validate_parameter_name`, `validate_label`, `is_valid_feature_type`, `LabelField`, `NameError`, `LabelError`.
- Registry seam: `FeatureInspector`, `FeatureLinks`, `ExprLink` (`pointer`, `expr`, `unit`), `OpaqueFeatures`.
- Problems: `StructureError`, `ValidationError`.
- Edits: `Edit` (15 variants), `Step`, `Transaction`, `RecordKind`, `EditError`, `TransactionError`; `UndoLog` (`commit`, `record`, `undo`, `redo`, labels, counts) and `UndoError`.
- Manifests (`manifest` module, also at the root): `FeatureManifest`, `Category`, `FieldHint`, `FieldUnit`, `SelectionKind`, `OptionHint`.
- Expressions: `Expr` (`parse`, `evaluate`, `references`, `rename_parameter`, `is_bare_number`, `with_unit`, `promoted_names`), `ParsedExpr`, `ExprNode`, `ExprNodeKind`, `BinaryOperator`, `Span`, `Unit`, `Function`, `Arity`, `Dimension`, `Quantity` (`display`), `EvaluationContext`, `EmptyContext`, `Scope`, `ParameterValues`, `References`, `ExprError` (`corrected`), `ExprErrorKind` (`UnknownName` carries its `suggestion`), `MismatchOperation`, `RenameError`, `edit_distance`.
- Files: `load`, `load_bytes`, `Loaded`, `save`, `migrate`, `migrate_with`, `MigrationStep`, `ptsm_schema`, `ptsm_schema_text`, `FeatureParamsSchema`, with `LoadError`, `SaveError`, `SchemaError`.
- `canonical` module: `to_string`, `format_number`, `format_string`, `to_value`, `normalize_numbers`, `parse`, `MAX_LINE_WIDTH`, `INDENT_WIDTH`; `CanonicalError` (also at the root).

## Internals

- `src/expr/`: `lexer.rs` (whole source to tokens first), `parser.rs` (operator-precedence parser on explicit stacks, so no input recurses; depth and parenthesis limits checked as nodes are built), `evaluate.rs` (post-order evaluation, unify and promotion; the suggestion for an unknown name is chosen once, after evaluation), `parameters.rs` (`ParameterGraph`, iterative Tarjan cycles, Kahn order with ties by position, `ParameterValues`; suggestions among every other parameter), `sizes.rs` (`is_bare_number`, `with_unit`, `promoted_names` and the promoting-position walk behind them and Keep sizes), `suggestion.rs` (`edit_distance` and the one-close-name rule), `dimension.rs`, `error.rs`, `unit.rs`, `function.rs`.
- `src/canonical/`: `layout.rs` (width check stops at the 120-column budget, so layout is linear), `number.rs` (normalized numbers, `ryu` for non-integers), `string.rs`, `parse.rs` (`serde_json` with a seed that rejects duplicate keys and normalizes numbers), `value.rs` (a no-output serializer that finds NaN and infinity with their path before `serde_json::to_value`).
- `src/file/`: `mod.rs` (load, save, errors), `read.rs` (hand-written, path-aware reader for `Document`, `Parameter`, `Body`, `FeatureRecord`, so `Invalid` errors carry paths without `serde_path_to_error`), `migrate.rs` (version checks and the chain; `FORMAT_VERSION` is 2 and the one built-in step, from version 1, returns the document unchanged, ADR 0025), `schema.rs`.
- `src/change_units.rs`: `units_edits` and `bare_numbers` share one pass that parses every expression once, then follows parameter uses as a size, a factor or another unit from each link (each parameter at most once per way, so cycles end).
- `src/path.rs`: borrowed JSON paths for error messages. `src/inspect.rs` also finds and replaces a link's expression at its pointer for `rename.rs` and `change_units.rs`. Other files hold one area each: `id.rs`, `units.rs`, `names.rs`, `document.rs`, `structure.rs`, `validation.rs`, `edit.rs`, `rename.rs`, `undo.rs`, `manifest.rs`.

## Commands

```sh
cargo test -p partsmith-model
cargo clippy -p partsmith-model --all-targets -- -D warnings
cargo fmt -p partsmith-model
# Section 5 budgets of the three specs (benches/budgets.rs); fails when one is exceeded
cargo bench -p partsmith-model --bench budgets
```

## Rules

- Pure Rust, no kernel dependency, no global state. Dependencies: `serde`, `serde_json`, `schemars`, `thiserror`, `ryu`, `getrandom` (model.md section 2.1); anything else needs an ADR.
- Canonical serialization must stay byte-stable; any change to output needs a format ADR.
- A `FORMAT_VERSION` bump moves every committed `.ptsm` to the new version: these fixtures, the engine's, `examples/` (checked by the CLI) and `ui/e2e/fixtures/` (checked by `tests/committed_documents.rs`).
- `serde_json` must stay without `arbitrary_precision`: the strict parser and number handling rely on plain number values.
