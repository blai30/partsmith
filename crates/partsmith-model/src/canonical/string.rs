//! Quoted, escaped strings (file-format.md section 3.4).

use std::fmt::Write;

/// How one character is written inside a string.
enum Escape {
    /// A two-character escape such as `\n`.
    Short(&'static str),
    /// A control character written as `\u00XX`.
    Unicode,
    /// The character itself.
    Plain,
}

/// The escape of `character`: `"` and `\` and the five short control escapes, `\u00XX` for every
/// other character below U+0020, and the character itself for everything else.
fn escape(character: char) -> Escape {
    match character {
        '"' => Escape::Short("\\\""),
        '\\' => Escape::Short("\\\\"),
        '\u{8}' => Escape::Short("\\b"),
        '\t' => Escape::Short("\\t"),
        '\n' => Escape::Short("\\n"),
        '\u{c}' => Escape::Short("\\f"),
        '\r' => Escape::Short("\\r"),
        '\u{0}'..='\u{1f}' => Escape::Unicode,
        _ => Escape::Plain,
    }
}

/// Writes one string, quoted and escaped (file-format.md section 3.4).
pub fn format_string(text: &str) -> String {
    let mut output = String::with_capacity(text.len() + 2);
    write_string(&mut output, text);
    output
}

/// Appends `text` to `output`, quoted and escaped.
pub(super) fn write_string(output: &mut String, text: &str) {
    output.push('"');
    // Characters written as themselves are copied in runs.
    let mut run_start = 0;
    for (index, character) in text.char_indices() {
        match escape(character) {
            Escape::Plain => continue,
            Escape::Short(escaped) => {
                output.push_str(&text[run_start..index]);
                output.push_str(escaped);
            }
            Escape::Unicode => {
                output.push_str(&text[run_start..index]);
                write!(output, "\\u{:04x}", u32::from(character))
                    .expect("writing to a String never fails");
            }
        }
        run_start = index + character.len_utf8();
    }
    output.push_str(&text[run_start..]);
    output.push('"');
}

/// Characters in the quoted, escaped form of `text`, or `None` once that exceeds `budget`.
pub(super) fn string_width(text: &str, budget: usize) -> Option<usize> {
    let mut width = 2;
    for character in text.chars() {
        width += match escape(character) {
            Escape::Short(_) => 2,
            Escape::Unicode => 6,
            Escape::Plain => 1,
        };
        if width > budget {
            return None;
        }
    }
    (width <= budget).then_some(width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_quoted() {
        assert_eq!(format_string("Base"), "\"Base\"");
        assert_eq!(format_string(""), "\"\"");
    }

    #[test]
    fn control_characters_use_lowercase_hex() {
        assert_eq!(format_string("\u{0}\u{1b}"), "\"\\u0000\\u001b\"");
    }

    #[test]
    fn separators_and_non_ascii_are_written_as_themselves() {
        assert_eq!(format_string("\u{2028}😀/"), "\"\u{2028}😀/\"");
    }

    #[test]
    fn width_counts_escaped_characters() {
        assert_eq!(string_width("a\"\u{1}é", 100), Some(2 + 1 + 2 + 6 + 1));
    }

    #[test]
    fn width_beyond_the_budget_is_none() {
        assert_eq!(string_width("abc", 5), Some(5));
        assert_eq!(string_width("abc", 4), None);
        assert_eq!(string_width("", 1), None);
    }

    #[test]
    fn width_matches_the_written_form() {
        let text = "Lid \"v2\" \\ draft\tAé°/x\u{1f}";

        assert_eq!(
            string_width(text, usize::MAX),
            Some(format_string(text).chars().count())
        );
    }
}
