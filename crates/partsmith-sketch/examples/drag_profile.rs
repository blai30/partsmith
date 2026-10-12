//! Temporary profiling driver for the chain drag.

#[path = "../tests/common/benchmark.rs"]
mod benchmark;

use std::time::Instant;

use benchmark::{CHAIN_FREE_END, chain_benchmark};
use partsmith_sketch::{DragSession, DragTarget, NativeSolver, PointRef, Solver};

fn main() {
    let frames: u32 = std::env::args().nth(1).map_or(100, |text| text.parse().expect("count"));
    let (sketch, values) = chain_benchmark();
    let free_end: PointRef = CHAIN_FREE_END.parse().expect("valid");
    let start = sketch.point(&free_end).expect("point");
    let begin = Instant::now();
    let mut session = NativeSolver
        .begin_drag(&sketch, &values, &DragTarget::Point { point: free_end })
        .expect("valid");
    println!("begin {:?}", begin.elapsed());
    let mut total = std::time::Duration::ZERO;
    for frame in 1..=frames {
        let step = f64::from(frame);
        let now = Instant::now();
        let result = session.update([start[0] - step, start[1] + step]).expect("finite");
        total += now.elapsed();
        assert!(!result.failed);
    }
    println!("frames {:?} per frame {:?}", total, total / frames);
}
