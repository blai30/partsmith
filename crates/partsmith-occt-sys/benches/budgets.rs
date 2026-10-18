//! The performance budgets of kernel spec section 5, timed with `std::time::Instant` (decision
//! D7).
//!
//! Run all of them with `cargo bench -p partsmith-occt-sys --bench budgets`, or some of them by
//! passing words of their names, for example `cargo bench -p partsmith-occt-sys --bench budgets
//! -- fillet step`. Each operation runs several times and its median time is compared with its
//! budget; the benchmark fails when any median is over budget. The budgets hold for a release
//! build on the reference laptop (technical design section 14), single thread. Web budgets are
//! three times these and are not measured here.

#[path = "../tests/common/mod.rs"]
mod common;

use std::f64::consts::TAU;
use std::process::ExitCode;
use std::time::Duration;

use partsmith_budgets::{Report, median_time_of};
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

/// A duration of whole milliseconds.
fn milliseconds(count: u64) -> Duration {
    Duration::from_millis(count)
}

/// A point in plane coordinates.
fn point(x: f64, y: f64) -> Point2 {
    Point2 { x, y }
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
            let tag = common::index_from_position(position);
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
    common::index_from_position(position)
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
            edge: common::index_from_position(position),
            radius: 5.0,
        })
        .collect();
    let rounded = fillet(&block, &vertical)
        .expect("the vertical edges round")
        .shape;

    let hole = make_cylinder(&common::xy_plane_at(20.0, 25.0, -1.0), 5.0, 32.0)
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

/// `RC` of spec test 131: a 20 mm cube from (-10,-10,0) with its four vertical edges rounded at
/// radius 2.
fn rounded_cube() -> Shape {
    let block = make_box(&common::xy_plane_at(-10.0, -10.0, 0.0), 20.0, 20.0, 20.0)
        .expect("the cube builds")
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
            edge: common::index_from_position(position),
            radius: 2.0,
        })
        .collect();
    fillet(&block, &vertical)
        .expect("the vertical edges round")
        .shape
}

/// A `size` x `size` x 10 box (origin (0,0,-5)) with `holes` through holes of radius 4 cut in
/// one call, in rows of `columns` holes 13.5 mm apart from (10, 10).
fn holed_plate(size: f64, holes: u32, columns: u32) -> Shape {
    let block = make_box(&common::xy_plane_at(0.0, 0.0, -5.0), size, size, 10.0)
        .expect("the plate builds")
        .shape;
    let tools: Vec<Shape> = (0..holes)
        .map(|position| {
            let column = f64::from(position % columns);
            let row = f64::from(position / columns);
            make_cylinder(
                &common::xy_plane_at(10.0 + 13.5 * column, 10.0 + 13.5 * row, -10.0),
                4.0,
                20.0,
            )
            .expect("the hole tool builds")
            .shape
        })
        .collect();
    let plate = cut(&block, &tools).expect("the holes cut").shape;
    assert_eq!(plate.count(SubShapeKind::Face), holes + 6);
    plate
}

/// `PLATE` of spec section 5: a 200 x 200 x 10 box with 194 through holes of radius 4 cut in one
/// call (200 faces, 594 edges, 396 vertices).
fn plate() -> Shape {
    holed_plate(200.0, 194, 14)
}

/// `PLATE1000` of spec section 5: `PLATE` grown to a 440 x 440 x 10 box with 994 holes (1,000
/// faces, 2,994 edges, 1,996 vertices).
fn plate_1000() -> Shape {
    holed_plate(440.0, 994, 32)
}

/// The slowest median time of `width_across` over every face of `shape` across each of its edges
/// that is not degenerated.
fn slowest_width_across(shape: &Shape) -> Duration {
    let topology = shape.topology().expect("topology can be queried");
    let mut slowest = Duration::ZERO;
    for (position, face) in topology.faces.iter().enumerate() {
        let face_index = common::index_from_position(position);
        for &edge in &face.edges {
            if topology.edges[edge as usize - 1].degenerate {
                continue;
            }
            // A failed query returns early, so its time would say nothing about the budget.
            shape
                .width_across(face_index, edge)
                .expect("the width is measured");
            let time = median_time_of(RUNS, || shape.width_across(face_index, edge));
            slowest = slowest.max(time);
        }
    }
    slowest
}

/// The kernel calls of a feature whose shell of `requested` mm fails, with the "too large" search of
/// features.md section 4.3: the failing call, then 10 bisections of (0, `requested`). The largest
/// value that `succeeds`, or nothing.
fn too_large_search(requested: f64, succeeds: impl Fn(f64) -> bool) -> Option<f64> {
    assert!(!succeeds(requested), "the requested value fails");
    let mut low = 0.0;
    let mut high = requested;
    let mut largest = None;
    for _ in 0..10 {
        let middle = (low + high) / 2.0;
        if succeeds(middle) {
            low = middle;
            largest = Some(middle);
        } else {
            high = middle;
        }
    }
    largest
}

fn main() -> ExitCode {
    let mut report = Report::from_arguments();

    report.measure("make_box", milliseconds(1), || {
        median_time_of(RUNS, || make_box(&Plane::XY, 10.0, 20.0, 30.0))
    });
    report.measure("make_cylinder", milliseconds(1), || {
        median_time_of(RUNS, || make_cylinder(&Plane::XY, 5.0, 10.0))
    });
    report.measure("make_sphere", milliseconds(1), || {
        median_time_of(RUNS, || make_sphere(&Plane::XY, 10.0))
    });
    report.measure("make_cone", milliseconds(1), || {
        median_time_of(RUNS, || make_cone(&Plane::XY, 5.0, 2.0, 10.0))
    });
    report.measure("make_torus", milliseconds(1), || {
        median_time_of(RUNS, || make_torus(&Plane::XY, 10.0, 2.0))
    });

    let hundred_gon = polygon(&regular_polygon(100));
    report.measure("make_profile, 100 curves", milliseconds(5), || {
        let regions = outline_regions(hundred_gon.clone());
        median_time_of(RUNS, || make_profile(&Plane::XY, &regions))
    });
    report.measure("extrude, 100-edge profile", milliseconds(15), || {
        let outline = profile(hundred_gon.clone());
        median_time_of(RUNS, || extrude(&outline, UP, 0.0, 10.0))
    });

    let (first, second) = (common::cube_a().shape, common::cube_b().shape);
    report.measure("fuse, two primitives", milliseconds(10), || {
        median_time_of(RUNS, || fuse(&first, std::slice::from_ref(&second)))
    });
    report.measure("cut, two primitives", milliseconds(10), || {
        median_time_of(RUNS, || cut(&first, std::slice::from_ref(&second)))
    });
    report.measure("cut, 20 cylinders from a box", milliseconds(60), || {
        let block = make_box(&Plane::XY, 80.0, 50.0, 30.0)
            .expect("the block builds")
            .shape;
        let cylinders: Vec<Shape> = (0..20)
            .map(|position| {
                let column = f64::from(position % 5);
                let row = f64::from(position / 5);
                make_cylinder(
                    &common::xy_plane_at(10.0 + 15.0 * column, 8.0 + 11.0 * row, -1.0),
                    2.0,
                    32.0,
                )
                .expect("the cylinder builds")
                .shape
            })
            .collect();
        median_time_of(RUNS, || cut(&block, &cylinders))
    });

    let block = make_box(&Plane::XY, 10.0, 20.0, 30.0)
        .expect("the box builds")
        .shape;
    // Every edge of the box rounded at radius 1 (kernel spec test 45).
    let every_edge: Vec<EdgeRadius> = (1..=12)
        .map(|edge| EdgeRadius { edge, radius: 1.0 })
        .collect();
    report.measure("fillet, 12 edges of a box", milliseconds(40), || {
        median_time_of(RUNS, || fillet(&block, &every_edge))
    });
    report.measure("shell, box", milliseconds(20), || {
        let top = planar_face(&block, UP, 30.0);
        median_time_of(RUNS, || shell(&block, &[top], 1.0))
    });
    report.measure(
        "shell, rounded box with an opening tangent to its rounds",
        milliseconds(60),
        || {
            let rounded = fillet(&block, &every_edge)
                .expect("every edge rounds")
                .shape;
            let top = planar_face(&rounded, UP, 30.0);
            // A failed shell returns early, so its time would say nothing about the budget.
            shell(&rounded, &[top], 0.5).expect("the rounded box shells open");
            median_time_of(RUNS, || shell(&rounded, &[top], 0.5))
        },
    );
    report.measure("transform, rigid", Duration::from_micros(100), || {
        let axis = Axis {
            origin: Point::new(0.0, 0.0, 0.0),
            direction: UP,
        };
        let motion = Transform::translation(Vector::new(1.0, 2.0, 3.0))
            .then(&Transform::rotation(&axis, 0.5));
        median_time_of(RUNS, || transform(&block, &motion))
    });
    report.measure(
        "elements(Face) and topology(), 200 faces",
        milliseconds(10),
        || {
            let prism = extrude(&profile(polygon(&regular_polygon(198))), UP, 0.0, 10.0)
                .expect("the prism extrudes")
                .shape;
            assert_eq!(prism.count(SubShapeKind::Face), 200);
            median_time_of(RUNS, || {
                (prism.elements(SubShapeKind::Face), prism.topology())
            })
        },
    );
    report.measure(
        "elements of every kind and topology(), PLATE",
        milliseconds(8),
        || {
            let plate = plate();
            median_time_of(RUNS, || {
                (
                    plate.elements(SubShapeKind::Face),
                    plate.elements(SubShapeKind::Edge),
                    plate.elements(SubShapeKind::Vertex),
                    plate.topology(),
                )
            })
        },
    );
    let engine_tolerance = |chord_deviation: f64| TessellationTolerance {
        chord_deviation,
        // The engine's 5 degrees (engine.md section 3.7).
        angular_deviation: 5.0_f64.to_radians(),
    };
    report.measure(
        "tessellate, sphere of radius 250 at 0.0125 mm and 5 degrees",
        milliseconds(1500),
        || {
            let sphere = make_sphere(&Plane::XY, 250.0)
                .expect("the sphere builds")
                .shape;
            median_time_of(RUNS, || sphere.tessellate(&engine_tolerance(0.0125)))
        },
    );
    report.measure(
        "tessellate, PLATE at 0.05 mm and 5 degrees",
        milliseconds(1500),
        || {
            let plate = plate();
            median_time_of(RUNS, || plate.tessellate(&engine_tolerance(0.05)))
        },
    );
    report.measure(
        "width_across, any face across any of its edges, STAIR and test 44",
        milliseconds(5),
        || {
            let rounded_top_front = common::fillet_top_front(&common::box_primitive()).shape;
            slowest_width_across(&common::stair()).max(slowest_width_across(&rounded_top_front))
        },
    );
    report.measure(
        "element_display, one hole wall of PLATE1000 at 0.05 mm and 5 degrees",
        milliseconds(5),
        || {
            let plate = plate_1000();
            let wall = common::faces_of_kind(&plate, GeometryKind::Cylinder)[0];
            let tolerance = engine_tolerance(0.05);
            // A failed display returns early, so its time would say nothing about the budget.
            plate
                .element_display(SubShapeKind::Face, wall, &tolerance)
                .expect("the hole wall displays");
            median_time_of(RUNS, || {
                plate.element_display(SubShapeKind::Face, wall, &tolerance)
            })
        },
    );
    report.measure(
        "shell, L with its step face removed",
        milliseconds(40),
        || {
            let (body, step) = common::l_body();
            // A failed shell returns early, so its time would say nothing about the budget.
            shell(&body, &[step], 2.0).expect("the fallback shells L");
            median_time_of(RUNS, || shell(&body, &[step], 2.0))
        },
    );
    report.measure(
        "shell, rounded cube with walls thicker than its rounds",
        milliseconds(40),
        || {
            let rounded = rounded_cube();
            let top = planar_face(&rounded, UP, 20.0);
            // A failed shell returns early, so its time would say nothing about the budget.
            shell(&rounded, &[top], 3.0).expect("the rounded cube shells past its rounds");
            median_time_of(RUNS, || shell(&rounded, &[top], 3.0))
        },
    );
    report.measure(
        "shell, rounded cube with walls as thick as its rounds",
        milliseconds(200),
        || {
            let rounded = rounded_cube();
            let top = planar_face(&rounded, UP, 20.0);
            shell(&rounded, &[top], 2.0).expect("the rounded cube shells as thick as its rounds");
            median_time_of(RUNS, || shell(&rounded, &[top], 2.0))
        },
    );
    report.measure(
        "shell search, rounded cube at 12 mm",
        milliseconds(600),
        || {
            let rounded = rounded_cube();
            let top = planar_face(&rounded, UP, 20.0);
            let succeeds = |thickness| shell(&rounded, &[top], thickness).is_ok();
            let largest = too_large_search(12.0, succeeds).expect("a thinner wall fits");
            assert!(
                (9.0..10.0).contains(&largest),
                "the walls meet at 10 mm, but the search found {largest} mm"
            );
            median_time_of(RUNS, || too_large_search(12.0, succeeds))
        },
    );

    let part = part();
    println!(
        "The part has {} faces, {} edges and {} vertices.",
        part.count(SubShapeKind::Face),
        part.count(SubShapeKind::Edge),
        part.count(SubShapeKind::Vertex)
    );
    report.measure("volume, part", milliseconds(2), || {
        median_time_of(RUNS, || part.volume())
    });
    report.measure(
        "tessellate, part at 0.05 mm and 0.1 rad",
        milliseconds(30),
        || {
            let tolerance = TessellationTolerance {
                chord_deviation: 0.05,
                angular_deviation: 0.1,
            };
            median_time_of(RUNS, || part.tessellate(&tolerance))
        },
    );
    report.measure("to_brep and from_brep, part", milliseconds(5), || {
        median_time_of(RUNS, || {
            let bytes = part.to_brep().expect("the part serializes");
            Shape::from_brep(&bytes)
        })
    });
    let bodies = [StepBody {
        shape: part.clone(),
        name: "Part".to_owned(),
        color: None,
    }];
    report.measure("write_step, part", milliseconds(100), || {
        median_time_of(RUNS, || write_step(&bodies))
    });
    report.measure("read_step, part", milliseconds(150), || {
        let bytes = write_step(&bodies).expect("the part exports");
        median_time_of(RUNS, || read_step(&bytes))
    });

    report.finish()
}
