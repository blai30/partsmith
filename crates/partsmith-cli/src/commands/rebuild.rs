//! `partsmith rebuild` (cli.md section 3.3).

use std::collections::BTreeMap;
use std::io;

use partsmith_engine::{EngineError, PrintRequest, Session, message_candidates, message_location};
use partsmith_model::{Body, Dimension, Document, Id};
use partsmith_protocol::{
    BoundingBox, Diagnostic, DocHandle, FeatureState, FeatureStatus, MeasureTarget, Measurement,
    Orientation, ParameterValue, PrintReport, Repair, Severity, Vec3,
};
use serde::Serialize;

use super::{open, usage_error};
use crate::arguments::RebuildArguments;
use crate::error::CliError;
use crate::messages::{MessageMembers, write_suggested};
use crate::output::{JsonDiagnostic, Output, count, human_number, wire_name};
use crate::rotation;
use crate::status::Status;

/// How far a message line, or a print finding's line, is indented under its feature's line or the
/// print health line.
const MESSAGE_INDENT: usize = 4;

/// The report of one rebuild, printed as text or as the JSON document of section 3.3.
#[derive(Debug, Serialize)]
struct Report {
    /// The document path as given.
    path: String,
    /// The id of the printer profile in effect.
    printer: String,
    /// Every feature in timeline order.
    features: Vec<FeatureReport>,
    /// Every parameter in document order.
    parameters: Vec<ParameterReport>,
    /// Every body in document order.
    bodies: Vec<BodyReport>,
    /// The print analysis, with `--checks`.
    #[serde(skip_serializing_if = "Option::is_none")]
    checks: Option<PrintReport>,
}

/// The status of one feature.
#[derive(Debug, Serialize)]
struct FeatureReport {
    /// The feature id.
    id: Id,
    /// The feature type.
    #[serde(rename = "type")]
    kind: String,
    /// Its status at the model state.
    status: FeatureState,
    /// Its error first, then its warnings, then its info notices.
    messages: Vec<MessageReport>,
}

/// One message of a feature (cli.md section 2.2).
#[derive(Debug, Serialize)]
struct MessageReport {
    /// Error, warning or info.
    severity: Severity,
    /// Stable snake_case code.
    code: String,
    /// The rendered text.
    message: String,
    /// Where it is in the document: `/features/<index>` followed by the message's field. Text
    /// reports print it; in JSON the field alone says it.
    #[serde(skip)]
    location: String,
    /// Its field, suggestion, actions, candidates, reason, root and detail.
    #[serde(flatten)]
    members: MessageMembers,
}

/// The value of one parameter, or why it has none.
#[derive(Debug, Serialize)]
struct ParameterReport {
    /// The parameter id.
    id: Id,
    /// The parameter name.
    name: String,
    /// The value in internal units.
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<f64>,
    /// The dimension of the value.
    #[serde(skip_serializing_if = "Option::is_none")]
    dimension: Option<Dimension>,
    /// The evaluation error.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// The metrics of one body, or the fact that it has no geometry.
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum BodyReport {
    /// A body with geometry.
    Measured(BodyMetrics),
    /// A body without geometry.
    Empty {
        /// The body id.
        id: Id,
        /// The body name.
        name: String,
        /// Always true.
        empty: bool,
    },
}

/// The mass properties and element counts of one body.
#[derive(Debug, Serialize)]
struct BodyMetrics {
    /// The body id.
    id: Id,
    /// The body name.
    name: String,
    /// Volume in mm^3.
    volume: f64,
    /// Area in mm^2.
    area: f64,
    /// Center of mass at uniform density.
    center_of_mass: Vec3,
    /// Exact bounding box.
    bounding_box: BoundingBox,
    /// Displayed faces.
    faces: u32,
    /// Displayed edges.
    edges: u32,
    /// Displayed vertices.
    vertices: u32,
}

/// What `rebuild --json` prints when the file could not be rebuilt.
#[derive(Debug, Serialize)]
struct FailedReport {
    /// The document path as given.
    path: String,
    /// The problem that stopped the rebuild.
    error: JsonDiagnostic,
}

/// Rebuilds the document and prints its report.
pub fn run(
    session: &mut Session,
    arguments: &RebuildArguments,
    output: &mut Output<'_>,
) -> io::Result<Status> {
    let orientation = match rotation::orientation(&arguments.rotate) {
        Ok(orientation) => orientation.unwrap_or(Orientation::AsModeled),
        Err(error) => return Ok(usage_error(output, error)),
    };
    let path = arguments.file.display().to_string();
    match rebuild(session, arguments, orientation, path.clone()) {
        Ok(report) => {
            if arguments.json {
                output.json(&report)?;
            } else {
                report.write_human(output)?;
            }
            Ok(report.status(arguments.deny_warnings))
        }
        Err(error) => {
            if arguments.json {
                output.json(&FailedReport {
                    path,
                    error: JsonDiagnostic::from(&error),
                })?;
            } else {
                output.error(&error);
            }
            Ok(error.status())
        }
    }
}

/// Opens, rebuilds and measures the document, and runs the print checks when asked.
fn rebuild(
    session: &mut Session,
    arguments: &RebuildArguments,
    orientation: Orientation,
    path: String,
) -> Result<Report, CliError> {
    let opened = open(session, &arguments.file, arguments.printer.as_deref())?;
    let doc = opened.doc;
    let document = session.document(doc)?;
    let features = feature_reports(document, session.feature_statuses(doc)?)?;
    let parameters = parameter_reports(document, session.parameter_values(doc)?)?;
    let declared = document.bodies.clone();
    let bodies = declared
        .iter()
        .map(|body| measure(session, doc, body))
        .collect::<Result<Vec<_>, _>>()?;
    let checks = if arguments.checks {
        let request = PrintRequest {
            profile: None,
            orientation,
            bodies: None,
        };
        Some(session.analyze_print(doc, &request)?.report)
    } else {
        None
    };
    Ok(Report {
        path,
        printer: opened.printer,
        features,
        parameters,
        bodies,
        checks,
    })
}

/// Pairs each feature status with the feature's type, and each of its messages with its location
/// and candidates. The engine reports the status of every feature of the document (engine.md
/// section 2), so a status without a record is an internal error.
fn feature_reports(
    document: &Document,
    statuses: Vec<FeatureStatus>,
) -> Result<Vec<FeatureReport>, CliError> {
    let records: BTreeMap<&Id, (usize, &str)> = document
        .features
        .iter()
        .enumerate()
        .map(|(index, record)| (&record.id, (index, record.kind.as_str())))
        .collect();
    statuses
        .into_iter()
        .map(|status| {
            let (index, kind) = recorded(&records, &status.id, "the status of feature")?;
            let messages = status
                .messages
                .into_iter()
                .map(|message| message_report(message, &status.repairs, index))
                .collect();
            Ok(FeatureReport {
                kind: kind.to_owned(),
                id: status.id,
                status: status.status,
                messages,
            })
        })
        .collect()
}

/// A message of the feature at `index` in the timeline, with the candidates of its `repairs` for
/// its field.
fn message_report(message: Diagnostic, repairs: &[Repair], index: usize) -> MessageReport {
    let field = message.field.as_deref();
    let location = message_location(index, field);
    let candidates = message_candidates(repairs, field);
    MessageReport {
        severity: message.severity,
        code: message.code.clone(),
        message: message.message.clone(),
        location,
        members: MessageMembers::new(message, candidates),
    }
}

/// Pairs each parameter value with the parameter's name. The engine reports the value of every
/// parameter of the document (engine.md section 2), so a value without a record is an internal
/// error.
fn parameter_reports(
    document: &Document,
    values: Vec<ParameterValue>,
) -> Result<Vec<ParameterReport>, CliError> {
    let names: BTreeMap<&Id, &str> = document
        .parameters
        .iter()
        .map(|parameter| (&parameter.id, parameter.name.as_str()))
        .collect();
    values
        .into_iter()
        .map(|value| {
            let name = recorded(&names, &value.id, "the value of parameter")?;
            Ok(ParameterReport {
                name: name.to_owned(),
                id: value.id,
                value: value.value,
                dimension: value.dimension,
                error: value.error,
            })
        })
        .collect()
}

/// What the document records for `id` in `records`, such as a parameter's name or a feature's
/// position and type. The engine reports only records of the document, so a missing one is an
/// internal error naming what was `reported`, for example "the status of feature".
fn recorded<T: Copy>(records: &BTreeMap<&Id, T>, id: &Id, reported: &str) -> Result<T, CliError> {
    records.get(id).copied().ok_or_else(|| CliError::Internal {
        detail: format!("the engine reported {reported} {id}, which the document does not have"),
    })
}

/// Measures one body; a body without geometry is reported as empty.
fn measure(session: &mut Session, doc: DocHandle, body: &Body) -> Result<BodyReport, CliError> {
    let target = MeasureTarget::Body {
        body: body.id.clone(),
    };
    match session.measure(doc, &[target]) {
        Ok(measurement) => body_metrics(body, measurement).map(BodyReport::Measured),
        Err(EngineError::BodyEmpty { .. }) => Ok(BodyReport::Empty {
            id: body.id.clone(),
            name: body.name.clone(),
            empty: true,
        }),
        Err(error) => Err(error.into()),
    }
}

/// The metrics of a body measurement, which has every body field (engine.md section 3.11).
fn body_metrics(body: &Body, measurement: Measurement) -> Result<BodyMetrics, CliError> {
    let incomplete = || CliError::Internal {
        detail: format!(
            "the engine measured body {} without every body field",
            body.id
        ),
    };
    Ok(BodyMetrics {
        id: body.id.clone(),
        name: body.name.clone(),
        volume: measurement.volume.ok_or_else(incomplete)?,
        area: measurement.area.ok_or_else(incomplete)?,
        center_of_mass: measurement.center_of_mass.ok_or_else(incomplete)?,
        bounding_box: measurement.bounding_box.ok_or_else(incomplete)?,
        faces: measurement.face_count.ok_or_else(incomplete)?,
        edges: measurement.edge_count.ok_or_else(incomplete)?,
        vertices: measurement.vertex_count.ok_or_else(incomplete)?,
    })
}

impl Report {
    /// Exit 1 when a feature failed, or has warnings when warnings are denied. Print findings
    /// never change the exit code.
    fn status(&self, deny_warnings: bool) -> Status {
        let fails = |feature: &FeatureReport| match feature.status {
            FeatureState::Error => true,
            FeatureState::Warning => deny_warnings,
            FeatureState::Ok | FeatureState::Suppressed | FeatureState::RolledBack => false,
        };
        if self.features.iter().any(fails) {
            Status::Problems
        } else {
            Status::Success
        }
    }

    /// Prints the human report: the path, one line per feature and message, one line per body,
    /// and with checks the print health and one line per finding, followed by its suggestion's
    /// words when it has any.
    fn write_human(&self, output: &mut Output<'_>) -> io::Result<()> {
        output.line(&self.path)?;
        for feature in &self.features {
            output.line(format_args!(
                "  {} {} {}",
                feature.id,
                feature.kind,
                wire_name(&feature.status)
            ))?;
            for message in &feature.messages {
                output.line(format_args!(
                    "{:MESSAGE_INDENT$}{}[{}] at {}: {}",
                    "",
                    wire_name(&message.severity),
                    message.code,
                    message.location,
                    message.message
                ))?;
                message.members.write_lines(output, MESSAGE_INDENT)?;
            }
        }
        for body in &self.bodies {
            output.line(body.human_line())?;
        }
        if let Some(checks) = &self.checks {
            write_findings(output, checks)?;
        }
        Ok(())
    }
}

/// Prints the print health line of `checks` and one line per finding, followed by its suggestion's
/// words when it has any (section 3.3).
fn write_findings(output: &mut Output<'_>, checks: &PrintReport) -> io::Result<()> {
    output.line(format_args!(
        "  print health: {} ({})",
        checks.summary.label, checks.profile
    ))?;
    for body in &checks.bodies {
        for check in &body.report.checks {
            for finding in &check.findings {
                output.line(format_args!(
                    "{:MESSAGE_INDENT$}{}[{}] {}: {}",
                    "",
                    wire_name(&finding.severity),
                    wire_name(&finding.check),
                    body.body,
                    finding.message
                ))?;
                let words = finding
                    .suggestion
                    .as_ref()
                    .and_then(|suggestion| suggestion.text());
                if let Some(words) = words {
                    write_suggested(output, MESSAGE_INDENT, &words)?;
                }
            }
        }
    }
    Ok(())
}

impl BodyReport {
    /// The human line of one body.
    fn human_line(&self) -> String {
        match self {
            BodyReport::Measured(metrics) => format!(
                "  {} \"{}\": volume {} mm^3, area {} mm^2, bounding box {} to {} mm, {}, {}, {}",
                metrics.id,
                metrics.name,
                human_number(metrics.volume),
                human_number(metrics.area),
                human_point(metrics.bounding_box.min),
                human_point(metrics.bounding_box.max),
                count(metrics.faces, "face", "faces"),
                count(metrics.edges, "edge", "edges"),
                count(metrics.vertices, "vertex", "vertices"),
            ),
            BodyReport::Empty { id, name, .. } => format!("  {id} \"{name}\": no geometry"),
        }
    }
}

/// A point for people, for example `(-10, -15, 0)`.
fn human_point(point: Vec3) -> String {
    let [x, y, z] = point.map(human_number);
    format!("({x}, {y}, {z})")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::{candidate, capture, id, pick_edge_again};

    fn block() -> BodyMetrics {
        BodyMetrics {
            id: id("b-1"),
            name: "Block".to_owned(),
            volume: 24000.0,
            area: 5200.0,
            center_of_mass: [0.0, 0.0, 20.0],
            bounding_box: BoundingBox {
                min: [-10.0, -15.0, 0.0],
                max: [10.0, 15.0, 40.0],
            },
            faces: 6,
            edges: 12,
            vertices: 8,
        }
    }

    fn feature(status: FeatureState, messages: Vec<Diagnostic>) -> FeatureReport {
        FeatureReport {
            id: id("f-box1"),
            kind: "box".to_owned(),
            status,
            messages: messages
                .into_iter()
                .map(|message| message_report(message, &[], 0))
                .collect(),
        }
    }

    /// A repair of the reference at `path` with one candidate of this query.
    fn element_repair(path: &str, query: &str) -> Repair {
        serde_json::from_value(json!({
            "kind": "element",
            "path": path,
            "body": "b-1",
            "reference": {"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-15,40|l=20"},
            "resolution": "missing",
            "description": "the front top edge of 'Box 1'",
            "candidates": [candidate(query, "on the front side of 'Box 1', 5 mm from where it was")]
        }))
        .expect("a valid element repair")
    }

    /// The `references_not_found` error of a fillet's `/edges`, with one pick action.
    fn lost_edges() -> Diagnostic {
        Diagnostic {
            field: Some("/edges".to_owned()),
            actions: vec![pick_edge_again()],
            ..Diagnostic::new(
                Severity::Error,
                "references_not_found",
                "The front top edge of 'Box 1' selected for Edges was cut away.",
            )
        }
    }

    fn report(features: Vec<FeatureReport>, bodies: Vec<BodyReport>) -> Report {
        Report {
            path: "box.ptsm".to_owned(),
            printer: "generic-fdm".to_owned(),
            features,
            parameters: Vec::new(),
            bodies,
            checks: None,
        }
    }

    fn human(report: &Report) -> String {
        let ((), stdout, stderr) = capture(|output| report.write_human(output));
        assert!(stderr.is_empty());
        stdout
    }

    /// The message of the bed fit finding of `checked_report`.
    const FOOTPRINT_MESSAGE: &str = "The part footprint is 300 x 30 mm, larger than the 256 x 256 mm bed. Split the part or scale it down.";

    /// A report with print checks whose body `b-1` has one bed fit finding with this protocol
    /// `suggestion` (cli spec section 3.3).
    fn checked_report(suggestion: serde_json::Value) -> Report {
        let checks: PrintReport = serde_json::from_value(json!({
            "profile": "generic-fdm",
            "orientation": {"kind": "as_modeled"},
            "summary": {"health": "error", "errors": 1, "warnings": 0, "infos": 0, "label": "1 error"},
            "bodies": [{
                "body": "b-1",
                "transform": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
                "report": {
                    "profile": "generic-fdm",
                    "rotation": {"rows": [[1, 0, 0], [0, 1, 0], [0, 0, 1]]},
                    "health": "error",
                    "checks": [{
                        "check": "bed_fit",
                        "status": "error",
                        "findings": [{
                            "check": "bed_fit",
                            "code": "footprint_too_large",
                            "severity": "error",
                            "message": FOOTPRINT_MESSAGE,
                            "location": {"kind": "bounds", "min": [-150, -15, 0], "max": [150, 15, 40]},
                            "value": 300.0,
                            "limit": 256.0,
                            "triangles": [],
                            "faces": [],
                            "suggestion": suggestion
                        }],
                        "omitted": 0
                    }]
                }
            }]
        }))
        .expect("a valid print report");
        Report {
            checks: Some(checks),
            ..report(Vec::new(), Vec::new())
        }
    }

    #[test]
    fn print_findings_print_their_suggestion_under_them() {
        let text = human(&checked_report(json!({
            "kind": "split",
            "planes": [{"origin": [0, 0, 20], "normal": [1, 0, 0]}]
        })));

        assert_eq!(
            text,
            format!(
                "box.ptsm\n  print health: 1 error (generic-fdm)\n    error[bed_fit] b-1: {FOOTPRINT_MESSAGE}\n      suggested: Split at x = 0 mm\n"
            )
        );
    }

    #[test]
    fn a_split_without_planes_prints_no_suggestion_line() {
        let text = human(&checked_report(json!({"kind": "split", "planes": []})));

        assert_eq!(
            text,
            format!(
                "box.ptsm\n  print health: 1 error (generic-fdm)\n    error[bed_fit] b-1: {FOOTPRINT_MESSAGE}\n"
            )
        );
    }

    #[test]
    fn print_findings_without_a_suggestion_print_no_suggestion_line() {
        let text = human(&checked_report(serde_json::Value::Null));

        assert!(!text.contains("suggested:"), "{text}");
    }

    #[test]
    fn measured_bodies_print_their_metrics() {
        assert_eq!(
            BodyReport::Measured(block()).human_line(),
            "  b-1 \"Block\": volume 24000 mm^3, area 5200 mm^2, bounding box (-10, -15, 0) to (10, 15, 40) mm, 6 faces, 12 edges, 8 vertices"
        );
    }

    #[test]
    fn single_elements_are_counted_in_the_singular() {
        let metrics = BodyMetrics {
            faces: 1,
            edges: 1,
            vertices: 1,
            ..block()
        };

        assert!(
            BodyReport::Measured(metrics)
                .human_line()
                .ends_with("1 face, 1 edge, 1 vertex")
        );
    }

    #[test]
    fn empty_bodies_print_no_geometry() {
        let body = BodyReport::Empty {
            id: id("b-1"),
            name: "Block".to_owned(),
            empty: true,
        };

        assert_eq!(body.human_line(), "  b-1 \"Block\": no geometry");
    }

    #[test]
    fn empty_bodies_serialize_with_id_name_and_empty() {
        let body = BodyReport::Empty {
            id: id("b-1"),
            name: "Block".to_owned(),
            empty: true,
        };

        assert_eq!(
            serde_json::to_string(&body).expect("bodies serialize"),
            r#"{"id":"b-1","name":"Block","empty":true}"#
        );
    }

    #[test]
    fn measured_bodies_serialize_in_the_spec_order() {
        let json = serde_json::to_string(&BodyReport::Measured(block())).expect("serializes");

        assert_eq!(
            json,
            r#"{"id":"b-1","name":"Block","volume":24000.0,"area":5200.0,"center_of_mass":[0.0,0.0,20.0],"bounding_box":{"min":[-10.0,-15.0,0.0],"max":[10.0,15.0,40.0]},"faces":6,"edges":12,"vertices":8}"#
        );
    }

    #[test]
    fn parameters_without_a_value_carry_the_error() {
        let parameter = ParameterReport {
            id: id("p-h"),
            name: "height".to_owned(),
            value: None,
            dimension: None,
            error: Some("Unknown name width.".to_owned()),
        };

        assert_eq!(
            serde_json::to_string(&parameter).expect("serializes"),
            r#"{"id":"p-h","name":"height","error":"Unknown name width."}"#
        );
    }

    #[test]
    fn the_report_lists_features_then_bodies() {
        let message = Diagnostic::new(Severity::Error, "feature_error", "The box is too small.");
        let text = human(&report(
            vec![feature(FeatureState::Error, vec![message])],
            vec![BodyReport::Measured(block())],
        ));

        assert_eq!(
            text,
            "box.ptsm\n  f-box1 box error\n    error[feature_error] at /features/0: The box is too small.\n  b-1 \"Block\": volume 24000 mm^3, area 5200 mm^2, bounding box (-10, -15, 0) to (10, 15, 40) mm, 6 faces, 12 edges, 8 vertices\n"
        );
    }

    #[test]
    fn a_message_is_located_by_its_feature_position_and_field() {
        let message = message_report(lost_edges(), &[], 3);

        assert_eq!(message.location, "/features/3/edges");
    }

    #[test]
    fn a_message_takes_the_candidates_of_the_repairs_under_its_field() {
        let repairs = [
            element_repair("/edges/0", "edge(f-box1/front|f-ext2/lateral/e-1)"),
            element_repair("/radius", "edge(f-box1/back|f-box1/top)"),
        ];

        let message = message_report(lost_edges(), &repairs, 3);

        let queries: Vec<String> = message
            .members
            .candidates
            .iter()
            .map(|candidate| candidate.reference.query.to_string())
            .collect();
        assert_eq!(queries, ["edge(f-box1/front|f-ext2/lateral/e-1)"]);
    }

    #[test]
    fn a_message_without_a_field_has_no_candidates() {
        let message = message_report(
            Diagnostic::new(
                Severity::Error,
                "upstream_failed",
                "Fix 'Datum plane 1' first.",
            ),
            &[element_repair("/edges/0", "edge(f-box1/back|f-box1/top)")],
            4,
        );

        assert_eq!(message.location, "/features/4");
        assert!(message.members.candidates.is_empty());
    }

    #[test]
    fn messages_print_their_actions_and_candidates_under_them() {
        let repairs = [element_repair(
            "/edges/0",
            "edge(f-box1/front|f-ext2/lateral/e-1)",
        )];
        let mut fillet = feature(FeatureState::Error, Vec::new());
        fillet.messages = vec![message_report(lost_edges(), &repairs, 1)];

        let text = human(&report(vec![fillet], Vec::new()));

        assert_eq!(
            text,
            "box.ptsm\n  f-box1 box error\n    error[references_not_found] at /features/1/edges: The front top edge of 'Box 1' selected for Edges was cut away.\n      suggested: Pick an edge again\n      candidate: edge(f-box1/front|f-ext2/lateral/e-1), on the front side of 'Box 1', 5 mm from where it was\n"
        );
    }

    #[test]
    fn messages_serialize_their_members_after_their_text() {
        let message = message_report(lost_edges(), &[], 1);

        assert_eq!(
            serde_json::to_string(&message).expect("messages serialize"),
            r#"{"severity":"error","code":"references_not_found","message":"The front top edge of 'Box 1' selected for Edges was cut away.","field":"/edges","actions":[{"label":"Pick an edge again","kind":"pick"}]}"#
        );
    }

    #[test]
    fn rolled_back_features_print_their_status() {
        let text = human(&report(
            vec![feature(FeatureState::RolledBack, Vec::new())],
            Vec::new(),
        ));

        assert_eq!(text, "box.ptsm\n  f-box1 box rolled_back\n");
    }

    #[test]
    fn failed_features_fail_the_rebuild() {
        let failed = report(vec![feature(FeatureState::Error, Vec::new())], Vec::new());

        assert_eq!(failed.status(false), Status::Problems);
    }

    #[test]
    fn warnings_fail_the_rebuild_only_when_denied() {
        let warned = report(vec![feature(FeatureState::Warning, Vec::new())], Vec::new());

        assert_eq!(warned.status(false), Status::Success);
        assert_eq!(warned.status(true), Status::Problems);
    }

    #[test]
    fn suppressed_features_do_not_fail_the_rebuild() {
        let suppressed = report(
            vec![feature(FeatureState::Suppressed, Vec::new())],
            Vec::new(),
        );

        assert_eq!(suppressed.status(true), Status::Success);
    }

    #[test]
    fn incomplete_measurements_are_internal_errors() {
        let body = Body::new(id("b-1"), "Block");

        let error = body_metrics(&body, Measurement::default()).expect_err("fields are missing");

        assert_eq!(error.status(), Status::Internal);
    }

    fn document() -> Document {
        partsmith_model::load(
            r#"{
              "format": "partsmith",
              "format_version": 1,
              "units": "mm",
              "parameters": [{"id": "p-h", "name": "height", "expr": "40 mm"}],
              "bodies": [{"id": "b-1", "name": "Block"}],
              "features": [{"id": "f-box1", "type": "box", "body": "b-1", "op": "new", "size": ["20 mm", "30 mm", "height"]}]
            }"#,
        )
        .expect("the document loads")
        .document
    }

    #[test]
    fn feature_reports_carry_the_feature_type() {
        let status = FeatureStatus {
            id: id("f-box1"),
            status: FeatureState::Ok,
            messages: Vec::new(),
            repairs: Vec::new(),
        };

        let reports = feature_reports(&document(), vec![status]).expect("the feature exists");

        assert_eq!(
            serde_json::to_string(&reports).expect("serializes"),
            r#"[{"id":"f-box1","type":"box","status":"ok","messages":[]}]"#
        );
    }

    #[test]
    fn parameter_reports_carry_the_parameter_name() {
        let value = ParameterValue {
            id: id("p-h"),
            value: Some(40.0),
            dimension: Some(Dimension {
                length: 1,
                angle: 0,
            }),
            error: None,
        };

        let reports = parameter_reports(&document(), vec![value]).expect("the parameter exists");

        assert_eq!(
            serde_json::to_string(&reports).expect("serializes"),
            r#"[{"id":"p-h","name":"height","value":40.0,"dimension":{"length":1,"angle":0}}]"#
        );
    }

    #[test]
    fn a_status_of_an_unknown_feature_is_an_internal_error() {
        let status = FeatureStatus {
            id: id("f-ghost"),
            status: FeatureState::Ok,
            messages: Vec::new(),
            repairs: Vec::new(),
        };

        let error = feature_reports(&document(), vec![status]).expect_err("no such feature");

        assert_eq!(error.status(), Status::Internal);
        assert_eq!(
            error.report_line(),
            "internal error: the engine reported the status of feature f-ghost, which the document does not have"
        );
    }

    #[test]
    fn a_value_of_an_unknown_parameter_is_an_internal_error() {
        let value = ParameterValue {
            id: id("p-ghost"),
            value: Some(1.0),
            dimension: None,
            error: None,
        };

        let error = parameter_reports(&document(), vec![value]).expect_err("no such parameter");

        assert_eq!(error.status(), Status::Internal);
        assert!(error.to_string().contains("p-ghost"), "{error}");
    }

    #[test]
    fn points_print_like_numbers() {
        assert_eq!(human_point([-0.0, 1.5, 1e-9]), "(0, 1.5, 0)");
    }
}
