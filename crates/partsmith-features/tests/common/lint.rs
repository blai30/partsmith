//! The message lint of the error guidelines (docs/guidelines/errors.md, "Checklist"; features.md section 3.12): no
//! rendered message matches `repair panel`, `\bop (new|join|cut|intersect)\b`, `\b[fbec]-[0-9a-z]+\b`,
//! `(Edge|Face|Vertex|Curve) [0-9]`, `[0-9]+% match`, a backtick or `os error`.

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
