//! `validate` and `format` for the CLI (engine.md sections 3.2 and 3.19).

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::builtin::hole::HoleParams;
use partsmith_features::{
    CancelToken, Dependencies, DocumentNames, ExprUse, FeatureError, FieldRef, Noun, Registry,
    Values,
};
use partsmith_model::{Document, ExprErrorKind, FeatureRecord, Id, LoadError, Scope, load};
use partsmith_print::{PrinterValues, Severity};
use partsmith_recompute::{Input, Severity as MessageSeverity};
use partsmith_sketch::{NativeSolver, PointRef, SketchParams, SolveIssue, Solver};
use serde_json::Value;

use crate::document::canonical_text;
use crate::error::EngineError;
use crate::session::{Session, asset_matches, canonicalize_params, load_error};
use crate::sketch::{SKETCH_TYPE, dimension_values, sketch_params};
use crate::types::{Asset, ValidateOptions, ValidationDiagnostic, ValidationReport};

/// The type name of the hole feature, whose `points` name sketch points (features/hole.md).
const HOLE_TYPE: &str = "hole";

/// The `kind` of an axis that is a sketch line (features.md `AxisRef::SketchLine`).
const SKETCH_LINE_KIND: &str = "sketch_line";

impl Session {
    /// Validates a `.ptsm` text without opening it.
    pub fn validate(
        &mut self,
        text: &str,
        assets: &[Asset],
        options: ValidateOptions,
    ) -> ValidationReport {
        let mut report = ValidationReport::default();
        let loaded = match load(text) {
            Ok(loaded) => loaded,
            Err(error) => {
                report.diagnostics.push(load_diagnostic(error));
                return report;
            }
        };
        let mut document = loaded.document;
        canonicalize_params(&self.registry, &mut document);
        let registry = Arc::clone(&self.registry);
        let names = DocumentNames {
            document: &document,
            registry: &registry,
        };
        let mut checks = Checks {
            registry: &registry,
            names: &names,
            document: &document,
            diagnostics: &mut report.diagnostics,
        };
        let dependencies = checks.params();
        let profile = self.profiles.resolve(document.printer.as_deref());
        let context = PrinterValues::new(&profile.profile);
        let parameters = document.evaluate_parameters(&context);
        let scope = Scope {
            units: document.units,
            parameters: &parameters,
            context: &context,
        };
        checks.expressions(&dependencies, &scope);
        checks.references(&dependencies);
        if let Some(diagnostic) = profile.diagnostic() {
            checks.push(
                Severity::Warning,
                &diagnostic.code,
                diagnostic.message,
                None,
            );
        }
        checks.sketches(&dependencies, &scope);
        let stored = checks.assets(&dependencies, assets);
        if options.rebuild {
            let input = Input {
                document: &document,
                context: &context,
                stop_at: None,
                assets: &stored,
            };
            let rebuilt = self.recompute.run(&input, &CancelToken::new());
            for (position, feature) in rebuilt.features.iter().enumerate() {
                for message in &feature.messages {
                    let (severity, code) = match message.severity {
                        MessageSeverity::Error => (Severity::Error, "feature_error"),
                        MessageSeverity::Warning => (Severity::Warning, "feature_warning"),
                    };
                    checks.push(
                        severity,
                        code,
                        format!("{}: {}", feature.feature, message.text),
                        Some(format!("/features/{position}")),
                    );
                }
            }
        }
        if options.canonical
            && let Ok(canonical) = canonical_text(&document)
            && canonical != text
        {
            let line = first_difference_line(text, &canonical);
            report.diagnostics.push(ValidationDiagnostic {
                severity: Severity::Error,
                code: "not_canonical".to_string(),
                message: format!(
                    "The file is not in canonical form; the first difference is on line {line}. Run partsmith fmt to rewrite it."
                ),
                line: Some(line),
                column: None,
                path: None,
            });
        }
        report
    }

    /// Loads a .ptsm text and returns its canonical text (params canonicalized by the registry),
    /// without recompute.
    pub fn format(&self, text: &str) -> Result<String, EngineError> {
        let mut document = load(text).map_err(load_error)?.document;
        canonicalize_params(&self.registry, &mut document);
        canonical_text(&document)
    }
}

/// The checks of section 3.19, steps 2 to 7, writing diagnostics in step order.
struct Checks<'a, 'r> {
    /// The registry.
    registry: &'a Registry,
    /// Names for rendering feature messages.
    names: &'a DocumentNames<'r>,
    /// The loaded document, params canonicalized.
    document: &'a Document,
    /// The report's diagnostics.
    diagnostics: &'a mut Vec<ValidationDiagnostic>,
}

impl Checks<'_, '_> {
    /// Adds a diagnostic without a position in the text.
    fn push(&mut self, severity: Severity, code: &str, message: String, path: Option<String>) {
        self.diagnostics.push(ValidationDiagnostic {
            severity,
            code: code.to_string(),
            message,
            line: None,
            column: None,
            path,
        });
    }

    /// Adds a feature error at `/features/<position>` plus its field.
    fn feature_error(&mut self, position: usize, code: &str, error: &FeatureError) {
        let message = error.render(self.names, self.document.units);
        let path = feature_path(position, error.field());
        self.push(Severity::Error, code, message, Some(path));
    }

    /// Step 2: unknown types and invalid params. Returns the dependencies of the valid records.
    fn params(&mut self) -> Vec<Option<Dependencies>> {
        let mut dependencies = Vec::with_capacity(self.document.features.len());
        for (position, record) in self.document.features.iter().enumerate() {
            let checked = match self.registry.validate_and_canonicalize(record) {
                Ok(validated) => self.registry.dependencies(&validated.record),
                Err(error) => Err(error),
            };
            match checked {
                Ok(found) => dependencies.push(Some(found)),
                Err(error) => {
                    let code = match error {
                        FeatureError::UnknownType { .. } => "unknown_type",
                        _ => "invalid_params",
                    };
                    self.feature_error(position, code, &error);
                    dependencies.push(None);
                }
            }
        }
        dependencies
    }

    /// Step 3: expression errors of parameters, then of feature params.
    fn expressions(&mut self, dependencies: &[Option<Dependencies>], scope: &Scope<'_>) {
        for (position, parameter) in self.document.parameters.iter().enumerate() {
            if let Some(Err(error)) = scope.parameters.get(&parameter.name) {
                self.push(
                    Severity::Error,
                    expression_code(&error.kind),
                    format!("{}: {error}", parameter.name),
                    Some(format!("/parameters/{position}/expr")),
                );
            }
        }
        for (position, (record, found)) in
            self.document.features.iter().zip(dependencies).enumerate()
        {
            let (Some(found), Some(feature)) = (found, self.registry.get(&record.kind)) else {
                continue;
            };
            let uses = self.evaluated_uses(record, &found.expressions);
            let values = Values::evaluate(&uses, scope, feature.manifest());
            let errors: Vec<FeatureError> = values
                .iter()
                .filter_map(|(_, value)| value.as_ref().err().cloned())
                .collect();
            for error in errors {
                if let FeatureError::Expression { error: cause, .. } = &error {
                    let code = expression_code(&cause.kind);
                    self.feature_error(position, code, &error);
                }
            }
        }
    }

    /// Step 4: feature, body and sketch entity links that name nothing or a later feature.
    fn references(&mut self, dependencies: &[Option<Dependencies>]) {
        for (position, (record, found)) in
            self.document.features.iter().zip(dependencies).enumerate()
        {
            let (Some(found), Some(feature)) = (found, self.registry.get(&record.kind)) else {
                continue;
            };
            let manifest = feature.manifest();
            for link in &found.features {
                let field = FieldRef::new(&link.field, manifest);
                let (code, error) = match self.document.feature_index(&link.id) {
                    None => (
                        "unknown_feature",
                        FeatureError::UnknownFeature {
                            field,
                            feature: link.id.clone(),
                        },
                    ),
                    Some(target) if target >= position => (
                        "forward_reference",
                        FeatureError::ForwardReference {
                            field,
                            feature: link.id.clone(),
                        },
                    ),
                    Some(_) => continue,
                };
                self.feature_error(position, code, &error);
            }
            for link in found.bodies_read.iter().chain(&found.bodies_written) {
                if self.document.body(&link.id).is_none() {
                    let error = FeatureError::UnknownBody {
                        field: FieldRef::new(&link.field, manifest),
                        body: link.id.clone(),
                    };
                    self.feature_error(position, "unknown_body", &error);
                }
            }
            for link in entity_links(record) {
                let Some(sketch) = self.earlier_sketch(&link.sketch, position) else {
                    continue;
                };
                let (exists, entity, noun) = match &link.target {
                    EntityTarget::Point(point) => (
                        sketch.point(point).is_some(),
                        point.to_string(),
                        Noun::Point,
                    ),
                    EntityTarget::Line(line) => {
                        (sketch.entity(line).is_some(), line.to_string(), Noun::Line)
                    }
                };
                if !exists {
                    let error = FeatureError::UnknownEntity {
                        field: FieldRef::new(&link.field, manifest),
                        sketch: link.sketch.clone(),
                        entity,
                        noun,
                    };
                    self.feature_error(position, "unknown_entity", &error);
                }
            }
        }
    }

    /// The params of the sketch `id` when it is a valid sketch feature before timeline position
    /// `position`. Other cases are reported by the feature link checks or at rebuild.
    fn earlier_sketch(&self, id: &Id, position: usize) -> Option<SketchParams> {
        let target = self.document.feature_index(id)?;
        let record = self.document.features.get(target)?;
        if target >= position || record.kind != SKETCH_TYPE {
            return None;
        }
        sketch_params(self.registry, self.document, record).ok()
    }

    /// The expression uses recompute evaluates: every declared use except the expressions of a
    /// sketch's reference dimensions (`driving: false`), which the sketch feature never evaluates
    /// (features/sketch.md).
    fn evaluated_uses(&self, record: &FeatureRecord, uses: &[ExprUse]) -> Vec<ExprUse> {
        let reference_fields: Vec<String> = if record.kind == SKETCH_TYPE {
            sketch_params(self.registry, self.document, record)
                .map(|params| reference_dimension_fields(&params))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        uses.iter()
            .filter(|usage| !reference_fields.contains(&usage.field))
            .cloned()
            .collect()
    }

    /// Step 6: each valid sketch is solved and its solver issues reported.
    fn sketches(&mut self, dependencies: &[Option<Dependencies>], scope: &Scope<'_>) {
        for (position, (record, found)) in
            self.document.features.iter().zip(dependencies).enumerate()
        {
            if record.kind != SKETCH_TYPE || found.is_none() {
                continue;
            }
            let Ok(params) = sketch_params(self.registry, self.document, record) else {
                continue;
            };
            let Ok(values) = dimension_values(&params, scope) else {
                continue;
            };
            let path = Some(format!("/features/{position}"));
            match NativeSolver.solve(&params, &values) {
                Ok(result) => {
                    for issue in result.issues(&params) {
                        let (severity, code) = match issue {
                            SolveIssue::Conflicting { .. } => {
                                (Severity::Error, "sketch_conflicting")
                            }
                            SolveIssue::Redundant { .. } => (Severity::Warning, "sketch_redundant"),
                            SolveIssue::NotConverged => (Severity::Error, "sketch_not_converged"),
                        };
                        self.push(severity, code, issue.to_string(), path.clone());
                    }
                }
                Err(error) => self.push(Severity::Error, "sketch_invalid", error.to_string(), path),
            }
        }
    }

    /// Step 7: referenced assets that were not supplied or do not match their name. Returns the
    /// supplied assets that match.
    fn assets(
        &mut self,
        dependencies: &[Option<Dependencies>],
        assets: &[Asset],
    ) -> BTreeMap<String, Arc<[u8]>> {
        let mut stored = BTreeMap::new();
        for (position, (record, found)) in
            self.document.features.iter().zip(dependencies).enumerate()
        {
            let (Some(found), Some(feature)) = (found, self.registry.get(&record.kind)) else {
                continue;
            };
            for link in &found.assets {
                match assets.iter().find(|asset| asset.name == link.name) {
                    None => {
                        let error = FeatureError::AssetMissing {
                            field: FieldRef::new(&link.field, feature.manifest()),
                            asset: link.name.clone(),
                        };
                        self.feature_error(position, "asset_missing", &error);
                    }
                    Some(asset) if !asset_matches(&asset.name, &asset.bytes) => {
                        let message = EngineError::AssetMismatch {
                            name: asset.name.clone(),
                        }
                        .to_string();
                        let path = feature_path(
                            position,
                            Some(&FieldRef::new(&link.field, feature.manifest())),
                        );
                        self.push(Severity::Error, "asset_mismatch", message, Some(path));
                    }
                    Some(asset) => {
                        stored.insert(asset.name.clone(), Arc::from(asset.bytes.as_slice()));
                    }
                }
            }
        }
        stored
    }
}

/// The field paths of a sketch's reference dimensions, as the sketch feature declares them
/// (`constraints[<index>].expr`).
fn reference_dimension_fields(params: &SketchParams) -> Vec<String> {
    params
        .constraints
        .iter()
        .enumerate()
        .filter(|(_, constraint)| {
            constraint
                .dimension()
                .is_some_and(|dimension| !dimension.driving)
        })
        .map(|(index, _)| format!("constraints[{index}].expr"))
        .collect()
}

/// A sketch entity that a feature's params name (step 4).
#[derive(Debug, Clone, PartialEq)]
struct EntityLink {
    /// The sketch feature.
    sketch: Id,
    /// Path of the params field that names the entity, for example `points[0]` or `axis`.
    field: String,
    /// The point or line it must be.
    target: EntityTarget,
}

/// What a sketch entity reference must resolve to.
#[derive(Debug, Clone, PartialEq)]
enum EntityTarget {
    /// A point of the sketch, as a hole point.
    Point(PointRef),
    /// An entity used as a line, as a sketch line axis.
    Line(Id),
}

/// The sketch entities a record's params name, in params order: the points of a hole, and every
/// `sketch_line` axis.
fn entity_links(record: &FeatureRecord) -> Vec<EntityLink> {
    let mut links = Vec::new();
    if record.kind == HOLE_TYPE
        && let Ok(hole) = serde_json::from_value::<HoleParams>(Value::Object(record.params.clone()))
    {
        for (index, point) in hole.points.into_iter().enumerate() {
            links.push(EntityLink {
                sketch: hole.sketch.clone(),
                field: format!("points[{index}]"),
                target: EntityTarget::Point(point),
            });
        }
    }
    for (key, value) in &record.params {
        sketch_lines(value, key, &mut links);
    }
    links
}

/// Adds every `sketch_line` axis inside `value`, found at params path `path`.
fn sketch_lines(value: &Value, path: &str, links: &mut Vec<EntityLink>) {
    match value {
        Value::Object(members) => {
            let text = |key: &str| members.get(key).and_then(Value::as_str);
            if text("kind") == Some(SKETCH_LINE_KIND)
                && let (Some(Ok(sketch)), Some(Ok(line))) =
                    (text("sketch").map(Id::new), text("line").map(Id::new))
            {
                links.push(EntityLink {
                    sketch,
                    field: path.to_string(),
                    target: EntityTarget::Line(line),
                });
                return;
            }
            for (key, member) in members {
                sketch_lines(member, &format!("{path}.{key}"), links);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                sketch_lines(item, &format!("{path}[{index}]"), links);
            }
        }
        _ => {}
    }
}

/// `/features/<position>` followed by the field's pointer.
fn feature_path(position: usize, field: Option<&FieldRef>) -> String {
    format!(
        "/features/{position}{}",
        field.map(FieldRef::pointer).unwrap_or_default()
    )
}

/// The diagnostic of a document that does not load (step 1).
fn load_diagnostic(error: LoadError) -> ValidationDiagnostic {
    let path = match &error {
        LoadError::Invalid { path, .. } if !path.is_empty() => Some(
            FieldRef {
                path: path.clone(),
                label: String::new(),
            }
            .pointer(),
        ),
        _ => None,
    };
    let engine_error = load_error(error);
    let (line, column) = match &engine_error {
        EngineError::Parse { line, column, .. } => (*line, *column),
        _ => (None, None),
    };
    ValidationDiagnostic {
        severity: Severity::Error,
        code: match engine_error {
            EngineError::UnsupportedFormatVersion { .. } => "unsupported_format_version",
            _ => "parse_error",
        }
        .to_string(),
        message: engine_error.to_string(),
        line,
        column,
        path,
    }
}

/// The 1-based line of the first byte where `text` and `canonical` differ.
fn first_difference_line(text: &str, canonical: &str) -> u32 {
    let position = text
        .bytes()
        .zip(canonical.bytes())
        .position(|(first, second)| first != second)
        .unwrap_or_else(|| text.len().min(canonical.len()));
    let newlines = text.as_bytes()[..position]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count();
    u32::try_from(newlines + 1).unwrap_or(u32::MAX)
}

/// The code of an expression error: its kind in snake case (expressions.md section 4).
fn expression_code(kind: &ExprErrorKind) -> &'static str {
    match kind {
        ExprErrorKind::Empty => "empty",
        ExprErrorKind::TooLong { .. } => "too_long",
        ExprErrorKind::TooDeep => "too_deep",
        ExprErrorKind::UnexpectedCharacter { .. } => "unexpected_character",
        ExprErrorKind::UnexpectedToken { .. } => "unexpected_token",
        ExprErrorKind::UnexpectedEnd => "unexpected_end",
        ExprErrorKind::UnclosedParenthesis => "unclosed_parenthesis",
        ExprErrorKind::UnmatchedParenthesis => "unmatched_parenthesis",
        ExprErrorKind::PowerOperator => "power_operator",
        ExprErrorKind::DecimalComma => "decimal_comma",
        ExprErrorKind::UnknownUnit { .. } => "unknown_unit",
        ExprErrorKind::UnknownFunction { .. } => "unknown_function",
        ExprErrorKind::MissingCall { .. } => "missing_call",
        ExprErrorKind::ArgumentCount { .. } => "argument_count",
        ExprErrorKind::UnknownName { .. } => "unknown_name",
        ExprErrorKind::UnknownValue { .. } => "unknown_value",
        ExprErrorKind::DependsOnFailed { .. } => "depends_on_failed",
        ExprErrorKind::Cycle { .. } => "cycle",
        ExprErrorKind::DimensionMismatch { .. } => "dimension_mismatch",
        ExprErrorKind::ArgumentDimension { .. } => "argument_dimension",
        ExprErrorKind::ExponentDimension { .. } => "exponent_dimension",
        ExprErrorKind::FractionalDimension { .. } => "fractional_dimension",
        ExprErrorKind::DimensionOverflow => "dimension_overflow",
        ExprErrorKind::ExpectedDimension { .. } => "expected_dimension",
        ExprErrorKind::DivisionByZero => "division_by_zero",
        ExprErrorKind::NegativeSquareRoot => "negative_square_root",
        ExprErrorKind::OutOfDomain { .. } => "out_of_domain",
        ExprErrorKind::NegativeBase => "negative_base",
        ExprErrorKind::NotFinite => "not_finite",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn differences_are_located_by_line() {
        assert_eq!(first_difference_line("a\nb\nc", "a\nb\nd"), 3);
        assert_eq!(first_difference_line("a\n    b", "a\n  b"), 2);
        assert_eq!(first_difference_line("ab", "abc"), 1);
    }

    #[test]
    fn json_errors_keep_their_position() {
        let diagnostic = load_diagnostic(LoadError::Syntax {
            line: 1,
            column: 23,
            message: "EOF while parsing an object".to_string(),
        });

        assert_eq!(diagnostic.code, "parse_error");
        assert_eq!((diagnostic.line, diagnostic.column), (Some(1), Some(23)));
        assert_eq!(diagnostic.path, None);
    }

    #[test]
    fn invalid_fields_point_into_the_document() {
        let diagnostic = load_diagnostic(LoadError::Invalid {
            path: "features[2].radius".to_string(),
            message: "must be a string".to_string(),
        });

        assert_eq!(diagnostic.path.as_deref(), Some("/features/2/radius"));
    }

    #[test]
    fn newer_versions_have_their_own_code() {
        let diagnostic = load_diagnostic(LoadError::NewerFormatVersion {
            found: 2,
            supported: 1,
        });

        assert_eq!(diagnostic.code, "unsupported_format_version");
    }

    #[test]
    fn expression_codes_are_snake_case_kinds() {
        assert_eq!(
            expression_code(&ExprErrorKind::UnknownName {
                name: "width".to_string()
            }),
            "unknown_name"
        );
        assert_eq!(
            expression_code(&ExprErrorKind::DivisionByZero),
            "division_by_zero"
        );
    }

    fn record(json: serde_json::Value) -> FeatureRecord {
        serde_json::from_value(json).expect("a valid feature record")
    }

    fn id(text: &str) -> Id {
        Id::new(text).expect("a valid test id")
    }

    #[test]
    fn hole_points_name_sketch_points() {
        let hole = record(serde_json::json!({
            "id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk1",
            "points": ["e-p9", "e-1.start"], "diameter": "3 mm", "extent": {"kind": "through_all"}
        }));

        let links = entity_links(&hole);

        assert_eq!(
            links,
            [
                EntityLink {
                    sketch: id("f-sk1"),
                    field: "points[0]".to_string(),
                    target: EntityTarget::Point("e-p9".parse().expect("a point ref")),
                },
                EntityLink {
                    sketch: id("f-sk1"),
                    field: "points[1]".to_string(),
                    target: EntityTarget::Point("e-1.start".parse().expect("a point ref")),
                },
            ]
        );
    }

    #[test]
    fn nested_sketch_line_axes_are_found_with_their_path() {
        let pattern = record(serde_json::json!({
            "id": "f-lp1", "type": "linear_pattern", "body": "b-1",
            "direction": {"axis": {"kind": "sketch_line", "sketch": "f-sk1", "line": "e-9"}, "count": "2", "spacing": "5"},
            "second_direction": {"axis": {"kind": "origin", "axis": "Y"}, "count": "2", "spacing": "5"}
        }));

        let links = entity_links(&pattern);

        assert_eq!(
            links,
            [EntityLink {
                sketch: id("f-sk1"),
                field: "direction.axis".to_string(),
                target: EntityTarget::Line(id("e-9")),
            }]
        );
    }

    #[test]
    fn records_without_entity_references_have_no_links() {
        let block = record(serde_json::json!({
            "id": "f-box1", "type": "box", "body": "b-1", "size": ["1", "2", "3"], "op": "new"
        }));

        assert!(entity_links(&block).is_empty());
    }

    #[test]
    fn only_reference_dimensions_are_left_unevaluated() {
        let params: SketchParams = serde_json::from_value(serde_json::json!({
            "plane": {"kind": "origin", "plane": "XY"},
            "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [10, 0]}],
            "constraints": [
                {"id": "c-1", "type": "horizontal", "line": "e-1"},
                {"id": "c-2", "type": "distance", "line": "e-1", "expr": "10 mm"},
                {"id": "c-3", "type": "distance", "line": "e-1", "expr": "wdth", "driving": false}
            ]
        }))
        .expect("valid sketch params");

        assert_eq!(reference_dimension_fields(&params), ["constraints[2].expr"]);
    }

    #[test]
    fn feature_paths_append_the_field_pointer() {
        let field = FieldRef {
            path: "size[2]".to_string(),
            label: "Height".to_string(),
        };

        assert_eq!(feature_path(0, Some(&field)), "/features/0/size/2");
        assert_eq!(feature_path(3, None), "/features/3");
    }
}
