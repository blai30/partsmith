//! Acceptance tests for region matching by boundary, coverage, successions and losses (spec section 6.18,
//! tests 221 to 236, and test 252 of section 6.21), plus one test per rule of sections 2.7 and 3.14 that those
//! leave out.

mod common;

use std::f64::consts::{FRAC_PI_2, PI};

use common::benchmark::entity_ids;
use common::matching::{
    key_texts, matched, reference_with_outline, region_ref_texts, regions_of, resolved, succession,
    without_outline,
};
use common::random::Rng;
use common::{
    arc, as_construction, assert_relative, circle, ids, line, plate_entities, rect_entities, slot,
};
use partsmith_sketch::{
    Id, MatchMethod, RegionLoss, RegionMatch, RegionRef, SketchEntity, build_profile, match_region,
};

/// `R0`: the reference test 220 serializes for `RECT`.
fn r0() -> RegionRef {
    reference_with_outline("e-1,e-2,e-3,e-4", "c=40,25|a=4000", "e-1,e-2,e-3,e-4")
}

/// `R0'`: `R0` without its outline, as written before file format version 2.
fn r0_without_outline() -> RegionRef {
    without_outline(&r0())
}

/// The lines of `RECT'` without `removed`, with `changed` replacing the lines of the same ids, then `added`.
fn rect_edit(
    removed: &[&str],
    changed: Vec<SketchEntity>,
    added: Vec<SketchEntity>,
) -> Vec<SketchEntity> {
    let mut entities: Vec<SketchEntity> = rect_entities()
        .into_iter()
        .filter(|entity| !removed.contains(&entity.id.as_str()))
        .map(|entity| {
            changed
                .iter()
                .find(|replacement| replacement.id == entity.id)
                .cloned()
                .unwrap_or(entity)
        })
        .collect();
    entities.extend(added);
    entities
}

/// The circle across the top line `e-3` of `RECT'` (corpus R04).
fn bite() -> SketchEntity {
    circle("e-5", [40.0, 50.0], 10.0)
}

/// The reference to the region of `RECT'` that `bite()` cuts into, as Keep these regions stores it after corpus
/// R04: its outline holds `e-3` twice.
fn bitten_region() -> RegionRef {
    let stored = regions_of(&rect_edit(&[], Vec::new(), vec![bite()]))[0].reference();
    assert_eq!(
        region_ref_texts(&stored),
        (
            "e-1,e-2,e-3,e-4,~e-5".to_owned(),
            Some("e-1,e-2,e-3,~e-5,e-3,e-4".to_owned())
        )
    );
    stored
}

/// `Missing { loss }`.
fn missing(loss: RegionLoss) -> RegionMatch {
    RegionMatch::Missing { loss }
}

/// Test 230: two overlapping rectangles, A from x 0 to 30 and B from x 20 to 70.
fn overlapping_rectangles() -> Vec<SketchEntity> {
    vec![
        line("e-1", [0.0, 0.0], [30.0, 0.0]),
        line("e-2", [30.0, 0.0], [30.0, 50.0]),
        line("e-3", [30.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-5", [20.0, 0.0], [70.0, 0.0]),
        line("e-6", [70.0, 0.0], [70.0, 50.0]),
        line("e-7", [70.0, 50.0], [20.0, 50.0]),
        line("e-8", [20.0, 50.0], [20.0, 0.0]),
    ]
}

/// The reference to rectangle A of test 230.
fn rectangle_a() -> RegionRef {
    reference_with_outline("e-1,e-2,e-3,e-4", "c=15,25|a=1500", "e-1,e-2,e-3,e-4")
}

/// The reference to rectangle B of test 230.
fn rectangle_b() -> RegionRef {
    reference_with_outline("e-5,e-6,e-7,e-8", "c=65,25|a=1500", "e-5,e-6,e-7,e-8")
}

/// Asserts the results of test 230 on these entities, passed to `match_region` in the order of `entity_ids`.
#[track_caller]
fn assert_overlapping_rectangles_match(entities: &[SketchEntity], entity_ids: &[Id]) {
    let regions = regions_of(entities);

    assert_eq!(
        key_texts(&regions),
        ["e-1,e-2,e-3,e-8", "e-1,e-3,e-4,~e-8", "~e-2,e-5,e-6,e-7"]
    );
    assert_eq!(
        match_region(&regions, entity_ids, &rectangle_a()),
        resolved(&[0, 1], MatchMethod::Coverage, Vec::new()),
        "rectangle A"
    );
    assert_eq!(
        match_region(&regions, entity_ids, &rectangle_b()),
        resolved(&[0, 2], MatchMethod::Coverage, Vec::new()),
        "rectangle B"
    );
}

#[test]
fn t221_corner_chamfer_line_keeps_the_region_without_successions() {
    for size in [0.5, 5.0, 10.0] {
        let entities = rect_edit(
            &[],
            vec![
                line("e-1", [0.0, 0.0], [80.0 - size, 0.0]),
                line("e-2", [80.0, size], [80.0, 50.0]),
            ],
            vec![line("e-6", [80.0 - size, 0.0], [80.0, size])],
        );
        let regions = regions_of(&entities);

        assert_eq!(key_texts(&regions), ["e-1,e-2,e-3,e-4,e-6"], "size {size}");
        assert_eq!(regions[0].outline().to_string(), "e-1,e-6,e-2,e-3,e-4");
        assert_relative(
            regions[0].area,
            4000.0 - size * size / 2.0,
            1e-9,
            "area with a chamfer line",
        );
        assert_eq!(
            matched(&entities, &r0()),
            resolved(&[0], MatchMethod::Boundary, Vec::new()),
            "size {size}"
        );
    }
}

#[test]
fn t222_line_redrawn_in_place_is_a_replacement() {
    let entities = rect_edit(
        &["e-2"],
        Vec::new(),
        vec![line("e-9", [80.0, 0.0], [80.0, 50.0])],
    );

    assert_eq!(
        matched(&entities, &r0()),
        resolved(
            &[0],
            MatchMethod::Boundary,
            vec![succession("e-2", &["e-9"])]
        )
    );
    assert_eq!(
        matched(&entities, &r0_without_outline()),
        resolved(&[0], MatchMethod::Boundary, Vec::new())
    );
}

#[test]
fn t222_two_lines_redrawn_in_place_are_two_replacements() {
    let entities = rect_edit(
        &["e-2", "e-4"],
        Vec::new(),
        vec![
            line("e-9", [80.0, 0.0], [80.0, 50.0]),
            line("e-10", [0.0, 50.0], [0.0, 0.0]),
        ],
    );

    assert_eq!(
        matched(&entities, &r0()),
        resolved(
            &[0],
            MatchMethod::Boundary,
            vec![succession("e-2", &["e-9"]), succession("e-4", &["e-10"])]
        )
    );
    assert_eq!(
        matched(&entities, &r0_without_outline()),
        resolved(&[0], MatchMethod::Boundary, Vec::new())
    );
}

#[test]
fn t223_line_redrawn_further_out_is_a_replacement() {
    let entities = rect_edit(
        &["e-2"],
        vec![
            line("e-1", [0.0, 0.0], [90.0, 0.0]),
            line("e-3", [90.0, 50.0], [0.0, 50.0]),
        ],
        vec![line("e-9", [90.0, 0.0], [90.0, 50.0])],
    );

    assert_eq!(
        matched(&entities, &r0()),
        resolved(
            &[0],
            MatchMethod::Boundary,
            vec![succession("e-2", &["e-9"])]
        )
    );
    assert_eq!(
        matched(&entities, &r0_without_outline()),
        resolved(&[0], MatchMethod::Boundary, Vec::new())
    );
}

#[test]
fn t224_side_line_split_in_two_is_an_extension() {
    let entities = rect_edit(
        &[],
        vec![line("e-2", [80.0, 0.0], [80.0, 25.0])],
        vec![line("e-6", [80.0, 25.0], [80.0, 50.0])],
    );
    let expected = resolved(
        &[0],
        MatchMethod::Boundary,
        vec![succession("e-2", &["e-2", "e-6"])],
    );

    assert_eq!(matched(&entities, &r0()), expected);
    assert_eq!(matched(&entities, &r0_without_outline()), expected);
}

#[test]
fn t225_side_line_replaced_by_a_notch() {
    let entities = rect_edit(
        &["e-3"],
        Vec::new(),
        vec![
            line("e-9", [80.0, 50.0], [50.0, 50.0]),
            line("e-10", [50.0, 50.0], [50.0, 40.0]),
            line("e-11", [50.0, 40.0], [30.0, 40.0]),
            line("e-12", [30.0, 40.0], [30.0, 50.0]),
            line("e-13", [30.0, 50.0], [0.0, 50.0]),
        ],
    );
    let regions = regions_of(&entities);
    assert_eq!(regions.len(), 1);
    assert_relative(regions[0].area, 3800.0, 1e-9, "area with the notch");

    assert_eq!(
        matched(&entities, &r0()),
        resolved(
            &[0],
            MatchMethod::Boundary,
            vec![succession("e-3", &["e-9", "e-10", "e-11", "e-12", "e-13"])]
        )
    );
    assert_eq!(
        matched(&entities, &r0_without_outline()),
        resolved(&[0], MatchMethod::Boundary, Vec::new())
    );
}

#[test]
fn t226_two_neighboring_lines_replaced_give_no_succession() {
    let entities = rect_edit(
        &["e-2", "e-3"],
        Vec::new(),
        vec![
            line("e-9", [80.0, 0.0], [80.0, 50.0]),
            line("e-10", [80.0, 50.0], [0.0, 50.0]),
        ],
    );
    let regions = regions_of(&entities);

    assert_eq!(key_texts(&regions), ["e-1,e-10,e-4,e-9"]);
    assert_eq!(regions[0].outline().to_string(), "e-1,e-9,e-10,e-4");
    assert_eq!(
        matched(&entities, &r0()),
        resolved(&[0], MatchMethod::Boundary, Vec::new())
    );
    assert_eq!(
        matched(&entities, &r0_without_outline()),
        resolved(&[0], MatchMethod::Boundary, Vec::new())
    );
}

#[test]
fn t227_arc_continued_on_its_circle_is_an_extension() {
    let stored = regions_of(&slot())[0].reference();
    assert_eq!(stored.key.to_string(), "e-1,e-2,e-3,e-4");
    assert_eq!(stored.fp.to_string(), "c=10,0|a=278.539816");
    assert_eq!(
        stored.outline.as_ref().map(ToString::to_string),
        Some("e-1,e-2,e-3,e-4".to_owned())
    );
    let mut entities = slot();
    entities[1] = arc("e-2", [20.0, 0.0], 5.0, -FRAC_PI_2, 0.0);
    entities.push(arc("e-9", [20.0, 0.0], 5.0, 0.0, FRAC_PI_2));

    assert_eq!(
        matched(&entities, &stored),
        resolved(
            &[0],
            MatchMethod::Boundary,
            vec![succession("e-2", &["e-2", "e-9"])]
        )
    );
}

#[test]
fn t228_circle_across_a_side_covers_with_the_inner_half_disc_only() {
    let entities = rect_edit(&[], Vec::new(), vec![bite()]);
    let regions = regions_of(&entities);

    assert_eq!(
        key_texts(&regions),
        ["e-1,e-2,e-3,e-4,~e-5", "e-3,e-5", "~e-3,e-5"]
    );
    assert_relative(regions[0].area, 4000.0 - 50.0 * PI, 1e-9, "big region");
    assert_relative(regions[1].area, 50.0 * PI, 1e-9, "lower half disc");
    assert!(
        regions[1].centroid[1] < 50.0,
        "the lower half disc lies inside the rectangle"
    );
    assert_eq!(
        matched(&entities, &r0()),
        resolved(&[0, 1], MatchMethod::Coverage, Vec::new())
    );
}

#[test]
fn t229_circle_inside_covers_with_ring_and_disc() {
    let entities = rect_edit(&[], Vec::new(), vec![circle("e-5", [40.0, 25.0], 10.0)]);

    assert_eq!(
        key_texts(&regions_of(&entities)),
        ["e-1,e-2,e-3,e-4;~e-5", "e-5"]
    );
    assert_eq!(
        matched(&entities, &r0()),
        resolved(&[0, 1], MatchMethod::Coverage, Vec::new())
    );
}

#[test]
fn t229_stored_hole_stays_a_hole() {
    let stored = reference_with_outline(
        "e-1,e-2,e-3,e-4;~e-5",
        "c=40,25|a=3921.460184",
        "e-1,e-2,e-3,e-4",
    );
    let mut entities: Vec<SketchEntity> = plate_entities()
        .into_iter()
        .filter(|entity| entity.id.as_str() != "e-2")
        .collect();
    entities.push(line("e-9", [80.0, 0.0], [80.0, 50.0]));

    assert_eq!(
        key_texts(&regions_of(&entities)),
        ["e-1,e-3,e-4,e-9;~e-5", "e-5"]
    );
    assert_eq!(
        matched(&entities, &stored),
        resolved(
            &[0],
            MatchMethod::Boundary,
            vec![succession("e-2", &["e-9"])]
        )
    );
}

#[test]
fn t230_overlapping_rectangles_each_cover_their_part_and_the_overlap() {
    let entities = overlapping_rectangles();
    let regions = regions_of(&entities);
    assert_relative(regions[0].area, 500.0, 1e-9, "overlap");
    assert_relative(regions[1].area, 1000.0, 1e-9, "A alone");
    assert_relative(regions[2].area, 2000.0, 1e-9, "B alone");

    assert_overlapping_rectangles_match(&entities, &entity_ids(&entities));
}

#[test]
fn t231_second_rectangle_on_an_extended_line_is_ambiguous() {
    let entities = rect_edit(
        &["e-2"],
        vec![line("e-1", [0.0, 0.0], [200.0, 0.0])],
        vec![
            line("e-9", [80.0, 0.0], [80.0, 50.0]),
            line("e-5", [200.0, 0.0], [200.0, 20.0]),
            line("e-6", [200.0, 20.0], [150.0, 20.0]),
            line("e-7", [150.0, 20.0], [150.0, 0.0]),
        ],
    );
    let profile = build_profile(&entities);

    assert_eq!(
        key_texts(&profile.regions),
        ["e-1,e-3,e-4,e-9", "e-1,e-5,e-6,e-7"]
    );
    assert!(
        profile.findings.is_empty(),
        "findings: {:?}",
        profile.findings
    );
    assert_eq!(
        matched(&entities, &r0()),
        RegionMatch::Ambiguous {
            candidates: vec![vec![0], vec![1]]
        }
    );
}

#[test]
fn t232_deleted_bevel_with_a_chamfer_elsewhere_gives_no_succession() {
    let stored = reference_with_outline(
        "e-1,e-2,e-3,e-4,e-5",
        "c=39.535865,24.725738|a=3950",
        "e-1,e-2,e-5,e-3,e-4",
    );
    let entities = vec![
        line("e-1", [5.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 50.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 5.0]),
        line("e-6", [0.0, 5.0], [5.0, 0.0]),
    ];

    assert_eq!(
        matched(&entities, &stored),
        resolved(&[0], MatchMethod::Boundary, Vec::new())
    );
    assert_eq!(
        matched(&entities, &without_outline(&stored)),
        resolved(&[0], MatchMethod::Boundary, Vec::new())
    );
}

#[test]
fn t233_fully_redrawn_outline_matches_by_fingerprint() {
    let entities = vec![
        line("e-11", [0.0, 0.0], [80.0, 0.0]),
        line("e-12", [80.0, 0.0], [80.0, 50.0]),
        line("e-13", [80.0, 50.0], [0.0, 50.0]),
        line("e-14", [0.0, 50.0], [0.0, 0.0]),
    ];

    assert_eq!(
        matched(&entities, &r0()),
        resolved(&[0], MatchMethod::Fingerprint, Vec::new())
    );
}

#[test]
fn t233_redrawn_outline_one_percent_larger_is_missing_as_redrawn() {
    let entities = vec![
        line("e-11", [0.0, 0.0], [80.8, 0.0]),
        line("e-12", [80.8, 0.0], [80.8, 50.0]),
        line("e-13", [80.8, 50.0], [0.0, 50.0]),
        line("e-14", [0.0, 50.0], [0.0, 0.0]),
    ];
    let regions = regions_of(&entities);
    assert_relative(regions[0].area, 4040.0, 1e-9, "area of the larger outline");

    assert_eq!(matched(&entities, &r0()), missing(RegionLoss::Redrawn));
}

#[test]
fn t234_deleted_side_line_opens_the_outline() {
    let entities = rect_edit(&["e-2"], Vec::new(), Vec::new());
    assert!(regions_of(&entities).is_empty(), "no region is left");
    let expected = missing(RegionLoss::Deleted {
        entities: ids(&["e-2"]),
    });

    assert_eq!(matched(&entities, &r0()), expected);
    assert_eq!(matched(&entities, &r0_without_outline()), expected);
}

#[test]
fn t235_stored_entities_that_no_longer_close_are_open() {
    let gap = rect_edit(
        &[],
        vec![line("e-2", [80.0, 1.0], [80.0, 50.0])],
        Vec::new(),
    );

    assert_eq!(matched(&gap, &r0()), missing(RegionLoss::Open));
}

#[test]
fn t235_stored_side_marked_construction_is_open() {
    let construction = rect_edit(
        &[],
        vec![as_construction(line("e-2", [80.0, 0.0], [80.0, 50.0]))],
        Vec::new(),
    );

    assert_eq!(matched(&construction, &r0()), missing(RegionLoss::Open));
}

#[test]
fn t236_order_of_entities_does_not_matter() {
    let mut random = Rng::new(236);
    for _ in 0..50 {
        let mut entities = overlapping_rectangles();
        random.shuffle(&mut entities);
        let mut reversed_ids = entity_ids(&entities);
        reversed_ids.reverse();

        assert_overlapping_rectangles_match(&entities, &reversed_ids);
    }
}

#[test]
fn t252_coverage_never_depends_on_the_stored_area() {
    let mut entities = overlapping_rectangles();
    entities[0] = line("e-1", [0.0, 0.0], [35.0, 0.0]);
    entities[1] = line("e-2", [35.0, 0.0], [35.0, 50.0]);
    entities[2] = line("e-3", [35.0, 50.0], [0.0, 50.0]);
    let regions = regions_of(&entities);

    assert_relative(regions[0].area, 750.0, 1e-9, "overlap");
    assert_relative(regions[1].area, 1000.0, 1e-9, "A alone");
    assert_relative(regions[2].area, 1750.0, 1e-9, "B alone");
    assert_overlapping_rectangles_match(&entities, &entity_ids(&entities));
}

#[test]
fn t252_pieces_that_reach_beyond_the_stored_boundary_are_grown() {
    let entities = rect_edit(
        &["e-2"],
        vec![
            line("e-1", [0.0, 0.0], [100.0, 0.0]),
            line("e-3", [100.0, 50.0], [0.0, 50.0]),
        ],
        vec![
            line("e-9", [100.0, 0.0], [100.0, 50.0]),
            line("e-10", [80.0, 0.0], [80.0, 50.0]),
        ],
    );
    let regions = regions_of(&entities);

    assert_eq!(
        key_texts(&regions),
        ["e-1,e-10,e-3,e-4", "e-1,~e-10,e-3,e-9"]
    );
    assert_relative(regions[0].area, 4000.0, 1e-9, "left piece");
    assert_relative(regions[1].area, 1000.0, 1e-9, "right piece");
    assert_eq!(
        matched(&entities, &r0()),
        resolved(&[0, 1], MatchMethod::Grown, Vec::new())
    );
}

#[test]
fn reference_written_after_the_edit_matches_by_key_without_successions() {
    let entities = rect_edit(
        &["e-2"],
        Vec::new(),
        vec![line("e-9", [80.0, 0.0], [80.0, 50.0])],
    );
    let rewritten = regions_of(&entities)[0].reference();

    assert_eq!(
        matched(&entities, &rewritten),
        resolved(&[0], MatchMethod::Key, Vec::new())
    );
}

#[test]
fn deleted_entities_are_sorted_by_id() {
    let mut entities = rect_edit(&["e-2", "e-3"], Vec::new(), Vec::new());
    entities.reverse();

    assert_eq!(
        matched(&entities, &r0()),
        missing(RegionLoss::Deleted {
            entities: ids(&["e-2", "e-3"]),
        })
    );
}

#[test]
fn moving_the_geometry_keeps_the_key_match() {
    let entities = rect_edit(
        &[],
        vec![
            line("e-1", [0.0, 0.0], [120.0, 0.0]),
            line("e-2", [120.0, 0.0], [120.0, 50.0]),
            line("e-3", [120.0, 50.0], [0.0, 50.0]),
        ],
        Vec::new(),
    );

    assert_eq!(
        matched(&entities, &r0()),
        resolved(&[0], MatchMethod::Key, Vec::new())
    );
}

#[test]
fn successions_are_sorted_by_entity_whichever_rule_found_them() {
    // `e-4` is redrawn as `e-9` (a replacement) and `e-2` is continued by `e-6` (an extension).
    let entities = rect_edit(
        &["e-4"],
        vec![line("e-2", [80.0, 0.0], [80.0, 25.0])],
        vec![
            line("e-6", [80.0, 25.0], [80.0, 50.0]),
            line("e-9", [0.0, 50.0], [0.0, 0.0]),
        ],
    );

    assert_eq!(
        matched(&entities, &r0()),
        resolved(
            &[0],
            MatchMethod::Boundary,
            vec![
                succession("e-2", &["e-2", "e-6"]),
                succession("e-4", &["e-9"])
            ]
        )
    );
    assert_eq!(
        matched(&entities, &r0_without_outline()),
        resolved(
            &[0],
            MatchMethod::Boundary,
            vec![succession("e-2", &["e-2", "e-6"])]
        )
    );
}

#[test]
fn no_replacement_is_inferred_next_to_a_line_that_occurs_twice() {
    // The edit deletes the bite and redraws `e-2` as `e-9`. The first `e-3` of the stored outline would give the
    // old run `[e-2]` between `e-1` and `e-3`, and so the replacement `e-2 -> [e-9]`.
    let entities = rect_edit(
        &["e-2"],
        Vec::new(),
        vec![line("e-9", [80.0, 0.0], [80.0, 50.0])],
    );
    assert_eq!(key_texts(&regions_of(&entities)), ["e-1,e-3,e-4,e-9"]);

    assert_eq!(
        matched(&entities, &bitten_region()),
        resolved(&[0], MatchMethod::Boundary, Vec::new())
    );
}

#[test]
fn region_with_a_bite_in_a_stored_line_is_ambiguous_after_its_key_changes() {
    // Corpus R01's 5 mm corner chamfer line, drawn after Keep these regions stored the bitten region.
    let entities = rect_edit(
        &[],
        vec![
            line("e-1", [0.0, 0.0], [75.0, 0.0]),
            line("e-2", [80.0, 5.0], [80.0, 50.0]),
        ],
        vec![bite(), line("e-6", [75.0, 0.0], [80.0, 5.0])],
    );
    assert_eq!(
        key_texts(&regions_of(&entities)),
        ["e-1,e-2,e-3,e-4,~e-5,e-6", "e-3,e-5", "~e-3,e-5"]
    );

    // The lower half disc keeps `e-3` on the same side, so it seeds a second component: the circle it shares
    // with the region is stored, not added (spec open issue 17, pending the review of ADR 0022).
    assert_eq!(
        matched(&entities, &bitten_region()),
        RegionMatch::Ambiguous {
            candidates: vec![vec![0], vec![1]]
        }
    );
}

#[test]
fn arc_on_the_stored_circle_keeps_the_stored_boundary() {
    let stored = regions_of(&[circle("e-5", [0.0, 0.0], 10.0)])[0].reference();
    // Where the new arc `e-1` lies on the circle, the piece of `e-1`, the smaller id, is kept (section 3.13.2).
    let entities = vec![
        arc("e-1", [0.0, 0.0], 10.0, 0.0, FRAC_PI_2),
        line("e-2", [-10.0, 0.0], [10.0, 0.0]),
        circle("e-5", [0.0, 0.0], 10.0),
    ];

    assert_eq!(
        key_texts(&regions_of(&entities)),
        ["e-1,e-2,e-5", "~e-2,e-5"]
    );
    assert_eq!(
        matched(&entities, &stored),
        resolved(&[0, 1], MatchMethod::Coverage, Vec::new())
    );
}

#[test]
fn disc_split_by_a_line_is_covered_by_both_halves() {
    let stored = regions_of(&[circle("e-1", [0.0, 0.0], 10.0)])[0].reference();
    let entities = vec![
        circle("e-1", [0.0, 0.0], 10.0),
        line("e-2", [-10.0, 0.0], [10.0, 0.0]),
    ];

    assert_eq!(key_texts(&regions_of(&entities)), ["e-1,e-2", "e-1,~e-2"]);
    assert_eq!(
        matched(&entities, &stored),
        resolved(&[0, 1], MatchMethod::Coverage, Vec::new())
    );
}

#[test]
fn loss_names_only_entities_of_the_stored_outer_boundary() {
    let stored = regions_of(&plate_entities())[0].reference();
    assert_eq!(stored.key.to_string(), "e-1,e-2,e-3,e-4;~e-5");
    let entities: Vec<SketchEntity> = plate_entities()
        .into_iter()
        .filter(|entity| !["e-2", "e-5"].contains(&entity.id.as_str()))
        .collect();
    assert!(regions_of(&entities).is_empty(), "no region is left");

    assert_eq!(
        matched(&entities, &stored),
        missing(RegionLoss::Deleted {
            entities: ids(&["e-2"]),
        })
    );
}

#[test]
fn region_beyond_a_stored_line_is_not_a_seed() {
    // The rectangle no longer closes, and the only region lies on the other side of `e-2`.
    let entities = rect_edit(
        &[],
        vec![line("e-2", [80.0, 1.0], [80.0, 50.0])],
        vec![
            line("e-5", [80.0, 10.0], [100.0, 10.0]),
            line("e-6", [100.0, 10.0], [100.0, 40.0]),
            line("e-7", [100.0, 40.0], [80.0, 40.0]),
        ],
    );
    assert_eq!(key_texts(&regions_of(&entities)), ["~e-2,e-5,e-6,e-7"]);

    assert_eq!(matched(&entities, &r0()), missing(RegionLoss::Open));
    assert_eq!(
        matched(&entities, &r0_without_outline()),
        missing(RegionLoss::Open)
    );
}

#[test]
fn hole_that_a_stored_entity_bounds_is_never_filled() {
    // The new circle `e-7` overlaps the stored hole `e-5`, so the ring's hole now runs along both circles.
    let stored = regions_of(&plate_entities())[0].reference();
    assert_eq!(stored.key.to_string(), "e-1,e-2,e-3,e-4;~e-5");
    let mut entities = plate_entities();
    entities.push(circle("e-7", [45.0, 25.0], 5.0));
    assert_eq!(
        key_texts(&regions_of(&entities)),
        [
            "e-1,e-2,e-3,e-4;~e-5,~e-7",
            "e-5,e-7",
            "e-5,~e-7",
            "~e-5,e-7"
        ]
    );

    assert_eq!(
        matched(&entities, &stored),
        resolved(&[0], MatchMethod::Boundary, Vec::new())
    );
}

#[test]
fn region_along_a_hole_of_a_filled_region_fills_too() {
    // The ring inside the new circle `e-7` has a hole along `e-5` and the new circle `e-8`; the crescent of `e-8`
    // outside `e-5` runs along that hole, so it is drawn inside the stored region too.
    let stored = regions_of(&plate_entities())[0].reference();
    let mut entities = plate_entities();
    entities.push(circle("e-7", [40.0, 25.0], 15.0));
    entities.push(circle("e-8", [45.0, 25.0], 5.0));
    let regions = regions_of(&entities);
    assert_eq!(
        key_texts(&regions),
        [
            "e-1,e-2,e-3,e-4;~e-7",
            "e-5,e-8",
            "e-5,~e-8",
            "e-7;~e-5,~e-8",
            "~e-5,e-8"
        ]
    );
    let stored_area = 4000.0 - 25.0 * PI;
    assert_relative(
        regions[0].area + regions[3].area + regions[4].area,
        stored_area,
        1e-9,
        "the plate, the ring and the crescent",
    );

    assert_eq!(
        matched(&entities, &stored),
        resolved(&[0, 3, 4], MatchMethod::Coverage, Vec::new())
    );
}

#[test]
fn redrawn_stored_hole_is_filled_until_the_decision() {
    // Open issue 20 of the spec: the new circle `e-9` takes the place of the deleted hole `e-5`, but its entities
    // are all added, so the plate is matched with the disc and the hole is lost.
    let stored = regions_of(&plate_entities())[0].reference();
    let mut entities: Vec<SketchEntity> = plate_entities()
        .into_iter()
        .filter(|entity| entity.id.as_str() != "e-5")
        .collect();
    entities.push(circle("e-9", [40.0, 25.0], 5.0));
    assert_eq!(
        key_texts(&regions_of(&entities)),
        ["e-1,e-2,e-3,e-4;~e-9", "e-9"]
    );

    assert_eq!(
        matched(&entities, &stored),
        resolved(&[0, 1], MatchMethod::Coverage, Vec::new())
    );
}

#[test]
fn entities_listed_twice_change_nothing() {
    let redrawn = rect_edit(
        &["e-2"],
        Vec::new(),
        vec![line("e-9", [80.0, 0.0], [80.0, 50.0])],
    );
    let opened = rect_edit(&["e-2"], Vec::new(), Vec::new());

    for entities in [redrawn, opened] {
        let mut twice = entity_ids(&entities);
        twice.extend(entity_ids(&entities));

        assert_eq!(
            match_region(&regions_of(&entities), &twice, &r0()),
            matched(&entities, &r0()),
            "{:?}",
            entity_ids(&entities)
        );
    }
}

#[test]
fn match_results_write_every_field() {
    let cases = [
        (
            resolved(&[0, 1], MatchMethod::Coverage, Vec::new()),
            r#"{"kind":"resolved","indices":[0,1],"method":"coverage","successions":[]}"#,
        ),
        (
            resolved(
                &[0],
                MatchMethod::Boundary,
                vec![succession("e-2", &["e-9"])],
            ),
            r#"{"kind":"resolved","indices":[0],"method":"boundary","successions":[{"entity":"e-2","now":["e-9"]}]}"#,
        ),
        (
            resolved(&[2], MatchMethod::Grown, Vec::new()),
            r#"{"kind":"resolved","indices":[2],"method":"grown","successions":[]}"#,
        ),
        (
            RegionMatch::Ambiguous {
                candidates: vec![vec![0], vec![1, 2]],
            },
            r#"{"kind":"ambiguous","candidates":[[0],[1,2]]}"#,
        ),
        (
            missing(RegionLoss::Deleted {
                entities: ids(&["e-2"]),
            }),
            r#"{"kind":"missing","loss":{"kind":"deleted","entities":["e-2"]}}"#,
        ),
        (
            missing(RegionLoss::Redrawn),
            r#"{"kind":"missing","loss":{"kind":"redrawn"}}"#,
        ),
        (
            missing(RegionLoss::Open),
            r#"{"kind":"missing","loss":{"kind":"open"}}"#,
        ),
    ];

    for (result, expected) in cases {
        assert_eq!(
            serde_json::to_string(&result).expect("the match result serializes"),
            expected
        );
    }
}

#[test]
fn match_results_read_the_json_examples_of_section_2_7() {
    let cases = [
        (
            r#"{"kind": "resolved", "indices": [0], "method": "boundary", "successions": [{"entity": "e-2", "now": ["e-9"]}]}"#,
            resolved(
                &[0],
                MatchMethod::Boundary,
                vec![succession("e-2", &["e-9"])],
            ),
        ),
        (
            r#"{"kind": "resolved", "indices": [0, 1], "method": "coverage", "successions": []}"#,
            resolved(&[0, 1], MatchMethod::Coverage, Vec::new()),
        ),
        (
            r#"{"kind": "resolved", "indices": [0, 1], "method": "grown", "successions": []}"#,
            resolved(&[0, 1], MatchMethod::Grown, Vec::new()),
        ),
        (
            r#"{"kind": "ambiguous", "candidates": [[0], [1]]}"#,
            RegionMatch::Ambiguous {
                candidates: vec![vec![0], vec![1]],
            },
        ),
        (
            r#"{"kind": "missing", "loss": {"kind": "deleted", "entities": ["e-2"]}}"#,
            missing(RegionLoss::Deleted {
                entities: ids(&["e-2"]),
            }),
        ),
        (
            r#"{"kind": "missing", "loss": {"kind": "redrawn"}}"#,
            missing(RegionLoss::Redrawn),
        ),
    ];

    for (text, expected) in cases {
        let read: RegionMatch = serde_json::from_str(text).expect("the example deserializes");

        assert_eq!(read, expected, "{text}");
        assert_eq!(
            serde_json::to_value(&read).expect("the match result serializes"),
            serde_json::from_str::<serde_json::Value>(text).expect("the example is JSON"),
            "{text}"
        );
    }
}

#[test]
fn match_methods_are_written_in_snake_case() {
    for (method, name) in [
        (MatchMethod::Key, "key"),
        (MatchMethod::Boundary, "boundary"),
        (MatchMethod::Coverage, "coverage"),
        (MatchMethod::Grown, "grown"),
        (MatchMethod::Fingerprint, "fingerprint"),
    ] {
        assert_eq!(
            serde_json::to_value(method).expect("the method serializes"),
            serde_json::Value::from(name)
        );
    }
}
