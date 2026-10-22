//! Acceptance tests for `partsmith rebuild` (cli spec sections 3.3 and 6, tests 26 to 33).

use crate::common;

use common::{
    BOX, FILLET, Workspace, assert_json_close, assert_pretty_json_layout, assert_relative,
    box_with_negative_length, entry, items, number,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The human example of cli spec section 3.3.
const BOX_REPORT: &str = "box.ptsm
  f-box1 box ok
  b-1 \"Block\": volume 24000 mm^3, area 5200 mm^2, bounding box (-10, -15, 0) to (10, 15, 40) mm, 6 faces, 12 edges, 8 vertices
";

/// The JSON example of cli spec section 3.3.
const BOX_JSON_REPORT: &str = r#"{
  "path": "box.ptsm",
  "printer": "generic-fdm",
  "features": [
    {
      "id": "f-box1",
      "type": "box",
      "status": "ok",
      "messages": []
    }
  ],
  "parameters": [
    {
      "id": "p-h",
      "name": "height",
      "value": 40.0,
      "dimension": {
        "length": 1,
        "angle": 0
      }
    }
  ],
  "bodies": [
    {
      "id": "b-1",
      "name": "Block",
      "volume": 24000.0,
      "area": 5200.0,
      "center_of_mass": [
        0.0,
        0.0,
        20.0
      ],
      "bounding_box": {
        "min": [
          -10.0,
          -15.0,
          0.0
        ],
        "max": [
          10.0,
          15.0,
          40.0
        ]
      },
      "faces": 6,
      "edges": 12,
      "vertices": 8
    }
  ]
}
"#;

#[test]
fn t026_rebuild_prints_feature_status_and_body_metrics() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);

    let run = workspace.run(&["rebuild", BOX]);

    run.assert_code(0);
    assert_eq!(run.stdout, BOX_REPORT);
}

#[test]
fn t027_rebuild_json_matches_the_spec_example() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);

    let run = workspace.run(&["rebuild", "--json", BOX]);

    run.assert_code(0);
    assert_pretty_json_layout(&run);
    let expected: Value = serde_json::from_str(BOX_JSON_REPORT).expect("the example is JSON");
    assert_json_close(&run.json(), &expected, 1e-9);
}

#[test]
fn t028_rebuild_json_measures_the_filleted_box() {
    let workspace = Workspace::new();
    workspace.copy_fixture(FILLET);

    let run = workspace.run(&["rebuild", "--json", FILLET]);

    run.assert_code(0);
    let report = run.json();
    let body = entry(&report["bodies"], "id", "b-1");
    assert_relative(
        number(&body["volume"], "volume"),
        23965.66370614359,
        1e-6,
        "volume",
    );
    assert_relative(
        number(&body["area"], "area"),
        5163.946891450772,
        1e-6,
        "area",
    );
    assert_eq!(body["faces"], 7);
    assert_eq!(body["edges"], 15);
    assert_eq!(body["vertices"], 10);
}

#[test]
fn t029_rebuild_reports_failed_features_and_empty_bodies() {
    let workspace = Workspace::new();
    workspace.write(BOX, box_with_negative_length());

    let run = workspace.run(&["rebuild", "--json", BOX]);

    run.assert_code(1);
    let report = run.json();
    assert_eq!(
        entry(&report["features"], "id", "f-box1")["status"],
        "error"
    );
    let body = entry(&report["bodies"], "id", "b-1");
    assert_eq!(
        serde_json::to_string(body).expect("JSON values serialize"),
        r#"{"id":"b-1","name":"Block","empty":true}"#
    );
}

#[test]
fn t030_rebuild_rejects_an_unknown_printer() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);

    let run = workspace.run(&["rebuild", "--printer", "nope", BOX]);

    run.assert_code(1);
    run.assert_stderr_line("error: Printer profile \"nope\" is not installed.");
}

#[test]
fn t031_rebuild_checks_report_ready_to_print_as_modeled() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);

    let run = workspace.run(&["rebuild", "--checks", "--json", BOX]);

    run.assert_code(0);
    let checks = &run.json()["checks"];
    assert_eq!(checks["summary"]["health"], "ok");
    assert_eq!(checks["summary"]["label"], "Ready to print");
    assert_eq!(checks["orientation"], json!({"kind": "as_modeled"}));
}

#[test]
fn t032_rotated_box_has_overhang_findings() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);

    let run = workspace.run(&["rebuild", "--checks", "--rotate", "x:30", "--json", BOX]);

    run.assert_code(0);
    let report = run.json();
    let body = entry(&report["checks"]["bodies"], "body", "b-1");
    let overhangs = entry(&body["report"]["checks"], "check", "overhangs");
    let findings = items(&overhangs["findings"]);
    assert!(!findings.is_empty(), "{overhangs}");
}

#[test]
fn t033_rotate_without_degrees_is_a_usage_error() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);

    let run = workspace.run(&["rebuild", "--rotate", "x", BOX]);

    run.assert_code(2);
    run.assert_stderr_line("error: --rotate takes <axis>:<degrees>, for example x:90.");
}
