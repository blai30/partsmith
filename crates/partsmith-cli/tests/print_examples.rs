//! Acceptance tests for the print check examples (print spec section 6.18, tests 117 to 123 and 125
//! to 127).
//!
//! Each test rebuilds its example with `partsmith rebuild --checks --json`, as the examples runner
//! (`examples.rs`) does, and checks every value the spec test names, so a bless that changes one
//! of them fails here even though the runner then accepts the new expected file.

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, FRAC_PI_6, PI};

use crate::common;

use common::{assert_json_close, entry, items, number, repository_root, run_in, text};
use pretty_assertions::assert_eq;
use serde_json::{Map, Value, json};

/// The comparison tolerance of the examples corpus (delivery spec section 2.8): relative, with
/// the same absolute floor.
const TOLERANCE: f64 = 1e-6;

/// The report of `partsmith rebuild --checks --json examples/<example>.ptsm`, run from the
/// repository root.
fn rebuild(example: &str) -> Value {
    let model = format!("examples/{example}.ptsm");
    let run = run_in(
        &repository_root(),
        &["rebuild", "--checks", "--json", &model],
        None,
    );
    run.assert_code(0);
    run.json()
}

/// Panics unless the report's printer and the checks' profile are both `printer`.
fn assert_printer(report: &Value, printer: &str) {
    assert_eq!(report["printer"], printer, "the report's printer");
    assert_eq!(report["checks"]["profile"], printer, "the checks' profile");
}

/// Panics unless the record of `array` with the `id` of `expected` has every member of
/// `expected`, numbers within the corpus tolerance.
fn assert_record(array: &Value, expected: Value) {
    let id = text(&expected["id"]).to_owned();
    let record = entry(array, "id", &id);
    let named: Map<String, Value> = expected
        .as_object()
        .unwrap_or_else(|| panic!("an expected record is an object, got {expected}"))
        .keys()
        .map(|key| (key.clone(), record[key].clone()))
        .collect();
    assert_json_close(&json!({ &id: named }), &json!({ &id: expected }), TOLERANCE);
}

/// Panics unless the checks' summary is `expected` (print spec section 2.7).
fn assert_summary(report: &Value, expected: Value) {
    assert_eq!(report["checks"]["summary"], expected, "the checks' summary");
}

/// The status of a check with `findings`: the worst of their severities, `ok` when there is none
/// (print spec section 2.6).
fn status(findings: &[Value]) -> &'static str {
    ["error", "warning", "info"]
        .into_iter()
        .find(|severity| {
            findings
                .iter()
                .any(|finding| finding["severity"] == *severity)
        })
        .unwrap_or("ok")
}

/// A check result without each finding's `triangles` and `faces`, which the tests leave to the
/// expected file.
fn without_mesh_members(check: &Value) -> Value {
    let mut check = check.clone();
    for finding in check["findings"].as_array_mut().into_iter().flatten() {
        if let Some(members) = finding.as_object_mut() {
            members.shift_remove("triangles");
            members.shift_remove("faces");
        }
    }
    check
}

/// Panics unless the checks analyzed exactly the bodies of `expected`, in order, and each body's
/// check results hold exactly its findings (an array of findings without `triangles` and `faces`):
/// a check with findings has the worst of their severities as its status, every other check is
/// `ok` with none, and no check omits any. Mismatches are reported at `/<body> <check>/...`.
fn assert_checks(report: &Value, expected: &[(&str, Value)]) {
    let bodies = items(&report["checks"]["bodies"]);
    let analyzed: Vec<&str> = bodies.iter().map(|body| text(&body["body"])).collect();
    let expected_bodies: Vec<&str> = expected.iter().map(|(id, _)| *id).collect();
    assert_eq!(analyzed, expected_bodies, "the bodies the checks analyzed");
    let mut actual = Map::new();
    let mut wanted = Map::new();
    for (body, (id, findings)) in bodies.iter().zip(expected) {
        let findings = items(findings);
        for check in items(&body["report"]["checks"]) {
            let name = text(&check["check"]);
            let check_findings: Vec<Value> = findings
                .iter()
                .filter(|finding| finding["check"] == name)
                .cloned()
                .collect();
            let key = format!("{id} {name}");
            wanted.insert(
                key.clone(),
                json!({"check": name, "status": status(&check_findings), "findings": check_findings, "omitted": 0}),
            );
            actual.insert(key, without_mesh_members(check));
        }
        for finding in findings {
            let key = format!("{id} {}", text(&finding["check"]));
            assert!(actual.contains_key(&key), "{id} has the check of {finding}");
        }
    }
    assert_json_close(&Value::Object(actual), &Value::Object(wanted), TOLERANCE);
}

/// The corner `(±38, ±23, 2)` of the closed hollow's floor on the side of the drain hole point
/// that b-1's trapped volume finding suggests (test 122: which corner follows the mesh's vertex
/// order). A point that is not at a corner then differs from it.
fn floor_corner_of_drain_hole(report: &Value) -> [f64; 3] {
    let checks = &entry(&report["checks"]["bodies"], "body", "b-1")["report"]["checks"];
    let at = &entry(checks, "check", "trapped_volumes")["findings"][0]["suggestion"]["at"];
    [
        38.0_f64.copysign(number(&at[0], "the drain hole's x")),
        23.0_f64.copysign(number(&at[1], "the drain hole's y")),
        2.0,
    ]
}

#[test]
fn t117_too_tall() {
    let report = rebuild("box/too-tall");

    assert_printer(&report, "generic-fdm");
    assert_record(
        &report["bodies"],
        json!({"id": "b-1", "volume": 480000.0, "bounding_box": {"min": [-20.0, -20.0, 0.0], "max": [20.0, 20.0, 300.0]}}),
    );
    assert_summary(
        &report,
        json!({"health": "error", "errors": 1, "warnings": 0, "infos": 0, "label": "1 error"}),
    );
    assert_checks(
        &report,
        &[(
            "b-1",
            json!([{
                "check": "bed_fit",
                "code": "too_tall",
                "severity": "error",
                "message": "The part is 300 mm tall, taller than the 256 mm build height. Split the part or lay it on its side.",
                "location": {"kind": "bounds", "min": [-20.0, -20.0, 0.0], "max": [20.0, 20.0, 300.0]},
                "value": 300.0,
                "limit": 256.0,
                "suggestion": {"kind": "split", "planes": [{"origin": [0.0, 0.0, 150.0], "normal": [0.0, 0.0, 1.0]}]}
            }]),
        )],
    );
}

#[test]
fn t118_wide_footprint() {
    let report = rebuild("box/wide-footprint");

    assert_printer(&report, "generic-fdm");
    assert_record(
        &report["bodies"],
        json!({"id": "b-1", "volume": 1200000.0, "bounding_box": {"min": [-200.0, -150.0, 0.0], "max": [200.0, 150.0, 10.0]}}),
    );
    assert_summary(
        &report,
        json!({"health": "error", "errors": 1, "warnings": 0, "infos": 0, "label": "1 error"}),
    );
    assert_checks(
        &report,
        &[(
            "b-1",
            json!([{
                "check": "bed_fit",
                "code": "footprint_too_large",
                "severity": "error",
                "message": "The part footprint is 400 x 300 mm, larger than the 256 x 256 mm bed. Split the part or scale it down.",
                "location": {"kind": "bounds", "min": [-200.0, -150.0, 0.0], "max": [200.0, 150.0, 10.0]},
                "value": 400.0,
                "limit": 256.0,
                "suggestion": {"kind": "split", "planes": [
                    {"origin": [0.0, 0.0, 5.0], "normal": [1.0, 0.0, 0.0]},
                    {"origin": [0.0, 0.0, 5.0], "normal": [0.0, 1.0, 0.0]}
                ]}
            }]),
        )],
    );
}

#[test]
fn t119_tall_and_thin() {
    let report = rebuild("box/tall-and-thin");

    assert_printer(&report, "generic-fdm");
    assert_record(
        &report["bodies"],
        json!({"id": "b-1", "volume": 2500.0, "bounding_box": {"min": [-2.5, -2.5, 0.0], "max": [2.5, 2.5, 100.0]}}),
    );
    assert_summary(
        &report,
        json!({"health": "warning", "errors": 0, "warnings": 1, "infos": 0, "label": "1 warning"}),
    );
    assert_checks(
        &report,
        &[(
            "b-1",
            json!([{
                "check": "first_layer_contact",
                "code": "tall_and_thin",
                "severity": "warning",
                "message": "The part is 100 mm tall but stands on only 25 mm² of the bed, so it may wobble or tip over. Add a brim or lay the part down.",
                "location": {"kind": "bounds", "min": [-2.5, -2.5, 0.0], "max": [2.5, 2.5, 100.0]},
                "value": (100.0 / (2.0 * (25.0 / PI).sqrt())),
                "limit": 10.0
            }]),
        )],
    );
}

#[test]
fn t120_island() {
    let report = rebuild("boolean/island");

    assert_printer(&report, "generic-fdm");
    assert_record(
        &report["bodies"],
        json!({"id": "b-1", "volume": 1760.0, "bounding_box": {"min": [0.0, 0.0, 0.0], "max": [20.0, 10.0, 20.0]}, "faces": 10}),
    );
    assert_record(&report["bodies"], json!({"id": "b-2", "empty": true}));
    assert_record(&report["bodies"], json!({"id": "b-3", "empty": true}));
    assert_summary(
        &report,
        json!({"health": "warning", "errors": 0, "warnings": 3, "infos": 0, "label": "3 warnings"}),
    );
    assert_checks(
        &report,
        &[(
            "b-1",
            json!([
                {
                    "check": "islands",
                    "code": "island",
                    "severity": "warning",
                    "message": "Part of the model starts in mid-air 8 mm above the bed. Add supports or reorient the part.",
                    "location": {"kind": "bounds", "min": [16.0, 0.0, 8.0], "max": [20.0, 10.0, 8.0]},
                    "value": 8.0
                },
                {
                    "check": "overhangs",
                    "code": "overhang",
                    "severity": "warning",
                    "message": "Overhang steeper than 45 degrees over 120 mm². Add supports, add a chamfer, or reorient the part.",
                    "location": {"kind": "bounds", "min": [4.0, 0.0, 16.0], "max": [16.0, 10.0, 16.0]},
                    "value": FRAC_PI_2,
                    "limit": FRAC_PI_4
                },
                {
                    "check": "overhangs",
                    "code": "overhang",
                    "severity": "warning",
                    "message": "Overhang steeper than 45 degrees over 40 mm². Add supports, add a chamfer, or reorient the part.",
                    "location": {"kind": "bounds", "min": [16.0, 0.0, 8.0], "max": [20.0, 10.0, 8.0]},
                    "value": FRAC_PI_2,
                    "limit": FRAC_PI_4
                }
            ]),
        )],
    );
}

#[test]
fn t121_fits_rotated() {
    let report = rebuild("box/fits-rotated");

    assert_printer(&report, "generic-resin");
    assert_record(
        &report["bodies"],
        json!({"id": "b-1", "volume": 70000.0, "bounding_box": {"min": [-35.0, -50.0, 0.0], "max": [35.0, 50.0, 10.0]}}),
    );
    assert_summary(
        &report,
        json!({"health": "ok", "errors": 0, "warnings": 0, "infos": 1, "label": "Ready to print"}),
    );
    assert_checks(
        &report,
        &[(
            "b-1",
            json!([{
                "check": "bed_fit",
                "code": "fits_rotated",
                "severity": "info",
                "message": "The part fits the bed only when turned 90 degrees about Z. Turn it in the slicer.",
                "location": {"kind": "bounds", "min": [-35.0, -50.0, 0.0], "max": [35.0, 50.0, 10.0]}
            }]),
        )],
    );
}

#[test]
fn t122_closed_hollow_on_resin() {
    let report = rebuild("shell/closed-hollow-resin");

    assert_printer(&report, "generic-resin");
    assert_record(
        &report["bodies"],
        json!({"id": "b-1", "volume": 29104.0, "faces": 12}),
    );
    assert_summary(
        &report,
        json!({"health": "warning", "errors": 0, "warnings": 2, "infos": 0, "label": "2 warnings"}),
    );
    let corner = floor_corner_of_drain_hole(&report);
    assert_checks(
        &report,
        &[(
            "b-1",
            json!([
                {
                    "check": "overhangs",
                    "code": "overhang",
                    "severity": "warning",
                    "message": "Overhang steeper than 30 degrees over 3496 mm². Add supports, add a chamfer, or reorient the part.",
                    "location": {"kind": "bounds", "min": [-38.0, -23.0, 28.0], "max": [38.0, 23.0, 28.0]},
                    "value": FRAC_PI_2,
                    "limit": FRAC_PI_6
                },
                {
                    "check": "trapped_volumes",
                    "code": "enclosed_hollow",
                    "severity": "warning",
                    "message": "An enclosed hollow of 90896 mm³ will trap liquid resin. Add a drain hole near its lowest point.",
                    "location": {"kind": "bounds", "min": [-38.0, -23.0, 2.0], "max": [38.0, 23.0, 28.0]},
                    "value": 90896.0,
                    "suggestion": {"kind": "drain_hole", "at": corner}
                }
            ]),
        )],
    );
}

#[test]
fn t123_resin_thresholds_overhang_limit_and_printer_values() {
    let report = rebuild("box/resin");

    assert_printer(&report, "generic-resin");
    assert_record(&report["parameters"], json!({"id": "p-wall", "value": 0.3}));
    assert_record(
        &report["bodies"],
        json!({"id": "b-1", "volume": 840.0, "bounding_box": {"min": [-70.0, -0.15, 0.0], "max": [70.0, 0.15, 20.0]}}),
    );
    assert_record(
        &report["bodies"],
        json!({"id": "b-2", "volume": 3750.0, "bounding_box": {"min": [-10.0, 20.0, 0.0], "max": [10.0, 40.0, 10.0]}, "faces": 7}),
    );
    assert_summary(
        &report,
        json!({"health": "error", "errors": 1, "warnings": 3, "infos": 0, "label": "1 error, 3 warnings"}),
    );
    let thin_wall = |y: f64| {
        json!({
            "check": "thin_walls",
            "code": "thin_wall",
            "severity": "warning",
            "message": "A wall 0.3 mm thick is thinner than 0.5 mm and may not print. Make it at least 0.5 mm thick.",
            "location": {"kind": "bounds", "min": [-70.0, y, 0.0], "max": [70.0, y, 20.0]},
            "value": 0.3,
            "limit": 0.5
        })
    };
    assert_checks(
        &report,
        &[
            (
                "b-1",
                json!([
                    {
                        "check": "bed_fit",
                        "code": "footprint_too_large",
                        "severity": "error",
                        "message": "The part footprint is 140 x 0.3 mm, larger than the 130 x 80 mm bed. Split the part or scale it down.",
                        "location": {"kind": "bounds", "min": [-70.0, -0.15, 0.0], "max": [70.0, 0.15, 20.0]},
                        "value": 140.0,
                        "limit": 130.0,
                        "suggestion": {"kind": "split", "planes": [{"origin": [0.0, 0.0, 10.0], "normal": [1.0, 0.0, 0.0]}]}
                    },
                    thin_wall(-0.15),
                    thin_wall(0.15)
                ]),
            ),
            (
                "b-2",
                json!([{
                    "check": "overhangs",
                    "code": "overhang",
                    "severity": "warning",
                    "message": "Overhang steeper than 30 degrees over 141.4 mm². Add supports, add a chamfer, or reorient the part.",
                    "location": {"kind": "bounds", "min": [-10.0, 20.0, 0.0], "max": [10.0, 25.0, 5.0]},
                    "value": FRAC_PI_4,
                    "limit": FRAC_PI_6
                }]),
            ),
        ],
    );
}

#[test]
fn t125_island_on_resin() {
    let report = rebuild("boolean/island-resin");

    assert_printer(&report, "generic-resin");
    assert_record(
        &report["bodies"],
        json!({"id": "b-1", "volume": 1760.0, "bounding_box": {"min": [0.0, 0.0, 0.0], "max": [20.0, 10.0, 20.0]}, "faces": 10}),
    );
    assert_record(&report["bodies"], json!({"id": "b-2", "empty": true}));
    assert_record(&report["bodies"], json!({"id": "b-3", "empty": true}));
    assert_summary(
        &report,
        json!({"health": "warning", "errors": 0, "warnings": 3, "infos": 0, "label": "3 warnings"}),
    );
    assert_checks(
        &report,
        &[(
            "b-1",
            json!([
                {
                    "check": "islands",
                    "code": "island",
                    "severity": "warning",
                    "message": "Part of the model starts in mid-air 8 mm above the bed. Add supports or reorient the part.",
                    "location": {"kind": "bounds", "min": [16.0, 0.0, 8.0], "max": [20.0, 10.0, 8.0]},
                    "value": 8.0
                },
                {
                    "check": "overhangs",
                    "code": "overhang",
                    "severity": "warning",
                    "message": "Overhang steeper than 30 degrees over 120 mm². Add supports, add a chamfer, or reorient the part.",
                    "location": {"kind": "bounds", "min": [4.0, 0.0, 16.0], "max": [16.0, 10.0, 16.0]},
                    "value": FRAC_PI_2,
                    "limit": FRAC_PI_6
                },
                {
                    "check": "overhangs",
                    "code": "overhang",
                    "severity": "warning",
                    "message": "Overhang steeper than 30 degrees over 40 mm². Add supports, add a chamfer, or reorient the part.",
                    "location": {"kind": "bounds", "min": [16.0, 0.0, 8.0], "max": [20.0, 10.0, 8.0]},
                    "value": FRAC_PI_2,
                    "limit": FRAC_PI_6
                }
            ]),
        )],
    );
}

#[test]
fn t126_wedge_on_its_edge_on_resin() {
    let report = rebuild("extrude/wedge-on-edge-resin");

    assert_printer(&report, "generic-resin");
    assert_record(
        &report["bodies"],
        json!({"id": "b-1", "volume": 1000.0, "bounding_box": {"min": [-5.0, -10.0, 0.0], "max": [5.0, 0.0, 20.0]}, "faces": 5}),
    );
    assert_summary(
        &report,
        json!({"health": "warning", "errors": 0, "warnings": 2, "infos": 0, "label": "2 warnings"}),
    );
    assert_checks(
        &report,
        &[(
            "b-1",
            json!([
                {
                    "check": "first_layer_contact",
                    "code": "no_bed_contact",
                    "severity": "warning",
                    "message": "The part touches the bed only at points or edges, so it may come loose. Lay a flat face on the bed or add supports.",
                    "location": {"kind": "point", "at": [0.0, 0.0, 0.0]},
                    "value": 0.0
                },
                {
                    "check": "small_features",
                    "code": "small_detail",
                    "severity": "warning",
                    "message": "A detail 0.013 mm wide is narrower than 0.2 mm and may not print. Make it at least 0.2 mm wide.",
                    "location": {"kind": "bounds", "min": [-5.0, -10.0, 0.0], "max": [5.0, 0.0, 20.0]},
                    "value": 0.0125,
                    "limit": 0.2
                }
            ]),
        )],
    );
}

#[test]
fn t127_corners_touching_on_resin() {
    let report = rebuild("extrude/corners-touching-resin");

    assert_printer(&report, "generic-resin");
    assert_record(
        &report["features"],
        json!({"id": "f-ext1", "status": "ok", "messages": []}),
    );
    assert_record(
        &report["bodies"],
        json!({"id": "b-1", "volume": 2000.0, "bounding_box": {"min": [-10.0, -10.0, 0.0], "max": [10.0, 10.0, 10.0]}, "faces": 12}),
    );
    assert_summary(
        &report,
        json!({"health": "error", "errors": 1, "warnings": 0, "infos": 0, "label": "1 error"}),
    );
    assert_checks(
        &report,
        &[(
            "b-1",
            json!([{
                "check": "mesh_validity",
                "code": "non_manifold_edges",
                "severity": "error",
                "message": "1 edge is shared by more than two triangles. Separate the touching parts or rebuild the body.",
                "location": {"kind": "bounds", "min": [0.0, 0.0, 0.0], "max": [0.0, 0.0, 10.0]},
                "value": 1.0
            }]),
        )],
    );
}
