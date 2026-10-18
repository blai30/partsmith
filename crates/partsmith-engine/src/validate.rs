//! `validate` and `format` for the CLI (engine.md sections 3.2 and 3.19).

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::builtin::hole::HoleParams;
use partsmith_features::{
    CancelToken, Dependencies, ExprUse, FeatureError, FieldRef, Noun, Registry, Values,
};
use partsmith_model::{
    Document, ExprErrorKind, FeatureManifest, FeatureRecord, Id, LoadError, Scope, load,
};
use partsmith_print::{PrinterValues, Severity};
use partsmith_protocol::{Repair, RepairCandidate};
use partsmith_recompute::Input;
use partsmith_sketch::{NativeSolver, PointRef, SketchParams, SolveIssue, Solver};
use serde_json::Value;

use crate::assets::asset_matches;
use crate::counts::saturating_u32;
use crate::diagnostics::render_feature_error;
use crate::document::{canonical_text, canonicalize_params};
use crate::error::EngineError;
use crate::run::RunView;
use crate::session::Session;
use crate::sketch::{SKETCH_TYPE, SketchWords, dimension_values};
use crate::types::{
    Asset, FeatureMessage, ValidateOptions, ValidationDiagnostic, ValidationReport,
};

/// The type name of the hole feature, whose `points` name sketch points (features/hole.md).
const HOLE_TYPE: &str = "hole";

/// The `kind` of an axis that is a sketch line (features.md `AxisRef::SketchLine`).
const SKETCH_LINE_KIND: &str = "sketch_line";

/// The revision of the temporary document `validate` loads, which the actions of its feature
/// messages carry (section 3.19 step 8).
const VALIDATED_REVISION: u64 = 0;

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
        let mut checks = Checks {
            registry: &registry,
            document: &document,
            diagnostics: &mut report.diagnostics,
        };
        let checked = checks.params();
        let profile = self.profiles.resolve(document.printer.as_deref());
        let context = PrinterValues::new(&profile.profile);
        let parameters = document.evaluate_parameters(&context);
        let scope = Scope {
            units: document.units,
            parameters: &parameters,
            context: &context,
        };
        checks.expressions(&checked, &scope);
        checks.references(&checked);
        if let Some(diagnostic) = profile.diagnostic() {
            checks.push(
                Severity::Warning,
                &diagnostic.code,
                diagnostic.message,
                None,
            );
        }
        checks.sketches(&checked, &scope);
        let stored = checks.assets(&checked, assets);
        if options.rebuild {
            let input = Input {
                document: &document,
                context: &context,
                stop_at: None,
                assets: &stored,
            };
            let rebuilt = self.recompute.run(&input, &CancelToken::new());
            let run = RunView::new(&document, &registry, &rebuilt);
            let messages = feature_messages(&run, &report.diagnostics);
            report.diagnostics.extend(messages);
        }
        if options.canonical
            && let Ok(canonical) = canonical_text(&document)
            && canonical != text
        {
            let line = first_difference_line(text, &canonical);
            report.diagnostics.push(ValidationDiagnostic {
                line: Some(line),
                ..ValidationDiagnostic::new(
                    Severity::Error,
                    "not_canonical",
                    format!(
                        "The file is not in canonical form; the first difference is on line {line}. Run partsmith fmt to rewrite it."
                    ),
                )
            });
        }
        report
    }

    /// Loads a .ptsm text and returns its canonical text (params canonicalized by the registry),
    /// without recompute.
    pub fn format(&self, text: &str) -> Result<String, EngineError> {
        let mut document = load(text).map_err(EngineError::from_load)?.document;
        canonicalize_params(&self.registry, &mut document);
        canonical_text(&document)
    }
}

/// Step 8: every feature message of the rebuild, features in timeline order and each feature's
/// messages in their order, with its feature, its projection (actions at revision 0) and the
/// candidates of its repairs, except those the steps before already reported: one at the path of
/// an earlier error, or with the code and path of an earlier warning.
fn feature_messages(
    run: &RunView<'_>,
    earlier: &[ValidationDiagnostic],
) -> Vec<ValidationDiagnostic> {
    let reported = |code: &str, path: &str| {
        earlier.iter().any(|diagnostic| {
            diagnostic.path.as_deref() == Some(path)
                && (diagnostic.severity == Severity::Error || diagnostic.code == code)
        })
    };
    let mut diagnostics = Vec::new();
    for feature in run.features() {
        for message in &feature.report.messages {
            let field = message.pointer.as_deref();
            let path = message_location(feature.position, field);
            if reported(message.code, &path) {
                continue;
            }
            let (diagnostic, repairs) = feature.project(message, VALIDATED_REVISION, None);
            let candidates = message_candidates(&repairs, field);
            let found = ValidationDiagnostic::new(
                diagnostic.severity,
                diagnostic.code.clone(),
                diagnostic.message.clone(),
            );
            diagnostics.push(ValidationDiagnostic {
                path: Some(path),
                feature: Some(Box::new(FeatureMessage {
                    feature: feature.id().clone(),
                    diagnostic,
                    candidates,
                })),
                ..found
            });
        }
    }
    diagnostics
}

/// Where a message of the feature at timeline position `position` is in the document (cli.md
/// section 2.2): `/features/<position>` followed by the message's field, a JSON Pointer.
/// `validate` reports it as the message's path, and hosts that read statuses locate messages
/// with it.
pub fn message_location(position: usize, field: Option<&str>) -> String {
    format!("/features/{position}{}", field.unwrap_or_default())
}

/// The candidates of a feature message whose field is `field` (cli.md section 2.2): those of the
/// element repairs among a feature's `repairs` whose path is `field` or lies under it (`/edges/0`
/// under `/edges`), in repair order, then candidate order; none for a message about no field.
/// `validate` reports them with each message, and hosts that read statuses pick them out of
/// `FeatureStatus::repairs` with it.
pub fn message_candidates(repairs: &[Repair], field: Option<&str>) -> Vec<RepairCandidate> {
    let Some(field) = field else {
        return Vec::new();
    };
    repairs
        .iter()
        .filter_map(|repair| match repair {
            Repair::Element {
                path, candidates, ..
            } if path == field
                || path
                    .strip_prefix(field)
                    .is_some_and(|rest| rest.starts_with('/')) =>
            {
                Some(candidates.iter().cloned())
            }
            _ => None,
        })
        .flatten()
        .collect()
}

/// What step 2 finds out about a valid record, for the later steps.
struct Checked<'a> {
    /// What it declares.
    dependencies: Dependencies,
    /// Its feature type's manifest.
    manifest: &'a FeatureManifest,
    /// Its params as sketch params, when it is a sketch.
    sketch: Option<SketchParams>,
}

/// The checks of section 3.19, steps 2 to 7, writing diagnostics in step order.
struct Checks<'a> {
    /// The registry.
    registry: &'a Registry,
    /// The loaded document, params canonicalized.
    document: &'a Document,
    /// The report's diagnostics.
    diagnostics: &'a mut Vec<ValidationDiagnostic>,
}

impl<'a> Checks<'a> {
    /// Adds a diagnostic without a position in the text.
    fn push(&mut self, severity: Severity, code: &str, message: String, path: Option<String>) {
        self.diagnostics.push(ValidationDiagnostic {
            path,
            ..ValidationDiagnostic::new(severity, code, message)
        });
    }

    /// Adds a feature error at `/features/<position>` plus its field.
    fn feature_error(&mut self, position: usize, code: &str, error: &FeatureError) {
        let feature = &self.document.features[position].id;
        let message = render_feature_error(error, self.document, self.registry, feature);
        let path = feature_path(position, error.field());
        self.push(Severity::Error, code, message, Some(path));
    }

    /// Step 2: unknown types and invalid params. Returns what each valid record declares, `None`
    /// for the others.
    fn params(&mut self) -> Vec<Option<Checked<'a>>> {
        let mut checked = Vec::with_capacity(self.document.features.len());
        for (position, record) in self.document.features.iter().enumerate() {
            let found = match self.registry.validate_and_canonicalize(record) {
                Ok(validated) => self
                    .registry
                    .dependencies(&validated.record)
                    .map(|dependencies| (validated.record, dependencies)),
                Err(error) => Err(error),
            };
            match found {
                Ok((canonical, dependencies)) => {
                    checked.push(self.checked(canonical, dependencies));
                }
                Err(error) => {
                    let code = match error {
                        FeatureError::UnknownType { .. } => "unknown_type",
                        _ => "invalid_params",
                    };
                    self.feature_error(position, code, &error);
                    checked.push(None);
                }
            }
        }
        checked
    }

    /// What the later steps need from a valid record with these canonical params, read once.
    fn checked(&self, canonical: FeatureRecord, dependencies: Dependencies) -> Option<Checked<'a>> {
        let manifest = self.registry.get(&canonical.kind)?.manifest();
        let sketch = (canonical.kind == SKETCH_TYPE)
            .then(|| serde_json::from_value(Value::Object(canonical.params)).ok())
            .flatten();
        Some(Checked {
            dependencies,
            manifest,
            sketch,
        })
    }

    /// Every valid record with its position and what step 2 found out about it, in timeline
    /// order.
    fn valid<'c>(
        &self,
        checked: &'c [Option<Checked<'a>>],
    ) -> impl Iterator<Item = (usize, &'a FeatureRecord, &'c Checked<'a>)> + use<'a, 'c> {
        self.document
            .features
            .iter()
            .zip(checked)
            .enumerate()
            .filter_map(|(position, (record, found))| Some((position, record, found.as_ref()?)))
    }

    /// Step 3: expression errors of parameters, then of feature params.
    fn expressions(&mut self, checked: &[Option<Checked<'a>>], scope: &Scope<'_>) {
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
        for (position, _, found) in self.valid(checked) {
            let uses = evaluated_uses(found);
            let values = Values::evaluate(&uses, scope, found.manifest);
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
    fn references(&mut self, checked: &[Option<Checked<'a>>]) {
        for (position, record, found) in self.valid(checked) {
            let manifest = found.manifest;
            let dependencies = &found.dependencies;
            for link in &dependencies.features {
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
            for link in dependencies
                .bodies_read
                .iter()
                .chain(&dependencies.bodies_written)
            {
                if self.document.body(&link.id).is_none() {
                    let error = FeatureError::UnknownBody {
                        field: FieldRef::new(&link.field, manifest),
                        body: link.id.clone(),
                    };
                    self.feature_error(position, "unknown_body", &error);
                }
            }
            for link in entity_links(record) {
                let Some(sketch) = self.earlier_sketch(checked, &link.sketch, position) else {
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
    fn earlier_sketch<'c>(
        &self,
        checked: &'c [Option<Checked<'a>>],
        id: &Id,
        position: usize,
    ) -> Option<&'c SketchParams> {
        let target = self.document.feature_index(id)?;
        if target >= position {
            return None;
        }
        checked.get(target)?.as_ref()?.sketch.as_ref()
    }

    /// Step 6: each valid sketch is solved and its solver issues reported, each with the message
    /// the sketch crate renders for the sketch and its solve (sketch.md section 4).
    fn sketches(&mut self, checked: &[Option<Checked<'a>>], scope: &Scope<'_>) {
        for (position, record, found) in self.valid(checked) {
            let Some(params) = &found.sketch else {
                continue;
            };
            let Ok(values) = dimension_values(params, scope) else {
                continue;
            };
            let path = Some(format!("/features/{position}"));
            let words = SketchWords::of(self.registry, self.document, &record.id);
            let solve = NativeSolver.solve(params, &values);
            let context = words.context(params, &values, solve.as_ref().ok());
            match &solve {
                Ok(result) => {
                    for issue in result.issues(params) {
                        let (severity, code) = match issue {
                            SolveIssue::Conflicting { .. } => {
                                (Severity::Error, "sketch_conflicting")
                            }
                            SolveIssue::Redundant { .. } => (Severity::Warning, "sketch_redundant"),
                            SolveIssue::NotConverged => (Severity::Error, "sketch_not_converged"),
                        };
                        self.push(severity, code, issue.render(&context), path.clone());
                    }
                }
                Err(error) => {
                    self.push(
                        Severity::Error,
                        "sketch_invalid",
                        error.render(&context),
                        path,
                    );
                }
            }
        }
    }

    /// Step 7: referenced assets that were not supplied or do not match their name. Returns the
    /// supplied assets that match.
    fn assets(
        &mut self,
        checked: &[Option<Checked<'a>>],
        assets: &[Asset],
    ) -> BTreeMap<String, Arc<[u8]>> {
        let mut stored = BTreeMap::new();
        for (position, _, found) in self.valid(checked) {
            for link in &found.dependencies.assets {
                match assets.iter().find(|asset| asset.name == link.name) {
                    None => {
                        let error = FeatureError::AssetMissing {
                            field: FieldRef::new(&link.field, found.manifest),
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
                            Some(&FieldRef::new(&link.field, found.manifest)),
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

/// The expression uses recompute evaluates: every declared use except the expressions of a
/// sketch's reference dimensions (`driving: false`), which the sketch feature never evaluates
/// (features/sketch.md).
fn evaluated_uses(found: &Checked<'_>) -> Vec<ExprUse> {
    let reference_fields = found
        .sketch
        .as_ref()
        .map(reference_dimension_fields)
        .unwrap_or_default();
    found
        .dependencies
        .expressions
        .iter()
        .filter(|usage| !reference_fields.contains(&usage.field))
        .cloned()
        .collect()
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
    message_location(position, field.map(FieldRef::pointer).as_deref())
}

/// The diagnostic of a document that does not load (step 1): the load error's code, message and
/// position.
fn load_diagnostic(error: LoadError) -> ValidationDiagnostic {
    let engine_error = EngineError::from_load(error);
    let (line, column, path) = match &engine_error {
        EngineError::Parse {
            line, column, path, ..
        } => (*line, *column, path.clone()),
        _ => (None, None, None),
    };
    let code = match engine_error {
        EngineError::UnsupportedFormatVersion { .. } => "unsupported_format_version",
        _ => "parse_error",
    };
    ValidationDiagnostic {
        line,
        column,
        path,
        ..ValidationDiagnostic::new(Severity::Error, code, engine_error.to_string())
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
    saturating_u32(newlines + 1)
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
    use crate::test_support::id;

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

    /// The code and message of every diagnostic `validate` reports, without the optional checks,
    /// for `drag.ptsm` (one line from (0, 0) to (10, 0), fixed and horizontal) with `constraint`
    /// added to its sketch.
    fn sketch_diagnostics(constraint: serde_json::Value) -> Vec<(String, String)> {
        let mut document: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/drag.ptsm"))
                .expect("drag.ptsm is JSON");
        document["features"][0]["constraints"]
            .as_array_mut()
            .expect("the sketch lists its constraints")
            .push(constraint);
        let mut session = Session::new(crate::SessionOptions::headless());

        session
            .validate(&document.to_string(), &[], ValidateOptions::default())
            .diagnostics
            .into_iter()
            .map(|diagnostic| (diagnostic.code, diagnostic.message))
            .collect()
    }

    #[test]
    fn a_sketch_the_solver_rejects_has_the_sketch_crate_message() {
        let diagnostics = sketch_diagnostics(serde_json::json!({
            "id": "c-4", "type": "distance", "points": ["e-1.start", "e-1.end"], "expr": "0 mm"
        }));

        assert_eq!(
            diagnostics,
            [(
                "sketch_invalid".to_string(),
                "Distance 0 mm must be greater than 0. To make two points meet, use Coincident instead."
                    .to_string()
            )]
        );
    }

    #[test]
    fn a_redundant_constraint_is_named_by_its_title() {
        let diagnostics = sketch_diagnostics(serde_json::json!({
            "id": "c-5", "type": "horizontal", "line": "e-1"
        }));

        assert_eq!(
            diagnostics,
            [(
                "sketch_redundant".to_string(),
                "In 'Sketch 1', Horizontal on the line from (0, 0) to (10, 0) is not needed: the other constraints already hold it."
                    .to_string()
            )]
        );
    }

    #[test]
    fn expression_codes_are_snake_case_kinds() {
        assert_eq!(
            expression_code(&ExprErrorKind::UnknownName {
                name: "width".to_string(),
                suggestion: None
            }),
            "unknown_name"
        );
        assert_eq!(
            expression_code(&ExprErrorKind::DivisionByZero),
            "division_by_zero"
        );
    }

    /// A repair of the element reference at `path` whose candidates have these queries.
    fn element_repair(path: &str, queries: &[&str]) -> Repair {
        let candidates: Vec<serde_json::Value> = queries
            .iter()
            .map(|query| {
                serde_json::json!({
                    "reference": {"q": query, "fp": "line|n=1,0,0|c=0,0,0|l=1"},
                    "description": "an edge",
                    "relation": "where it was",
                    "meaning": true,
                    "distance": 0.0,
                    "score": 1.0
                })
            })
            .collect();
        serde_json::from_value(serde_json::json!({
            "kind": "element",
            "path": path,
            "body": "b-1",
            "reference": {"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,0,0|l=1"},
            "resolution": "missing",
            "description": "the front top edge of 'Box 1'",
            "candidates": candidates
        }))
        .expect("a valid element repair")
    }

    /// The queries of candidates, in order.
    fn queries(candidates: Vec<RepairCandidate>) -> Vec<String> {
        candidates
            .into_iter()
            .map(|candidate| candidate.reference.query.to_string())
            .collect()
    }

    #[test]
    fn message_candidates_are_those_of_repairs_at_or_under_the_field() {
        let region: Repair = serde_json::from_value(serde_json::json!({
            "kind": "region",
            "path": "/edges/2",
            "sketch": "f-sk1",
            "region": {"key": "e-1", "fp": "c=0,0|a=1"},
            "resolution": "missing",
            "description": "the region of 'Sketch 1' around (0, 0)",
            "candidates": []
        }))
        .expect("a valid region repair");
        let repairs = [
            element_repair(
                "/edges/0",
                &[
                    "edge(f-box1/front|f-ext1/lateral/e-1)",
                    "edge(f-box1/top|f-ext1/lateral/e-2)",
                ],
            ),
            element_repair("/edges10", &["edge(f-box1/left|f-box1/top)"]),
            region,
            element_repair("/edges/1", &["edge(f-box1/back|f-box1/top)"]),
        ];

        assert_eq!(
            queries(message_candidates(&repairs, Some("/edges"))),
            [
                "edge(f-box1/front|f-ext1/lateral/e-1)",
                "edge(f-box1/top|f-ext1/lateral/e-2)",
                "edge(f-box1/back|f-box1/top)"
            ]
        );
        assert_eq!(
            queries(message_candidates(&repairs, Some("/edges/1"))),
            ["edge(f-box1/back|f-box1/top)"]
        );
        assert!(message_candidates(&repairs, Some("/radius")).is_empty());
        assert!(message_candidates(&repairs, None).is_empty());
    }

    #[test]
    fn message_locations_append_the_field() {
        assert_eq!(message_location(2, Some("/edges/0")), "/features/2/edges/0");
        assert_eq!(message_location(0, None), "/features/0");
    }

    fn record(json: serde_json::Value) -> FeatureRecord {
        serde_json::from_value(json).expect("a valid feature record")
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
