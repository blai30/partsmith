//! Document recipe model for Partsmith: ids, units, expressions, edits and undo, and canonical `.ptsm` serialization.
//!
//! A [`Document`] is the recipe only: parameters, bodies and feature records. Geometry is always
//! regenerated from it by other crates. Every change goes through an [`Edit`], which
//! [`Document::apply`] applies and answers with its exact inverse; an [`UndoLog`] groups applied
//! edits into labeled transactions.
//!
//! Numeric fields hold expressions ([`Expr`]), which are parsed, dimension-checked and evaluated
//! here. Feature params are a generic JSON map that this crate never interprets; the feature
//! registry reports what they declare and refer to through [`FeatureInspector`].
//!
//! Documents are saved as canonical `.ptsm` JSON ([`save`], [`load`], and the writer in
//! [`canonical`]): saving an unchanged document is byte-identical.
//!
//! The crate is pure Rust with no kernel dependency and no global state, and never panics on user
//! input. The behavior is specified in `docs/spec/model.md`, `docs/spec/expressions.md` and
//! `docs/spec/file-format.md`.

pub mod canonical;
mod document;
mod edit;
mod expr;
mod file;
mod id;
mod inspect;
pub mod manifest;
mod names;
mod path;
mod rename;
mod structure;
mod undo;
mod units;
mod validation;

pub use canonical::CanonicalError;
pub use document::{Body, Document, FeatureRecord, Parameter};
pub use edit::{EditError, Edit, RecordKind, Step, Transaction, TransactionError};
pub use expr::{
    Arity, BinaryOperator, Dimension, EmptyContext, EvaluationContext, Expr, ExprError,
    ExprErrorKind, ExprNode, ExprNodeKind, Function, MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_LENGTH,
    MismatchOperation, ParameterValues, ParsedExpr, Quantity, References, RenameError, Scope, Span,
    Unit,
};
pub use file::{
    FILE_EXTENSION, FORMAT, FORMAT_VERSION, FeatureParamsSchema, LoadError, Loaded, MIGRATIONS,
    MigrationStep, OLDEST_FORMAT_VERSION, SaveError, SchemaError, load, load_bytes, migrate,
    migrate_with, ptsm_schema, ptsm_schema_text, save,
};
pub use id::{
    Id, IdError, IdGenerator, IdKind, MAX_ID_ATTEMPTS, RandomIdGenerator, SequentialIds,
};
pub use inspect::{ExprLink, FeatureInspector, FeatureLinks, OpaqueFeatures};
pub use manifest::{
    Category, FeatureManifest, FieldHint, FieldUnit, OptionHint, SelectionKind,
};
pub use names::{
    LabelError, LabelField, MAX_FEATURE_TYPE_LENGTH, MAX_LABEL_LENGTH, MAX_PARAMETER_NAME_LENGTH,
    NameError, RESERVED_NAMES, is_valid_feature_type, validate_label, validate_parameter_name,
};
pub use structure::StructureError;
pub use undo::{UndoError, UndoLog};
pub use units::{BodyRole, Color, ColorError, Units, UnitsError};
pub use validation::ValidationError;

/// Modeling tolerance in millimeters, shared by every crate.
pub const LINEAR_TOLERANCE: f64 = 1e-6;
/// Angular tolerance in radians, shared by every crate.
pub const ANGULAR_TOLERANCE: f64 = 1e-9;
