//! Python code generation (engine.md section 3.16, python-api.md section 3.8): the script layout,
//! variables and sketch blocks; every other feature's call comes from `Registry::python`.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};

use partsmith_features::{BodyOp, PythonCall, PythonNames, PythonValue, Registry};
use partsmith_model::{BodyRole, Document, FeatureRecord, Id, Units};
use partsmith_sketch::{
    Alignment, ConstraintKind, DistanceTarget, Geometry, HintFrame, ORIGIN, PointRef, PointRole,
    RegionRef, SketchConstraint, SketchEntity, SketchParams, SketchPlane, X_AXIS, Y_AXIS,
};
use serde_json::Value;

use partsmith_protocol::DocHandle;

use crate::error::EngineError;
use crate::session::Session;
use crate::sketch::{SKETCH_TYPE, sketch_params};
use crate::sketching::solved_regions;

/// Longest statement, indentation included, written on one line (rule 11).
const MAX_LINE_LENGTH: usize = 88;

/// Indentation of one block level.
const INDENT: usize = 4;

/// Names a variable cannot take (rule 7): Python keywords and soft keywords, the names of
/// `partsmith.__all__` (python-api.md section 2.3), and `doc`.
const RESERVED_VARIABLES: &[&str] = &[
    "False",
    "None",
    "True",
    "and",
    "as",
    "assert",
    "async",
    "await",
    "break",
    "class",
    "continue",
    "def",
    "del",
    "elif",
    "else",
    "except",
    "finally",
    "for",
    "from",
    "global",
    "if",
    "import",
    "in",
    "is",
    "lambda",
    "nonlocal",
    "not",
    "or",
    "pass",
    "raise",
    "return",
    "try",
    "while",
    "with",
    "yield",
    "_",
    "case",
    "match",
    "type",
    "doc",
    "Document",
    "current_document",
    "param",
    "Param",
    "rollback",
    "expr",
    "Expr",
    "mm",
    "cm",
    "meter",
    "inch",
    "deg",
    "rad",
    "printer",
    "Part",
    "Sketch",
    "Plane",
    "Placement",
    "Region",
    "Point",
    "Line",
    "Circle",
    "Arc",
    "Rectangle",
    "Polygon",
    "Coincident",
    "Horizontal",
    "Vertical",
    "Parallel",
    "Perpendicular",
    "Tangent",
    "Collinear",
    "Equal",
    "Concentric",
    "Midpoint",
    "PointOn",
    "Symmetric",
    "Fix",
    "Distance",
    "HorizontalDistance",
    "VerticalDistance",
    "Radius",
    "Diameter",
    "Angle",
    "Feature",
    "DatumPlane",
    "extrude",
    "revolve",
    "fillet",
    "chamfer",
    "shell",
    "hole",
    "box",
    "cylinder",
    "sphere",
    "cone",
    "torus",
    "boolean",
    "mirror",
    "linear_pattern",
    "circular_pattern",
    "datum_plane",
    "move",
    "split",
    "feature",
    "Axis",
    "Element",
    "Selection",
    "Ref",
    "faces",
    "edges",
    "vertices",
    "export_stl",
    "export_3mf",
    "export_obj",
    "export_step",
    "BoundingBox",
    "PrintReport",
    "Finding",
    "run_script",
    "run_code",
    "PartsmithError",
    "EngineError",
    "FeatureError",
    "SelectionError",
    "ExpressionError",
    "PartsmithWarning",
];

impl Session {
    /// The document as a Python script.
    pub fn export_python(&self, doc: DocHandle) -> Result<String, EngineError> {
        let open = self.open(doc)?;
        let regions =
            |sketch: &Id| solved_regions(&self.registry, &open.document, &open.computed, sketch);
        Ok(script(&open.document, &self.registry, &regions))
    }
}

/// The script that rebuilds `document`. `regions` gives every region of a sketch in solved
/// order, or `None` when the sketch does not solve.
pub(crate) fn script(
    document: &Document,
    registry: &Registry,
    regions: &dyn Fn(&Id) -> Option<Vec<RegionRef>>,
) -> String {
    let mut variables = Variables::new();
    for parameter in &document.parameters {
        let variable = variables.claim(&parameter.name);
        variables
            .parameters
            .insert(parameter.name.clone(), variable);
    }
    for (number, body) in (1..).zip(&document.bodies) {
        let variable = variables.claim(&base_name(Some(&body.name), "part", number));
        variables.parts.insert(body.id.clone(), variable);
    }
    let names = Names {
        document,
        variables: RefCell::new(variables),
        recording: Cell::new(true),
        requests: RefCell::default(),
        current: Cell::new(0),
        visible_parameters: Cell::new(document.parameters.len()),
        regions,
        solved: RefCell::default(),
    };
    let plans: Vec<Plan> = (0..document.features.len())
        .map(|position| plan(registry, &names, position))
        .collect();
    assign_timeline_variables(document, &plans, &names);
    names.recording.set(false);

    let mut sections = vec![
        "from partsmith import *".to_string(),
        document_line(document),
    ];
    let parameters = parameter_lines(document, &names);
    if !parameters.is_empty() {
        sections.push(parameters);
    }
    if !document.bodies.is_empty() {
        sections.push(part_lines(document, &names));
    }
    if !document.features.is_empty() {
        sections.push(timeline(document, registry, &plans, &names));
    }
    if let Some(rollback) = &document.rollback {
        sections.push(format!(
            "rollback({})",
            PythonValue::Str(rollback.to_string()).render()
        ));
    }
    let mut text = sections.join("\n\n");
    text.push('\n');
    text
}

/// Variables of the script, by what they name.
#[derive(Debug)]
struct Variables {
    /// Every variable name in use, plus the reserved names.
    taken: BTreeSet<String>,
    /// Parameter variables by parameter name.
    parameters: BTreeMap<String, String>,
    /// Part variables by body id.
    parts: BTreeMap<Id, String>,
    /// Feature variables by feature id.
    features: BTreeMap<Id, String>,
    /// Entity variables by sketch id and entity id.
    entities: BTreeMap<(Id, Id), String>,
}

impl Variables {
    /// No variables yet; only the reserved names are taken.
    fn new() -> Variables {
        Variables {
            taken: RESERVED_VARIABLES
                .iter()
                .map(|name| name.to_string())
                .collect(),
            parameters: BTreeMap::new(),
            parts: BTreeMap::new(),
            features: BTreeMap::new(),
            entities: BTreeMap::new(),
        }
    }

    /// Claims `base`, or the first free `base_2`, `base_3`, ... (rule 7).
    fn claim(&mut self, base: &str) -> String {
        let mut candidate = base.to_string();
        let mut suffix = 1;
        while self.taken.contains(&candidate) {
            suffix += 1;
            candidate = format!("{base}_{suffix}");
        }
        self.taken.insert(candidate.clone());
        candidate
    }
}

/// The variable base of an object (rule 7): its name in snake case, prefixed with `<kind>_` when
/// that starts with a digit or has one character, else `<kind>_<number>`.
fn base_name(name: Option<&str>, kind: &str, number: usize) -> String {
    let snake = name.map(snake_case).unwrap_or_default();
    if snake.is_empty() {
        format!("{kind}_{number}")
    } else if snake.starts_with(|character: char| character.is_ascii_digit())
        || snake.chars().count() == 1
    {
        format!("{kind}_{snake}")
    } else {
        snake
    }
}

/// Lowercase, each run of characters outside `[a-z0-9]` replaced by `_`, and leading and
/// trailing `_` removed.
fn snake_case(name: &str) -> String {
    let mut snake = String::with_capacity(name.len());
    for character in name.to_lowercase().chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            snake.push(character);
        } else if !snake.ends_with('_') {
            snake.push('_');
        }
    }
    snake.trim_matches('_').to_string()
}

/// How one feature is written.
#[derive(Debug, Clone)]
enum Plan {
    /// A `with Sketch(...)` block with its entities and constraints.
    Sketch(Box<SketchParams>),
    /// The registry's call.
    Call,
}

/// What the generator's `PythonNames` resolve against, and what the planning pass records.
struct Names<'a> {
    /// The document.
    document: &'a Document,
    /// The variables assigned so far.
    variables: RefCell<Variables>,
    /// True while planning: feature and entity lookups are recorded and answered with a
    /// placeholder.
    recording: Cell<bool>,
    /// Features and entities later statements refer to.
    requests: RefCell<Requests>,
    /// Timeline position of the feature being written.
    current: Cell<usize>,
    /// Number of parameters, in document order, an expression may name.
    visible_parameters: Cell<usize>,
    /// Solved regions of a sketch.
    regions: &'a dyn Fn(&Id) -> Option<Vec<RegionRef>>,
    /// Memoized regions by sketch.
    solved: RefCell<BTreeMap<Id, Option<Vec<RegionRef>>>>,
}

/// Objects that need a variable because a statement refers to them.
#[derive(Debug, Default)]
struct Requests {
    /// Features.
    features: BTreeSet<Id>,
    /// Sketch entities by sketch.
    entities: BTreeSet<(Id, Id)>,
}

impl Requests {
    /// Adds the requests of `other`.
    fn extend(&mut self, other: Requests) {
        self.features.extend(other.features);
        self.entities.extend(other.entities);
    }
}

impl Names<'_> {
    /// The timeline position of `feature` when it comes before the current feature.
    fn earlier(&self, feature: &Id) -> Option<usize> {
        let position = self.document.feature_index(feature)?;
        (position < self.current.get()).then_some(position)
    }

    /// The variable of an earlier feature: a placeholder while planning (recording the request).
    fn feature_variable(&self, feature: &Id) -> Option<String> {
        self.earlier(feature)?;
        if self.recording.get() {
            self.requests.borrow_mut().features.insert(feature.clone());
            return Some(feature.to_string());
        }
        self.variables.borrow().features.get(feature).cloned()
    }

    /// The variable of an entity of an earlier sketch: a placeholder while planning (recording
    /// the request). Only sketches written as blocks have entity variables.
    fn entity_variable(&self, sketch: &Id, entity: &Id) -> Option<String> {
        let position = self.earlier(sketch)?;
        let record = &self.document.features[position];
        if record.kind != SKETCH_TYPE || !stored_entities(record).contains(entity) {
            return None;
        }
        let key = (sketch.clone(), entity.clone());
        if self.recording.get() {
            self.requests.borrow_mut().entities.insert(key);
            return Some(entity.to_string());
        }
        self.variables.borrow().entities.get(&key).cloned()
    }
}

impl PythonNames for Names<'_> {
    fn units(&self) -> Units {
        self.document.units
    }

    fn body(&self, body: &Id) -> Option<String> {
        self.variables.borrow().parts.get(body).cloned()
    }

    fn feature(&self, feature: &Id) -> Option<String> {
        self.feature_variable(feature)
    }

    fn entity(&self, sketch: &Id, entity: &Id) -> Option<String> {
        self.entity_variable(sketch, entity)
    }

    fn parameter(&self, name: &str) -> Option<String> {
        let visible = self.visible_parameters.get();
        self.document.parameters[..visible.min(self.document.parameters.len())]
            .iter()
            .any(|parameter| parameter.name == name)
            .then(|| self.variables.borrow().parameters.get(name).cloned())
            .flatten()
    }

    fn regions(&self, sketch: &Id) -> Option<Vec<RegionRef>> {
        self.solved
            .borrow_mut()
            .entry(sketch.clone())
            .or_insert_with(|| (self.regions)(sketch))
            .clone()
    }

    fn default_op(&self, body: &Id) -> BodyOp {
        let current = self.current.get();
        let targeted = self.document.features[..current.min(self.document.features.len())]
            .iter()
            .any(|record| targets(record, body));
        if targeted { BodyOp::Join } else { BodyOp::New }
    }
}

/// True when a record's `body` param, or its `target` param, names `body` (python-api.md
/// section 3.3).
fn targets(record: &FeatureRecord, body: &Id) -> bool {
    ["body", "target"]
        .iter()
        .any(|key| record.params.get(*key).and_then(Value::as_str) == Some(body.as_str()))
}

/// The block part of a feature (rule 5): its `body` param (`target` for `boolean`) when it
/// names a document body.
fn own_part(document: &Document, record: &FeatureRecord) -> Option<Id> {
    let key = if record.kind == "boolean" {
        "target"
    } else {
        "body"
    };
    let body = record.params.get(key).and_then(Value::as_str)?;
    document
        .bodies
        .iter()
        .find(|candidate| candidate.id.as_str() == body)
        .map(|candidate| candidate.id.clone())
}

/// The entity ids a sketch record stores.
fn stored_entities(record: &FeatureRecord) -> Vec<Id> {
    record
        .params
        .get("entities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entity| entity.get("id").and_then(Value::as_str))
        .filter_map(|id| Id::new(id).ok())
        .collect()
}

/// Decides how the feature at `position` is written and records what it refers to.
fn plan(registry: &Registry, names: &Names<'_>, position: usize) -> Plan {
    names.current.set(position);
    let record = &names.document.features[position];
    if record.kind == SKETCH_TYPE
        && let Some(params) = block_params(registry, names, record)
    {
        let mut requests = names.requests.borrow_mut();
        if let SketchPlane::Datum { feature } = &params.plane {
            requests.features.insert(feature.clone());
        }
        for constraint in &params.constraints {
            for entity in constraint.referenced_entities() {
                requests
                    .entities
                    .insert((record.id.clone(), entity.clone()));
            }
        }
        return Plan::Sketch(Box::new(params));
    }
    record_call(registry, names, record);
    Plan::Call
}

/// Records what the registry's call of `record` refers to. A hook that gives up after looking up
/// some names leaves the generic form, which refers to none of them, so their requests are dropped
/// (rule 7).
fn record_call(registry: &Registry, names: &Names<'_>, record: &FeatureRecord) {
    let earlier = names.requests.take();
    let dedicated = registry.python_dedicated(record, names).is_some();
    let lookups = names.requests.replace(earlier);
    if dedicated {
        names.requests.borrow_mut().extend(lookups);
    }
}

/// The params of a sketch that can be written as a block: valid params, a plane with a variable
/// (an earlier datum, or a face of a document body), and constraints that refer only to the
/// sketch's own entities and the reserved references (rule 9).
fn block_params(
    registry: &Registry,
    names: &Names<'_>,
    record: &FeatureRecord,
) -> Option<SketchParams> {
    let params = sketch_params(registry, names.document, record).ok()?;
    let plane_ok = match &params.plane {
        SketchPlane::Origin { .. } => true,
        SketchPlane::Datum { feature } => names.earlier(feature).is_some(),
        SketchPlane::Face { body, .. } => names.body(body).is_some(),
    };
    let own_entities = params.constraints.iter().all(|constraint| {
        constraint
            .referenced_entities()
            .iter()
            .all(|entity| params.entity(entity).is_some())
    });
    (plane_ok && own_entities).then_some(params)
}

/// The Python name of a reserved sketch reference (sketch.md section 3.16, rule 9), `None` for an
/// entity id.
fn reserved_name(entity: &Id) -> Option<&'static str> {
    match entity.as_str() {
        ORIGIN => Some("Sketch.ORIGIN"),
        X_AXIS => Some("Sketch.X_AXIS"),
        Y_AXIS => Some("Sketch.Y_AXIS"),
        _ => None,
    }
}

/// Gives variables to the features and entities statements refer to, in emission order (rule 7).
fn assign_timeline_variables(document: &Document, plans: &[Plan], names: &Names<'_>) {
    let requests = names.requests.borrow();
    let mut variables = names.variables.borrow_mut();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut count = |kind: &str| {
        let count = counts.entry(kind.to_string()).or_insert(0);
        *count += 1;
        *count
    };
    for (record, plan) in document.features.iter().zip(plans) {
        let kind = match record.kind.as_str() {
            "datum_plane" => "plane",
            kind => kind,
        };
        let number = count(kind);
        if requests.features.contains(&record.id) {
            let variable = variables.claim(&base_name(record.name.as_deref(), kind, number));
            variables.features.insert(record.id.clone(), variable);
        }
        let Plan::Sketch(params) = plan else {
            continue;
        };
        let mut entity_counts: BTreeMap<&str, usize> = BTreeMap::new();
        for entity in &params.entities {
            let kind = entity_type(entity);
            let number = entity_counts.entry(kind).or_insert(0);
            *number += 1;
            let key = (record.id.clone(), entity.id.clone());
            if requests.entities.contains(&key) {
                let variable = variables.claim(&format!("{kind}_{number}"));
                variables.entities.insert(key, variable);
            }
        }
    }
}

/// The JSON type name of an entity.
fn entity_type(entity: &SketchEntity) -> &'static str {
    match entity.geometry {
        Geometry::Point { .. } => "point",
        Geometry::Line { .. } => "line",
        Geometry::Circle { .. } => "circle",
        Geometry::Arc { .. } => "arc",
    }
}

/// `doc = Document(units="<units>"[, printer="<id>"])` (rule 2).
fn document_line(document: &Document) -> String {
    let mut call = PythonCall::new("Document");
    call.keywords.push((
        "units".to_string(),
        PythonValue::Str(document.units.symbol().to_string()),
    ));
    if let Some(printer) = &document.printer {
        call.keywords
            .push(("printer".to_string(), PythonValue::Str(printer.clone())));
    }
    statement(0, "doc = ", &call, "")
}

/// One `param(...)` line per parameter (rule 3); an expression names only earlier parameters.
fn parameter_lines(document: &Document, names: &Names<'_>) -> String {
    let mut lines = Vec::with_capacity(document.parameters.len());
    for (position, parameter) in document.parameters.iter().enumerate() {
        names.visible_parameters.set(position);
        let mut call = PythonCall::new("param");
        call.positional = vec![
            PythonValue::Str(parameter.name.clone()),
            PythonValue::expression(&parameter.expr, names),
        ];
        if let Some(comment) = &parameter.comment {
            call.keywords
                .push(("comment".to_string(), PythonValue::Str(comment.clone())));
        }
        call.keywords
            .push(("id".to_string(), PythonValue::Str(parameter.id.to_string())));
        let variable = names.variables.borrow().parameters[&parameter.name].clone();
        lines.push(statement(0, &format!("{variable} = "), &call, ""));
    }
    names.visible_parameters.set(document.parameters.len());
    lines.join("\n")
}

/// One `Part(...)` line per body (rule 4), keywords only when they differ from their defaults.
fn part_lines(document: &Document, names: &Names<'_>) -> String {
    let variables = names.variables.borrow();
    let mut lines = Vec::with_capacity(document.bodies.len());
    for body in &document.bodies {
        let mut call = PythonCall::new("Part");
        call.positional.push(PythonValue::Str(body.name.clone()));
        if let Some(material) = &body.material {
            call.keywords
                .push(("material".to_string(), PythonValue::Str(material.clone())));
        }
        if let Some(color) = body.color {
            call.keywords
                .push(("color".to_string(), PythonValue::Str(color.to_string())));
        }
        if !body.visible {
            call.keywords
                .push(("visible".to_string(), PythonValue::Bool(false)));
        }
        if body.role != BodyRole::Part
            && let Ok(Value::String(role)) = serde_json::to_value(body.role)
        {
            call.keywords
                .push(("role".to_string(), PythonValue::Str(role)));
        }
        call.keywords
            .push(("id".to_string(), PythonValue::Str(body.id.to_string())));
        lines.push(statement(
            0,
            &format!("{} = ", variables.parts[&body.id]),
            &call,
            "",
        ));
    }
    lines.join("\n")
}

/// The timeline (rules 5 and 6): runs of features with the same block part in one `with` block,
/// features without one at top level.
fn timeline(document: &Document, registry: &Registry, plans: &[Plan], names: &Names<'_>) -> String {
    let parts: Vec<Option<Id>> = document
        .features
        .iter()
        .map(|record| own_part(document, record))
        .collect();
    let mut blocks: Vec<Option<Id>> = vec![None; parts.len()];
    let mut next = None;
    for position in (0..parts.len()).rev() {
        if parts[position].is_some() {
            next = parts[position].clone();
        }
        blocks[position] = next.clone();
    }
    let mut lines = Vec::new();
    let mut position = 0;
    while position < blocks.len() {
        let block = blocks[position].clone();
        let mut end = position;
        while end < blocks.len() && blocks[end] == block {
            end += 1;
        }
        let indent = match &block {
            Some(body) => {
                lines.push(format!("with {}:", names.variables.borrow().parts[body]));
                INDENT
            }
            None => 0,
        };
        let features = document.features.iter().zip(plans).enumerate();
        for (current, (record, plan)) in features.take(end).skip(position) {
            names.current.set(current);
            let variable = names.variables.borrow().features.get(&record.id).cloned();
            lines.push(match plan {
                Plan::Sketch(params) => {
                    sketch_block(record, params, variable.as_deref(), indent, names)
                }
                Plan::Call => {
                    let call = registry.python(record, names);
                    let prefix = variable
                        .map(|variable| format!("{variable} = "))
                        .unwrap_or_default();
                    statement(indent, &prefix, &call, "")
                }
            });
        }
        position = end;
    }
    lines.join("\n")
}

/// A sketch as `with Sketch(...) [as <var>]:` with its entities, then its constraints (rule 6).
fn sketch_block(
    record: &FeatureRecord,
    params: &SketchParams,
    variable: Option<&str>,
    indent: usize,
    names: &Names<'_>,
) -> String {
    let units = names.document.units;
    let mut call = PythonCall::new("Sketch");
    match &params.plane {
        SketchPlane::Origin { plane } => call.positional.push(PythonValue::Name(format!(
            "Plane.{}",
            match plane {
                partsmith_sketch::OriginPlane::Xy => "XY",
                partsmith_sketch::OriginPlane::Xz => "XZ",
                partsmith_sketch::OriginPlane::Yz => "YZ",
            }
        ))),
        SketchPlane::Datum { feature } => call.positional.push(PythonValue::Name(
            names.feature_variable(feature).unwrap_or_default(),
        )),
        SketchPlane::Face {
            body,
            face,
            fallback,
            fallback_frame,
        } => {
            call.positional
                .push(PythonValue::reference(face, names.body(body)));
            let mut placement = PythonCall::new("Placement");
            placement.positional = vec![
                PythonValue::Tuple(
                    fallback
                        .origin
                        .iter()
                        .map(|&value| PythonValue::length(value, units))
                        .collect(),
                ),
                plain_tuple(&fallback.x_axis),
                plain_tuple(&fallback.normal),
            ];
            call.keywords.push((
                "fallback".to_string(),
                PythonValue::Call(Box::new(placement)),
            ));
            // A fallback in world coordinates, the default, is written without its frame, as
            // before file format version 2 (python-api.md section 3.8 rule 9).
            if *fallback_frame == HintFrame::Body {
                call.keywords.push((
                    "fallback_frame".to_string(),
                    PythonValue::Str("body".to_string()),
                ));
            }
        }
    }
    record_keywords(record, &mut call.keywords);
    let suffix = match variable {
        Some(variable) => format!(" as {variable}:"),
        None => ":".to_string(),
    };
    let mut lines = vec![statement(indent, "with ", &call, &suffix)];
    let inner = indent + INDENT;
    let entity = |id: &Id| {
        names
            .variables
            .borrow()
            .entities
            .get(&(record.id.clone(), id.clone()))
            .cloned()
    };
    for stored in &params.entities {
        let call = entity_call(stored, units);
        let prefix = entity(&stored.id)
            .map(|variable| format!("{variable} = "))
            .unwrap_or_default();
        lines.push(statement(inner, &prefix, &call, ""));
    }
    for constraint in &params.constraints {
        let call = constraint_call(constraint, &entity, names);
        lines.push(statement(inner, "", &call, ""));
    }
    if params.entities.is_empty() && params.constraints.is_empty() {
        lines.push(format!("{}pass", " ".repeat(inner)));
    }
    lines.join("\n")
}

/// A tuple of plain numbers in canonical form.
fn plain_tuple(values: &[f64; 3]) -> PythonValue {
    // A raw length in millimeters is written as the plain canonical number.
    PythonValue::Tuple(
        values
            .iter()
            .map(|&value| PythonValue::length(value, Units::Mm))
            .collect(),
    )
}

/// The `name=`, `suppressed=True` and `id=` keywords of a record, each only when set.
fn record_keywords(record: &FeatureRecord, keywords: &mut Vec<(String, PythonValue)>) {
    if let Some(name) = &record.name {
        keywords.push(("name".to_string(), PythonValue::Str(name.clone())));
    }
    if record.suppressed {
        keywords.push(("suppressed".to_string(), PythonValue::Bool(true)));
    }
    keywords.push(("id".to_string(), PythonValue::Str(record.id.to_string())));
}

/// The call that creates a sketch entity with its stored geometry (raw numbers, rule 9).
fn entity_call(entity: &SketchEntity, units: Units) -> PythonCall {
    let length = |value: f64| PythonValue::length(value, units);
    let point = |point: [f64; 2]| PythonValue::Tuple(vec![length(point[0]), length(point[1])]);
    let mut call;
    match entity.geometry {
        Geometry::Point { position } => {
            call = PythonCall::new("Point");
            call.positional = vec![length(position[0]), length(position[1])];
        }
        Geometry::Line { start, end } => {
            call = PythonCall::new("Line");
            call.positional = vec![point(start), point(end)];
        }
        Geometry::Circle { center, radius } => {
            call = PythonCall::new("Circle");
            call.positional = vec![length(radius)];
            if center != [0.0, 0.0] {
                call.keywords.push(("center".to_string(), point(center)));
            }
        }
        Geometry::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            call = PythonCall::new("Arc");
            call.positional = vec![
                point(center),
                length(radius),
                PythonValue::angle(start_angle),
                PythonValue::angle(end_angle),
            ];
        }
    }
    if entity.construction {
        call.keywords
            .push(("construction".to_string(), PythonValue::Bool(true)));
    }
    call.keywords
        .push(("id".to_string(), PythonValue::Str(entity.id.to_string())));
    call
}

/// The constructor call of a constraint (python-api.md section 3.4).
fn constraint_call(
    constraint: &SketchConstraint,
    entity: &dyn Fn(&Id) -> Option<String>,
    names: &Names<'_>,
) -> PythonCall {
    // A reserved reference is written by its name; an entity by its variable.
    let name = |id: &Id| {
        reserved_name(id)
            .map(str::to_owned)
            .or_else(|| entity(id))
            .unwrap_or_default()
    };
    let variable = |id: &Id| PythonValue::Name(name(id));
    let point = |point: &PointRef| {
        let base = name(&point.entity);
        PythonValue::Name(match point.role {
            PointRole::Point => base,
            PointRole::Start => format!("{base}.start"),
            PointRole::End => format!("{base}.end"),
            PointRole::Center => format!("{base}.center"),
        })
    };
    let alignment = |target: &Alignment| match target {
        Alignment::Line { line } => vec![variable(line)],
        Alignment::Points { points } => points.iter().map(point).collect(),
    };
    let (function, mut positional, dimension) = match &constraint.kind {
        ConstraintKind::Coincident { points } => {
            ("Coincident", points.iter().map(point).collect(), None)
        }
        ConstraintKind::PointOnObject { point: on, curve } => {
            ("PointOn", vec![point(on), variable(curve)], None)
        }
        ConstraintKind::Horizontal(target) => ("Horizontal", alignment(target), None),
        ConstraintKind::Vertical(target) => ("Vertical", alignment(target), None),
        ConstraintKind::Parallel { lines } => {
            ("Parallel", lines.iter().map(variable).collect(), None)
        }
        ConstraintKind::Perpendicular { lines } => {
            ("Perpendicular", lines.iter().map(variable).collect(), None)
        }
        ConstraintKind::Collinear { lines } => {
            ("Collinear", lines.iter().map(variable).collect(), None)
        }
        ConstraintKind::Tangent { curves } => {
            ("Tangent", curves.iter().map(variable).collect(), None)
        }
        ConstraintKind::Concentric { curves } => {
            ("Concentric", curves.iter().map(variable).collect(), None)
        }
        ConstraintKind::Equal { curves } => ("Equal", curves.iter().map(variable).collect(), None),
        ConstraintKind::Symmetric { points, line } => (
            "Symmetric",
            vec![point(&points[0]), point(&points[1]), variable(line)],
            None,
        ),
        ConstraintKind::Midpoint {
            point: middle,
            line,
        } => ("Midpoint", vec![point(middle), variable(line)], None),
        ConstraintKind::Fix { target } => ("Fix", vec![point(target)], None),
        ConstraintKind::Distance { target, dimension } => {
            let targets = match target {
                DistanceTarget::Points { points } => points.iter().map(point).collect(),
                DistanceTarget::PointLine { point: from, line } => {
                    vec![point(from), variable(line)]
                }
                DistanceTarget::Line { line } => vec![variable(line)],
            };
            ("Distance", targets, Some(dimension))
        }
        ConstraintKind::HorizontalDistance { target, dimension } => {
            ("HorizontalDistance", alignment(target), Some(dimension))
        }
        ConstraintKind::VerticalDistance { target, dimension } => {
            ("VerticalDistance", alignment(target), Some(dimension))
        }
        ConstraintKind::Radius { curve, dimension } => {
            ("Radius", vec![variable(curve)], Some(dimension))
        }
        ConstraintKind::Diameter { curve, dimension } => {
            ("Diameter", vec![variable(curve)], Some(dimension))
        }
        ConstraintKind::Angle { lines, dimension } => (
            "Angle",
            lines.iter().map(variable).collect(),
            Some(dimension),
        ),
    };
    let mut call = PythonCall::new(function);
    if let Some(dimension) = dimension {
        positional.push(PythonValue::expression(&dimension.expr, names));
        if !dimension.driving {
            call.keywords
                .push(("driving".to_string(), PythonValue::Bool(false)));
        }
    }
    call.positional = positional;
    call.keywords.push((
        "id".to_string(),
        PythonValue::Str(constraint.id.to_string()),
    ));
    call
}

/// One statement: `<prefix><call><suffix>` at `indent`, on one line when that fits in 88
/// characters, else with one argument per line (rule 11).
fn statement(indent: usize, prefix: &str, call: &PythonCall, suffix: &str) -> String {
    let padding = " ".repeat(indent);
    let arguments = arguments(call);
    let line = format!(
        "{padding}{prefix}{}({}){suffix}",
        call.function,
        arguments.join(", ")
    );
    if arguments.is_empty() || line.chars().count() <= MAX_LINE_LENGTH {
        return line;
    }
    let argument_padding = " ".repeat(indent + INDENT);
    let mut text = format!("{padding}{prefix}{}(\n", call.function);
    for argument in &arguments {
        text.push_str(&argument_padding);
        text.push_str(argument);
        text.push_str(",\n");
    }
    text.push_str(&padding);
    text.push(')');
    text.push_str(suffix);
    text
}

/// Each argument of a call as text: positional, keywords, then `**{...}` for `extra`.
fn arguments(call: &PythonCall) -> Vec<String> {
    let mut arguments: Vec<String> = call.positional.iter().map(PythonValue::render).collect();
    arguments.extend(
        call.keywords
            .iter()
            .map(|(name, value)| format!("{name}={}", value.render())),
    );
    if !call.extra.is_empty() {
        arguments.push(format!(
            "**{}",
            PythonValue::Dict(call.extra.clone()).render()
        ));
    }
    arguments
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::id;

    #[test]
    fn names_become_snake_case() {
        assert_eq!(snake_case("Lid (top)"), "lid_top");
        assert_eq!(snake_case("  Big--Box  "), "big_box");
        assert_eq!(snake_case("Été"), "t");
        assert_eq!(snake_case("!!!"), "");
    }

    #[test]
    fn bases_prefix_digits_and_single_characters() {
        assert_eq!(base_name(Some("3D"), "part", 1), "part_3d");
        assert_eq!(base_name(Some("A"), "part", 1), "part_a");
        assert_eq!(base_name(Some("Block"), "part", 1), "block");
        assert_eq!(base_name(Some("***"), "part", 4), "part_4");
        assert_eq!(base_name(None, "sketch", 2), "sketch_2");
    }

    #[test]
    fn reserved_and_taken_names_get_a_suffix() {
        let mut variables = Variables::new();

        assert_eq!(variables.claim("class"), "class_2");
        assert_eq!(variables.claim("box"), "box_2");
        assert_eq!(variables.claim("block"), "block");
        assert_eq!(variables.claim("block"), "block_2");
        assert_eq!(variables.claim("doc"), "doc_2");
        assert_eq!(variables.claim("match"), "match_2");
    }

    #[test]
    fn short_statements_stay_on_one_line() {
        let mut call = PythonCall::new("box");
        call.positional = vec![PythonValue::Number("20".to_string())];
        call.keywords
            .push(("id".to_string(), PythonValue::Str("f-1".to_string())));

        assert_eq!(statement(4, "", &call, ""), r#"    box(20, id="f-1")"#);
        assert_eq!(statement(0, "b = ", &call, ""), r#"b = box(20, id="f-1")"#);
    }

    #[test]
    fn long_statements_put_one_argument_per_line() {
        let mut call = PythonCall::new("Sketch");
        call.positional = vec![PythonValue::Str("x".repeat(80))];
        call.keywords
            .push(("id".to_string(), PythonValue::Str("f-sk2".to_string())));

        let text = statement(0, "with ", &call, " as sketch_1:");

        assert_eq!(
            text,
            format!(
                "with Sketch(\n    \"{}\",\n    id=\"f-sk2\",\n) as sketch_1:",
                "x".repeat(80)
            )
        );
    }

    #[test]
    fn statements_match_the_registry_rendering() {
        let mut call = PythonCall::new("feature");
        call.positional = vec![PythonValue::Str("wedge".to_string())];
        call.extra
            .push(("from".to_string(), PythonValue::Number("1".to_string())));

        assert_eq!(statement(4, "", &call, ""), call.render(4));
    }

    #[test]
    fn arcs_write_their_angles_in_radians() {
        let entity = SketchEntity {
            id: id("e-1"),
            geometry: Geometry::Arc {
                center: [0.0, 0.0],
                radius: 5.0,
                start_angle: 0.0,
                end_angle: 1.5,
            },
            construction: true,
        };

        let call = entity_call(&entity, Units::Mm);

        assert_eq!(
            statement(0, "", &call, ""),
            r#"Arc((0, 0), 5, 0*rad, 1.5*rad, construction=True, id="e-1")"#
        );
    }

    #[test]
    fn circles_at_the_origin_omit_their_center() {
        let circle = |center| SketchEntity {
            id: id("e-1"),
            geometry: Geometry::Circle {
                center,
                radius: 5.0,
            },
            construction: false,
        };

        assert_eq!(
            statement(0, "", &entity_call(&circle([0.0, 0.0]), Units::Mm), ""),
            r#"Circle(5, id="e-1")"#
        );
        assert_eq!(
            statement(0, "", &entity_call(&circle([1.0, 2.0]), Units::In), ""),
            r#"Circle(5*mm, center=(1*mm, 2*mm), id="e-1")"#
        );
    }

    #[test]
    fn records_target_their_body_or_target_param() {
        let mut record = FeatureRecord::new(id("f-1"), "boolean");
        record
            .params
            .insert("target".to_string(), Value::String("b-2".to_string()));

        assert!(targets(&record, &id("b-2")));
        assert!(!targets(&record, &id("b-1")));
    }

    #[test]
    fn reserved_references_are_written_by_their_python_names() {
        assert_eq!(reserved_name(&id("origin")), Some("Sketch.ORIGIN"));
        assert_eq!(reserved_name(&id("x_axis")), Some("Sketch.X_AXIS"));
        assert_eq!(reserved_name(&id("y_axis")), Some("Sketch.Y_AXIS"));
        assert_eq!(reserved_name(&id("e-1")), None);
    }
}
