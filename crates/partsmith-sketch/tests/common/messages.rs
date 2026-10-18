//! Message contexts and rendering helpers for the message tests (spec sections 4, 6.10 and 6.20).

use partsmith_message_lint::assert_passes_lint;
use partsmith_model::Units;
use partsmith_sketch::{
    Connection, EntityKind, MessageContext, ORIGIN, PlacementProblem, Profile, ProfileFinding,
    SketchError, SketchFix, SolveIssue, SolveResult, X_AXIS, Y_AXIS, build_profile,
};

use super::{Sketch, distance_line, id, parallel, point_ref, rect_with_circle_and_point};

/// The label of the sketch in every message test.
pub const LABEL: &str = "Sketch 1";

/// `DeleteConstraint` of this constraint.
pub fn delete(constraint: &str) -> SketchFix {
    SketchFix::DeleteConstraint {
        constraint: id(constraint),
    }
}

/// `MakeReference` of this constraint.
pub fn make_reference(constraint: &str) -> SketchFix {
    SketchFix::MakeReference {
        constraint: id(constraint),
    }
}

/// The `fixes()` of every finding of the profile, in order.
pub fn fixes_of(profile: &Profile) -> Vec<Vec<SketchFix>> {
    profile.findings.iter().map(ProfileFinding::fixes).collect()
}

/// The context of section 6.10: label "Sketch 1", the sketch as stored, no solve, the sketch's values, no measured
/// values, millimeters.
pub fn context(sketch: &Sketch) -> MessageContext<'_> {
    MessageContext {
        label: LABEL,
        sketch: &sketch.params,
        solved: None,
        values: &sketch.values,
        measured: None,
        units: Units::Mm,
    }
}

/// The context of section 6.10 with `solved` the entities of `result` (test 145).
pub fn solved_context<'a>(sketch: &'a Sketch, result: &'a SolveResult) -> MessageContext<'a> {
    MessageContext {
        solved: Some(result.entities.as_slice()),
        ..context(sketch)
    }
}

/// The error that validating or solving the sketch gives, panicking when it solves.
pub fn sketch_error(sketch: &Sketch) -> SketchError {
    match sketch.validate().and_then(|()| sketch.try_solve()) {
        Err(error) => error,
        Ok(result) => panic!("the sketch fails, but it solves to {:?}", result.status),
    }
}

/// The message of the error that validating or solving the sketch gives (context of section 6.10).
pub fn error_message(sketch: &Sketch) -> String {
    sketch_error(sketch).render(&context(sketch))
}

/// The messages of the issues of solving the sketch, in the context of test 145.
pub fn issue_messages(sketch: &Sketch) -> Vec<String> {
    let result = sketch.solve();
    let context = solved_context(sketch, &result);
    result
        .issues(&sketch.params)
        .iter()
        .map(|issue| issue.render(&context))
        .collect()
}

/// The messages of the profile findings of the stored entities (context of section 6.10).
pub fn finding_messages(sketch: &Sketch) -> Vec<String> {
    let context = context(sketch);
    sketch
        .profile()
        .findings
        .iter()
        .map(|finding| finding.render(&context))
        .collect()
}

/// Every message the sketch gives: the error of validating or solving it, or else the messages of its solve
/// issues and of the profile findings of the solved entities.
pub fn rendered_messages(sketch: &Sketch) -> Vec<String> {
    let result = match sketch.validate().and_then(|()| sketch.try_solve()) {
        Err(error) => return vec![error.render(&context(sketch))],
        Ok(result) => result,
    };
    let context = MessageContext {
        measured: Some(&result.measured),
        ..solved_context(sketch, &result)
    };
    let mut messages: Vec<String> = result
        .issues(&sketch.params)
        .iter()
        .map(|issue| issue.render(&context))
        .collect();
    messages.extend(
        build_profile(&result.entities)
            .findings
            .iter()
            .map(|finding| finding.render(&context)),
    );
    messages
}

/// The sketch every hand-built error of `every_error` is rendered on: `RECT` plus `circle e-5 c(40,25) r5`,
/// `point e-7 (1,1)`, `c-9 parallel [e-1, e-3]` and `c-10 distance e-1 = 80`.
pub fn every_error_sketch() -> Sketch {
    rect_with_circle_and_point()
        .with("c-9", parallel("e-1", "e-3"))
        .with_value("c-10", distance_line("e-1", "80 mm"), 80.0)
}

/// One error of every `SketchError` variant, each with the fixes section 4.1 gives it, on `every_error_sketch`.
pub fn every_error() -> Vec<(SketchError, Vec<SketchFix>)> {
    vec![
        (
            SketchError::InvalidPointRef {
                text: "e-1.mid".to_owned(),
            },
            Vec::new(),
        ),
        (SketchError::DuplicateId { id: id("e-1") }, Vec::new()),
        (SketchError::ReservedId { id: id("origin") }, Vec::new()),
        (
            SketchError::InvalidPlacement {
                reason: PlacementProblem::NormalNotUnit,
            },
            Vec::new(),
        ),
        (
            SketchError::NonFiniteGeometry { entity: id("e-1") },
            Vec::new(),
        ),
        (
            SketchError::NegativeRadius {
                entity: id("e-5"),
                kind: EntityKind::Circle,
                radius: -1.0,
            },
            Vec::new(),
        ),
        (
            SketchError::UnknownEntity {
                constraint: id("c-9"),
                entity: id("e-9"),
            },
            vec![delete("c-9")],
        ),
        (
            SketchError::ReservedMisuse {
                constraint: id("c-9"),
                type_name: "parallel",
                reference: "origin".to_owned(),
            },
            vec![delete("c-9")],
        ),
        (
            SketchError::InvalidPointRole {
                constraint: id("c-9"),
                point: point_ref("e-1.center"),
                kind: EntityKind::Line,
            },
            vec![delete("c-9")],
        ),
        (
            SketchError::WrongEntityKind {
                constraint: id("c-9"),
                type_name: "parallel",
                entity: id("e-5"),
                found: EntityKind::Circle,
                expected: "a line",
            },
            vec![delete("c-9")],
        ),
        (
            SketchError::SameEntity {
                constraint: id("c-9"),
                type_name: "parallel",
                entity: id("e-1"),
            },
            vec![delete("c-9")],
        ),
        (
            SketchError::SamePoint {
                constraint: id("c-9"),
                type_name: "parallel",
                point: point_ref("e-1.start"),
            },
            vec![delete("c-9")],
        ),
        (
            SketchError::PointOnOwnEntity {
                constraint: id("c-9"),
                type_name: "parallel",
                point: point_ref("e-1.start"),
                entity: id("e-1"),
            },
            vec![delete("c-9")],
        ),
        (
            SketchError::MissingValue {
                constraint: id("c-10"),
                type_name: "distance",
            },
            Vec::new(),
        ),
        (
            SketchError::NonFiniteValue {
                constraint: id("c-10"),
                type_name: "distance",
            },
            Vec::new(),
        ),
        (
            SketchError::ValueNotPositive {
                constraint: id("c-10"),
                type_name: "distance",
                value: 0.0,
                points: true,
            },
            Vec::new(),
        ),
        (
            SketchError::ValueNegative {
                constraint: id("c-10"),
                value: -1.0,
            },
            Vec::new(),
        ),
        (
            SketchError::UnknownDragTarget {
                target: "e-9.start".to_owned(),
            },
            Vec::new(),
        ),
        (SketchError::NonFiniteCursor, Vec::new()),
        (
            SketchError::InvalidRegionKey {
                text: String::new(),
            },
            Vec::new(),
        ),
        (
            SketchError::InvalidFingerprint {
                text: "c=1,2".to_owned(),
            },
            Vec::new(),
        ),
    ]
}

/// One issue of every `SolveIssue` variant on `every_error_sketch`, with conflicts of two constraints and of one, and
/// redundant constraints with and without a value.
pub fn every_issue() -> Vec<SolveIssue> {
    vec![
        SolveIssue::Conflicting {
            constraints: vec![id("c-9"), id("c-10")],
        },
        SolveIssue::Conflicting {
            constraints: vec![id("c-9")],
        },
        SolveIssue::Redundant {
            constraint: id("c-9"),
            type_name: "parallel",
            dimension: false,
        },
        SolveIssue::Redundant {
            constraint: id("c-10"),
            type_name: "distance",
            dimension: true,
        },
        SolveIssue::NotConverged,
    ]
}

/// One finding of every `ProfileFinding` variant on `every_error_sketch`, the open end with a connection.
pub fn every_finding() -> Vec<ProfileFinding> {
    vec![
        ProfileFinding::OpenEndpoint {
            entity: id("e-4"),
            point: [0.0, 0.0],
            connect: Some(Connection {
                first: point_ref("e-4.end"),
                second: point_ref("e-1.start"),
                gap: 0.3,
            }),
        },
        ProfileFinding::Overlap {
            entities: [id("e-1"), id("e-3")],
            start: [20.0, 0.0],
            end: [60.0, 0.0],
        },
        ProfileFinding::SelfIntersection {
            entities: [id("e-1"), id("e-2")],
            point: [80.0, 0.0],
        },
        ProfileFinding::ZeroLength { entity: id("e-5") },
    ]
}

/// The number of `SketchError` variants.
pub const ERROR_VARIANTS: usize = 21;

/// The position of the error's variant among the `ERROR_VARIANTS` of `SketchError`. The match has no wildcard arm, so
/// a new variant does not compile until it is counted here and given a sample in `every_error`.
pub fn error_variant(error: &SketchError) -> usize {
    match error {
        SketchError::InvalidPointRef { .. } => 0,
        SketchError::DuplicateId { .. } => 1,
        SketchError::ReservedId { .. } => 2,
        SketchError::InvalidPlacement { .. } => 3,
        SketchError::NonFiniteGeometry { .. } => 4,
        SketchError::NegativeRadius { .. } => 5,
        SketchError::UnknownEntity { .. } => 6,
        SketchError::ReservedMisuse { .. } => 7,
        SketchError::InvalidPointRole { .. } => 8,
        SketchError::WrongEntityKind { .. } => 9,
        SketchError::SameEntity { .. } => 10,
        SketchError::SamePoint { .. } => 11,
        SketchError::PointOnOwnEntity { .. } => 12,
        SketchError::MissingValue { .. } => 13,
        SketchError::NonFiniteValue { .. } => 14,
        SketchError::ValueNotPositive { .. } => 15,
        SketchError::ValueNegative { .. } => 16,
        SketchError::UnknownDragTarget { .. } => 17,
        SketchError::NonFiniteCursor => 18,
        SketchError::InvalidRegionKey { .. } => 19,
        SketchError::InvalidFingerprint { .. } => 20,
    }
}

/// The number of `SolveIssue` variants.
pub const ISSUE_VARIANTS: usize = 3;

/// The position of the issue's variant among the `ISSUE_VARIANTS` of `SolveIssue`, with no wildcard arm.
pub fn issue_variant(issue: &SolveIssue) -> usize {
    match issue {
        SolveIssue::Conflicting { .. } => 0,
        SolveIssue::Redundant { .. } => 1,
        SolveIssue::NotConverged => 2,
    }
}

/// The number of `ProfileFinding` variants.
pub const FINDING_VARIANTS: usize = 4;

/// The position of the finding's variant among the `FINDING_VARIANTS` of `ProfileFinding`, with no wildcard arm.
pub fn finding_variant(finding: &ProfileFinding) -> usize {
    match finding {
        ProfileFinding::OpenEndpoint { .. } => 0,
        ProfileFinding::Overlap { .. } => 1,
        ProfileFinding::SelfIntersection { .. } => 2,
        ProfileFinding::ZeroLength { .. } => 3,
    }
}

/// True when `word` occurs in `text` with no id character (letter, digit, `_` or `-`) right before or after it.
fn contains_word(text: &str, word: &str) -> bool {
    let is_id_character =
        |character: char| character.is_ascii_alphanumeric() || character == '_' || character == '-';
    text.match_indices(word).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        let after = text[start + word.len()..].chars().next();
        !before.is_some_and(is_id_character) && !after.is_some_and(is_id_character)
    })
}

/// Asserts that the message passes the message lint and names no entity or constraint id of the sketch (test 247).
///
/// Ids that are reserved names (test 202 names a point `origin`) are words of messages ("the origin"), so they are
/// not checked.
#[track_caller]
pub fn assert_plain_message(message: &str, sketch: &Sketch) {
    assert_passes_lint(message);
    let ids = sketch
        .params
        .entities
        .iter()
        .map(|entity| &entity.id)
        .chain(
            sketch
                .params
                .constraints
                .iter()
                .map(|constraint| &constraint.id),
        )
        .filter(|id| ![ORIGIN, X_AXIS, Y_AXIS].contains(&id.as_str()));
    for id in ids {
        assert!(
            !contains_word(message, id.as_str()),
            "the message {message:?} names the id {id}"
        );
    }
}
