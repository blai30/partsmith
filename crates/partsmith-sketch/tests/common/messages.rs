//! Message contexts and rendering helpers for the message tests (spec sections 4, 6.10 and 6.20).

use partsmith_model::Units;
use partsmith_sketch::{MessageContext, SketchError, SolveResult, build_profile};

use super::Sketch;
use super::lint::assert_passes_lint;

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
        solved: Some(&result.entities),
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
#[track_caller]
pub fn assert_plain_message(message: &str, sketch: &Sketch) {
    assert_passes_lint(message);
    let ids = sketch
        .params
        .entities
        .iter()
        .map(|entity| &entity.id)
        .chain(sketch.params.constraints.iter().map(|constraint| &constraint.id));
    for id in ids {
        assert!(
            !contains_word(message, id.as_str()),
            "the message {message:?} names the id {id}"
        );
    }
}
