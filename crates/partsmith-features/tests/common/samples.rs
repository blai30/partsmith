//! One sample of every error and warning of features.md sections 3.15 and 4, each with the feature it is rendered on,
//! the builders they share with the message tests, the diagnoses they are rendered with, and the record of the sketch
//! the sketch crate's messages read. The message lint (section 6.5 test 14) renders every sample with
//! `LabelNames::standard`; the tests of a phase's rows iterate that phase's list.

use std::collections::BTreeMap;

use partsmith_features::{
    Because, Bound, Diagnosis, Explanation, FallbackCause, FeatureError, FeatureWarning, FieldRef,
    FieldUnit, Fix, IgnoredReason, ImportContent, Limit, Loss, LossCandidate, NoEffectHow, Noun,
    Placed, RegionLost,
};
use partsmith_model::{
    Dimension, EmptyContext, Expr, ExprError, FeatureRecord, ParameterValues, Scope, Units,
};
use partsmith_naming::{ElementKind, ElementName, Follow, Lost, Reference};
use partsmith_occt_sys::{KernelError, KernelErrorKind, Operation};
use partsmith_sketch::{
    DimensionValues, EntityKind, RegionLoss, RegionRef, SketchError, SolveIssue,
};
use serde_json::json;

use super::records::{BLOCK_TOP, RISER_EDGE};
use super::timeline::stored;
use super::{field, id, record, show};

/// The number of variants of `FeatureError`. `every_error` holds a sample of each; a new variant adds one and raises
/// this count.
pub const ERROR_VARIANTS: usize = 50;

/// The number of variants of `FeatureWarning`. `every_warning` holds a sample of each; a new variant adds one and
/// raises this count.
pub const WARNING_VARIANTS: usize = 20;

// Builders of the phase 1 and phase 2 rows.

/// The front top edge of `CUBE20` (features/fillet.md `E-FT`).
pub const FRONT_TOP: (&str, &str) = (
    "edge(f-box1/front|f-box1/top)",
    "line|n=1,0,0|c=0,-10,20|l=20",
);

/// A diagnosis with one explanation for the loss at position 0.
pub fn explained(text: &str, after: Option<&str>, fixes: Vec<Fix>, upstream: bool) -> Diagnosis {
    Diagnosis {
        losses: BTreeMap::from([(
            0,
            Explanation {
                text: text.to_string(),
                after: after.map(id),
                fixes,
                upstream,
            },
        )]),
        ..Diagnosis::default()
    }
}

/// The reason of section 6.5 test 3.
pub const CUT_AWAY: &str = "was cut away by 'Extrude 1', which comes before this fillet";

/// `BodyEmpty` of `b-1` at the fillet's Body.
pub fn fillet_body_empty(use_new: bool) -> FeatureError {
    FeatureError::BodyEmpty {
        field: field("body", "Body"),
        body: id("b-1"),
        use_new,
    }
}

/// `ToolEmpty` of `b-2` at a boolean's first tool.
pub fn tool_empty() -> FeatureError {
    FeatureError::ToolEmpty {
        field: field("tools[0]", "Tool bodies"),
        body: id("b-2"),
    }
}

/// A stored region reference.
pub fn region(key: &str, fingerprint: &str) -> RegionRef {
    serde_json::from_value(serde_json::json!({"key": key, "fp": fingerprint}))
        .expect("region references written in tests are valid")
}

/// The region of `RECT(0, 0, 80, 50)`.
pub fn rectangle_region() -> RegionRef {
    region("e-1,e-2,e-3,e-4", "c=40,25|a=4000")
}

/// The field of the region at `position` of a profile.
pub fn region_field(position: usize) -> FieldRef {
    field(&format!("profile.regions[{position}]"), "Profile")
}

/// A region warning of `f-sk1` at position 0.
pub fn region_split() -> FeatureWarning {
    FeatureWarning::RegionSplit {
        field: region_field(0),
        position: 0,
        sketch: id("f-sk1"),
        region: rectangle_region(),
        count: 2,
    }
}

/// A missing region at position 1 with `loss`.
pub fn region_missing(loss: RegionLoss) -> FeatureWarning {
    FeatureWarning::RegionMissing {
        field: region_field(1),
        position: 1,
        sketch: id("f-sk1"),
        region: rectangle_region(),
        loss,
    }
}

/// `ProfileNotFound` of `f-sk1` with these lost regions.
pub fn profile_not_found(regions: Vec<RegionLost>) -> FeatureError {
    FeatureError::ProfileNotFound {
        field: field("profile", "Profile"),
        sketch: id("f-sk1"),
        regions,
    }
}

/// A lost region at `position` with `loss` and `candidates`.
pub fn lost_region(
    position: usize,
    loss: Option<RegionLoss>,
    candidates: Vec<Vec<usize>>,
) -> RegionLost {
    RegionLost {
        position,
        region: rectangle_region(),
        loss,
        candidates,
    }
}

// Builders of the phase 3 rows.

/// The field `target` of mirror, patterns and split body.
pub fn target() -> FieldRef {
    field("target", "Target body")
}

/// The field `profile.sketch` of extrude and revolve.
pub fn profile_sketch() -> FieldRef {
    field("profile.sketch", "Profile")
}

/// The kernel failure of section 6.5 test 7.
pub fn make_box_failure() -> KernelError {
    KernelError {
        operation: Operation::MakeBox,
        kind: KernelErrorKind::Failed,
        exception: "StdFail_NotDone".to_string(),
        message: "BRep_API: command not done".to_string(),
    }
}

/// The kernel failure of a fillet.
pub fn fillet_failure() -> KernelError {
    KernelError {
        operation: Operation::Fillet,
        kind: KernelErrorKind::Failed,
        exception: "StdFail_NotDone".to_string(),
        message: "BRep_API: command not done".to_string(),
    }
}

/// An `OutOfRange` error of a length field.
pub fn length_out_of_range(path: &str, label: &str, value: f64, bound: Bound) -> FeatureError {
    FeatureError::OutOfRange {
        field: field(path, label),
        kind: FieldUnit::Length,
        value,
        bound,
        bound_name: None,
    }
}

/// `FilletTooLarge` of a 3 mm radius on `STEP` with this largest working radius and limit.
pub fn fillet_too_large(max: Option<f64>, limit: Option<Limit>) -> FeatureError {
    FeatureError::FilletTooLarge {
        field: field("radius", "Radius"),
        expr: Expr::new("3"),
        radius: 3.0,
        max,
        limit,
        cause: fillet_failure(),
    }
}

/// `ChamferTooLarge` of a 3 mm distance on `STEP`: an equal distance (field `distance.expr`, with its expression) or
/// two distances (field `distance`, without one).
pub fn chamfer_too_large(equal: bool, max: Option<f64>, limit: Option<Limit>) -> FeatureError {
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

/// `ShellTooThick` of `expr` (`thickness` mm) on `b-1` with this largest working wall and limit.
pub fn shell_too_thick(
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

/// A face of `b-1` stored by name and fingerprint, for the samples.
pub fn stored_face(name: &str) -> Reference {
    stored(name, "plane|n=0,0,1|c=0,0,20|a=400")
}

/// The top of `BLOCK`, the attach face of the newcomer error corpus NC02 and NC04.
pub fn block_top() -> Reference {
    stored(BLOCK_TOP.0, BLOCK_TOP.1)
}

/// `NoEffect` on `b-1` with `how`, a Reverse that does not reach the body, and the attach face `attach` of the sketch
/// `f-sk2`, when given.
pub fn no_effect(how: NoEffectHow, attach: Option<Reference>) -> FeatureWarning {
    FeatureWarning::NoEffect {
        body: id("b-1"),
        how,
        reverse_hits: false,
        attach: attach.map(|face| (id("f-sk2"), face)),
    }
}

/// `SeparateSolid` on `b-1` with 2 pieces, none of its tools joined, and the attach face of `f-sk2`, when given.
pub fn separate_solid(attach: Option<Reference>) -> FeatureWarning {
    FeatureWarning::SeparateSolid {
        body: id("b-1"),
        pieces: 2,
        part_joined: false,
        attach: attach.map(|face| (id("f-sk2"), face)),
    }
}

/// `HoleMissesBody` of the point `e-p3` of `sketch` at `position` on `b-1`.
pub fn hole_misses(sketch: &str, position: [f64; 2], reverse_hits: bool) -> FeatureWarning {
    FeatureWarning::HoleMissesBody {
        sketch: id(sketch),
        point: "e-p3".to_string(),
        position,
        body: id("b-1"),
        reverse_hits,
    }
}

/// `PointMissing` of the deleted point `e-p9` at `points[1]` of `f-sk1`.
pub fn point_missing() -> FeatureWarning {
    FeatureWarning::PointMissing {
        field: field("points[1]", "Points"),
        position: 1,
        sketch: id("f-sk1"),
        point: "e-p9".to_string(),
    }
}

/// `UnknownParams` of a shell with `keys`, of which `remove_face` nearly matches Faces to remove.
pub fn unknown_params(keys: &[&str]) -> FeatureWarning {
    let suggestions = keys
        .iter()
        .filter(|key| **key == "remove_face")
        .map(|key| (key.to_string(), field("remove_faces", "Faces to remove")))
        .collect();
    FeatureWarning::UnknownParams {
        keys: keys.iter().map(ToString::to_string).collect(),
        suggestions,
    }
}

/// A diagnosis whose profile or point sketch `f-sk2` lost its face in the same run (section 3.15).
pub fn lost_face() -> Diagnosis {
    Diagnosis {
        lost_face: Some(id("f-sk2")),
        ..Diagnosis::default()
    }
}

/// A diagnosis whose profile or point sketch `f-sk2` turned over on its face in the same run (section 3.15).
pub fn turned_over() -> Diagnosis {
    Diagnosis {
        turned_over: Some(id("f-sk2")),
        ..Diagnosis::default()
    }
}

// Builders of the rows no phase rewrote.

/// The sketch the samples of the sketch crate's messages are on, 'Sketch 3' in `LabelNames::standard`.
pub const SAMPLE_SKETCH: &str = "f-sk3";

/// The record of `SAMPLE_SKETCH`: a line with two horizontal constraints, the second redundant, and two distances that
/// conflict, and a circle.
pub fn sample_sketch_record() -> FeatureRecord {
    record(json!({
        "id": SAMPLE_SKETCH,
        "type": "sketch",
        "plane": {"kind": "origin", "plane": "XY"},
        "entities": [
            {"id": "e-1", "type": "line", "start": [0, 0], "end": [40, 0]},
            {"id": "e-2", "type": "circle", "center": [20, 10], "radius": 5}
        ],
        "constraints": [
            {"id": "c-1", "type": "horizontal", "line": "e-1"},
            {"id": "c-2", "type": "horizontal", "line": "e-1"},
            {"id": "c-3", "type": "distance", "line": "e-1", "expr": "40 mm"},
            {"id": "c-4", "type": "distance", "line": "e-1", "expr": "30 mm"}
        ]
    }))
}

/// The driving dimension values of the distances of `SAMPLE_SKETCH`.
fn sample_sketch_values() -> DimensionValues {
    DimensionValues::from([(id("c-3"), 40.0), (id("c-4"), 30.0)])
}

/// The evaluator's error for `heigth` in a document without parameters.
fn unknown_name() -> ExprError {
    let parameters = ParameterValues::default();
    let scope = Scope {
        units: Units::Mm,
        parameters: &parameters,
        context: &EmptyContext,
    };
    Expr::new("heigth")
        .evaluate(&scope, Some(Dimension::LENGTH))
        .expect_err("no parameter is named heigth")
}

// The samples.

/// One sample of every error of the phase 1 and phase 2 rows, with the feature it is rendered on.
pub fn phase_1_and_2_errors() -> Vec<(FeatureError, &'static str)> {
    let lost = |position: usize, ambiguous: bool| Loss {
        position,
        body: id("b-1"),
        reference: stored(FRONT_TOP.0, FRONT_TOP.1),
        kind: ElementKind::Edge,
        ambiguous,
        lost: Lost {
            parts: vec![ElementName::parse("f-box1/front").expect("a valid name")],
            ..Lost::default()
        },
        candidates: Vec::new(),
        matches: if ambiguous { 2 } else { 0 },
    };
    vec![
        (
            FeatureError::ReferencesNotFound {
                field: field("edges", "Edges"),
                kind: ElementKind::Edge,
                losses: vec![lost(0, false)],
            },
            "f-fil1",
        ),
        (
            FeatureError::ReferencesNotFound {
                field: field("edges", "Edges"),
                kind: ElementKind::Edge,
                losses: vec![lost(0, true)],
            },
            "f-fil1",
        ),
        (
            FeatureError::ReferencesNotFound {
                field: field("edges", "Edges"),
                kind: ElementKind::Edge,
                losses: vec![lost(0, false), lost(1, false)],
            },
            "f-fil1",
        ),
        (
            FeatureError::ReferencesNotFound {
                field: field("edges", "Edges"),
                kind: ElementKind::Edge,
                losses: vec![lost(0, true), lost(1, false)],
            },
            "f-fil1",
        ),
        (
            FeatureError::ReferenceNotFound {
                field: field("base.face", "Base plane"),
                loss: lost(0, false),
            },
            "f-dp1",
        ),
        (
            FeatureError::ReferenceNotFound {
                field: field("base.face", "Base plane"),
                loss: lost(0, true),
            },
            "f-dp1",
        ),
        (
            profile_not_found(vec![lost_region(0, Some(RegionLoss::Open), Vec::new())]),
            "f-ext1",
        ),
        (
            profile_not_found(vec![lost_region(0, None, vec![vec![0], vec![1]])]),
            "f-ext1",
        ),
        (
            profile_not_found(vec![
                lost_region(0, Some(RegionLoss::Redrawn), Vec::new()),
                lost_region(1, None, vec![vec![0], vec![1]]),
            ]),
            "f-ext1",
        ),
        (
            FeatureError::UpstreamFailed {
                feature: id("f-sk1"),
            },
            "f-ext1",
        ),
        (fillet_body_empty(false), "f-fil1"),
        (fillet_body_empty(true), "f-ext1"),
        (tool_empty(), "f-bool1"),
        (
            FeatureError::Internal {
                message: "called Option::unwrap() on a None value".to_string(),
            },
            "f-sh1",
        ),
        (
            FeatureError::UndeclaredDependency {
                what: "body b-9".to_string(),
            },
            "f-hole1",
        ),
        (FeatureError::NameTooDeep { length: 16400 }, "f-fil1"),
        (
            FeatureError::NotCoplanar {
                field: field("base", "Base plane"),
                body: id("b-1"),
                reference: stored("f-box1/top", "plane|n=0,0,1|c=-10,0,10|a=400"),
                faces: vec![
                    stored("f-box1/top", "plane|n=0,0,1|c=-10,0,10|a=400"),
                    stored("f-box2/top", "plane|n=0,0,1|c=10,0,20|a=400"),
                ],
            },
            "f-dp1",
        ),
        (
            FeatureError::NotCollinear {
                field: field("direction.axis", "Direction"),
                body: id("b-1"),
                reference: stored(
                    "edge(f-box1/front|f-box1/top)",
                    "line|n=1,0,0|c=-10,-10,10|l=20",
                ),
                elements: vec![
                    stored(
                        "edge(f-box1/front|f-box1/top)",
                        "line|n=1,0,0|c=-10,-10,10|l=20",
                    ),
                    stored(
                        "edge(f-box1/front|f-box2/top)",
                        "line|n=1,0,0|c=10,-10,20|l=20",
                    ),
                ],
                kind: ElementKind::Edge,
            },
            "f-ext1",
        ),
        (
            FeatureError::NotCollinear {
                field: field("axis", "Axis"),
                body: id("b-2"),
                reference: stored(
                    "f-cyl2/side",
                    "cylinder|n=0,0,1|c=-20,0,2.5|a=62.831853|r=2",
                ),
                elements: vec![
                    stored(
                        "f-cyl2/side",
                        "cylinder|n=0,0,1|c=-20,0,2.5|a=62.831853|r=2",
                    ),
                    stored(
                        "f-cyl3/side",
                        "cylinder|n=0,0,1|c=-30,0,2.5|a=62.831853|r=2",
                    ),
                ],
                kind: ElementKind::Face,
            },
            "f-ext1",
        ),
    ]
}

/// One sample of every warning of the phase 1 and phase 2 rows, with the feature it is rendered on.
pub fn phase_1_and_2_warnings() -> Vec<(FeatureWarning, &'static str)> {
    let reference: Reference = stored(FRONT_TOP.0, FRONT_TOP.1);
    let element = stored(
        "edge(f-box1/back|f-box1/top)",
        "line|n=1,0,0|c=0,10,20|l=20",
    );
    let face = |name: &str| ElementName::parse(name).expect("a valid name");
    let followed = |follow: Follow, reference: &Reference| FeatureWarning::ReferenceFollowed {
        field: field("edges[0]", "Edges"),
        position: 0,
        body: id("b-1"),
        reference: reference.clone(),
        elements: vec![element.clone()],
        follow,
        via: vec![face("f-box1/front")],
    };
    let vertex = stored(
        "vertex(f-box1/front|f-box1/left|f-box1/top)",
        "point|c=-10,-10,20",
    );
    let top = stored("f-box1/top", "plane|n=0,0,1|c=0,0,20|a=400");
    let mut samples = vec![
        (
            FeatureWarning::ReferenceLost {
                field: field("edges[0]", "Edges"),
                loss: Loss {
                    position: 0,
                    body: id("b-1"),
                    reference: reference.clone(),
                    kind: ElementKind::Edge,
                    ambiguous: false,
                    lost: Lost {
                        unmet: true,
                        ..Lost::default()
                    },
                    candidates: Vec::new(),
                    matches: 0,
                },
            },
            "f-fil1",
        ),
        (
            FeatureWarning::ReferenceLost {
                field: field("edges[0]", "Edges"),
                loss: Loss {
                    position: 0,
                    body: id("b-1"),
                    reference: reference.clone(),
                    kind: ElementKind::Edge,
                    ambiguous: true,
                    lost: Lost::default(),
                    candidates: Vec::new(),
                    matches: 2,
                },
            },
            "f-fil1",
        ),
        (
            FeatureWarning::ReferenceGuessed {
                field: field("edges[0]", "Edges"),
                position: 0,
                body: id("b-1"),
                reference: stored(
                    "edge(f-box1/front|f-box1/top#1)",
                    "line|n=1,0,0|c=0,-10,20|l=10",
                ),
                elements: vec![stored(
                    "edge(f-box1/front|f-box1/top#1)",
                    "line|n=1,0,0|c=-5,-10,20|l=10",
                )],
                alternatives: vec![LossCandidate {
                    reference: stored(
                        "edge(f-box1/front|f-box1/top#2)",
                        "line|n=1,0,0|c=5,-10,20|l=10",
                    ),
                    meaning: true,
                    distance: 5.0,
                    score: 0.9,
                }],
                score: 0.7,
            },
            "f-fil1",
        ),
        (
            FeatureWarning::ReferenceGuessed {
                field: field("edges[0]", "Edges"),
                position: 0,
                body: id("b-1"),
                reference: stored(
                    "edge(f-box1/front|f-box1/top#1)",
                    "line|n=1,0,0|c=0,-10,20|l=10",
                ),
                elements: vec![stored(
                    "edge(f-box1/front|f-box1/top#1)",
                    "line|n=1,0,0|c=-5,-10,20|l=10",
                )],
                alternatives: Vec::new(),
                score: 0.7,
            },
            "f-fil1",
        ),
        (
            FeatureWarning::ReferenceGuessed {
                field: field("edges[0]", "Edges"),
                position: 0,
                body: id("b-1"),
                reference: stored(
                    "edge(f-box1/front|f-box1/top#3)",
                    "line|n=1,0,0|c=0,-10,20|l=10",
                ),
                elements: vec![stored(
                    "edge(f-box1/front|f-box1/top#2)",
                    "line|n=1,0,0|c=5,-10,20|l=10",
                )],
                alternatives: Vec::new(),
                score: 0.9,
            },
            "f-fil1",
        ),
        (
            FeatureWarning::ReferenceWidened {
                field: field("edges[0]", "Edges"),
                position: 0,
                body: id("b-1"),
                reference: reference.clone(),
                elements: vec![element.clone(), top.clone()],
                via: vec![face("f-box1/front")],
                first: element.clone(),
            },
            "f-fil1",
        ),
        (region_split(), "f-ext1"),
        (
            FeatureWarning::RegionResized {
                field: region_field(0),
                position: 0,
                sketch: id("f-sk1"),
                region: rectangle_region(),
                count: 2,
            },
            "f-ext1",
        ),
        (
            FeatureWarning::RegionRemapped {
                field: region_field(0),
                position: 0,
                sketch: id("f-sk1"),
                region: rectangle_region(),
            },
            "f-ext1",
        ),
        (
            FeatureWarning::RegionAmbiguous {
                field: region_field(0),
                position: 0,
                sketch: id("f-sk1"),
                region: rectangle_region(),
                candidates: vec![vec![0], vec![1]],
            },
            "f-ext1",
        ),
        (region_missing(RegionLoss::Redrawn), "f-ext1"),
        (region_missing(RegionLoss::Open), "f-ext1"),
        (
            region_missing(RegionLoss::Deleted {
                entities: vec![id("e-3")],
            }),
            "f-ext1",
        ),
    ];
    for follow in [
        Follow::Replaced,
        Follow::Extended,
        Follow::Merged,
        Follow::Continued,
        Follow::Related,
    ] {
        samples.push((followed(follow, &reference), "f-fil1"));
    }
    samples.push((followed(Follow::Continued, &top), "f-fil1"));
    let lost_top = |ambiguous: bool| Loss {
        position: 0,
        body: id("b-1"),
        reference: top.clone(),
        kind: ElementKind::Face,
        ambiguous,
        lost: Lost::default(),
        candidates: Vec::new(),
        matches: 2,
    };
    for (loss, cause, placed) in [
        (Some(lost_top(false)), FallbackCause::Missing, Placed::Saved),
        (
            Some(lost_top(false)),
            FallbackCause::Missing,
            Placed::BeforeRemoval { by: id("f-ext3") },
        ),
        (None, FallbackCause::Missing, Placed::Saved),
        (None, FallbackCause::NotPlanar, Placed::Saved),
        (
            Some(lost_top(true)),
            FallbackCause::Ambiguous,
            Placed::Saved,
        ),
        (
            Some(lost_top(true)),
            FallbackCause::NotCoplanar,
            Placed::Saved,
        ),
    ] {
        let warning = FeatureWarning::PlaneFallback {
            field: field("plane.face", "Plane"),
            body: id("b-1"),
            reference: top.clone(),
            loss,
            cause,
            placed,
        };
        samples.push((warning, "f-sk2"));
    }
    samples.push((followed(Follow::Continued, &vertex), "f-fil1"));
    samples.push((
        FeatureWarning::SketchTurnedOver {
            field: field("plane.face", "Plane"),
            body: id("b-1"),
            reference: top.clone(),
        },
        "f-sk2",
    ));
    for matches in [0, 1] {
        samples.push((
            FeatureWarning::ReferenceLost {
                field: field("edges[0]", "Edges"),
                loss: Loss {
                    position: 0,
                    body: id("b-1"),
                    reference: reference.clone(),
                    kind: ElementKind::Edge,
                    ambiguous: true,
                    lost: Lost::default(),
                    candidates: Vec::new(),
                    matches,
                },
            },
            "f-fil1",
        ));
    }
    samples
}

/// One sample of every error row phase 3 rewrites, with the feature it is rendered on.
pub fn phase_3_errors() -> Vec<(FeatureError, &'static str)> {
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
                new_body: true,
            },
            "f-ext1",
        ),
        (
            FeatureError::BodyNotEmpty {
                body: id("b-2"),
                target: Some(target()),
                new_body: false,
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

/// One sample of every warning row phase 3 adds or rewrites, with the feature it is rendered on.
pub fn phase_3_warnings() -> Vec<(FeatureWarning, &'static str)> {
    let mut samples = Vec::new();
    for (how, feature) in [
        (NoEffectHow::Inside, "f-ext2"),
        (NoEffectHow::Inside, "f-mir1"),
        (NoEffectHow::Inside, "f-lp1"),
        (NoEffectHow::Away { gap: 10.0 }, "f-ext2"),
        (NoEffectHow::Away { gap: 0.0 }, "f-ext2"),
        (NoEffectHow::Short { gap: 5.0 }, "f-ext2"),
        (NoEffectHow::Short { gap: 0.0 }, "f-ext2"),
        (NoEffectHow::Beside, "f-bool1"),
        (NoEffectHow::Covers, "f-box2"),
        (
            NoEffectHow::Skipped {
                field: field("edges", "Edges"),
            },
            "f-fil1",
        ),
        (NoEffectHow::Missed, "f-hole1"),
        (
            NoEffectHow::NoCopies {
                fields: vec![field("direction.count", "Count")],
            },
            "f-lp1",
        ),
        (
            NoEffectHow::NoCopies {
                fields: vec![
                    field("direction.count", "Count"),
                    field("second_direction.count", "Second count"),
                ],
            },
            "f-lp1",
        ),
        (NoEffectHow::NotMoved, "f-mv1"),
    ] {
        samples.push((no_effect(how.clone(), Some(block_top())), feature));
        samples.push((
            FeatureWarning::NoEffect {
                body: id("b-1"),
                how,
                reverse_hits: true,
                attach: None,
            },
            feature,
        ));
    }
    samples.push((separate_solid(Some(block_top())), "f-ext2"));
    samples.push((separate_solid(None), "f-box2"));
    samples.push((FeatureWarning::ClosedVoid { body: id("b-1") }, "f-box3"));
    samples.push((hole_misses("f-sk1", [100.0, 0.0], false), "f-hole1"));
    samples.push((hole_misses("f-sk2", [20.0, 10.0], true), "f-hole1"));
    samples.push((point_missing(), "f-hole1"));
    samples.push((
        FeatureWarning::Ignored {
            field: field("edges[1]", "Edges"),
            reason: IgnoredReason::SmoothEdge,
        },
        "f-fil1",
    ));
    for keys in [
        &["remove_face"][..],
        &["zzz"][..],
        &["remove_face", "zzz"][..],
    ] {
        samples.push((unknown_params(keys), "f-sh1"));
    }
    samples
}

/// One sample of every variant of `FeatureError` (`ERROR_VARIANTS`), with the feature it is rendered on: the samples of
/// both phases' lists and of the rows no phase rewrote.
pub fn every_error() -> Vec<(FeatureError, &'static str)> {
    let mut samples = phase_1_and_2_errors();
    samples.extend(phase_3_errors());
    samples.extend([
        (
            FeatureError::EmptySelection {
                field: field("edges", "Edges"),
                noun: Noun::Edge,
            },
            "f-fil1",
        ),
        (
            FeatureError::SameBody {
                field: field("tools[0]", "Tool bodies"),
                body: id("b-1"),
            },
            "f-bool1",
        ),
        (
            FeatureError::Expression {
                field: field("extent.expr", "Distance"),
                expr: Expr::new("heigth"),
                error: unknown_name(),
            },
            "f-ext1",
        ),
        (
            FeatureError::WrongFeatureType {
                field: profile_sketch(),
                feature: id("f-dp1"),
                expected: Noun::Sketch,
            },
            "f-ext1",
        ),
        (FeatureError::ProfileCrossesAxis, "f-rev1"),
        (FeatureError::AxisNotInPlane, "f-rev1"),
        (FeatureError::BothSidesZero, "f-ext1"),
        (FeatureError::ConeRadiiZero, "f-cone1"),
        (
            FeatureError::PlaneMissesBody { body: id("b-1") },
            "f-split1",
        ),
        (
            FeatureError::SketchInvalid {
                error: SketchError::NegativeRadius {
                    entity: id("e-2"),
                    kind: EntityKind::Circle,
                    radius: -5.0,
                },
                values: DimensionValues::new(),
            },
            SAMPLE_SKETCH,
        ),
        (
            FeatureError::SketchConflict {
                constraints: vec![id("c-3"), id("c-4")],
                values: sample_sketch_values(),
            },
            SAMPLE_SKETCH,
        ),
        (FeatureError::SketchNotSolved, SAMPLE_SKETCH),
    ]);
    samples
}

/// One sample of every variant of `FeatureWarning` (`WARNING_VARIANTS`), with the feature it is rendered on: the samples
/// of both phases' lists and of the rows no phase rewrote.
pub fn every_warning() -> Vec<(FeatureWarning, &'static str)> {
    let mut samples = phase_1_and_2_warnings();
    samples.extend(phase_3_warnings());
    samples.extend([(
        FeatureWarning::RedundantConstraints {
            issues: vec![SolveIssue::Redundant {
                constraint: id("c-2"),
                type_name: "horizontal",
                dimension: false,
            }],
            values: sample_sketch_values(),
        },
        SAMPLE_SKETCH,
    )]);
    samples.extend(
        import_warnings()
            .into_iter()
            .map(|warning| (warning, IMPORT_FEATURE)),
    );
    samples
}

/// The import feature the `ImportWarning` samples are rendered on, `'Import 1'` (section 6.5 test 18).
pub const IMPORT_FEATURE: &str = "f-imp1";

/// An `ImportWarning` counting `count` solids or shapes of `content`, with `detail`.
pub fn import_warning(content: ImportContent, count: usize, detail: &str) -> FeatureWarning {
    FeatureWarning::ImportWarning {
        content,
        count,
        detail: detail.to_string(),
    }
}

/// The four `ImportWarning` samples of section 6.5 test 18, each with its own detail: two invalid solids, one, three
/// skipped shapes, one.
pub fn import_warnings() -> Vec<FeatureWarning> {
    vec![
        import_warning(
            ImportContent::InvalidSolids,
            2,
            "read_step: Solid 1 is not valid after repair; booleans with it may fail. Solid 2 is not valid after repair; booleans with it may fail.",
        ),
        import_warning(
            ImportContent::InvalidSolids,
            1,
            "read_step: Solid 1 is not valid after repair; booleans with it may fail.",
        ),
        import_warning(
            ImportContent::SkippedShapes,
            3,
            "read_step: Skipped 3 shapes that are not closed solids.",
        ),
        import_warning(
            ImportContent::SkippedShapes,
            1,
            "read_step: Skipped 1 shape that is not a closed solid.",
        ),
    ]
}

/// The diagnoses the phase 1 and phase 2 rows are rendered with: none, one that explains everything a phase 1 message
/// reads, and one whose loss is explained as upstream.
pub fn phase_1_and_2_diagnoses() -> Vec<Diagnosis> {
    let mut full = explained(CUT_AWAY, Some("f-box2"), vec![show("f-sk1")], false);
    full.root = Some(id("f-dp1"));
    full.because = Some(Because::Failed(id("f-ext1")));
    full.by = Some(id("f-sk1"));
    let mut upstream = explained(
        "is missing because 'Extrude 1' failed",
        None,
        vec![show("f-ext1")],
        true,
    );
    upstream.because = Some(Because::UsedAsTool {
        feature: id("f-bool1"),
        target: id("b-2"),
    });
    vec![Diagnosis::default(), full, upstream]
}

/// Every diagnosis a sample is rendered with: those of `phase_1_and_2_diagnoses`, one with a root and a sketch that lost
/// its face, and the profile or point sketch `f-sk2` that lost its face or turned over (section 3.15).
pub fn every_diagnosis() -> Vec<Diagnosis> {
    let mut diagnoses = phase_1_and_2_diagnoses();
    diagnoses.extend([
        Diagnosis {
            root: Some(id("f-dp1")),
            lost_face: Some(id("f-sk2")),
            ..Diagnosis::default()
        },
        lost_face(),
        turned_over(),
    ]);
    diagnoses
}
