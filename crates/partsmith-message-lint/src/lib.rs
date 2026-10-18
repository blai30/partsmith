//! The message lint of the error guidelines (docs/guidelines/errors.md, "Checklist"): no rendered
//! message matches `repair panel`, `\bop (new|join|cut|intersect)\b`, `\b[fbec]-[0-9a-z]+\b`,
//! `(Edge|Face|Vertex|Curve) [0-9]`, `[0-9]+% match`, a backtick or `os error`.
//!
//! The kernel tests (kernel.md test 130), the sketch tests (sketch.md test 247), the features tests
//! (features.md section 3.12) and the recompute tests run it over every message they render. The
//! crate has no dependencies and is a dev-dependency of tests only (ADR 0027).

/// The lint rules a message breaks, by the pattern each one checks; empty when it passes.
pub fn lint_failures(message: &str) -> Vec<&'static str> {
    let mut failures = Vec::new();
    if message.contains("repair panel") {
        failures.push("repair panel");
    }
    if mentions_an_op(message) {
        failures.push(r"\bop (new|join|cut|intersect)\b");
    }
    if mentions_an_id(message) {
        failures.push(r"\b[fbec]-[0-9a-z]+\b");
    }
    if mentions_a_numbered_element(message) {
        failures.push("(Edge|Face|Vertex|Curve) [0-9]");
    }
    if mentions_a_match_percentage(message) {
        failures.push("[0-9]+% match");
    }
    if message.contains('`') {
        failures.push("a backtick");
    }
    if message.contains("os error") {
        failures.push("os error");
    }
    failures
}

/// Asserts that `message` passes the message lint.
#[track_caller]
pub fn assert_passes_lint(message: &str) {
    let failures = lint_failures(message);
    assert!(
        failures.is_empty(),
        "the message {message:?} breaks the message lint: {failures:?}"
    );
}

/// True for a character a regular expression's `\w` matches.
fn is_word(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

/// True when the word starting at byte `start` is not preceded by a word character.
fn starts_word(text: &str, start: usize) -> bool {
    !text[..start].chars().next_back().is_some_and(is_word)
}

/// True when byte `end` is not followed by a word character.
fn ends_word(text: &str, end: usize) -> bool {
    !text[end..].chars().next().is_some_and(is_word)
}

/// `\bop (new|join|cut|intersect)\b`.
fn mentions_an_op(text: &str) -> bool {
    ["op new", "op join", "op cut", "op intersect"]
        .iter()
        .any(|phrase| {
            text.match_indices(phrase)
                .any(|(start, _)| starts_word(text, start) && ends_word(text, start + phrase.len()))
        })
}

/// `\b[fbec]-[0-9a-z]+\b`: a feature, body, entity or constraint id.
fn mentions_an_id(text: &str) -> bool {
    let characters: Vec<(usize, char)> = text.char_indices().collect();
    characters
        .iter()
        .enumerate()
        .any(|(position, &(start, letter))| {
            if !matches!(letter, 'f' | 'b' | 'e' | 'c') || !starts_word(text, start) {
                return false;
            }
            if characters.get(position + 1).map(|&(_, next)| next) != Some('-') {
                return false;
            }
            let run: Vec<&(usize, char)> = characters[position + 2..]
                .iter()
                .take_while(|(_, character)| {
                    character.is_ascii_digit() || character.is_ascii_lowercase()
                })
                .collect();
            match run.last() {
                None => false,
                Some(&&(last, character)) => ends_word(text, last + character.len_utf8()),
            }
        })
}

/// `(Edge|Face|Vertex|Curve) [0-9]`: an element named by its number.
fn mentions_a_numbered_element(text: &str) -> bool {
    ["Edge ", "Face ", "Vertex ", "Curve "].iter().any(|word| {
        text.match_indices(word).any(|(start, _)| {
            text[start + word.len()..]
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_digit())
        })
    })
}

/// `[0-9]+% match`.
fn mentions_a_match_percentage(text: &str) -> bool {
    text.match_indices("% match").any(|(start, _)| {
        text[..start]
            .chars()
            .next_back()
            .is_some_and(|character| character.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_lint_accepts_plain_words() {
        assert!(lint_failures(
            "The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet."
        )
        .is_empty());
    }

    #[test]
    fn the_message_lint_rejects_ids_numbered_elements_ops_scores_and_the_repair_panel() {
        assert_eq!(
            lint_failures("Edge 12 of f-box1 is lost."),
            [r"\b[fbec]-[0-9a-z]+\b", "(Edge|Face|Vertex|Curve) [0-9]"]
        );
        assert_eq!(
            lint_failures("Use op new."),
            [r"\bop (new|join|cut|intersect)\b"]
        );
        assert_eq!(lint_failures("A 91% match."), ["[0-9]+% match"]);
        assert_eq!(
            lint_failures("Re-pick it in the repair panel."),
            ["repair panel"]
        );
        assert_eq!(lint_failures("The `edges` field."), ["a backtick"]);
        assert_eq!(lint_failures("No such file (os error 2)."), ["os error"]);
    }

    #[test]
    fn an_op_counts_only_as_a_whole_word() {
        assert_eq!(
            lint_failures("Set it to (op cut)."),
            [r"\bop (new|join|cut|intersect)\b"]
        );
        assert!(lint_failures("Develop new parts.").is_empty());
        assert!(lint_failures("Keep op newer.").is_empty());
        assert!(lint_failures("Stop joining.").is_empty());
    }

    #[test]
    fn an_id_counts_only_as_a_whole_word_of_lowercase_letters_and_digits() {
        assert_eq!(
            lint_failures("Delete c-12 first."),
            [r"\b[fbec]-[0-9a-z]+\b"]
        );
        assert_eq!(
            lint_failures("Made by f-3fa91c0e72b4."),
            [r"\b[fbec]-[0-9a-z]+\b"]
        );
        assert!(lint_failures("Run a self-check, then re-pick it.").is_empty());
        assert!(lint_failures("Use b-Box or c-12_a.").is_empty());
        assert!(lint_failures("Add an f- prefix.").is_empty());
    }
}
