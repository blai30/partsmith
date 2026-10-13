//! The "too large" search of fillet, chamfer and shell (features.md section 4.3).

use crate::context::BuildContext;
use crate::error::FeatureError;

/// Number of bisection attempts. Fixed, so the result does not depend on the machine.
const ATTEMPTS: usize = 10;

/// Searches (0, `requested`) for the largest value at which `succeeds` holds, by bisection:
/// `lo = 0`, `hi = requested`; for 10 attempts, `mid = (lo + hi) / 2`; on success `lo = mid` and
/// `max = mid`, otherwise `hi = mid`. Returns the largest attempted value that succeeded, or None
/// if none did. Checks cancellation before each attempt.
pub fn too_large_search(
    context: &BuildContext<'_>,
    requested: f64,
    mut succeeds: impl FnMut(f64) -> bool,
) -> Result<Option<f64>, FeatureError> {
    let mut low = 0.0;
    let mut high = requested;
    let mut largest = None;
    for _ in 0..ATTEMPTS {
        context.check_canceled()?;
        let middle = (low + high) / 2.0;
        if succeeds(middle) {
            low = middle;
            largest = Some(middle);
        } else {
            high = middle;
        }
    }
    Ok(largest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Fixture;

    #[test]
    fn finds_the_largest_working_value_below_a_limit() {
        let fixture = Fixture::new("f-fil1");
        let mut attempts = Vec::new();

        let largest = too_large_search(&fixture.context(), 3.0, |value| {
            attempts.push(value);
            value <= 2.4
        })
        .expect("not canceled");

        assert_eq!(attempts.len(), 10);
        assert_eq!(attempts[0], 1.5);
        assert_eq!(attempts[1], 2.25);
        let largest = largest.expect("some value worked");
        assert!(largest <= 2.4 && largest > 2.39, "got {largest}");
    }

    #[test]
    fn nothing_works_gives_none() {
        let fixture = Fixture::new("f-fil1");

        let largest = too_large_search(&fixture.context(), 3.0, |_| false).expect("not canceled");

        assert_eq!(largest, None);
    }

    #[test]
    fn the_attempt_sequence_is_fixed() {
        let fixture = Fixture::new("f-fil1");
        let mut first = Vec::new();
        let mut second = Vec::new();

        too_large_search(&fixture.context(), 1.0, |value| {
            first.push(value);
            value < 0.3
        })
        .expect("not canceled");
        too_large_search(&fixture.context(), 1.0, |value| {
            second.push(value);
            value < 0.3
        })
        .expect("not canceled");

        assert_eq!(first, second);
    }

    #[test]
    fn cancellation_stops_the_search() {
        let fixture = Fixture::new("f-fil1");
        fixture.cancel.cancel();

        let result = too_large_search(&fixture.context(), 3.0, |_| true);

        assert_eq!(result, Err(FeatureError::Canceled));
    }
}
