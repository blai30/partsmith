//! The chain a pick of one curve stands for (spec section 3.18.4).

use std::collections::VecDeque;

use partsmith_model::Id;

use super::ChainKey;
use crate::PROFILE_TOLERANCE;
use crate::entity::SketchEntity;
use crate::point_ref::PointRole;
use crate::profile::{KeyEdge, Shape};
use crate::vector::{norm, subtract};

/// The start or end point of a curve that is a line or an arc.
#[derive(Clone, Copy, PartialEq)]
struct End {
    /// The curve's position among the curves.
    curve: usize,
    /// `Start` or `End`.
    role: PointRole,
    /// Where it lies.
    point: [f64; 2],
}

/// The ends of the curves, sorted by u, so the ends at a point are found without reading every end.
struct Ends(Vec<End>);

impl Ends {
    /// The start and end of every line and arc among `shapes`.
    fn new(shapes: &[Shape]) -> Ends {
        let mut ends: Vec<End> = shapes
            .iter()
            .enumerate()
            .filter_map(|(curve, shape)| Some((curve, shape.endpoints()?)))
            .flat_map(|(curve, [start, end])| {
                [(PointRole::Start, start), (PointRole::End, end)].map(|(role, point)| End {
                    curve,
                    role,
                    point,
                })
            })
            .collect();
        ends.sort_by(|first, second| first.point[0].total_cmp(&second.point[0]));
        Ends(ends)
    }

    /// The ends within `PROFILE_TOLERANCE` of `own`, other than `own`.
    fn at(&self, own: End) -> Vec<End> {
        let first = self
            .0
            .partition_point(|end| end.point[0] < own.point[0] - PROFILE_TOLERANCE);
        self.0[first..]
            .iter()
            .take_while(|end| end.point[0] <= own.point[0] + PROFILE_TOLERANCE)
            .filter(|end| **end != own && norm(subtract(end.point, own.point)) <= PROFILE_TOLERANCE)
            .copied()
            .collect()
    }
}

/// The end of the line or arc `curve` with `role`.
fn end_of(shapes: &[Shape], curve: usize, role: PointRole) -> End {
    let [start, end] = shapes[curve]
        .endpoints()
        .expect("a chain extends only lines and arcs");
    let point = if role == PointRole::Start { start } else { end };
    End { curve, role, point }
}

/// The roles of the ends a chain entry starts and ends at.
fn roles(reversed: bool) -> [PointRole; 2] {
    if reversed {
        [PointRole::End, PointRole::Start]
    } else {
        [PointRole::Start, PointRole::End]
    }
}

/// The chain a pick of `seed` stands for (section 3.18.4); None when `seed` is not a non-construction line, arc or
/// circle of `entities` with a length. The chain runs in the seed's own direction and extends through every point
/// where exactly two curve ends meet.
pub fn chain_through(entities: &[SketchEntity], seed: &Id) -> Option<ChainKey> {
    let (ids, shapes): (Vec<&Id>, Vec<Shape>) = entities
        .iter()
        .filter(|entity| !entity.construction)
        .filter_map(|entity| {
            let shape = Shape::of(&entity.geometry).filter(|shape| !shape.is_zero_size())?;
            Some((&entity.id, shape))
        })
        .unzip();
    let seed = ids.iter().position(|id| *id == seed)?;
    let mut chain = VecDeque::from([(seed, false)]);
    if shapes[seed].endpoints().is_some() {
        extend(&shapes, &mut chain);
    }
    let entries = chain
        .into_iter()
        .map(|(curve, reversed)| KeyEdge {
            entity: ids[curve].clone(),
            reversed,
        })
        .collect();
    Some(ChainKey::new(entries).expect("a walk adds each curve once"))
}

/// Extends a chain of one line or arc, given as `(curve, reversed)` entries, forward from its end, then backward from
/// its start unless it closed.
fn extend(shapes: &[Shape], chain: &mut VecDeque<(usize, bool)>) {
    let ends = Ends::new(shapes);
    let (seed, _) = chain[0];
    let seed_start = end_of(shapes, seed, PointRole::Start);
    let mut in_chain = vec![false; shapes.len()];
    in_chain[seed] = true;
    loop {
        let (last, reversed) = chain[chain.len() - 1];
        let [found] = ends.at(end_of(shapes, last, roles(reversed)[1]))[..] else {
            break;
        };
        if found == seed_start {
            return;
        }
        if in_chain[found.curve] {
            break;
        }
        in_chain[found.curve] = true;
        chain.push_back((found.curve, found.role == PointRole::End));
    }
    loop {
        let (first, reversed) = chain[0];
        let [found] = ends.at(end_of(shapes, first, roles(reversed)[0]))[..] else {
            break;
        };
        if in_chain[found.curve] {
            break;
        }
        in_chain[found.curve] = true;
        chain.push_front((found.curve, found.role == PointRole::Start));
    }
}
