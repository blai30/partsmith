//! Message contexts and rendering helpers for the message tests (spec sections 4, 6.10 and 6.20).

use partsmith_model::Units;
use partsmith_sketch::{
    EntityKind, MessageContext, ORIGIN, PlacementProblem, SketchError, SketchFix, SolveResult,
    X_AXIS, Y_AXIS, build_profile,
};

use super::lint::assert_passes_lint;
use super::{Sketch, distance_line, id, parallel, point_ref, rect_with_circle_and_point};

/// The label of the sketch in every message test.
pub const LABEL: &str = "Sketch 1";

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
    let delete = || {
        vec![SketchFix::DeleteConstraint {
            constraint: id("c-9"),
        }]
    };
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
            delete(),
        ),
        (
            SketchError::ReservedMisuse {
                constraint: id("c-9"),
                type_name: "parallel",
                reference: "origin".to_owned(),
            },
            delete(),
        ),
        (
            SketchError::InvalidPointRole {
                constraint: id("c-9"),
                point: point_ref("e-1.center"),
                kind: EntityKind::Line,
            },
            delete(),
        ),
        (
            SketchError::WrongEntityKind {
                constraint: id("c-9"),
                type_name: "parallel",
                entity: id("e-5"),
                found: EntityKind::Circle,
                expected: "a line",
            },
            delete(),
        ),
        (
            SketchError::SameEntity {
                constraint: id("c-9"),
                type_name: "parallel",
                entity: id("e-1"),
            },
            delete(),
        ),
        (
            SketchError::SamePoint {
                constraint: id("c-9"),
                type_name: "parallel",
                point: point_ref("e-1.start"),
            },
            delete(),
        ),
        (
            SketchError::PointOnOwnEntity {
                constraint: id("c-9"),
                type_name: "parallel",
                point: point_ref("e-1.start"),
                entity: id("e-1"),
            },
            delete(),
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
