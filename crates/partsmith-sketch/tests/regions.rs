//! Acceptance tests for region keys, fingerprints and matching (spec section 6.12, tests 174 to 188), plus the
//! rules of steps 1, 3 and 4 of section 3.14 that those leave out.

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

/// A hand-built region with this key, area 100 and empty loops (tests 185 and 186, and the fingerprint matches).
fn hand_built_region(key_text: &str, centroid: [f64; 2]) -> Region {
    hand_built_region_of_area(key_text, 100.0, centroid)
}

/// A hand-built region with this key, area and centroid, and empty loops.
fn hand_built_region_of_area(key_text: &str, area: f64, centroid: [f64; 2]) -> Region {
    Region {
        key: key(key_text),
        outer: ProfileLoop { edges: Vec::new() },
        holes: Vec::new(),
        area,
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
        hand_built_region("e-1,e-2", [0.0, 0.0]),
        hand_built_region("e-1,e-2", [10.0, 0.0]),
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
    let regions = [
        hand_built_region("e-1,e-2", [0.0, 0.0]),
        hand_built_region("e-1,e-2", [0.0, 0.0]),
    ];

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
fn only_the_regions_tied_at_the_lowest_score_are_candidates() {
    let regions = [
        hand_built_region("e-1,e-2", [0.0, 0.0]),
        hand_built_region("e-1,e-2", [10.0, 0.0]),
        hand_built_region("e-1,e-2", [0.0, 0.0]),
    ];

    let found = match_region(
        &regions,
        &ids(&["e-1", "e-2"]),
        &reference("e-1,e-2", "c=0,0|a=100"),
    );

    assert_eq!(
        found,
        RegionMatch::Ambiguous {
            candidates: vec![vec![0], vec![2]]
        }
    );
}

#[test]
fn key_ties_with_a_zero_stored_area_score_plain_differences() {
    // Scores `norm(c - c0) + abs(A - A0)`: 0 + 5 for the first region, 3 + 1 for the second.
    let regions = [
        hand_built_region_of_area("e-1,e-2", 5.0, [0.0, 0.0]),
        hand_built_region_of_area("e-1,e-2", 1.0, [3.0, 0.0]),
    ];

    let found = match_region(
        &regions,
        &ids(&["e-1", "e-2"]),
        &reference("e-1,e-2", "c=0,0|a=0"),
    );

    assert_eq!(found, resolved(&[1], MatchMethod::Key, Vec::new()));
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

/// The match of `{key: e-1,e-2, fp: c=0,0|a=100}` among two hand-built regions of other entities, `e-11,e-12` and
/// `e-21,e-22`, with these centroids: neither keeps `e-1` or `e-2`, so only their fingerprints can match.
fn match_among_redrawn_regions(first: [f64; 2], second: [f64; 2]) -> RegionMatch {
    let regions = [
        hand_built_region("e-11,e-12", first),
        hand_built_region("e-21,e-22", second),
    ];
    match_region(
        &regions,
        &ids(&["e-11", "e-12", "e-21", "e-22"]),
        &reference("e-1,e-2", "c=0,0|a=100"),
    )
}

#[test]
fn fingerprint_match_takes_the_region_with_the_lowest_score() {
    // Both centroids lie within the fingerprint tolerance (0.01 mm for 100 mm^2).
    assert_eq!(
        match_among_redrawn_regions([0.008, 0.0], [0.0, 0.002]),
        resolved(&[1], MatchMethod::Fingerprint, Vec::new())
    );
}

#[test]
fn fingerprint_match_with_equal_scores_is_ambiguous() {
    assert_eq!(
        match_among_redrawn_regions([0.005, 0.0], [-0.005, 0.0]),
        RegionMatch::Ambiguous {
            candidates: vec![vec![0], vec![1]]
        }
    );
}

#[test]
fn redrawn_shape_within_a_tenth_of_a_percent_matches_by_fingerprint() {
    // Area 4002 (0.05 percent more) and centroid (40.02, 25), within 1e-3 * sqrt(4000) of (40, 25).
    let entities = rectangle(["e-11", "e-12", "e-13", "e-14"], 80.04, 50.0);

    let found = matched(&entities, &reference("e-1,e-2,e-3,e-4", "c=40,25|a=4000"));

    assert_eq!(found, resolved(&[0], MatchMethod::Fingerprint, Vec::new()));
}

#[test]
fn deleted_entities_are_listed_once_whatever_their_flags() {
    let found = match_region(&[], &ids(&["e-2"]), &reference("e-1,~e-1,e-2", "c=0,0|a=1"));

    assert_eq!(
        found,
        RegionMatch::Missing {
            loss: RegionLoss::Deleted {
                entities: ids(&["e-1"]),
            }
        }
    );
}

#[test]
fn redrawn_shape_in_another_place_is_missing_as_redrawn() {
    // The size of test 184 one millimeter to the right: the centroid is 1 mm off, beyond 1e-3 * sqrt(4000).
    let entities = vec![
        line("e-11", [1.0, 0.0], [81.0, 0.0]),
        line("e-12", [81.0, 0.0], [81.0, 50.0]),
        line("e-13", [81.0, 50.0], [1.0, 50.0]),
        line("e-14", [1.0, 50.0], [1.0, 0.0]),
    ];

    let found = matched(&entities, &reference("e-1,e-2,e-3,e-4", "c=40,25|a=4000"));

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
