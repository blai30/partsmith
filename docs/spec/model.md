# Model (`partsmith-model`)

Behavior spec for the document recipe model: ids, document units, colors, the `Document` and its records, structural rules and validation, edits with exact inverses, transactions and the undo log. Two companion specs cover the rest of the crate:

- [expressions.md](expressions.md): `Expr`, units in expressions, `Dimension`, `Quantity`, evaluation, the parameter graph.
- [file-format.md](file-format.md): canonical `.ptsm` JSON, loading, saving, numbers, `format_version`, migration, JSON Schema generation.

Contracts come from [architecture.md](../design/architecture.md) (section "Document model") and ADRs [0005](../adr/0005-feature-registry-with-generic-params.md), [0008](../adr/0008-short-random-ids.md) and [0011](../adr/0011-feature-contract-extensions.md). Where this spec had to interpret them, the interpretation is listed under [Open issues](#7-open-issues).

## 1. Purpose and scope

`partsmith-model` is the recipe layer every other crate builds on. It is pure Rust with no kernel dependency, builds unchanged for native and `wasm32-unknown-emscripten`, has no global mutable state, and never panics on user input. It does not interpret feature params: they are a generic JSON map, and the feature registry in `partsmith-features` owns their typed form, validation, key order and dependency extraction (ADR 0005). The model reaches the registry only through the `FeatureInspector` trait defined here.

In this milestone (M1):

- `Id` validation, `IdKind` prefixes, the `IdGenerator` trait with `RandomIdGenerator` (interactive edits) and `SequentialIds` (script runs, CLI, tests).
- Document `Units`, `Color`, `BodyRole`.
- `Document`, `Parameter`, `Body`, `FeatureRecord` (params as a generic JSON map flattened into the record).
- Parameter name rules, label rules, feature type rules.
- Structural rules that every document in memory satisfies (`check_structure`), and full validation with references, expressions and the parameter graph (`validate`).
- `Edit` with all 15 variants and exact inverses, atomic multi-edit application (`apply_all`), the edit set that renames a parameter everywhere (`rename_parameter_edits`), and the edit set that changes the document units while keeping sizes (`units_edits`, with `bare_numbers` to count what it rewrites).
- `UndoLog`: labeled transactions, undo, redo, redo cleared by a new commit, maximum depth.
- Feature manifests (`FeatureManifest` and its parts): pure data the feature registry defines and the protocol carries.
- Expressions and parameter evaluation (expressions.md), canonical JSON, load and save, `format_version` 2 with the migration from version 1, and JSON Schema generation (file-format.md).

Explicitly later (not in M1):

- Components (local and linked from other `.ptsm` files) and their records.
- Joints and joint values.
- Configurations (design tables) and per-configuration suppression (v1.1).
- `.ptsz` bundles.
- Script features (Python blocks in the timeline) and their trust state.
- Plugin feature records carrying a plugin id and version.
- Body "export as" setting and extruder or filament slot.
- Settings for settings-modifier bodies (for example infill percentage).
- Parameter metadata: declared dimension, minimum, maximum, slider and dropdown hints for the Customizer panel.
- Cross-file parameter links.
- Undo transaction merging (coalescing consecutive slider drags into one step).
- Persisting undo history across sessions.
- The git merge driver.

## 2. Public API

### 2.1 Crate layout and dependencies

Every public item in this spec, expressions.md and file-format.md is re-exported from the crate root (`partsmith_model::Document`, `partsmith_model::Expr`, `partsmith_model::canonical::to_string`). Acceptance tests import from the root (and from `partsmith_model::canonical` for the canonical writer module).

Allowed dependencies: `serde`, `serde_json` (features `preserve_order` and `float_roundtrip`), `schemars`, `thiserror`, `ryu`, `getrandom`. Dev dependencies: `pretty_assertions`, `proptest`. No other dependency without an ADR.

Every public type that is part of the document or of an edit derives `Clone`, `Debug`, `PartialEq`, `serde::Serialize`, `serde::Deserialize` and `schemars::JsonSchema` (or implements them by hand where this spec says so). Every error type derives `Clone`, `Debug`, `PartialEq` and `thiserror::Error`; its `Display` is the user-facing message in section 4.

Error types: `StructureError` and `ValidationError` are declared in section 2.8. `IdError`, `UnitsError`, `ColorError`, `NameError`, `LabelError`, `EditError` and `UndoError` are enums whose exact variants and field types are the rows of the tables in section 4. `TransactionError` is a struct with public fields `index: usize` (0-based position of the failing edit), `count: usize` (number of edits in the batch) and `error: EditError`.

### 2.2 Constants

```rust
/// Modeling tolerance in millimeters, shared by every crate.
pub const LINEAR_TOLERANCE: f64 = 1e-6;
/// Angular tolerance in radians, shared by every crate.
pub const ANGULAR_TOLERANCE: f64 = 1e-9;
```

The sketch solver convergence tolerance (`1e-10`) belongs to `partsmith-sketch`.

### 2.3 Ids

```rust
/// A validated record id: 1 to 64 characters, each one of `A-Z`, `a-z`, `0-9`, `-` or `_`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Id(String);

impl Id {
    /// Longest valid id, in characters.
    pub const MAX_LENGTH: usize = 64;
    /// Validates `value` and wraps it.
    pub fn new(value: impl Into<String>) -> Result<Id, IdError>;
    /// Builds the generated form: the kind prefix plus 12 lowercase hex digits of the low 48 bits of `value`.
    pub fn generated(kind: IdKind, value: u64) -> Id;
    /// The id text.
    pub fn as_str(&self) -> &str;
}
// Also implemented: FromStr (Err = IdError), TryFrom<String>, TryFrom<&str>, From<Id> for String,
// Display (the id text), AsRef<str>, Borrow<str>, and JsonSchema as
// {"type": "string", "pattern": "^[A-Za-z0-9_-]{1,64}$"} under the definition name "Id".

/// The kind of record a generated id is for; it selects the prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IdKind {
    /// Prefix `p-`.
    Parameter,
    /// Prefix `b-`.
    Body,
    /// Prefix `f-`.
    Feature,
    /// Prefix `e-`.
    SketchEntity,
    /// Prefix `c-`.
    SketchConstraint,
}

impl IdKind {
    /// Every kind, in declaration order.
    pub const ALL: [IdKind; 5];
    /// The prefix, such as `"f-"`.
    pub fn prefix(self) -> &'static str;
}

/// Most candidate values `RandomIdGenerator` tries before giving up.
pub const MAX_ID_ATTEMPTS: usize = 64;

/// Source of new ids. Each generator owns its state; there is no global generator.
pub trait IdGenerator: Send {
    /// Returns a new id of `kind`, starting with `kind.prefix()`, for which `is_taken` returns false (section 3.2).
    fn generate(&mut self, kind: IdKind, is_taken: &dyn Fn(&Id) -> bool) -> Result<Id, IdError>;
}

/// Generator backed by the operating system random source; used for interactive edits.
#[derive(Clone, Copy, Debug, Default)]
pub struct RandomIdGenerator;

impl RandomIdGenerator {
    /// Creates the generator.
    pub fn new() -> RandomIdGenerator;
}

/// Deterministic generator with one counter per kind (`f-000000000001`, `f-000000000002`, ...).
/// Used by script runs, the CLI and tests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SequentialIds { /* private: next value per IdKind */ }

impl SequentialIds {
    /// Creates a generator whose counters all start at 1.
    pub fn new() -> SequentialIds;
}
```

### 2.4 Units and colors

```rust
/// Document units: the unit of bare numbers in length expressions and the display unit for lengths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Units {
    /// Millimeters, `"mm"`.
    #[default]
    Mm,
    /// Centimeters, `"cm"`.
    Cm,
    /// Meters, `"m"`.
    M,
    /// Inches, `"in"`.
    In,
}

impl Units {
    /// Every document unit, in declaration order.
    pub const ALL: [Units; 4];
    /// Millimeters in one of this unit: 1, 10, 1000 or 25.4.
    pub fn millimeters(self) -> f64;
    /// The symbol: "mm", "cm", "m" or "in".
    pub fn symbol(self) -> &'static str;
    /// The matching expression unit (expressions.md section 2).
    pub fn unit(self) -> Unit;
}
// Also implemented: FromStr (accepts exactly the four symbols, Err = UnitsError), Display (the symbol),
// and Deserialize through FromStr, so an invalid value reports the UnitsError message.

/// An sRGB color, serialized as "#rrggbb" with lowercase hex digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red channel.
    pub red: u8,
    /// Green channel.
    pub green: u8,
    /// Blue channel.
    pub blue: u8,
}

impl Color {
    /// Creates a color from its channels.
    pub fn new(red: u8, green: u8, blue: u8) -> Color;
}
// Also implemented: FromStr (Err = ColorError), Display ("#rrggbb", lowercase), Serialize and
// Deserialize as that string, JsonSchema as {"type": "string", "pattern": "^#[0-9A-Fa-f]{6}$"}
// under the definition name "Color".

/// What a body is for when it is printed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BodyRole {
    /// A printed part (default, omitted when saved).
    #[default]
    Part,
    /// Negative volume: subtracted by the slicer.
    NegativeVolume,
    /// Region where the slicer must not place supports.
    SupportBlocker,
    /// Region where the slicer must place supports.
    SupportEnforcer,
    /// Region with its own slicer settings (the settings themselves come later).
    SettingsModifier,
}

impl BodyRole {
    /// True for `Part`; used to omit the default when saving.
    pub fn is_part(&self) -> bool;
}
```

### 2.5 Document records

```rust
/// The document recipe. Geometry is never stored; it is regenerated from this.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Document {
    /// Always "partsmith".
    pub format: String,
    /// Format version; equals FORMAT_VERSION for every document in memory.
    pub format_version: u32,
    /// Unit of bare numbers in length expressions, and the display unit for lengths.
    pub units: Units,
    /// Printer profile id whose values `printer.*` names resolve to; `None` when no printer is chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub printer: Option<String>,
    /// Parameters in the order the user sees them.
    #[serde(default)]
    pub parameters: Vec<Parameter>,
    /// Declared bodies in browser order.
    #[serde(default)]
    pub bodies: Vec<Body>,
    /// Feature records in timeline order.
    #[serde(default)]
    pub features: Vec<FeatureRecord>,
    /// First rolled-back feature: it and every later feature do not run. `None` means nothing is rolled back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback: Option<Id>,
}

impl Document {
    /// An empty, structurally valid document in the given units.
    pub fn new(units: Units) -> Document;
    /// The parameter with this id.
    pub fn parameter(&self, id: &Id) -> Option<&Parameter>;
    /// The parameter with this name (exact, case-sensitive).
    pub fn parameter_by_name(&self, name: &str) -> Option<&Parameter>;
    /// The body with this id.
    pub fn body(&self, id: &Id) -> Option<&Body>;
    /// The feature with this id.
    pub fn feature(&self, id: &Id) -> Option<&FeatureRecord>;
    /// Timeline position of the feature with this id.
    pub fn feature_index(&self, id: &Id) -> Option<usize>;
    /// Features before the rollback feature, or all features (section 3.8).
    pub fn active_features(&self) -> &[FeatureRecord];
    /// True when a parameter, body or feature has this id.
    pub fn contains_id(&self, id: &Id) -> bool;
    /// Every id in use: parameter, body and feature ids plus ids declared inside feature params.
    pub fn used_ids(&self, inspector: &dyn FeatureInspector) -> BTreeSet<Id>;
    /// Structural problems (section 3.6) in a fixed order; empty for a structurally valid document.
    pub fn check_structure(&self) -> Vec<StructureError>;
    /// Structural problems plus reference, expression and parameter-graph problems (section 3.7).
    pub fn validate(&self, inspector: &dyn FeatureInspector) -> Vec<ValidationError>;
    /// Applies one edit and returns its exact inverse; on error the document is unchanged.
    pub fn apply(&mut self, edit: Edit) -> Result<Edit, EditError>;
    /// Applies edits in order, all or nothing, and returns each edit with its inverse.
    pub fn apply_all(&mut self, edits: Vec<Edit>) -> Result<Vec<Step>, TransactionError>;
    /// The edits that rename parameter `id` to `new_name` and rewrite every expression that uses it (section 3.13).
    pub fn rename_parameter_edits(&self, id: &Id, new_name: &str, inspector: &dyn FeatureInspector) -> Result<Vec<Edit>, EditError>;
    /// The edits that set the document units to `units`; with `keep_sizes`, also the edits that add the current unit to
    /// every unit-dependent bare number, so those sizes do not change (section 3.12).
    pub fn units_edits(&self, units: Units, keep_sizes: bool, inspector: &dyn FeatureInspector) -> Vec<Edit>;
    /// How many unit-dependent bare numbers the document has: the expressions `units_edits` with `keep_sizes` rewrites (section 3.12).
    pub fn bare_numbers(&self, inspector: &dyn FeatureInspector) -> usize;
    /// Evaluates every parameter (expressions.md section 3.13).
    pub fn evaluate_parameters(&self, context: &dyn EvaluationContext) -> ParameterValues;
    /// Rewrites every number inside feature params to its normalized form (file-format.md section 3.3).
    pub fn normalize(&mut self);
}

impl Default for Document {
    /// Same as `Document::new(Units::Mm)`.
    fn default() -> Document;
}

/// A named expression that other expressions use by name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    /// Permanent id.
    pub id: Id,
    /// Name used in expressions; a valid parameter name, unique in the document.
    pub name: String,
    /// Expression source text.
    pub expr: Expr,
    /// Free-text note shown next to the parameter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

impl Parameter {
    /// Creates a parameter without a comment.
    pub fn new(id: Id, name: impl Into<String>, expr: impl Into<Expr>) -> Parameter;
}

/// A declared solid body that features build into.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Body {
    /// Permanent id.
    pub id: Id,
    /// Display name; a valid label.
    pub name: String,
    /// Material name such as "PETG"; a valid label when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// Display and export color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Shown in the viewport; default true, omitted when true.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub visible: bool,
    /// Print role; default `Part`, omitted when `Part`.
    #[serde(default, skip_serializing_if = "BodyRole::is_part")]
    pub role: BodyRole,
}

impl Body {
    /// Creates a visible part body with no material and no color.
    pub fn new(id: Id, name: impl Into<String>) -> Body;
}

/// One timeline entry. Its params are stored generically and owned by the feature registry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FeatureRecord {
    /// Permanent id.
    pub id: Id,
    /// Feature type name such as "extrude"; serialized as "type".
    #[serde(rename = "type")]
    pub kind: String,
    /// Display name in the timeline; a valid label when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Skipped by recompute; default false, omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub suppressed: bool,
    /// Feature params, flattened into the record. Key order is significant (file-format.md section 3.2).
    #[serde(flatten)]
    pub params: serde_json::Map<String, serde_json::Value>,
}

impl FeatureRecord {
    /// Keys the record itself uses; params may not contain them.
    pub const RESERVED_KEYS: [&'static str; 4] = ["id", "type", "name", "suppressed"];
    /// Creates an unsuppressed, unnamed record with empty params.
    pub fn new(id: Id, kind: impl Into<String>) -> FeatureRecord;
}
```

`default_true`, `is_true` and `is_false` are private helpers. `Document`, `Parameter` and `Body` reject unknown keys when deserialized; `FeatureRecord` cannot (its params are flattened), so every key other than the four reserved ones goes into `params` in file order.

### 2.6 Names and labels

```rust
/// Longest parameter name, in characters.
pub const MAX_PARAMETER_NAME_LENGTH: usize = 64;
/// Longest label (body name, feature name, material, printer), in characters.
pub const MAX_LABEL_LENGTH: usize = 128;
/// Longest feature type name, in characters.
pub const MAX_FEATURE_TYPE_LENGTH: usize = 64;
/// Words that cannot be parameter names, in the order listed in section 3.3.
pub const RESERVED_NAMES: &[&str];

/// Checks a parameter name against section 3.3.
pub fn validate_parameter_name(name: &str) -> Result<(), NameError>;
/// Checks a label against section 3.4; `field` only selects the wording of the error.
pub fn validate_label(field: LabelField, text: &str) -> Result<(), LabelError>;
/// True when `kind` is a valid feature type name (section 3.5).
pub fn is_valid_feature_type(kind: &str) -> bool;

/// Which label a `LabelError` is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LabelField {
    /// `Body::name`, worded "Body name".
    BodyName,
    /// `FeatureRecord::name`, worded "Feature name".
    FeatureName,
    /// `Body::material`, worded "Material".
    Material,
    /// `Document::printer`, worded "Printer".
    Printer,
}
```

### 2.7 Feature inspection

The model cannot read feature params, so the registry tells it what a record declares and refers to.

```rust
/// What a feature record declares and refers to, as reported by the feature registry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FeatureLinks {
    /// Ids the params define, such as sketch entity and constraint ids, in params order.
    pub declared_ids: Vec<Id>,
    /// Bodies the feature writes, then bodies it reads, each in params order.
    pub bodies: Vec<Id>,
    /// Other features whose output the feature reads directly (sketches, datums), in params order.
    pub features: Vec<Id>,
    /// Every expression stored in the params, evaluated by build or not, in params order.
    pub expressions: Vec<ExprLink>,
}

/// One expression stored in feature params, with its location and value kind.
#[derive(Clone, Debug, PartialEq)]
pub struct ExprLink {
    /// JSON Pointer (RFC 6901) of the expression string within the params, for example "/extent/expr".
    pub pointer: String,
    /// The expression.
    pub expr: Expr,
    /// The value kind the feature evaluates it as (section 2.12), which says whether its bare numbers take the
    /// document units (`Length`).
    pub unit: FieldUnit,
}

/// Reports the links of a feature record; implemented by the feature registry.
pub trait FeatureInspector: Send + Sync {
    /// Returns the record's links, or a user-facing message saying why its params are invalid.
    fn inspect(&self, feature: &FeatureRecord) -> Result<FeatureLinks, String>;
}

/// Inspector that treats every record as having no links; used where no registry is available.
#[derive(Clone, Copy, Debug, Default)]
pub struct OpaqueFeatures;

impl FeatureInspector for OpaqueFeatures {
    /// Always `Ok(FeatureLinks::default())`.
    fn inspect(&self, feature: &FeatureRecord) -> Result<FeatureLinks, String>;
}
```

### 2.8 Structure and validation

```rust
/// A violation of the structural rules (section 3.6).
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum StructureError {
    /// `format` is not "partsmith".
    WrongFormat { found: String },
    /// `format_version` is not FORMAT_VERSION.
    WrongFormatVersion { found: u32 },
    /// An id appears on more than one parameter, body or feature; reported at each repeat.
    DuplicateId { id: Id },
    /// A parameter name breaks the name rules.
    InvalidParameterName { id: Id, error: NameError },
    /// A parameter name is already used by an earlier parameter.
    DuplicateParameterName { id: Id, name: String },
    /// A body name, material, feature name or the printer breaks the label rules; `id` is `None` for the printer.
    InvalidLabel { id: Option<Id>, error: LabelError },
    /// A feature type name breaks the type rules.
    InvalidFeatureType { id: Id, kind: String },
    /// Feature params contain a reserved key.
    ReservedParamKey { id: Id, key: String },
    /// `rollback` names no feature.
    UnknownRollback { id: Id },
}

/// A problem found by `Document::validate`.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ValidationError {
    /// A structural problem.
    Structure(StructureError),
    /// The inspector rejected the feature's params.
    InvalidFeature { feature: Id, message: String },
    /// An id declared inside feature params is already used elsewhere in the document.
    DuplicateDeclaredId { feature: Id, id: Id },
    /// The feature refers to a body that does not exist.
    UnknownBody { feature: Id, body: Id },
    /// The feature refers to a feature that does not exist.
    UnknownFeature { feature: Id, target: Id },
    /// The feature refers to itself.
    SelfReference { feature: Id },
    /// The feature refers to a feature later in the timeline.
    LaterFeature { feature: Id, target: Id },
    /// An expression of a parameter or feature does not parse.
    InvalidExpression { owner: Id, expr: Expr, error: ExprError },
    /// An expression uses a name that is not a parameter.
    UnknownParameter { owner: Id, name: String },
    /// Parameters that depend on each other in a loop, in document order.
    ParameterCycle { names: Vec<String> },
}
```

### 2.9 Edits and transactions

```rust
/// One change to a document. Every change goes through an Edit; applying one returns its exact inverse.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    /// Inserts a parameter at `index` (0 to the number of parameters).
    AddParameter { index: usize, parameter: Parameter },
    /// Replaces the parameter with the same id.
    UpdateParameter { parameter: Parameter },
    /// Removes a parameter.
    RemoveParameter { id: Id },
    /// Moves a parameter so that it ends up at `index`.
    MoveParameter { id: Id, index: usize },
    /// Inserts a body at `index`.
    AddBody { index: usize, body: Body },
    /// Replaces the body with the same id.
    UpdateBody { body: Body },
    /// Removes a body.
    RemoveBody { id: Id },
    /// Moves a body so that it ends up at `index`.
    MoveBody { id: Id, index: usize },
    /// Inserts a feature record at timeline position `index`.
    AddFeature { index: usize, feature: FeatureRecord },
    /// Replaces the feature record with the same id.
    UpdateFeature { feature: FeatureRecord },
    /// Removes a feature record.
    RemoveFeature { id: Id },
    /// Moves a feature record so that it ends up at timeline position `index`.
    MoveFeature { id: Id, index: usize },
    /// Sets the rollback marker; `None` means nothing is rolled back.
    SetRollback {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rollback: Option<Id>,
    },
    /// Sets the document units.
    SetUnits { units: Units },
    /// Sets or clears the printer profile id.
    SetPrinter {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        printer: Option<String>,
    },
}

/// An applied edit together with its inverse.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Step {
    /// The edit as applied.
    pub edit: Edit,
    /// The edit that undoes it.
    pub inverse: Edit,
}

/// A labeled group of steps that undo and redo as one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Transaction {
    /// Text shown as "Undo <label>", such as "Extrude".
    pub label: String,
    /// Steps in the order they were applied.
    pub steps: Vec<Step>,
}

/// Kind of record named in `EditError::NotFound`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RecordKind {
    /// Worded "parameter".
    Parameter,
    /// Worded "body".
    Body,
    /// Worded "feature".
    Feature,
}
```

`Edit` is read only from protocol requests, so every variant rejects unknown fields when deserialized (protocol.md section 2.3), while the records it carries follow section 2.5 (a `FeatureRecord`'s unknown keys are still params).

### 2.10 Undo log

```rust
/// Undo and redo stacks of transactions for one document. Holds no reference to the document.
#[derive(Clone, Debug, PartialEq)]
pub struct UndoLog { /* private */ }

impl UndoLog {
    /// Default maximum number of transactions kept for undo.
    pub const DEFAULT_MAX_DEPTH: usize = 1000;
    /// An empty log with the default maximum depth.
    pub fn new() -> UndoLog;
    /// An empty log keeping at most `max_depth` transactions (0 keeps none).
    pub fn with_max_depth(max_depth: usize) -> UndoLog;
    /// The maximum depth.
    pub fn max_depth(&self) -> usize;
    /// Applies `edits` to `document` as one transaction, all or nothing, and records it.
    pub fn commit(&mut self, document: &mut Document, label: impl Into<String>, edits: Vec<Edit>) -> Result<(), TransactionError>;
    /// Records a transaction whose steps the caller has already applied, in order, to the document.
    pub fn record(&mut self, transaction: Transaction);
    /// Undoes the newest transaction; `Ok(None)` when there is nothing to undo.
    pub fn undo(&mut self, document: &mut Document) -> Result<Option<&Transaction>, UndoError>;
    /// Redoes the newest undone transaction; `Ok(None)` when there is nothing to redo.
    pub fn redo(&mut self, document: &mut Document) -> Result<Option<&Transaction>, UndoError>;
    /// True when a transaction can be undone.
    pub fn can_undo(&self) -> bool;
    /// True when a transaction can be redone.
    pub fn can_redo(&self) -> bool;
    /// Label of the transaction `undo` would undo.
    pub fn undo_label(&self) -> Option<&str>;
    /// Label of the transaction `redo` would redo.
    pub fn redo_label(&self) -> Option<&str>;
    /// Number of transactions on the undo stack.
    pub fn undo_count(&self) -> usize;
    /// Number of transactions on the redo stack.
    pub fn redo_count(&self) -> usize;
    /// Drops both stacks.
    pub fn clear(&mut self);
}

impl Default for UndoLog {
    /// Same as `UndoLog::new()`.
    fn default() -> UndoLog;
}
```

### 2.11 JSON examples

Ids, units, colors and roles serialize as JSON strings:

```json
"f-3fa91c0e72b4"
"mm"
"#3b82f6"
"support_blocker"
```

Every `BodyRole` value: `"part"`, `"negative_volume"`, `"support_blocker"`, `"support_enforcer"`, `"settings_modifier"`.

`Parameter` (with and without comment):

```json
{"id": "p-wall", "name": "wall", "expr": "2 mm", "comment": "Outer wall thickness"}
{"id": "p-clear", "name": "clearance", "expr": "printer.fit.slip"}
```

`Body` (defaults omitted; a hidden support blocker):

```json
{"id": "b-1", "name": "Enclosure", "material": "PETG", "color": "#3b82f6"}
{"id": "b-2", "name": "Keep out", "color": "#ff0000", "visible": false, "role": "support_blocker"}
```

`FeatureRecord` (the four record keys first, then params in their stored order; the param names shown are illustrative, the feature specs own them):

```json
{"id": "f-ext1", "type": "extrude", "body": "b-1", "op": "new"}
{"id": "f-cyl1", "type": "cylinder", "name": "Screw hole", "suppressed": true, "body": "b-1", "op": "cut", "radius": "1.6 mm"}
```

`Document`: see file-format.md section 3.6 for complete, byte-exact canonical files.

`Edit`, one example per variant (the `type` tag comes first, then the variant's fields in declaration order):

```json
{"type": "add_parameter", "index": 0, "parameter": {"id": "p-wall", "name": "wall", "expr": "2 mm"}}
{"type": "update_parameter", "parameter": {"id": "p-wall", "name": "wall", "expr": "2.4 mm", "comment": "Outer wall"}}
{"type": "remove_parameter", "id": "p-wall"}
{"type": "move_parameter", "id": "p-wall", "index": 1}
{"type": "add_body", "index": 0, "body": {"id": "b-1", "name": "Enclosure", "material": "PETG", "color": "#3b82f6"}}
{"type": "update_body", "body": {"id": "b-1", "name": "Enclosure", "visible": false}}
{"type": "remove_body", "id": "b-1"}
{"type": "move_body", "id": "b-1", "index": 0}
{"type": "add_feature", "index": 1, "feature": {"id": "f-ext1", "type": "extrude", "body": "b-1", "op": "new"}}
{"type": "update_feature", "feature": {"id": "f-ext1", "type": "extrude", "suppressed": true, "body": "b-1", "op": "new"}}
{"type": "remove_feature", "id": "f-ext1"}
{"type": "move_feature", "id": "f-ext1", "index": 0}
{"type": "set_rollback", "rollback": "f-sk1"}
{"type": "set_rollback"}
{"type": "set_units", "units": "in"}
{"type": "set_printer", "printer": "generic-fdm"}
{"type": "set_printer"}
```

`{"type": "set_rollback"}` clears the marker (nothing is rolled back) and `{"type": "set_printer"}` clears the printer. On input, an explicit `null` for `rollback` or `printer` is accepted and means the same.

`Step` and `Transaction`, as written by the canonical writer (file-format.md section 3.5):

```json
{
  "label": "Add parameter wall",
  "steps": [
    {
      "edit": {"type": "add_parameter", "index": 0, "parameter": {"id": "p-wall", "name": "wall", "expr": "2 mm"}},
      "inverse": {"type": "remove_parameter", "id": "p-wall"}
    },
    {"edit": {"type": "set_rollback"}, "inverse": {"type": "set_rollback", "rollback": "f-ext1"}}
  ]
}
```

### 2.12 Feature manifests

A manifest describes a feature type for the UI, the schema and the registry. The types are pure data defined here, in the module `manifest` (re-exported from the crate root), so `partsmith-protocol` carries them typed and the UI gets generated TypeScript types (ADR 0011). `partsmith-features` re-exports them, each feature returns its manifest (features.md section 2.3), and each feature spec lists its complete manifest as a table.

```rust
/// Static description of a feature type for the UI, schema and registry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FeatureManifest {
    /// Display name, for example "Extrude". Also the base of default feature labels.
    pub label: String,
    /// Toolbar and palette grouping.
    pub category: Category,
    /// Icon name in the UI icon set, kebab-case, for example "linear-pattern".
    pub icon: String,
    /// Params version of this type, starting at 1. Bumped only with a document migration.
    pub version: u32,
    /// Inspector fields in display order.
    pub fields: Vec<FieldHint>,
}

/// Toolbar and palette group of a feature type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Sketches.
    Sketch,
    /// Features that add or remove material from scratch or from a sketch.
    Create,
    /// Features that change existing bodies.
    Modify,
    /// Mirrors and patterns.
    Pattern,
    /// Construction geometry such as datum planes.
    Construct,
}

/// How the inspector shows one params field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FieldHint {
    /// Path of the field inside params, for example "extent.expr" or "size[0]" (grammar in features.md section 3.2).
    pub path: String,
    /// Label shown next to the field, for example "Distance".
    pub label: String,
    /// Shown without the "More" toggle. Default false, omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub essential: bool,
    /// Value kind of an `Expr` field. Omitted for non-expression fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<FieldUnit>,
    /// What the viewport picks for a reference field. Omitted for typed fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<SelectionKind>,
    /// Labels for the values of an enumeration or a tag field, in display order. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<OptionHint>,
}

/// Value kind of an expression field, which sets its input unit, its expected dimension and its validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FieldUnit {
    /// A length in millimeters (expected dimension `LENGTH`).
    Length,
    /// An angle in radians (expected dimension `ANGLE`).
    Angle,
    /// A plain number (expected dimension `NONE`).
    Scalar,
    /// A plain non-negative whole number (expected dimension `NONE`).
    Count,
}

/// What a reference field accepts from the viewport or the browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SelectionKind {
    /// A body.
    Body,
    /// A sketch feature.
    Sketch,
    /// One or more closed regions of a sketch.
    SketchRegion,
    /// One or more points of a sketch (point entities, line and arc ends, arc and circle centers).
    SketchPoint,
    /// Faces of a body.
    Face,
    /// Edges of a body.
    Edge,
    /// An origin plane, a datum plane or a flat face.
    Plane,
    /// An origin axis, a sketch line, a straight edge, or a cylindrical, conical, spherical or toroidal face.
    Axis,
}

/// Display label for one value of an enumeration field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OptionHint {
    /// The serialized value, for example "two_sides".
    pub value: String,
    /// The label, for example "Two sides".
    pub label: String,
}
```

`FieldUnit` is also the value kind the features crate uses for expression uses, range errors and value formatting (features.md).

JSON form (the extrude manifest, as the host receives it in the protocol's feature catalog):

```json
{
  "label": "Extrude",
  "category": "create",
  "icon": "extrude",
  "version": 1,
  "fields": [
    {"path": "profile", "label": "Profile", "essential": true, "selection": "sketch_region"},
    {"path": "extent.kind", "label": "Extent", "essential": true, "options": [{"value": "distance", "label": "Distance"}, {"value": "symmetric", "label": "Symmetric"}, {"value": "two_sides", "label": "Two sides"}, {"value": "through_all", "label": "Through all"}]},
    {"path": "extent.expr", "label": "Distance", "essential": true, "unit": "length"},
    {"path": "extent.opposite_expr", "label": "Opposite distance", "essential": true, "unit": "length"},
    {"path": "op", "label": "Operation", "essential": true, "options": [{"value": "new", "label": "New body"}, {"value": "join", "label": "Join"}, {"value": "cut", "label": "Cut"}, {"value": "intersect", "label": "Intersect"}]},
    {"path": "reverse", "label": "Reverse"},
    {"path": "body", "label": "Body", "selection": "body"}
  ]
}
```

## 3. Behavior

### 3.1 Ids

- An id is valid when it has 1 to 64 characters and every character is an ASCII letter (`A-Z`, `a-z`), an ASCII digit, `-` or `_`. Equivalently it matches `^[A-Za-z0-9_-]{1,64}$`.
- `Id::new` checks, in this order: empty (`IdError::Empty`), longer than 64 characters (`IdError::TooLong`, length counted in Unicode scalar values), then the first character that is not allowed (`IdError::InvalidCharacter`, scanning left to right).
- Ids compare and sort by their bytes (`"B" < "a"`).
- Deserializing an invalid id fails with the `IdError` message.
- Hand-written ids such as `f-sk1` are valid. The prefix is a convention for generated ids only; no rule ties an id's prefix to its record kind.
- Ids are unique within a document across parameters, bodies, features and the ids declared inside feature params (section 3.7). Two records never share an id even when they are of different kinds.
- The model never assigns a removed record's id to a different record on its own. Generation checks only ids currently in use; the 48-bit random space makes reuse of a removed id negligible. Undo restores a removed record under its original id. `SequentialIds` may give a new record the id of one removed in an earlier session; it is meant for runs that rebuild a document from scratch.

### 3.2 Generating ids

`Id::generated(kind, value)` returns `kind.prefix()` followed by `format!("{:012x}", value & 0xFFFF_FFFF_FFFF)`: exactly 12 lowercase hex digits, zero-padded, from the low 48 bits. The result is always valid.

Callers of `IdGenerator::generate(kind, is_taken)` pass a predicate that rejects every id already in the document (`Document::used_ids`) plus any ids they generated in the same batch but have not inserted yet. Every generator returns a valid id that starts with `kind.prefix()` and for which `is_taken` is false, or an error.

`RandomIdGenerator::generate` tries up to `MAX_ID_ATTEMPTS` (64) candidates. Each candidate is 8 bytes from `getrandom`, read as a little-endian `u64` and passed to `Id::generated(kind, value)`, so only the low 48 bits matter. It returns the first candidate that is not taken. If the random source fails, it returns `IdError::RandomUnavailable`. After 64 taken candidates, it returns `IdError::Exhausted { attempts: 64 }`. The generator has no state.

`SequentialIds` keeps one counter per kind, each starting at 1. Each call to `generate` tries `Id::generated(kind, n)` for `n` equal to the kind's counter, then `n + 1`, and so on; it returns the first id that is not taken and sets that kind's counter to one past it. If `n` would exceed `0xFFFF_FFFF_FFFF`, it returns `IdError::Exhausted { attempts }` with the number of candidates tried in that call. So `SequentialIds::new().generate(IdKind::Feature, &|_| false)` returns `f-000000000001`, and a document that already has `f-000000000001` gets `f-000000000002`.

### 3.3 Parameter names

A parameter name is how expressions refer to the parameter, and the Python code generator uses it as a variable name (adding a suffix when it is a Python keyword, python-api.md section 3.8). `validate_parameter_name` checks, in this order, and returns the first failure:

1. Not empty (`NameError::Empty`).
2. At most 64 characters, counted in Unicode scalar values (`NameError::TooLong`).
3. The first character is an ASCII letter or `_` (`NameError::InvalidStart`).
4. Every other character is an ASCII letter, ASCII digit or `_` (`NameError::InvalidCharacter`, the first offending character).
5. Not in `RESERVED_NAMES` (`NameError::Reserved`). Matching is exact and case-sensitive (`Pi` and `MM` are allowed).

`RESERVED_NAMES`, in this order:

- The constant: `pi`.
- Expression units (expressions.md section 3.6): `um`, `mm`, `cm`, `m`, `in`, `ft`, `deg`, `rad`.
- Expression functions (expressions.md section 3.9): `abs`, `sqrt`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `min`, `max`, `floor`, `ceil`, `round`.
- The printer namespace: `printer`.

The constant has 24 entries, each listed once.

Parameter names are unique within a document (exact, case-sensitive comparison: `wall` and `Wall` may coexist).

### 3.4 Labels

Labels are display text: `Body::name`, `Body::material`, `FeatureRecord::name` and `Document::printer`. `validate_label` checks, in this order:

1. Contains at least one character that is not whitespace (`char::is_whitespace`); otherwise `LabelError::Empty` (this covers `""` and `"   "`).
2. At most 128 characters, counted in Unicode scalar values (`LabelError::TooLong`).
3. No control character (`char::is_control`, which includes tab, line feed and carriage return) (`LabelError::ControlCharacter`).

Leading and trailing spaces are allowed and preserved. Labels need not be unique: two bodies may share a name. `Parameter::comment` is free text with no rules.

### 3.5 Feature types and reserved keys

- `is_valid_feature_type(kind)` is true when `kind` has 1 to 64 characters, its first character is an ASCII lowercase letter, and every other character is an ASCII lowercase letter, ASCII digit or `_` (`^[a-z][a-z0-9_]{0,63}$`). Whether a valid type is registered is the registry's concern, reported through the inspector.
- `FeatureRecord::params` never contains `id`, `type`, `name` or `suppressed`. Deserialization cannot produce such keys (they bind to the record fields); records built in code are checked by `apply` and `check_structure`, and `save` refuses them (file-format.md).

### 3.6 Structural rules

A document is structurally valid when `check_structure()` is empty. `load` returns only structurally valid documents (or fails), `Document::new` returns one, and `apply` keeps a structurally valid document structurally valid. The rules:

1. `format == "partsmith"`.
2. `format_version == FORMAT_VERSION` (2).
3. `printer`, when present, is a valid label.
4. Parameter, body and feature ids are pairwise distinct.
5. Every parameter name is valid (section 3.3) and no two parameters share a name.
6. Every body name is a valid label; every material, when present, is a valid label.
7. Every feature type is valid (section 3.5); every feature name, when present, is a valid label; no params key is reserved.
8. `rollback`, when present, is the id of a feature in the document.

`check_structure` walks the document in this fixed order and reports every violation it meets:

1. `WrongFormat` (rule 1), then `WrongFormatVersion` (rule 2), then `InvalidLabel { id: None, .. }` for the printer (rule 3).
2. Parameters in order. For each: `DuplicateId` if its id appeared earlier in the walk; `InvalidParameterName` if its name is invalid; otherwise `DuplicateParameterName` if an earlier parameter has the same name.
3. Bodies in order. For each: `DuplicateId`; `InvalidLabel` for the name; `InvalidLabel` for the material.
4. Features in order. For each: `DuplicateId`; `InvalidFeatureType`; `InvalidLabel` for the name; one `ReservedParamKey` per reserved key, in params order.
5. `UnknownRollback` (rule 8).

"Appeared earlier in the walk" means among the ids of parameters, then bodies, then features already visited. The first occurrence of an id is never reported; every later occurrence is reported once.

Not structural (reported by `validate`, tolerated in memory): ids declared inside feature params, references from features to bodies and features, expression syntax, unknown parameter names, parameter cycles. Edits can make these temporarily wrong (for example removing a body a feature still uses); recompute reports them as feature errors and the user repairs them.

### 3.7 Validation

`validate(inspector)` returns `check_structure()` (each wrapped in `ValidationError::Structure`) followed by the problems below, in this order:

1. **Parameter expressions**, parameters in document order. Parse the expression (expressions.md). If it fails, report `InvalidExpression { owner: parameter id, expr, error }`. Otherwise, for each name in `Expr::references().parameters` (first-occurrence order) that is not the name of a parameter, report `UnknownParameter { owner: parameter id, name }`. Dotted names (`printer.fit.slip`) are not checked here: they depend on the evaluation context.
2. **Parameter cycles.** Build the graph with one node per parameter whose expression parses and an edge from each parameter to every existing parameter it names. Every strongly connected component with two or more members, or a single member with an edge to itself, is one `ParameterCycle { names }`, with `names` in document order. Cycles are reported in the document order of their first member.
3. **Features**, in timeline order. Call `inspector.inspect(feature)`.
   - On `Err(message)`, report `InvalidFeature { feature, message }` and skip the rest for this feature.
   - Otherwise, for each id in `declared_ids` (in order): report `DuplicateDeclaredId { feature, id }` if it equals a parameter, body or feature id, or an id declared earlier (in this feature's list or in an earlier feature's).
   - For each id in `bodies` (in order): `UnknownBody` if no body has it.
   - For each id in `features` (in order): `SelfReference` if it is the feature's own id; else `UnknownFeature` if no feature has it; else `LaterFeature` if that feature's timeline index is greater than this feature's.
   - For each link in `expressions` (in order): `InvalidExpression { owner: feature id, expr: link.expr, .. }` if its expression does not parse, else `UnknownParameter` for each unknown name as in step 1.

The inspector's own results are taken as given; the model does not deduplicate them. A document is valid when `validate` returns an empty list. `partsmith validate` (CLI spec) reports this list.

`used_ids(inspector)` returns the parameter, body and feature ids plus the `declared_ids` of every feature whose inspection succeeds.

### 3.8 Rollback and active features

- `rollback == None` means every feature is active (nothing is rolled back).
- `rollback == Some(id)` means the features before `id` are active; `id` and every later feature do not run (recompute.md).
- `active_features()` returns `&features[..i]`, where `i` is the index of the rollback feature, or `&features[..]` when `rollback` is `None` or names no feature.
- `Some(first feature id)` makes no feature active, so rolling back before the first feature is representable.
- A feature added at the rollback feature's index is active; a feature appended after the rollback feature is rolled back. The model never moves the marker on its own.
- Suppressed features are part of `active_features()`; recompute skips them.

### 3.9 Edits

`apply(edit)` checks the edit's preconditions in the order listed, returns the first failure as `EditError` with the document unchanged, and otherwise performs the change and returns the inverse.

| Edit | Preconditions, in order | Effect | Inverse returned |
| --- | --- | --- | --- |
| `AddParameter { index, parameter }` | id not used by any parameter, body or feature (`IdInUse`); name valid (`InvalidParameterName`); no parameter has the name (`NameInUse`); `index <= len` (`IndexOutOfRange { max: len }`) | Inserts at `index` | `RemoveParameter { id }` |
| `UpdateParameter { parameter }` | a parameter has `parameter.id` (`NotFound`); name valid; no other parameter has the name | Replaces it in place | `UpdateParameter { previous record }` |
| `RemoveParameter { id }` | exists (`NotFound`) | Removes it | `AddParameter { index: its index, parameter: its record }` |
| `MoveParameter { id, index }` | exists; `index <= len - 1` (`IndexOutOfRange { max: len - 1 }`) | Removes it, then inserts it at `index` | `MoveParameter { id, index: its previous index }` |
| `AddBody { index, body }` | id unused (`IdInUse`); name is a valid label; material, when present, is a valid label; `index <= len` | Inserts at `index` | `RemoveBody { id }` |
| `UpdateBody { body }` | exists; name and material labels valid | Replaces it in place | `UpdateBody { previous record }` |
| `RemoveBody { id }` | exists | Removes it | `AddBody { index, body }` |
| `MoveBody { id, index }` | exists; `index <= len - 1` | Moves it | `MoveBody { id, previous index }` |
| `AddFeature { index, feature }` | id unused (`IdInUse`); type valid (`InvalidFeatureType`); name, when present, a valid label; no reserved params key (`ReservedParamKey`, the first in params order); `index <= len` | Normalizes numbers in params, inserts at `index` | `RemoveFeature { id }` |
| `UpdateFeature { feature }` | exists; type valid; name valid; no reserved key | Normalizes numbers, replaces it in place | `UpdateFeature { previous record }` |
| `RemoveFeature { id }` | exists; not the rollback feature (`RemovesRollbackFeature`) | Removes it | `AddFeature { index, feature }` |
| `MoveFeature { id, index }` | exists; `index <= len - 1` | Moves it | `MoveFeature { id, previous index }` |
| `SetRollback { rollback }` | when `Some(id)`, a feature has `id` (`UnknownRollbackFeature`) | Sets `rollback` | `SetRollback { previous value }` |
| `SetUnits { units }` | none | Sets `units` | `SetUnits { previous value }` |
| `SetPrinter { printer }` | when `Some`, a valid label (`InvalidLabel` with `LabelField::Printer`) | Sets `printer` | `SetPrinter { previous value }` |

Rules that apply to every edit:

- `len` is the length of the list the edit targets, before the edit. For a move, `index` is the record's position after the move: moving `a` in `[a, b, c]` to index 2 gives `[b, c, a]`. Moving a record to its current index is valid and changes nothing.
- An update finds its target by the record's `id`; ids cannot be changed by an update. Any other field may change, including a feature's `type`.
- No-op edits (an update to an identical record, a set to the current value) succeed and return an inverse equal to the edit.
- Numbers in feature params are normalized on the way in (file-format.md section 3.3), so a document stays normalized.
- `apply` never checks references from features or expressions: removing a body or parameter that something still uses succeeds. `apply` never rewrites expressions; `rename_parameter_edits` (section 3.13) builds the rename transaction, which the engine's `rename_parameter` command commits.
- `AddFeature` and `MoveFeature` never move the rollback marker; an `AddFeature` at the rollback feature's index is active without a `SetRollback`.
- `apply` does not check ids declared inside params; generators avoid them through `used_ids`, and `validate` reports collisions.

**Inverse guarantee.** If `doc.check_structure()` is empty and `doc.apply(e)` returns `Ok(inv)`, then `doc.check_structure()` is still empty, and applying `inv` succeeds, restores a document equal to the original, and returns an edit equal to `e` after number normalization. Every precondition in the table is a property of the resulting document, so the inverse always meets them.

### 3.10 Transactions

`apply_all(edits)` applies the edits in order. If edit `i` fails, it applies the inverses of edits `i - 1` down to `0`, in that order, and returns `TransactionError { index: i, count: edits.len(), error }` with the document equal to what it was before the call. Those rollback applications cannot fail by the inverse guarantee; the implementation uses `expect` with that invariant stated. On success it returns one `Step { edit, inverse }` per edit, in order, where `edit` is the edit as applied (with normalized numbers). An empty list succeeds with no steps.

### 3.11 Undo log

- `commit(document, label, edits)`:
  - If `edits` is empty, it does nothing and returns `Ok(())`; both stacks keep their contents.
  - Otherwise it calls `document.apply_all(edits)`. On error it returns the error; the document and both stacks are unchanged (the redo stack is kept).
  - On success it pushes `Transaction { label, steps }` onto the undo stack, clears the redo stack, and, while the undo stack holds more than `max_depth` transactions, discards the oldest. With `max_depth == 0` nothing is kept: the edits are applied, the redo stack is cleared, and `can_undo()` stays false.
- `record(transaction)` is for callers that apply edits over several calls before closing a transaction (the engine's open transactions with live preview). If `transaction.steps` is empty it does nothing. Otherwise it pushes the transaction, clears the redo stack and trims to `max_depth`, exactly as a successful `commit` does. The caller guarantees the steps were applied to the document in order through `Document::apply` or `apply_all`; the log does not check.
- `undo(document)`:
  - With an empty undo stack it returns `Ok(None)`.
  - Otherwise it applies the inverses of the newest transaction's steps in reverse step order, as one `apply_all`. On success it moves the transaction to the redo stack and returns it.
  - If an inverse fails (possible only when the document was changed outside this log), it returns `UndoError::Undo { label, error }`; the document and both stacks are unchanged.
- `redo(document)` mirrors `undo`: it applies the stored `edit` of each step in step order, moves the transaction back to the undo stack (without clearing the redo stack below it), and returns it. Failure gives `UndoError::Redo` with nothing changed.
- `undo_label` and `redo_label` return the label of the transaction on top of each stack. Labels are free text; an empty label is allowed.
- `clear()` empties both stacks and keeps `max_depth`.
- The log is independent of saving: saving a document does not clear its log (technical design section 13: undo history survives a save).
- Guarantee: if every change to a structurally valid document goes through one log's `commit`, `record` (as described), `undo` and `redo`, then every `undo` and `redo` succeeds, and after `k` undos the document equals its state before the last `k` commits.

### 3.12 Changing units

`SetUnits` changes only the `units` field. Because bare numbers in length expressions take the document units (expressions.md section 3.10), changing units changes the geometry of every expression that relies on them: `"30"` in a length field is 30 mm under `mm` and 762 mm under `in`. JSON numbers stored directly in params (such as sketch coordinates) are internal millimeters and radians and do not change.

`units_edits(units, keep_sizes, inspector)` returns the edits that change the units, as one transaction for `UndoLog::commit`; the engine's `set_document_units` commits it labeled `Change units to {units}` (engine.md section 3.25). It does not change the document.

1. When `units` equals the current units, it returns no edits, whatever `keep_sizes` is.
2. Otherwise the first edit is `SetUnits { units }`. Without `keep_sizes`, that is the only edit.
3. With `keep_sizes`, it continues with the edits that rewrite every unit-dependent bare number (below) to `Expr::with_unit(self.units.unit())` (expressions.md section 3.17), so it carries the current, old unit: `40` becomes `40 mm`, ` 20 ` becomes ` 20 mm `, `-2` becomes `-2 mm`. First one `UpdateParameter` per such parameter, in document order; then, in timeline order, one `UpdateFeature` per feature record with at least one such expression, a copy of the record with each such expression string replaced at its link's `pointer` (a link whose pointer does not lead to a JSON string equal to its `expr` is left alone, as in section 3.13).

**Unit-dependent bare numbers**, which `bare_numbers(inspector)` counts, are the expressions that are bare numbers (`Expr::is_bare_number`: a number without a unit, optionally negated, without parentheses) and whose value the document units decide:

- a feature expression whose link has `unit: FieldUnit::Length`;
- a parameter used as a length only. A link *reaches* every parameter its expression names, and, recursively, every parameter named by the expression of a parameter it reaches; each way from the link's expression to the parameter, through the expressions of the parameters in between, is a *path*. A parameter counts when at least one `Length` link reaches it, no link with another unit (`Angle`, `Scalar`, `Count`) does, and on every path from a `Length` link each name is in `Expr::promoted_names` of the expression that names it (expressions.md section 3.19): the parameter is a size wherever it is used, not a factor, a ratio or an argument. A parameter that a count or an angle also uses has no single unit that keeps all its uses, a parameter that only multiplies a length (`40 mm * scale`) would turn it into an area, and a parameter nothing uses has no size to keep, so all of them are left alone.

Only links of feature records whose inspection succeeds take part, in every record (suppressed and rolled-back ones included). Expressions that do not parse and dotted names are never rewritten or followed. Applying the edits and undoing them restores the document exactly (section 3.11).

Keep sizes rewrites bare numbers only, which is what hosts announce with the count ("add mm to the 4 numbers without a unit", ui.md). A number inside a larger expression (the `0.5` of `wall + 0.5`) or a plain ratio still takes the new units (open issue 8). Because a parameter is rewritten only where it is promoted as a size, Keep sizes never breaks an expression that switching the units alone would not: a parameter that an expression squares (`wall * wall`) or uses as a factor (`40 mm * scale`) keeps its bare number.

### 3.13 Renaming a parameter

`rename_parameter_edits(id, new_name, inspector)` returns the edits that rename one parameter and rewrite every expression that uses its old name, as one transaction for `UndoLog::commit`. It does not change the document.

1. Errors, checked in this order: no parameter has `id` (`NotFound { kind: Parameter, id }`); `new_name` is not a valid parameter name (`InvalidParameterName`); another parameter already has `new_name` (`NameInUse`). Renaming a parameter to its current name returns `Ok(vec![])`.
2. The first edit is an `UpdateParameter` of the renamed parameter with name `new_name` and its own expression rewritten through `Expr::rename_parameter(old, new_name)` (expressions.md section 3.15).
3. Then one `UpdateParameter` for every other parameter whose expression changes under the same rewrite, in document order.
4. Then, in timeline order, one `UpdateFeature` per feature record whose inspection succeeds and at least one of whose `ExprLink`s changes. The record is copied and each changed expression string is replaced at its `pointer` within the params; a link whose pointer does not lead to a JSON string equal to its `expr` is left alone.
5. Expressions that do not parse, and records whose inspection fails, stay unchanged.

Every edit keeps ids, other names, types and keys, so `apply_all` of the result always succeeds on the same document.

## 4. Errors

Each message is the error's `Display`. Placeholders in braces are filled from the variant's fields; `{field}` is the `LabelField` wording ("Body name", "Feature name", "Material", "Printer"); `{kind}` for `RecordKind` is "parameter", "body" or "feature"; `{character}` is the character as written by `char::escape_debug` (the templates supply the surrounding single quotes).

### 4.1 `IdError`

| Variant | Message |
| --- | --- |
| `Empty` | `An id cannot be empty.` |
| `TooLong { length: usize }` | `This id is {length} characters long; ids can be at most 64 characters.` |
| `InvalidCharacter { id: String, character: char }` | `The id '{id}' contains '{character}'. Ids use only letters A to Z and a to z, digits, - and _.` |
| `RandomUnavailable` | `Could not create a new id because the system random number source is unavailable.` |
| `Exhausted { attempts: usize }` | `Could not find an unused id after {attempts} attempts. The id generator keeps producing ids that are already taken.` |

### 4.2 `UnitsError` and `ColorError`

| Variant | Message |
| --- | --- |
| `UnitsError::Unknown { text: String }` | `'{text}' is not a document unit. Use mm, cm, m or in.` |
| `ColorError::Invalid { text: String }` | `'{text}' is not a color. Use # followed by six hex digits, such as #3b82f6.` |

### 4.3 `NameError` (parameter names)

| Variant | Message |
| --- | --- |
| `Empty` | `A parameter name cannot be empty.` |
| `TooLong { name: String, length: usize }` | `The parameter name '{name}' is {length} characters long; the limit is 64.` |
| `InvalidStart { name: String }` | `The parameter name '{name}' must start with a letter or _.` |
| `InvalidCharacter { name: String, character: char }` | `The parameter name '{name}' contains '{character}'. Use only letters, digits and _, as in wall_2.` |
| `Reserved { name: String }` | `'{name}' is a reserved word. Pick another name, such as {name}_1.` |

### 4.4 `LabelError`

| Variant | Message |
| --- | --- |
| `Empty { field: LabelField }` | `{field} cannot be empty.` |
| `TooLong { field: LabelField, length: usize }` | `{field} is {length} characters long; the limit is 128.` |
| `ControlCharacter { field: LabelField }` | `{field} contains a control character such as a tab or line break. Remove it.` |

### 4.5 `StructureError`

| Variant | Message |
| --- | --- |
| `WrongFormat { found }` | `The format field is '{found}', but a Partsmith document needs 'partsmith'.` |
| `WrongFormatVersion { found }` | `The document is at format version {found}, but documents in memory must be at version 2. Load it from text so it is migrated.` (the number is `FORMAT_VERSION`) |
| `DuplicateId { id }` | `The id {id} is used by more than one record. Give the later record a new id.` |
| `InvalidParameterName { id, error }` | `Parameter {id}: {error}` |
| `DuplicateParameterName { id, name }` | `Parameter {id} is named {name}, but an earlier parameter already uses that name. Rename one of them.` |
| `InvalidLabel { id: Some(id), error }` | `{id}: {error}` |
| `InvalidLabel { id: None, error }` | `{error}` |
| `InvalidFeatureType { id, kind }` | `Feature {id} has the type '{kind}', which is not a valid type name. Type names use lowercase letters, digits and _, and start with a letter.` |
| `ReservedParamKey { id, key }` | `Feature {id} has a parameter named '{key}', which the record itself uses. Rename that parameter.` |
| `UnknownRollback { id }` | `The rollback marker points at {id}, which is not a feature in this document. Move the marker to an existing feature or to the end.` |

### 4.6 `ValidationError`

| Variant | Message |
| --- | --- |
| `Structure(error)` | `{error}` |
| `InvalidFeature { feature, message }` | `Feature {feature}: {message}` |
| `DuplicateDeclaredId { feature, id }` | `Feature {feature} defines the id {id}, which is already used elsewhere in the document. Give it a new id.` |
| `UnknownBody { feature, body }` | `Feature {feature} uses body {body}, which does not exist. Pick an existing body or add one.` |
| `UnknownFeature { feature, target }` | `Feature {feature} refers to {target}, which does not exist. Pick another input or remove the reference.` |
| `SelfReference { feature }` | `Feature {feature} refers to itself. Remove that reference.` |
| `LaterFeature { feature, target }` | `Feature {feature} refers to {target}, which comes after it in the timeline. Move {target} above {feature}.` |
| `InvalidExpression { owner, expr, error }` | `{owner}: the expression '{expr}' is invalid. {error}` |
| `UnknownParameter { owner, name }` | `{owner} uses '{name}', but no parameter has that name. Add the parameter or fix the spelling.` |
| `ParameterCycle { names }` with one name | `Parameter {name} depends on itself. Change its expression so it does not use {name}.` |
| `ParameterCycle { names }` with two or more | `Parameters {list} depend on each other in a loop. Change one of them so it does not use the others.` |

`{list}` joins names with `, ` and puts ` and ` before the last: `a and b`, `a, b and c`.

### 4.7 `EditError`

| Variant | Message |
| --- | --- |
| `NotFound { kind: RecordKind, id: Id }` | `There is no {kind} with the id {id}.` |
| `IdInUse { id: Id }` | `The id {id} is already used in this document. Use a new id.` |
| `InvalidParameterName(NameError)` | `{error}` |
| `NameInUse { name: String }` | `A parameter named {name} already exists. Pick another name.` |
| `InvalidLabel(LabelError)` | `{error}` |
| `InvalidFeatureType { kind: String }` | `'{kind}' is not a valid feature type. Type names use lowercase letters, digits and _, and start with a letter.` |
| `ReservedParamKey { key: String }` | `Feature parameters cannot use the key '{key}', because the record itself uses it.` |
| `IndexOutOfRange { index: usize, max: usize }` | `Position {index} is out of range. Use a position from 0 to {max}.` |
| `RemovesRollbackFeature { id: Id }` | `Feature {id} holds the rollback marker. Move the rollback marker before removing it.` |
| `UnknownRollbackFeature { id: Id }` | `Cannot roll back to {id} because there is no feature with that id.` |

### 4.8 `TransactionError` and `UndoError`

| Variant | Message |
| --- | --- |
| `TransactionError { index, count, error }` | `Change {index + 1} of {count} could not be applied: {error}` |
| `UndoError::Undo { label: String, error: TransactionError }` | `Cannot undo {label}: the document no longer matches the undo history. {error}` |
| `UndoError::Redo { label: String, error: TransactionError }` | `Cannot redo {label}: the document no longer matches the undo history. {error}` |

Expression errors (`ExprError`) are in expressions.md section 4; load, save, migration, canonical writer and schema errors in file-format.md section 4.

## 5. Performance budgets

Measured as `cargo bench` style benchmarks on the reference desktop (2022 mid-range laptop, release build); web budgets are 3 times the desktop numbers. These are CI benchmarks, not acceptance tests.

| Operation | Desktop budget |
| --- | --- |
| `apply` of one edit on a document with 1,000 features (each with 10 params) | 50 microseconds |
| `apply_all` of 100 edits, and `undo` and `redo` of that transaction | 2 ms each |
| `check_structure` on a document with 1,000 parameters, 100 bodies and 1,000 features | 2 ms |
| `validate` with `OpaqueFeatures` on the same document | 10 ms |
| `used_ids` on the same document with `OpaqueFeatures` | 1 ms |
| `SequentialIds::generate` with a `BTreeSet` predicate of 10,000 ids | 2 microseconds |
| `units_edits(In, true, &TestInspector)` and `bare_numbers` on a document with 1,000 parameters and 1,000 features of 10 `lengths` each (section 6) | 10 ms each |

## 6. Acceptance tests

Conventions: `id("x")` means `Id::new("x").unwrap()`. Unless a test says otherwise, documents start from `Document::new(Units::Mm)`.

**Fixture D1** (structurally valid):

- parameters: `p-wall` named `wall` with expr `2 mm`; `p-h` named `height` with expr `10 * wall`.
- bodies: `b-1` named `Base`.
- features, in order: `f-sk1` of type `sketch` with params `{}`; `f-ext1` of type `extrude` with params `{"body": "b-1"}`; `f-fil1` of type `fillet` with params `{"body": "b-1"}`.
- `rollback`: `None`.

**TestInspector** (written in the test crate, implementing `FeatureInspector`): if params contains the key `invalid`, returns `Err("Bad params.")`. Otherwise returns `FeatureLinks` with `bodies` from the string param `body` (if present), `features` from the string array `uses`, `declared_ids` from the string array `declares`, and `expressions` from the string array `exprs` (unit `Scalar`), then from the string arrays `lengths`, `angles` and `counts` (units `Length`, `Angle` and `Count`), each with pointer `/<key>/<index>`, such as `/exprs/0` or `/lengths/2` (each absent key gives an empty list).

### Ids

1. `Id::new("f-3fa91c0e72b4")`, `Id::new("f-sk1")` and `Id::new("A_Z-09")` succeed, and `as_str()` returns the input.
2. `Id::new("")` returns `Err(IdError::Empty)`.
3. `Id::new("a".repeat(64))` succeeds; `Id::new("a".repeat(65))` returns `Err(IdError::TooLong { length: 65 })`.
4. `Id::new("f 1")` returns `Err(IdError::InvalidCharacter { id: "f 1", character: ' ' })`; `Id::new("f.1")` reports `'.'`; `Id::new("é1")` reports `'é'`.
5. `serde_json::to_string(&id("f-1"))` is `"\"f-1\""`; `serde_json::from_str::<Id>("\"f 1\"")` fails and its message contains `The id 'f 1' contains ' '`.
6. `id("B") < id("a")` and `id("f-10") < id("f-9")`.
7. `Id::generated(IdKind::Feature, 0x3fa91c0e72b4)` is `f-3fa91c0e72b4`; `Id::generated(IdKind::Parameter, 0)` is `p-000000000000`; `Id::generated(IdKind::Body, 0xFFFF_0000_0000_0001)` is `b-000000000001`.
8. `IdKind::ALL.map(IdKind::prefix)` is `["p-", "b-", "f-", "e-", "c-"]`.
9. `SequentialIds::new()` called with `IdKind::Feature`, `Feature`, `Body` and `SketchEntity` in turn (with `is_taken` always false) returns `f-000000000001`, `f-000000000002`, `b-000000000001` and `e-000000000001`.
10. With `is_taken` true exactly for `f-000000000001` and `f-000000000002`, the first two `IdKind::Feature` ids of `SequentialIds::new()` are `f-000000000003` and `f-000000000004`.
11. Two `SequentialIds::new()` generators give identical sequences of 1,000 ids when both cycle through `IdKind::ALL`.
12. A clone of a `SequentialIds` that has generated three ids continues exactly like its original for the next 10 ids.
13. `RandomIdGenerator::new().generate(IdKind::Feature, ..)` with `is_taken` always true returns `Err(IdError::Exhausted { attempts: 64 })` after exactly 64 calls of the predicate.
14. 1,000 ids from `SequentialIds::new()`, cycling through `IdKind::ALL`, all pass `Id::new` and match `^[pbfec]-[0-9a-f]{12}$`.
15. `&mut SequentialIds` and `&mut RandomIdGenerator` both coerce to `&mut dyn IdGenerator`, and an id generated through each trait object passes `Id::new`.
16. `RandomIdGenerator::new()` generates 10,000 feature ids (with `is_taken` rejecting ids already produced); all match `^f-[0-9a-f]{12}$` and all are distinct.

### Units and colors

17. `Units` serializes as `"mm"`, `"cm"`, `"m"`, `"in"`; deserializing `"MM"` or `"inch"` fails; `Units::default()` is `Units::Mm`.
18. `Units::millimeters` returns exactly `1.0`, `10.0`, `1000.0`, `25.4` for `Mm`, `Cm`, `M`, `In`; `"in".parse::<Units>()` is `Ok(Units::In)`; `"ft".parse::<Units>()` is `Err(UnitsError::Unknown { text: "ft" })`.
19. `"#3B82F6".parse::<Color>()` is `Color::new(0x3b, 0x82, 0xf6)` and displays as `#3b82f6`; `serde_json::from_str::<Color>("\"#3B82F6\"")` then `to_string` gives `"\"#3b82f6\""`.
20. `"#12345"`, `"3b82f6"`, `"#3b82fg"`, `"#3b82f6ff"` and `""` all fail to parse as `Color` with `ColorError::Invalid`.
21. `serde_json::to_value(BodyRole::SettingsModifier)` is `"settings_modifier"`; `BodyRole::default()` is `Part`.

### Names and labels

22. `validate_parameter_name` accepts `wall`, `_x`, `wall_2`, `W`, `Pi`, `MM`, `mm2`, `in_1`, `class`, `None` and a 64-character name of `a`.
23. `validate_parameter_name("")` is `Err(NameError::Empty)`; a 65-character name gives `TooLong { length: 65, .. }`.
24. `"2x"` gives `InvalidStart`; `"wall-2"` gives `InvalidCharacter { character: '-', .. }`; `"wäll"` gives `InvalidCharacter { character: 'ä', .. }`.
25. Each of `pi`, `mm`, `in`, `deg`, `rad`, `um`, `ft`, `sqrt`, `atan2`, `round`, `printer` gives `NameError::Reserved`; `RESERVED_NAMES.len()` is 24 with no duplicates.
26. `NameError::Reserved { name: "pi".into() }.to_string()` is `'pi' is a reserved word. Pick another name, such as pi_1.`
27. `validate_label(LabelField::BodyName, "Enclosure")` and `" Lid "` succeed; `""` and `"   "` give `Empty`; 128 characters succeed; 129 give `TooLong { length: 129, .. }`; `"Lid\n"` and `"a\tb"` give `ControlCharacter`.
28. `LabelError::Empty { field: LabelField::BodyName }.to_string()` is `Body name cannot be empty.`
29. `is_valid_feature_type` is true for `extrude`, `linear_pattern`, `a1`, and false for `""`, `Extrude`, `1box`, `my-feature`, `_box`, and a 65-character name.

### Document basics

30. `Document::new(Units::In)` has `format == "partsmith"`, `format_version == 2`, `units == In`, empty lists, `printer == None`, `rollback == None`, and `check_structure()` is empty. `Document::default().units == Units::Mm`.
31. On D1: `parameter(&id("p-h"))` has name `height`; `parameter_by_name("wall")` has id `p-wall`; `parameter_by_name("Wall")` is `None`; `body(&id("b-1"))` is `Some`; `feature_index(&id("f-fil1"))` is `Some(2)`; `feature(&id("f-x"))` is `None`; `contains_id(&id("b-1"))` is true.
32. D1 with `rollback = Some(f-ext1)`: `active_features()` has ids `[f-sk1]`; with `Some(f-sk1)` it is empty; with `None` it has all three; with `Some(f-missing)` (built directly) it has all three.
33. `used_ids(&TestInspector)` on D1 where `f-sk1` params are `{"declares": ["e-1", "c-1"]}` is `{b-1, c-1, e-1, f-ext1, f-fil1, f-sk1, p-h, p-wall}`.

### Structure

34. D1 with body `b-1` renamed to id `p-wall` (so a parameter and a body share it): `check_structure()` is exactly `[DuplicateId { id: p-wall }]`.
35. D1 plus a third parameter `p-3` named `wall`: `[DuplicateParameterName { id: p-3, name: "wall" }]`.
36. D1 with `p-h` renamed to `2h`: `[InvalidParameterName { id: p-h, error: InvalidStart { name: "2h" } }]`.
37. D1 with body name `"  "`: `[InvalidLabel { id: Some(b-1), error: Empty { field: BodyName } }]`; with material `Some("")` instead: `[InvalidLabel { id: Some(b-1), error: Empty { field: Material } }]`.
38. D1 with `f-ext1.kind = "Extrude"`: `[InvalidFeatureType { id: f-ext1, kind: "Extrude" }]`.
39. D1 with `f-ext1.params` containing `"suppressed": true` then `"name": "x"` (in that order, built in code): `[ReservedParamKey { id: f-ext1, key: "suppressed" }, ReservedParamKey { id: f-ext1, key: "name" }]`.
40. D1 with `rollback = Some(f-x)`: `[UnknownRollback { id: f-x }]`.
41. D1 with `format = "other"`, `format_version = 3`, `printer = Some("")`, `p-h` named `wall`, body named `""`, and `rollback = Some(f-x)`: `check_structure()` is exactly `[WrongFormat { found: "other" }, WrongFormatVersion { found: 3 }, InvalidLabel { id: None, error: Empty { field: Printer } }, DuplicateParameterName { id: p-h, name: "wall" }, InvalidLabel { id: Some(b-1), error: Empty { field: BodyName } }, UnknownRollback { id: f-x }]`.

### Validation

42. `D1.validate(&OpaqueFeatures)` and `D1.validate(&TestInspector)` are empty.
43. D1 with `p-h` expr `10 * ` gives exactly one error, `InvalidExpression { owner: p-h, expr: "10 * ", .. }` whose inner error kind is `UnexpectedEnd`.
44. D1 with `p-h` expr `10 * wal + depth` gives `[UnknownParameter { owner: p-h, name: "wal" }, UnknownParameter { owner: p-h, name: "depth" }]`.
45. D1 with `p-wall` expr `height / 10` gives `[ParameterCycle { names: ["wall", "height"] }]` (one entry). With `p-wall` expr `wall + 1` instead: `[ParameterCycle { names: ["wall"] }]`.
46. D1 with `p-wall` expr `printer.fit.slip` validates with no errors.
47. With `TestInspector`, D1 with `f-ext1` params `{"invalid": true}` gives `[InvalidFeature { feature: f-ext1, message: "Bad params." }]`.
48. With `TestInspector`, D1 with `f-ext1` params `{"body": "b-2"}` gives `[UnknownBody { feature: f-ext1, body: b-2 }]`.
49. With `TestInspector`, D1 with `f-ext1` params `{"body": "b-1", "uses": ["f-sk1", "f-zz", "f-ext1", "f-fil1"]}` gives `[UnknownFeature { feature: f-ext1, target: f-zz }, SelfReference { feature: f-ext1 }, LaterFeature { feature: f-ext1, target: f-fil1 }]`.
50. With `TestInspector`, D1 with `f-sk1` params `{"declares": ["e-1", "p-wall"]}` and `f-ext1` params `{"body": "b-1", "declares": ["e-1"]}` gives `[DuplicateDeclaredId { feature: f-sk1, id: p-wall }, DuplicateDeclaredId { feature: f-ext1, id: e-1 }]`.
51. With `TestInspector`, D1 with `f-ext1` params `{"body": "b-1", "exprs": ["2 * wall", "depth + 1", "3 +"]}` gives `[UnknownParameter { owner: f-ext1, name: "depth" }, InvalidExpression { owner: f-ext1, expr: "3 +", .. }]`.
52. Order across sections: D1 with body name `""`, `p-h` expr `wal`, and (with `TestInspector`) `f-ext1` params `{"body": "b-9"}` gives `[Structure(InvalidLabel { .. }), UnknownParameter { owner: p-h, name: "wal" }, UnknownBody { feature: f-ext1, body: b-9 }]`.

### Edits

53. On an empty document, `apply(AddParameter { index: 0, parameter: wall })` returns `Ok(RemoveParameter { id: p-wall })` and `parameters == [wall]`.
54. On D1, `AddParameter { index: 3, .. }` with a new id and name returns `Err(IndexOutOfRange { index: 3, max: 2 })`; with `index: 2` it appends.
55. On D1, `AddParameter` with id `b-1` returns `Err(IdInUse { id: b-1 })`; with a new id and name `pi`, `Err(InvalidParameterName(Reserved { name: "pi" }))`; with name `wall`, `Err(NameInUse { name: "wall" })`.
56. On D1, `UpdateParameter { p-h named "height", expr "12 * wall" }` returns `Ok(UpdateParameter { p-h named "height", expr "10 * wall" })`; updating `p-h` to name `wall` returns `Err(NameInUse { name: "wall" })`; updating `p-zz` returns `Err(NotFound { kind: RecordKind::Parameter, id: p-zz })`.
57. On D1, `RemoveParameter { id: p-wall }` returns `Ok(AddParameter { index: 0, parameter: <wall record> })` and leaves `parameters == [height]` (no reference check, although `height` uses `wall`).
58. On a document with parameters `[a, b, c]` (ids `p-a`, `p-b`, `p-c`), `MoveParameter { id: p-a, index: 2 }` gives order `[b, c, a]` and returns `MoveParameter { id: p-a, index: 0 }`; `MoveParameter { id: p-a, index: 3 }` returns `Err(IndexOutOfRange { index: 3, max: 2 })`.
59. On D1, `AddBody` with name `"\n"` returns `Err(InvalidLabel(ControlCharacter { field: BodyName }))`; `UpdateBody` of `b-1` with `material: Some("")` returns `Err(InvalidLabel(Empty { field: Material }))`.
60. On D1, `AddFeature` whose params contain `"id": "x"` returns `Err(ReservedParamKey { key: "id" })`; with type `Extrude`, `Err(InvalidFeatureType { kind: "Extrude" })`; with name `Some("")`, `Err(InvalidLabel(Empty { field: FeatureName }))`.
61. `AddFeature` of a record whose params are `{"depth": 80.0}` built with `serde_json::Value::from(80.0_f64)` stores the value equal to `serde_json::json!(80)` (an integer `Number`), and the returned inverse is `RemoveFeature`.
62. On D1 with `rollback = Some(f-ext1)`, `RemoveFeature { id: f-ext1 }` returns `Err(RemovesRollbackFeature { id: f-ext1 })`; `RemoveFeature { id: f-fil1 }` returns `Ok(AddFeature { index: 2, feature: <f-fil1 record> })`.
63. On D1, `SetRollback { rollback: Some(f-x) }` returns `Err(UnknownRollbackFeature { id: f-x })`; `SetRollback { rollback: Some(f-ext1) }` returns `Ok(SetRollback { rollback: None })`.
64. On D1, `SetUnits { units: In }` returns `Ok(SetUnits { units: Mm })`; `SetPrinter { printer: Some("generic-fdm") }` returns `Ok(SetPrinter { printer: None })`; `SetPrinter { printer: Some("\t") }` returns `Err(InvalidLabel(ControlCharacter { field: Printer }))`.
65. For every failing edit in tests 54 to 64, the document after the call equals a clone taken before it.
66. Inverse round trip: on D1, for each of these 15 edits in turn (each on a fresh copy of D1): `AddParameter` (new `p-d` named `depth`, index 1), `UpdateParameter` (`p-h` expr `12 * wall`), `RemoveParameter` (`p-h`), `MoveParameter` (`p-wall` to 1), `AddBody` (new `b-2`, index 0), `UpdateBody` (`b-1` with color `#ff0000`), `RemoveBody` (`b-1`), `MoveBody` (`b-1` to 0), `AddFeature` (new `f-new` of type `box`, index 3), `UpdateFeature` (`f-fil1` suppressed), `RemoveFeature` (`f-sk1`), `MoveFeature` (`f-fil1` to 0), `SetRollback` (`f-sk1`), `SetUnits` (`Cm`), `SetPrinter` (`generic-resin`): `apply(e)` returns `Ok(inv)`, `apply(inv)` returns `Ok(e2)`, the document equals D1, and `e2 == e`.
67. Property test (proptest, 256 cases): from D1, apply a random sequence of up to 20 edits drawn from the 15 variants with arbitrary arguments; collect the inverses of those that succeed; `check_structure()` stays empty after every step; applying the collected inverses in reverse order returns `Ok` each time and restores D1 exactly.

### Serialization

68. Each `Edit` JSON line in section 2.11 deserializes to the matching variant, and serializing that value with `serde_json::to_string` gives the same text as `serde_json::to_string(&serde_json::from_str::<Value>(line))` (same keys, same order).
69. `serde_json::from_str::<Edit>("{\"type\": \"set_rollback\", \"rollback\": null}")` is `SetRollback { rollback: None }`.
70. `serde_json::to_value(Body::new(id("b-1"), "Base"))` is exactly `{"id": "b-1", "name": "Base"}`; with `visible = false` and `role = SupportBlocker` it is `{"id": "b-1", "name": "Base", "visible": false, "role": "support_blocker"}` in that key order.
71. Deserializing `{"type": "box", "size": [1, 2, 3], "id": "f-1", "suppressed": false, "label": "x"}` as `FeatureRecord` gives `id == f-1`, `kind == "box"`, `suppressed == false`, `name == None`, and `params` keys `["size", "label"]` in that order.
72. Deserializing `Parameter` from `{"id": "p-1", "name": "a", "expr": "1", "unit": "mm"}` fails (unknown field `unit`); deserializing `Parameter` whose `expr` is the number `2` fails.
73. `serde_json::from_str::<Transaction>` of the transaction example in section 2.11 gives a transaction with label `Add parameter wall` and two steps, the second being `Step { edit: SetRollback { rollback: None }, inverse: SetRollback { rollback: Some(f-ext1) } }`.

### Transactions and undo

74. On D1, `apply_all([AddParameter p-d index 2, SetUnits In])` returns two steps whose inverses are `RemoveParameter { p-d }` and `SetUnits { Mm }`.
75. On D1, `apply_all([SetUnits In, AddParameter p-d named depth, AddParameter p-e named depth])` returns `Err(TransactionError { index: 2, count: 3, error: NameInUse { name: "depth" } })` and the document equals D1. Its message is `Change 3 of 3 could not be applied: A parameter named depth already exists. Pick another name.`
76. `UndoLog::new()` on D1: `commit(doc, "Add depth", [AddParameter p-d])` succeeds; `undo_label() == Some("Add depth")`; `undo` returns a transaction labeled `Add depth` and the document equals D1; `redo_label() == Some("Add depth")`; `redo` restores the post-commit document; `can_redo()` is false.
77. A transaction `[AddParameter p-d named a, UpdateParameter p-d named b]` committed on D1 and undone gives D1 (inverses run in reverse order).
78. After commit A, commit B, undo: committing C makes `can_redo()` false and `redo_count() == 0`.
79. After commit A and undo, a commit whose edits fail returns the error and leaves `redo_count() == 1`, `undo_count() == 0`, and the document unchanged.
80. `commit(doc, "Nothing", vec![])` returns `Ok(())`, leaves the document unchanged, and keeps `undo_count()` and `redo_count()` as they were.
81. `UndoLog::with_max_depth(3)`: five commits setting units to `Cm`, `M`, `In`, `Mm`, `Cm` in turn; `undo_count() == 3`; three undos give units `Mm`, `In`, `M` in turn, and a fourth `undo` returns `Ok(None)` with units still `M`.
82. `UndoLog::with_max_depth(0)`: `commit` applies the edits; `can_undo()` is false; `undo` returns `Ok(None)`.
83. `undo` and `redo` on a new log return `Ok(None)`; `UndoLog::new().max_depth() == 1000`.
84. Conflict: commit `[AddFeature f-new]` on D1, then remove `f-new` with `doc.apply` directly; `undo` returns `Err(UndoError::Undo { label, .. })` whose inner error is `NotFound { kind: Feature, id: f-new }`; the document is unchanged and `undo_count() == 1`.
85. `clear()` makes `can_undo()` and `can_redo()` false and keeps `max_depth()`.
86. `record`: start from D1 with a log on which `[SetPrinter generic-fdm]` was committed and then undone (so the document equals D1 and `redo_count() == 1`). Apply `[AddParameter p-d, SetUnits In]` with `doc.apply_all`, then call `record(Transaction { label: "Two steps", steps })` with the returned steps: `redo_count() == 0`, `undo_count() == 1`, `undo_label() == Some("Two steps")`, and `undo` restores D1. `record` of a transaction with no steps changes neither stack.

### Rollback, parameter rename and manifests

87. On D1 with `rollback = Some(f-fil1)`, `apply(AddFeature { index: 2, feature: f-new of type box })` succeeds, `rollback` stays `Some(f-fil1)`, and `active_features()` has ids `[f-sk1, f-ext1, f-new]`.
88. On D1 with `f-ext1` params `{"body": "b-1", "exprs": ["2 * wall", "printer.wall + 1"]}`, `rename_parameter_edits(&id("p-wall"), "thickness", &TestInspector)` returns exactly three edits: `UpdateParameter` of `p-wall` named `thickness` with expr `2 mm`; `UpdateParameter` of `p-h` named `height` with expr `10 * thickness`; `UpdateFeature` of `f-ext1` with params `{"body": "b-1", "exprs": ["2 * thickness", "printer.wall + 1"]}`. Applying them with `apply_all` succeeds and `validate(&TestInspector)` is then empty.
89. On D1, `rename_parameter_edits` of `p-wall` to `height` returns `Err(NameInUse { name: "height" })`; to `2x` returns `Err(InvalidParameterName(InvalidStart { .. }))`; of `p-zz` returns `Err(NotFound { kind: RecordKind::Parameter, id: p-zz })`; of `p-wall` to `wall` returns `Ok(vec![])`.
90. On D1 with `p-h` expr `10 * wall +` (does not parse) and `f-ext1` params `{"invalid": true, "exprs": ["wall"]}`, renaming `p-wall` to `t` returns only the `UpdateParameter` of `p-wall`.
91. The extrude manifest JSON of section 2.12 deserializes as `FeatureManifest`, and serializing it with `serde_json::to_string` gives the same text as `serde_json::to_string(&serde_json::from_str::<Value>(example))`. `FieldHint { path: "body", label: "Body", essential: false, unit: None, selection: Some(SelectionKind::Body), options: vec![] }` serializes to exactly `{"path":"body","label":"Body","selection":"body"}`.

### Document units

**Fixture D3** (bare numbers): units `Mm`; parameters, in order: `p-wall` named `wall` with expr `2`; `p-n` named `n` with expr `4`; `p-gap` named `gap` with expr `wall / 4`; `p-both` named `both` with expr `3`; `p-free` named `free` with expr `7`; `p-h` named `height` with expr `30 mm`. Bodies: `b-1` named `Base`. Features, in order: `f-box1` of type `box` with params `{"body": "b-1", "lengths": ["40", " 20 ", "10 mm", "gap", "both"], "counts": ["n", "both"], "angles": ["45"]}`; `f-fil1` of type `fillet` with params `{"body": "b-1", "lengths": ["-2", "wall + 0.5"]}`; `f-bad` of type `box` with params `{"invalid": true, "lengths": ["5"]}`.

92. `D3.bare_numbers(&TestInspector)` is 4: `40` and ` 20 ` of `f-box1`, `-2` of `f-fil1`, and parameter `wall` (reached by `Length` links through `gap` and through `wall + 0.5`, and by no other). Not counted: `10 mm` and `30 mm` (they have a unit), `45` (an angle), `n` (reached by a count), `both` (reached by a length and a count), `free` (reached by nothing), `gap` and `wall + 0.5` (not bare numbers), and `5` of `f-bad` (its inspection fails).
93. `D3.units_edits(Units::In, true, &TestInspector)` is exactly `[SetUnits { units: In }, UpdateParameter { p-wall named wall with expr "2 mm" }, UpdateFeature { f-box1 with lengths ["40 mm", " 20 mm ", "10 mm", "gap", "both"] and its other params unchanged }, UpdateFeature { f-fil1 with lengths ["-2 mm", "wall + 0.5"] }]`. After `UndoLog::commit` of it: `units == In`, `bare_numbers` is 0, `40 mm` and `-2 mm` evaluate (expected `LENGTH`, units `In`) to the lengths 40 and -2 that `40` and `-2` gave under `Mm`, and `gap` to 0.5 mm as before; `wall + 0.5` now gives 14.7 mm instead of 2.5 mm (the limit of section 3.12). `undo` restores D3 exactly.
94. `D3.units_edits(Units::In, false, &TestInspector)` is exactly `[SetUnits { units: In }]`; `D3.units_edits(Units::Mm, true, &TestInspector)` and `D3.units_edits(Units::Mm, false, &TestInspector)` are empty; `units_edits` does not change D3.
95. Newcomer corpus NC09: a document in `Mm` with `f-box1` of type `box` with params `{"body": "b-1", "lengths": ["40", "20", "10"]}` and `f-fil1` of type `fillet` with params `{"body": "b-1", "lengths": ["2"]}` has `bare_numbers` 4, and `units_edits(Units::In, true, &TestInspector)` writes `"40 mm"`, `"20 mm"`, `"10 mm"` and `"2 mm"`; from `Cm` it writes `"40 cm"` and so on.
96. With `OpaqueFeatures`, `bare_numbers` counts no feature expression and no parameter, and `units_edits(units, true, &OpaqueFeatures)` is `[SetUnits { units }]` for any other units.
97. Factors are not sizes: D3 plus the parameter `p-scale` named `scale` with expr `2`, and `40 mm * scale` appended to the `lengths` of `f-box1`. `bare_numbers` is still 4: a `Length` link reaches `scale`, but only as a factor of `40 mm`. `units_edits(Units::In, true, &TestInspector)` is the result of test 93, with the `UpdateFeature` of `f-box1` keeping `40 mm * scale` at the end of its lengths and no edit of `p-scale`; after `UndoLog::commit` of it, `40 mm * scale` evaluates (expected `LENGTH`, units `In`) to the length 80, as before, and not to an area.

## 7. Open issues

1. **Structural rules are enforced at load.** Architecture lists the validation rules but not which are hard. This spec makes the rules in section 3.6 hard (load fails, `apply` refuses edits that would break them) so that every inverse edit is guaranteed to apply. References, declared ids, expressions and cycles stay soft (reported, tolerated). A file with, for example, a duplicate id after a bad merge will not open in the app until fixed by hand; `partsmith validate` still lists every problem.
2. **Printer id syntax.** Treated as a label (any non-blank text up to 128 characters without control characters). The print spec may define profile ids more strictly.
3. **`BodyRole` values.** Architecture says "part (default, omitted) | modifier kinds". This spec names the four modifier kinds from technical design section 11: `negative_volume`, `support_blocker`, `support_enforcer`, `settings_modifier`. The settings themselves come later; the IO spec owns how each exports to 3MF.
4. **`Units` variant names.** Architecture writes `mm | cm | m | in`; the Rust variants are `Mm`, `Cm`, `M`, `In` with lowercase serialization.
5. **Normalization of params numbers.** `FeatureRecord::params` stays a public `serde_json::Map` as in architecture, so a record built in code may hold `80.0` as a float while a loaded one holds the integer `80`. `apply` and `load` normalize, `Document::normalize` is public, and the round-trip equality `load(save(doc)) == doc` is stated for normalized documents (file-format.md section 3.8).
6. **Default undo depth and open transactions.** The default of 1000 matches the engine spec's `undo_limit`. Update edits hold full copies of records, so a long history of large sketch edits costs memory; the engine may lower the limit through `with_max_depth`. The engine's open transactions (edits applied over several commands, then committed) use `UndoLog::record`; coalescing of repeated updates is engine behavior, not part of this crate in M1.
7. **No reference checks in `apply`.** Removing a body that features use, or a parameter that expressions use, succeeds. The engine and UI decide whether to warn or to offer a cascading change.
8. **Keep sizes rewrites bare numbers only.** `units_edits` adds the old unit to expressions that are a number and nothing else, and to parameters used as lengths only, in promoting positions (section 3.12). Numbers inside larger expressions still change meaning. Rewriting every promoted literal would need the evaluator to report which nodes it promoted; the count hosts show (`bare_numbers`) keeps the promise exact meanwhile. `ExprLink` gained `unit` for this: the feature registry fills it from each `ExprUse::kind` (features.md section 2.4), so `FeatureInspector::inspect` in features.md must set it.
