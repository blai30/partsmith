//! The planar arrangement of profile curves, its faces and its findings (spec sections 3.13.1 to 3.13.7).

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};

use partsmith_model::Id;

use super::area::{moments, winding_number};
use super::key::{KeyEdge, RegionKey, loop_text};
use super::{Profile, ProfileCurve, ProfileEdge, ProfileFinding, ProfileLoop, Region, signed_area};
use crate::PROFILE_TOLERANCE;
use crate::entity::{Geometry, SketchEntity, point_at_angle};
use crate::union_find::UnionFind;
use crate::vector::{add, cross, dot, norm, scale, signed_distance, subtract, unit};

/// Outgoing tangents whose angles differ by at most this much (rad) are ordered by curvature.
const TANGENT_TIE: f64 = 1e-9;

/// Regions with at most this area (mm^2) are dropped.
const MIN_REGION_AREA: f64 = 1e-12;

/// A non-construction line, circle or arc of positive size.
struct Curve<'a> {
    entity: &'a Id,
    shape: Shape,
    /// Smallest and largest coordinates, widened by the tolerance: `[min u, min v, max u, max v]`.
    bounds: [f64; 4],
}

#[derive(Clone, Copy, Debug)]
enum Shape {
    Line {
        start: [f64; 2],
        end: [f64; 2],
    },
    Circle {
        center: [f64; 2],
        radius: f64,
    },
    Arc {
        center: [f64; 2],
        radius: f64,
        start_angle: f64,
        sweep: f64,
        start: [f64; 2],
        end: [f64; 2],
    },
}

impl Shape {
    /// The shape of a profile entity, or `None` for points and zero-size curves.
    fn of(geometry: &Geometry) -> Option<Shape> {
        let shape = match *geometry {
            Geometry::Point { .. } => return None,
            Geometry::Line { start, end } => Shape::Line { start, end },
            Geometry::Circle { center, radius } => Shape::Circle { center, radius },
            Geometry::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => Shape::Arc {
                center,
                radius,
                start_angle,
                sweep: geometry.arc_sweep()?,
                start: point_at_angle(center, radius, start_angle),
                end: point_at_angle(center, radius, end_angle),
            },
        };
        Some(shape)
    }

    /// True when the curve is too small to use (section 3.13.1).
    fn is_zero_size(&self) -> bool {
        match *self {
            Shape::Line { start, end } => norm(subtract(end, start)) <= PROFILE_TOLERANCE,
            Shape::Circle { radius, .. } => radius <= PROFILE_TOLERANCE,
            Shape::Arc { radius, sweep, .. } => {
                radius <= PROFILE_TOLERANCE || radius * sweep <= PROFILE_TOLERANCE
            }
        }
    }

    fn endpoints(&self) -> Option<[[f64; 2]; 2]> {
        match *self {
            Shape::Line { start, end } | Shape::Arc { start, end, .. } => Some([start, end]),
            Shape::Circle { .. } => None,
        }
    }

    /// The circle of a circle or arc.
    fn circle(&self) -> Option<([f64; 2], f64)> {
        match *self {
            Shape::Circle { center, radius } | Shape::Arc { center, radius, .. } => {
                Some((center, radius))
            }
            Shape::Line { .. } => None,
        }
    }

    /// The parameter of the curve's end: a line's length or an arc's sweep.
    fn end_parameter(&self) -> f64 {
        match *self {
            Shape::Line { start, end } => norm(subtract(end, start)),
            Shape::Arc { sweep, .. } => sweep,
            Shape::Circle { .. } => TAU,
        }
    }

    /// The position of `point` along the curve when it lies on it within the tolerance: the distance
    /// from a line's start, the counterclockwise angle from an arc's start, or a circle's angle from +u.
    fn parameter(&self, point: [f64; 2]) -> Option<f64> {
        match *self {
            Shape::Line { start, end } => {
                let length = norm(subtract(end, start));
                let direction = unit(subtract(end, start));
                let relative = subtract(point, start);
                let along = dot(relative, direction);
                let within = cross(direction, relative).abs() <= PROFILE_TOLERANCE
                    && along >= -PROFILE_TOLERANCE
                    && along <= length + PROFILE_TOLERANCE;
                within.then_some(along)
            }
            Shape::Circle { center, radius } => {
                let offset = subtract(point, center);
                ((norm(offset) - radius).abs() <= PROFILE_TOLERANCE)
                    .then(|| angle_of(offset).rem_euclid(TAU))
            }
            Shape::Arc {
                center,
                radius,
                start_angle,
                sweep,
                ..
            } => {
                let offset = subtract(point, center);
                if (norm(offset) - radius).abs() > PROFILE_TOLERANCE {
                    return None;
                }
                let extension = PROFILE_TOLERANCE / radius;
                let along = (angle_of(offset) - start_angle).rem_euclid(TAU);
                if along <= sweep + extension {
                    Some(along)
                } else if along >= TAU - extension {
                    Some(along - TAU)
                } else {
                    None
                }
            }
        }
    }

    /// The point at a parameter of the curve.
    fn point_at(&self, parameter: f64) -> [f64; 2] {
        match *self {
            Shape::Line { start, end } => add(start, scale(unit(subtract(end, start)), parameter)),
            Shape::Circle { center, radius } => point_at_angle(center, radius, parameter),
            Shape::Arc {
                center,
                radius,
                start_angle,
                ..
            } => point_at_angle(center, radius, start_angle + parameter),
        }
    }

    fn bounds(&self) -> [f64; 4] {
        let [low_u, low_v, high_u, high_v] = match *self {
            Shape::Line { start, end } => [
                start[0].min(end[0]),
                start[1].min(end[1]),
                start[0].max(end[0]),
                start[1].max(end[1]),
            ],
            Shape::Circle { center, radius } | Shape::Arc { center, radius, .. } => [
                center[0] - radius,
                center[1] - radius,
                center[0] + radius,
                center[1] + radius,
            ],
        };
        [
            low_u - PROFILE_TOLERANCE,
            low_v - PROFILE_TOLERANCE,
            high_u + PROFILE_TOLERANCE,
            high_v + PROFILE_TOLERANCE,
        ]
    }
}

fn angle_of(offset: [f64; 2]) -> f64 {
    offset[1].atan2(offset[0])
}

fn boxes_overlap(first: &[f64; 4], second: &[f64; 4]) -> bool {
    first[0] <= second[2] && second[0] <= first[2] && first[1] <= second[3] && second[1] <= first[3]
}

fn box_contains(bounds: &[f64; 4], point: [f64; 2]) -> bool {
    bounds[0] <= point[0] && point[0] <= bounds[2] && bounds[1] <= point[1] && point[1] <= bounds[3]
}

/// Builds the profile of these entities (section 3.13).
pub(super) fn build(entities: &[SketchEntity]) -> Profile {
    let mut findings = Vec::new();
    let curves = input_curves(entities, &mut findings);

    // Step 1: intersections and shared stretches of every pair, in entity order.
    let mut crossings: Vec<(usize, usize, Vec<[f64; 2]>)> = Vec::new();
    let mut stretches: Vec<(usize, usize, [[f64; 2]; 2])> = Vec::new();
    for (first_index, first) in curves.iter().enumerate() {
        for (second_index, second) in curves.iter().enumerate().skip(first_index + 1) {
            if !boxes_overlap(&first.bounds, &second.bounds) {
                continue;
            }
            let (points, shared) = meet(first, second);
            if !points.is_empty() {
                crossings.push((first_index, second_index, points));
            }
            stretches.extend(
                shared
                    .into_iter()
                    .map(|stretch| (first_index, second_index, stretch)),
            );
        }
    }

    // Step 2: vertices, from curve endpoints, then intersection points, then stretch ends.
    let mut vertices = Vertices::default();
    let endpoint_vertices: Vec<Option<[usize; 2]>> = curves
        .iter()
        .map(|curve| {
            curve
                .shape
                .endpoints()
                .map(|ends| ends.map(|point| vertices.snap(point)))
        })
        .collect();
    let crossing_vertices: Vec<(usize, usize, usize)> = crossings
        .iter()
        .flat_map(|(first, second, points)| {
            points.iter().map(move |point| (*first, *second, *point))
        })
        .map(|(first, second, point)| (first, second, vertices.snap(point)))
        .collect();
    for (_, _, stretch) in &stretches {
        for point in stretch {
            vertices.snap(*point);
        }
    }
    let vertices = vertices.points;

    for (first, second, [start, end]) in &stretches {
        let mut entities = [
            curves[*first].entity.clone(),
            curves[*second].entity.clone(),
        ];
        entities.sort();
        findings.push(ProfileFinding::Overlap {
            entities,
            start: *start,
            end: *end,
        });
    }
    findings.extend(self_intersections(
        &curves,
        &endpoint_vertices,
        &crossing_vertices,
        &vertices,
    ));

    // Steps 3 to 6: edges, overlap removal, open endpoints and bridges.
    let mut graph = Graph::split(&curves, &endpoint_vertices, &vertices);
    graph.remove_overlaps(&curves, &vertices);
    findings.extend(graph.open_endpoints(&curves, &vertices));
    graph.remove_bridges(vertices.len());

    // Steps 7 to 9: faces, holes and regions.
    let loops = graph.loops(&curves, &vertices);
    let regions = regions(loops);

    findings.sort_by(compare_findings);
    findings.dedup();
    Profile { regions, findings }
}

/// The curves that take part, in entity order; zero-size curves become findings.
fn input_curves<'a>(
    entities: &'a [SketchEntity],
    findings: &mut Vec<ProfileFinding>,
) -> Vec<Curve<'a>> {
    let mut curves = Vec::new();
    for entity in entities.iter().filter(|entity| !entity.construction) {
        let Some(shape) = Shape::of(&entity.geometry) else {
            continue;
        };
        if shape.is_zero_size() {
            findings.push(ProfileFinding::ZeroLength {
                entity: entity.id.clone(),
            });
            continue;
        }
        curves.push(Curve {
            entity: &entity.id,
            shape,
            bounds: shape.bounds(),
        });
    }
    curves
}

/// The intersection points of two curves, sorted along `first`, and their shared stretches,
/// each oriented along the curve with the smaller id (section 3.13.2, step 1).
fn meet(first: &Curve<'_>, second: &Curve<'_>) -> (Vec<[f64; 2]>, Vec<[[f64; 2]; 2]>) {
    let (by_id_first, by_id_second) = if first.entity <= second.entity {
        (first, second)
    } else {
        (second, first)
    };
    let candidates = match (first.shape, second.shape) {
        (
            Shape::Line {
                start: first_start,
                end: first_end,
            },
            Shape::Line {
                start: second_start,
                end: second_end,
            },
        ) => {
            let collinear = [
                signed_distance(second_start, first_start, first_end),
                signed_distance(second_end, first_start, first_end),
                signed_distance(first_start, second_start, second_end),
                signed_distance(first_end, second_start, second_end),
            ]
            .iter()
            .all(|distance| distance.abs() <= PROFILE_TOLERANCE);
            if collinear {
                let shared = line_overlap(&by_id_first.shape, &by_id_second.shape);
                return (Vec::new(), shared.into_iter().collect());
            }
            line_line(first_start, first_end, second_start, second_end)
        }
        (Shape::Line { start, end }, other) | (other, Shape::Line { start, end }) => {
            let (center, radius) = other.circle().expect("a non-line shape is circular");
            line_circle(start, end, center, radius)
        }
        (first_shape, second_shape) => {
            let (first_center, first_radius) = first_shape.circle().expect("circular shape");
            let (second_center, second_radius) = second_shape.circle().expect("circular shape");
            let same_circle = norm(subtract(second_center, first_center)) <= PROFILE_TOLERANCE
                && (first_radius - second_radius).abs() <= PROFILE_TOLERANCE;
            if same_circle {
                return (
                    Vec::new(),
                    circle_overlaps(&by_id_first.shape, &by_id_second.shape),
                );
            }
            circle_circle(first_center, first_radius, second_center, second_radius)
        }
    };
    let mut points: Vec<(f64, [f64; 2])> = candidates
        .into_iter()
        .filter(|point| second.shape.parameter(*point).is_some())
        .filter_map(|point| {
            first
                .shape
                .parameter(point)
                .map(|parameter| (parameter, point))
        })
        .collect();
    points.sort_by(|first, second| first.0.total_cmp(&second.0));
    (
        points.into_iter().map(|(_, point)| point).collect(),
        Vec::new(),
    )
}

/// The intersection of two non-collinear lines' infinite extensions, if they are not parallel.
fn line_line(
    first_start: [f64; 2],
    first_end: [f64; 2],
    second_start: [f64; 2],
    second_end: [f64; 2],
) -> Vec<[f64; 2]> {
    let first_direction = subtract(first_end, first_start);
    let second_direction = subtract(second_end, second_start);
    let denominator = cross(first_direction, second_direction);
    if denominator == 0.0 {
        return Vec::new();
    }
    let along = cross(subtract(second_start, first_start), second_direction) / denominator;
    vec![add(first_start, scale(first_direction, along))]
}

/// The stretch two collinear lines share, along `first`, when it is longer than the tolerance.
fn line_overlap(first: &Shape, second: &Shape) -> Option<[[f64; 2]; 2]> {
    let (
        Shape::Line { start, end },
        Shape::Line {
            start: other_start,
            end: other_end,
        },
    ) = (*first, *second)
    else {
        return None;
    };
    let length = norm(subtract(end, start));
    let direction = unit(subtract(end, start));
    let mut projected =
        [other_start, other_end].map(|point| (dot(subtract(point, start), direction), point));
    projected.sort_by(|first, second| first.0.total_cmp(&second.0));
    let [low, high] = projected;
    let from = if low.0 <= 0.0 { (0.0, start) } else { low };
    let to = if high.0 >= length {
        (length, end)
    } else {
        high
    };
    (to.0 - from.0 > PROFILE_TOLERANCE).then_some([from.1, to.1])
}

/// Where a line's infinite extension meets a circle: none, the touch point, or two points along the line.
fn line_circle(start: [f64; 2], end: [f64; 2], center: [f64; 2], radius: f64) -> Vec<[f64; 2]> {
    let direction = unit(subtract(end, start));
    let relative = subtract(center, start);
    let foot = add(start, scale(direction, dot(relative, direction)));
    let distance = cross(direction, relative).abs();
    if distance > radius + PROFILE_TOLERANCE {
        Vec::new()
    } else if (distance - radius).abs() <= PROFILE_TOLERANCE {
        vec![foot]
    } else {
        let half = (radius * radius - distance * distance).sqrt();
        vec![
            subtract(foot, scale(direction, half)),
            add(foot, scale(direction, half)),
        ]
    }
}

/// Where two circles that are not the same circle meet: none, a touch point, or two points.
fn circle_circle(
    first_center: [f64; 2],
    first_radius: f64,
    second_center: [f64; 2],
    second_radius: f64,
) -> Vec<[f64; 2]> {
    let distance = norm(subtract(second_center, first_center));
    if distance <= PROFILE_TOLERANCE
        || distance > first_radius + second_radius + PROFILE_TOLERANCE
        || distance < (first_radius - second_radius).abs() - PROFILE_TOLERANCE
    {
        return Vec::new();
    }
    let direction = scale(subtract(second_center, first_center), 1.0 / distance);
    if (distance - (first_radius + second_radius)).abs() <= PROFILE_TOLERANCE {
        return vec![add(first_center, scale(direction, first_radius))];
    }
    if (distance - (first_radius - second_radius).abs()).abs() <= PROFILE_TOLERANCE {
        let toward = if first_radius >= second_radius {
            first_radius
        } else {
            -first_radius
        };
        return vec![add(first_center, scale(direction, toward))];
    }
    let along = (distance * distance + first_radius * first_radius - second_radius * second_radius)
        / (2.0 * distance);
    let height = (first_radius * first_radius - along * along)
        .max(0.0)
        .sqrt();
    let base = add(first_center, scale(direction, along));
    let normal = [-direction[1], direction[0]];
    vec![
        add(base, scale(normal, height)),
        subtract(base, scale(normal, height)),
    ]
}

/// The stretches two curves on one circle share, counterclockwise along `first` (the one with the smaller id).
fn circle_overlaps(first: &Shape, second: &Shape) -> Vec<[[f64; 2]; 2]> {
    match (*first, *second) {
        (Shape::Circle { center, radius }, Shape::Circle { .. }) => {
            let point = point_at_angle(center, radius, 0.0);
            vec![[point, point]]
        }
        (Shape::Circle { .. }, Shape::Arc { start, end, .. })
        | (Shape::Arc { start, end, .. }, Shape::Circle { .. }) => vec![[start, end]],
        (
            Shape::Arc {
                radius,
                start_angle,
                sweep,
                ..
            },
            Shape::Arc {
                start_angle: other_start,
                sweep: other_sweep,
                ..
            },
        ) => {
            let offset = (other_start - start_angle).rem_euclid(TAU);
            let mut pieces = Vec::new();
            if offset + other_sweep > TAU {
                pieces.push((0.0, (offset + other_sweep - TAU).min(sweep)));
            }
            if offset < sweep {
                pieces.push((offset, (offset + other_sweep).min(sweep)));
            }
            pieces
                .into_iter()
                .filter(|(low, high)| (high - low) * radius > PROFILE_TOLERANCE)
                .map(|(low, high)| [first.point_at(low), first.point_at(high)])
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Vertices in creation order; coordinates never change after creation.
#[derive(Default)]
struct Vertices {
    points: Vec<[f64; 2]>,
}

impl Vertices {
    /// The first vertex within the tolerance of `point`, or a new vertex at `point`.
    fn snap(&mut self, point: [f64; 2]) -> usize {
        if let Some(index) = self
            .points
            .iter()
            .position(|vertex| norm(subtract(*vertex, point)) <= PROFILE_TOLERANCE)
        {
            return index;
        }
        self.points.push(point);
        self.points.len() - 1
    }
}

/// Self-intersections: crossings of two curves of one chain away from both curves' ends (section 3.13.3).
fn self_intersections(
    curves: &[Curve<'_>],
    endpoint_vertices: &[Option<[usize; 2]>],
    crossing_vertices: &[(usize, usize, usize)],
    vertices: &[[f64; 2]],
) -> Vec<ProfileFinding> {
    // Chains: curves joined at a vertex that is an end of both.
    let mut chains = UnionFind::new(curves.len());
    let mut first_curve_at: BTreeMap<usize, usize> = BTreeMap::new();
    for (curve, ends) in endpoint_vertices.iter().enumerate() {
        for vertex in ends.iter().flatten() {
            match first_curve_at.get(vertex) {
                Some(&other) => {
                    chains.union(other, curve);
                }
                None => {
                    first_curve_at.insert(*vertex, curve);
                }
            }
        }
    }
    let is_end = |curve: usize, vertex: usize| {
        endpoint_vertices[curve].is_some_and(|ends| ends.contains(&vertex))
    };
    crossing_vertices
        .iter()
        .filter(|&&(first, second, vertex)| {
            !is_end(first, vertex)
                && !is_end(second, vertex)
                && chains.root(first) == chains.root(second)
        })
        .map(|&(first, second, vertex)| {
            let mut entities = [curves[first].entity.clone(), curves[second].entity.clone()];
            entities.sort();
            ProfileFinding::SelfIntersection {
                entities,
                point: vertices[vertex],
            }
        })
        .collect()
}

/// A piece of a curve between two vertices, in its entity's direction. A closed piece starts and ends
/// at one vertex: a circle with one vertex on it, or an arc whose ends met.
#[derive(Clone, Copy, Debug)]
struct Piece {
    curve: usize,
    from: usize,
    to: usize,
    /// Counterclockwise sweep from `from` to `to` for a piece of a circle or arc; `None` for a line piece.
    sweep: Option<f64>,
}

impl Piece {
    /// True when the piece starts and ends at one vertex.
    fn is_closed(&self) -> bool {
        self.from == self.to
    }
}

/// A whole circle with no vertex on it, which forms loops on its own.
#[derive(Clone, Copy, Debug)]
struct Ring {
    curve: usize,
}

/// An edge that closes on itself: a closed piece or a ring, by index.
#[derive(Clone, Copy, Debug)]
enum ClosedEdge {
    Piece(usize),
    Ring(usize),
}

/// The edges of the arrangement.
struct Graph {
    pieces: Vec<Piece>,
    rings: Vec<Ring>,
}

impl Graph {
    /// Splits every curve at the vertices on it (section 3.13.2, step 3).
    fn split(
        curves: &[Curve<'_>],
        endpoint_vertices: &[Option<[usize; 2]>],
        vertices: &[[f64; 2]],
    ) -> Graph {
        let mut pieces = Vec::new();
        let mut rings = Vec::new();
        for (index, curve) in curves.iter().enumerate() {
            let ends = endpoint_vertices[index];
            let mut on: Vec<(f64, usize)> = vertices
                .iter()
                .enumerate()
                .filter(|(vertex, point)| {
                    !ends.is_some_and(|ends| ends.contains(vertex))
                        && box_contains(&curve.bounds, **point)
                })
                .filter_map(|(vertex, point)| {
                    curve
                        .shape
                        .parameter(*point)
                        .map(|parameter| (parameter, vertex))
                })
                .collect();
            if let Some([start, end]) = ends {
                on.push((0.0, start));
                on.push((curve.shape.end_parameter(), end));
            }
            on.sort_by(|first, second| first.0.total_cmp(&second.0).then(first.1.cmp(&second.1)));
            match curve.shape {
                Shape::Line { .. } => {
                    // A straight piece cannot start and end at one vertex.
                    for pair in on.windows(2).filter(|pair| pair[0].1 != pair[1].1) {
                        pieces.push(Piece {
                            curve: index,
                            from: pair[0].1,
                            to: pair[1].1,
                            sweep: None,
                        });
                    }
                }
                Shape::Arc { .. } => {
                    // Ends that met at one vertex close an arc that sweeps more than half a turn; a
                    // shorter piece between one vertex has no length to speak of and is left out.
                    for pair in on.windows(2) {
                        let sweep = pair[1].0 - pair[0].0;
                        if pair[0].1 != pair[1].1 || sweep > PI {
                            pieces.push(Piece {
                                curve: index,
                                from: pair[0].1,
                                to: pair[1].1,
                                sweep: Some(sweep),
                            });
                        }
                    }
                }
                Shape::Circle { .. } => {
                    if on.is_empty() {
                        rings.push(Ring { curve: index });
                    }
                    // k vertices give k arcs; one vertex gives one closed piece around the whole circle.
                    for (position, &(parameter, vertex)) in on.iter().enumerate() {
                        let (next_parameter, next_vertex) = on[(position + 1) % on.len()];
                        let sweep = if on.len() == 1 {
                            TAU
                        } else {
                            (next_parameter - parameter).rem_euclid(TAU)
                        };
                        pieces.push(Piece {
                            curve: index,
                            from: vertex,
                            to: next_vertex,
                            sweep: Some(sweep),
                        });
                    }
                }
            }
        }
        Graph { pieces, rings }
    }

    /// Keeps one of every set of pieces that cover the same stretch: the one whose entity has the smallest id (step 4).
    fn remove_overlaps(&mut self, curves: &[Curve<'_>], vertices: &[[f64; 2]]) {
        let mut dropped = vec![false; self.pieces.len()];
        let mut by_ends: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        for (index, piece) in self.pieces.iter().enumerate() {
            by_ends
                .entry((piece.from.min(piece.to), piece.from.max(piece.to)))
                .or_default()
                .push(index);
        }
        for group in by_ends.values() {
            for (position, &first) in group.iter().enumerate() {
                for &second in &group[position + 1..] {
                    if dropped[first] || dropped[second] {
                        continue;
                    }
                    let (one, other) = (&self.pieces[first], &self.pieces[second]);
                    if same_stretch(one, other, curves, vertices) {
                        let later = if curves[one.curve].entity <= curves[other.curve].entity {
                            second
                        } else {
                            first
                        };
                        dropped[later] = true;
                    }
                }
            }
        }
        // Closed edges on one circle each cover all of it, whatever vertex they have, so only the one
        // of the smallest id stays.
        let mut dropped_rings = vec![false; self.rings.len()];
        let closed_edges: Vec<ClosedEdge> = (0..self.pieces.len())
            .filter(|&index| !dropped[index] && self.pieces[index].is_closed())
            .map(ClosedEdge::Piece)
            .chain((0..self.rings.len()).map(ClosedEdge::Ring))
            .collect();
        let mut kept: Vec<ClosedEdge> = Vec::new();
        for edge in closed_edges {
            let curve = &curves[self.closed_curve(edge)];
            let duplicate = kept.iter_mut().find(|other| {
                same_circle(
                    curves[self.closed_curve(**other)].shape.circle(),
                    curve.shape.circle(),
                )
            });
            let leaving = match duplicate {
                None => {
                    kept.push(edge);
                    continue;
                }
                Some(other) if curve.entity < curves[self.closed_curve(*other)].entity => {
                    std::mem::replace(other, edge)
                }
                Some(_) => edge,
            };
            match leaving {
                ClosedEdge::Piece(index) => dropped[index] = true,
                ClosedEdge::Ring(index) => dropped_rings[index] = true,
            }
        }
        let mut index = 0;
        self.pieces.retain(|_| {
            index += 1;
            !dropped[index - 1]
        });
        let mut index = 0;
        self.rings.retain(|_| {
            index += 1;
            !dropped_rings[index - 1]
        });
    }

    /// The curve of a closed edge.
    fn closed_curve(&self, edge: ClosedEdge) -> usize {
        match edge {
            ClosedEdge::Piece(index) => self.pieces[index].curve,
            ClosedEdge::Ring(index) => self.rings[index].curve,
        }
    }

    /// Ends of curves that touch nothing else: vertices of degree 1 (step 5). A closed piece counts twice.
    fn open_endpoints(&self, curves: &[Curve<'_>], vertices: &[[f64; 2]]) -> Vec<ProfileFinding> {
        let mut degree = vec![0_usize; vertices.len()];
        let mut last_curve = vec![0_usize; vertices.len()];
        for piece in &self.pieces {
            for vertex in [piece.from, piece.to] {
                degree[vertex] += 1;
                last_curve[vertex] = piece.curve;
            }
        }
        (0..vertices.len())
            .filter(|&vertex| degree[vertex] == 1)
            .map(|vertex| ProfileFinding::OpenEndpoint {
                entity: curves[last_curve[vertex]].entity.clone(),
                point: vertices[vertex],
            })
            .collect()
    }

    /// Removes the pieces that lie on no cycle (step 6).
    fn remove_bridges(&mut self, vertex_count: usize) {
        let ends: Vec<(usize, usize)> = self
            .pieces
            .iter()
            .map(|piece| (piece.from, piece.to))
            .collect();
        let bridges = bridges(vertex_count, &ends);
        let mut index = 0;
        self.pieces.retain(|_| {
            index += 1;
            !bridges[index - 1]
        });
    }

    /// Traces every face boundary with its face on the left (step 7). Closed pieces take part in the
    /// tracing, so a circle that touches other curves at one vertex sits in the right faces, and then
    /// each closed edge is split off into a chain of its own.
    fn loops(&self, curves: &[Curve<'_>], vertices: &[[f64; 2]]) -> Vec<TracedLoop> {
        let half_edges = 2 * self.pieces.len();
        let origin = |half_edge: usize| {
            let piece = &self.pieces[half_edge / 2];
            if half_edge.is_multiple_of(2) {
                piece.from
            } else {
                piece.to
            }
        };
        // Outgoing half-edges of every vertex in counterclockwise order of their tangents.
        let mut rotation: Vec<Vec<usize>> = vec![Vec::new(); vertices.len()];
        for half_edge in 0..half_edges {
            rotation[origin(half_edge)].push(half_edge);
        }
        let mut position = vec![0; half_edges];
        for outgoing in &mut rotation {
            let keys: Vec<(f64, f64, usize)> = outgoing
                .iter()
                .map(|&half_edge| {
                    let (tangent, curvature) = self.departure(half_edge, curves, vertices);
                    let mut angle = angle_of(tangent).rem_euclid(TAU);
                    if angle >= TAU - TANGENT_TIE {
                        angle -= TAU;
                    }
                    (angle, curvature, half_edge)
                })
                .collect();
            *outgoing = counterclockwise_order(keys);
            for (index, &half_edge) in outgoing.iter().enumerate() {
                position[half_edge] = index;
            }
        }
        let next = |half_edge: usize| {
            let twin = half_edge ^ 1;
            let around = &rotation[origin(twin)];
            around[(position[twin] + around.len() - 1) % around.len()]
        };
        let mut components = UnionFind::new(vertices.len() + self.rings.len());
        for piece in &self.pieces {
            components.union(piece.from, piece.to);
        }
        let mut loops = Vec::new();
        let mut visited = vec![false; half_edges];
        for start in 0..half_edges {
            if visited[start] {
                continue;
            }
            let mut chains = Vec::new();
            let mut rest = Chain::default();
            let mut half_edge = start;
            while !visited[half_edge] {
                visited[half_edge] = true;
                let edge = self.profile_edge(half_edge, curves, vertices);
                let edge_start = Some(origin(half_edge));
                if self.pieces[half_edge / 2].is_closed() {
                    chains.push(Chain {
                        edges: vec![edge],
                        starts: vec![edge_start],
                    });
                } else {
                    rest.edges.push(edge);
                    rest.starts.push(edge_start);
                }
                half_edge = next(half_edge);
            }
            // Without its closed edges the boundary still closes: each one leaves where it arrived.
            if !rest.edges.is_empty() {
                chains.push(rest);
            }
            loops.push(TracedLoop::new(
                chains,
                components.root(origin(start)),
                vertices[origin(start)],
            ));
        }
        for (index, ring) in self.rings.iter().enumerate() {
            let (center, radius) = curves[ring.curve]
                .shape
                .circle()
                .expect("rings are circles");
            for ccw in [true, false] {
                let edge = ProfileEdge {
                    entity: curves[ring.curve].entity.clone(),
                    reversed: !ccw,
                    curve: ProfileCurve::Circle {
                        center,
                        radius,
                        ccw,
                    },
                };
                loops.push(TracedLoop::new(
                    vec![Chain {
                        edges: vec![edge],
                        starts: vec![None],
                    }],
                    vertices.len() + index,
                    point_at_angle(center, radius, 0.0),
                ));
            }
        }
        loops
    }

    /// The tangent direction and signed curvature of a half-edge where it leaves its origin.
    fn departure(
        &self,
        half_edge: usize,
        curves: &[Curve<'_>],
        vertices: &[[f64; 2]],
    ) -> ([f64; 2], f64) {
        let piece = &self.pieces[half_edge / 2];
        let backward = half_edge % 2 == 1;
        let (from, to) = if backward {
            (piece.to, piece.from)
        } else {
            (piece.from, piece.to)
        };
        match (piece.sweep, curves[piece.curve].shape.circle()) {
            (Some(_), Some((center, radius))) => {
                let radial = subtract(vertices[from], center);
                if backward {
                    ([radial[1], -radial[0]], -1.0 / radius)
                } else {
                    ([-radial[1], radial[0]], 1.0 / radius)
                }
            }
            _ => (subtract(vertices[to], vertices[from]), 0.0),
        }
    }

    /// The profile edge of a half-edge, in traversal direction; a closed piece is a whole circle.
    fn profile_edge(
        &self,
        half_edge: usize,
        curves: &[Curve<'_>],
        vertices: &[[f64; 2]],
    ) -> ProfileEdge {
        let piece = &self.pieces[half_edge / 2];
        let backward = half_edge % 2 == 1;
        let (start, end) = if backward {
            (vertices[piece.to], vertices[piece.from])
        } else {
            (vertices[piece.from], vertices[piece.to])
        };
        let curve = match (piece.sweep, curves[piece.curve].shape.circle()) {
            (Some(_), Some((center, radius))) if piece.is_closed() => ProfileCurve::Circle {
                center,
                radius,
                ccw: !backward,
            },
            (Some(_), Some((center, radius))) => ProfileCurve::Arc {
                center,
                radius,
                start,
                end,
                ccw: !backward,
            },
            _ => ProfileCurve::Line { start, end },
        };
        ProfileEdge {
            entity: curves[piece.curve].entity.clone(),
            reversed: backward,
            curve,
        }
    }
}

/// True when two pieces with the same end vertices cover the same stretch.
fn same_stretch(
    first: &Piece,
    second: &Piece,
    curves: &[Curve<'_>],
    vertices: &[[f64; 2]],
) -> bool {
    match (first.sweep, second.sweep) {
        (None, None) => true,
        (Some(first_sweep), Some(second_sweep)) => {
            let first_circle = curves[first.curve].shape.circle();
            let second_circle = curves[second.curve].shape.circle();
            if !same_circle(first_circle, second_circle) {
                return false;
            }
            let middle = |piece: &Piece, sweep: f64, circle: Option<([f64; 2], f64)>| {
                let (center, radius) = circle.expect("arc pieces lie on circles");
                let start_angle = angle_of(subtract(vertices[piece.from], center));
                point_at_angle(center, radius, start_angle + sweep / 2.0)
            };
            let first_middle = middle(first, first_sweep, first_circle);
            let second_middle = middle(second, second_sweep, second_circle);
            norm(subtract(first_middle, second_middle)) <= PROFILE_TOLERANCE
        }
        _ => false,
    }
}

fn same_circle(first: Option<([f64; 2], f64)>, second: Option<([f64; 2], f64)>) -> bool {
    match (first, second) {
        (Some((first_center, first_radius)), Some((second_center, second_radius))) => {
            norm(subtract(first_center, second_center)) <= PROFILE_TOLERANCE
                && (first_radius - second_radius).abs() <= PROFILE_TOLERANCE
        }
        _ => false,
    }
}

/// Sorts (angle, curvature, half-edge) counterclockwise: by angle, with ties within `TANGENT_TIE` broken by curvature.
fn counterclockwise_order(mut keys: Vec<(f64, f64, usize)>) -> Vec<usize> {
    keys.sort_by(|first, second| first.0.total_cmp(&second.0).then(first.2.cmp(&second.2)));
    let mut ordered = Vec::with_capacity(keys.len());
    let mut group_start = 0;
    for index in 1..=keys.len() {
        let ends_group = index == keys.len() || keys[index].0 - keys[index - 1].0 > TANGENT_TIE;
        if ends_group {
            let group = &mut keys[group_start..index];
            group
                .sort_by(|first, second| first.1.total_cmp(&second.1).then(first.2.cmp(&second.2)));
            ordered.extend(group.iter().map(|key| key.2));
            group_start = index;
        }
    }
    ordered
}

/// The edges that lie on no cycle of the multigraph with these edge ends (Tarjan's bridge algorithm).
fn bridges(vertex_count: usize, ends: &[(usize, usize)]) -> Vec<bool> {
    let mut adjacency: Vec<Vec<(usize, usize)>> = vec![Vec::new(); vertex_count];
    for (edge, &(from, to)) in ends.iter().enumerate() {
        adjacency[from].push((to, edge));
        adjacency[to].push((from, edge));
    }
    let mut discovered: Vec<Option<usize>> = vec![None; vertex_count];
    let mut low = vec![0; vertex_count];
    let mut is_bridge = vec![false; ends.len()];
    let mut time = 0;
    for root in 0..vertex_count {
        if discovered[root].is_some() {
            continue;
        }
        discovered[root] = Some(time);
        low[root] = time;
        time += 1;
        // Each frame: (vertex, the tree edge it was reached by, next adjacency position).
        let mut stack: Vec<(usize, Option<usize>, usize)> = vec![(root, None, 0)];
        while let Some(frame) = stack.last_mut() {
            let (vertex, parent_edge, next) = *frame;
            if let Some(&(neighbor, edge)) = adjacency[vertex].get(next) {
                frame.2 += 1;
                if Some(edge) == parent_edge {
                    continue;
                }
                match discovered[neighbor] {
                    Some(order) => low[vertex] = low[vertex].min(order),
                    None => {
                        discovered[neighbor] = Some(time);
                        low[neighbor] = time;
                        time += 1;
                        stack.push((neighbor, Some(edge), 0));
                    }
                }
            } else {
                stack.pop();
                if let (Some(&(parent, _, _)), Some(edge)) = (stack.last(), parent_edge) {
                    low[parent] = low[parent].min(low[vertex]);
                    let parent_order = discovered[parent].expect("stacked vertices are discovered");
                    if low[vertex] > parent_order {
                        is_bridge[edge] = true;
                    }
                }
            }
        }
    }
    is_bridge
}

/// A closed chain of edges in traversal order.
#[derive(Default)]
struct Chain {
    edges: Vec<ProfileEdge>,
    /// Creation index of each edge's start vertex; `None` for a circle without vertices.
    starts: Vec<Option<usize>>,
}

impl Chain {
    fn signed_area(&self) -> f64 {
        signed_area(&self.edges)
    }

    /// Rotates the chain to start with the edge of the smallest entity id, then the earliest start vertex.
    fn normalized(mut self) -> Chain {
        let first = (0..self.edges.len())
            .min_by(|&first, &second| {
                self.edges[first]
                    .entity
                    .cmp(&self.edges[second].entity)
                    .then(self.starts[first].cmp(&self.starts[second]))
            })
            .unwrap_or(0);
        self.edges.rotate_left(first);
        self.starts.rotate_left(first);
        self
    }

    fn entries(&self) -> Vec<KeyEdge> {
        self.edges
            .iter()
            .map(|edge| KeyEdge {
                entity: edge.entity.clone(),
                reversed: edge.reversed,
            })
            .collect()
    }
}

/// A traced face boundary with what classification needs.
struct TracedLoop {
    /// The boundary as closed chains: each closed edge on its own, then the other edges.
    chains: Vec<Chain>,
    /// Connected component after bridge removal; circles without vertices form their own.
    component: usize,
    /// The boundary's first vertex (a point of the circle for a circle without vertices), for containment tests.
    point: [f64; 2],
    /// Signed area of the whole boundary.
    area: f64,
    /// `[min u, min v, max u, max v]` of the boundary's curves.
    bounds: [f64; 4],
}

impl TracedLoop {
    fn new(chains: Vec<Chain>, component: usize, point: [f64; 2]) -> TracedLoop {
        let area = chains.iter().map(Chain::signed_area).sum();
        let bounds = curve_bounds(chains.iter().flat_map(|chain| &chain.edges));
        TracedLoop {
            chains,
            component,
            point,
            area,
            bounds,
        }
    }

    fn curves(&self) -> Vec<ProfileCurve> {
        self.chains
            .iter()
            .flat_map(|chain| &chain.edges)
            .map(|edge| edge.curve)
            .collect()
    }
}

/// Bounds of a boundary's curves (circles and arcs by their whole circle).
fn curve_bounds<'a>(edges: impl IntoIterator<Item = &'a ProfileEdge>) -> [f64; 4] {
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    let mut include = |low: [f64; 2], high: [f64; 2]| {
        bounds = [
            bounds[0].min(low[0]),
            bounds[1].min(low[1]),
            bounds[2].max(high[0]),
            bounds[3].max(high[1]),
        ];
    };
    for edge in edges {
        match edge.curve {
            ProfileCurve::Line { start, end } => {
                include(start, start);
                include(end, end);
            }
            ProfileCurve::Arc { center, radius, .. }
            | ProfileCurve::Circle { center, radius, .. } => {
                include(
                    [center[0] - radius, center[1] - radius],
                    [center[0] + radius, center[1] + radius],
                );
            }
        }
    }
    bounds
}

/// Assigns holes to outer loops and builds the sorted regions (section 3.13.2, steps 8 and 9).
fn regions(loops: Vec<TracedLoop>) -> Vec<Region> {
    let mut holes_of: Vec<Vec<usize>> = vec![Vec::new(); loops.len()];
    for (hole, candidate) in loops.iter().enumerate() {
        if candidate.area >= 0.0 {
            continue;
        }
        let container = loops
            .iter()
            .enumerate()
            .filter(|(_, outer)| {
                outer.area > 0.0
                    && outer.component != candidate.component
                    && box_contains(&outer.bounds, candidate.point)
                    && winding_number(&outer.curves(), candidate.point) != 0
            })
            .min_by(|(first_index, first), (second_index, second)| {
                first
                    .area
                    .total_cmp(&second.area)
                    .then(first_index.cmp(second_index))
            });
        if let Some((outer, _)) = container {
            holes_of[outer].push(hole);
        }
    }
    let mut loops: Vec<Option<TracedLoop>> = loops.into_iter().map(Some).collect();
    let mut regions = Vec::new();
    for outer_index in 0..loops.len() {
        let is_outer = loops[outer_index]
            .as_ref()
            .is_some_and(|candidate| candidate.area > 0.0);
        if !is_outer {
            continue;
        }
        let TracedLoop {
            mut chains, point, ..
        } = loops[outer_index]
            .take()
            .expect("an outer loop is used once");
        // The chain with the largest signed area bounds the face from outside; any other chain is a
        // closed edge that touches the face from inside, so it is a hole.
        let areas: Vec<f64> = chains.iter().map(Chain::signed_area).collect();
        let outermost = (1..areas.len()).fold(0, |best, index| {
            if areas[index] > areas[best] {
                index
            } else {
                best
            }
        });
        let outer = chains.remove(outermost).normalized();
        let assigned = holes_of[outer_index].iter().flat_map(|&hole| {
            loops[hole]
                .take()
                .expect("a hole belongs to one outer loop")
                .chains
        });
        let mut holes: Vec<Chain> = chains
            .into_iter()
            .chain(assigned)
            .map(Chain::normalized)
            .collect();
        holes.sort_by_cached_key(|hole| loop_text(&canonical_entries(hole.entries())));
        if let Some(region) = region(outer, holes, point) {
            regions.push(region);
        }
    }
    let mut keyed: Vec<(String, Region)> = regions
        .into_iter()
        .map(|region| (region.key.to_string(), region))
        .collect();
    keyed.sort_by(|(first_text, first), (second_text, second)| {
        first_text
            .cmp(second_text)
            .then(first.centroid[0].total_cmp(&second.centroid[0]))
            .then(first.centroid[1].total_cmp(&second.centroid[1]))
    });
    keyed.into_iter().map(|(_, region)| region).collect()
}

/// A chain's key entries as a region key sorts them.
fn canonical_entries(mut entries: Vec<KeyEdge>) -> Vec<KeyEdge> {
    entries.sort();
    entries.dedup();
    entries
}

/// The region of an outer chain and its holes, or `None` when its area is too small. Moments are
/// taken about `origin`, a point near the region.
fn region(outer: Chain, holes: Vec<Chain>, origin: [f64; 2]) -> Option<Region> {
    let mut totals = [0.0; 3];
    let edges = outer
        .edges
        .iter()
        .chain(holes.iter().flat_map(|hole| &hole.edges));
    for edge in edges {
        let part = moments(&edge.curve, origin);
        totals = [
            totals[0] + part[0],
            totals[1] + part[1],
            totals[2] + part[2],
        ];
    }
    let area = totals[0];
    if area <= MIN_REGION_AREA {
        return None;
    }
    let key = RegionKey::new(outer.entries(), holes.iter().map(Chain::entries).collect());
    Some(Region {
        key,
        centroid: [origin[0] + totals[1] / area, origin[1] + totals[2] / area],
        area,
        outer: ProfileLoop { edges: outer.edges },
        holes: holes
            .into_iter()
            .map(|hole| ProfileLoop { edges: hole.edges })
            .collect(),
    })
}

/// Sorts findings by type, then entity ids, then point (section 3.13.7).
fn compare_findings(first: &ProfileFinding, second: &ProfileFinding) -> Ordering {
    let key = |finding: &ProfileFinding| -> (u8, Vec<Id>, [f64; 2]) {
        match finding {
            ProfileFinding::ZeroLength { entity } => (0, vec![entity.clone()], [0.0, 0.0]),
            ProfileFinding::OpenEndpoint { entity, point } => (1, vec![entity.clone()], *point),
            ProfileFinding::Overlap {
                entities, start, ..
            } => (2, entities.to_vec(), *start),
            ProfileFinding::SelfIntersection { entities, point } => (3, entities.to_vec(), *point),
        }
    };
    let (first_rank, first_ids, first_point) = key(first);
    let (second_rank, second_ids, second_point) = key(second);
    first_rank
        .cmp(&second_rank)
        .then(first_ids.cmp(&second_ids))
        .then(first_point[0].total_cmp(&second_point[0]))
        .then(first_point[1].total_cmp(&second_point[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(id: &str, geometry: Geometry) -> SketchEntity {
        SketchEntity {
            id: Id::new(id).expect("test ids are valid"),
            geometry,
            construction: false,
        }
    }

    fn line(id: &str, start: [f64; 2], end: [f64; 2]) -> SketchEntity {
        entity(id, Geometry::Line { start, end })
    }

    fn circle(id: &str, center: [f64; 2], radius: f64) -> SketchEntity {
        entity(id, Geometry::Circle { center, radius })
    }

    /// The 80 x 50 rectangle e-1 to e-4, drawn counterclockwise from the origin.
    fn rectangle() -> Vec<SketchEntity> {
        vec![
            line("e-1", [0.0, 0.0], [80.0, 0.0]),
            line("e-2", [80.0, 0.0], [80.0, 50.0]),
            line("e-3", [80.0, 50.0], [0.0, 50.0]),
            line("e-4", [0.0, 50.0], [0.0, 0.0]),
        ]
    }

    fn keys(profile: &Profile) -> Vec<String> {
        profile
            .regions
            .iter()
            .map(|region| region.key.to_string())
            .collect()
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1e-9 * expected.abs(),
            "{actual} is not {expected}"
        );
    }

    /// Every loop closes, and a circle edge is the only edge of its loop, as kernel profiles need.
    fn assert_loops_close(profile: &Profile) {
        let loops = profile
            .regions
            .iter()
            .flat_map(|region| std::iter::once(&region.outer).chain(&region.holes));
        for profile_loop in loops {
            let ends: Vec<Option<([f64; 2], [f64; 2])>> = profile_loop
                .edges
                .iter()
                .map(|edge| match edge.curve {
                    ProfileCurve::Line { start, end } | ProfileCurve::Arc { start, end, .. } => {
                        Some((start, end))
                    }
                    ProfileCurve::Circle { .. } => None,
                })
                .collect();
            if ends.contains(&None) {
                assert_eq!(
                    ends.len(),
                    1,
                    "a circle edge shares its loop: {profile_loop:?}"
                );
                continue;
            }
            let ends: Vec<([f64; 2], [f64; 2])> = ends.into_iter().flatten().collect();
            for (position, (_, end)) in ends.iter().enumerate() {
                let (next_start, _) = ends[(position + 1) % ends.len()];
                assert_eq!(*end, next_start, "the loop breaks: {profile_loop:?}");
            }
        }
    }

    #[test]
    fn circle_touching_an_edge_from_inside_is_a_disc_and_a_hole() {
        // Exactly tangent, and overlapping the edge by 1e-10, which is within the tolerance.
        for center_v in [5.0, 4.9999999999] {
            let mut entities = rectangle();
            entities.push(circle("e-5", [40.0, center_v], 5.0));

            let profile = build(&entities);

            assert_eq!(
                keys(&profile),
                ["e-1,e-2,e-3,e-4;~e-5", "e-5"],
                "{center_v}"
            );
            assert_close(profile.regions[0].area, 4000.0 - 25.0 * PI);
            assert_close(profile.regions[1].area, 25.0 * PI);
            assert_eq!(
                profile.regions[0].holes[0].edges[0].curve,
                ProfileCurve::Circle {
                    center: [40.0, center_v],
                    radius: 5.0,
                    ccw: false
                }
            );
            assert!(profile.findings.is_empty(), "{:?}", profile.findings);
            assert_loops_close(&profile);
        }
    }

    #[test]
    fn circle_touching_an_edge_from_outside_keeps_its_disc() {
        for center_v in [-5.0, -4.9999999999] {
            let mut entities = rectangle();
            entities.push(circle("e-5", [40.0, center_v], 5.0));

            let profile = build(&entities);

            assert_eq!(keys(&profile), ["e-1,e-2,e-3,e-4", "e-5"], "{center_v}");
            assert_close(profile.regions[0].area, 4000.0);
            assert_close(profile.regions[1].area, 25.0 * PI);
            assert!(profile.findings.is_empty(), "{:?}", profile.findings);
            assert_loops_close(&profile);
        }
    }

    #[test]
    fn touching_shapes_inside_an_outline_are_separate_holes() {
        let entities = [
            line("e-1", [0.0, 0.0], [100.0, 0.0]),
            line("e-2", [100.0, 0.0], [100.0, 100.0]),
            line("e-3", [100.0, 100.0], [0.0, 100.0]),
            line("e-4", [0.0, 100.0], [0.0, 0.0]),
            line("e-5", [20.0, 20.0], [40.0, 20.0]),
            line("e-6", [40.0, 20.0], [40.0, 30.0]),
            line("e-7", [40.0, 30.0], [20.0, 30.0]),
            line("e-8", [20.0, 30.0], [20.0, 20.0]),
            circle("e-9", [30.0, 15.0], 5.0),
        ];

        let profile = build(&entities);

        assert_eq!(
            keys(&profile),
            [
                "e-1,e-2,e-3,e-4;~e-5,~e-6,~e-7,~e-8;~e-9",
                "e-5,e-6,e-7,e-8",
                "e-9"
            ]
        );
        assert_close(profile.regions[0].area, 10000.0 - 200.0 - 25.0 * PI);
        assert_eq!(profile.regions[0].holes.len(), 2);
        assert!(profile.findings.is_empty(), "{:?}", profile.findings);
        assert_loops_close(&profile);
    }

    #[test]
    fn corner_on_a_circle_keeps_the_disc() {
        // Drawn clockwise from the corner, so the outside of the triangle is traced from the corner.
        let corner = [4.9999999999, 0.0];
        let entities = [
            circle("e-1", [0.0, 0.0], 5.0),
            line("e-2", corner, [15.0, 5.0]),
            line("e-3", [15.0, 5.0], [15.0, -5.0]),
            line("e-4", [15.0, -5.0], corner),
        ];

        let profile = build(&entities);

        assert_eq!(keys(&profile), ["e-1", "~e-2,~e-3,~e-4"]);
        assert_close(profile.regions[0].area, 25.0 * PI);
        assert_close(profile.regions[1].area, 5.0 * (15.0 - corner[0]));
        assert!(profile.findings.is_empty(), "{:?}", profile.findings);
        assert_loops_close(&profile);
    }

    #[test]
    fn corner_touching_a_circle_from_inside_is_a_hole_of_the_disc() {
        let entities = [
            circle("e-1", [0.0, 0.0], 5.0),
            line("e-2", [5.0, 0.0], [0.0, 2.0]),
            line("e-3", [0.0, 2.0], [0.0, -2.0]),
            line("e-4", [0.0, -2.0], [5.0, 0.0]),
        ];

        let profile = build(&entities);

        assert_eq!(keys(&profile), ["e-1;~e-2,~e-3,~e-4", "e-2,e-3,e-4"]);
        assert_close(profile.regions[0].area, 25.0 * PI - 10.0);
        assert_close(profile.regions[1].area, 10.0);
        assert_loops_close(&profile);
    }

    #[test]
    fn circle_tangent_inside_a_circle_is_its_hole() {
        let entities = [
            circle("e-1", [0.0, 0.0], 10.0),
            circle("e-2", [5.0, 0.0], 5.0),
        ];

        let profile = build(&entities);

        assert_eq!(keys(&profile), ["e-1;~e-2", "e-2"]);
        assert_close(profile.regions[0].area, 75.0 * PI);
        assert_close(profile.regions[1].area, 25.0 * PI);
        assert_loops_close(&profile);
    }

    #[test]
    fn nearly_full_arc_whose_ends_meet_is_a_circle_edge() {
        let entities = [entity(
            "e-1",
            Geometry::Arc {
                center: [0.0, 0.0],
                radius: 5.0,
                start_angle: 0.0,
                end_angle: 6.2831852,
            },
        )];

        let profile = build(&entities);

        assert_eq!(keys(&profile), ["e-1"]);
        assert_eq!(
            profile.regions[0].outer.edges,
            [ProfileEdge {
                entity: Id::new("e-1").expect("valid id"),
                reversed: false,
                curve: ProfileCurve::Circle {
                    center: [0.0, 0.0],
                    radius: 5.0,
                    ccw: true
                },
            }]
        );
        assert_close(profile.regions[0].area, 25.0 * PI);
        assert!(profile.findings.is_empty(), "{:?}", profile.findings);
    }

    #[test]
    fn identical_circles_keep_the_smaller_id() {
        // The second circle is the first within the tolerance but misses the overlap's vertex at (5, 0).
        for second in [
            circle("e-2", [0.0, 0.0], 5.0),
            circle("e-2", [-0.9e-6, 0.0], 5.0 - 0.9e-6),
        ] {
            let entities = [circle("e-1", [0.0, 0.0], 5.0), second];

            let profile = build(&entities);

            assert_eq!(keys(&profile), ["e-1"]);
            assert_close(profile.regions[0].area, 25.0 * PI);
            assert!(matches!(
                profile.findings.as_slice(),
                [ProfileFinding::Overlap { .. }]
            ));
        }
    }

    #[test]
    fn bridges_of_two_triangles_joined_by_an_edge() {
        let ends = [(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 5), (5, 3)];

        assert_eq!(
            bridges(6, &ends),
            [false, false, false, true, false, false, false]
        );
    }

    #[test]
    fn parallel_edges_are_not_bridges() {
        assert_eq!(bridges(2, &[(0, 1), (0, 1)]), [false, false]);
    }

    #[test]
    fn a_dangling_chain_is_all_bridges() {
        assert_eq!(bridges(3, &[(0, 1), (1, 2)]), [true, true]);
    }

    #[test]
    fn tied_tangents_order_by_curvature() {
        let order =
            counterclockwise_order(vec![(1.0, 0.5, 0), (1.0 + 1e-12, -0.5, 1), (0.5, 0.0, 2)]);

        assert_eq!(order, [2, 1, 0]);
    }

    #[test]
    fn line_circle_touch_gives_one_point() {
        assert_eq!(
            line_circle([0.0, 0.0], [10.0, 0.0], [5.0, 3.0], 3.0),
            [[5.0, 0.0]]
        );
    }

    #[test]
    fn crossing_circles_meet_twice() {
        let points = circle_circle([0.0, 0.0], 5.0, [8.0, 0.0], 5.0);

        assert_eq!(points.len(), 2);
        assert!((points[0][0] - 4.0).abs() < 1e-12 && (points[0][1] - 3.0).abs() < 1e-12);
    }

    #[test]
    fn collinear_overlap_runs_along_the_first_line() {
        let first = Shape::Line {
            start: [0.0, 0.0],
            end: [60.0, 0.0],
        };
        let second = Shape::Line {
            start: [80.0, 0.0],
            end: [20.0, 0.0],
        };

        assert_eq!(
            line_overlap(&first, &second),
            Some([[20.0, 0.0], [60.0, 0.0]])
        );
    }

    #[test]
    fn arcs_on_one_circle_share_their_common_range() {
        let arc = |start_angle: f64, sweep: f64| Shape::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle,
            sweep,
            start: point_at_angle([0.0, 0.0], 1.0, start_angle),
            end: point_at_angle([0.0, 0.0], 1.0, start_angle + sweep),
        };

        let shared = circle_overlaps(&arc(0.0, PI), &arc(PI / 2.0, PI));

        assert_eq!(shared.len(), 1);
        assert!(norm(subtract(shared[0][0], [0.0, 1.0])) < 1e-12);
        assert!(norm(subtract(shared[0][1], [-1.0, 0.0])) < 1e-12);
    }
}
