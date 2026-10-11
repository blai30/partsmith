//! Compensated summation for metrics over many triangles.

/// A running sum with Neumaier's compensation: the rounding error of every addition is collected
/// separately and added back at the end, so long sums with cancellation stay accurate.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CompensatedSum {
    /// The plain running sum.
    total: f64,
    /// The rounding error lost so far.
    compensation: f64,
}

impl CompensatedSum {
    /// Adds one term.
    pub(crate) fn add(&mut self, value: f64) {
        let total = self.total + value;
        if self.total.abs() >= value.abs() {
            self.compensation += (self.total - total) + value;
        } else {
            self.compensation += (value - total) + self.total;
        }
        self.total = total;
    }

    /// The compensated sum. A sum that overflowed is returned as is.
    pub(crate) fn value(&self) -> f64 {
        if self.total.is_finite() {
            self.total + self.compensation
        } else {
            self.total
        }
    }
}

/// The compensated sum of `values`, added in order.
pub(crate) fn compensated_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut sum = CompensatedSum::default();
    for value in values {
        sum.add(value);
    }
    sum.value()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_sum_is_zero() {
        assert_eq!(compensated_sum([]), 0.0);
    }

    #[test]
    fn small_terms_survive_large_cancelling_terms() {
        // A plain sum gives 0 here because 1 is lost next to 1e16.
        assert_eq!(compensated_sum([1e16, 1.0, -1e16]), 1.0);
    }

    #[test]
    fn many_tenths_sum_exactly_to_the_rounded_result() {
        let sum = compensated_sum(std::iter::repeat_n(0.1, 1000));

        assert_eq!(sum, 100.0);
    }

    #[test]
    fn overflowing_sum_stays_infinite() {
        assert_eq!(compensated_sum([f64::MAX, f64::MAX]), f64::INFINITY);
    }
}
