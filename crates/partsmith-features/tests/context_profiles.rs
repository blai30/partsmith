//! Build context tests for profiles (features.md section 3.7): one test per row of the match table, the profile that
//! resolves to nothing, and regions used once. The sketches are those of sketch.md tests 222 to 231 and 252, whose
//! matches sketch.md pins, built by the sketch feature (`common::timeline`); the stored region is `R0`, the rectangle
//! as a pick stores it since file format version 2.

mod common;

use common::records::{circle, edited, line, rectangle, rectangle_region, sketch, xy};
use common::timeline::Timeline;
use common::{id, pointer, warning_fixes};
use partsmith_features::{
    Diagnosis, FeatureError, FeatureWarning, Fix, ProfileRef, Repair, ResolvedProfile, Severity,
};
use partsmith_sketch::{RegionLoss, RegionRef};
use serde_json::{Value, json};

/// A sketch `f-sk1` on XY with `entities`, built alone.
fn sketched(entities: Vec<Value>) -> Timeline {
    Timeline::run(&["b-1"], &[sketch("f-sk1", xy(), entities)])
}

/// A stored region reference from its JSON.
fn region(value: Value) -> RegionRef {
    serde_json::from_value(value).expect("region references written in tests are valid")
}

/// `R0`: the rectangle region with its outline.
fn rectangle_ref() -> RegionRef {
    region(rectangle_region())
}

/// `R0'`: the rectangle region as written before file format version 2, without an outline.
fn rectangle_ref_without_outline() -> RegionRef {
    region(json!({"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}))
}

/// A region that matches nothing in these sketches: its entities were drawn again.
fn redrawn_ref() -> RegionRef {
    region(json!({"key": "e-7,e-8", "fp": "c=200,200|a=100"}))
}

/// Resolves `regions` of `f-sk1` as the profile of an extrude `f-ext1` and passes the result to `check`.
fn with_profile<T>(
    timeline: &Timeline,
    regions: Vec<RegionRef>,
    check: impl FnOnce(Result<ResolvedProfile<'_>, FeatureError>) -> T,
) -> T {
    let fixture = timeline.fixture("f-ext1", "extrude");
    let profile = ProfileRef {
        sketch: id("f-sk1"),
        regions,
    };
    let context = fixture.context();
    check(context.profile("profile", &profile))
}

/// `KeepRegions` then `Pick` at `pointer`: the fixes of a region still used (section 3.7).
fn keep_regions_and_pick(pointer: &str) -> Vec<Fix> {
    vec![
        Fix::KeepRegions {
            pointer: pointer.to_string(),
        },
        Fix::Pick {
            pointer: pointer.to_string(),
        },
    ]
}

/// `Show` of the sketch then `Pick` at `pointer`: the fixes of a missing region (section 3.7).
fn show_and_pick(pointer: &str) -> Vec<Fix> {
    vec![
        Fix::Show {
            feature: id("f-sk1"),
            entity: None,
        },
        Fix::Pick {
            pointer: pointer.to_string(),
        },
    ]
}

/// `RECT` with `e-2` deleted and drawn again in place as `e-9` (sketch.md test 222, corpus R06a).
fn redrawn_side() -> Vec<Value> {
    edited(
        rectangle(),
        &["e-2"],
        vec![line("e-9", [80.0, 0.0], [80.0, 50.0])],
    )
}

/// `RECT` plus a circle inside (sketch.md test 229, corpus R50).
fn circle_inside() -> Vec<Value> {
    let mut entities = rectangle();
    entities.push(circle("e-5", [40.0, 25.0], 10.0));
    entities
}

/// `RECT` without `e-2`, `e-1` and `e-3` reaching x = 100, closed by `e-9` at x = 100 and divided by `e-10` at x = 80
/// (sketch.md test 252 (b)): the new line `e-9` is on the boundary of the union.
fn grown_outline() -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [100.0, 0.0]),
        line("e-3", [100.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-9", [100.0, 0.0], [100.0, 50.0]),
        line("e-10", [80.0, 0.0], [80.0, 50.0]),
    ]
}

/// `RECT` without `e-2`, with `e-1` reaching x = 200, closed by `e-9` at x = 80, plus a second rectangle on the
/// extended line (sketch.md test 231): two regions keep a stored line, joined by nothing new.
fn second_rectangle_on_an_extended_line() -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [200.0, 0.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-9", [80.0, 0.0], [80.0, 50.0]),
        line("e-5", [200.0, 0.0], [200.0, 20.0]),
        line("e-6", [200.0, 20.0], [150.0, 20.0]),
        line("e-7", [150.0, 20.0], [150.0, 0.0]),
    ]
}

/// The references `Region::reference()` gives the regions of `f-sk1`, in solved order.
fn solved_references(timeline: &Timeline) -> Vec<RegionRef> {
    timeline
        .sketch("f-sk1")
        .profile
        .regions
        .iter()
        .map(|region| region.reference())
        .collect()
}

#[test]
fn a_region_matched_by_key_is_used_without_a_message() {
    let timeline = sketched(rectangle());

    with_profile(&timeline, vec![rectangle_ref()], |result| {
        let resolved = result.expect("the profile resolves");
        assert_eq!(resolved.regions, [0]);
        assert!(resolved.successions.is_empty());
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
        assert!(resolved.notices.is_empty(), "{:?}", resolved.notices);
    });
}

#[test]
fn a_region_matched_by_boundary_is_used_with_its_successions() {
    let timeline = sketched(redrawn_side());

    with_profile(&timeline, vec![rectangle_ref()], |result| {
        let resolved = result.expect("the profile resolves");
        assert_eq!(resolved.regions, [0]);
        let successions: Vec<(String, Vec<String>)> = resolved
            .successions
            .iter()
            .map(|succession| {
                (
                    succession.entity.to_string(),
                    succession.now.iter().map(ToString::to_string).collect(),
                )
            })
            .collect();
        assert_eq!(successions, [("e-2".to_string(), vec!["e-9".to_string()])]);
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
        assert!(resolved.notices.is_empty(), "{:?}", resolved.notices);
    });
}

#[test]
fn a_region_without_an_outline_matched_by_boundary_has_no_succession() {
    let timeline = sketched(redrawn_side());

    with_profile(&timeline, vec![rectangle_ref_without_outline()], |result| {
        let resolved = result.expect("the profile resolves");
        assert_eq!(resolved.regions, [0]);
        assert!(
            resolved.successions.is_empty(),
            "{:?}",
            resolved.successions
        );
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
        assert!(resolved.notices.is_empty(), "{:?}", resolved.notices);
    });
}

#[test]
fn a_region_split_by_new_lines_is_used_whole_with_a_notice() {
    // Coverage (sketch.md test 229): the ring and the disc are both used.
    let timeline = sketched(circle_inside());

    with_profile(&timeline, vec![rectangle_ref()], |result| {
        let resolved = result.expect("the profile resolves");
        assert_eq!(resolved.regions, [0, 1]);
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
        let [notice] = resolved.notices.as_slice() else {
            panic!("expected one notice, got {:?}", resolved.notices);
        };
        assert_eq!(notice.code(), "region_split");
        assert_eq!(notice.severity(), Severity::Info);
        assert!(
            matches!(
                notice,
                FeatureWarning::RegionSplit {
                    position: 0,
                    count: 2,
                    ..
                }
            ),
            "{notice:?}"
        );
        assert_eq!(pointer(notice).as_deref(), Some("/profile/regions/0"));
        assert_eq!(
            warning_fixes(notice, "f-ext1"),
            keep_regions_and_pick("/profile/regions/0")
        );
    });
}

#[test]
fn a_split_region_repairs_as_a_region_that_is_still_used() {
    // features.md section 4.2: a split region is ambiguous for repair mode, never missing.
    let timeline = sketched(circle_inside());

    with_profile(&timeline, vec![rectangle_ref()], |result| {
        let resolved = result.expect("the profile resolves");
        let Some(Repair::Region {
            pointer,
            sketch,
            ambiguous,
            loss,
            ..
        }) = resolved.notices[0].repair()
        else {
            panic!("expected a region repair");
        };
        assert_eq!(pointer, "/profile/regions/0");
        assert_eq!(sketch, id("f-sk1"));
        assert!(ambiguous);
        assert_eq!(loss, None);
    });
}

#[test]
fn a_region_grown_beyond_its_boundary_is_used_with_a_warning() {
    // Grown (sketch.md test 252 (b)).
    let timeline = sketched(grown_outline());

    with_profile(&timeline, vec![rectangle_ref()], |result| {
        let resolved = result.expect("the profile resolves");
        assert_eq!(resolved.regions, [0, 1]);
        assert!(resolved.notices.is_empty(), "{:?}", resolved.notices);
        let [warning] = resolved.warnings.as_slice() else {
            panic!("expected one warning, got {:?}", resolved.warnings);
        };
        assert_eq!(warning.code(), "region_resized");
        assert_eq!(warning.severity(), Severity::Warning);
        assert!(
            matches!(
                warning,
                FeatureWarning::RegionResized {
                    position: 0,
                    count: 2,
                    ..
                }
            ),
            "{warning:?}"
        );
        assert_eq!(
            warning_fixes(warning, "f-ext1"),
            keep_regions_and_pick("/profile/regions/0")
        );
    });
}

#[test]
fn a_region_matched_by_fingerprint_is_a_guess_warning() {
    // Fingerprint: no entity of the stored key is left (features/extrude.md test 30).
    let timeline = sketched(rectangle());
    let stored = region(json!({"key": "e-11,e-12,e-13,e-14", "fp": "c=40,25|a=4000"}));

    with_profile(&timeline, vec![stored], |result| {
        let resolved = result.expect("the profile resolves");
        assert_eq!(resolved.regions, [0]);
        let [warning] = resolved.warnings.as_slice() else {
            panic!("expected one warning, got {:?}", resolved.warnings);
        };
        assert_eq!(warning.code(), "region_remapped");
        assert_eq!(warning.severity(), Severity::Warning);
        assert_eq!(pointer(warning).as_deref(), Some("/profile/regions/0"));
        assert_eq!(
            warning_fixes(warning, "f-ext1"),
            keep_regions_and_pick("/profile/regions/0")
        );
    });
}

#[test]
fn an_ambiguous_region_is_skipped_with_each_set_of_regions() {
    // Sketch.md test 231; the second stored region is region 1 itself, so the profile still has a region.
    let timeline = sketched(second_rectangle_on_an_extended_line());
    let references = solved_references(&timeline);

    with_profile(
        &timeline,
        vec![rectangle_ref(), references[1].clone()],
        |result| {
            let resolved = result.expect("the second region resolves");
            assert_eq!(resolved.regions, [1]);
            let [warning] = resolved.warnings.as_slice() else {
                panic!("expected one warning, got {:?}", resolved.warnings);
            };
            assert_eq!(warning.code(), "region_ambiguous");
            assert_eq!(warning.severity(), Severity::Warning);
            let FeatureWarning::RegionAmbiguous {
                position,
                candidates,
                ..
            } = warning
            else {
                panic!("expected an ambiguous region, got {warning:?}");
            };
            assert_eq!(*position, 0);
            assert_eq!(candidates, &[vec![0], vec![1]]);
            // The sets' references come from the sketch's result, which the document's names know.
            let fixes = timeline.with_context("f-ext1", &Diagnosis::default(), |context| {
                warning.fixes(context)
            });
            assert_eq!(
                fixes,
                [
                    Fix::UseRegions {
                        pointer: "/profile/regions/0".to_string(),
                        regions: vec![references[0].clone()],
                    },
                    Fix::UseRegions {
                        pointer: "/profile/regions/0".to_string(),
                        regions: vec![references[1].clone()],
                    },
                    Fix::Pick {
                        pointer: "/profile/regions/0".to_string()
                    },
                ]
            );
        },
    );
}

#[test]
fn a_missing_region_is_skipped_with_its_loss() {
    let timeline = sketched(rectangle());

    with_profile(&timeline, vec![rectangle_ref(), redrawn_ref()], |result| {
        let resolved = result.expect("the first region resolves");
        assert_eq!(resolved.regions, [0]);
        let [warning] = resolved.warnings.as_slice() else {
            panic!("expected one warning, got {:?}", resolved.warnings);
        };
        assert_eq!(warning.code(), "region_missing");
        assert_eq!(warning.severity(), Severity::Warning);
        assert!(
            matches!(
                warning,
                FeatureWarning::RegionMissing {
                    position: 1,
                    loss: RegionLoss::Redrawn,
                    ..
                }
            ),
            "{warning:?}"
        );
        assert_eq!(pointer(warning).as_deref(), Some("/profile/regions/1"));
        assert_eq!(
            warning_fixes(warning, "f-ext1"),
            show_and_pick("/profile/regions/1")
        );
        let Some(Repair::Region {
            ambiguous, loss, ..
        }) = warning.repair()
        else {
            panic!("expected a region repair");
        };
        assert!(!ambiguous);
        assert_eq!(loss, Some(RegionLoss::Redrawn));
    });
}

/// A lost region of `ProfileNotFound` as (position, loss, candidates).
type LostRegion = (usize, Option<RegionLoss>, Vec<Vec<usize>>);

/// Asserts that `result` is `ProfileNotFound` on `f-sk1` whose regions have these (position, loss, candidates).
#[track_caller]
fn assert_profile_not_found(
    result: Result<ResolvedProfile<'_>, FeatureError>,
    expected: &[LostRegion],
) -> FeatureError {
    let Err(error) = result else {
        panic!("expected profile_not_found");
    };
    assert_eq!(error.code(), "profile_not_found");
    let FeatureError::ProfileNotFound {
        field,
        sketch,
        regions,
    } = &error
    else {
        panic!("expected profile_not_found, got {error:?}");
    };
    assert_eq!(field.path, "profile");
    assert_eq!(sketch, &id("f-sk1"));
    let actual: Vec<LostRegion> = regions
        .iter()
        .map(|lost| (lost.position, lost.loss.clone(), lost.candidates.clone()))
        .collect();
    assert_eq!(actual, expected);
    error
}

#[test]
fn a_profile_whose_outline_lost_a_line_is_not_found() {
    // Newcomer error corpus NC11: the back line was deleted.
    let timeline = sketched(edited(rectangle(), &["e-3"], Vec::new()));

    with_profile(&timeline, vec![rectangle_ref()], |result| {
        let error = assert_profile_not_found(
            result,
            &[(
                0,
                Some(RegionLoss::Deleted {
                    entities: vec![id("e-3")],
                }),
                Vec::new(),
            )],
        );
        let repairs = error.repairs();
        let [
            Repair::Region {
                pointer, ambiguous, ..
            },
        ] = repairs.as_slice()
        else {
            panic!("expected one region repair");
        };
        assert_eq!(pointer, "/profile/regions/0");
        assert!(!*ambiguous);
    });
}

#[test]
fn a_profile_whose_outline_is_open_is_not_found() {
    // Every entity is still there, but e-2 no longer reaches e-3.
    let timeline = sketched(edited(
        rectangle(),
        &["e-2"],
        vec![line("e-2", [80.0, 0.0], [80.0, 40.0])],
    ));

    with_profile(&timeline, vec![rectangle_ref()], |result| {
        assert_profile_not_found(result, &[(0, Some(RegionLoss::Open), Vec::new())]);
    });
}

#[test]
fn a_profile_whose_only_region_is_ambiguous_is_not_found_with_its_sets() {
    let timeline = sketched(second_rectangle_on_an_extended_line());

    with_profile(&timeline, vec![rectangle_ref()], |result| {
        let error = assert_profile_not_found(result, &[(0, None, vec![vec![0], vec![1]])]);
        let repairs = error.repairs();
        let [
            Repair::Region {
                ambiguous,
                candidates,
                ..
            },
        ] = repairs.as_slice()
        else {
            panic!("expected one region repair");
        };
        assert!(*ambiguous);
        assert_eq!(candidates, &[vec![0], vec![1]]);
    });
}

#[test]
fn a_profile_that_resolves_to_nothing_keeps_every_region() {
    let timeline = sketched(rectangle());

    with_profile(&timeline, vec![redrawn_ref(), redrawn_ref()], |result| {
        assert_profile_not_found(
            result,
            &[
                (0, Some(RegionLoss::Redrawn), Vec::new()),
                (1, Some(RegionLoss::Redrawn), Vec::new()),
            ],
        );
    });
}

#[test]
fn two_references_to_one_region_use_it_once() {
    let timeline = sketched(rectangle());

    with_profile(
        &timeline,
        vec![rectangle_ref(), rectangle_ref()],
        |result| {
            let resolved = result.expect("the profile resolves");
            assert_eq!(resolved.regions, [0]);
        },
    );
}

#[test]
fn equal_successions_are_kept_once() {
    // Both stored references match the redrawn rectangle by boundary with the same succession.
    let timeline = sketched(redrawn_side());

    with_profile(
        &timeline,
        vec![rectangle_ref(), rectangle_ref()],
        |result| {
            let resolved = result.expect("the profile resolves");
            assert_eq!(resolved.regions, [0]);
            assert_eq!(resolved.successions.len(), 1);
        },
    );
}
