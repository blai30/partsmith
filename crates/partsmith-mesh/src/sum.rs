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

#[cfg(test)]
mod tests {
    use super::*;

    /// The compensated sum of `values`, added in order.
    fn sum_of(values: &[f64]) -> f64 {
        let mut sum = CompensatedSum::default();
        for &value in values {
            sum.add(value);
        }
        sum.value()
    }

    #[test]
    fn empty_sum_is_zero() {
        assert_eq!(sum_of(&[]), 0.0);
    }

    #[test]
    fn small_terms_survive_large_cancelling_terms() {
        // A plain sum gives 0 here because 1 is lost next to 1e16.
        assert_eq!(sum_of(&[1e16, 1.0, -1e16]), 1.0);
    }

    #[test]
    fn many_tenths_sum_to_the_correctly_rounded_result() {
        assert_eq!(sum_of(&[0.1; 1000]), 100.0);
    }

    #[test]
    fn overflowing_sum_stays_infinite() {
        assert_eq!(sum_of(&[f64::MAX, f64::MAX]), f64::INFINITY);
    }
}
