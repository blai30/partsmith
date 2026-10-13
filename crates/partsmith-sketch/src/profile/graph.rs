//! The edges of the planar arrangement and the face boundaries traced along them (spec section
//! 3.13.2, steps 3 to 7).

use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};

use super::intersection::{Bounds, Curve, Shape, same_circle};
use super::key::KeyEdge;
use super::{ProfileCurve, ProfileEdge, ProfileFinding, signed_area};
use crate::PROFILE_TOLERANCE;
use crate::entity::point_at_angle;
use crate::union_find::UnionFind;
use crate::vector::{angle_of, norm, subtract};

/// Outgoing tangents whose angles differ by at most this much (rad) are ordered by curvature.
const TANGENT_TIE: f64 = 1e-9;

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

/// A piece in the direction of one of its half-edges.
struct HalfEdge<'a> {
    piece: &'a Piece,
    /// The vertex the half-edge leaves.
    from: usize,
    /// The vertex the half-edge reaches.
    to: usize,
    /// True when the half-edge runs against its piece.
    backward: bool,
}

/// An edge that closes on itself: a closed piece or a ring, by index.
#[derive(Clone, Copy, Debug)]
enum ClosedEdge {
    Piece(usize),
    Ring(usize),
}

/// The edges of the arrangement.
pub(super) struct Graph {
    pieces: Vec<Piece>,
    rings: Vec<Ring>,
}

impl Graph {
    /// Splits every curve at the vertices on it (section 3.13.2, step 3).
    pub(super) fn split(
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
                        && curve.bounds.contains(**point)
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
    pub(super) fn remove_overlaps(&mut self, curves: &[Curve<'_>], vertices: &[[f64; 2]]) {
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
        retain_unmarked(&mut self.pieces, &dropped);
        retain_unmarked(&mut self.rings, &dropped_rings);
    }

    /// The curve of a closed edge.
    fn closed_curve(&self, edge: ClosedEdge) -> usize {
        match edge {
            ClosedEdge::Piece(index) => self.pieces[index].curve,
            ClosedEdge::Ring(index) => self.rings[index].curve,
        }
    }

    /// Ends of curves that touch nothing else: vertices of degree 1 (step 5). A closed piece counts twice.
    pub(super) fn open_endpoints(
        &self,
        curves: &[Curve<'_>],
        vertices: &[[f64; 2]],
    ) -> Vec<ProfileFinding> {
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
    pub(super) fn remove_bridges(&mut self, vertex_count: usize) {
        let ends: Vec<(usize, usize)> = self
            .pieces
            .iter()
            .map(|piece| (piece.from, piece.to))
            .collect();
        retain_unmarked(&mut self.pieces, &bridges(vertex_count, &ends));
    }

    /// Traces every face boundary with its face on the left (step 7). Closed pieces take part in the
    /// tracing, so a circle that touches other curves at one vertex sits in the right faces, and then
    /// each closed edge is split off into a chain of its own.
    pub(super) fn loops(&self, curves: &[Curve<'_>], vertices: &[[f64; 2]]) -> Vec<TracedLoop> {
        let half_edges = 2 * self.pieces.len();
        let origin = |half_edge: usize| self.half_edge(half_edge).from;
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
                if self.half_edge(half_edge).piece.is_closed() {
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

    /// Half-edge `2 i` runs along piece `i` and half-edge `2 i + 1` against it.
    fn half_edge(&self, half_edge: usize) -> HalfEdge<'_> {
        let piece = &self.pieces[half_edge / 2];
        let backward = half_edge % 2 == 1;
        let (from, to) = if backward {
            (piece.to, piece.from)
        } else {
            (piece.from, piece.to)
        };
        HalfEdge {
            piece,
            from,
            to,
            backward,
        }
    }

    /// The tangent direction and signed curvature of a half-edge where it leaves its origin.
    fn departure(
        &self,
        half_edge: usize,
        curves: &[Curve<'_>],
        vertices: &[[f64; 2]],
    ) -> ([f64; 2], f64) {
        let HalfEdge {
            piece,
            from,
            to,
            backward,
        } = self.half_edge(half_edge);
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
        let HalfEdge {
            piece,
            from,
            to,
            backward,
        } = self.half_edge(half_edge);
        let (start, end) = (vertices[from], vertices[to]);
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

/// Keeps the items whose mark is false; `marks` has one mark per item.
fn retain_unmarked<T>(items: &mut Vec<T>, marks: &[bool]) {
    let mut marks = marks.iter();
    items.retain(|_| marks.next() == Some(&false));
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
pub(super) struct Chain {
    pub(super) edges: Vec<ProfileEdge>,
    /// Creation index of each edge's start vertex; `None` for a circle without vertices.
    starts: Vec<Option<usize>>,
}

impl Chain {
    pub(super) fn signed_area(&self) -> f64 {
        signed_area(&self.edges)
    }

    /// Rotates the chain to start with the edge of the smallest entity id, then the earliest start vertex.
    pub(super) fn normalized(mut self) -> Chain {
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

    pub(super) fn entries(&self) -> Vec<KeyEdge> {
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
pub(super) struct TracedLoop {
    /// The boundary as closed chains: each closed edge on its own, then the other edges.
    pub(super) chains: Vec<Chain>,
    /// Connected component after bridge removal; circles without vertices form their own.
    pub(super) component: usize,
    /// The boundary's first vertex (a point of the circle for a circle without vertices), for containment tests.
    pub(super) point: [f64; 2],
    /// Signed area of the whole boundary.
    pub(super) area: f64,
    /// The box around the boundary's curves.
    pub(super) bounds: Bounds,
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

    pub(super) fn curves(&self) -> Vec<ProfileCurve> {
        self.chains
            .iter()
            .flat_map(|chain| &chain.edges)
            .map(|edge| edge.curve)
            .collect()
    }
}

/// Bounds of a boundary's curves (circles and arcs by their whole circle).
fn curve_bounds<'a>(edges: impl IntoIterator<Item = &'a ProfileEdge>) -> Bounds {
    let mut bounds = Bounds::EMPTY;
    for edge in edges {
        bounds.include(match edge.curve {
            ProfileCurve::Line { start, end } => Bounds::around(start, end),
            ProfileCurve::Arc { center, radius, .. }
            | ProfileCurve::Circle { center, radius, .. } => Bounds::of_circle(center, radius),
        });
    }
    bounds
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
