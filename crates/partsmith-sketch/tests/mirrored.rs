//! Acceptance tests for the mirrored drawing of a sketch (spec section 3.17; section 6.21, tests 248 to 250).

mod common;

use std::f64::consts::{FRAC_PI_2, FRAC_PI_6};

use common::benchmark::entity_ids;
use common::matching::{
    key_texts, region_ref, region_ref_texts, region_ref_with_outline, regions_of, resolved,
};
use common::{
    Sketch, TOLERANCE, arc, assert_bitwise, assert_close, assert_point_close, circle, coincident,
    construction_line, equal, fix, line, measured, plate, plate_entities, point_at_angle,
    point_ref, radius, rect, rect_entities, slot, tangent, vertical_distance_line,
    vertical_distance_points,
};
use partsmith_sketch::{
    ConstraintKind, Geometry, MatchMethod, RegionFingerprint, SketchConstraint, SketchEntity,
    SketchParams, SketchStatus, SolveResult, match_region,
};

/// `SLOT` of spec section 6.21: the slot of test 166 with its constraints and values.
fn slot_sketch() -> Sketch {
    Sketch::new(slot())
        .with("c-1", coincident("e-1.end", "e-2.start"))
        .with("c-2", coincident("e-2.end", "e-3.start"))
        .with("c-3", coincident("e-3.end", "e-4.start"))
        .with("c-4", coincident("e-4.end", "e-1.start"))
        .with("c-5", tangent("e-1", "e-2"))
        .with("c-6", tangent("e-3", "e-2"))
        .with("c-7", tangent("e-3", "e-4"))
        .with("c-8", tangent("e-1", "e-4"))
        .with("c-9", fix("e-4.center"))
        .with_value("c-10", radius("e-4", "5 mm"), 5.0)
        .with("c-11", equal("e-2", "e-4"))
        .with_value("c-12", common::angle("x_axis", "e-1", "30 deg"), FRAC_PI_6)
        .with_value(
            "c-13",
            vertical_distance_points("e-4.center", "e-2.center", "10 mm"),
            10.0,
        )
}

/// The same sketch with its params mirrored and the same values.
fn mirrored_sketch(sketch: &Sketch) -> Sketch {
    Sketch {
        params: sketch.params.mirrored(),
        values: sketch.values.clone(),
    }
}

/// The constraints with the one of id `constraint_id` given a new kind.
fn with_kind(
    constraints: &[SketchConstraint],
    constraint_id: &str,
    kind: ConstraintKind,
) -> Vec<SketchConstraint> {
    let mut changed = constraints.to_vec();
    let target = changed
        .iter_mut()
        .find(|constraint| constraint.id.as_str() == constraint_id)
        .unwrap_or_else(|| panic!("{constraint_id} is a constraint of the sketch"));
    target.kind = kind;
    changed
}

/// The point mirrored across the sketch's x axis.
fn mirror(point: [f64; 2]) -> [f64; 2] {
    [point[0], -point[1]]
}

/// Start and end points of an arc's geometry.
fn arc_ends(center: [f64; 2], radius: f64, start_angle: f64, end_angle: f64) -> [[f64; 2]; 2] {
    [
        point_at_angle(center, radius, start_angle),
        point_at_angle(center, radius, end_angle),
    ]
}

/// Asserts that `mirrored` is `original` mirrored across the x axis within `TOLERANCE`: line ends, centers and
/// arc end points, with an arc's start the mirror of the original's end.
#[track_caller]
fn assert_mirror_of(mirrored: &[SketchEntity], original: &[SketchEntity]) {
    assert_eq!(
        entity_ids(mirrored),
        entity_ids(original),
        "the same entities in the same order"
    );
    for (mirrored_entity, original_entity) in mirrored.iter().zip(original) {
        let name = mirrored_entity.id.as_str();
        match (&mirrored_entity.geometry, &original_entity.geometry) {
            (
                Geometry::Point { position },
                Geometry::Point {
                    position: original_position,
                },
            ) => assert_point_close(*position, mirror(*original_position), TOLERANCE, name),
            (
                Geometry::Line { start, end },
                Geometry::Line {
                    start: original_start,
                    end: original_end,
                },
            ) => {
                assert_point_close(*start, mirror(*original_start), TOLERANCE, name);
                assert_point_close(*end, mirror(*original_end), TOLERANCE, name);
            }
            (
                Geometry::Circle { center, radius },
                Geometry::Circle {
                    center: original_center,
                    radius: original_radius,
                },
            ) => {
                assert_point_close(*center, mirror(*original_center), TOLERANCE, name);
                assert_close(*radius, *original_radius, TOLERANCE, name);
            }
            (
                Geometry::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                },
                Geometry::Arc {
                    center: original_center,
                    radius: original_radius,
                    start_angle: original_start_angle,
                    end_angle: original_end_angle,
                },
            ) => {
                assert_point_close(*center, mirror(*original_center), TOLERANCE, name);
                assert_close(*radius, *original_radius, TOLERANCE, name);
                let [start, end] = arc_ends(*center, *radius, *start_angle, *end_angle);
                let [original_start, original_end] = arc_ends(
                    *original_center,
                    *original_radius,
                    *original_start_angle,
                    *original_end_angle,
                );
                assert_point_close(start, mirror(original_end), TOLERANCE, name);
                assert_point_close(end, mirror(original_start), TOLERANCE, name);
            }
            (geometry, original_geometry) => {
                panic!("{name} changed kind: {original_geometry:?} became {geometry:?}")
            }
        }
    }
}

/// Asserts the diagnosis of two solve results is the same.
#[track_caller]
fn assert_same_diagnosis(
    mirrored_result: &SolveResult,
    mirrored_params: &SketchParams,
    original_result: &SolveResult,
    original_params: &SketchParams,
) {
    assert_eq!(mirrored_result.failed, original_result.failed, "failed");
    assert_eq!(mirrored_result.dof, original_result.dof, "dof");
    assert_eq!(mirrored_result.status, original_result.status, "status");
    assert_eq!(
        mirrored_result.redundant, original_result.redundant,
        "redundant"
    );
    assert_eq!(
        mirrored_result.conflicting, original_result.conflicting,
        "conflicting"
    );
    assert_eq!(
        mirrored_result.entity_states, original_result.entity_states,
        "entity states"
    );
    assert_eq!(
        mirrored_result.issues(mirrored_params),
        original_result.issues(original_params),
        "issues"
    );
}

/// Asserts a line of a solve result within `TOLERANCE`.
#[track_caller]
fn assert_line(result: &SolveResult, entity: &str, start: [f64; 2], end: [f64; 2]) {
    let found = common::find(&result.entities, entity);
    let (actual_start, actual_end) = common::line_ends(found);
    assert_point_close(actual_start, start, TOLERANCE, entity);
    assert_point_close(actual_end, end, TOLERANCE, entity);
}

#[test]
fn t248_mirrored_plate_has_mirrored_entities() {
    let plate = plate();

    let mirrored = plate.params.mirrored();

    assert_eq!(
        mirrored.entities,
        [
            line("e-1", [0.0, 0.0], [80.0, 0.0]),
            line("e-2", [80.0, 0.0], [80.0, -50.0]),
            line("e-3", [80.0, -50.0], [0.0, -50.0]),
            line("e-4", [0.0, -50.0], [0.0, 0.0]),
            circle("e-5", [40.0, -25.0], 5.0),
            construction_line("e-6", [40.0, 0.0], [40.0, -50.0]),
        ]
    );
}

#[test]
fn t248_mirrored_plate_swaps_the_points_of_its_vertical_distance() {
    let plate = plate();

    let mirrored = plate.params.mirrored();

    assert_eq!(
        mirrored.constraints,
        with_kind(
            &plate.params.constraints,
            "c-15",
            vertical_distance_points("e-5.center", "e-1.start", "25 mm"),
        )
    );
    assert_eq!(mirrored.plane, plate.params.plane);
}

#[test]
fn t248_mirrored_plate_solves_unchanged_and_fully_constrained() {
    let mirrored = mirrored_sketch(&plate());

    let result = mirrored.solve();

    assert_bitwise(&result.entities, &mirrored.params.entities, "entities");
    assert_eq!(result.status, SketchStatus::FullyConstrained);
    assert_eq!(result.dof, 0);
    let measured_ids: Vec<String> = result.measured.keys().map(ToString::to_string).collect();
    assert_eq!(measured_ids, ["c-10", "c-11", "c-15", "c-16"]);
    assert_close(measured(&result, "c-10"), 80.0, TOLERANCE, "c-10");
    assert_close(measured(&result, "c-11"), 50.0, TOLERANCE, "c-11");
    assert_close(measured(&result, "c-15"), 25.0, TOLERANCE, "c-15");
    assert_close(measured(&result, "c-16"), 10.0, TOLERANCE, "c-16");
}

#[test]
fn t248_mirrored_plate_follows_a_new_width_mirrored() {
    let mirrored = mirrored_sketch(&plate().value("c-10", 100.0));

    let result = mirrored.solve();

    assert!(!result.failed);
    assert_line(&result, "e-1", [0.0, 0.0], [100.0, 0.0]);
    assert_line(&result, "e-2", [100.0, 0.0], [100.0, -50.0]);
    assert_line(&result, "e-3", [100.0, -50.0], [0.0, -50.0]);
    assert_line(&result, "e-4", [0.0, -50.0], [0.0, 0.0]);
    let (center, circle_radius) = common::circle_parts(common::find(&result.entities, "e-5"));
    assert_point_close(center, [50.0, -25.0], TOLERANCE, "e-5 center");
    assert_close(circle_radius, 5.0, TOLERANCE, "e-5 radius");
    assert_line(&result, "e-6", [50.0, 0.0], [50.0, -50.0]);
}

#[test]
fn t249_slot_turns_by_thirty_degrees() {
    let result = slot_sketch().solve();

    assert!(!result.failed);
    let center = common::arc_parts(common::find(&result.entities, "e-2")).center;
    assert_point_close(center, [17.320508075688775, 10.0], TOLERANCE, "e-2 center");
}

#[test]
fn t249_mirrored_slot_rewrites_arc_ends_angle_and_vertical_distance() {
    let slot = slot_sketch();

    let mirrored = slot.params.mirrored();

    let mut expected = slot.params.constraints.clone();
    for (constraint_id, kind) in [
        ("c-1", coincident("e-1.end", "e-2.end")),
        ("c-2", coincident("e-2.start", "e-3.start")),
        ("c-3", coincident("e-3.end", "e-4.end")),
        ("c-4", coincident("e-4.start", "e-1.start")),
        ("c-12", common::angle("e-1", "x_axis", "30 deg")),
        (
            "c-13",
            vertical_distance_points("e-2.center", "e-4.center", "10 mm"),
        ),
    ] {
        expected = with_kind(&expected, constraint_id, kind);
    }
    assert_eq!(mirrored.constraints, expected);
    assert_eq!(
        mirrored.entities,
        [
            line("e-1", [0.0, 5.0], [20.0, 5.0]),
            arc("e-2", [20.0, 0.0], 5.0, -FRAC_PI_2, FRAC_PI_2),
            line("e-3", [20.0, -5.0], [0.0, -5.0]),
            arc("e-4", [0.0, 0.0], 5.0, -3.0 * FRAC_PI_2, -FRAC_PI_2),
        ]
    );
}

#[test]
fn t249_mirrored_slot_solves_to_the_mirror_of_the_slot() {
    let slot = slot_sketch();
    let mirrored = mirrored_sketch(&slot);

    let original_result = slot.solve();
    let mirrored_result = mirrored.solve();

    assert_mirror_of(&mirrored_result.entities, &original_result.entities);
    let center = common::arc_parts(common::find(&mirrored_result.entities, "e-2")).center;
    assert_point_close(center, [17.320508075688775, -10.0], TOLERANCE, "e-2 center");
    assert_same_diagnosis(
        &mirrored_result,
        &mirrored.params,
        &original_result,
        &slot.params,
    );
}

#[test]
fn t250_mirroring_twice_gives_the_input() {
    for (name, sketch) in [("PLATE", plate()), ("SLOT", slot_sketch())] {
        assert_eq!(sketch.params.mirrored().mirrored(), sketch.params, "{name}");
    }
}

#[test]
fn t250_vertical_distance_on_a_line_comes_back_on_its_points() {
    let sketch = rect().with_value("c-9", vertical_distance_line("e-2", "50 mm"), 50.0);

    let twice = sketch.params.mirrored().mirrored();

    assert_eq!(
        twice.constraints,
        with_kind(
            &sketch.params.constraints,
            "c-9",
            vertical_distance_points("e-2.start", "e-2.end", "50 mm"),
        )
    );
    assert_eq!(twice.entities, sketch.params.entities);
}

#[test]
fn t250_mirrored_point_swaps_only_arc_ends() {
    let params = slot_sketch().params;

    for (point, expected) in [
        ("e-2.start", "e-2.end"),
        ("e-4.end", "e-4.start"),
        ("e-1.start", "e-1.start"),
        ("e-1.end", "e-1.end"),
        ("e-4.center", "e-4.center"),
        ("origin", "origin"),
        ("e-99.start", "e-99.start"),
    ] {
        assert_eq!(
            params.mirrored_point(&point_ref(point)),
            point_ref(expected),
            "{point}"
        );
    }
}

#[test]
fn t250_every_mirrored_region_is_found_by_key_in_the_mirrored_profile() {
    for (name, entities) in [
        ("test 150", plate_entities()),
        ("test 151", rect_with_diagonal()),
        ("test 166", slot()),
    ] {
        let params = Sketch::new(entities).params;
        let mirrored_params = params.mirrored();
        let mirrored_regions = regions_of(&mirrored_params.entities);
        let mirrored_ids = entity_ids(&mirrored_params.entities);

        for region in regions_of(&params.entities) {
            let mirrored_reference = params.mirrored_region(&region.reference());
            let found: Vec<usize> = mirrored_regions
                .iter()
                .enumerate()
                .filter(|(_, candidate)| {
                    (candidate.centroid[0] - region.centroid[0]).abs() <= TOLERANCE
                        && (candidate.centroid[1] + region.centroid[1]).abs() <= TOLERANCE
                        && (candidate.area - region.area).abs() <= TOLERANCE * region.area
                })
                .map(|(index, _)| index)
                .collect();
            let [index] = found.as_slice() else {
                panic!("{name}: one mirrored region of {} is {found:?}", region.key)
            };
            let expected = mirrored_regions[*index].reference();

            assert_eq!(
                mirrored_reference.key, expected.key,
                "{name} {}",
                region.key
            );
            assert_eq!(
                mirrored_reference.outline, expected.outline,
                "{name} {}",
                region.key
            );
            assert_eq!(
                match_region(&mirrored_regions, &mirrored_ids, &mirrored_reference),
                resolved(&[*index], MatchMethod::Key, Vec::new()),
                "{name} {}",
                region.key
            );
        }
    }
}

#[test]
fn t250_mirrored_region_examples() {
    let diagonal = Sketch::new(rect_with_diagonal()).params;
    let slot_params = Sketch::new(slot()).params;
    let plate_params = Sketch::new(plate_entities()).params;

    let lower_triangle = regions_of(&diagonal.entities)[0].reference();
    assert_eq!(
        region_ref_texts(&lower_triangle),
        ("e-1,e-2,~e-5".to_owned(), Some("e-1,e-2,~e-5".to_owned()))
    );
    assert_eq!(
        region_ref_texts(&diagonal.mirrored_region(&lower_triangle)),
        ("~e-1,~e-2,e-5".to_owned(), Some("e-5,~e-2,~e-1".to_owned()))
    );
    let slot_region = regions_of(&slot_params.entities)[0].reference();
    assert_eq!(
        slot_params.mirrored_region(&slot_region).key.to_string(),
        "~e-1,e-2,~e-3,e-4"
    );
    let plate_region = regions_of(&plate_params.entities)[0].reference();
    assert_eq!(
        plate_params.mirrored_region(&plate_region).key.to_string(),
        "~e-1,~e-2,~e-3,~e-4;~e-5"
    );
}

#[test]
fn mirrored_region_keeps_the_area_and_negates_the_centroid_v() {
    let params = Sketch::new(rect_with_diagonal()).params;

    for region in regions_of(&params.entities) {
        let mirrored_reference = params.mirrored_region(&region.reference());

        assert_eq!(
            mirrored_reference.fp,
            RegionFingerprint {
                area: region.area,
                centroid: mirror(region.centroid),
            },
            "{}",
            region.key
        );
    }
}

#[test]
fn mirrored_region_keeps_the_flag_of_an_entity_the_sketch_does_not_have() {
    let params = Sketch::new(rect_entities()).params;
    let stored = region_ref_with_outline("e-1,~e-77", "c=1,1|a=1", "e-1,~e-77");

    let mirrored_reference = params.mirrored_region(&stored);

    assert_eq!(
        region_ref_texts(&mirrored_reference),
        ("~e-1,~e-77".to_owned(), Some("~e-1,~e-77".to_owned()))
    );
}

#[test]
fn mirrored_region_without_outline_stays_without_outline() {
    let params = Sketch::new(rect_entities()).params;
    let stored = region_ref("e-1,e-2,e-3,e-4", "c=40,25|a=4000");

    let mirrored_reference = params.mirrored_region(&stored);

    assert_eq!(
        region_ref_texts(&mirrored_reference),
        ("~e-1,~e-2,~e-3,~e-4".to_owned(), None)
    );
}

#[test]
fn mirrored_keeps_ids_order_flags_and_plane() {
    let plate = plate();

    let mirrored = plate.params.mirrored();

    assert_eq!(
        entity_ids(&mirrored.entities),
        entity_ids(&plate.params.entities)
    );
    let constraint_ids = |params: &SketchParams| -> Vec<String> {
        params
            .constraints
            .iter()
            .map(|constraint| constraint.id.to_string())
            .collect()
    };
    assert_eq!(constraint_ids(&mirrored), constraint_ids(&plate.params));
    let construction = |params: &SketchParams| -> Vec<bool> {
        params
            .entities
            .iter()
            .map(|entity| entity.construction)
            .collect()
    };
    assert_eq!(construction(&mirrored), construction(&plate.params));
    assert_eq!(mirrored.plane, plate.params.plane);
}

#[test]
fn mirrored_profile_has_the_mirrored_regions() {
    let params = Sketch::new(rect_with_diagonal()).params;

    let mirrored_regions = regions_of(&params.mirrored().entities);

    assert_eq!(
        key_texts(&mirrored_regions),
        ["~e-1,~e-2,e-5", "~e-3,~e-4,~e-5"]
    );
}

/// Test 151: `RECT` split by its diagonal `e-5`.
fn rect_with_diagonal() -> Vec<SketchEntity> {
    let mut entities = rect_entities();
    entities.push(line("e-5", [0.0, 0.0], [80.0, 50.0]));
    entities
}
