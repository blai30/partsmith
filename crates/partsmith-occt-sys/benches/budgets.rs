//! The performance budgets of kernel spec section 5, timed with `std::time::Instant` (decision
//! D7).
//!
//! Run all of them with `cargo bench -p partsmith-occt-sys --bench budgets`, or some of them by
//! passing words of their names, for example `cargo bench -p partsmith-occt-sys --bench budgets
//! -- fillet step`. Each operation runs several times and its median time is compared with its
//! budget; the benchmark fails when any median is over budget. The budgets hold for a release
//! build on the reference laptop (technical design section 14), single thread. Web budgets are
//! three times these and are not measured here.

use std::f64::consts::TAU;
use std::hint::black_box;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use partsmith_mesh::TessellationTolerance;
use partsmith_occt_sys::{
    Axis, EdgeRadius, GeometryKind, PlanarCurve, PlanarLoop, PlanarProfile, PlanarRegion, Plane,
    Point, Point2, Shape, StepBody, SubShapeKind, TaggedCurve, Transform, Vector, cut, extrude,
    fillet, fuse, make_box, make_cone, make_cylinder, make_profile, make_sphere, make_torus,
    read_step, shell, transform, write_step,
};

/// Times each operation runs; the median counts.
const RUNS: usize = 7;
/// The sweep direction of every extrusion.
const UP: Vector = Vector::new(0.0, 0.0, 1.0);

/// One timed operation and its budget.
struct Measurement {
    /// What was timed, as spec section 5 names it.
    operation: &'static str,
    /// The median time.
    time: Duration,
    /// The budget from spec section 5.
    budget: Duration,
}

/// The operations selected on the command line, or all of them when none is named.
struct Selection {
    /// Words that select operations by their name.
    words: Vec<String>,
}

impl Selection {
    /// Reads the words from the command line, skipping the flags cargo passes.
    fn from_arguments() -> Selection {
        Selection {
            words: std::env::args()
                .skip(1)
                .filter(|argument| !argument.starts_with("--"))
                .collect(),
        }
    }

    /// True when the operation runs.
    fn includes(&self, operation: &str) -> bool {
        self.words.is_empty()
            || self
                .words
                .iter()
                .any(|word| operation.contains(word.as_str()))
    }
}

/// The median of `RUNS` timings of `operation`. Dropping the result is not timed.
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

/// A duration of whole milliseconds.
fn milliseconds(count: u64) -> Duration {
    Duration::from_millis(count)
}

/// A duration as milliseconds with three decimals.
fn readable(time: Duration) -> String {
    format!("{:.3} ms", time.as_secs_f64() * 1e3)
}

/// A point in plane coordinates.
fn point(x: f64, y: f64) -> Point2 {
    Point2 { x, y }
}

/// A plane parallel to XY through a point.
fn placed(x: f64, y: f64, z: f64) -> Plane {
    Plane {
        origin: Point::new(x, y, z),
        ..Plane::XY
    }
}

/// A line curve with a tag.
fn line(tag: u32, start: Point2, end: Point2) -> TaggedCurve {
    TaggedCurve {
        tag,
        curve: PlanarCurve::Line { start, end },
    }
}

/// A counterclockwise arc curve with a tag.
fn arc(tag: u32, center: Point2, radius: f64, start: Point2, end: Point2) -> TaggedCurve {
    TaggedCurve {
        tag,
        curve: PlanarCurve::Arc {
            center,
            radius,
            start,
            end,
        },
    }
}

/// The closed polygon through `corners`, one line per side, tagged from 1.
fn polygon(corners: &[Point2]) -> PlanarLoop {
    let curves = (0..corners.len())
        .map(|position| {
            let tag = u32::try_from(position + 1).expect("benchmark polygons are small");
            let next = corners[(position + 1) % corners.len()];
            line(tag, corners[position], next)
        })
        .collect();
    PlanarLoop { curves }
}

/// The corners of a regular polygon with `sides` sides inscribed in a circle of radius 40 mm.
fn regular_polygon(sides: u32) -> Vec<Point2> {
    (0..sides)
        .map(|corner| {
            let angle = TAU * f64::from(corner) / f64::from(sides);
            point(40.0 * angle.cos(), 40.0 * angle.sin())
        })
        .collect()
}

/// The regions of one outline without holes.
fn outline_regions(outline: PlanarLoop) -> Vec<PlanarRegion> {
    vec![PlanarRegion {
        outer: outline,
        holes: Vec::new(),
    }]
}

/// The profile of one outline on the XY plane.
fn profile(outline: PlanarLoop) -> PlanarProfile {
    make_profile(&Plane::XY, &outline_regions(outline)).expect("benchmark outlines are valid")
}

/// The first face that is planar with the given normal and centroid height.
fn planar_face(shape: &Shape, normal: Vector, height: f64) -> u32 {
    let faces = shape
        .elements(SubShapeKind::Face)
        .expect("faces can be queried");
    let position = faces
        .iter()
        .position(|face| {
            face.kind == GeometryKind::Plane
                && face
                    .direction
                    .is_some_and(|direction| direction.dot(normal) > 1.0 - 1e-9)
                && (face.centroid.z - height).abs() < 1e-9
        })
        .expect("the face exists");
    u32::try_from(position + 1).expect("benchmark shapes have few faces")
}

/// The part of spec section 5: the 80 x 50 x 30 extrude of naming spec test 81 with its four
/// vertical edges rounded (radius 5), a hole and a slot cut through it, then shelled to 2 mm
/// walls with the top face removed.
fn part() -> Shape {
    let rectangle = polygon(&[
        point(0.0, 0.0),
        point(80.0, 0.0),
        point(80.0, 50.0),
        point(0.0, 50.0),
    ]);
    let block = extrude(&profile(rectangle), UP, 0.0, 30.0)
        .expect("the block extrudes")
        .shape;
    let vertical: Vec<EdgeRadius> = block
        .elements(SubShapeKind::Edge)
        .expect("edges can be queried")
        .iter()
        .enumerate()
        .filter(|(_, edge)| {
            edge.direction
                .is_some_and(|direction| direction.z.abs() > 0.5)
        })
        .map(|(position, _)| EdgeRadius {
            edge: u32::try_from(position + 1).expect("a block has 12 edges"),
            radius: 5.0,
        })
        .collect();
    let rounded = fillet(&block, &vertical)
        .expect("the vertical edges round")
        .shape;

    let hole = make_cylinder(&placed(20.0, 25.0, -1.0), 5.0, 32.0)
        .expect("the hole cylinder builds")
        .shape;
    let slot_outline = PlanarLoop {
        curves: vec![
            line(1, point(50.0, 21.0), point(65.0, 21.0)),
            arc(
                2,
                point(65.0, 25.0),
                4.0,
                point(65.0, 21.0),
                point(65.0, 29.0),
            ),
            line(3, point(65.0, 29.0), point(50.0, 29.0)),
            arc(
                4,
                point(50.0, 25.0),
                4.0,
                point(50.0, 29.0),
                point(50.0, 21.0),
            ),
        ],
    };
    let slot = extrude(&profile(slot_outline), UP, -1.0, 31.0)
        .expect("the slot extrudes")
        .shape;
    let drilled = cut(&rounded, &[hole, slot])
        .expect("the hole and slot cut")
        .shape;

    let top = planar_face(&drilled, UP, 30.0);
    shell(&drilled, &[top], 2.0).expect("the part shells").shape
}

/// `PLATE` of spec section 5: a 200 x 200 x 10 box with 194 through holes of radius 4 cut in one
/// call (200 faces, 594 edges, 396 vertices).
fn plate() -> Shape {
    let block = make_box(&placed(0.0, 0.0, -5.0), 200.0, 200.0, 10.0)
        .expect("the plate builds")
        .shape;
    let holes: Vec<Shape> = (0..194)
        .map(|position| {
            let column = f64::from(position % 14);
            let row = f64::from(position / 14);
            make_cylinder(
                &placed(10.0 + 13.5 * column, 10.0 + 13.5 * row, -10.0),
                4.0,
                20.0,
            )
            .expect("the hole tool builds")
            .shape
        })
        .collect();
    let plate = cut(&block, &holes).expect("the holes cut").shape;
    assert_eq!(plate.count(SubShapeKind::Face), 200);
    plate
}

/// `L` of spec test 102: a 40 x 20 x 10 box fused with a 20 x 20 x 10 box standing on its right
/// half, with the index of the step face (the top of the low half, at z = 10).
fn l_body() -> (Shape, u32) {
    let low = make_box(&placed(-20.0, -10.0, 0.0), 40.0, 20.0, 10.0)
        .expect("the low box builds")
        .shape;
    let high = make_box(&placed(0.0, -10.0, 10.0), 20.0, 20.0, 10.0)
        .expect("the high box builds")
        .shape;
    let body = fuse(&low, &[high]).expect("the boxes fuse").shape;
    let step = planar_face(&body, UP, 10.0);
    (body, step)
}

fn main() -> ExitCode {
    let selection = Selection::from_arguments();
    let mut measurements: Vec<Measurement> = Vec::new();
    let mut measure = |operation: &'static str, budget: Duration, time: &dyn Fn() -> Duration| {
        if selection.includes(operation) {
            let time = time();
            println!(
                "{operation:<48} {:>12}   budget {:>12}",
                readable(time),
                readable(budget)
            );
            measurements.push(Measurement {
                operation,
                time,
                budget,
            });
        }
    };

    measure("make_box", milliseconds(1), &|| {
        median_time(|| make_box(&Plane::XY, 10.0, 20.0, 30.0))
    });
    measure("make_cylinder", milliseconds(1), &|| {
        median_time(|| make_cylinder(&Plane::XY, 5.0, 10.0))
    });
    measure("make_sphere", milliseconds(1), &|| {
        median_time(|| make_sphere(&Plane::XY, 10.0))
    });
    measure("make_cone", milliseconds(1), &|| {
        median_time(|| make_cone(&Plane::XY, 5.0, 2.0, 10.0))
    });
    measure("make_torus", milliseconds(1), &|| {
        median_time(|| make_torus(&Plane::XY, 10.0, 2.0))
    });

    let hundred_gon = polygon(&regular_polygon(100));
    measure("make_profile, 100 curves", milliseconds(5), &|| {
        let regions = outline_regions(hundred_gon.clone());
        median_time(|| make_profile(&Plane::XY, &regions))
    });
    measure("extrude, 100-edge profile", milliseconds(15), &|| {
        let outline = profile(hundred_gon.clone());
        median_time(|| extrude(&outline, UP, 0.0, 10.0))
    });

    let cube = |x: f64, y: f64, z: f64| {
        make_box(&placed(x, y, z), 10.0, 10.0, 10.0)
            .expect("the cube builds")
            .shape
    };
    let (first, second) = (cube(0.0, 0.0, 0.0), cube(5.0, 5.0, 5.0));
    measure("fuse, two primitives", milliseconds(10), &|| {
        median_time(|| fuse(&first, std::slice::from_ref(&second)))
    });
    measure("cut, two primitives", milliseconds(10), &|| {
        median_time(|| cut(&first, std::slice::from_ref(&second)))
    });
    measure("cut, 20 cylinders from a box", milliseconds(60), &|| {
        let block = make_box(&Plane::XY, 80.0, 50.0, 30.0)
            .expect("the block builds")
            .shape;
        let cylinders: Vec<Shape> = (0..20)
            .map(|position| {
                let column = f64::from(position % 5);
                let row = f64::from(position / 5);
                make_cylinder(
                    &placed(10.0 + 15.0 * column, 8.0 + 11.0 * row, -1.0),
                    2.0,
                    32.0,
                )
                .expect("the cylinder builds")
                .shape
            })
            .collect();
        median_time(|| cut(&block, &cylinders))
    });

    let block = make_box(&Plane::XY, 10.0, 20.0, 30.0)
        .expect("the box builds")
        .shape;
    measure("fillet, 12 edges of a box", milliseconds(40), &|| {
        let edges: Vec<EdgeRadius> = (1..=12)
            .map(|edge| EdgeRadius { edge, radius: 1.0 })
            .collect();
        median_time(|| fillet(&block, &edges))
    });
    measure("shell, box", milliseconds(20), &|| {
        let top = planar_face(&block, UP, 30.0);
        median_time(|| shell(&block, &[top], 1.0))
    });
    measure("transform, rigid", Duration::from_micros(100), &|| {
        let axis = Axis {
            origin: Point::new(0.0, 0.0, 0.0),
            direction: UP,
        };
        let motion = Transform::translation(Vector::new(1.0, 2.0, 3.0))
            .then(&Transform::rotation(&axis, 0.5));
        median_time(|| transform(&block, &motion))
    });
    measure(
        "elements(Face) and topology(), 200 faces",
        milliseconds(10),
        &|| {
            let prism = extrude(&profile(polygon(&regular_polygon(198))), UP, 0.0, 10.0)
                .expect("the prism extrudes")
                .shape;
            assert_eq!(prism.count(SubShapeKind::Face), 200);
            median_time(|| (prism.elements(SubShapeKind::Face), prism.topology()))
        },
    );
    let plate = plate();
    measure(
        "elements of every kind and topology(), PLATE",
        milliseconds(8),
        &|| {
            median_time(|| {
                (
                    plate.elements(SubShapeKind::Face),
                    plate.elements(SubShapeKind::Edge),
                    plate.elements(SubShapeKind::Vertex),
                    plate.topology(),
                )
            })
        },
    );
    measure("TEMP elements(Face), PLATE", milliseconds(8), &|| {
        median_time(|| plate.elements(SubShapeKind::Face))
    });
    measure("TEMP elements(Edge), PLATE", milliseconds(8), &|| {
        median_time(|| plate.elements(SubShapeKind::Edge))
    });
    measure("TEMP elements(Vertex), PLATE", milliseconds(8), &|| {
        median_time(|| plate.elements(SubShapeKind::Vertex))
    });
    measure("TEMP topology(), PLATE", milliseconds(8), &|| {
        median_time(|| plate.topology())
    });
    measure("shell, L with its step face removed", milliseconds(40), &|| {
        let (body, step) = l_body();
        median_time(|| shell(&body, &[step], 2.0))
    });

    let part = part();
    println!(
        "The part has {} faces, {} edges and {} vertices.",
        part.count(SubShapeKind::Face),
        part.count(SubShapeKind::Edge),
        part.count(SubShapeKind::Vertex)
    );
    measure("volume, part", milliseconds(2), &|| {
        median_time(|| part.volume())
    });
    measure(
        "tessellate, part at 0.05 mm and 0.1 rad",
        milliseconds(30),
        &|| {
            let tolerance = TessellationTolerance {
                chord_deviation: 0.05,
                angular_deviation: 0.1,
            };
            median_time(|| part.tessellate(&tolerance))
        },
    );
    measure("to_brep and from_brep, part", milliseconds(5), &|| {
        median_time(|| {
            let bytes = part.to_brep().expect("the part serializes");
            Shape::from_brep(&bytes)
        })
    });
    let bodies = [StepBody {
        shape: part.clone(),
        name: "Part".to_owned(),
        color: None,
    }];
    measure("write_step, part", milliseconds(100), &|| {
        median_time(|| write_step(&bodies))
    });
    measure("read_step, part", milliseconds(150), &|| {
        let bytes = write_step(&bodies).expect("the part exports");
        median_time(|| read_step(&bytes))
    });

    let over: Vec<&Measurement> = measurements
        .iter()
        .filter(|measurement| measurement.time > measurement.budget)
        .collect();
    if over.is_empty() {
        println!("Every measured operation is within its budget.");
        ExitCode::SUCCESS
    } else {
        for measurement in over {
            eprintln!(
                "{} took {}, over its budget of {}.",
                measurement.operation,
                readable(measurement.time),
                readable(measurement.budget)
            );
        }
        ExitCode::FAILURE
    }
}
