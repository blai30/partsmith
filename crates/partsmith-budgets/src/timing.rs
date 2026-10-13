//! Median times taken with `std::time::Instant`.

use std::hint::black_box;
use std::time::{Duration, Instant};

/// The median of `runs` samples, each the time `sample` returns for its sample number, from 0.
///
/// # Panics
///
/// When `runs` is 0.
pub fn median(runs: usize, sample: impl FnMut(usize) -> Duration) -> Duration {
    let mut times: Vec<Duration> = (0..runs).map(sample).collect();
    times.sort_unstable();
    times[runs / 2]
}

/// The median of `runs` timings of `operation`, with a fresh input from `setup` for each run.
/// Neither the setup nor dropping the result is timed. The input and the result pass through
/// `black_box`, so the work cannot be optimized away.
///
/// # Panics
///
/// When `runs` is 0.
pub fn median_time<Input, Output>(
    runs: usize,
    mut setup: impl FnMut() -> Input,
    mut operation: impl FnMut(Input) -> Output,
) -> Duration {
    median(runs, |_| {
        let input = setup();
        let start = Instant::now();
        let output = black_box(operation(black_box(input)));
        let time = start.elapsed();
        drop(output);
        time
    })
}

/// The median of `runs` timings of an operation that needs no fresh input. Dropping the result
/// is not timed.
///
/// # Panics
///
/// When `runs` is 0.
pub fn median_time_of<Output>(runs: usize, mut operation: impl FnMut() -> Output) -> Duration {
    median_time(runs, || (), |()| operation())
}

/// The median over `runs` runs of the mean time of one call of `operation`, each run making
/// `repeats` calls; for operations too short to time one call at a time. Dropping each result
/// is timed with its call.
///
/// # Panics
///
/// When `runs` or `repeats` is 0.
pub fn median_mean_time<Output>(
    runs: usize,
    repeats: u32,
    mut operation: impl FnMut() -> Output,
) -> Duration {
    median(runs, |_| {
        let start = Instant::now();
        for _ in 0..repeats {
            black_box(operation());
        }
        start.elapsed() / repeats
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_is_the_middle_of_the_sorted_samples() {
        let samples = [5, 1, 4, 2, 3].map(Duration::from_millis);

        assert_eq!(
            median(samples.len(), |sample| samples[sample]),
            Duration::from_millis(3)
        );
    }

    #[test]
    fn median_of_an_even_count_is_the_upper_middle() {
        let samples = [4, 1, 3, 2].map(Duration::from_millis);

        assert_eq!(
            median(samples.len(), |sample| samples[sample]),
            Duration::from_millis(3)
        );
    }

    #[test]
    fn median_numbers_the_samples_in_order() {
        let mut numbers = Vec::new();

        median(3, |sample| {
            numbers.push(sample);
            Duration::ZERO
        });

        assert_eq!(numbers, [0, 1, 2]);
    }

    #[test]
    #[should_panic]
    fn median_of_no_runs_panics() {
        median(0, |_| Duration::ZERO);
    }

    #[test]
    fn median_time_gives_each_run_a_fresh_input() {
        let mut setups = 0;
        let mut inputs = Vec::new();

        median_time(
            3,
            || {
                setups += 1;
                setups
            },
            |input| inputs.push(input),
        );

        assert_eq!(inputs, [1, 2, 3]);
    }

    #[test]
    fn median_time_of_runs_the_operation_once_per_run() {
        let mut calls = 0;

        median_time_of(5, || calls += 1);

        assert_eq!(calls, 5);
    }

    #[test]
    fn median_mean_time_repeats_the_operation_in_every_run() {
        let mut calls = 0;

        median_mean_time(3, 10, || calls += 1);

        assert_eq!(calls, 30);
    }
}
