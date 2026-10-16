//! Acceptance tests for region loops and references with outlines (spec section 6.17, tests 218 to 220), plus the
//! rules of sections 2.7 and 3.13.8 that those leave out.

mod common;

use std::f64::consts::PI;

use common::matching::{key_edge, key_texts, region_loop, regions_of};
use common::{arc, circle, line, plate_entities, rect_entities};
use partsmith_sketch::{KeyEdge, RegionLoop, RegionRef, SketchEntity, SketchError};

/// The entries of a loop text, in written order, without canonicalizing.
fn entries(text: &str) -> Vec<KeyEdge> {
    text.split(',')
        .map(|written| match written.strip_prefix('~') {
            Some(entity) => key_edge(entity, true),
            None => key_edge(written, false),
        })
        .collect()
}

/// The outline text of every region of these entities' profile, in region order.
fn outlines(entities: &[SketchEntity]) -> Vec<String> {
    regions_of(entities)
        .iter()
        .map(|region| region.outline().to_string())
        .collect()
}

/// Test 148: the rectangle drawn clockwise.
fn clockwise_rectangle() -> Vec<SketchEntity> {
    vec![
        line("e-1", [0.0, 0.0], [0.0, 50.0]),
        line("e-2", [0.0, 50.0], [80.0, 50.0]),
        line("e-3", [80.0, 50.0], [80.0, 0.0]),
        line("e-4", [80.0, 0.0], [0.0, 0.0]),
    ]
}

/// Test 167: a rectangle whose top is an arc traversed against its direction.
fn reversed_arc() -> Vec<SketchEntity> {
    vec![
        line("e-1", [0.0, 0.0], [20.0, 0.0]),
        line("e-2", [20.0, 0.0], [20.0, 20.0]),
        arc("e-3", [10.0, 20.0], 10.0, PI, 2.0 * PI),
        line("e-4", [0.0, 20.0], [0.0, 0.0]),
    ]
}

/// `RECT` plus `extra`.
fn rect_with(extra: SketchEntity) -> Vec<SketchEntity> {
    let mut entities = rect_entities();
    entities.push(extra);
    entities
}

#[test]
fn t218_outline_of_a_counterclockwise_rectangle() {
    assert_eq!(outlines(&rect_entities()), ["e-1,e-2,e-3,e-4"]);
}

#[test]
fn t218_outline_of_a_clockwise_rectangle_is_in_traversal_order() {
    let entities = clockwise_rectangle();

    assert_eq!(key_texts(&regions_of(&entities)), ["~e-1,~e-2,~e-3,~e-4"]);
    assert_eq!(outlines(&entities), ["~e-1,~e-4,~e-3,~e-2"]);
}

#[test]
fn t218_outline_writes_two_edges_of_one_line_once() {
    let spur = rect_with(line("e-5", [40.0, 0.0], [40.0, 20.0]));

    assert_eq!(outlines(&spur), ["e-1,e-2,e-3,e-4"]);
}

#[test]
fn outline_merges_the_last_and_the_first_pieces_of_one_line() {
    // The spur comes first, so its vertex on `e-1` is created first and the loop starts in the middle of `e-1`.
    let mut spur_first = vec![line("e-5", [40.0, 0.0], [40.0, 20.0])];
    spur_first.extend(rect_entities());
    let regions = regions_of(&spur_first);
    let edges = &regions[0].outer.edges;
    assert_eq!(edges.len(), 5);
    assert_eq!(edges[0].entity.as_str(), "e-1");
    assert_eq!(edges[4].entity.as_str(), "e-1");

    assert_eq!(regions[0].outline().to_string(), "e-1,e-2,e-3,e-4");
}

#[test]
fn t218_outlines_of_a_rectangle_split_by_its_diagonal() {
    let split = rect_with(line("e-5", [0.0, 0.0], [80.0, 50.0]));

    assert_eq!(outlines(&split), ["e-1,e-2,~e-5", "e-3,e-4,e-5"]);
}

#[test]
fn t218_outline_of_a_reversed_arc() {
    assert_eq!(outlines(&reversed_arc()), ["e-1,e-2,~e-3,e-4"]);
}

#[test]
fn t218_outline_of_a_line_interrupted_by_an_arc() {
    let circle_across_top = rect_with(circle("e-5", [40.0, 50.0], 10.0));
    let regions = regions_of(&circle_across_top);

    assert_eq!(regions[0].key.to_string(), "e-1,e-2,e-3,e-4,~e-5");
    assert_eq!(regions[0].outline().to_string(), "e-1,e-2,e-3,~e-5,e-3,e-4");
}

#[test]
fn t219_loop_text_is_rotated_to_its_smallest_form() {
    let cases = [
        ("e-2,e-3,e-1", "e-1,e-2,e-3"),
        ("e-3,~e-5,e-3,e-4,e-1,e-2", "e-1,e-2,e-3,~e-5,e-3,e-4"),
        ("e-1,e-1,e-2,e-1", "e-1,e-2"),
        ("e-1,~e-1", "e-1,~e-1"),
        ("e-10,e-9", "e-10,e-9"),
        ("e-9,e-10", "e-10,e-9"),
    ];

    for (text, canonical) in cases {
        assert_eq!(region_loop(text).to_string(), canonical, "{text:?}");
    }
}

#[test]
fn t219_invalid_loop_text_fails_with_its_text() {
    for text in ["", "e-1,,e-2", "~~e-1", "e 1", "e-1;e-2", "e-1,"] {
        assert_eq!(
            text.parse::<RegionLoop>(),
            Err(SketchError::InvalidRegionKey {
                text: text.to_owned()
            }),
            "{text:?}"
        );
    }
}

#[test]
fn t219_new_without_entries_fails_with_empty_text() {
    assert_eq!(
        RegionLoop::new(Vec::new()),
        Err(SketchError::InvalidRegionKey {
            text: String::new()
        })
    );
}

#[test]
fn t219_new_canonicalizes_like_parsing() {
    let built = RegionLoop::new(entries("e-2,e-3,e-1")).expect("the entries form a loop");

    assert_eq!(built.to_string(), "e-1,e-2,e-3");
    assert_eq!(built, region_loop("e-2,e-3,e-1"));
}

#[test]
fn t220_region_ref_with_outline_round_trips_in_canonical_form() {
    let region_ref: RegionRef = serde_json::from_str(
        r#"{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-2,e-3,e-4,e-1"}"#,
    )
    .expect("the region reference deserializes");

    assert_eq!(region_ref.outline, Some(region_loop("e-1,e-2,e-3,e-4")));
    assert_eq!(
        serde_json::to_string(&region_ref).expect("the region reference serializes"),
        r#"{"key":"e-1,e-2,e-3,e-4","fp":"c=40,25|a=4000","outline":"e-1,e-2,e-3,e-4"}"#
    );
}

#[test]
fn t220_region_ref_with_an_empty_outline_fails() {
    let result = serde_json::from_str::<RegionRef>(
        r#"{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": ""}"#,
    );

    assert!(
        result.is_err(),
        "an empty outline must not deserialize, got {result:?}"
    );
}

#[test]
fn region_ref_with_a_malformed_outline_fails() {
    for outline in ["e-1;e-2", "~~e-1", "e-1,", "e 1"] {
        let text =
            format!(r#"{{"key": "e-1,e-2", "fp": "c=40,25|a=4000", "outline": "{outline}"}}"#);

        let result = serde_json::from_str::<RegionRef>(&text);

        assert!(
            result.is_err(),
            "the outline {outline:?} must not deserialize, got {result:?}"
        );
    }
}

#[test]
fn region_loop_serializes_as_its_canonical_text() {
    assert_eq!(
        serde_json::to_string(&region_loop("e-3,~e-5,e-3,e-4,e-1,e-2"))
            .expect("the loop serializes"),
        r#""e-1,e-2,e-3,~e-5,e-3,e-4""#
    );
    assert_eq!(
        serde_json::from_str::<RegionLoop>(r#""e-2,e-3,e-1""#).expect("the loop deserializes"),
        region_loop("e-1,e-2,e-3")
    );
}

#[test]
fn t220_reference_of_a_region_writes_its_outline() {
    let regions = regions_of(&rect_entities());

    assert_eq!(
        serde_json::to_string(&regions[0].reference()).expect("the reference serializes"),
        r#"{"key":"e-1,e-2,e-3,e-4","fp":"c=40,25|a=4000","outline":"e-1,e-2,e-3,e-4"}"#
    );
}

#[test]
fn region_ref_schema_does_not_require_the_outline() {
    let schema =
        serde_json::to_value(schemars::schema_for!(RegionRef)).expect("the schema serializes");

    let mut properties: Vec<&str> = schema["properties"]
        .as_object()
        .expect("a region reference has properties")
        .keys()
        .map(String::as_str)
        .collect();
    properties.sort_unstable();
    let mut required: Vec<&str> = schema["required"]
        .as_array()
        .expect("a region reference requires fields")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    required.sort_unstable();

    assert_eq!(properties, ["fp", "key", "outline"]);
    assert_eq!(required, ["fp", "key"]);
}

#[test]
fn entries_start_where_the_canonical_text_starts() {
    let rotated = region_loop("e-3,~e-5,e-3,e-4,e-1,e-2");

    assert_eq!(rotated.entries(), entries("e-1,e-2,e-3,~e-5,e-3,e-4"));
}

#[test]
fn entries_keep_traversal_order_rather_than_sorting() {
    let clockwise = region_loop("~e-1,~e-4,~e-3,~e-2");

    assert_eq!(clockwise.entries(), entries("~e-1,~e-4,~e-3,~e-2"));
}

#[test]
fn every_reference_carries_its_region_outline_key_and_fingerprint() {
    for region in regions_of(&plate_entities()) {
        let written = region.reference();

        assert_eq!(written.key, region.key);
        assert_eq!(written.fp, region.fingerprint());
        assert_eq!(written.outline, Some(region.outline()), "{}", region.key);
    }
}

#[test]
fn outline_leaves_out_the_holes() {
    let regions = regions_of(&plate_entities());

    assert_eq!(regions[0].key.to_string(), "e-1,e-2,e-3,e-4;~e-5");
    assert_eq!(regions[0].outline().to_string(), "e-1,e-2,e-3,e-4");
    assert_eq!(regions[1].outline().to_string(), "e-5");
}

#[test]
fn outline_survives_moving_geometry() {
    let mut moved = rect_entities();
    for (entity, (start, end)) in moved.iter_mut().zip([
        ([10.0, 5.0], [110.0, 5.0]),
        ([110.0, 5.0], [110.0, 65.0]),
        ([110.0, 65.0], [10.0, 65.0]),
        ([10.0, 65.0], [10.0, 5.0]),
    ]) {
        *entity = line(entity.id.as_str(), start, end);
    }

    assert_eq!(outlines(&moved), outlines(&rect_entities()));
}
