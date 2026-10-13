//! `partsmith rebuild` (cli.md section 3.3).

use std::collections::BTreeMap;
use std::io;

use partsmith_engine::{EngineError, PrintRequest, Session};
use partsmith_model::{Body, Dimension, Document, Id};
use partsmith_protocol::{
    BoundingBox, Diagnostic, DocHandle, FeatureState, FeatureStatus, MeasureTarget, Measurement,
    Orientation, ParameterValue, PrintReport, Vec3,
};
use serde::Serialize;

use super::{open, usage_error};
use crate::arguments::RebuildArguments;
use crate::error::CliError;
use crate::output::{JsonDiagnostic, Output, count, human_number, wire_name};
use crate::rotation;
use crate::status::Status;

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
    /// Its error first, then its warnings.
    messages: Vec<Diagnostic>,
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

/// Pairs each feature status with the feature's type. The engine reports the status of every
/// feature of the document (engine.md section 2), so a status without a record is an internal
/// error.
fn feature_reports(
    document: &Document,
    statuses: Vec<FeatureStatus>,
) -> Result<Vec<FeatureReport>, CliError> {
    let kinds: BTreeMap<&Id, &str> = document
        .features
        .iter()
        .map(|record| (&record.id, record.kind.as_str()))
        .collect();
    statuses
        .into_iter()
        .map(|status| {
            let kind = recorded(&kinds, &status.id, "the status of feature")?;
            Ok(FeatureReport {
                kind: kind.to_owned(),
                id: status.id,
                status: status.status,
                messages: status.messages,
            })
        })
        .collect()
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

/// The text the document records for `id` in `names`. The engine reports only records of the
/// document, so a missing one is an internal error naming what was `reported`, for example "the
/// status of feature".
fn recorded<'a>(
    names: &BTreeMap<&Id, &'a str>,
    id: &Id,
    reported: &str,
) -> Result<&'a str, CliError> {
    names.get(id).copied().ok_or_else(|| CliError::Internal {
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
    /// and with checks the print health and one line per finding.
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
                    "    {}[{}]: {}",
                    wire_name(&message.severity),
                    message.code,
                    message.message
                ))?;
            }
        }
        for body in &self.bodies {
            output.line(body.human_line())?;
        }
        if let Some(checks) = &self.checks {
            output.line(format_args!(
                "  print health: {} ({})",
                checks.summary.label, checks.profile
            ))?;
            for body in &checks.bodies {
                for check in &body.report.checks {
                    for finding in &check.findings {
                        output.line(format_args!(
                            "    {}[{}] {}: {}",
                            wire_name(&finding.severity),
                            wire_name(&finding.check),
                            body.body,
                            finding.message
                        ))?;
                    }
                }
            }
        }
        Ok(())
    }
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
    use partsmith_protocol::Severity;

    use super::*;
    use crate::commands::test_support::id;
    use crate::output::capture;

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
            messages,
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
        let message = Diagnostic {
            severity: Severity::Error,
            code: "feature_error".to_owned(),
            message: "The box is too small.".to_owned(),
            field: None,
            suggestion: None,
        };
        let text = human(&report(
            vec![feature(FeatureState::Error, vec![message])],
            vec![BodyReport::Measured(block())],
        ));

        assert_eq!(
            text,
            "box.ptsm\n  f-box1 box error\n    error[feature_error]: The box is too small.\n  b-1 \"Block\": volume 24000 mm^3, area 5200 mm^2, bounding box (-10, -15, 0) to (10, 15, 40) mm, 6 faces, 12 edges, 8 vertices\n"
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
