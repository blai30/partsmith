//! Acceptance tests for region keys, fingerprints and matching (spec section 6.12, tests 174 to 188).

mod common;

use std::f64::consts::PI;

use common::benchmark::entity_ids;
use common::matching::{fingerprint, key, matched, reference, regions_of, resolved};
use common::{
    crossing_rectangles, id, ids, line, nested_circles, overlapping_circles, plate_entities,
    rect_entities,
};
use partsmith_sketch::{
    KeyEdge, MatchMethod, ProfileLoop, Region, RegionFingerprint, RegionKey, RegionLoss,
    RegionMatch, RegionRef, SketchEntity, SketchError, match_region,
};

fn entry(entity: &str, reversed: bool) -> KeyEdge {
    KeyEdge {
        entity: id(entity),
        reversed,
    }
}

/// A rectangle with the given corners and ids, drawn counterclockwise from its lower-left corner.
fn rectangle(ids: [&str; 4], width: f64, height: f64) -> Vec<SketchEntity> {
    vec![
        line(ids[0], [0.0, 0.0], [width, 0.0]),
        line(ids[1], [width, 0.0], [width, height]),
        line(ids[2], [width, height], [0.0, height]),
        line(ids[3], [0.0, height], [0.0, 0.0]),
    ]
}

/// A hand-built region with key `e-1,e-2`, area 100 and empty loops (tests 185 and 186).
fn hand_built_region(centroid: [f64; 2]) -> Region {
    Region {
        key: key("e-1,e-2"),
        outer: ProfileLoop { edges: Vec::new() },
        holes: Vec::new(),
        area: 100.0,
        centroid,
    }
}

#[test]
fn t174_new_canonicalizes_entries_and_holes() {
    let region_key = RegionKey::new(
        vec![
            entry("e-3", false),
            entry("e-1", true),
            entry("e-1", false),
            entry("e-3", false),
        ],
        vec![vec![entry("e-9", true)], vec![entry("e-5", true)]],
    );

    assert_eq!(region_key.to_string(), "e-1,~e-1,e-3;~e-5;~e-9");
}

#[test]
fn t175_canonical_text_parses_and_round_trips() {
    let region_key = key("e-1,e-2;~e-5");

    assert_eq!(
        region_key.outer(),
        [entry("e-1", false), entry("e-2", false)]
    );
    assert_eq!(region_key.holes(), [vec![entry("e-5", true)]]);
    assert_eq!(region_key.to_string(), "e-1,e-2;~e-5");
}

#[test]
fn t176_non_canonical_key_text_fails() {
    let invalid = [
        "",
        "e-1,,e-2",
        "e-2,e-1",
        "e-1,e-1",
        "~~e-1",
        "e-1;",
        ";e-1",
        "e-1;~e-6;~e-5",
        "e 1",
        "e-1,~",
    ];

    for text in invalid {
        assert_eq!(
            text.parse::<RegionKey>(),
            Err(SketchError::InvalidRegionKey {
                text: text.to_owned()
            }),
            "{text:?}"
        );
    }
}

#[test]
fn t177_fingerprint_text_rounds_to_six_decimals() {
    let cases = [
        (4000.0, [40.0, 25.0], "c=40,25|a=4000"),
        (
            PI,
            [1.0 / 3.0, -2.0 / 3.0],
            "c=0.333333,-0.666667|a=3.141593",
        ),
        (0.5, [-0.0, 1e-7], "c=0,0|a=0.5"),
    ];

    for (area, centroid, expected) in cases {
        let region_fingerprint = RegionFingerprint { area, centroid };
        assert_eq!(region_fingerprint.to_string(), expected);
    }
}

#[test]
fn t178_fingerprint_text_parses() {
    assert_eq!(
        fingerprint("c=40,25|a=4000"),
        RegionFingerprint {
            area: 4000.0,
            centroid: [40.0, 25.0],
        }
    );
}

#[test]
fn t178_invalid_fingerprint_text_fails() {
    for text in [
        "c=1,2",
        "a=1|c=1,2",
        "c=x,2|a=1",
        "c=1,2|a=-1",
        "c=1,2|a=inf",
    ] {
        assert_eq!(
            text.parse::<RegionFingerprint>(),
            Err(SketchError::InvalidFingerprint {
                text: text.to_owned()
            }),
            "{text:?}"
        );
    }
}

#[test]
fn t179_region_ref_without_outline_serializes_as_two_strings() {
    let region_ref: RegionRef =
        serde_json::from_str(r#"{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}"#)
            .expect("the region reference deserializes");

    assert_eq!(region_ref.outline, None);
    assert_eq!(
        serde_json::to_string(&region_ref).expect("the region reference serializes"),
        r#"{"key":"e-1,e-2,e-3,e-4","fp":"c=40,25|a=4000"}"#
    );
}

#[test]
fn t179_region_ref_with_a_non_canonical_key_fails() {
    let result = serde_json::from_str::<RegionRef>(r#"{"key": "e-2,e-1", "fp": "c=40,25|a=4000"}"#);

    assert!(
        result.is_err(),
        "a non-canonical key must not deserialize, got {result:?}"
    );
}

#[test]
fn t180_key_match() {
    let found = matched(
        &rect_entities(),
        &reference("e-1,e-2,e-3,e-4", "c=40,25|a=4000"),
    );

    assert_eq!(found, resolved(&[0], MatchMethod::Key, Vec::new()));
}

#[test]
fn t181_orientation_markers_decide_between_lunes() {
    let found = matched(&overlapping_circles(), &reference("e-1,~e-2", "c=0,0|a=1"));

    assert_eq!(found, resolved(&[1], MatchMethod::Key, Vec::new()));
}

#[test]
fn t182_added_hole_keeps_covering_the_same_area() {
    let found = matched(
        &plate_entities(),
        &reference("e-1,e-2,e-3,e-4", "c=40,25|a=4000"),
    );

    assert_eq!(found, resolved(&[0, 1], MatchMethod::Coverage, Vec::new()));
}

#[test]
fn t183_removed_hole_matches_by_boundary() {
    let found = matched(
        &rect_entities(),
        &reference("e-1,e-2,e-3,e-4;~e-5", "c=40,25|a=3921.460184"),
    );

    assert_eq!(found, resolved(&[0], MatchMethod::Boundary, Vec::new()));
}

#[test]
fn t184_redrawn_shape_matches_by_fingerprint() {
    let entities = rectangle(["e-11", "e-12", "e-13", "e-14"], 80.0, 50.0);
    assert_eq!(
        entity_ids(&entities),
        ids(&["e-11", "e-12", "e-13", "e-14"])
    );

    let found = matched(&entities, &reference("e-1,e-2,e-3,e-4", "c=40,25|a=4000"));

    assert_eq!(found, resolved(&[0], MatchMethod::Fingerprint, Vec::new()));
}

#[test]
fn t184_redrawn_shape_of_another_size_is_missing_as_redrawn() {
    let entities = rectangle(["e-11", "e-12", "e-13", "e-14"], 88.0, 50.0);

    let found = matched(&entities, &reference("e-1,e-2,e-3,e-4", "c=40,25|a=4000"));

    assert_eq!(
        found,
        RegionMatch::Missing {
            loss: RegionLoss::Redrawn
        }
    );
}

#[test]
fn t185_score_breaks_key_ties() {
    let regions = [
        hand_built_region([0.0, 0.0]),
        hand_built_region([10.0, 0.0]),
    ];

    let found = match_region(
        &regions,
        &ids(&["e-1", "e-2"]),
        &reference("e-1,e-2", "c=9,0|a=100"),
    );

    assert_eq!(found, resolved(&[1], MatchMethod::Key, Vec::new()));
}

#[test]
fn t186_equal_scores_are_ambiguous() {
    let regions = [hand_built_region([0.0, 0.0]), hand_built_region([0.0, 0.0])];

    let found = match_region(
        &regions,
        &ids(&["e-1", "e-2"]),
        &reference("e-1,e-2", "c=9,0|a=100"),
    );

    assert_eq!(
        found,
        RegionMatch::Ambiguous {
            candidates: vec![vec![0], vec![1]]
        }
    );
}

#[test]
fn t187_nothing_similar_is_missing_as_redrawn() {
    let found = matched(&rect_entities(), &reference("e-77", "c=1000,1000|a=1"));

    assert_eq!(
        found,
        RegionMatch::Missing {
            loss: RegionLoss::Redrawn
        }
    );
}

#[test]
fn t188_every_region_finds_itself_by_key() {
    for (name, entities) in [
        ("test 150", plate_entities()),
        ("test 157", overlapping_circles()),
        ("test 161", crossing_rectangles()),
        ("test 163", nested_circles()),
    ] {
        let regions = regions_of(&entities);
        let entity_ids = entity_ids(&entities);
        assert!(!regions.is_empty(), "{name} has regions");
        for (index, region) in regions.iter().enumerate() {
            assert_eq!(
                match_region(&regions, &entity_ids, &region.reference()),
                resolved(&[index], MatchMethod::Key, Vec::new()),
                "region {index} ({}) of {name}",
                region.key
            );
        }
    }
}
