//! Known-solution generator for the property tests (spec section 6.13).
//!
//! Entities are built one at a time, each placed so that its relation to an earlier entity holds by
//! construction, and the constraints that state those relations are added with it. Dimensions are
//! then measured on the finished geometry, so the generated geometry (the known solution) satisfies
//! every constraint. The solver's warm start is the known solution plus small noise.

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::{PI, TAU};

use partsmith_sketch::{
    Alignment, ConstraintKind, DimensionExpr, DimensionValues, DistanceTarget, Expr, Geometry, Id,
    OriginPlane, PointRef, PointRole, SketchConstraint, SketchEntity, SketchParams, SketchPlane,
};

use super::random::Rng;

/// The 19 constraint type names of spec section 2.5.
pub const CONSTRAINT_TYPES: [&str; 19] = [
    "coincident",
    "point_on_object",
    "horizontal",
    "vertical",
    "parallel",
    "perpendicular",
    "tangent",
    "collinear",
    "concentric",
    "equal",
    "symmetric",
    "midpoint",
    "fix",
    "distance",
    "horizontal_distance",
    "vertical_distance",
    "radius",
    "diameter",
    "angle",
];

/// One generated case.
#[derive(Clone, Debug)]
pub struct GeneratedCase {
    /// The sketch with its known solution as geometry.
    pub known: SketchParams,
    /// The same sketch with noise added to the geometry: the solver's warm start.
    pub warm_start: SketchParams,
    /// Values of the driving dimensions.
    pub values: DimensionValues,
}

impl GeneratedCase {
    /// True when the case has a `tangent` constraint.
    pub fn has_tangent(&self) -> bool {
        self.known
            .constraints
            .iter()
            .any(|constraint| matches!(constraint.kind, ConstraintKind::Tangent { .. }))
    }

    /// Number of solver parameters (point 2, line 4, circle 3, arc 6; spec section 3.6.1).
    pub fn parameter_count(&self) -> u32 {
        self.known
            .entities
            .iter()
            .map(|entity| match entity.geometry {
                Geometry::Point { .. } => 2,
                Geometry::Line { .. } => 4,
                Geometry::Circle { .. } => 3,
                Geometry::Arc { .. } => 6,
            })
            .sum()
    }

    /// Number of arcs.
    pub fn arc_count(&self) -> u32 {
        self.known
            .entities
            .iter()
            .filter(|entity| matches!(entity.geometry, Geometry::Arc { .. }))
            .count() as u32
    }

    /// Point references with at least one coordinate that the substitution of section 3.7.3 does not freeze.
    pub fn draggable_points(&self) -> Vec<PointRef> {
        let mut classes = CoordinateClasses::default();
        let mut frozen = Vec::new();
        for constraint in &self.known.constraints {
            match &constraint.kind {
                ConstraintKind::Coincident { points } => {
                    classes.merge_point(&points[0], &points[1], &[0, 1]);
                }
                ConstraintKind::Concentric { curves } => {
                    classes.merge_point(&center(&curves[0]), &center(&curves[1]), &[0, 1]);
                }
                ConstraintKind::Horizontal(alignment) => {
                    let (first, second) = alignment_refs(alignment);
                    classes.merge_point(&first, &second, &[1]);
                }
                ConstraintKind::Vertical(alignment) => {
                    let (first, second) = alignment_refs(alignment);
                    classes.merge_point(&first, &second, &[0]);
                }
                ConstraintKind::Fix { target } => frozen.push(target.clone()),
                _ => {}
            }
        }
        let frozen_roots: BTreeSet<String> = frozen
            .iter()
            .flat_map(|point| [0, 1].map(|axis| classes.root(&coordinate_key(point, axis))))
            .collect();
        point_refs(&self.known.entities)
            .into_iter()
            .filter(|point| {
                [0, 1].iter().any(|axis| {
                    !frozen_roots.contains(&classes.root(&coordinate_key(point, *axis)))
                })
            })
            .collect()
    }
}

/// Generates the case for `seed`, or `None` when the case is discarded (spec section 6.13).
pub fn generate(seed: u64) -> Option<GeneratedCase> {
    let mut builder = Builder {
        rng: Rng::new(seed),
        entities: Vec::new(),
        constraints: Vec::new(),
        values: DimensionValues::new(),
    };

    let entity_count = 2 + builder.rng.index(11);
    for _ in 0..entity_count {
        let related = builder.rng.chance(0.6);
        match builder.rng.index(4) {
            0 => builder.add_point(related),
            1 => builder.add_line(related),
            2 => builder.add_curve(false, related),
            _ => builder.add_curve(true, related),
        }
    }

    let dimension_count = builder.rng.index(7);
    for _ in 0..dimension_count {
        builder.add_dimension();
    }

    if builder.rng.chance(0.3) {
        let references = point_refs(&builder.entities);
        let target = builder.rng.pick(&references).clone();
        builder.push_constraint(ConstraintKind::Fix { target });
    }

    if builder.discarded() {
        return None;
    }

    let warm_entities = builder.warm_start();
    let known = sketch(builder.entities, builder.constraints.clone());
    let warm_start = sketch(warm_entities, builder.constraints);
    Some(GeneratedCase {
        known,
        warm_start,
        values: builder.values,
    })
}

fn sketch(entities: Vec<SketchEntity>, constraints: Vec<SketchConstraint>) -> SketchParams {
    SketchParams {
        plane: SketchPlane::Origin {
            plane: OriginPlane::Xy,
        },
        entities,
        constraints,
    }
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

type Vector = [f64; 2];

fn subtract(a: Vector, b: Vector) -> Vector {
    [a[0] - b[0], a[1] - b[1]]
}

fn add(a: Vector, b: Vector) -> Vector {
    [a[0] + b[0], a[1] + b[1]]
}

fn scale(a: Vector, factor: f64) -> Vector {
    [a[0] * factor, a[1] * factor]
}

fn norm(a: Vector) -> f64 {
    a[0].hypot(a[1])
}

fn cross(a: Vector, b: Vector) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

fn dot(a: Vector, b: Vector) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn unit(a: Vector) -> Vector {
    scale(a, 1.0 / norm(a))
}

/// Left normal of a unit direction.
fn left_normal(direction: Vector) -> Vector {
    [-direction[1], direction[0]]
}

fn make_id(text: String) -> Id {
    Id::new(text.as_str()).unwrap_or_else(|_| panic!("{text} is a valid id"))
}

fn center(curve: &Id) -> PointRef {
    PointRef {
        entity: curve.clone(),
        role: PointRole::Center,
    }
}

fn alignment_refs(alignment: &Alignment) -> (PointRef, PointRef) {
    match alignment {
        Alignment::Line { line } => (
            PointRef {
                entity: line.clone(),
                role: PointRole::Start,
            },
            PointRef {
                entity: line.clone(),
                role: PointRole::End,
            },
        ),
        Alignment::Points { points } => (points[0].clone(), points[1].clone()),
    }
}

/// Every point reference of these entities, in entity order.
pub fn point_refs(entities: &[SketchEntity]) -> Vec<PointRef> {
    let mut references = Vec::new();
    for entity in entities {
        let roles: &[PointRole] = match entity.geometry {
            Geometry::Point { .. } => &[PointRole::Point],
            Geometry::Line { .. } => &[PointRole::Start, PointRole::End],
            Geometry::Circle { .. } => &[PointRole::Center],
            Geometry::Arc { .. } => &[PointRole::Start, PointRole::End, PointRole::Center],
        };
        for role in roles {
            references.push(PointRef {
                entity: entity.id.clone(),
                role: *role,
            });
        }
    }
    references
}

/// Coordinates of a point reference on these entities.
pub fn reference_position(entities: &[SketchEntity], point: &PointRef) -> Vector {
    let entity = entities
        .iter()
        .find(|entity| entity.id == point.entity)
        .unwrap_or_else(|| panic!("{point} names an existing entity"));
    match (&entity.geometry, point.role) {
        (Geometry::Point { position }, PointRole::Point) => *position,
        (Geometry::Line { start, .. }, PointRole::Start) => *start,
        (Geometry::Line { end, .. }, PointRole::End) => *end,
        (Geometry::Circle { center, .. }, PointRole::Center)
        | (Geometry::Arc { center, .. }, PointRole::Center) => *center,
        (
            Geometry::Arc {
                center,
                radius,
                start_angle,
                ..
            },
            PointRole::Start,
        ) => add(*center, scale([start_angle.cos(), start_angle.sin()], *radius)),
        (
            Geometry::Arc {
                center,
                radius,
                end_angle,
                ..
            },
            PointRole::End,
        ) => add(*center, scale([end_angle.cos(), end_angle.sin()], *radius)),
        (geometry, role) => panic!("{point} does not resolve: {role:?} of {geometry:?}"),
    }
}

fn coordinate_key(point: &PointRef, axis: usize) -> String {
    format!("{point}/{axis}")
}

/// Union-find over named coordinates.
#[derive(Default)]
struct CoordinateClasses {
    parent: BTreeMap<String, String>,
}

impl CoordinateClasses {
    fn root(&self, key: &str) -> String {
        let mut current = key.to_owned();
        while let Some(parent) = self.parent.get(&current) {
            current = parent.clone();
        }
        current
    }

    fn merge(&mut self, first: &str, second: &str) {
        let first_root = self.root(first);
        let second_root = self.root(second);
        if first_root != second_root {
            self.parent.insert(first_root, second_root);
        }
    }

    fn merge_point(&mut self, first: &PointRef, second: &PointRef, axes: &[usize]) {
        for axis in axes {
            self.merge(&coordinate_key(first, *axis), &coordinate_key(second, *axis));
        }
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

struct Builder {
    rng: Rng,
    entities: Vec<SketchEntity>,
    constraints: Vec<SketchConstraint>,
    values: DimensionValues,
}

#[derive(Clone, Copy)]
enum PointRelation {
    Coincident,
    OnCurve,
    Midpoint,
    Mirror,
}

#[derive(Clone, Copy)]
enum LineRelation {
    StartCoincident,
    Horizontal,
    Vertical,
    Parallel,
    Perpendicular,
    Angle,
    EqualLength,
    Collinear,
}

#[derive(Clone, Copy)]
enum CurveRelation {
    Concentric,
    EqualRadius,
    TangentToLine,
    TangentToCircle,
    TangentAtLineEnd,
}

#[derive(Clone, Copy)]
enum DimensionChoice {
    PointDistance,
    LineLength,
    PointLineDistance,
    AxisDistance,
    Radius,
    Diameter,
    Angle,
}

impl Builder {
    fn next_entity_id(&self) -> Id {
        make_id(format!("e-{}", self.entities.len() + 1))
    }

    fn push_constraint(&mut self, kind: ConstraintKind) -> Id {
        let constraint_id = make_id(format!("c-{}", self.constraints.len() + 1));
        self.constraints.push(SketchConstraint {
            id: constraint_id.clone(),
            kind,
        });
        constraint_id
    }

    fn push_driving(&mut self, kind: ConstraintKind, value: f64) {
        let constraint_id = self.push_constraint(kind);
        self.values.insert(constraint_id, value);
    }

    fn indices(&self, predicate: impl Fn(&Geometry) -> bool) -> Vec<usize> {
        self.entities
            .iter()
            .enumerate()
            .filter(|(_, entity)| predicate(&entity.geometry))
            .map(|(index, _)| index)
            .collect()
    }

    fn lines(&self) -> Vec<usize> {
        self.indices(|geometry| matches!(geometry, Geometry::Line { .. }))
    }

    fn curves(&self) -> Vec<usize> {
        self.indices(|geometry| matches!(geometry, Geometry::Circle { .. } | Geometry::Arc { .. }))
    }

    fn circles(&self) -> Vec<usize> {
        self.indices(|geometry| matches!(geometry, Geometry::Circle { .. }))
    }

    fn non_points(&self) -> Vec<usize> {
        self.indices(|geometry| !matches!(geometry, Geometry::Point { .. }))
    }

    fn random_point(&mut self) -> Vector {
        [
            self.rng.uniform(-100.0, 100.0),
            self.rng.uniform(-100.0, 100.0),
        ]
    }

    fn line_at(&self, index: usize) -> (Vector, Vector) {
        match self.entities[index].geometry {
            Geometry::Line { start, end } => (start, end),
            _ => unreachable!("index {index} was chosen from the lines"),
        }
    }

    fn curve_at(&self, index: usize) -> (Vector, f64) {
        match self.entities[index].geometry {
            Geometry::Circle { center, radius } | Geometry::Arc { center, radius, .. } => {
                (center, radius)
            }
            _ => unreachable!("index {index} was chosen from the circles and arcs"),
        }
    }

    fn position(&self, point: &PointRef) -> Vector {
        reference_position(&self.entities, point)
    }

    fn pick_index(&mut self, indices: &[usize]) -> usize {
        *self.rng.pick(indices)
    }

    fn add_point(&mut self, related: bool) {
        let new_id = self.next_entity_id();
        let references = point_refs(&self.entities);
        let lines = self.lines();
        let non_points = self.non_points();

        let mut options = Vec::new();
        if related {
            if !references.is_empty() {
                options.push(PointRelation::Coincident);
            }
            if !non_points.is_empty() {
                options.push(PointRelation::OnCurve);
            }
            if !lines.is_empty() {
                options.push(PointRelation::Midpoint);
                options.push(PointRelation::Mirror);
            }
        }
        let new_point = PointRef {
            entity: new_id.clone(),
            role: PointRole::Point,
        };

        let choice = if options.is_empty() {
            None
        } else {
            Some(*self.rng.pick(&options))
        };
        let mut relation = None;
        let position = match choice {
            None => self.random_point(),
            Some(PointRelation::Coincident) => {
                let other = self.rng.pick(&references).clone();
                relation = Some(ConstraintKind::Coincident {
                    points: [other.clone(), new_point.clone()],
                });
                self.position(&other)
            }
            Some(PointRelation::OnCurve) => {
                let index = self.pick_index(&non_points);
                let curve = self.entities[index].id.clone();
                relation = Some(ConstraintKind::PointOnObject {
                    point: new_point.clone(),
                    curve,
                });
                match self.entities[index].geometry {
                    Geometry::Line { start, end } => {
                        let fraction = self.rng.uniform(-0.25, 1.25);
                        add(start, scale(subtract(end, start), fraction))
                    }
                    _ => {
                        let (center, radius) = self.curve_at(index);
                        let heading = self.rng.uniform(0.0, TAU);
                        add(center, scale([heading.cos(), heading.sin()], radius))
                    }
                }
            }
            Some(PointRelation::Midpoint) => {
                let index = self.pick_index(&lines);
                let (start, end) = self.line_at(index);
                relation = Some(ConstraintKind::Midpoint {
                    point: new_point.clone(),
                    line: self.entities[index].id.clone(),
                });
                scale(add(start, end), 0.5)
            }
            Some(PointRelation::Mirror) => {
                let index = self.pick_index(&lines);
                let line_id = self.entities[index].id.clone();
                let candidates: Vec<PointRef> = references
                    .iter()
                    .filter(|point| point.entity != line_id)
                    .cloned()
                    .collect();
                if candidates.is_empty() {
                    self.random_point()
                } else {
                    let other = self.rng.pick(&candidates).clone();
                    let (start, end) = self.line_at(index);
                    let direction = unit(subtract(end, start));
                    let other_position = self.position(&other);
                    let along = dot(subtract(other_position, start), direction);
                    let foot = add(start, scale(direction, along));
                    relation = Some(ConstraintKind::Symmetric {
                        points: [other, new_point.clone()],
                        line: line_id,
                    });
                    subtract(scale(foot, 2.0), other_position)
                }
            }
        };

        self.entities.push(SketchEntity {
            id: new_id,
            geometry: Geometry::Point { position },
            construction: false,
        });
        if let Some(kind) = relation {
            self.push_constraint(kind);
        }
    }

    fn add_line(&mut self, related: bool) {
        let new_id = self.next_entity_id();
        let references = point_refs(&self.entities);
        let lines = self.lines();

        let mut start = self.random_point();
        let mut length = self.rng.uniform(5.0, 60.0);
        let heading = self.rng.uniform(0.0, TAU);
        let mut direction = [heading.cos(), heading.sin()];

        let mut options = Vec::new();
        if related {
            if !references.is_empty() {
                options.push(LineRelation::StartCoincident);
            }
            options.push(LineRelation::Horizontal);
            options.push(LineRelation::Vertical);
            if !lines.is_empty() {
                options.extend([
                    LineRelation::Parallel,
                    LineRelation::Perpendicular,
                    LineRelation::Angle,
                    LineRelation::EqualLength,
                    LineRelation::Collinear,
                ]);
            }
        }

        let mut relation = None;
        let mut relation_value = None;
        if !options.is_empty() {
            let choice = *self.rng.pick(&options);
            let other_index = if lines.is_empty() {
                None
            } else {
                Some(self.pick_index(&lines))
            };
            let other = other_index.map(|index| {
                let (other_start, other_end) = self.line_at(index);
                (
                    self.entities[index].id.clone(),
                    other_start,
                    other_end,
                    unit(subtract(other_end, other_start)),
                )
            });
            let sign = self.rng.sign();
            match (choice, other) {
                (LineRelation::StartCoincident, _) => {
                    let point = self.rng.pick(&references).clone();
                    start = self.position(&point);
                    relation = Some(ConstraintKind::Coincident {
                        points: [
                            point,
                            PointRef {
                                entity: new_id.clone(),
                                role: PointRole::Start,
                            },
                        ],
                    });
                }
                (LineRelation::Horizontal, _) => {
                    direction = [sign, 0.0];
                    relation = Some(ConstraintKind::Horizontal(Alignment::Line {
                        line: new_id.clone(),
                    }));
                }
                (LineRelation::Vertical, _) => {
                    direction = [0.0, sign];
                    relation = Some(ConstraintKind::Vertical(Alignment::Line {
                        line: new_id.clone(),
                    }));
                }
                (LineRelation::Parallel, Some((other_id, _, _, other_direction))) => {
                    direction = scale(other_direction, sign);
                    relation = Some(ConstraintKind::Parallel {
                        lines: [other_id, new_id.clone()],
                    });
                }
                (LineRelation::Perpendicular, Some((other_id, _, _, other_direction))) => {
                    direction = scale(left_normal(other_direction), sign);
                    relation = Some(ConstraintKind::Perpendicular {
                        lines: [other_id, new_id.clone()],
                    });
                }
                (LineRelation::Angle, Some((other_id, _, _, other_direction))) => {
                    let turn = self.rng.uniform(0.2, PI - 0.2);
                    let rotated = [
                        other_direction[0] * turn.cos() - other_direction[1] * turn.sin(),
                        other_direction[0] * turn.sin() + other_direction[1] * turn.cos(),
                    ];
                    direction = scale(rotated, sign);
                    relation = Some(ConstraintKind::Angle {
                        lines: [other_id, new_id.clone()],
                        dimension: driving(format!("{turn} rad")),
                    });
                    relation_value = Some(turn);
                }
                (LineRelation::EqualLength, Some((other_id, other_start, other_end, _))) => {
                    length = norm(subtract(other_end, other_start));
                    relation = Some(ConstraintKind::Equal {
                        curves: [other_id, new_id.clone()],
                    });
                }
                (LineRelation::Collinear, Some((other_id, other_start, other_end, other_direction))) => {
                    let fraction = self.rng.uniform(-1.0, 2.0);
                    start = add(other_start, scale(subtract(other_end, other_start), fraction));
                    direction = scale(other_direction, sign);
                    relation = Some(ConstraintKind::Collinear {
                        lines: [other_id, new_id.clone()],
                    });
                }
                _ => {}
            }
        }

        let end = add(start, scale(direction, length));
        self.entities.push(SketchEntity {
            id: new_id,
            geometry: Geometry::Line { start, end },
            construction: false,
        });
        match (relation, relation_value) {
            (Some(kind), Some(value)) => self.push_driving(kind, value),
            (Some(kind), None) => {
                self.push_constraint(kind);
            }
            _ => {}
        }
    }

    fn add_curve(&mut self, is_arc: bool, related: bool) {
        let new_id = self.next_entity_id();
        let lines = self.lines();
        let curves = self.curves();
        let circles = self.circles();

        let mut center = self.random_point();
        let mut radius = self.rng.uniform(2.0, 30.0);
        let mut start_angle = self.rng.uniform(0.0, TAU);
        let sweep = self.rng.uniform(0.3, TAU - 0.3);

        let mut options = Vec::new();
        if related {
            if !curves.is_empty() {
                options.push(CurveRelation::Concentric);
                options.push(CurveRelation::EqualRadius);
            }
            if !lines.is_empty() {
                options.push(CurveRelation::TangentToLine);
                if is_arc {
                    options.push(CurveRelation::TangentAtLineEnd);
                }
            }
            if !circles.is_empty() {
                options.push(CurveRelation::TangentToCircle);
            }
        }

        let mut relations = Vec::new();
        if !options.is_empty() {
            match *self.rng.pick(&options) {
                CurveRelation::Concentric => {
                    let index = self.pick_index(&curves);
                    center = self.curve_at(index).0;
                    relations.push(ConstraintKind::Concentric {
                        curves: [self.entities[index].id.clone(), new_id.clone()],
                    });
                }
                CurveRelation::EqualRadius => {
                    let index = self.pick_index(&curves);
                    radius = self.curve_at(index).1;
                    relations.push(ConstraintKind::Equal {
                        curves: [self.entities[index].id.clone(), new_id.clone()],
                    });
                }
                CurveRelation::TangentToLine => {
                    let index = self.pick_index(&lines);
                    let (start, end) = self.line_at(index);
                    let fraction = self.rng.uniform(0.1, 0.9);
                    let foot = add(start, scale(subtract(end, start), fraction));
                    let side = self.rng.sign();
                    let normal = left_normal(unit(subtract(end, start)));
                    center = add(foot, scale(normal, side * radius));
                    relations.push(ConstraintKind::Tangent {
                        curves: [self.entities[index].id.clone(), new_id.clone()],
                    });
                }
                CurveRelation::TangentToCircle => {
                    let index = self.pick_index(&circles);
                    let (other_center, other_radius) = self.curve_at(index);
                    let heading = self.rng.uniform(0.0, TAU);
                    center = add(
                        other_center,
                        scale([heading.cos(), heading.sin()], other_radius + radius),
                    );
                    relations.push(ConstraintKind::Tangent {
                        curves: [self.entities[index].id.clone(), new_id.clone()],
                    });
                }
                CurveRelation::TangentAtLineEnd => {
                    let index = self.pick_index(&lines);
                    let (start, end) = self.line_at(index);
                    let side = self.rng.sign();
                    let normal = left_normal(unit(subtract(end, start)));
                    center = add(end, scale(normal, side * radius));
                    let toward_end = subtract(end, center);
                    start_angle = toward_end[1].atan2(toward_end[0]).rem_euclid(TAU);
                    let line_id = self.entities[index].id.clone();
                    relations.push(ConstraintKind::Coincident {
                        points: [
                            PointRef {
                                entity: line_id.clone(),
                                role: PointRole::End,
                            },
                            PointRef {
                                entity: new_id.clone(),
                                role: PointRole::Start,
                            },
                        ],
                    });
                    relations.push(ConstraintKind::Tangent {
                        curves: [line_id, new_id.clone()],
                    });
                }
            }
        }

        let geometry = if is_arc {
            Geometry::Arc {
                center,
                radius,
                start_angle,
                end_angle: start_angle + sweep,
            }
        } else {
            Geometry::Circle { center, radius }
        };
        self.entities.push(SketchEntity {
            id: new_id,
            geometry,
            construction: false,
        });
        for kind in relations {
            self.push_constraint(kind);
        }
    }

    fn add_dimension(&mut self) {
        let references = point_refs(&self.entities);
        let lines = self.lines();
        let curves = self.curves();

        let mut options = Vec::new();
        if references.len() >= 2 {
            options.push(DimensionChoice::PointDistance);
            options.push(DimensionChoice::AxisDistance);
        }
        if !lines.is_empty() {
            options.push(DimensionChoice::LineLength);
            options.push(DimensionChoice::PointLineDistance);
        }
        if !curves.is_empty() {
            options.push(DimensionChoice::Radius);
            options.push(DimensionChoice::Diameter);
        }
        if lines.len() >= 2 {
            options.push(DimensionChoice::Angle);
        }
        if options.is_empty() {
            return;
        }

        let is_reference = self.rng.chance(0.2);
        let measured = match *self.rng.pick(&options) {
            DimensionChoice::PointDistance => {
                let first = self.rng.pick(&references).clone();
                let second = self.rng.pick(&references).clone();
                let distance = norm(subtract(self.position(&second), self.position(&first)));
                (first != second && distance >= 1.0).then(|| {
                    (
                        ConstraintKind::Distance {
                            target: DistanceTarget::Points {
                                points: [first, second],
                            },
                            dimension: driving(format!("{distance} mm")),
                        },
                        distance,
                    )
                })
            }
            DimensionChoice::LineLength => {
                let index = self.pick_index(&lines);
                let (start, end) = self.line_at(index);
                let length = norm(subtract(end, start));
                Some((
                    ConstraintKind::Distance {
                        target: DistanceTarget::Line {
                            line: self.entities[index].id.clone(),
                        },
                        dimension: driving(format!("{length} mm")),
                    },
                    length,
                ))
            }
            DimensionChoice::PointLineDistance => {
                let index = self.pick_index(&lines);
                let line_id = self.entities[index].id.clone();
                let candidates: Vec<PointRef> = references
                    .iter()
                    .filter(|point| point.entity != line_id)
                    .cloned()
                    .collect();
                if candidates.is_empty() {
                    None
                } else {
                    let point = self.rng.pick(&candidates).clone();
                    let (start, end) = self.line_at(index);
                    let direction = subtract(end, start);
                    let signed = cross(direction, subtract(self.position(&point), start))
                        / norm(direction);
                    let distance = signed.abs();
                    (distance >= 0.5).then(|| {
                        (
                            ConstraintKind::Distance {
                                target: DistanceTarget::PointLine {
                                    point,
                                    line: line_id,
                                },
                                dimension: driving(format!("{distance} mm")),
                            },
                            distance,
                        )
                    })
                }
            }
            DimensionChoice::AxisDistance => {
                let vertical = self.rng.chance(0.5);
                let axis = usize::from(vertical);
                let use_line = !lines.is_empty() && self.rng.chance(0.5);
                let target = if use_line {
                    let index = self.pick_index(&lines);
                    Alignment::Line {
                        line: self.entities[index].id.clone(),
                    }
                } else {
                    let first = self.rng.pick(&references).clone();
                    let second = self.rng.pick(&references).clone();
                    Alignment::Points {
                        points: [first, second],
                    }
                };
                let (first, second) = alignment_refs(&target);
                let signed = self.position(&second)[axis] - self.position(&first)[axis];
                (first != second && signed.abs() >= 0.5).then(|| {
                    let dimension = driving(format!("{signed} mm"));
                    let kind = if vertical {
                        ConstraintKind::VerticalDistance { target, dimension }
                    } else {
                        ConstraintKind::HorizontalDistance { target, dimension }
                    };
                    (kind, signed)
                })
            }
            DimensionChoice::Radius => {
                let index = self.pick_index(&curves);
                let radius = self.curve_at(index).1;
                Some((
                    ConstraintKind::Radius {
                        curve: self.entities[index].id.clone(),
                        dimension: driving(format!("{radius} mm")),
                    },
                    radius,
                ))
            }
            DimensionChoice::Diameter => {
                let index = self.pick_index(&curves);
                let diameter = 2.0 * self.curve_at(index).1;
                Some((
                    ConstraintKind::Diameter {
                        curve: self.entities[index].id.clone(),
                        dimension: driving(format!("{diameter} mm")),
                    },
                    diameter,
                ))
            }
            DimensionChoice::Angle => {
                let first = self.pick_index(&lines);
                let second = self.pick_index(&lines);
                if first == second {
                    None
                } else {
                    let (first_start, first_end) = self.line_at(first);
                    let (second_start, second_end) = self.line_at(second);
                    let first_direction = subtract(first_end, first_start);
                    let second_direction = subtract(second_end, second_start);
                    let theta = cross(first_direction, second_direction)
                        .atan2(dot(first_direction, second_direction))
                        .rem_euclid(PI);
                    ((0.1..=PI - 0.1).contains(&theta)).then(|| {
                        (
                            ConstraintKind::Angle {
                                lines: [
                                    self.entities[first].id.clone(),
                                    self.entities[second].id.clone(),
                                ],
                                dimension: driving(format!("{theta} rad")),
                            },
                            theta,
                        )
                    })
                }
            }
        };

        if let Some((mut kind, value)) = measured {
            if is_reference {
                set_reference(&mut kind);
                self.push_constraint(kind);
            } else {
                self.push_driving(kind, value);
            }
        }
    }

    /// The discard rules of spec section 6.13.
    fn discarded(&self) -> bool {
        for entity in &self.entities {
            match entity.geometry {
                Geometry::Line { start, end } if norm(subtract(end, start)) < 1.0 => return true,
                Geometry::Circle { radius, .. } | Geometry::Arc { radius, .. } if radius < 1.0 => {
                    return true;
                }
                _ => {}
            }
        }

        let references = point_refs(&self.entities);
        let positions: Vec<Vector> = references
            .iter()
            .map(|point| self.position(point))
            .collect();
        if positions
            .iter()
            .any(|position| position[0].abs() > 1000.0 || position[1].abs() > 1000.0)
        {
            return true;
        }

        // Identified points: connected through coincident constraints, plus the shared centers of
        // concentric curves (which coincide by construction). The x coordinate key stands for the point.
        let mut classes = CoordinateClasses::default();
        for constraint in &self.constraints {
            match &constraint.kind {
                ConstraintKind::Coincident { points } => {
                    classes.merge_point(&points[0], &points[1], &[0]);
                }
                ConstraintKind::Concentric { curves } => {
                    classes.merge_point(&center(&curves[0]), &center(&curves[1]), &[0]);
                }
                _ => {}
            }
        }
        for (i, first) in references.iter().enumerate() {
            for (second, second_position) in references.iter().zip(&positions).skip(i + 1) {
                let identified = classes.root(&coordinate_key(first, 0))
                    == classes.root(&coordinate_key(second, 0));
                if !identified && norm(subtract(positions[i], *second_position)) < 0.1 {
                    return true;
                }
            }
        }
        false
    }

    /// The known solution plus uniform noise; entities referenced by a `fix` keep the known solution.
    fn warm_start(&mut self) -> Vec<SketchEntity> {
        let fixed: BTreeSet<Id> = self
            .constraints
            .iter()
            .filter_map(|constraint| match &constraint.kind {
                ConstraintKind::Fix { target } => Some(target.entity.clone()),
                _ => None,
            })
            .collect();
        let entities = self.entities.clone();
        entities
            .into_iter()
            .map(|mut entity| {
                if fixed.contains(&entity.id) {
                    return entity;
                }
                entity.geometry = match entity.geometry {
                    Geometry::Point { position } => Geometry::Point {
                        position: self.jitter(position),
                    },
                    Geometry::Line { start, end } => Geometry::Line {
                        start: self.jitter(start),
                        end: self.jitter(end),
                    },
                    Geometry::Circle { center, radius } => Geometry::Circle {
                        center: self.jitter(center),
                        radius: radius + self.rng.uniform(-0.01, 0.01),
                    },
                    Geometry::Arc {
                        center,
                        radius,
                        start_angle,
                        end_angle,
                    } => Geometry::Arc {
                        center: self.jitter(center),
                        radius: radius + self.rng.uniform(-0.01, 0.01),
                        start_angle: start_angle + self.rng.uniform(-1e-4, 1e-4),
                        end_angle: end_angle + self.rng.uniform(-1e-4, 1e-4),
                    },
                };
                entity
            })
            .collect()
    }

    fn jitter(&mut self, point: Vector) -> Vector {
        [
            point[0] + self.rng.uniform(-0.01, 0.01),
            point[1] + self.rng.uniform(-0.01, 0.01),
        ]
    }
}

fn driving(expr: String) -> DimensionExpr {
    DimensionExpr {
        expr: Expr::new(expr),
        driving: true,
    }
}

fn set_reference(kind: &mut ConstraintKind) {
    if let ConstraintKind::Distance { dimension, .. }
    | ConstraintKind::HorizontalDistance { dimension, .. }
    | ConstraintKind::VerticalDistance { dimension, .. }
    | ConstraintKind::Radius { dimension, .. }
    | ConstraintKind::Diameter { dimension, .. }
    | ConstraintKind::Angle { dimension, .. } = kind
    {
        dimension.expr = Expr::new("1 mm");
        dimension.driving = false;
    }
}
