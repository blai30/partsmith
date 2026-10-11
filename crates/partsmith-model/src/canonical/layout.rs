//! The canonical layout: expanded containers with one-line leaves (file-format.md section 3.5).

use serde_json::Value;

use super::number::{number_width, write_number};
use super::string::{string_width, write_string};
use super::{INDENT_WIDTH, MAX_LINE_WIDTH};

/// Writes any JSON value in the canonical layout, with a trailing line feed (file-format.md section 3.5).
pub fn to_string(value: &Value) -> String {
    let mut output = String::new();
    write_value(&mut output, value, 0, 0, true);
    output.push('\n');
    output
}

/// Writes `value` at `indent` spaces, after a key prefix `prefix_width` characters wide. The
/// root container is always expanded; any other container prints on one line when it contains no
/// array of objects and its line fits in `MAX_LINE_WIDTH` characters.
fn write_value(
    output: &mut String,
    value: &Value,
    indent: usize,
    prefix_width: usize,
    is_root: bool,
) {
    let expand =
        |is_empty: bool| !is_empty && (is_root || !fits_on_one_line(value, indent + prefix_width));
    let inner = indent + INDENT_WIDTH;
    match value {
        Value::Array(items) if expand(items.is_empty()) => {
            output.push_str("[\n");
            for (position, item) in items.iter().enumerate() {
                push_spaces(output, inner);
                write_value(output, item, inner, 0, false);
                end_member(output, position + 1 == items.len());
            }
            push_spaces(output, indent);
            output.push(']');
        }
        Value::Object(members) if expand(members.is_empty()) => {
            output.push_str("{\n");
            for (position, (key, member)) in members.iter().enumerate() {
                push_spaces(output, inner);
                let key_start = output.len();
                write_string(output, key);
                output.push_str(": ");
                let key_width = output[key_start..].chars().count();
                write_value(output, member, inner, key_width, false);
                end_member(output, position + 1 == members.len());
            }
            push_spaces(output, indent);
            output.push('}');
        }
        _ => write_one_line(output, value),
    }
}

/// Appends the one-line form of `value` (file-format.md section 3.5).
pub(crate) fn write_one_line(output: &mut String, value: &Value) {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(true) => output.push_str("true"),
        Value::Bool(false) => output.push_str("false"),
        Value::Number(number) => write_number(output, number),
        Value::String(text) => write_string(output, text),
        Value::Array(items) => {
            output.push('[');
            for (position, item) in items.iter().enumerate() {
                if position > 0 {
                    output.push_str(", ");
                }
                write_one_line(output, item);
            }
            output.push(']');
        }
        Value::Object(members) => {
            output.push('{');
            for (position, (key, member)) in members.iter().enumerate() {
                if position > 0 {
                    output.push_str(", ");
                }
                write_string(output, key);
                output.push_str(": ");
                write_one_line(output, member);
            }
            output.push('}');
        }
    }
}

/// True when `value` may print on one line that already holds `used` characters.
fn fits_on_one_line(value: &Value, used: usize) -> bool {
    // The width check stops after at most `MAX_LINE_WIDTH` characters, which also bounds the
    // size of any value the array-of-objects check then walks.
    MAX_LINE_WIDTH
        .checked_sub(used)
        .and_then(|budget| one_line_width(value, budget))
        .is_some()
        && !contains_array_of_objects(value)
}

/// Characters in the one-line form of `value`, or `None` once that exceeds `budget`.
fn one_line_width(value: &Value, budget: usize) -> Option<usize> {
    let width = match value {
        Value::Null | Value::Bool(true) => 4,
        Value::Bool(false) => 5,
        Value::Number(number) => number_width(number),
        Value::String(text) => string_width(text, budget)?,
        Value::Array(items) => {
            let mut width = 2 + separators_width(items.len());
            for item in items {
                width += one_line_width(item, budget.checked_sub(width)?)?;
            }
            width
        }
        Value::Object(members) => {
            let mut width = 2 + separators_width(members.len());
            for (key, member) in members {
                // The key, then `: `.
                width += string_width(key, budget.checked_sub(width)?)? + 2;
                width += one_line_width(member, budget.checked_sub(width)?)?;
            }
            width
        }
    };
    (width <= budget).then_some(width)
}

/// Characters of the `, ` separators between `count` elements.
fn separators_width(count: usize) -> usize {
    2 * count.saturating_sub(1)
}

/// True when `value` is an array with an object element, or holds one at any depth.
fn contains_array_of_objects(value: &Value) -> bool {
    match value {
        Value::Array(items) => items
            .iter()
            .any(|item| item.is_object() || contains_array_of_objects(item)),
        Value::Object(members) => members.values().any(contains_array_of_objects),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
    }
}

/// Ends one member of an expanded container: a comma unless it is the last, then a line feed.
fn end_member(output: &mut String, is_last: bool) {
    if !is_last {
        output.push(',');
    }
    output.push('\n');
}

/// Appends `count` spaces.
fn push_spaces(output: &mut String, count: usize) {
    output.extend(std::iter::repeat_n(' ', count));
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn width(value: &Value) -> Option<usize> {
        one_line_width(value, usize::MAX)
    }

    #[test]
    fn widths_match_the_one_line_form() {
        let value = json!({"a": [1, "é\n", null, false], "b": {}, "c": [], "d": 0.5});
        let mut line = String::new();
        write_one_line(&mut line, &value);

        assert_eq!(width(&value), Some(line.chars().count()));
    }

    #[test]
    fn width_stops_at_the_budget() {
        let value = json!([1, 2, 3]);

        assert_eq!(one_line_width(&value, 9), Some(9));
        assert_eq!(one_line_width(&value, 8), None);
    }

    #[test]
    fn objects_alone_are_not_arrays_of_objects() {
        assert!(!contains_array_of_objects(&json!({"a": {"b": 1}})));
        assert!(contains_array_of_objects(&json!({"a": {"b": [[{}]]}})));
    }

    #[test]
    fn a_one_line_member_may_end_at_column_121_with_its_comma() {
        // Indentation and `"k": ` take 7 columns and `["..."]` takes 4 plus the text, so the line is 120 wide.
        let text = "x".repeat(109);
        let value = json!({"k": [text.clone()], "z": 1});

        let output = to_string(&value);

        assert!(
            output.contains(&format!("  \"k\": [\"{text}\"],\n")),
            "{output}"
        );
    }

    #[test]
    fn a_line_one_too_wide_is_expanded() {
        let text = "x".repeat(110);
        let value = json!({"k": [text.clone()]});

        let output = to_string(&value);

        assert_eq!(output, format!("{{\n  \"k\": [\n    \"{text}\"\n  ]\n}}\n"));
    }

    #[test]
    fn scalars_at_the_root_stay_on_one_line() {
        assert_eq!(to_string(&json!(null)), "null\n");
        assert_eq!(to_string(&json!(-0.5)), "-0.5\n");
    }

    #[test]
    fn keys_are_escaped() {
        assert_eq!(to_string(&json!({"a\tb": 1})), "{\n  \"a\\tb\": 1\n}\n");
    }
}
