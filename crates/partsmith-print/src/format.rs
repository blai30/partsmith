//! Number formatting for messages (spec section 3.9.0), so tests can compare text exactly.

/// `x` with at most three decimals, trailing zeros and a trailing `.` removed, and `-0` written
/// as `0` (`0.6`, `0.294`, `256`, `113.137`). The spec calls this `len`.
pub(crate) fn format_length(value: f64) -> String {
    trimmed(format!("{value:.3}"))
}

/// `x` with at most one decimal, trimmed like [`format_length`] (`80`, `111.8`). The spec calls
/// this `num1`.
pub(crate) fn format_one_decimal(value: f64) -> String {
    trimmed(format!("{value:.1}"))
}

/// An angle in radians written in degrees with at most one decimal (`45`, `63.4`). The spec calls
/// this `deg`.
pub(crate) fn format_degrees(radians: f64) -> String {
    format_one_decimal(radians.to_degrees())
}

/// A ratio written as a percentage with at most one decimal (`4`, `0.3`). The spec calls this
/// `pct`.
pub(crate) fn format_percent(ratio: f64) -> String {
    format_one_decimal(100.0 * ratio)
}

/// `"1 {singular}"` for one, `"{count} {plural}"` otherwise.
pub(crate) fn format_count(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("1 {singular}")
    } else {
        format!("{count} {plural}")
    }
}

/// Removes trailing zeros and a trailing `.` from a fixed-point number and writes `-0` as `0`.
fn trimmed(mut text: String) -> String {
    if text.contains('.') {
        let kept = text.trim_end_matches('0').trim_end_matches('.').len();
        text.truncate(kept);
    }
    if text == "-0" { "0".to_string() } else { text }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_keeps_up_to_three_decimals() {
        assert_eq!(format_length(0.6), "0.6");
        assert_eq!(
            format_length(0.3 * (std::f64::consts::PI / 16.0).cos()),
            "0.294"
        );
        assert_eq!(format_length(113.13708498984761), "113.137");
    }

    #[test]
    fn length_drops_the_point_of_whole_numbers() {
        assert_eq!(format_length(256.0), "256");
        assert_eq!(format_length(2000.0), "2000");
        assert_eq!(format_length(100.0), "100");
    }

    #[test]
    fn length_rounds_to_three_decimals() {
        assert_eq!(format_length(0.99518), "0.995");
        assert_eq!(format_length(1.9999), "2");
    }

    #[test]
    fn negative_zero_is_written_as_zero() {
        assert_eq!(format_length(-0.0), "0");
        assert_eq!(format_length(-0.0001), "0");
        assert_eq!(format_one_decimal(-0.01), "0");
    }

    #[test]
    fn negative_numbers_keep_their_sign() {
        assert_eq!(format_length(-0.05), "-0.05");
        assert_eq!(format_one_decimal(-12.0), "-12");
    }

    #[test]
    fn one_decimal_rounds_and_trims() {
        assert_eq!(format_one_decimal(80.0), "80");
        assert_eq!(format_one_decimal(111.80339887498948), "111.8");
        assert_eq!(format_one_decimal(1000.0), "1000");
        assert_eq!(format_one_decimal(141.4213562373095), "141.4");
    }

    #[test]
    fn degrees_convert_from_radians() {
        assert_eq!(format_degrees(std::f64::consts::FRAC_PI_4), "45");
        assert_eq!(format_degrees(0.5235987755982988), "30");
        assert_eq!(format_degrees(2.0_f64.atan()), "63.4");
        assert_eq!(format_degrees(1.6), "91.7");
    }

    #[test]
    fn percent_scales_ratios() {
        assert_eq!(format_percent(0.04), "4");
        assert_eq!(format_percent(0.05), "5");
        assert_eq!(format_percent(0.003), "0.3");
    }

    #[test]
    fn non_finite_numbers_are_written_as_rust_does() {
        assert_eq!(format_length(f64::NAN), "NaN");
        assert_eq!(format_length(f64::INFINITY), "inf");
    }

    #[test]
    fn counts_use_the_singular_for_one() {
        assert_eq!(format_count(1, "edge", "edges"), "1 edge");
        assert_eq!(format_count(0, "edge", "edges"), "0 edges");
        assert_eq!(format_count(4, "edge", "edges"), "4 edges");
    }
}
