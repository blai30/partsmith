//! Acceptance tests for the rows of features.md sections 4.1 and 4.3 that the reference stability plan's phase 3
//! rewrites (its section 7.2): the texts and fixes of the errors that name a field, a body, a feature or a limit, the
//! messages of the "too large" search with the face that limits it, the round faces of a shell and the bare-number
//! sentence, kernel failures (section 6.5 test 7), values next to their bounds and beyond the modeling limit (section
//! 6.5 test 9), and the message lint over every template of these rows (section 6.5 test 14). Messages that need a run
//! of a whole document are in `crates/partsmith-recompute/tests/feature_*.rs`.

mod common;

use std::collections::BTreeMap;

use common::lint::assert_passes_lint;
use common::records::{
    RISER_EDGE, ROUNDED_BOX_TOP, STEP_BOTTOM_LEFT_EDGE, reference, rounded_box, shell, step,
    suppressed,
};
use common::timeline::{Timeline, stored};
use common::{LabelNames, field, id, pick};
use partsmith_features::{
    Bound, Diagnosis, FeatureError, FieldRef, FieldUnit, Fix, Limit, Noun, Place, RenderContext,
    Severity,
};
use partsmith_model::{Expr, Units};
use partsmith_naming::Reference;
use partsmith_occt_sys::{KernelError, KernelErrorKind, Operation};
use serde_json::{Value, json};

/// The labels the hand-built messages of this file are rendered with.
fn labels() -> LabelNames {
    LabelNames::new(
        &[("b-1", "Body 1"), ("b-2", "Body 2")],
        &[
            ("f-sk1", "Sketch 1", "Sketch"),
            ("f-sk2", "Sketch 2", "Sketch"),
            ("f-ext1", "Extrude 1", "Extrude"),
            ("f-box1", "Box 1", "Box"),
            ("f-box2", "Box 2", "Box"),
            ("f-cyl1", "Cylinder 1", "Cylinder"),
            ("f-fil1", "Fillet 1", "Fillet"),
            ("f-cha1", "Chamfer 1", "Chamfer"),
            ("f-sh1", "Shell 1", "Shell"),
            ("f-hole1", "Hole 1", "Hole"),
            ("f-imp1", "Import 1", "Import"),
            ("f-bool1", "Boolean 1", "Boolean"),
            ("f-mir1", "Mirror 1", "Mirror"),
            ("f-lp1", "Linear pattern 1", "Linear pattern"),
            ("f-split1", "Split body 1", "Split body"),
            ("f-rev1", "Revolve 1", "Revolve"),
            ("f-dp1", "Datum plane 1", "Datum plane"),
            ("f-tor1", "Torus 1", "Torus"),
        ],
    )
}

/// Calls `use_context` with the render context of `feature` in a document of `units`, with the labels of this file,
/// `diagnosis` and no body states.
fn with_labels<T>(
    feature: &str,
    units: Units,
    diagnosis: &Diagnosis,
    use_context: impl FnOnce(&RenderContext<'_>) -> T,
) -> T {
    let names = labels();
    let feature = id(feature);
    let bodies = BTreeMap::new();
    use_context(&RenderContext {
        feature: &feature,
        names: &names,
        units,
        bodies: &bodies,
        diagnosis,
    })
}

/// `error` rendered on `feature` with the labels of this file in a millimeter document, without a diagnosis.
fn rendered(error: &FeatureError, feature: &str) -> String {
    with_labels(feature, Units::Mm, &Diagnosis::default(), |context| {
        error.render(context)
    })
}

/// The fixes of `error` on `feature` with the labels of this file in a millimeter document, without a diagnosis.
fn fixes_of(error: &FeatureError, feature: &str) -> Vec<Fix> {
    with_labels(feature, Units::Mm, &Diagnosis::default(), |context| {
        error.fixes(context)
    })
}

/// `SetField` of this feature's field `path` to `value`.
fn set_field(path: &str, label: &str, value: Value) -> Fix {
    Fix::SetField {
        feature: None,
        field: field(path, label),
        value,
    }
}

/// "Use {value}": `SetField` of this feature's field `path` to the expression `value`.
fn use_value(path: &str, label: &str, value: &str) -> Fix {
    set_field(path, label, json!(value))
}

/// `SetField` of Operation to `op` (features.md section 4.1: "Set Operation to Join").
fn set_operation(op: &str) -> Fix {
    use_value("op", "Operation", op)
}

/// `Suppress` of a feature.
fn suppress(feature: &str) -> Fix {
    Fix::Suppress {
        feature: id(feature),
    }
}

/// `RemoveEntry` of the list entry at `pointer`.
fn remove_entry(pointer: &str) -> Fix {
    Fix::RemoveEntry {
        pointer: pointer.to_string(),
    }
}

/// The field `target` of mirror, patterns and split body.
fn target() -> FieldRef {
    field("target", "Target body")
}

/// The field `profile.sketch` of extrude and revolve.
fn profile_sketch() -> FieldRef {
    field("profile.sketch", "Profile")
}

#[test]
fn target_required_asks_for_an_empty_body_or_join() {
    let error = FeatureError::TargetRequired { field: target() };

    assert_eq!(
        rendered(&error, "f-mir1"),
        "Operation New body puts the copies into another body, so Target body needs an empty body. Pick one, or set Operation to Join."
    );
    assert_eq!(
        fixes_of(&error, "f-mir1"),
        [set_operation("join"), pick("/target")]
    );
}

#[test]
fn target_not_allowed_offers_new_body_or_clearing_the_target() {
    let error = FeatureError::TargetNotAllowed { field: target() };

    assert_eq!(
        rendered(&error, "f-mir1"),
        "Target body is used only when Operation is New body. Clear Target body, or set Operation to New body."
    );
    assert_eq!(
        fixes_of(&error, "f-mir1"),
        [
            set_operation("new"),
            set_field("target", "Target body", Value::Null)
        ]
    );
}

#[test]
fn an_unknown_body_names_the_field_that_names_it() {
    let error = FeatureError::UnknownBody {
        field: field("body", "Body"),
        body: id("b-9"),
    };

    assert_eq!(
        rendered(&error, "f-fil1"),
        "Body names a body that is not in this document. Pick a body for Body."
    );
    assert_eq!(fixes_of(&error, "f-fil1"), [pick("/body")]);
}

#[test]
fn an_unknown_tool_body_names_its_entry() {
    let error = FeatureError::UnknownBody {
        field: field("tools[0]", "Tool bodies"),
        body: id("b-9"),
    };

    assert_eq!(
        rendered(&error, "f-bool1"),
        "Tool bodies names a body that is not in this document. Pick a body for Tool bodies."
    );
    assert_eq!(fixes_of(&error, "f-bool1"), [pick("/tools/0")]);
}

#[test]
fn an_unknown_feature_names_the_field_that_names_it() {
    let error = FeatureError::UnknownFeature {
        field: profile_sketch(),
        feature: id("f-sk9"),
    };

    assert_eq!(
        rendered(&error, "f-ext1"),
        "Profile names a feature that is not in this document. Pick another one for Profile."
    );
    assert_eq!(fixes_of(&error, "f-ext1"), [pick("/profile/sketch")]);
}

#[test]
fn a_forward_reference_offers_to_move_this_feature_below_it() {
    let error = FeatureError::ForwardReference {
        field: profile_sketch(),
        feature: id("f-sk1"),
    };

    assert_eq!(
        rendered(&error, "f-ext1"),
        "Profile uses 'Sketch 1', which comes after this extrude. Move this extrude below 'Sketch 1', or pick something earlier."
    );
    assert_eq!(
        fixes_of(&error, "f-ext1"),
        [
            Fix::Move {
                feature: id("f-ext1"),
                place: Place::Below(id("f-sk1")),
            },
            pick("/profile/sketch"),
        ]
    );
}

#[test]
fn a_suppressed_upstream_feature_offers_unsuppress_then_pick() {
    let error = FeatureError::UpstreamSuppressed {
        field: profile_sketch(),
        feature: id("f-sk1"),
    };

    assert_eq!(
        rendered(&error, "f-ext1"),
        "'Sketch 1' is suppressed, so this extrude cannot use it. Unsuppress 'Sketch 1', or pick something else."
    );
    assert_eq!(
        fixes_of(&error, "f-ext1"),
        [
            Fix::Unsuppress {
                feature: id("f-sk1")
            },
            pick("/profile/sketch"),
        ]
    );
    assert_eq!(error.field(), Some(&profile_sketch()));
}

#[test]
fn a_missing_asset_says_where_the_file_should_be() {
    let asset = format!("{}.step", "0".repeat(64));
    let error = FeatureError::AssetMissing {
        field: field("asset", "File"),
        asset: asset.clone(),
    };

    assert_eq!(
        rendered(&error, "f-imp1"),
        "The STEP file of this import is not in this document's assets folder. Copy the assets folder that came with the document next to it, or import the file again."
    );
    // Import again is a host action, so the core offers none (section 4.1).
    assert_eq!(fixes_of(&error, "f-imp1"), []);
    let detail = error.detail().expect("the asset name is in the detail");
    assert!(detail.contains(&asset), "detail {detail:?}");
}

#[test]
fn operation_new_body_on_a_body_with_a_solid_offers_join_or_a_new_body() {
    let error = FeatureError::BodyNotEmpty {
        body: id("b-1"),
        target: None,
    };

    assert_eq!(
        rendered(&error, "f-ext1"),
        "Body 'Body 1' already has a solid here, so Operation New body cannot start it. Set Operation to Join, or put this extrude in a new body."
    );
    assert_eq!(
        fixes_of(&error, "f-ext1"),
        [set_operation("join"), Fix::NewBody]
    );
}

#[test]
fn a_target_with_a_solid_asks_for_an_empty_target() {
    let error = FeatureError::BodyNotEmpty {
        body: id("b-2"),
        target: Some(target()),
    };

    assert_eq!(
        rendered(&error, "f-mir1"),
        "Body 'Body 2' already has a solid here, so this mirror cannot put its result there. Pick an empty body for Target body."
    );
    assert_eq!(fixes_of(&error, "f-mir1"), [pick("/target")]);
    assert_eq!(
        rendered(&error, "f-split1"),
        "Body 'Body 2' already has a solid here, so this split body cannot put its result there. Pick an empty body for Target body."
    );
}

#[test]
fn a_cut_that_removes_everything_asks_to_make_it_smaller() {
    let error = FeatureError::EmptyResult {
        body: id("b-1"),
        op: partsmith_features::BodyOp::Cut,
    };

    assert_eq!(
        rendered(&error, "f-box2"),
        "This cut removes all of body 'Body 1'. Make it smaller, or change its position or direction."
    );
    assert_eq!(fixes_of(&error, "f-box2"), []);
}

#[test]
fn an_empty_intersection_asks_to_move_the_shapes() {
    let error = FeatureError::EmptyResult {
        body: id("b-1"),
        op: partsmith_features::BodyOp::Intersect,
    };

    assert_eq!(
        rendered(&error, "f-box2"),
        "The shapes do not overlap body 'Body 1', so the intersection is empty. Move them so they overlap it."
    );
}

#[test]
fn an_entity_the_sketch_no_longer_has_asks_to_pick_one_again() {
    let error = FeatureError::UnknownEntity {
        field: field("points[0]", "Points"),
        sketch: id("f-sk1"),
        entity: "e-p9".to_string(),
        noun: Noun::Point,
    };

    assert_eq!(
        rendered(&error, "f-hole1"),
        "'Sketch 1' no longer has the point picked for Points. Pick a point of 'Sketch 1' again."
    );
    assert_eq!(fixes_of(&error, "f-hole1"), [pick("/points/0")]);
    // The entity's id is for reports only (section 2.8).
    assert_eq!(error.detail().as_deref(), Some("e-p9"));
}

#[test]
fn a_line_the_sketch_no_longer_has_names_its_kind() {
    let error = FeatureError::UnknownEntity {
        field: field("axis", "Axis"),
        sketch: id("f-sk1"),
        entity: "e-9".to_string(),
        noun: Noun::Line,
    };

    assert_eq!(
        rendered(&error, "f-rev1"),
        "'Sketch 1' no longer has the line picked for Axis. Pick a line of 'Sketch 1' again."
    );
}

/// The kernel failure of section 6.5 test 7.
fn make_box_failure() -> KernelError {
    KernelError {
        operation: Operation::MakeBox,
        kind: KernelErrorKind::Failed,
        exception: "StdFail_NotDone".to_string(),
        message: "BRep_API: command not done".to_string(),
    }
}

#[test]
fn t007_a_kernel_failure_names_the_feature_and_gives_the_kernel_hint() {
    let error = FeatureError::Kernel {
        error: make_box_failure(),
    };

    assert_eq!(
        rendered(&error, "f-box1"),
        "Partsmith's geometry kernel could not build this box. Its sizes passed every check, so this is likely a bug; report it with the details."
    );
    assert_eq!(fixes_of(&error, "f-box1"), [suppress("f-box1")]);
    assert_eq!(
        error.detail().as_deref(),
        Some("make_box failed: StdFail_NotDone: BRep_API: command not done")
    );
}

#[test]
fn a_shell_that_would_remove_every_face_says_how_many_it_may_remove() {
    let error = FeatureError::ShellRemovesAll {
        body: id("b-1"),
        count: 6,
    };

    assert_eq!(error.code(), "shell_removes_all");
    assert_eq!(error.severity(), Severity::Error);
    assert_eq!(
        rendered(&error, "f-sh1"),
        "This shell would remove all 6 faces of body 'Body 1', which leaves nothing to hollow. Remove at most 5 faces, or leave Faces to remove empty for a closed hollow."
    );
    assert_eq!(fixes_of(&error, "f-sh1"), [pick("/remove_faces")]);
}

#[test]
fn through_all_that_reversing_reaches_says_only_where_it_points() {
    let error = FeatureError::ThroughAllMisses {
        body: id("b-1"),
        reverse_hits: true,
    };

    assert_eq!(
        rendered(&error, "f-ext1"),
        "Through all points away from body 'Body 1', so there is nothing to go through."
    );
}

#[test]
fn through_all_that_reversing_misses_too_asks_to_point_it_at_the_body() {
    let error = FeatureError::ThroughAllMisses {
        body: id("b-1"),
        reverse_hits: false,
    };

    assert_eq!(
        rendered(&error, "f-ext1"),
        "Through all points away from body 'Body 1', so there is nothing to go through. Point it at the body, or use a distance."
    );
    // Turn on Reverse is offered only when it reaches the body (section 4.4).
    assert_eq!(fixes_of(&error, "f-ext1"), []);
}

#[test]
fn through_all_into_a_body_without_a_solid_asks_for_a_distance() {
    let error = FeatureError::ThroughAllNeedsBody { body: id("b-1") };

    assert_eq!(
        rendered(&error, "f-ext1"),
        "Through all goes through an existing solid, and body 'Body 1' has none here. Use a distance instead."
    );
    assert_eq!(fixes_of(&error, "f-ext1"), []);
}

#[test]
fn too_many_copies_name_the_pattern_and_the_limit() {
    let error = FeatureError::TooManyInstances {
        count: 20000,
        limit: 10000,
    };

    assert_eq!(
        rendered(&error, "f-lp1"),
        "This linear pattern would make 20000 copies, more than the limit of 10000. Lower Count."
    );
}

#[test]
fn a_canceled_build_names_the_feature() {
    assert_eq!(
        rendered(&FeatureError::Canceled, "f-lp1"),
        "Recompute was canceled before this linear pattern finished."
    );
}

/// An `OutOfRange` error of a length field.
fn length_out_of_range(path: &str, label: &str, value: f64, bound: Bound) -> FeatureError {
    FeatureError::OutOfRange {
        field: field(path, label),
        kind: FieldUnit::Length,
        value,
        bound,
        bound_name: None,
    }
}

#[test]
fn t009_a_value_next_to_a_strict_bound_gets_the_digits_that_tell_it_apart() {
    let error = length_out_of_range("extent.expr", "Distance", 1e-7, Bound::GreaterThan(0.0));

    assert_eq!(
        rendered(&error, "f-ext1"),
        "Distance must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm."
    );
}

#[test]
fn t009_a_value_next_to_a_named_strict_bound_gets_the_digits_that_tell_it_apart() {
    let error = FeatureError::OutOfRange {
        field: field("hole_type.depth", "Counterbore depth"),
        kind: FieldUnit::Length,
        value: 4.9999999,
        bound: Bound::LessThan(5.0),
        bound_name: Some("the hole depth".to_string()),
    };

    assert_eq!(
        rendered(&error, "f-hole1"),
        "Counterbore depth must be less than the hole depth (5 mm), but it is 4.9999999 mm. Use a value less than 5 mm."
    );
}

#[test]
fn t009_a_length_beyond_the_modeling_limit_is_written_in_exponent_form() {
    let error = length_out_of_range("size[0]", "Length", 1e200, Bound::AtMost(1_000_000.0));

    assert_eq!(
        rendered(&error, "f-box1"),
        "Length must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less."
    );
    assert_eq!(
        fixes_of(&error, "f-box1"),
        [use_value("size[0]", "Length", "1000000 mm")]
    );
    assert_eq!(error.suggestion(Units::Mm).as_deref(), Some("1000000 mm"));
}

#[test]
fn a_negative_length_beyond_the_modeling_limit_offers_the_lower_limit() {
    let error = length_out_of_range(
        "translation[0]",
        "Move X",
        -1e7,
        Bound::AtLeast(-1_000_000.0),
    );

    assert_eq!(
        rendered(&error, "f-ext1"),
        "Move X must be at least -1000000 mm, but it is -10000000 mm. Use -1000000 mm or more."
    );
    assert_eq!(
        fixes_of(&error, "f-ext1"),
        [use_value("translation[0]", "Move X", "-1000000 mm")]
    );
}

#[test]
fn a_huge_negative_size_reports_its_own_bound_in_exponent_form() {
    let error = length_out_of_range("size[0]", "Length", -1e200, Bound::GreaterThan(0.0));

    assert_eq!(
        rendered(&error, "f-box1"),
        "Length must be greater than 0 mm, but it is -1e200 mm. Use a value greater than 0 mm."
    );
    assert_eq!(fixes_of(&error, "f-box1"), []);
}

#[test]
fn an_angle_next_to_its_strict_bound_gets_the_digits_that_tell_it_apart() {
    let error = FeatureError::OutOfRange {
        field: field("extent.expr", "Angle"),
        kind: FieldUnit::Angle,
        value: 1e-8_f64.to_radians(),
        bound: Bound::GreaterThan(0.0),
        bound_name: None,
    };

    assert_eq!(
        rendered(&error, "f-rev1"),
        "Angle must be greater than 0 deg, but it is 0.00000001 deg. Use a value greater than 0 deg."
    );
}

#[test]
fn a_value_just_under_a_named_upper_bound_gets_the_digits_that_tell_it_apart() {
    let error = FeatureError::OutOfRange {
        field: field("minor_radius", "Tube radius"),
        kind: FieldUnit::Length,
        value: 9.9999999,
        bound: Bound::LessThan(10.0),
        bound_name: Some("the ring radius".to_string()),
    };

    assert_eq!(
        rendered(&error, "f-tor1"),
        "Tube radius must be less than the ring radius (10 mm), but it is 9.9999999 mm. Use a value less than 10 mm."
    );
}

// The "too large" search (section 4.3), on `STEP` of features/fillet.md tests 19 to 22 (newcomer error corpus NC05):
// the limiting face is `f-box2/left`, the riser, 2.4 mm across `E-RISER`.

/// The kernel failure of a fillet.
fn fillet_failure() -> KernelError {
    KernelError {
        operation: Operation::Fillet,
        kind: KernelErrorKind::Failed,
        exception: "StdFail_NotDone".to_string(),
        message: "BRep_API: command not done".to_string(),
    }
}

/// `STEP` followed by `feature` suppressed, so messages on it read the stepped body, its label and its type without
/// running the search.
fn stepped(feature: Value) -> Timeline {
    let mut features = step();
    features.push(suppressed(feature));
    Timeline::run(&["b-1"], &features)
}

/// `STEP` with `f-fil1` of `E-RISER` and the bottom left edge at radius 3 after it (features/fillet.md test 20).
fn stepped_fillet() -> Timeline {
    stepped_fillet_of(&[RISER_EDGE, STEP_BOTTOM_LEFT_EDGE])
}

/// `STEP` with `f-fil1` of `edges`, in this order, at radius 3 after it.
fn stepped_fillet_of(edges: &[(&str, &str)]) -> Timeline {
    let edges: Vec<Value> = edges
        .iter()
        .map(|(query, fingerprint)| reference(query, fingerprint))
        .collect();
    stepped(json!({"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": edges, "radius": "3"}))
}

/// `STEP` with `f-cha1` of `E-RISER` and the bottom left edge with `distance` after it.
fn stepped_chamfer(distance: Value) -> Timeline {
    stepped(json!({
        "id": "f-cha1",
        "type": "chamfer",
        "body": "b-1",
        "edges": [
            reference(RISER_EDGE.0, RISER_EDGE.1),
            reference(STEP_BOTTOM_LEFT_EDGE.0, STEP_BOTTOM_LEFT_EDGE.1)
        ],
        "distance": distance
    }))
}

/// The riser of `STEP` measured 2.4 mm across `E-RISER`, the edge at `position` of the list, as the search reports it.
fn riser_limit(timeline: &Timeline, position: usize, removable: bool) -> Limit {
    Limit::Face {
        position,
        edge: timeline.picked("b-1", RISER_EDGE.0),
        face: timeline.picked("b-1", "f-box2/left"),
        width: 2.4,
        removable,
    }
}

/// `FilletTooLarge` of a 3 mm radius on `STEP` with this largest working radius and limit.
fn fillet_too_large(max: Option<f64>, limit: Option<Limit>) -> FeatureError {
    FeatureError::FilletTooLarge {
        field: field("radius", "Radius"),
        expr: Expr::new("3"),
        radius: 3.0,
        max,
        limit,
        cause: fillet_failure(),
    }
}

/// `error` rendered on `feature` of `timeline`, with the stepped body.
fn rendered_on(timeline: &Timeline, feature: &str, error: &FeatureError) -> String {
    timeline.with_end_context(feature, &Diagnosis::default(), |context| {
        error.render(context)
    })
}

/// The fixes of `error` on `feature` of `timeline`.
fn fixes_on(timeline: &Timeline, feature: &str, error: &FeatureError) -> Vec<Fix> {
    timeline.with_end_context(feature, &Diagnosis::default(), |context| {
        error.fixes(context)
    })
}

/// The sentence that names the riser as the limit (newcomer error corpus NC05).
const RISER_SENTENCE: &str = "The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across.";

#[test]
fn a_fillet_with_a_limit_and_a_limiting_face_names_both() {
    let timeline = stepped_fillet();
    let error = fillet_too_large(Some(2.396), Some(riser_limit(&timeline, 0, false)));

    assert_eq!(
        rendered_on(&timeline, "f-fil1", &error),
        format!(
            "Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm. {RISER_SENTENCE}"
        )
    );
    assert_eq!(
        fixes_on(&timeline, "f-fil1", &error),
        [use_value("radius", "Radius", "2.3 mm")]
    );
    // With a working value, the detail is not needed (section 2.8).
    assert_eq!(error.detail(), None);
}

#[test]
fn a_removable_limiting_edge_is_offered_after_the_limit() {
    let timeline = stepped_fillet();
    let error = fillet_too_large(Some(2.396), Some(riser_limit(&timeline, 0, true)));

    assert_eq!(
        fixes_on(&timeline, "f-fil1", &error),
        [
            use_value("radius", "Radius", "2.3 mm"),
            remove_entry("/edges/0")
        ]
    );
}

#[test]
fn the_removal_names_the_entry_of_the_limiting_edge() {
    let timeline = stepped_fillet_of(&[STEP_BOTTOM_LEFT_EDGE, RISER_EDGE]);
    let error = fillet_too_large(Some(2.396), Some(riser_limit(&timeline, 1, true)));

    assert_eq!(
        fixes_on(&timeline, "f-fil1", &error),
        [
            use_value("radius", "Radius", "2.3 mm"),
            remove_entry("/edges/1")
        ]
    );
}

#[test]
fn a_fillet_with_a_limit_and_no_measured_face_states_the_limit_alone() {
    let timeline = stepped_fillet();
    let error = fillet_too_large(Some(2.396), None);

    assert_eq!(
        rendered_on(&timeline, "f-fil1", &error),
        "Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm."
    );
    assert_eq!(
        fixes_on(&timeline, "f-fil1", &error),
        [use_value("radius", "Radius", "2.3 mm")]
    );
}

#[test]
fn a_fillet_without_a_useful_limit_still_names_the_limiting_face() {
    let timeline = stepped_fillet();
    let error = fillet_too_large(None, Some(riser_limit(&timeline, 0, true)));

    assert_eq!(
        rendered_on(&timeline, "f-fil1", &error),
        format!(
            "Fillet radius 3 mm does not work on the selected edges, even when smaller. {RISER_SENTENCE}"
        )
    );
    assert_eq!(
        fixes_on(&timeline, "f-fil1", &error),
        [remove_entry("/edges/0"), suppress("f-fil1")]
    );
}

#[test]
fn a_limiting_face_whose_edge_cannot_be_removed_leaves_suppress() {
    let timeline = stepped_fillet();
    let error = fillet_too_large(None, Some(riser_limit(&timeline, 0, false)));

    assert_eq!(fixes_on(&timeline, "f-fil1", &error), [suppress("f-fil1")]);
}

#[test]
fn a_fillet_without_a_limit_or_a_face_says_partsmith_could_not_tell_why() {
    let timeline = stepped_fillet();
    let error = fillet_too_large(None, None);

    assert_eq!(
        rendered_on(&timeline, "f-fil1", &error),
        "Fillet radius 3 mm does not work on the selected edges, even when smaller, and Partsmith could not tell why."
    );
    assert_eq!(fixes_on(&timeline, "f-fil1", &error), [suppress("f-fil1")]);
    // Without a working value, the kernel's text is what a report needs (section 2.8).
    assert_eq!(error.detail(), Some(fillet_failure().detail()));
}

/// `ChamferTooLarge` of a 3 mm distance on `STEP`: an equal distance (field `distance.expr`, with its expression) or
/// two distances (field `distance`, without one).
fn chamfer_too_large(equal: bool, max: Option<f64>, limit: Option<Limit>) -> FeatureError {
    FeatureError::ChamferTooLarge {
        field: if equal {
            field("distance.expr", "Distance")
        } else {
            field("distance", "Distance")
        },
        expr: equal.then(|| Expr::new("3")),
        distance: 3.0,
        max,
        limit,
        cause: KernelError {
            operation: Operation::Chamfer,
            ..fillet_failure()
        },
    }
}

#[test]
fn a_chamfer_with_a_limit_and_a_limiting_face_names_both() {
    let timeline = stepped_chamfer(json!({"kind": "equal", "expr": "3"}));
    let error = chamfer_too_large(true, Some(2.396), Some(riser_limit(&timeline, 0, true)));

    assert_eq!(
        rendered_on(&timeline, "f-cha1", &error),
        format!(
            "Chamfer distance 3 mm is too large for the selected edges: the largest that works is about 2.3 mm. {RISER_SENTENCE}"
        )
    );
    assert_eq!(
        fixes_on(&timeline, "f-cha1", &error),
        [
            use_value("distance.expr", "Distance", "2.3 mm"),
            remove_entry("/edges/0")
        ]
    );
}

#[test]
fn a_two_distance_chamfer_offers_no_limit_but_its_removal() {
    let timeline =
        stepped_chamfer(json!({"kind": "two_distances", "expr": "1", "second_expr": "3"}));
    let error = chamfer_too_large(false, Some(2.396), Some(riser_limit(&timeline, 0, true)));

    assert_eq!(
        fixes_on(&timeline, "f-cha1", &error),
        [remove_entry("/edges/0")]
    );
}

#[test]
fn a_chamfer_without_a_limit_or_a_face_says_partsmith_could_not_tell_why() {
    let timeline = stepped_chamfer(json!({"kind": "equal", "expr": "3"}));
    let error = chamfer_too_large(true, None, None);

    assert_eq!(
        rendered_on(&timeline, "f-cha1", &error),
        "Chamfer distance 3 mm does not work on the selected edges, even when smaller, and Partsmith could not tell why."
    );
    assert_eq!(fixes_on(&timeline, "f-cha1", &error), [suppress("f-cha1")]);
}

// Shells too thick (section 4.3): the round faces of the body smaller than the wall, made by one feature.

/// `ShellTooThick` of `expr` (`thickness` mm) on `b-1` with this largest working wall and limit.
fn shell_too_thick(
    expr: &str,
    thickness: f64,
    max: Option<f64>,
    limit: Option<Limit>,
) -> FeatureError {
    FeatureError::ShellTooThick {
        field: field("thickness", "Thickness"),
        expr: Expr::new(expr),
        body: id("b-1"),
        thickness,
        max,
        limit,
        cause: KernelError {
            operation: Operation::Shell,
            ..fillet_failure()
        },
    }
}

/// The round faces of `f-fil1`, a fillet of radius 1 that wrote the body before the shell.
fn fillet_rounds() -> Limit {
    Limit::RoundFaces {
        feature: id("f-fil1"),
        radius: 1.0,
        fillet: true,
    }
}

/// The rounded box of newcomer error corpus NC06 with its rounds' radius `radius`, then the shell `f-sh1` of 2.1 mm
/// removing its top, suppressed, in a document with `parameters`.
fn rounded_then_shelled(parameters: &[(&str, &str)], radius: &str) -> Timeline {
    let mut features = rounded_box(radius);
    features.push(suppressed(shell(
        "2.1",
        vec![reference(ROUNDED_BOX_TOP.0, ROUNDED_BOX_TOP.1)],
    )));
    Timeline::run_with_parameters(&["b-1"], parameters, &features)
}

/// "Set Radius of 'Fillet 1' to {value}": `SetField` of the radius of `f-fil1`.
fn set_fillet_radius(value: &str) -> Fix {
    Fix::SetField {
        feature: Some(id("f-fil1")),
        field: field("radius", "Radius"),
        value: json!(value),
    }
}

/// "Move 'Shell 1' above 'Fillet 1'".
fn move_shell_above_the_fillet() -> Fix {
    Fix::Move {
        feature: id("f-sh1"),
        place: Place::Above(id("f-fil1")),
    }
}

#[test]
fn rounds_of_a_fillet_before_the_shell_are_named_with_their_radius() {
    let timeline = rounded_then_shelled(&[], "1");
    let error = shell_too_thick("2.1", 2.1, Some(0.995), Some(fillet_rounds()));

    assert_eq!(
        rendered_on(&timeline, "f-sh1", &error),
        "Shell thickness 2.1 mm is too thick for body 'Body 1': the largest that works is about 0.99 mm. Its rounded faces made by 'Fillet 1' have radius 1 mm, smaller than the wall."
    );
}

#[test]
fn rounds_of_a_fillet_with_a_plain_radius_offer_a_larger_radius_then_the_move_then_the_limit() {
    let timeline = rounded_then_shelled(&[], "1");
    let error = shell_too_thick("2.1", 2.1, Some(0.995), Some(fillet_rounds()));

    // 1.5 times 2.1 mm is 3.15 mm, rounded up to 2 significant digits.
    assert_eq!(
        fixes_on(&timeline, "f-sh1", &error),
        [
            set_fillet_radius("3.2 mm"),
            move_shell_above_the_fillet(),
            use_value("thickness", "Thickness", "0.99 mm"),
        ]
    );
}

#[test]
fn rounds_of_a_fillet_whose_radius_is_a_parameter_keep_the_link() {
    let timeline = rounded_then_shelled(&[("r", "1 mm")], "r");
    let error = shell_too_thick("2.1", 2.1, Some(0.995), Some(fillet_rounds()));

    assert_eq!(
        fixes_on(&timeline, "f-sh1", &error),
        [
            move_shell_above_the_fillet(),
            use_value("thickness", "Thickness", "0.99 mm"),
        ]
    );
}

#[test]
fn rounds_without_a_working_wall_offer_the_radius_the_move_and_suppress() {
    let timeline = rounded_then_shelled(&[], "1");
    let error = shell_too_thick("2.1", 2.1, None, Some(fillet_rounds()));

    assert_eq!(
        rendered_on(&timeline, "f-sh1", &error),
        "Shell thickness 2.1 mm does not work on body 'Body 1', even when thinner. Its rounded faces made by 'Fillet 1' have radius 1 mm, smaller than the wall."
    );
    assert_eq!(
        fixes_on(&timeline, "f-sh1", &error),
        [
            set_fillet_radius("3.2 mm"),
            move_shell_above_the_fillet(),
            suppress("f-sh1"),
        ]
    );
}

#[test]
fn rounds_not_made_by_a_fillet_are_stated_without_a_fix_of_their_own() {
    let error = shell_too_thick(
        "30",
        30.0,
        Some(19.4),
        Some(Limit::RoundFaces {
            feature: id("f-cyl1"),
            radius: 20.0,
            fillet: false,
        }),
    );

    assert_eq!(
        rendered(&error, "f-sh1"),
        "Shell thickness 30 mm is too thick for body 'Body 1': the largest that works is about 19 mm. Its rounded faces made by 'Cylinder 1' have radius 20 mm, smaller than the wall."
    );
    assert_eq!(
        fixes_of(&error, "f-sh1"),
        [use_value("thickness", "Thickness", "19 mm")]
    );
}

#[test]
fn a_shell_with_a_limit_and_no_round_faces_states_the_limit_alone() {
    let error = shell_too_thick("30", 30.0, Some(19.4), None);

    assert_eq!(
        rendered(&error, "f-sh1"),
        "Shell thickness 30 mm is too thick for body 'Body 1': the largest that works is about 19 mm."
    );
}

#[test]
fn a_shell_without_a_limit_or_round_faces_says_partsmith_could_not_tell_why() {
    let error = shell_too_thick("30", 30.0, None, None);

    assert_eq!(
        rendered(&error, "f-sh1"),
        "Shell thickness 30 mm does not work on body 'Body 1', even when thinner. Partsmith could not tell why."
    );
    assert_eq!(fixes_of(&error, "f-sh1"), [suppress("f-sh1")]);
}

#[test]
fn a_bare_thickness_in_an_inch_document_says_what_it_means_in_place_of_the_round_faces() {
    // '2' in an inch document is 50.8 mm; 19 mm is 0.748031 in, rounded down to 0.74 in.
    let error = shell_too_thick(
        "2",
        50.8,
        Some(19.0),
        Some(Limit::RoundFaces {
            feature: id("f-cyl1"),
            radius: 20.0,
            fillet: false,
        }),
    );
    let diagnosis = Diagnosis::default();

    let text = with_labels("f-sh1", Units::In, &diagnosis, |context| {
        error.render(context)
    });
    let fixes = with_labels("f-sh1", Units::In, &diagnosis, |context| {
        error.fixes(context)
    });

    assert_eq!(
        text,
        "Shell thickness 2 in is too thick for body 'Body 1': the largest that works is about 0.74 in. '2' has no unit, so it means 2 in in this document."
    );
    assert_eq!(
        fixes,
        [
            use_value("thickness", "Thickness", "2 mm"),
            use_value("thickness", "Thickness", "0.74 in"),
        ]
    );
    assert_eq!(error.suggestion(Units::In).as_deref(), Some("2 mm"));
}

// The message lint over every template of the rows above (section 6.5 test 14).

/// A face of `b-1` stored by name and fingerprint, for the samples.
fn stored_face(name: &str) -> Reference {
    stored(name, "plane|n=0,0,1|c=0,0,20|a=400")
}

/// One sample of every error row phase 3 rewrites, with the feature it is rendered on.
fn sample_errors() -> Vec<(FeatureError, &'static str)> {
    let asset = format!("{}.step", "0".repeat(64));
    let round_faces = |fillet: bool| Limit::RoundFaces {
        feature: id("f-fil1"),
        radius: 1.0,
        fillet,
    };
    let face_limit = |removable: bool| Limit::Face {
        position: 0,
        edge: stored(RISER_EDGE.0, RISER_EDGE.1),
        face: stored("f-box2/left", "plane|n=-1,0,0|c=0,0,8.8|a=48"),
        width: 2.4,
        removable,
    };
    let mut samples = vec![
        (FeatureError::TargetRequired { field: target() }, "f-mir1"),
        (FeatureError::TargetNotAllowed { field: target() }, "f-lp1"),
        (
            FeatureError::UnknownBody {
                field: field("body", "Body"),
                body: id("b-9"),
            },
            "f-fil1",
        ),
        (
            FeatureError::UnknownFeature {
                field: profile_sketch(),
                feature: id("f-sk9"),
            },
            "f-ext1",
        ),
        (
            FeatureError::ForwardReference {
                field: profile_sketch(),
                feature: id("f-sk1"),
            },
            "f-ext1",
        ),
        (
            FeatureError::UpstreamSuppressed {
                field: profile_sketch(),
                feature: id("f-sk1"),
            },
            "f-ext1",
        ),
        (
            FeatureError::AssetMissing {
                field: field("asset", "File"),
                asset,
            },
            "f-imp1",
        ),
        (
            FeatureError::BodyNotEmpty {
                body: id("b-1"),
                target: None,
            },
            "f-ext1",
        ),
        (
            FeatureError::BodyNotEmpty {
                body: id("b-2"),
                target: Some(target()),
            },
            "f-split1",
        ),
        (
            FeatureError::EmptyResult {
                body: id("b-1"),
                op: partsmith_features::BodyOp::Cut,
            },
            "f-ext1",
        ),
        (
            FeatureError::EmptyResult {
                body: id("b-1"),
                op: partsmith_features::BodyOp::Intersect,
            },
            "f-ext1",
        ),
        (
            FeatureError::NotPlanar {
                field: field("base.face", "Base plane"),
                element: stored("f-cyl1/side", "cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5"),
            },
            "f-dp1",
        ),
        (
            FeatureError::NotStraight {
                field: field("axis", "Axis"),
                element: stored(
                    "edge(f-cyl1/side|f-cyl1/top)",
                    "circle|n=0,0,1|c=0,0,10|l=31.415927|r=5",
                ),
            },
            "f-rev1",
        ),
        (
            FeatureError::NoAxis {
                field: field("axis", "Axis"),
                element: stored_face("f-box1/top"),
            },
            "f-rev1",
        ),
        (
            FeatureError::UnknownEntity {
                field: field("points[0]", "Points"),
                sketch: id("f-sk1"),
                entity: "e-p9".to_string(),
                noun: Noun::Point,
            },
            "f-hole1",
        ),
        (
            FeatureError::NotALine {
                field: field("axis", "Axis"),
                sketch: id("f-sk1"),
                entity: id("e-5"),
            },
            "f-rev1",
        ),
        (
            FeatureError::Kernel {
                error: make_box_failure(),
            },
            "f-box1",
        ),
        (
            FeatureError::ShellRemovesAll {
                body: id("b-1"),
                count: 6,
            },
            "f-sh1",
        ),
        (
            FeatureError::ThroughAllMisses {
                body: id("b-1"),
                reverse_hits: true,
            },
            "f-ext1",
        ),
        (
            FeatureError::ThroughAllMisses {
                body: id("b-1"),
                reverse_hits: false,
            },
            "f-ext1",
        ),
        (
            FeatureError::ThroughAllNeedsBody { body: id("b-1") },
            "f-ext1",
        ),
        (
            FeatureError::TooManyInstances {
                count: 20000,
                limit: 10000,
            },
            "f-lp1",
        ),
        (FeatureError::Canceled, "f-lp1"),
        (
            FeatureError::NotWholeNumber {
                field: field("direction.count", "Count"),
                value: 2.5,
            },
            "f-lp1",
        ),
        (
            length_out_of_range("size[0]", "Length", 1e200, Bound::AtMost(1_000_000.0)),
            "f-box1",
        ),
        (
            length_out_of_range(
                "translation[0]",
                "Move X",
                -1e7,
                Bound::AtLeast(-1_000_000.0),
            ),
            "f-box1",
        ),
        (
            length_out_of_range("extent.expr", "Distance", 1e-7, Bound::GreaterThan(0.0)),
            "f-ext1",
        ),
        (
            FeatureError::InvalidParams {
                field: Some(field("body", "Body")),
                detail: "the setting 'body' is missing".to_string(),
            },
            "f-ext1",
        ),
        (
            FeatureError::InvalidParams {
                field: None,
                detail: "the settings are not a JSON object".to_string(),
            },
            "f-ext1",
        ),
        (
            FeatureError::UnknownType {
                type_name: "gear".to_string(),
            },
            "f-ext1",
        ),
    ];
    for max in [Some(2.396), None] {
        for limit in [Some(face_limit(true)), Some(face_limit(false)), None] {
            samples.push((fillet_too_large(max, limit.clone()), "f-fil1"));
            samples.push((chamfer_too_large(true, max, limit.clone()), "f-cha1"));
            samples.push((chamfer_too_large(false, max, limit), "f-cha1"));
        }
        for limit in [Some(round_faces(true)), Some(round_faces(false)), None] {
            samples.push((shell_too_thick("2", 2.0, max, limit), "f-sh1"));
        }
    }
    samples
}

/// The diagnoses every sample is rendered with: none, and one with a root, a cause and a sketch that lost its face.
fn sample_diagnoses() -> Vec<Diagnosis> {
    let full = Diagnosis {
        root: Some(id("f-dp1")),
        lost_face: Some(id("f-sk2")),
        ..Diagnosis::default()
    };
    vec![Diagnosis::default(), full]
}

#[test]
fn t014_every_phase_3_error_template_passes_the_message_lint() {
    for units in [Units::Mm, Units::In] {
        for diagnosis in sample_diagnoses() {
            for (error, feature) in sample_errors() {
                let text = with_labels(feature, units, &diagnosis, |context| error.render(context));
                assert_passes_lint(&text);
            }
        }
    }
}

#[test]
fn the_display_of_every_phase_3_error_has_no_kernel_text() {
    // `Display` renders the text of `render` with ids for names (section 2.8); the kernel's text is in the detail only.
    for (error, _) in sample_errors() {
        let text = error.to_string();
        assert!(!text.contains("StdFail"), "{text}");
        assert!(!text.contains("BRep_API"), "{text}");
    }
}

#[test]
fn every_phase_3_error_is_an_error() {
    for (error, _) in sample_errors() {
        assert_eq!(error.severity(), Severity::Error, "{error:?}");
    }
}
