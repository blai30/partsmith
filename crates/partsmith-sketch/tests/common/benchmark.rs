//! Benchmark sketches: the plate of spec section 2.9 and the generated sketches of sections 5.2 and 5.3,
//! including the split grid and its cells, and the long outline.
//!
//! This file depends only on `partsmith_sketch` and `std`, because the bench target
//! (`benches/solver.rs`) includes it with `#[path]`.

// The bench target and each test binary use only some of these functions.
#![allow(dead_code)]

use std::f64::consts::{FRAC_PI_2, FRAC_PI_6, TAU};

use partsmith_sketch::{
    ConstraintKind, DimensionExpr, DimensionValues, DistanceTarget, Expr, Geometry, Id,
    OriginPlane, PointRef, Region, SketchConstraint, SketchEntity, SketchParams, SketchPlane,
};

/// The `PLATE` sketch of spec section 2.9 as `SketchParams` JSON (the feature record without `id`, `type` and `name`).
pub const PLATE_PARAMS_JSON: &str = r#"{
  "plane": {"kind": "origin", "plane": "XY"},
  "entities": [
    {"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]},
    {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]},
    {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]},
    {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]},
    {"id": "e-5", "type": "circle", "center": [40, 25], "radius": 5},
    {"id": "e-6", "type": "line", "start": [40, 0], "end": [40, 50], "construction": true}
  ],
  "constraints": [
    {"id": "c-1", "type": "coincident", "points": ["e-1.end", "e-2.start"]},
    {"id": "c-2", "type": "coincident", "points": ["e-2.end", "e-3.start"]},
    {"id": "c-3", "type": "coincident", "points": ["e-3.end", "e-4.start"]},
    {"id": "c-4", "type": "coincident", "points": ["e-4.end", "e-1.start"]},
    {"id": "c-5", "type": "horizontal", "line": "e-1"},
    {"id": "c-6", "type": "horizontal", "line": "e-3"},
    {"id": "c-7", "type": "vertical", "line": "e-2"},
    {"id": "c-8", "type": "vertical", "line": "e-4"},
    {"id": "c-9", "type": "fix", "target": "e-1.start"},
    {"id": "c-10", "type": "distance", "line": "e-1", "expr": "width"},
    {"id": "c-11", "type": "distance", "line": "e-2", "expr": "50 mm"},
    {"id": "c-12", "type": "midpoint", "point": "e-6.start", "line": "e-1"},
    {"id": "c-13", "type": "midpoint", "point": "e-6.end", "line": "e-3"},
    {"id": "c-14", "type": "point_on_object", "point": "e-5.center", "curve": "e-6"},
    {"id": "c-15", "type": "vertical_distance", "points": ["e-1.start", "e-5.center"], "expr": "25 mm"},
    {"id": "c-16", "type": "diameter", "curve": "e-5", "expr": "hole_d"}
  ]
}"#;

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

/// Number of first-row cells that the line of the split grid divides (u from 0 to 50).
pub const SPLIT_CELLS: usize = 10;

/// Id of the line the split grid adds.
pub const SPLIT_LINE: &str = "e-x";

/// The split grid of spec section 5.3: the grid benchmark plus `line e-x (-3, 1.2)->(50, 1.2)`.
pub fn split_grid_benchmark() -> Vec<SketchEntity> {
    let mut entities = grid_benchmark();
    entities.push(entity(
        SPLIT_LINE,
        Geometry::Line {
            start: [-3.0, 1.2],
            end: [50.0, 1.2],
        },
        false,
    ));
    entities
}

/// Number of lines of the long outline.
pub const LONG_OUTLINE_LINES: usize = 1000;

/// Id of the line the redrawn long outline draws in place of `e-p-0`.
pub const REDRAWN_LINE: &str = "e-q";

/// The long outline of spec section 5.3: a closed polygon of 1000 lines `e-p-<i>` with corners on a circle of
/// radius 100.
pub fn long_outline_benchmark() -> Vec<SketchEntity> {
    let corners: Vec<[f64; 2]> = (0..LONG_OUTLINE_LINES)
        .map(|i| {
            let angle = TAU * i as f64 / LONG_OUTLINE_LINES as f64;
            [100.0 * angle.cos(), 100.0 * angle.sin()]
        })
        .collect();
    (0..LONG_OUTLINE_LINES)
        .map(|i| {
            entity(
                &format!("e-p-{i}"),
                Geometry::Line {
                    start: corners[i],
                    end: corners[(i + 1) % LONG_OUTLINE_LINES],
                },
                false,
            )
        })
        .collect()
}

/// The redrawn long outline of spec section 5.3: `e-p-0` deleted, and `e-q` drawn between the same points after
/// the other lines.
pub fn redrawn_long_outline_benchmark() -> Vec<SketchEntity> {
    let mut entities = long_outline_benchmark();
    let deleted = entities.remove(0);
    entities.push(entity(REDRAWN_LINE, deleted.geometry, false));
    entities
}

/// The ids of these entities, in their order: the `entities` argument of `match_region`.
pub fn entity_ids(entities: &[SketchEntity]) -> Vec<Id> {
    entities.iter().map(|entity| entity.id.clone()).collect()
}

/// Index of the grid cell in `row` and `column` among regions of the grid benchmark: the region bounded by
/// `e-h-<row>`, `e-h-<row + 1>`, `e-v-<column>` and `e-v-<column + 1>` with the hole `e-c-<row>-<column>`.
pub fn cell(regions: &[Region], row: usize, column: usize) -> usize {
    let mut sides = [
        format!("e-h-{row}"),
        format!("e-h-{}", row + 1),
        format!("e-v-{column}"),
        format!("e-v-{}", column + 1),
    ];
    sides.sort();
    let sides: Vec<&str> = sides.iter().map(String::as_str).collect();
    let hole = format!("e-c-{row}-{column}");
    let found: Vec<usize> = regions
        .iter()
        .enumerate()
        .filter(|(_, region)| {
            let mut outer: Vec<&str> = region
                .key
                .outer()
                .iter()
                .map(|entry| entry.entity.as_str())
                .collect();
            outer.sort();
            outer.dedup();
            let holes: Vec<Vec<&str>> = region
                .key
                .holes()
                .iter()
                .map(|loop_entries| {
                    loop_entries
                        .iter()
                        .map(|entry| entry.entity.as_str())
                        .collect()
                })
                .collect();
            outer == sides && holes == [vec![hole.as_str()]]
        })
        .map(|(index, _)| index)
        .collect();
    match found.as_slice() {
        [index] => *index,
        _ => panic!("cell ({row}, {column}) is exactly one region, found {found:?}"),
    }
}

/// Index of the first cell (spec section 5.3) among regions of the grid benchmark.
pub fn first_cell(regions: &[Region]) -> usize {
    cell(regions, 0, 0)
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
