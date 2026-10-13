//! The `ps_call` performance budget of protocol.md section 5, timed with `std::time::Instant` (decision D7).
//!
//! Run it with `cargo bench -p partsmith-wasm --bench budgets`. It times the round trip of example A (`engine_info`) the way `callCore` makes it: allocate a request block and copy the request in, call `ps_call`, copy the response out, and free both blocks. The median round trip is compared with the budget, 0.2 ms natively and 1 ms for WebAssembly (`--target wasm32-unknown-emscripten`, run under Node, where the copies happen inside wasm memory). The benchmark fails when the median is over budget. The budgets hold for a release build on the reference laptop (technical design section 14).

#[path = "../tests/common/mod.rs"]
mod common;

use std::hint::black_box;
use std::process::ExitCode;
use std::time::Duration;

use common::block::Block;
use common::{example_a, ok_result, response_frame};
use partsmith_budgets::{Report, median_time_of};

/// Round trips timed; the median counts.
const RUNS: usize = 1001;
/// Untimed round trips first, so the session and the allocator are warm.
const WARM_UP_RUNS: usize = 100;
/// The budget of a round trip in protocol.md section 5.
const BUDGET: Duration = if cfg!(target_family = "wasm") {
    Duration::from_millis(1)
} else {
    Duration::from_micros(200)
};

/// One round trip through the C ABI: the request copied into a fresh block, `ps_call`, and the response copied out; both blocks are freed on return.
fn round_trip(request: &[u8]) -> Vec<u8> {
    let request_block = Block::holding(request);
    let response_block = request_block.call();
    response_block.bytes()
}

fn main() -> ExitCode {
    let mut report = Report::from_arguments();
    let request = example_a();
    partsmith_wasm::initialize_session();
    ok_result(&response_frame(&round_trip(&request)), "engine_info");
    for _ in 0..WARM_UP_RUNS {
        black_box(round_trip(black_box(&request)));
    }

    report.measure("ps_call engine_info", BUDGET, || {
        median_time_of(RUNS, || round_trip(black_box(&request)))
    });
    report.finish()
}
