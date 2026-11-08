//! Building a chain from solved entities: the checks of spec section 3.18.2 in their order, then the edges.

use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};

use partsmith_model::{ANGULAR_TOLERANCE, Id};

use super::{Chain, ChainKey, ChainLoss, Junction};
use crate::PROFILE_TOLERANCE;
use crate::entity::{EntityKind, SketchEntity};
use crate::point_ref::{PointRef, PointRole};
use crate::profile::{Curve, ProfileCurve, ProfileEdge, Shape, meet};
use crate::vector::{cross, dot, norm, scale, subtract, unit};

/// One entry of a key whose entity passed the checks of step 2.
struct Entry<'a> {
    /// The entity.
    entity: &'a Id,
    /// True when the chain runs against the entity's own direction.
    reversed: bool,
    /// The entity's line, arc or circle.
    shape: Shape,
    /// The traversal start: the entity's own start, or its own end when reversed, and for a circle the point at
    /// angle 0.
    start: [f64; 2],
    /// The traversal end, the start for a circle.
    end: [f64; 2],
    /// The unit tangents along the chain at the traversal start and end (step 5).
    tangents: [[f64; 2]; 2],
}

impl Entry<'_> {
    /// The point references of the traversal start and end: `.start` and `.end`, swapped when reversed.
    fn end_references(&self) -> [PointRef; 2] {
        let [first, second] = [PointRole::Start, PointRole::End].map(|role| PointRef {
            entity: self.entity.clone(),
            role,
        });
        if self.reversed {
            [second, first]
        } else {
            [first, second]
        }
    }
}

/// Builds the chain `key` names on `entities`, or gives the first check that fails (section 3.18.2). With
/// `stored_closed`, a chain that is not closed is lost as two entries that do not meet (section 3.18.3).
pub(super) fn build(
    entities: &[SketchEntity],
    key: &ChainKey,
    stored_closed: bool,
) -> Result<Chain, ChainLoss> {
    let by_id: BTreeMap<&Id, &SketchEntity> =
        entities.iter().map(|entity| (&entity.id, entity)).collect();
    let entries = checked_entries(&by_id, key)?;
    for pair in entries.windows(2) {
        connected(&pair[0], &pair[1])?;
    }
    let (first, last) = (&entries[0], &entries[entries.len() - 1]);
    let closing = connected(last, first);
    let closed = closing.is_ok();
    if stored_closed {
        closing?;
    }
    let junctions = junctions(&entries, closed);
    if let Some(index) = junctions
        .iter()
        .position(|junction| junction.turn.abs() >= PI - ANGULAR_TOLERANCE)
    {
        return Err(ChainLoss::TurnsBack {
            entities: [
                entries[index].entity.clone(),
                entries[(index + 1) % entries.len()].entity.clone(),
            ],
            point: junctions[index].point,
        });
    }
    crossings(&entries, &junctions, closed)?;
    Ok(Chain {
        key: key.clone(),
        edges: edges(&entries, closed),
        closed,
        junctions,
        length: entries.iter().map(|entry| own_length(entry.shape)).sum(),
        start_tangent: first.tangents[0],
        end_tangent: last.tangents[1],
    })
}

/// Steps 1 and 2: the entities the key names, each a non-construction line or arc with a length, or one circle.
fn checked_entries<'a>(
    by_id: &BTreeMap<&Id, &'a SketchEntity>,
    key: &ChainKey,
) -> Result<Vec<Entry<'a>>, ChainLoss> {
    let mut deleted: Vec<Id> = key
        .entries()
        .iter()
        .filter(|entry| !by_id.contains_key(&entry.entity))
        .map(|entry| entry.entity.clone())
        .collect();
    if !deleted.is_empty() {
        deleted.sort();
        return Err(ChainLoss::Deleted {
            all: deleted.len() == key.entries().len(),
            entities: deleted,
        });
    }
    let alone = key.entries().len() == 1;
    key.entries()
        .iter()
        .map(|entry| {
            let entity = by_id[&entry.entity];
            let kind = entity.kind();
            let shape = Shape::of(&entity.geometry).filter(|_| alone || kind != EntityKind::Circle);
            let Some(shape) = shape else {
                return Err(ChainLoss::NotCurve {
                    entity: entity.id.clone(),
                    kind,
                });
            };
            if entity.construction {
                return Err(ChainLoss::Construction {
                    entity: entity.id.clone(),
                });
            }
            if shape.is_zero_size() {
                return Err(ChainLoss::ZeroLength {
                    entity: entity.id.clone(),
                });
            }
            let [start, end] = match shape.endpoints() {
                Some([start, end]) if entry.reversed => [end, start],
                Some(ends) => ends,
                None => [shape.point_at(0.0); 2],
            };
            Ok(Entry {
                entity: &entity.id,
                reversed: entry.reversed,
                shape,
                start,
                end,
                tangents: tangents(shape, entry.reversed),
            })
        })
        .collect()
}

/// Step 3: `Ok` when `arriving` ends within the tolerance of where `leaving` starts, otherwise `Disconnected`.
fn connected(arriving: &Entry<'_>, leaving: &Entry<'_>) -> Result<(), ChainLoss> {
    let gap = norm(subtract(leaving.start, arriving.end));
    if gap <= PROFILE_TOLERANCE {
        return Ok(());
    }
    let [_, end] = arriving.end_references();
    let [start, _] = leaving.end_references();
    Err(ChainLoss::Disconnected { end, start, gap })
}

/// The unit tangents along the chain at an entry's traversal start and end, on the entity's own geometry (step 5): a
/// line's direction and an arc's counterclockwise tangent at its point, each negated when the entry is reversed, and
/// (0, 1) or (0, -1) for a circle.
fn tangents(shape: Shape, reversed: bool) -> [[f64; 2]; 2] {
    let sign = if reversed { -1.0 } else { 1.0 };
    let on_arc = |center: [f64; 2], radius: f64, point: [f64; 2]| {
        scale(
            [
                -(point[1] - center[1]) / radius,
                (point[0] - center[0]) / radius,
            ],
            sign,
        )
    };
    match shape {
        Shape::Line { start, end } => [scale(unit(subtract(end, start)), sign); 2],
        Shape::Arc {
            center,
            radius,
            start,
            end,
            ..
        } => {
            let [at_start, at_end] = [on_arc(center, radius, start), on_arc(center, radius, end)];
            if reversed {
                [at_end, at_start]
            } else {
                [at_start, at_end]
            }
        }
        Shape::Circle { .. } => [[0.0, sign]; 2],
    }
}

/// Step 5: one junction per two consecutive entries, and for a closed chain of lines and arcs a last one from the
/// last entry to the first. A circle has none.
fn junctions(entries: &[Entry<'_>], closed: bool) -> Vec<Junction> {
    let count = entries.len();
    let pairs = match entries[0].shape {
        Shape::Circle { .. } => 0,
        _ if closed => count,
        _ => count - 1,
    };
    (0..pairs)
        .map(|index| {
            let (arriving, leaving) = (&entries[index], &entries[(index + 1) % count]);
            Junction {
                point: leaving.start,
                turn: turn(arriving.tangents[1], leaving.tangents[0]),
            }
        })
        .collect()
}

/// The signed angle from `arriving` to `leaving` in (-pi, pi].
fn turn(arriving: [f64; 2], leaving: [f64; 2]) -> f64 {
    let angle = cross(arriving, leaving).atan2(dot(arriving, leaving));
    if angle == -PI { PI } else { angle }
}

/// Step 7: the first two entries, in order of the first then the second, whose curves meet away from the junctions
/// they share, or share a stretch.
fn crossings(entries: &[Entry<'_>], junctions: &[Junction], closed: bool) -> Result<(), ChainLoss> {
    let count = entries.len();
    let curves: Vec<Curve<'_>> = entries
        .iter()
        .map(|entry| Curve {
            entity: entry.entity,
            shape: entry.shape,
            bounds: entry.shape.bounds(),
        })
        .collect();
    for (first_index, first) in curves.iter().enumerate() {
        for (second_index, second) in curves.iter().enumerate().skip(first_index + 1) {
            if !first.bounds.overlaps(&second.bounds) {
                continue;
            }
            let mut shared = Vec::new();
            if second_index == first_index + 1 {
                shared.push(junctions[first_index].point);
            }
            if closed && first_index == 0 && second_index == count - 1 {
                shared.push(junctions[count - 1].point);
            }
            if let Some(point) = crossing(first, second, &shared) {
                return Err(ChainLoss::Crosses {
                    entities: [first.entity.clone(), second.entity.clone()],
                    point,
                });
            }
        }
    }
    Ok(())
}

/// Where two entries meet away from the `shared` junctions: the first such point along `first`, else the end of a
/// shared stretch that comes first along `first`.
fn crossing(first: &Curve<'_>, second: &Curve<'_>, shared: &[[f64; 2]]) -> Option<[f64; 2]> {
    let (points, stretches) = meet(first, second);
    // The parameter along the first entity orders the points and the ends of the stretches.
    let along = |point: [f64; 2]| first.shape.parameter(point).unwrap_or(f64::INFINITY);
    points
        .into_iter()
        .find(|point| {
            shared
                .iter()
                .all(|junction| norm(subtract(*point, *junction)) > PROFILE_TOLERANCE)
        })
        .or_else(|| {
            stretches
                .into_iter()
                .flatten()
                .min_by(|left, right| along(*left).total_cmp(&along(*right)))
        })
}

/// The edges of a chain whose checks passed: each entry oriented along the chain and ending exactly where the next
/// one starts, the last edge of a closed chain ending where the first starts. A circle, and one arc whose ends meet,
/// is a whole circle.
fn edges(entries: &[Entry<'_>], closed: bool) -> Vec<ProfileEdge> {
    let whole_circle = closed && entries.len() == 1;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let end = match entries.get(index + 1) {
                Some(next) => next.start,
                None if closed => entries[0].start,
                None => entry.end,
            };
            let ccw = !entry.reversed;
            let curve = match entry.shape {
                Shape::Line { .. } => ProfileCurve::Line {
                    start: entry.start,
                    end,
                },
                Shape::Arc { center, radius, .. } if !whole_circle => ProfileCurve::Arc {
                    center,
                    radius,
                    start: entry.start,
                    end,
                    ccw,
                },
                Shape::Arc { center, radius, .. } | Shape::Circle { center, radius } => {
                    ProfileCurve::Circle {
                        center,
                        radius,
                        ccw,
                    }
                }
            };
            ProfileEdge {
                entity: entry.entity.clone(),
                reversed: entry.reversed,
                curve,
            }
        })
        .collect()
}

/// The length of an entity's own curve: a line's length, an arc's `radius * sweep`, a circle's circumference.
fn own_length(shape: Shape) -> f64 {
    match shape {
        Shape::Line { start, end } => norm(subtract(end, start)),
        Shape::Arc { radius, sweep, .. } => radius * sweep,
        Shape::Circle { radius, .. } => TAU * radius,
    }
}
