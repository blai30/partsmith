//! Python export: acceptance test 96 of `docs/spec/engine.md` section 6, plus the code generation tests of `docs/spec/python-api.md` section 6 whose checks are about the generated text (python-api.md says they also run from Rust here). Tests named `python_api_tNNN_` carry python-api.md's test number.

mod common;

use common::{doc_handle, fixture, open_doc, open_text, python_script, replace_once, session};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The fingerprint text of the box fixture's top face (naming.md section 3.7.3).
const TOP_FINGERPRINT: &str = "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4";

/// A `.ptsm` text with these parts; written compactly, which `open_document` accepts.
fn document_text(units: &str, parameters: Value, bodies: Value, features: Value) -> String {
    json!({
        "format": "partsmith",
        "format_version": 1,
        "units": units,
        "parameters": parameters,
        "bodies": bodies,
        "features": features
    })
    .to_string()
}

/// The script of a document text.
fn script_of(text: &str) -> String {
    let mut session = session();
    let doc = doc_handle(&open_text(&mut session, text));
    python_script(&mut session, doc)
}

/// For each `id="..."` of a feature (`f-` ids), the index of the top-level `with` block it is in.
fn feature_blocks(script: &str) -> Vec<(usize, String)> {
    let mut block = None;
    let mut placed = Vec::new();
    for line in script.lines() {
        if line.starts_with("with ") {
            block = Some(block.map_or(0, |index: usize| index + 1));
        }
        let Some(start) = line.find("id=\"f-") else {
            continue;
        };
        let rest = &line[start + 4..];
        let id = &rest[..rest.find('"').expect("ids are quoted")];
        placed.push((block.expect("features sit inside a block"), id.to_owned()));
    }
    placed
}

#[test]
fn t096_export_python_of_the_box_is_exact() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let script = python_script(&mut session, doc);

    assert_eq!(
        script,
        r#"from partsmith import *

doc = Document(units="mm")

height = param("height", 40*mm, id="p-h")

block = Part("Block", id="b-1")

with block:
    box(20*mm, 30*mm, height, id="f-box1")
"#
    );
}

#[test]
fn python_api_t002_export_python_of_the_plate_is_exact() {
    let mut session = session();
    let doc = open_doc(&mut session, "plate.ptsm");

    let script = python_script(&mut session, doc);

    assert_eq!(
        script,
        r#"from partsmith import *

doc = Document(units="mm")

plate = Part("Plate", id="b-1")

with plate:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        line_1 = Line((0, 0), (80, 0), id="e-1")
        line_2 = Line((80, 0), (80, 50), id="e-2")
        line_3 = Line((80, 50), (0, 50), id="e-3")
        line_4 = Line((0, 50), (0, 0), id="e-4")
        Coincident(line_1.end, line_2.start, id="c-1")
        Coincident(line_2.end, line_3.start, id="c-2")
        Coincident(line_3.end, line_4.start, id="c-3")
        Coincident(line_4.end, line_1.start, id="c-4")
        Horizontal(line_1, id="c-5")
        Horizontal(line_3, id="c-6")
        Vertical(line_2, id="c-7")
        Vertical(line_4, id="c-8")
    extrude(sketch_1, 10*mm, id="f-ext1")
"#
    );
}

#[test]
fn python_api_t004_a_long_fillet_call_is_split_one_argument_per_line() {
    let mut session = session();
    let doc = open_doc(&mut session, "fillet.ptsm");

    let script = python_script(&mut session, doc);

    let ending = r#"with block:
    box(20*mm, 30*mm, height, id="f-box1")
    fillet(
        Ref("edge(f-box1/front|f-box1/left)", "line|n=0,0,1|c=-10,-15,20|l=40|adj=2,2"),
        2*mm,
        id="f-fil1",
    )
"#;
    assert!(
        script.ends_with(ending),
        "the script ends with the split call:\n{script}"
    );
}

#[test]
fn python_api_t005_parameter_expressions_are_emitted_as_python_values() {
    let cases = [
        ("40", "40"),
        ("2 mm", "2*mm"),
        ("1.5 in", "1.5*inch"),
        ("10 m", "10*meter"),
        ("30 deg", "30*deg"),
        ("printer.fit.slip", "printer.fit.slip"),
        ("height", "height"),
        ("2*height + 1", r#""2*height + 1""#),
        ("0.30", r#""0.30""#),
        ("-0", r#""-0""#),
        ("0.00001", r#""0.00001""#),
        ("2mm", r#""2mm""#),
        ("0.1", "0.1"),
    ];

    for (source, emitted) in cases {
        let text = document_text(
            "mm",
            json!([
                {"id": "p-h", "name": "height", "expr": "40 mm"},
                {"id": "p-x", "name": "x", "expr": source}
            ]),
            json!([]),
            json!([]),
        );

        let script = script_of(&text);

        let expected = format!(r#"x = param("x", {emitted}, id="p-x")"#);
        assert!(
            script.lines().any(|line| line == expected),
            "source {source:?} gives `{expected}`:\n{script}"
        );
    }
}

#[test]
fn python_api_t007_part_names_are_escaped_string_literals() {
    let text = document_text(
        "mm",
        json!([]),
        json!([{"id": "b-1", "name": "He said \"hi\"\\"}]),
        json!([]),
    );

    let script = script_of(&text);

    assert!(
        script
            .lines()
            .any(|line| line.ends_with(r#"Part("He said \"hi\"\\", id="b-1")"#)),
        "the name is escaped:\n{script}"
    );
}

#[test]
fn python_api_t008_part_variables_are_snake_case_and_unique() {
    let text = document_text(
        "mm",
        json!([]),
        json!([
            {"id": "b-1", "name": "Lid (top)"},
            {"id": "b-2", "name": "class"},
            {"id": "b-3", "name": "Block"},
            {"id": "b-4", "name": "Block"},
            {"id": "b-5", "name": "3D"},
            {"id": "b-6", "name": "box"}
        ]),
        json!([]),
    );

    let script = script_of(&text);

    let parts = r#"lid_top = Part("Lid (top)", id="b-1")
class_2 = Part("class", id="b-2")
block = Part("Block", id="b-3")
block_2 = Part("Block", id="b-4")
part_3d = Part("3D", id="b-5")
box_2 = Part("box", id="b-6")
"#;
    assert!(
        script.contains(parts),
        "the parts section is:\n{parts}\ngot:\n{script}"
    );
}

#[test]
fn python_api_t010_unknown_feature_types_use_the_generic_form() {
    let text = document_text(
        "mm",
        json!([]),
        json!([{"id": "b-1", "name": "Block"}]),
        json!([{"id": "f-1", "type": "wedge_v9", "body": "b-1", "size": ["20", "30"], "from": 1}]),
    );

    let script = script_of(&text);

    let block = "with block:\n    feature(\"wedge_v9\", id=\"f-1\", body=\"b-1\", size=[\"20\", \"30\"], **{\"from\": 1})\n";
    assert!(
        script.contains(block),
        "the generic call sits in the part block:\n{script}"
    );
}

#[test]
fn python_api_t011_consecutive_features_of_one_part_share_a_block() {
    let text = document_text(
        "mm",
        json!([]),
        json!([{"id": "b-1", "name": "Base"}, {"id": "b-2", "name": "Lid"}]),
        json!([
            {"id": "f-a", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"},
            {
                "id": "f-s",
                "type": "sketch",
                "plane": {"kind": "origin", "plane": "XY"},
                "entities": [{"id": "e-1", "type": "circle", "center": [0, 0], "radius": 5}],
                "constraints": []
            },
            {
                "id": "f-b",
                "type": "extrude",
                "body": "b-2",
                "profile": {"sketch": "f-s", "regions": [{"key": "e-1", "fp": "c=0,0|a=78.539816"}]},
                "extent": {"kind": "distance", "expr": "10"},
                "op": "new"
            },
            {"id": "f-c", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "position": ["0", "0", "10"], "op": "join"}
        ]),
    );

    let script = script_of(&text);

    let blocks: Vec<&str> = script
        .lines()
        .filter(|line| line.starts_with("with "))
        .collect();
    assert_eq!(
        blocks,
        ["with base:", "with lid:", "with base:"],
        "\n{script}"
    );
    assert_eq!(
        feature_blocks(&script),
        [
            (0, "f-a".to_owned()),
            (1, "f-s".to_owned()),
            (1, "f-b".to_owned()),
            (2, "f-c".to_owned())
        ],
        "\n{script}"
    );
}

#[test]
fn python_api_t012_a_rollback_marker_ends_the_script() {
    let text = replace_once(
        &fixture("box.ptsm"),
        "  ]\n}\n",
        "  ],\n  \"rollback\": \"f-box1\"\n}\n",
    );

    let script = script_of(&text);

    assert_eq!(script.lines().last(), Some(r#"rollback("f-box1")"#));
}

#[test]
fn python_api_t013_name_and_suppressed_come_before_the_id() {
    let text = replace_once(
        &fixture("box.ptsm"),
        r#"{"id": "f-box1", "type": "box", "body""#,
        r#"{"id": "f-box1", "type": "box", "name": "Base block", "suppressed": true, "body""#,
    );

    let script = script_of(&text);

    let line = r#"    box(20*mm, 30*mm, height, name="Base block", suppressed=True, id="f-box1")"#;
    assert!(
        script.lines().any(|candidate| candidate == line),
        "{line}\nin:\n{script}"
    );
}

#[test]
fn python_api_t014_a_trailing_sketch_on_a_face_is_a_top_level_block() {
    let sketch = json!({
        "id": "f-sk2",
        "type": "sketch",
        "plane": {
            "kind": "face",
            "body": "b-1",
            "face": {"q": "f-box1/top", "fp": TOP_FINGERPRINT},
            "fallback": {"origin": [0, 0, 40], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}
        },
        "entities": [{"id": "e-p1", "type": "point", "position": [0, 0]}],
        "constraints": []
    });
    let text = document_text(
        "mm",
        json!([{"id": "p-h", "name": "height", "expr": "40 mm"}]),
        json!([{"id": "b-1", "name": "Block"}]),
        json!([
            {"id": "f-box1", "type": "box", "body": "b-1", "size": ["20 mm", "30 mm", "height"], "op": "new"},
            sketch
        ]),
    );

    let script = script_of(&text);

    let ending = format!(
        r#"with Sketch(
    Ref("f-box1/top", "{TOP_FINGERPRINT}", part=block),
    fallback=Placement((0, 0, 40), (1, 0, 0), (0, 0, 1)),
    id="f-sk2",
):
    Point(0, 0, id="e-p1")
"#
    );
    assert!(
        script.ends_with(&ending),
        "the script ends with:\n{ending}\ngot:\n{script}"
    );
}

#[test]
fn python_api_t086_a_fallback_in_body_coordinates_is_written_with_its_frame() {
    let sketch = json!({
        "id": "f-sk2",
        "type": "sketch",
        "plane": {
            "kind": "face",
            "body": "b-1",
            "face": {"q": "f-box1/top", "fp": TOP_FINGERPRINT},
            "fallback": {"origin": [0, 0, 40], "x_axis": [1, 0, 0], "normal": [0, 0, 1]},
            "fallback_frame": "body"
        },
        "entities": [{"id": "e-p1", "type": "point", "position": [0, 0]}],
        "constraints": []
    });
    let text = document_text(
        "mm",
        json!([]),
        json!([{"id": "b-1", "name": "Block"}]),
        json!([
            {"id": "f-box1", "type": "box", "body": "b-1", "size": ["20 mm", "30 mm", "40 mm"], "op": "new"},
            sketch
        ]),
    );

    let script = script_of(&text);

    let ending = format!(
        r#"with Sketch(
    Ref("f-box1/top", "{TOP_FINGERPRINT}", part=block),
    fallback=Placement((0, 0, 40), (1, 0, 0), (0, 0, 1)),
    fallback_frame="body",
    id="f-sk2",
):
    Point(0, 0, id="e-p1")
"#
    );
    assert!(
        script.ends_with(&ending),
        "the script ends with:\n{ending}\ngot:\n{script}"
    );
}

#[test]
fn python_api_t015_raw_lengths_carry_mm_in_inch_documents() {
    let text = replace_once(
        &fixture("drag.ptsm"),
        r#""units": "mm""#,
        r#""units": "in""#,
    );

    let script = script_of(&text);

    let line = r#"line_1 = Line((0*mm, 0*mm), (10*mm, 0*mm), id="e-1")"#;
    assert!(
        script
            .lines()
            .any(|candidate| candidate.trim_start() == line),
        "{line}\nin:\n{script}"
    );
}

#[test]
fn python_api_t016_export_python_is_deterministic() {
    let mut session = session();
    let doc = open_doc(&mut session, "fillet.ptsm");

    let first = python_script(&mut session, doc);
    let second = python_script(&mut session, doc);

    assert_eq!(first, second);
}

#[test]
fn python_api_t085_reserved_sketch_references_keep_the_sketch_block() {
    let sketch = json!({
        "id": "f-sk1",
        "type": "sketch",
        "plane": {"kind": "origin", "plane": "XY"},
        "entities": [
            {"id": "e-l1", "type": "line", "start": [1, 1], "end": [9, 3]},
            {"id": "e-p1", "type": "point", "position": [2, 7]}
        ],
        "constraints": [
            {"id": "c-1", "type": "coincident", "points": ["e-l1.start", "origin"]},
            {"id": "c-2", "type": "angle", "lines": ["x_axis", "e-l1"], "expr": "30 deg"},
            {"id": "c-3", "type": "distance", "line": "e-l1", "expr": "10"},
            {"id": "c-4", "type": "point_on_object", "point": "e-p1", "curve": "y_axis"}
        ]
    });
    let text = document_text("mm", json!([]), json!([]), json!([sketch]));

    let script = script_of(&text);

    let ending = r#"with Sketch(Plane.XY, id="f-sk1"):
    line_1 = Line((1, 1), (9, 3), id="e-l1")
    point_1 = Point(2, 7, id="e-p1")
    Coincident(line_1.start, Sketch.ORIGIN, id="c-1")
    Angle(Sketch.X_AXIS, line_1, 30*deg, id="c-2")
    Distance(line_1, 10, id="c-3")
    PointOn(point_1, Sketch.Y_AXIS, id="c-4")
"#;
    assert!(
        script.ends_with(ending),
        "the script ends with:\n{ending}\ngot:\n{script}"
    );
    assert!(!script.contains("feature(\"sketch\""), "{script}");
}
