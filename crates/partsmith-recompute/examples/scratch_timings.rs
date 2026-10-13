//! Temporary: per-feature medians of cold bench50 runs.

#[path = "../tests/common/mod.rs"]
mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use common::Harness;

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/bench/fifty-features/model.ptsm");
    let text = std::fs::read_to_string(path).expect("bench50");
    let document = partsmith_model::load(&text).expect("loads").document;
    let mut order = Vec::new();
    let mut times: BTreeMap<String, Vec<Duration>> = BTreeMap::new();
    let mut totals = Vec::new();
    for _ in 0..5 {
        let mut harness = Harness::builtins();
        let report = harness.run(&document);
        totals.push(report.elapsed);
        for feature in &report.features {
            let name = feature.feature.as_str().to_owned();
            if !times.contains_key(&name) {
                order.push(name.clone());
            }
            times.entry(name).or_default().push(feature.timing.duration);
        }
    }
    totals.sort();
    println!("total {:.1} ms", totals[2].as_secs_f64() * 1000.0);
    for name in order {
        let mut values = times[&name].clone();
        values.sort();
        println!("{name} {:.2}", values[2].as_secs_f64() * 1000.0);
    }
}
