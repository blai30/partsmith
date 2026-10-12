//! Names, text and numbers shared by the writers (spec sections 3.1.2 and 3.1.3).

use partsmith_mesh::Point3;

/// Replaces every control character, and the characters U+FFFE and U+FFFF that XML 1.0 does not
/// allow, with `_`. Every other character is kept, including spaces at either end.
pub(crate) fn clean(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_control() || matches!(character, '\u{FFFE}' | '\u{FFFF}') {
                '_'
            } else {
                character
            }
        })
        .collect()
}

/// True when the text has no character other than whitespace.
fn is_blank(text: &str) -> bool {
    text.chars().all(char::is_whitespace)
}

/// The name of the body at input index `index` in output files, warnings and errors: its cleaned
/// name, or `Body {index + 1}` when that is blank.
pub(crate) fn display_name(index: usize, name: &str) -> String {
    let cleaned = clean(name);
    if is_blank(&cleaned) {
        format!("Body {}", index + 1)
    } else {
        cleaned
    }
}

/// The cleaned text when it is not blank (materials and titles); `None` means absent.
pub(crate) fn non_blank(text: &str) -> Option<String> {
    let cleaned = clean(text);
    (!is_blank(&cleaned)).then_some(cleaned)
}

/// The value every format writes for a coordinate or normal component: the nearest `f32` (ties
/// to even), with `-0.0` written as `+0.0`.
///
/// `Display` of the result is the text form: the shortest decimal that parses back to the same
/// `f32`, without exponent or trailing `.0`, and `0` for zero since the result is never `-0.0`.
pub(crate) fn written(value: f64) -> f32 {
    let nearest = value as f32;
    if nearest == 0.0 { 0.0 } else { nearest }
}

/// [`written`] for each coordinate of a point or vector.
pub(crate) fn written_point(point: Point3) -> [f32; 3] {
    point.map(written)
}

/// True when some coordinate rounds to an infinite `f32` (its magnitude is beyond `f32::MAX`).
pub(crate) fn is_out_of_range(point: Point3) -> bool {
    point
        .iter()
        .any(|&coordinate| (coordinate as f32).is_infinite())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text form of a written value.
    fn printed(value: f64) -> String {
        written(value).to_string()
    }

    #[test]
    fn clean_keeps_ordinary_text() {
        assert_eq!(clean("Bracket v2"), "Bracket v2");
    }

    #[test]
    fn clean_keeps_outer_spaces() {
        assert_eq!(clean("  Block "), "  Block ");
    }

    #[test]
    fn clean_keeps_non_ascii_letters() {
        assert_eq!(clean("Grüße"), "Grüße");
    }

    #[test]
    fn clean_replaces_tab_and_line_breaks() {
        assert_eq!(clean("A\tB\nC\rD"), "A_B_C_D");
    }

    #[test]
    fn clean_replaces_delete_and_c1_controls() {
        assert_eq!(clean("a\u{7F}b\u{85}c"), "a_b_c");
    }

    #[test]
    fn clean_replaces_xml_noncharacters() {
        assert_eq!(clean("\u{FFFE}x\u{FFFF}"), "_x_");
    }

    #[test]
    fn clean_keeps_the_last_allowed_bmp_character() {
        assert_eq!(clean("\u{FFFD}"), "\u{FFFD}");
    }

    #[test]
    fn display_name_is_the_cleaned_name() {
        assert_eq!(display_name(0, "x\ny"), "x_y");
    }

    #[test]
    fn display_name_numbers_an_empty_name_from_one() {
        assert_eq!(display_name(0, ""), "Body 1");
        assert_eq!(display_name(4, ""), "Body 5");
    }

    #[test]
    fn display_name_numbers_a_whitespace_name() {
        assert_eq!(display_name(1, " \u{3000} "), "Body 2");
    }

    #[test]
    fn display_name_of_a_lone_tab_is_an_underscore() {
        assert_eq!(display_name(0, "\t"), "_");
    }

    #[test]
    fn non_blank_cleans_text() {
        assert_eq!(non_blank("PLA\t+"), Some("PLA_+".to_owned()));
    }

    #[test]
    fn non_blank_rejects_empty_and_whitespace() {
        assert_eq!(non_blank(""), None);
        assert_eq!(non_blank("   "), None);
    }

    #[test]
    fn integers_print_without_fraction() {
        assert_eq!(printed(10.0), "10");
        assert_eq!(printed(256.0), "256");
    }

    #[test]
    fn numbers_print_the_shortest_f32_decimal() {
        assert_eq!(printed(0.1), "0.1");
        assert_eq!(printed(1.0 / 3.0), "0.33333334");
        assert_eq!(printed(123456.789), "123456.79");
        assert_eq!(printed(12.345678901234567), "12.345679");
        assert_eq!(printed(2.0_f64.sqrt()), "1.4142135");
    }

    #[test]
    fn negative_numbers_have_a_leading_minus() {
        assert_eq!(printed(-2.5), "-2.5");
    }

    #[test]
    fn small_numbers_print_without_exponent() {
        assert_eq!(printed(1e-7), "0.0000001");
    }

    #[test]
    fn negative_zero_is_written_as_zero() {
        assert_eq!(written(-0.0).to_bits(), 0);
        assert_eq!(printed(-0.0), "0");
    }

    #[test]
    fn values_below_the_smallest_f32_are_zero() {
        assert_eq!(printed(1e-46), "0");
        assert_eq!(written(-1e-46).to_bits(), 0);
    }

    #[test]
    fn written_rounds_to_nearest_f32() {
        assert_eq!(written(0.1).to_bits(), 0x3dcc_cccd);
    }

    #[test]
    fn written_point_converts_each_coordinate() {
        assert_eq!(written_point([1.0, -0.0, 0.5]), [1.0, 0.0, 0.5]);
    }

    #[test]
    fn coordinates_within_f32_range_are_accepted() {
        assert!(!is_out_of_range([3.4e38, -3.4e38, 0.0]));
    }

    #[test]
    fn coordinates_that_round_to_f32_max_are_accepted() {
        assert!(!is_out_of_range([
            f64::from(f32::MAX) * (1.0 + 1e-9),
            0.0,
            0.0
        ]));
    }

    #[test]
    fn coordinates_beyond_f32_range_are_rejected() {
        assert!(is_out_of_range([0.0, 1e39, 0.0]));
        assert!(is_out_of_range([0.0, 0.0, -1e39]));
    }
}
