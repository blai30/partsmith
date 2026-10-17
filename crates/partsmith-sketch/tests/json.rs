//! Acceptance tests for the JSON form and JSON Schema of sketch params (spec section 6.1, tests 1 to 17, and
//! section 6.16, test 217).

mod common;

use std::collections::BTreeSet;
use std::fmt::Debug;

use common::benchmark::PLATE_PARAMS_JSON;
use common::{FACE_PLANE_JSON, Sketch, id, plate, rect};
use partsmith_sketch::{
    DragTarget, Geometry, HintFrame, OriginPlane, PointRef, PointRole, ProfileCurve,
    ProfileFinding, RegionMatch, RegionRef, SketchConstraint, SketchEntity, SketchError,
    SketchParams, SketchPlane,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// The `PointRef` pattern of spec section 2.4.
const POINT_REF_PATTERN: &str = r"^[A-Za-z0-9_-]{1,64}(\.(start|end|center))?$";

/// Every constraint example of spec section 2.5, including those with reserved references.
const CONSTRAINT_EXAMPLES: [&str; 30] = [
    r#"{"id": "c-1", "type": "coincident", "points": ["e-1.end", "e-2.start"]}"#,
    r#"{"id": "c-2", "type": "point_on_object", "point": "e-7", "curve": "e-5"}"#,
    r#"{"id": "c-3", "type": "horizontal", "line": "e-1"}"#,
    r#"{"id": "c-4", "type": "horizontal", "points": ["e-5.center", "e-11.center"]}"#,
    r#"{"id": "c-5", "type": "vertical", "line": "e-2"}"#,
    r#"{"id": "c-6", "type": "vertical", "points": ["e-7", "e-5.center"]}"#,
    r#"{"id": "c-7", "type": "parallel", "lines": ["e-1", "e-3"]}"#,
    r#"{"id": "c-8", "type": "perpendicular", "lines": ["e-1", "e-2"]}"#,
    r#"{"id": "c-9", "type": "tangent", "curves": ["e-3", "e-9"]}"#,
    r#"{"id": "c-10", "type": "collinear", "lines": ["e-1", "e-12"]}"#,
    r#"{"id": "c-11", "type": "concentric", "curves": ["e-5", "e-11"]}"#,
    r#"{"id": "c-12", "type": "equal", "curves": ["e-1", "e-3"]}"#,
    r#"{"id": "c-13", "type": "symmetric", "points": ["e-9.start", "e-9.end"], "line": "e-6"}"#,
    r#"{"id": "c-14", "type": "midpoint", "point": "e-7", "line": "e-1"}"#,
    r#"{"id": "c-15", "type": "fix", "target": "e-1.start"}"#,
    r#"{"id": "c-16", "type": "fix", "target": "e-5"}"#,
    r#"{"id": "c-17", "type": "distance", "points": ["e-1.start", "e-7"], "expr": "12.5 mm"}"#,
    r#"{"id": "c-18", "type": "distance", "line": "e-1", "expr": "width"}"#,
    r#"{"id": "c-19", "type": "distance", "point": "e-7", "line": "e-3", "expr": "wall + 1 mm", "driving": false}"#,
    r#"{"id": "c-20", "type": "horizontal_distance", "points": ["e-1.start", "e-5.center"], "expr": "20 mm"}"#,
    r#"{"id": "c-21", "type": "horizontal_distance", "line": "e-1", "expr": "80 mm"}"#,
    r#"{"id": "c-22", "type": "vertical_distance", "points": ["e-1.start", "e-5.center"], "expr": "15 mm"}"#,
    r#"{"id": "c-23", "type": "vertical_distance", "line": "e-2", "expr": "50 mm"}"#,
    r#"{"id": "c-24", "type": "radius", "curve": "e-9", "expr": "3 mm"}"#,
    r#"{"id": "c-25", "type": "diameter", "curve": "e-5", "expr": "hole_d"}"#,
    r#"{"id": "c-26", "type": "angle", "lines": ["e-1", "e-12"], "expr": "30 deg"}"#,
    r#"{"id": "c-27", "type": "coincident", "points": ["e-1.start", "origin"]}"#,
    r#"{"id": "c-28", "type": "point_on_object", "point": "e-5.center", "curve": "y_axis"}"#,
    r#"{"id": "c-29", "type": "angle", "lines": ["x_axis", "e-12"], "expr": "30 deg"}"#,
    r#"{"id": "c-30", "type": "symmetric", "points": ["e-9.start", "e-9.end"], "line": "x_axis"}"#,
];

/// Deserializes `text`, panicking with the serde message on failure.
fn parse<T: DeserializeOwned>(text: &str) -> T {
    serde_json::from_str(text).unwrap_or_else(|error| panic!("{text} deserializes: {error}"))
}

/// Serializes with `serde_json::to_string`.
fn write<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("the value serializes")
}

/// The keys of a JSON object text, in written order.
fn keys(text: &str) -> Vec<String> {
    let value: Value = parse(text);
    value
        .as_object()
        .unwrap_or_else(|| panic!("{text} is a JSON object"))
        .keys()
        .cloned()
        .collect()
}

/// Deserialize, serialize, deserialize gives an equal value.
#[track_caller]
fn assert_round_trip<T: DeserializeOwned + Serialize + PartialEq + Debug>(text: &str) {
    let first: T = parse(text);
    let written = write(&first);
    let second: T = parse(&written);
    assert_eq!(first, second, "{text} round-trips through {written}");
}

/// Collects the `const` value, or the single `enum` value, of every property named `property`.
fn collect_tags(schema: &Value, property: &str, found: &mut BTreeSet<String>) {
    match schema {
        Value::Object(map) => {
            if let Some(Value::Object(properties)) = map.get("properties")
                && let Some(tag) = properties.get(property)
            {
                if let Some(Value::String(text)) = tag.get("const") {
                    found.insert(text.clone());
                }
                if let Some(Value::Array(items)) = tag.get("enum")
                    && let [Value::String(text)] = items.as_slice()
                {
                    found.insert(text.clone());
                }
            }
            for child in map.values() {
                collect_tags(child, property, found);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_tags(item, property, found);
            }
        }
        _ => {}
    }
}

/// True when some `pattern` keyword in the schema equals `pattern`.
fn has_pattern(schema: &Value, pattern: &str) -> bool {
    match schema {
        Value::Object(map) => {
            map.get("pattern").and_then(Value::as_str) == Some(pattern)
                || map.values().any(|child| has_pattern(child, pattern))
        }
        Value::Array(items) => items.iter().any(|item| has_pattern(item, pattern)),
        _ => false,
    }
}

#[test]
fn t001_line_deserializes_and_serializes_compactly() {
    let entity: SketchEntity =
        parse(r#"{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}"#);

    assert_eq!(
        entity,
        SketchEntity {
            id: id("e-1"),
            geometry: Geometry::Line {
                start: [0.0, 0.0],
                end: [80.0, 0.0],
            },
            construction: false,
        }
    );
    assert_eq!(
        write(&entity),
        r#"{"id":"e-1","type":"line","start":[0.0,0.0],"end":[80.0,0.0]}"#
    );
}

#[test]
fn t002_construction_flag_is_written_last_when_true() {
    let entity: SketchEntity = parse(
        r#"{"id": "e-6", "type": "line", "start": [40, 0], "end": [40, 50], "construction": true}"#,
    );

    assert_eq!(
        write(&entity),
        r#"{"id":"e-6","type":"line","start":[40.0,0.0],"end":[40.0,50.0],"construction":true}"#
    );
}

#[test]
fn t003_false_construction_flag_is_omitted() {
    let entity: SketchEntity = parse(
        r#"{"id": "e-6", "type": "line", "start": [40, 0], "end": [40, 50], "construction": false}"#,
    );

    assert_eq!(
        write(&entity),
        r#"{"id":"e-6","type":"line","start":[40.0,0.0],"end":[40.0,50.0]}"#
    );
}

#[test]
fn t004_point_circle_and_arc_keep_field_order() {
    let cases = [
        (
            r#"{"id": "e-7", "type": "point", "position": [40, 25]}"#,
            vec!["id", "type", "position"],
        ),
        (
            r#"{"id": "e-5", "type": "circle", "center": [40, 25], "radius": 5}"#,
            vec!["id", "type", "center", "radius"],
        ),
        (
            r#"{"id": "e-9", "type": "arc", "center": [20, 0], "radius": 5, "start_angle": 4.71238898038469, "end_angle": 1.5707963267948966}"#,
            vec!["id", "type", "center", "radius", "start_angle", "end_angle"],
        ),
    ];

    for (text, expected) in cases {
        let entity: SketchEntity = parse(text);
        assert_eq!(keys(&write(&entity)), expected, "key order of {text}");
    }
}

#[test]
fn t005_invalid_entities_fail_to_deserialize() {
    let invalid = [
        r#"{"id": "e-1", "type": "ellipse", "center": [0, 0]}"#,
        r#"{"id": "e-1", "type": "line", "start": [0, 0]}"#,
        r#"{"id": "e-1", "type": "point", "position": [0, 0], "color": "red"}"#,
        r#"{"id": "e-1", "type": "point", "position": [0, 0, 0]}"#,
        r#"{"id": "e 1", "type": "point", "position": [0, 0]}"#,
    ];

    for text in invalid {
        let result = serde_json::from_str::<SketchEntity>(text);
        assert!(
            result.is_err(),
            "{text} must not deserialize, got {result:?}"
        );
    }
}

#[test]
fn t006_every_constraint_example_round_trips_to_compact_text() {
    for text in CONSTRAINT_EXAMPLES {
        let constraint: SketchConstraint = parse(text);
        let expected = text.replace(": ", ":").replace(", ", ",");
        assert_eq!(write(&constraint), expected, "serialization of {text}");
    }
}

#[test]
fn t007_true_driving_flag_is_omitted() {
    let constraint: SketchConstraint = parse(
        r#"{"id": "c-2", "type": "distance", "points": ["e-1", "e-2"], "expr": "10 mm", "driving": true}"#,
    );

    assert_eq!(
        write(&constraint),
        r#"{"id":"c-2","type":"distance","points":["e-1","e-2"],"expr":"10 mm"}"#
    );
}

#[test]
fn t008_invalid_constraints_fail_to_deserialize() {
    let invalid = [
        r#"{"id": "c-1", "type": "horizontal"}"#,
        r#"{"id": "c-1", "type": "horizontal", "line": "e-1", "points": ["e-1.start", "e-2"]}"#,
        r#"{"id": "c-1", "type": "distance", "points": ["e-1", "e-2"], "line": "e-3", "expr": "1"}"#,
        r#"{"id": "c-1", "type": "radius", "curve": "e-1"}"#,
        r#"{"id": "c-1", "type": "parallel", "lines": ["e-1"]}"#,
        r#"{"id": "c-1", "type": "smooth", "curves": ["e-1", "e-2"]}"#,
        r#"{"id": "c-1", "type": "coincident", "points": ["e-1.mid", "e-2"]}"#,
    ];

    for text in invalid {
        let result = serde_json::from_str::<SketchConstraint>(text);
        assert!(
            result.is_err(),
            "{text} must not deserialize, got {result:?}"
        );
    }
}

#[test]
fn t009_point_ref_parses_each_role() {
    let cases = [
        ("e-1.start", "e-1", PointRole::Start),
        ("e-1.end", "e-1", PointRole::End),
        ("e-5.center", "e-5", PointRole::Center),
        ("e-7", "e-7", PointRole::Point),
        ("f_Sk-2", "f_Sk-2", PointRole::Point),
    ];

    for (text, entity, role) in cases {
        let parsed: PointRef = text
            .parse()
            .unwrap_or_else(|error| panic!("{text} parses: {error}"));
        assert_eq!(
            parsed,
            PointRef {
                entity: id(entity),
                role
            },
            "{text}"
        );
    }
}

#[test]
fn t010_invalid_point_refs_fail_with_their_text() {
    let long_id = "e".repeat(65);
    let invalid = [
        "",
        ".start",
        "e-1.",
        "e-1.mid",
        "e-1.start.end",
        "e 1",
        "e-1.Start",
        long_id.as_str(),
    ];

    for text in invalid {
        assert_eq!(
            text.parse::<PointRef>(),
            Err(SketchError::InvalidPointRef {
                text: text.to_owned()
            }),
            "{text:?}"
        );
    }
}

#[test]
fn t011_point_ref_text_round_trips() {
    for text in ["e-1.start", "e-1.end", "e-5.center", "e-7", "f_Sk-2"] {
        let parsed: PointRef = text
            .parse()
            .unwrap_or_else(|error| panic!("{text} parses: {error}"));
        assert_eq!(parsed.to_string(), text);
        assert_eq!(write(&parsed), format!("\"{text}\""));
    }
}

#[test]
fn t012_origin_plane_deserializes() {
    let plane: SketchPlane = parse(r#"{"kind": "origin", "plane": "XZ"}"#);

    assert_eq!(
        plane,
        SketchPlane::Origin {
            plane: OriginPlane::Xz
        }
    );
}

#[test]
fn t012_origin_plane_names_are_case_sensitive() {
    let result = serde_json::from_str::<SketchPlane>(r#"{"kind": "origin", "plane": "xz"}"#);

    assert!(
        result.is_err(),
        "lowercase xz must not deserialize, got {result:?}"
    );
}

#[test]
fn t012_datum_plane_round_trips() {
    let plane: SketchPlane = parse(r#"{"kind": "datum", "feature": "f-dp1"}"#);

    assert_eq!(
        plane,
        SketchPlane::Datum {
            feature: id("f-dp1")
        }
    );
    assert_eq!(write(&plane), r#"{"kind":"datum","feature":"f-dp1"}"#);
    assert_round_trip::<SketchPlane>(r#"{"kind": "datum", "feature": "f-dp1"}"#);
}

#[test]
fn t012_face_plane_serializes_in_field_order() {
    let plane: SketchPlane = parse(FACE_PLANE_JSON);

    assert_eq!(
        write(&plane),
        r#"{"kind":"face","body":"b-1","face":{"q":"f-ext1/end-cap","fp":"plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"},"fallback":{"origin":[0.0,0.0,30.0],"x_axis":[1.0,0.0,0.0],"normal":[0.0,0.0,1.0]}}"#
    );
}

#[test]
fn t012_face_plane_without_body_fails() {
    let text = FACE_PLANE_JSON.replace(r#""body": "b-1", "#, "");
    assert!(!text.contains("body"), "the fixture removed the body field");

    let result = serde_json::from_str::<SketchPlane>(&text);

    assert!(
        result.is_err(),
        "a face plane without body must not deserialize, got {result:?}"
    );
}

#[test]
fn t013_missing_entity_and_constraint_lists_load_empty_and_are_written() {
    let params: SketchParams = parse(r#"{"plane": {"kind": "origin", "plane": "XY"}}"#);

    assert!(params.entities.is_empty());
    assert!(params.constraints.is_empty());
    assert_eq!(
        write(&params),
        r#"{"plane":{"kind":"origin","plane":"XY"},"entities":[],"constraints":[]}"#
    );
}

#[test]
fn t014_plate_params_deserialize_and_round_trip() {
    let params: SketchParams = parse(PLATE_PARAMS_JSON);

    assert_eq!(params.entities.len(), 6);
    assert_eq!(params.constraints.len(), 16);
    assert_eq!(
        params,
        plate().params,
        "the JSON and the PLATE fixture agree"
    );
    let again: SketchParams = parse(&write(&params));
    assert_eq!(again, params);
}

#[test]
fn t015_schema_lists_exactly_the_m1_type_and_kind_tags() {
    let schema =
        serde_json::to_value(schemars::schema_for!(SketchParams)).expect("the schema serializes");

    let mut types = BTreeSet::new();
    collect_tags(&schema, "type", &mut types);
    let mut kinds = BTreeSet::new();
    collect_tags(&schema, "kind", &mut kinds);

    let expected_types: BTreeSet<String> = [
        "point",
        "line",
        "circle",
        "arc",
        "coincident",
        "point_on_object",
        "horizontal",
        "vertical",
        "parallel",
        "perpendicular",
        "tangent",
        "collinear",
        "concentric",
        "equal",
        "symmetric",
        "midpoint",
        "fix",
        "distance",
        "horizontal_distance",
        "vertical_distance",
        "radius",
        "diameter",
        "angle",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(expected_types.len(), 23);
    assert_eq!(types, expected_types);
    let expected_kinds: BTreeSet<String> = ["origin", "datum", "face"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(kinds, expected_kinds);
}

#[test]
fn t015_schema_has_point_ref_pattern_and_origin_plane_names() {
    let schema =
        serde_json::to_value(schemars::schema_for!(SketchParams)).expect("the schema serializes");
    let text = schema.to_string();

    assert!(
        has_pattern(&schema, POINT_REF_PATTERN),
        "the schema contains the PointRef pattern {POINT_REF_PATTERN}"
    );
    for name in [r#""XY""#, r#""XZ""#, r#""YZ""#] {
        assert!(text.contains(name), "the schema contains {name}");
    }
}

#[test]
fn t016_solve_result_fields_are_written_in_declaration_order() {
    let result = plate().solve();

    assert_eq!(
        keys(&write(&result)),
        [
            "status",
            "failed",
            "entities",
            "dof",
            "redundant",
            "conflicting",
            "entity_states",
            "measured"
        ]
    );
}

#[test]
fn t016_empty_lists_and_maps_of_a_solve_result_are_written() {
    let result = Sketch::new(Vec::new()).solve();
    let text = write(&result);

    for fragment in [
        r#""redundant":[]"#,
        r#""conflicting":[]"#,
        r#""entity_states":{}"#,
        r#""measured":{}"#,
    ] {
        assert!(text.contains(fragment), "{text} contains {fragment}");
    }
}

#[test]
fn t017_drag_targets_round_trip() {
    assert_round_trip::<DragTarget>(r#"{"kind": "point", "point": "e-1.start"}"#);
    assert_round_trip::<DragTarget>(r#"{"kind": "entity", "entity": "e-2", "grab": [5, 0]}"#);
}

#[test]
fn t017_profile_findings_round_trip() {
    assert_round_trip::<ProfileFinding>(
        r#"{"type": "open_endpoint", "entity": "e-4", "point": [0, 0.3], "connect": {"first": "e-4.end", "second": "e-1.start", "gap": 0.3}}"#,
    );
    assert_round_trip::<ProfileFinding>(
        r#"{"type": "open_endpoint", "entity": "e-3", "point": [80, 50]}"#,
    );
    assert_round_trip::<ProfileFinding>(
        r#"{"type": "overlap", "entities": ["e-1", "e-5"], "start": [20, 0], "end": [60, 0]}"#,
    );
    assert_round_trip::<ProfileFinding>(
        r#"{"type": "self_intersection", "entities": ["e-1", "e-3"], "point": [5, 5]}"#,
    );
    assert_round_trip::<ProfileFinding>(r#"{"type": "zero_length", "entity": "e-7"}"#);
}

#[test]
fn an_open_endpoint_writes_its_connection_and_leaves_out_an_absent_one() {
    let connected: ProfileFinding = parse(
        r#"{"type": "open_endpoint", "entity": "e-4", "point": [0, 0.3], "connect": {"first": "e-4.end", "second": "e-1.start", "gap": 0.3}}"#,
    );
    let alone: ProfileFinding =
        parse(r#"{"type": "open_endpoint", "entity": "e-3", "point": [80, 50]}"#);

    assert_eq!(
        write(&connected),
        r#"{"type":"open_endpoint","entity":"e-4","point":[0.0,0.3],"connect":{"first":"e-4.end","second":"e-1.start","gap":0.3}}"#
    );
    assert_eq!(
        write(&alone),
        r#"{"type":"open_endpoint","entity":"e-3","point":[80.0,50.0]}"#
    );
    assert_eq!(
        alone,
        ProfileFinding::OpenEndpoint {
            entity: id("e-3"),
            point: [80.0, 50.0],
            connect: None,
        }
    );
}

#[test]
fn t017_profile_curves_round_trip() {
    assert_round_trip::<ProfileCurve>(r#"{"kind": "line", "start": [0, 0], "end": [80, 0]}"#);
    assert_round_trip::<ProfileCurve>(
        r#"{"kind": "arc", "center": [20, 0], "radius": 5, "start": [20, -5], "end": [20, 5], "ccw": true}"#,
    );
    assert_round_trip::<ProfileCurve>(
        r#"{"kind": "circle", "center": [40, 25], "radius": 5, "ccw": false}"#,
    );
}

#[test]
fn t017_region_matches_round_trip() {
    assert_round_trip::<RegionMatch>(
        r#"{"kind": "resolved", "indices": [0], "method": "boundary", "successions": [{"entity": "e-2", "now": ["e-9"]}]}"#,
    );
    assert_round_trip::<RegionMatch>(
        r#"{"kind": "resolved", "indices": [0, 1], "method": "coverage", "successions": []}"#,
    );
    assert_round_trip::<RegionMatch>(
        r#"{"kind": "resolved", "indices": [0, 1], "method": "grown", "successions": []}"#,
    );
    assert_round_trip::<RegionMatch>(r#"{"kind": "ambiguous", "candidates": [[0], [1]]}"#);
    assert_round_trip::<RegionMatch>(
        r#"{"kind": "missing", "loss": {"kind": "deleted", "entities": ["e-2"]}}"#,
    );
    assert_round_trip::<RegionMatch>(r#"{"kind": "missing", "loss": {"kind": "redrawn"}}"#);
}

#[test]
fn t017_region_refs_round_trip() {
    assert_round_trip::<RegionRef>(
        r#"{"key": "e-1,e-2,e-3,e-4;~e-5", "fp": "c=40,25|a=3921.460184", "outline": "e-1,e-2,e-3,e-4"}"#,
    );
    assert_round_trip::<RegionRef>(
        r#"{"key": "e-1,e-2,e-3,e-4;~e-5", "fp": "c=40,25|a=3921.460184"}"#,
    );
}

#[test]
fn t017_region_fields_are_written_in_declaration_order() {
    let profile = rect().profile();
    let region = profile.regions.first().expect("RECT has a region");

    assert_eq!(
        keys(&write(region)),
        ["key", "outer", "holes", "area", "centroid"]
    );
}

/// The second face plane example of spec section 2.2: the first with its fallback in body coordinates.
fn face_plane_json(fallback_frame: &str) -> String {
    let marked = FACE_PLANE_JSON.replace(
        r#""normal": [0, 0, 1]}}"#,
        &format!(r#""normal": [0, 0, 1]}}, "fallback_frame": "{fallback_frame}"}}"#),
    );
    assert_ne!(marked, FACE_PLANE_JSON, "the fixture added the frame");
    marked
}

/// The `fallback_frame` of a face plane.
fn fallback_frame_of(plane: &SketchPlane) -> HintFrame {
    match plane {
        SketchPlane::Face { fallback_frame, .. } => *fallback_frame,
        other => panic!("a face plane, found {other:?}"),
    }
}

/// The schema of the `face` variant of `SketchPlane`: the object whose `kind` property is the constant `face`.
fn face_variant(schema: &Value) -> Option<&Value> {
    match schema {
        Value::Object(map) => {
            let kind = map
                .get("properties")
                .and_then(|properties| properties.get("kind"));
            let is_face = kind.is_some_and(|kind| {
                kind.get("const").and_then(Value::as_str) == Some("face")
                    || kind.get("enum") == Some(&serde_json::json!(["face"]))
            });
            if is_face {
                return Some(schema);
            }
            map.values().find_map(face_variant)
        }
        Value::Array(items) => items.iter().find_map(face_variant),
        _ => None,
    }
}

/// Follows a `$ref` (directly or as the only entry of `allOf`) to its definition in the root schema.
fn resolved_schema<'a>(root: &'a Value, schema: &'a Value) -> &'a Value {
    let target = schema.get("$ref").or_else(|| {
        schema
            .get("allOf")
            .and_then(Value::as_array)
            .and_then(|entries| match entries.as_slice() {
                [only] => only.get("$ref"),
                _ => None,
            })
    });
    match target.and_then(Value::as_str) {
        Some(pointer) => root
            .pointer(pointer.trim_start_matches('#'))
            .unwrap_or_else(|| panic!("the schema defines {pointer}")),
        None => schema,
    }
}

/// The string values a schema allows, in declaration order, from `const`, `enum`, `oneOf` and `anyOf`.
fn allowed_strings(schema: &Value) -> Vec<String> {
    let mut values = Vec::new();
    if let Some(constant) = schema.get("const").and_then(Value::as_str) {
        values.push(constant.to_owned());
    }
    for item in schema
        .get("enum")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(text) = item.as_str() {
            values.push(text.to_owned());
        }
    }
    for keyword in ["oneOf", "anyOf"] {
        for entry in schema
            .get(keyword)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            values.extend(allowed_strings(entry));
        }
    }
    values
}

#[test]
fn t217_face_plane_with_a_body_frame_round_trips_with_the_marker() {
    let plane: SketchPlane = parse(&face_plane_json("body"));

    assert_eq!(fallback_frame_of(&plane), HintFrame::Body);
    assert_eq!(
        write(&plane),
        r#"{"kind":"face","body":"b-1","face":{"q":"f-ext1/end-cap","fp":"plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"},"fallback":{"origin":[0.0,0.0,30.0],"x_axis":[1.0,0.0,0.0],"normal":[0.0,0.0,1.0]},"fallback_frame":"body"}"#
    );
}

#[test]
fn t217_face_plane_with_a_world_frame_is_written_without_the_marker() {
    let plane: SketchPlane = parse(&face_plane_json("world"));

    assert_eq!(fallback_frame_of(&plane), HintFrame::World);
    assert_eq!(write(&plane), write(&parse::<SketchPlane>(FACE_PLANE_JSON)));
    assert_eq!(
        write(&plane),
        r#"{"kind":"face","body":"b-1","face":{"q":"f-ext1/end-cap","fp":"plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"},"fallback":{"origin":[0.0,0.0,30.0],"x_axis":[1.0,0.0,0.0],"normal":[0.0,0.0,1.0]}}"#
    );
}

#[test]
fn t217_face_plane_with_an_unknown_frame_fails() {
    let result = serde_json::from_str::<SketchPlane>(&face_plane_json("local"));

    assert!(
        result.is_err(),
        "an unknown fallback frame must not deserialize, got {result:?}"
    );
}

#[test]
fn t217_schema_allows_world_and_body_and_does_not_require_the_frame() {
    let schema =
        serde_json::to_value(schemars::schema_for!(SketchParams)).expect("the schema serializes");

    let face = face_variant(&schema).expect("the schema has the face variant");
    let frame = face
        .pointer("/properties/fallback_frame")
        .expect("the face variant has the property fallback_frame");
    assert_eq!(
        allowed_strings(resolved_schema(&schema, frame)),
        ["world", "body"]
    );
    let required: Vec<&str> = face
        .get("required")
        .and_then(Value::as_array)
        .expect("the face variant lists its required properties")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(required.contains(&"fallback"), "required: {required:?}");
    assert!(
        !required.contains(&"fallback_frame"),
        "required: {required:?}"
    );
}

#[test]
fn face_plane_without_the_marker_loads_in_world_coordinates() {
    let plane: SketchPlane = parse(FACE_PLANE_JSON);

    assert_eq!(fallback_frame_of(&plane), HintFrame::World);
}

#[test]
fn hint_frame_defaults_to_world() {
    assert_eq!(HintFrame::default(), HintFrame::World);
}

#[test]
fn only_world_is_world() {
    assert!(HintFrame::World.is_world());
    assert!(!HintFrame::Body.is_world());
}

#[test]
fn hint_frames_are_written_in_snake_case() {
    assert_eq!(write(&HintFrame::World), r#""world""#);
    assert_eq!(write(&HintFrame::Body), r#""body""#);
    assert_eq!(parse::<HintFrame>(r#""body""#), HintFrame::Body);
}
