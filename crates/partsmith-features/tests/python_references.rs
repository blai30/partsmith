//! Acceptance tests for the file format version 2 fields in params and Python code (features.md sections 2.5, 3.11
//! and 6.5, tests 12 and 13): anchors and merged names of references, region outlines, picked axis directions, and a
//! face sketch's `fallback_frame`, each kept through canonicalization and written as a keyword only when stored.

use crate::common;

use common::{id, record};
use partsmith_features::{AxisRef, BodyOp, PythonNames, PythonValue, Registry};
use partsmith_model::canonical::{normalize_numbers, to_value};
use partsmith_model::{FeatureRecord, Id, Units};
use partsmith_naming::Reference;
use partsmith_sketch::RegionRef;
use serde_json::{Value, json};

/// The first reference example of naming.md section 2.4, with anchors.
fn anchored_json() -> Value {
    json!({"q": "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)", "fp": "line|n=1,0,0|c=65,0,30|l=30|adj=2,2", "nb": {"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]}})
}

/// The second reference example of naming.md section 2.4, with merged names.
fn merged_json() -> Value {
    json!({"q": "f-box1/top", "fp": "plane|n=0,0,1|c=50,10,10|a=2000|adj=4,4", "merged": {"f-box1/top": ["f-box1/top@f-lp1:*"]}})
}

/// A reference from its JSON.
fn reference(value: Value) -> Reference {
    serde_json::from_value(value).expect("references written in tests are valid")
}

/// The `edge` axis example of features.md section 2.5, with `direction`.
fn edge_axis_json() -> Value {
    json!({"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-ext1/lateral/e-4|f-ext1/start-cap)", "fp": "line|n=0,1,0|c=0,10,0|l=20|adj=2,2"}, "direction": [0, 1, 0]})
}

/// Script names: units mm, body `b-1` as `block`, sketch `f-sk1` as `sketch_1` with the regions of `RECT(0, 0, 80,
/// 50)` and of a circle beside it, and default op new.
struct Script;

impl PythonNames for Script {
    fn units(&self) -> Units {
        Units::Mm
    }

    fn body(&self, body: &Id) -> Option<String> {
        (body.as_str() == "b-1").then(|| "block".to_string())
    }

    fn feature(&self, feature: &Id) -> Option<String> {
        (feature.as_str() == "f-sk1").then(|| "sketch_1".to_string())
    }

    fn entity(&self, _sketch: &Id, _entity: &Id) -> Option<String> {
        None
    }

    fn parameter(&self, _name: &str) -> Option<String> {
        None
    }

    fn regions(&self, sketch: &Id) -> Option<Vec<RegionRef>> {
        (sketch.as_str() == "f-sk1").then(|| {
            vec![
                serde_json::from_value(json!({"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-1,e-2,e-3,e-4"}))
                    .expect("a valid region"),
                serde_json::from_value(json!({"key": "e-5", "fp": "c=100,25|a=78.539816", "outline": "e-5"}))
                    .expect("a valid region"),
            ]
        })
    }

    fn default_op(&self, _body: &Id) -> BodyOp {
        BodyOp::New
    }
}

/// The dedicated form of `value` as one rendered statement.
fn dedicated(value: Value) -> String {
    let registry = Registry::with_builtins();
    registry.python(&record(value), &Script).render(0)
}

#[test]
fn t012_a_reference_with_anchors_writes_them() {
    assert_eq!(
        PythonValue::reference(&reference(anchored_json()), None).render(),
        r#"Ref("edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)", "line|n=1,0,0|c=65,0,30|l=30|adj=2,2", anchors={"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]})"#
    );
}

#[test]
fn t012_a_reference_with_merged_names_writes_them() {
    assert_eq!(
        PythonValue::reference(&reference(merged_json()), None).render(),
        r#"Ref("f-box1/top", "plane|n=0,0,1|c=50,10,10|a=2000|adj=4,4", merged={"f-box1/top": ["f-box1/top@f-lp1:*"]})"#
    );
}

#[test]
fn t012_an_edge_axis_with_a_direction_writes_it_after_the_part() {
    let axis: AxisRef = serde_json::from_value(edge_axis_json()).expect("the axis deserializes");

    let value = PythonValue::axis(&axis, &Script).expect("the body has a part variable");

    assert_eq!(
        value.render(),
        r#"Ref("edge(f-ext1/lateral/e-4|f-ext1/start-cap)", "line|n=0,1,0|c=0,10,0|l=20|adj=2,2", part=block, direction=(0, 1, 0))"#
    );
}

#[test]
fn t012_a_region_with_an_outline_writes_it() {
    let region: RegionRef = serde_json::from_value(
        json!({"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-1,e-2,e-3,e-4"}),
    )
    .expect("the region deserializes");

    assert_eq!(
        PythonValue::region(&region).render(),
        r#"Region("e-1,e-2,e-3,e-4", "c=40,25|a=4000", outline="e-1,e-2,e-3,e-4")"#
    );
}

#[test]
fn t012_a_region_without_an_outline_writes_none() {
    let region: RegionRef =
        serde_json::from_value(json!({"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}))
            .expect("the region deserializes");

    assert_eq!(
        PythonValue::region(&region).render(),
        r#"Region("e-1,e-2,e-3,e-4", "c=40,25|a=4000")"#
    );
}

#[test]
fn a_reference_without_anchors_or_merged_names_writes_neither() {
    let short = reference(
        json!({"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}),
    );

    assert_eq!(
        PythonValue::reference(&short, Some("block".to_string())).render(),
        r#"Ref("edge(f-box1/front|f-box1/top)", "line|n=1,0,0|c=0,-10,20|l=20", part=block)"#
    );
}

#[test]
fn a_reference_with_a_part_writes_the_part_before_anchors() {
    assert_eq!(
        PythonValue::reference(&reference(anchored_json()), Some("block".to_string())).render(),
        r#"Ref("edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)", "line|n=1,0,0|c=65,0,30|l=30|adj=2,2", part=block, anchors={"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]})"#
    );
}

#[test]
fn an_edge_axis_without_a_direction_writes_none() {
    let mut value = edge_axis_json();
    value
        .as_object_mut()
        .expect("an object")
        .remove("direction");
    let axis: AxisRef = serde_json::from_value(value).expect("the axis deserializes");

    assert_eq!(
        PythonValue::axis(&axis, &Script)
            .expect("the body has a part variable")
            .render(),
        r#"Ref("edge(f-ext1/lateral/e-4|f-ext1/start-cap)", "line|n=0,1,0|c=0,10,0|l=20|adj=2,2", part=block)"#
    );
}

#[test]
fn a_face_axis_with_a_direction_writes_it() {
    let axis: AxisRef = serde_json::from_value(json!({"kind": "face", "body": "b-1", "face": {"q": "f-cyl1/side", "fp": "cylinder|n=0,0,1|c=0,0,2.5|a=157.079633|r=5"}, "direction": [0, 0, -1]}))
        .expect("the axis deserializes");

    assert_eq!(
        PythonValue::axis(&axis, &Script)
            .expect("the body has a part variable")
            .render(),
        r#"Ref("f-cyl1/side", "cylinder|n=0,0,1|c=0,0,2.5|a=157.079633|r=5", part=block, direction=(0, 0, -1))"#
    );
}

#[test]
fn a_fillet_writes_the_anchors_of_its_one_edge() {
    let text = dedicated(json!({
        "id": "f-fil1",
        "type": "fillet",
        "body": "b-1",
        "edges": [anchored_json()],
        "radius": "2"
    }));

    assert!(
        text.contains(r#"Ref("edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)", "line|n=1,0,0|c=65,0,30|l=30|adj=2,2", anchors={"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]})"#),
        "{text}"
    );
    assert!(text.starts_with("fillet("), "{text}");
}

#[test]
fn a_shell_writes_the_merged_names_of_its_faces_in_a_list() {
    let short = json!({"q": "f-box1/front", "fp": "plane|n=0,-1,0|c=0,-25,15|a=2400"});
    let text = dedicated(json!({
        "id": "f-sh1",
        "type": "shell",
        "body": "b-1",
        "thickness": "2",
        "remove_faces": [merged_json(), short]
    }));

    assert!(
        text.contains(r#"[Ref("f-box1/top", "plane|n=0,0,1|c=50,10,10|a=2000|adj=4,4", merged={"f-box1/top": ["f-box1/top@f-lp1:*"]}), Ref("f-box1/front", "plane|n=0,-1,0|c=0,-25,15|a=2400")]"#),
        "{text}"
    );
}

#[test]
fn an_extrude_of_some_regions_writes_their_outlines() {
    let text = dedicated(json!({
        "id": "f-ext1",
        "type": "extrude",
        "body": "b-1",
        "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-1,e-2,e-3,e-4"}]},
        "extent": {"kind": "distance", "expr": "30"},
        "op": "new"
    }));

    assert!(
        text.contains(
            r#"regions=[Region("e-1,e-2,e-3,e-4", "c=40,25|a=4000", outline="e-1,e-2,e-3,e-4")]"#
        ),
        "{text}"
    );
}

#[test]
fn an_extrude_of_every_region_as_stored_writes_no_regions() {
    let text = dedicated(json!({
        "id": "f-ext1",
        "type": "extrude",
        "body": "b-1",
        "profile": {"sketch": "f-sk1", "regions": [
            {"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-1,e-2,e-3,e-4"},
            {"key": "e-5", "fp": "c=100,25|a=78.539816", "outline": "e-5"}
        ]},
        "extent": {"kind": "distance", "expr": "30"},
        "op": "new"
    }));

    assert_eq!(text, r#"extrude(sketch_1, 30, id="f-ext1")"#);
}

#[test]
fn a_revolve_writes_the_direction_of_its_edge_axis() {
    let text = dedicated(json!({
        "id": "f-rev1",
        "type": "revolve",
        "body": "b-1",
        "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-1,e-2,e-3,e-4"}]},
        "axis": edge_axis_json(),
        "extent": {"kind": "full"},
        "op": "new"
    }));

    assert!(text.contains("part=block, direction=(0, 1, 0))"), "{text}");
}

/// Compact JSON text of `value` with numbers in canonical form (whole numbers without a fraction), keys in order.
fn compact(value: &Value) -> String {
    let mut value = value.clone();
    normalize_numbers(&mut value);
    serde_json::to_string(&value).expect("JSON values serialize")
}

/// Canonicalizes `value` with the built-in registry and returns the record and the compact text of its params.
fn canonical(value: Value) -> (FeatureRecord, String) {
    let validated = Registry::with_builtins()
        .validate_and_canonicalize(&record(value))
        .expect("the record is valid");
    let text = compact(&Value::Object(validated.record.params.clone()));
    (validated.record, text)
}

#[test]
fn t013_a_picked_direction_round_trips_after_the_edge() {
    let (_, text) = canonical(json!({
        "id": "f-lp1",
        "type": "linear_pattern",
        "body": "b-1",
        "direction": {"axis": edge_axis_json(), "count": "2", "spacing": "40"}
    }));

    assert!(
        text.contains(r#""axis":{"kind":"edge","body":"b-1","edge":{"q":"edge(f-ext1/lateral/e-4|f-ext1/start-cap)","fp":"line|n=0,1,0|c=0,10,0|l=20|adj=2,2"},"direction":[0,1,0]}"#),
        "{text}"
    );
}

#[test]
fn t013_an_axis_without_a_direction_writes_none() {
    let mut axis = edge_axis_json();
    axis.as_object_mut().expect("an object").remove("direction");

    let (_, text) = canonical(json!({
        "id": "f-lp1",
        "type": "linear_pattern",
        "body": "b-1",
        "direction": {"axis": axis, "count": "2", "spacing": "40"}
    }));

    assert!(!text.contains("direction\":["), "{text}");
}

#[test]
fn t013_a_direction_that_is_not_a_unit_vector_is_invalid() {
    for direction in [json!([0, 2, 0]), json!([0, 1])] {
        let mut axis = edge_axis_json();
        axis["direction"] = direction.clone();
        let result = Registry::with_builtins().validate_and_canonicalize(&record(json!({
            "id": "f-lp1",
            "type": "linear_pattern",
            "body": "b-1",
            "direction": {"axis": axis, "count": "2", "spacing": "40"}
        })));
        let error = result.expect_err("the direction is invalid");
        assert_eq!(error.code(), "invalid_params", "{direction}");
    }
}

#[test]
fn t013_an_axis_direction_deserializes_into_the_axis() {
    let axis: AxisRef = serde_json::from_value(edge_axis_json()).expect("the axis deserializes");

    let AxisRef::Edge {
        body, direction, ..
    } = &axis
    else {
        panic!("expected an edge axis, got {axis:?}");
    };
    assert_eq!(body, &id("b-1"));
    assert_eq!(*direction, Some([0.0, 1.0, 0.0]));
    assert_eq!(
        compact(&to_value(&axis).expect("the axis serializes")),
        compact(&edge_axis_json())
    );
}

#[test]
fn t013_an_axis_without_a_direction_serializes_without_the_key() {
    let mut value = edge_axis_json();
    value
        .as_object_mut()
        .expect("an object")
        .remove("direction");
    let axis: AxisRef = serde_json::from_value(value.clone()).expect("the axis deserializes");

    assert_eq!(
        compact(&to_value(&axis).expect("the axis serializes")),
        compact(&value)
    );
}

#[test]
fn anchors_merged_names_and_outlines_are_kept_by_canonicalization() {
    let (_, fillet) = canonical(json!({
        "id": "f-fil1",
        "type": "fillet",
        "body": "b-1",
        "edges": [anchored_json()],
        "radius": "2"
    }));
    let (_, shell) = canonical(json!({
        "id": "f-sh1",
        "type": "shell",
        "body": "b-1",
        "thickness": "2",
        "remove_faces": [merged_json()]
    }));
    let (_, extrude) = canonical(json!({
        "id": "f-ext1",
        "type": "extrude",
        "body": "b-1",
        "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-1,e-2,e-3,e-4"}]},
        "extent": {"kind": "distance", "expr": "30"},
        "op": "new"
    }));

    assert!(
        fillet.contains(r#""nb":{"f-ext1/end-cap#2":["f-ext1/lateral/e-2","f-ext2/lateral/e-6"]}"#),
        "{fillet}"
    );
    assert!(
        shell.contains(r#""merged":{"f-box1/top":["f-box1/top@f-lp1:*"]}"#),
        "{shell}"
    );
    assert!(
        extrude.contains(r#""outline":"e-1,e-2,e-3,e-4""#),
        "{extrude}"
    );
}

#[test]
fn a_face_sketch_keeps_its_fallback_frame() {
    let plane = |frame: Option<&str>| {
        let mut plane = json!({
            "kind": "face",
            "body": "b-1",
            "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=100"},
            "fallback": {"origin": [0, 0, 10], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}
        });
        if let Some(frame) = frame {
            plane["fallback_frame"] = json!(frame);
        }
        json!({"id": "f-sk1", "type": "sketch", "plane": plane, "entities": [{"id": "e-p1", "type": "point", "position": [0, 0]}]})
    };

    let (_, body) = canonical(plane(Some("body")));
    let (_, world) = canonical(plane(Some("world")));
    let (_, absent) = canonical(plane(None));

    assert!(
        body.contains(r#""fallback":{"origin":[0,0,10],"x_axis":[1,0,0],"normal":[0,0,1]},"fallback_frame":"body""#),
        "{body}"
    );
    assert!(!world.contains("fallback_frame"), "{world}");
    assert_eq!(world, absent);
}
