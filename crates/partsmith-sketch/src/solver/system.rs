//! The equation system of a sketch: rows with their warm-start choices, fix values and components
//! (spec sections 3.6 and 3.7.1).

use std::collections::BTreeMap;
use std::ops::Range;

use partsmith_model::Id;

use super::layout::Layout;
use super::rows::{Anchor, Equality, LineTerm, PointTerm, Row, RowOwner, Term};
use crate::ORIGIN;
use crate::constraint::{Alignment, ConstraintKind, DistanceTarget, SketchConstraint};
use crate::entity::{Geometry, SketchEntity};
use crate::params::{DimensionValues, SketchParams};
use crate::point_ref::{PointRef, PointRole};
use crate::reserved::{axis_line, is_reserved};
use crate::union_find::UnionFind;
use crate::vector::{norm, signed_distance, subtract, unit};

/// The rows of a sketch and how they split into components.
#[derive(Clone, Debug)]
pub(crate) struct System {
    /// Where each entity's parameters sit.
    pub(crate) layout: Layout,
    /// Every row in the order of section 3.6.5: arc rules in entity order, then constraint rows in document order.
    pub(crate) rows: Vec<Row>,
    /// For each constraint, the indices of its rows.
    pub(crate) constraint_rows: Vec<Range<usize>>,
    /// For each constraint, the parameters a `fix` freezes with their fix values (empty for other types).
    pub(crate) freezes: Vec<Vec<(usize, f64)>>,
    /// Connected components in the document order of their first entity.
    pub(crate) components: Vec<Component>,
}

/// A connected set of entities with the rows and constraints that belong to it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Component {
    /// Entity indices, ascending.
    pub(crate) entities: Vec<usize>,
    /// Parameter indices, ascending.
    pub(crate) parameters: Vec<usize>,
    /// Row indices, ascending.
    pub(crate) rows: Vec<usize>,
    /// Constraint indices in document order.
    pub(crate) constraints: Vec<usize>,
}

impl System {
    /// Builds the rows of a validated sketch whose values were checked; warm-start choices and fix values come from `choices` (section 3.6.3).
    pub(crate) fn new(
        sketch: &SketchParams,
        values: &DimensionValues,
        choices: &[SketchEntity],
    ) -> System {
        let layout = Layout::new(&sketch.entities);
        let choice_vector = layout.vector(choices);
        let builder = Builder {
            sketch,
            layout: &layout,
            x: &choice_vector,
            identification: Identification::new(sketch),
            points_on: points_on_curves(sketch),
        };
        let mut rows = Vec::new();
        for (index, entity) in sketch.entities.iter().enumerate() {
            if matches!(entity.geometry, Geometry::Arc { .. }) {
                let center = layout.parameters(index).start;
                let length = |point| Term::Length {
                    from: PointTerm::Parameters(center),
                    to: PointTerm::Parameters(point),
                };
                rows.push(Row::new(
                    RowOwner::ArcRule(index),
                    vec![(1.0, length(center + 4)), (-1.0, length(center + 2))],
                    0.0,
                ));
            }
        }
        let mut constraint_rows = Vec::with_capacity(sketch.constraints.len());
        let mut freezes = Vec::with_capacity(sketch.constraints.len());
        for (index, constraint) in sketch.constraints.iter().enumerate() {
            let (new_rows, freeze) = builder.constraint(index, constraint, values);
            constraint_rows.push(rows.len()..rows.len() + new_rows.len());
            rows.extend(new_rows);
            freezes.push(freeze);
        }
        let components = components(sketch, &layout, &rows);
        System {
            layout,
            rows,
            constraint_rows,
            freezes,
            components,
        }
    }
}

/// Builds rows on the warm-start choice vector `x`.
struct Builder<'a> {
    sketch: &'a SketchParams,
    layout: &'a Layout,
    x: &'a [f64],
    identification: Identification,
    /// For each curve, the `point_on_object` constraints on it as (constraint index, point).
    points_on: BTreeMap<&'a Id, Vec<(usize, &'a PointRef)>>,
}

/// A circle or arc as row terms.
#[derive(Clone, Copy)]
struct CurveTerm {
    center: PointTerm,
    /// `r` for a circle, `norm(s - c)` for an arc.
    radius: Term,
}

impl Builder<'_> {
    fn entity(&self, id: &Id) -> (usize, &Geometry) {
        let index = self
            .layout
            .entity_index(id)
            .expect("validation guarantees that constraints name existing entities");
        (index, &self.sketch.entities[index].geometry)
    }

    fn point(&self, point: &PointRef) -> PointTerm {
        if point.entity.as_str() == ORIGIN {
            return PointTerm::Constant([0.0, 0.0]);
        }
        let (index, geometry) = self.entity(&point.entity);
        let parameter = self
            .layout
            .point_parameter(index, geometry, point.role)
            .expect("validation guarantees that point roles exist on their entities");
        PointTerm::Parameters(parameter)
    }

    fn line(&self, line: &Id) -> LineTerm {
        if let Some((start, end)) = axis_line(line.as_str()) {
            return LineTerm {
                start: PointTerm::Constant(start),
                end: PointTerm::Constant(end),
            };
        }
        let start = self.layout.parameters(self.entity(line).0).start;
        LineTerm {
            start: PointTerm::Parameters(start),
            end: PointTerm::Parameters(start + 2),
        }
    }

    fn curve(&self, curve: &Id) -> CurveTerm {
        let (index, geometry) = self.entity(curve);
        let center = self.layout.parameters(index).start;
        let radius = match geometry {
            Geometry::Circle { .. } => Term::Coordinate(center + 2),
            _ => Term::Length {
                from: PointTerm::Parameters(center),
                to: PointTerm::Parameters(center + 2),
            },
        };
        CurveTerm {
            center: PointTerm::Parameters(center),
            radius,
        }
    }

    /// True for a line entity (or an axis) in a curve field.
    fn is_straight(&self, curve: &Id) -> bool {
        is_reserved(curve.as_str()) || matches!(self.entity(curve).1, Geometry::Line { .. })
    }

    /// `size` of section 3.6.3, or `None` for an axis.
    fn size(&self, entity: &Id) -> Option<f64> {
        if is_reserved(entity.as_str()) {
            return None;
        }
        Some(if self.is_straight(entity) {
            let line = self.line(entity);
            norm(subtract(line.end.value(self.x), line.start.value(self.x)))
        } else {
            self.curve(entity).radius.value(self.x)
        })
    }

    /// The angular scale `S` of section 3.6.3.
    fn angular_scale(&self, first: &Id, second: &Id) -> f64 {
        let product = match (self.size(first), self.size(second)) {
            (Some(first), Some(second)) => first * second,
            (Some(size), None) | (None, Some(size)) => size * size,
            (None, None) => 1.0,
        };
        product.sqrt().max(1.0)
    }

    /// The side choice `sigma` of a point relative to a line: +1 on the left or on the line, else -1.
    fn side(&self, point: PointTerm, line: LineTerm) -> f64 {
        let distance = signed_distance(
            point.value(self.x),
            line.start.value(self.x),
            line.end.value(self.x),
        );
        if distance >= 0.0 { 1.0 } else { -1.0 }
    }

    /// The rows of one constraint and the parameters it freezes.
    fn constraint(
        &self,
        index: usize,
        constraint: &SketchConstraint,
        values: &DimensionValues,
    ) -> (Vec<Row>, Vec<(usize, f64)>) {
        let owner = RowOwner::Constraint(index);
        let value = || {
            *values
                .get(&constraint.id)
                .expect("values of driving dimensions are checked before rows are built")
        };
        let single =
            |terms: Vec<(f64, Term)>, constant: f64| vec![Row::new(owner, terms, constant)];
        let rows = match &constraint.kind {
            ConstraintKind::Coincident { points } => {
                let (first, second) = (self.point(&points[0]), self.point(&points[1]));
                equality_rows(owner, first, second, &[0, 1])
            }
            ConstraintKind::PointOnObject { point, curve } => {
                let point = self.point(point);
                if self.is_straight(curve) {
                    single(
                        vec![(
                            1.0,
                            Term::SignedDistance {
                                point: Anchor::Point(point),
                                line: self.line(curve),
                            },
                        )],
                        0.0,
                    )
                } else {
                    let curve = self.curve(curve);
                    single(
                        vec![
                            (
                                1.0,
                                Term::Length {
                                    from: curve.center,
                                    to: point,
                                },
                            ),
                            (-1.0, curve.radius),
                        ],
                        0.0,
                    )
                }
            }
            ConstraintKind::Horizontal(alignment) => {
                let (first, second) = self.alignment(alignment);
                equality_rows(owner, second, first, &[1])
            }
            ConstraintKind::Vertical(alignment) => {
                let (first, second) = self.alignment(alignment);
                equality_rows(owner, second, first, &[0])
            }
            ConstraintKind::Parallel { lines } => single(self.angle_terms(lines, 1.0, 0.0), 0.0),
            ConstraintKind::Perpendicular { lines } => {
                single(self.angle_terms(lines, 0.0, -1.0), 0.0)
            }
            ConstraintKind::Angle { lines, dimension } if dimension.driving => {
                let angle = value();
                single(self.angle_terms(lines, angle.cos(), angle.sin()), 0.0)
            }
            ConstraintKind::Tangent { curves } => single(self.tangent_terms(curves), 0.0),
            ConstraintKind::Collinear { lines } => {
                let first = self.line(&lines[0]);
                let second = self.line(&lines[1]);
                [second.start, second.end]
                    .into_iter()
                    .map(|point| {
                        Row::new(
                            owner,
                            vec![(
                                1.0,
                                Term::SignedDistance {
                                    point: Anchor::Point(point),
                                    line: first,
                                },
                            )],
                            0.0,
                        )
                    })
                    .collect()
            }
            ConstraintKind::Concentric { curves } => {
                let (first, second) = (self.curve(&curves[0]), self.curve(&curves[1]));
                equality_rows(owner, first.center, second.center, &[0, 1])
            }
            ConstraintKind::Equal { curves } => {
                let terms = if self.is_straight(&curves[0]) {
                    let length = |line: LineTerm| Term::Length {
                        from: line.start,
                        to: line.end,
                    };
                    vec![
                        (1.0, length(self.line(&curves[0]))),
                        (-1.0, length(self.line(&curves[1]))),
                    ]
                } else {
                    vec![
                        (1.0, self.curve(&curves[0]).radius),
                        (-1.0, self.curve(&curves[1]).radius),
                    ]
                };
                single(terms, 0.0)
            }
            ConstraintKind::Symmetric { points, line } => {
                let (first, second) = (self.point(&points[0]), self.point(&points[1]));
                let line = self.line(line);
                vec![
                    Row::new(
                        owner,
                        vec![(
                            1.0,
                            Term::SignedDistance {
                                point: Anchor::Midpoint(first, second),
                                line,
                            },
                        )],
                        0.0,
                    ),
                    Row::new(
                        owner,
                        vec![(
                            1.0,
                            Term::Projection {
                                from: first,
                                to: second,
                                line,
                            },
                        )],
                        0.0,
                    ),
                ]
            }
            ConstraintKind::Midpoint { point, line } => {
                let point = self.point(point);
                let line = self.line(line);
                [0, 1]
                    .into_iter()
                    .map(|axis| {
                        linear_row(
                            owner,
                            &[(1.0, point), (-0.5, line.start), (-0.5, line.end)],
                            axis,
                            0.0,
                        )
                    })
                    .collect()
            }
            ConstraintKind::Fix { target } => {
                let freeze = self.fixed_parameters(target);
                return (self.fix_rows(owner, target, &freeze), freeze);
            }
            ConstraintKind::Distance { target, dimension } if dimension.driving => {
                let value = value();
                match target {
                    DistanceTarget::Points { points } => single(
                        vec![(
                            1.0,
                            Term::Length {
                                from: self.point(&points[0]),
                                to: self.point(&points[1]),
                            },
                        )],
                        -value,
                    ),
                    DistanceTarget::Line { line } => {
                        let line = self.line(line);
                        single(
                            vec![(
                                1.0,
                                Term::Length {
                                    from: line.start,
                                    to: line.end,
                                },
                            )],
                            -value,
                        )
                    }
                    DistanceTarget::PointLine { point, line } => {
                        let point = self.point(point);
                        let line = self.line(line);
                        single(
                            vec![(
                                self.side(point, line),
                                Term::SignedDistance {
                                    point: Anchor::Point(point),
                                    line,
                                },
                            )],
                            -value,
                        )
                    }
                }
            }
            ConstraintKind::HorizontalDistance { target, dimension } if dimension.driving => {
                let (first, second) = self.alignment(target);
                vec![linear_row(
                    owner,
                    &[(1.0, second), (-1.0, first)],
                    0,
                    -value(),
                )]
            }
            ConstraintKind::VerticalDistance { target, dimension } if dimension.driving => {
                let (first, second) = self.alignment(target);
                vec![linear_row(
                    owner,
                    &[(1.0, second), (-1.0, first)],
                    1,
                    -value(),
                )]
            }
            ConstraintKind::Radius { curve, dimension } if dimension.driving => {
                single(vec![(1.0, self.curve(curve).radius)], -value())
            }
            ConstraintKind::Diameter { curve, dimension } if dimension.driving => {
                single(vec![(1.0, self.curve(curve).radius)], -value() / 2.0)
            }
            // Reference dimensions add no rows.
            ConstraintKind::Distance { .. }
            | ConstraintKind::HorizontalDistance { .. }
            | ConstraintKind::VerticalDistance { .. }
            | ConstraintKind::Radius { .. }
            | ConstraintKind::Diameter { .. }
            | ConstraintKind::Angle { .. } => Vec::new(),
        };
        (rows, Vec::new())
    }

    /// The first and second point of an alignment: a line's start and end, or the two points.
    fn alignment(&self, alignment: &Alignment) -> (PointTerm, PointTerm) {
        match alignment {
            Alignment::Line { line } => {
                let line = self.line(line);
                (line.start, line.end)
            }
            Alignment::Points { points } => (self.point(&points[0]), self.point(&points[1])),
        }
    }

    /// `S * (cross(u1, u2) * cos - dot(u1, u2) * sin)` for two lines.
    fn angle_terms(&self, lines: &[Id; 2], cos: f64, sin: f64) -> Vec<(f64, Term)> {
        vec![(
            self.angular_scale(&lines[0], &lines[1]),
            Term::Angle {
                first: self.line(&lines[0]),
                second: self.line(&lines[1]),
                cos,
                sin,
            },
        )]
    }

    /// The tangent row of section 3.6.5 in the form its touch point and warm-start choices select.
    fn tangent_terms(&self, curves: &[Id; 2]) -> Vec<(f64, Term)> {
        let touch = self
            .touch_point(&curves[0], &curves[1])
            .map(|point| self.point(&point));
        let straight = curves.iter().find(|curve| self.is_straight(curve));
        if let Some(line_id) = straight {
            let curve_id = if line_id == &curves[0] {
                &curves[1]
            } else {
                &curves[0]
            };
            let line = self.line(line_id);
            let curve = self.curve(curve_id);
            return match touch {
                Some(touch) => vec![(
                    self.angular_scale(&curves[0], &curves[1]),
                    Term::Angle {
                        first: line,
                        second: LineTerm {
                            start: curve.center,
                            end: touch,
                        },
                        cos: 0.0,
                        sin: -1.0,
                    },
                )],
                None => vec![
                    (
                        self.side(curve.center, line),
                        Term::SignedDistance {
                            point: Anchor::Point(curve.center),
                            line,
                        },
                    ),
                    (-1.0, curve.radius),
                ],
            };
        }
        let first = self.curve(&curves[0]);
        let second = self.curve(&curves[1]);
        if let Some(touch) = touch {
            return vec![(
                self.angular_scale(&curves[0], &curves[1]),
                Term::Angle {
                    first: LineTerm {
                        start: first.center,
                        end: touch,
                    },
                    second: LineTerm {
                        start: second.center,
                        end: touch,
                    },
                    cos: 1.0,
                    sin: 0.0,
                },
            )];
        }
        let distance = norm(subtract(
            second.center.value(self.x),
            first.center.value(self.x),
        ));
        let first_radius = first.radius.value(self.x);
        let second_radius = second.radius.value(self.x);
        let centers = Term::Length {
            from: first.center,
            to: second.center,
        };
        let external = (distance - (first_radius + second_radius)).abs()
            <= (distance - (first_radius - second_radius).abs()).abs();
        if external {
            vec![(1.0, centers), (-1.0, first.radius), (-1.0, second.radius)]
        } else {
            let tau = if first_radius >= second_radius {
                1.0
            } else {
                -1.0
            };
            vec![(1.0, centers), (-tau, first.radius), (tau, second.radius)]
        }
    }

    /// The touch point of `tangent [first, second]` (section 3.6.4).
    fn touch_point(&self, first: &Id, second: &Id) -> Option<PointRef> {
        let mut candidates = Vec::new();
        for curve in [first, second] {
            if self.has_ends(curve) {
                for role in [PointRole::Start, PointRole::End] {
                    candidates.push(PointRef {
                        entity: curve.clone(),
                        role,
                    });
                }
            }
        }
        let mut on_either: Vec<(usize, &PointRef)> = [first, second]
            .into_iter()
            .filter_map(|curve| self.points_on.get(curve))
            .flatten()
            .copied()
            .collect();
        on_either.sort_by_key(|(constraint, _)| *constraint);
        candidates.extend(on_either.into_iter().map(|(_, point)| point.clone()));
        candidates
            .into_iter()
            .find(|candidate| self.on_curve(candidate, first) && self.on_curve(candidate, second))
    }

    /// True for a line or arc entity, which has end points (axes have none).
    fn has_ends(&self, curve: &Id) -> bool {
        !is_reserved(curve.as_str())
            && matches!(
                self.entity(curve).1,
                Geometry::Line { .. } | Geometry::Arc { .. }
            )
    }

    /// True when `point` is identified with an end of `curve` or with the point of a `point_on_object` on `curve`.
    fn on_curve(&self, point: &PointRef, curve: &Id) -> bool {
        let on_end = self.has_ends(curve)
            && [PointRole::Start, PointRole::End].into_iter().any(|role| {
                self.identification.same(
                    point,
                    &PointRef {
                        entity: curve.clone(),
                        role,
                    },
                )
            });
        on_end
            || self.points_on.get(curve).is_some_and(|points| {
                points
                    .iter()
                    .any(|(_, other)| self.identification.same(point, other))
            })
    }

    /// The parameters a `fix` freezes with their warm-start values: point 2, line 4, circle 3, arc all 6.
    fn fixed_parameters(&self, target: &PointRef) -> Vec<(usize, f64)> {
        if target.entity.as_str() == ORIGIN {
            return Vec::new();
        }
        let (index, geometry) = self.entity(&target.entity);
        let parameters = match self.layout.point_parameter(index, geometry, target.role) {
            Some(first) => first..first + 2,
            None => self.layout.parameters(index),
        };
        parameters
            .map(|parameter| (parameter, self.x[parameter]))
            .collect()
    }

    /// The rows of a `fix` on `target` with the fix values in `freeze`.
    fn fix_rows(&self, owner: RowOwner, target: &PointRef, freeze: &[(usize, f64)]) -> Vec<Row> {
        if target.entity.as_str() == ORIGIN {
            return [0, 1]
                .map(|axis| linear_row(owner, &[(1.0, PointTerm::Constant([0.0, 0.0]))], axis, 0.0))
                .to_vec();
        }
        let difference = |&(parameter, value): &(usize, f64)| {
            Row::new(owner, vec![(1.0, Term::Coordinate(parameter))], -value)
        };
        let (_, geometry) = self.entity(&target.entity);
        let whole_arc = target.role == PointRole::Point && matches!(geometry, Geometry::Arc { .. });
        if !whole_arc {
            return freeze.iter().map(difference).collect();
        }
        // An arc keeps its center and start point, and its end point stays on the ray from the center through the fixed end.
        let mut rows: Vec<Row> = freeze[..4].iter().map(difference).collect();
        let [center_x, center_y, _, _, end_x, end_y] = [0, 1, 2, 3, 4, 5].map(|i| freeze[i]);
        let ray = unit(subtract([end_x.1, end_y.1], [center_x.1, center_y.1]));
        rows.push(Row::new(
            owner,
            vec![
                (ray[0], Term::Coordinate(end_y.0)),
                (-ray[0], Term::Coordinate(center_y.0)),
                (-ray[1], Term::Coordinate(end_x.0)),
                (ray[1], Term::Coordinate(center_x.0)),
            ],
            0.0,
        ));
        rows
    }
}

/// `weights . points[axis] + constant` as a row over coordinates.
fn linear_row(owner: RowOwner, points: &[(f64, PointTerm)], axis: usize, constant: f64) -> Row {
    let mut terms = Vec::with_capacity(points.len());
    let mut total = constant;
    for &(weight, point) in points {
        match point {
            PointTerm::Parameters(index) => terms.push((weight, Term::Coordinate(index + axis))),
            PointTerm::Constant(value) => total += weight * value[axis],
        }
    }
    Row::new(owner, terms, total)
}

/// Rows `first[axis] - second[axis]` for each axis, with the equality each states.
fn equality_rows(owner: RowOwner, first: PointTerm, second: PointTerm, axes: &[usize]) -> Vec<Row> {
    axes.iter()
        .map(|&axis| {
            let mut row = linear_row(owner, &[(1.0, first), (-1.0, second)], axis, 0.0);
            row.equality = match (first, second) {
                (PointTerm::Parameters(first), PointTerm::Parameters(second)) => {
                    Some(Equality::Pair(first + axis, second + axis))
                }
                (PointTerm::Parameters(parameter), PointTerm::Constant(_))
                | (PointTerm::Constant(_), PointTerm::Parameters(parameter)) => {
                    Some(Equality::Zero(parameter + axis))
                }
                (PointTerm::Constant(_), PointTerm::Constant(_)) => None,
            };
            row
        })
        .collect()
}

/// For each curve, the `point_on_object` constraints on it, in document order.
fn points_on_curves(sketch: &SketchParams) -> BTreeMap<&Id, Vec<(usize, &PointRef)>> {
    let mut points_on: BTreeMap<&Id, Vec<(usize, &PointRef)>> = BTreeMap::new();
    for (index, constraint) in sketch.constraints.iter().enumerate() {
        if let ConstraintKind::PointOnObject { point, curve } = &constraint.kind {
            points_on.entry(curve).or_default().push((index, point));
        }
    }
    points_on
}

/// Point references identified through `coincident` constraints (section 3.6.4).
struct Identification {
    /// Class index of every point reference that appears in a `coincident`.
    references: BTreeMap<PointRef, usize>,
    /// The classes merged by the `coincident` constraints.
    classes: UnionFind,
}

impl Identification {
    fn new(sketch: &SketchParams) -> Identification {
        let mut identification = Identification {
            references: BTreeMap::new(),
            classes: UnionFind::default(),
        };
        for constraint in &sketch.constraints {
            if let ConstraintKind::Coincident { points } = &constraint.kind {
                let first = identification.class(&points[0]);
                let second = identification.class(&points[1]);
                identification.classes.union(first, second);
            }
        }
        identification
    }

    fn class(&mut self, point: &PointRef) -> usize {
        if let Some(&class) = self.references.get(point) {
            return class;
        }
        let class = self.classes.push();
        self.references.insert(point.clone(), class);
        class
    }

    fn same(&self, first: &PointRef, second: &PointRef) -> bool {
        if first == second {
            return true;
        }
        match (self.references.get(first), self.references.get(second)) {
            (Some(&first), Some(&second)) => self.classes.root(first) == self.classes.root(second),
            _ => false,
        }
    }
}

/// Splits the entities into connected components and assigns rows and constraints to them (section 3.7.1).
fn components(sketch: &SketchParams, layout: &Layout, rows: &[Row]) -> Vec<Component> {
    let mut linked = UnionFind::new(sketch.entities.len());
    let constraint_entities: Vec<Vec<usize>> = sketch
        .constraints
        .iter()
        .map(|constraint| {
            constraint
                .referenced_entities()
                .into_iter()
                .map(|entity| {
                    layout
                        .entity_index(entity)
                        .expect("validation guarantees that constraints name existing entities")
                })
                .collect()
        })
        .collect();
    for entities in &constraint_entities {
        for pair in entities.windows(2) {
            linked.union(pair[0], pair[1]);
        }
    }
    // Roots are the lowest entity index of each component, so components come out in document order.
    let mut component_of_root = BTreeMap::new();
    let mut components: Vec<Component> = Vec::new();
    let mut component_of_entity = Vec::with_capacity(sketch.entities.len());
    for entity in 0..sketch.entities.len() {
        let entity_root = linked.root(entity);
        let component = *component_of_root.entry(entity_root).or_insert_with(|| {
            components.push(Component {
                entities: Vec::new(),
                parameters: Vec::new(),
                rows: Vec::new(),
                constraints: Vec::new(),
            });
            components.len() - 1
        });
        components[component].entities.push(entity);
        components[component]
            .parameters
            .extend(layout.parameters(entity));
        component_of_entity.push(component);
    }
    for (constraint, entities) in constraint_entities.iter().enumerate() {
        if let Some(&first) = entities.first() {
            components[component_of_entity[first]]
                .constraints
                .push(constraint);
        }
    }
    for (index, row) in rows.iter().enumerate() {
        let entity = match row.owner {
            RowOwner::ArcRule(entity) => Some(entity),
            RowOwner::Constraint(constraint) => constraint_entities[constraint].first().copied(),
        };
        if let Some(entity) = entity {
            components[component_of_entity[entity]].rows.push(index);
        }
    }
    components
}
