//! Generated benchmark sketches of spec sections 5.2 and 5.3.
//!
//! This file depends only on `partsmith_sketch` and `std`, because the bench target
//! (`benches/solver.rs`) includes it with `#[path]`.

// The bench target and each test binary use only some of these functions.
#![allow(dead_code)]

use std::f64::consts::{FRAC_PI_2, FRAC_PI_6};

use partsmith_sketch::{
    ConstraintKind, DimensionExpr, DimensionValues, DistanceTarget, Expr, Geometry, Id,
    OriginPlane, PointRef, SketchConstraint, SketchEntity, SketchParams, SketchPlane,
};

/// Number of slot links in the chain benchmark.
pub const CHAIN_LINKS: usize = 25;

/// Id of the dragged point in the chain drag scenario.
pub const CHAIN_FREE_END: &str = "e-24-ar.center";

/// The chain benchmark of spec section 5.2: 25 links, 200 entities, 500 constraints, 25 DOF.
pub fn chain_benchmark() -> (SketchParams, DimensionValues) {
    chain(CHAIN_LINKS)
}

/// The first `links` links of the chain benchmark (section 5.2 with `k = 0..links`).
pub fn chain(links: usize) -> (SketchParams, DimensionValues) {
    let mut entities = Vec::with_capacity(8 * links);
    let mut constraints = Vec::with_capacity(20 * links);
    let mut values = DimensionValues::new();
    let mut pivot = [0.0_f64, 0.0_f64];

    for k in 0..links {
        let theta = if k % 2 == 0 { FRAC_PI_6 } else { -FRAC_PI_6 };
        let direction = [theta.cos(), theta.sin()];
        let normal = [-theta.sin(), theta.cos()];
        let next_pivot = [
            pivot[0] + 20.0 * direction[0],
            pivot[1] + 20.0 * direction[1],
        ];
        let middle = [
            pivot[0] + 10.0 * direction[0],
            pivot[1] + 10.0 * direction[1],
        ];
        let offset =
            |base: [f64; 2], scale: f64| [base[0] + scale * normal[0], base[1] + scale * normal[1]];

        let name = |role: &str| format!("e-{k}-{role}");
        entities.push(entity(
            &name("t"),
            Geometry::Line {
                start: offset(pivot, 5.0),
                end: offset(next_pivot, 5.0),
            },
            false,
        ));
        entities.push(entity(
            &name("ar"),
            Geometry::Arc {
                center: next_pivot,
                radius: 5.0,
                start_angle: theta - FRAC_PI_2,
                end_angle: theta + FRAC_PI_2,
            },
            false,
        ));
        entities.push(entity(
            &name("b"),
            Geometry::Line {
                start: offset(next_pivot, -5.0),
                end: offset(pivot, -5.0),
            },
            false,
        ));
        entities.push(entity(
            &name("al"),
            Geometry::Arc {
                center: pivot,
                radius: 5.0,
                start_angle: theta + FRAC_PI_2,
                end_angle: theta + 3.0 * FRAC_PI_2,
            },
            false,
        ));
        entities.push(entity(
            &name("c"),
            Geometry::Circle {
                center: next_pivot,
                radius: 2.0,
            },
            false,
        ));
        entities.push(entity(
            &name("m"),
            Geometry::Line {
                start: pivot,
                end: next_pivot,
            },
            true,
        ));
        entities.push(entity(
            &name("p"),
            Geometry::Point { position: middle },
            false,
        ));
        entities.push(entity(
            &name("n"),
            Geometry::Line {
                start: offset(middle, -5.0),
                end: offset(middle, 5.0),
            },
            true,
        ));

        let reference = |text: String| -> PointRef {
            text.parse()
                .unwrap_or_else(|_| panic!("{text} is a valid point reference"))
        };
        let points = |first: String, second: String| [reference(first), reference(second)];
        let mut add = |number: usize, kind: ConstraintKind| {
            constraints.push(SketchConstraint {
                id: id(&format!("c-{k}-{number}")),
                kind,
            });
        };

        add(
            1,
            ConstraintKind::Coincident {
                points: points(name("t.end"), name("ar.end")),
            },
        );
        add(
            2,
            ConstraintKind::Coincident {
                points: points(name("ar.start"), name("b.start")),
            },
        );
        add(
            3,
            ConstraintKind::Coincident {
                points: points(name("b.end"), name("al.end")),
            },
        );
        add(
            4,
            ConstraintKind::Coincident {
                points: points(name("al.start"), name("t.start")),
            },
        );
        add(
            5,
            ConstraintKind::Tangent {
                curves: [id(&name("t")), id(&name("ar"))],
            },
        );
        add(
            6,
            ConstraintKind::Tangent {
                curves: [id(&name("b")), id(&name("ar"))],
            },
        );
        add(
            7,
            ConstraintKind::Tangent {
                curves: [id(&name("b")), id(&name("al"))],
            },
        );
        add(
            8,
            ConstraintKind::Tangent {
                curves: [id(&name("t")), id(&name("al"))],
            },
        );
        add(
            9,
            ConstraintKind::Equal {
                curves: [id(&name("ar")), id(&name("al"))],
            },
        );
        add(
            10,
            ConstraintKind::Radius {
                curve: id(&name("ar")),
                dimension: driving("5 mm"),
            },
        );
        add(
            11,
            ConstraintKind::Coincident {
                points: points(name("m.start"), name("al.center")),
            },
        );
        add(
            12,
            ConstraintKind::Coincident {
                points: points(name("m.end"), name("ar.center")),
            },
        );
        add(
            13,
            ConstraintKind::Distance {
                target: DistanceTarget::Line {
                    line: id(&name("m")),
                },
                dimension: driving("20 mm"),
            },
        );
        add(
            14,
            ConstraintKind::Midpoint {
                point: reference(name("p")),
                line: id(&name("m")),
            },
        );
        add(
            15,
            ConstraintKind::Concentric {
                curves: [id(&name("c")), id(&name("ar"))],
            },
        );
        add(
            16,
            ConstraintKind::Diameter {
                curve: id(&name("c")),
                dimension: driving("4 mm"),
            },
        );
        add(
            17,
            ConstraintKind::Symmetric {
                points: points(name("n.start"), name("n.end")),
                line: id(&name("m")),
            },
        );
        add(
            18,
            ConstraintKind::PointOnObject {
                point: reference(name("p")),
                curve: id(&name("n")),
            },
        );
        add(
            19,
            ConstraintKind::Distance {
                target: DistanceTarget::Line {
                    line: id(&name("n")),
                },
                dimension: driving("10 mm"),
            },
        );
        if k == 0 {
            add(
                20,
                ConstraintKind::Fix {
                    target: reference(name("al.center")),
                },
            );
        } else {
            add(
                20,
                ConstraintKind::Coincident {
                    points: points(format!("e-{}-ar.center", k - 1), name("al.center")),
                },
            );
        }

        values.insert(id(&format!("c-{k}-10")), 5.0);
        values.insert(id(&format!("c-{k}-13")), 20.0);
        values.insert(id(&format!("c-{k}-16")), 4.0);
        values.insert(id(&format!("c-{k}-19")), 10.0);

        pivot = next_pivot;
    }

    let sketch = SketchParams {
        plane: SketchPlane::Origin {
            plane: OriginPlane::Xy,
        },
        entities,
        constraints,
    };
    (sketch, values)
}

/// The grid benchmark of spec section 5.3: 20 horizontal lines, 20 vertical lines and a circle in each of the 361 cells.
pub fn grid_benchmark() -> Vec<SketchEntity> {
    let mut entities = Vec::with_capacity(401);
    for i in 0..20 {
        let v = 5.0 * i as f64;
        entities.push(entity(
            &format!("e-h-{i}"),
            Geometry::Line {
                start: [-5.0, v],
                end: [100.0, v],
            },
            false,
        ));
    }
    for j in 0..20 {
        let u = 5.0 * j as f64;
        entities.push(entity(
            &format!("e-v-{j}"),
            Geometry::Line {
                start: [u, -5.0],
                end: [u, 100.0],
            },
            false,
        ));
    }
    for i in 0..19 {
        for j in 0..19 {
            entities.push(entity(
                &format!("e-c-{i}-{j}"),
                Geometry::Circle {
                    center: [5.0 * j as f64 + 2.5, 5.0 * i as f64 + 2.5],
                    radius: 1.0,
                },
                false,
            ));
        }
    }
    entities
}

fn id(text: &str) -> Id {
    Id::new(text).unwrap_or_else(|_| panic!("{text} is a valid id"))
}

fn entity(entity_id: &str, geometry: Geometry, construction: bool) -> SketchEntity {
    SketchEntity {
        id: id(entity_id),
        geometry,
        construction,
    }
}

fn driving(expr: &str) -> DimensionExpr {
    DimensionExpr {
        expr: Expr::new(expr),
        driving: true,
    }
}
