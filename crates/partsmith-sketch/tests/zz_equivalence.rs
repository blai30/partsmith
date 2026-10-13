//! Temporary bitwise equivalence dump for the cleanup pass; never committed.

mod common;

use std::collections::hash_map::DefaultHasher;
use std::f64::consts::TAU;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};

use common::benchmark::{chain, chain_benchmark, grid_benchmark};
use common::generator::{generate, point_refs};
use common::random::Rng;
use partsmith_sketch::{
    DimensionValues, DragSession, DragTarget, Geometry, Id, NativeSolver, OriginPlane,
    SketchEntity, SketchParams, SketchPlane, Solver, build_profile, match_region,
};

fn digest(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn drag(out: &mut String, sketch: &SketchParams, values: &DimensionValues, target: &DragTarget) {
    match NativeSolver.begin_drag(sketch, values, target) {
        Ok(mut session) => {
            writeln!(out, "initial {:?}", session.initial()).unwrap();
            for frame in 0..4 {
                let cursor = [3.0 * frame as f64 - 2.0, 1.5 * frame as f64 + 0.5];
                writeln!(out, "frame {:?}", session.update(cursor)).unwrap();
            }
        }
        Err(error) => writeln!(out, "drag error {error}").unwrap(),
    }
}

fn solve_all(out: &mut String, sketch: &SketchParams, values: &DimensionValues) {
    match NativeSolver.solve(sketch, values) {
        Ok(result) => {
            writeln!(out, "{result:?}").unwrap();
            let issues: Vec<String> = result
                .issues(sketch)
                .iter()
                .map(ToString::to_string)
                .collect();
            writeln!(out, "issues {issues:?}").unwrap();
            profile(out, &result.entities);
        }
        Err(error) => writeln!(out, "error {error}").unwrap(),
    }
}

fn profile(out: &mut String, entities: &[SketchEntity]) {
    let profile = build_profile(entities);
    writeln!(out, "profile {profile:?}").unwrap();
    for region in &profile.regions {
        let reference = region.reference();
        writeln!(out, "fingerprint {:?}", region.fingerprint()).unwrap();
        writeln!(out, "reference {:?}", serde_json::to_string(&reference)).unwrap();
        writeln!(
            out,
            "match {:?}",
            match_region(&profile.regions, &reference)
        )
        .unwrap();
        writeln!(out, "area {:?}", region.outer.signed_area()).unwrap();
        for hole in &region.holes {
            writeln!(out, "hole area {:?}", hole.signed_area()).unwrap();
        }
    }
    if entities.len() > 1 {
        // Match the regions against the profile without the first entity.
        let fewer = build_profile(&entities[1..]);
        for region in &profile.regions {
            writeln!(
                out,
                "fewer match {:?}",
                match_region(&fewer.regions, &region.reference())
            )
            .unwrap();
        }
    }
}

fn entity(name: String, geometry: Geometry, construction: bool) -> SketchEntity {
    SketchEntity {
        id: Id::new(name.as_str()).unwrap(),
        geometry,
        construction,
    }
}

/// A random soup of entities on a coarse grid, so that ends meet, curves overlap and touch.
fn soup(seed: u64) -> Vec<SketchEntity> {
    let mut rng = Rng::new(seed);
    let count = 1 + rng.index(9);
    let coarse = rng.chance(0.7);
    let grid = |rng: &mut Rng| {
        if coarse {
            (rng.index(5) as f64) * 10.0
        } else {
            rng.uniform(-5.0, 45.0)
        }
    };
    let mut entities = Vec::new();
    for i in 0..count {
        let geometry = match rng.index(4) {
            0 => Geometry::Line {
                start: [grid(&mut rng), grid(&mut rng)],
                end: [grid(&mut rng), grid(&mut rng)],
            },
            1 => Geometry::Circle {
                center: [grid(&mut rng), grid(&mut rng)],
                radius: if coarse {
                    (1 + rng.index(3)) as f64 * 10.0
                } else {
                    rng.uniform(0.0, 20.0)
                },
            },
            2 => {
                let quarter = TAU / 4.0;
                let (start_angle, end_angle) = if coarse {
                    (rng.index(4) as f64 * quarter, rng.index(4) as f64 * quarter)
                } else {
                    (rng.uniform(-7.0, 7.0), rng.uniform(-7.0, 7.0))
                };
                Geometry::Arc {
                    center: [grid(&mut rng), grid(&mut rng)],
                    radius: if coarse {
                        (1 + rng.index(3)) as f64 * 10.0
                    } else {
                        rng.uniform(0.0, 20.0)
                    },
                    start_angle,
                    end_angle,
                }
            }
            _ => Geometry::Point {
                position: [grid(&mut rng), grid(&mut rng)],
            },
        };
        entities.push(entity(format!("e-{i}"), geometry, rng.chance(0.1)));
    }
    entities
}

#[test]
fn dump() {
    let Some(path) = std::env::var_os("EQUIV_OUT") else {
        return;
    };
    let seeds: u64 = std::env::var("EQUIV_SEEDS")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(1500);
    let verbose = std::env::var("EQUIV_VERBOSE").ok();
    let mut out = String::new();
    for seed in 0..seeds {
        let mut case_text = String::new();
        match generate(seed) {
            None => writeln!(case_text, "discarded").unwrap(),
            Some(case) => {
                writeln!(
                    case_text,
                    "json {}",
                    serde_json::to_string(&case.warm_start).unwrap()
                )
                .unwrap();
                writeln!(
                    case_text,
                    "value {}",
                    serde_json::to_value(&case.known).unwrap()
                )
                .unwrap();
                writeln!(
                    case_text,
                    "dimensions {:?}",
                    case.known.driving_dimensions()
                )
                .unwrap();
                solve_all(&mut case_text, &case.warm_start, &case.values);
                solve_all(&mut case_text, &case.known, &case.values);
                let sketch = &case.warm_start;
                for point in point_refs(&sketch.entities).iter().take(4) {
                    drag(
                        &mut case_text,
                        sketch,
                        &case.values,
                        &DragTarget::Point {
                            point: point.clone(),
                        },
                    );
                }
                for entity in &sketch.entities {
                    drag(
                        &mut case_text,
                        sketch,
                        &case.values,
                        &DragTarget::Entity {
                            entity: entity.id.clone(),
                            grab: [1.0, 2.0],
                        },
                    );
                }
            }
        }
        let soup_entities = soup(seed);
        profile(&mut case_text, &soup_entities);
        let soup_sketch = SketchParams {
            plane: SketchPlane::Origin {
                plane: OriginPlane::Xy,
            },
            entities: soup_entities,
            constraints: Vec::new(),
        };
        writeln!(
            case_text,
            "soup json {}",
            serde_json::to_string(&soup_sketch).unwrap()
        )
        .unwrap();
        if verbose.as_deref() == Some(&seed.to_string()) {
            println!("{case_text}");
        }
        writeln!(out, "seed {seed} {:016x}", digest(&case_text)).unwrap();
    }
    let mut bench_text = String::new();
    let (sketch, values) = chain_benchmark();
    solve_all(&mut bench_text, &sketch, &values);
    let (small, small_values) = chain(3);
    solve_all(&mut bench_text, &small, &small_values);
    drag(
        &mut bench_text,
        &sketch,
        &values,
        &DragTarget::Point {
            point: common::benchmark::CHAIN_FREE_END.parse().unwrap(),
        },
    );
    profile(&mut bench_text, &grid_benchmark());
    writeln!(out, "benchmarks {:016x}", digest(&bench_text)).unwrap();
    std::fs::write(path, out).unwrap();
}

#[test]
fn time_scenarios() {
    let Ok(scenario) = std::env::var("EQUIV_TIME") else {
        return;
    };
    let repetitions = 200;
    let grid = grid_benchmark();
    let (chain_sketch, chain_values) = chain_benchmark();
    let run = || match scenario.as_str() {
        "grid" => {
            std::hint::black_box(build_profile(&grid));
        }
        "solve" => {
            std::hint::black_box(NativeSolver.solve(&chain_sketch, &chain_values).unwrap());
        }
        "begin" => {
            let target = DragTarget::Point {
                point: common::benchmark::CHAIN_FREE_END.parse().unwrap(),
            };
            std::hint::black_box(
                NativeSolver
                    .begin_drag(&chain_sketch, &chain_values, &target)
                    .unwrap(),
            );
        }
        "drag" => {
            let target = DragTarget::Point {
                point: common::benchmark::CHAIN_FREE_END.parse().unwrap(),
            };
            let mut session = NativeSolver
                .begin_drag(&chain_sketch, &chain_values, &target)
                .unwrap();
            for frame in 1..=20 {
                let step = f64::from(frame);
                std::hint::black_box(session.update([-step, step]).unwrap());
            }
        }
        _ => panic!("unknown scenario"),
    };
    for _ in 0..5 {
        run();
    }
    let mut times: Vec<f64> = (0..repetitions)
        .map(|_| {
            let start = std::time::Instant::now();
            run();
            start.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    times.sort_by(f64::total_cmp);
    println!(
        "TIME {scenario} median {:.4} min {:.4}",
        times[repetitions / 2],
        times[0]
    );
}
