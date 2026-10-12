//! The native performance budgets of protocol.md section 5, timed with `std::time::Instant` (decision D7).
//!
//! Run them with `cargo bench -p partsmith-protocol --bench budgets`. Each operation runs several
//! times and its median time is compared with its budget; the benchmark fails when any median is
//! over budget. The budgets hold for a release build on the reference laptop (technical design
//! section 14), which the CI performance runner stands in for.

use std::hint::black_box;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use partsmith_model::{Document, Id, Parameter, Units};
use partsmith_protocol::{
    Buffer, CommandResult, Frame, Message, Outcome, Response, decode_frame, encode_frame,
};

/// Times each operation runs; the median counts.
const RUNS: usize = 7;
/// Parameters in the benchmark document, which make its header about 200 KB.
const PARAMETER_COUNT: u32 = 2_700;
/// Elements of each 4-byte buffer: 12.5 MB, so three of them and an equally long byte buffer make 50 MB.
const ELEMENT_COUNT: u32 = 3_125_000;

/// A frame with a header of about 200 KB and 50 MB of buffers: a `get_document` response with a
/// large document, `f32` and `u32` buffers like a mesh's, and a `u8` buffer like an exported file.
fn benchmark_frame() -> Frame {
    let mut document = Document::new(Units::Mm);
    document.parameters = (0..PARAMETER_COUNT)
        .map(|i| {
            let id = Id::new(format!("p-{i:012x}")).expect("generated ids are valid");
            Parameter::new(id, format!("length_{i}"), format!("{i} mm + 2 * width"))
        })
        .collect();
    let response = Response {
        id: 1,
        result: Outcome::Ok(CommandResult::GetDocument {
            revision: 1,
            document,
        }),
    };
    let positions: Vec<f32> = (0..ELEMENT_COUNT).map(|i| i as f32 * 0.25).collect();
    let indices: Vec<u32> = (0..ELEMENT_COUNT).collect();
    let file = vec![0x5a; 4 * ELEMENT_COUNT as usize];
    Frame {
        messages: vec![Message::Response(response)],
        buffers: vec![
            Buffer::F32(positions.clone()),
            Buffer::F32(positions),
            Buffer::U32(indices),
            Buffer::U8(file),
        ],
    }
}

/// The median time of `RUNS` runs of `operation`.
fn median_time<Output>(mut operation: impl FnMut() -> Output) -> Duration {
    let mut times: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let start = Instant::now();
            let output = black_box(operation());
            let time = start.elapsed();
            drop(output);
            time
        })
        .collect();
    times.sort_unstable();
    times[RUNS / 2]
}

/// A duration in milliseconds.
fn readable(duration: Duration) -> String {
    format!("{:.3} ms", duration.as_secs_f64() * 1e3)
}

fn main() -> ExitCode {
    let frame = benchmark_frame();
    let bytes = encode_frame(&frame).expect("the benchmark header is below the limit");
    let header_length = u32::from_le_bytes(
        bytes[4..8]
            .try_into()
            .expect("a four-byte range converts to a four-byte array"),
    );
    println!(
        "Frame: {header_length} header bytes, {} bytes in total",
        bytes.len()
    );

    let budget = Duration::from_millis(40);
    let measurements = [
        (
            "encode_frame",
            median_time(|| encode_frame(black_box(&frame))),
        ),
        (
            "decode_frame",
            median_time(|| decode_frame(black_box(&bytes))),
        ),
    ];

    let mut within_budget = true;
    for (operation, time) in measurements {
        println!(
            "{operation:<16} {:>12}   budget {:>12}",
            readable(time),
            readable(budget)
        );
        if time > budget {
            println!("Over budget: {operation}");
            within_budget = false;
        }
    }
    if within_budget {
        println!("Every measured operation is within its budget.");
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
