//! Successions of a `Boundary` match: which entities now carry the place of a stored boundary entity (spec
//! section 3.14).

use std::collections::BTreeMap;

use partsmith_model::Id;

use super::super::hashing::{FastMap, FastSet};
use super::super::intersection::lies_on;
use super::super::{ProfileEdge, Region, RegionLoop};
use super::{Stored, Succession};

/// The successions of the one region a stored reference matched by boundary, sorted by entity: replacements
/// first (only with a stored outline), then extensions among the new entities no replacement uses.
pub(super) fn successions(region: &Region, stored: &Stored<'_>) -> Vec<Succession> {
    // A hand-built region can have an empty outer loop, which has no entities to follow.
    let Some(outline) = region.try_outline() else {
        return Vec::new();
    };
    let current = Sequence::of(&outline);
    let mut found = match &stored.reference.outline {
        Some(previous) => replacements(&current, &Sequence::of(previous), stored),
        // The stored key does not say where a removed entity was, so nothing tells a replacement from a
        // deletion plus an unrelated addition.
        None => Vec::new(),
    };
    let replacing: FastSet<&Id> = found
        .iter()
        .flat_map(|succession| &succession.now)
        .collect();
    let extensions = extensions(region, &current, stored, &replacing);
    found.extend(extensions);
    found.sort_by(|first, second| first.entity.cmp(&second.entity));
    found
}

/// Each removed entity that is the only entry of the stored outline between two survivors that follow each other
/// in the current loop, where the current loop has new entities between them. The runs that replace the pieces of
/// one removed entity (a line another curve interrupted) form its one succession, in the order of the current
/// loop.
fn replacements(
    current: &Sequence<'_>,
    previous: &Sequence<'_>,
    stored: &Stored<'_>,
) -> Vec<Succession> {
    let survivors: Vec<usize> = (0..current.entities.len())
        .filter(|&position| !stored.is_added(current.entities[position]))
        .collect();
    let mut found: BTreeMap<&Id, Vec<&Id>> = BTreeMap::new();
    for (rank, &position) in survivors.iter().enumerate() {
        let next = survivors[(rank + 1) % survivors.len()];
        // Every entry between two consecutive survivors is new.
        let new_run: Vec<&Id> = run_between(&current.entities, position, next).collect();
        if new_run.is_empty() {
            continue;
        }
        let Some(removed) = removed_between(current, previous, position, next) else {
            continue;
        };
        found.entry(removed).or_default().extend(new_run);
    }
    found
        .into_iter()
        .map(|(entity, now)| Succession {
            entity: entity.clone(),
            now: distinct(now),
        })
        .collect()
}

/// The removed entity that was the only entry of the stored outline between the survivors at positions `from`
/// and `to` of the current loop. `None` when either survivor occurs more than once in either loop (a line another
/// curve interrupts), which leaves the runs undetermined.
fn removed_between<'a>(
    current: &Sequence<'_>,
    previous: &Sequence<'a>,
    from: usize,
    to: usize,
) -> Option<&'a Id> {
    let (first, second) = (current.entities[from], current.entities[to]);
    current.only_position(first)?;
    current.only_position(second)?;
    let mut old_run = run_between(
        &previous.entities,
        previous.only_position(first)?,
        previous.only_position(second)?,
    );
    match (old_run.next(), old_run.next()) {
        (Some(removed), None) if current.positions_of(removed).is_empty() => Some(removed),
        _ => None,
    }
}

/// Each surviving entity that new entities next to it continue on its line or circle, with those entities in
/// traversal order starting after it.
fn extensions(
    region: &Region,
    current: &Sequence<'_>,
    stored: &Stored<'_>,
    replacing: &FastSet<&Id>,
) -> Vec<Succession> {
    let edges = outer_edges_by_entity(region);
    let edges_of = |entity: &Id| edges.get(entity).map_or(&[][..], Vec::as_slice);
    let mut marked = vec![false; current.entities.len()];
    let mut found = Vec::new();
    for (first_position, &survivor) in current.entities.iter().enumerate() {
        let positions = current.positions_of(survivor);
        // One succession per survivor, found at its first occurrence.
        if stored.is_added(survivor) || positions.first() != Some(&first_position) {
            continue;
        }
        let Some(carrier) = edges_of(survivor).first() else {
            continue;
        };
        let continues = |entity: &Id| {
            stored.is_added(entity)
                && !replacing.contains(entity)
                && edges_of(entity)
                    .iter()
                    .all(|edge| lies_on(&edge.curve, &carrier.curve))
        };
        let chained = continuing_positions(&current.entities, positions, continues, &mut marked);
        if chained.is_empty() {
            continue;
        }
        let now = chained
            .into_iter()
            .map(|position| current.entities[position]);
        found.push(Succession {
            entity: survivor.clone(),
            now: distinct(std::iter::once(survivor).chain(now)),
        });
    }
    found
}

/// The positions of the entities that `continues` accepts in a row next to any of `positions` (the occurrences
/// of one entity), in traversal order starting after the first occurrence. `marked` holds one `false` per entry
/// before and after the call.
fn continuing_positions(
    entities: &[&Id],
    positions: &[usize],
    continues: impl Fn(&Id) -> bool,
    marked: &mut [bool],
) -> Vec<usize> {
    let count = entities.len();
    let mut chained: Vec<usize> = Vec::new();
    for &position in positions {
        // 1 walks forward, count - 1 walks backward, cyclically.
        for step in [1, count - 1] {
            let mut next = (position + step) % count;
            while next != position && !marked[next] && continues(entities[next]) {
                marked[next] = true;
                chained.push(next);
                next = (next + step) % count;
            }
        }
    }
    for &position in &chained {
        marked[position] = false;
    }
    if let Some(&first) = positions.first() {
        chained.sort_by_key(|&position| (position + count - first) % count);
    }
    chained
}

/// A loop's entities as a cyclic sequence, with the positions of each entity, so every lookup is one step.
struct Sequence<'a> {
    /// The entities of the loop's entries, in its order.
    entities: Vec<&'a Id>,
    /// The positions of each entity in `entities`, ascending.
    positions: FastMap<&'a Id, Vec<usize>>,
}

impl<'a> Sequence<'a> {
    fn of(region_loop: &'a RegionLoop) -> Sequence<'a> {
        let entities: Vec<&Id> = region_loop
            .entries()
            .iter()
            .map(|entry| &entry.entity)
            .collect();
        let mut positions: FastMap<&Id, Vec<usize>> = FastMap::default();
        for (position, &entity) in entities.iter().enumerate() {
            positions.entry(entity).or_default().push(position);
        }
        Sequence {
            entities,
            positions,
        }
    }

    /// The positions where `entity` occurs, ascending; empty when it does not occur.
    fn positions_of(&self, entity: &Id) -> &[usize] {
        self.positions.get(entity).map_or(&[], Vec::as_slice)
    }

    /// The position of an entity that occurs exactly once, or `None`.
    fn only_position(&self, entity: &Id) -> Option<usize> {
        match *self.positions_of(entity) {
            [position] => Some(position),
            _ => None,
        }
    }
}

/// The entries strictly between positions `from` and `to` of a cyclic sequence; every other entry when they are
/// the same position.
fn run_between<'a, 'b>(
    sequence: &'b [&'a Id],
    from: usize,
    to: usize,
) -> impl Iterator<Item = &'a Id> + 'b {
    let count = sequence.len();
    (1..count)
        .map(move |offset| (from + offset) % count)
        .take_while(move |&position| position != to)
        .map(move |position| sequence[position])
}

/// The edges of each entity in the region's outer loop, in traversal order.
fn outer_edges_by_entity(region: &Region) -> FastMap<&Id, Vec<&ProfileEdge>> {
    let mut edges: FastMap<&Id, Vec<&ProfileEdge>> = FastMap::default();
    for edge in &region.outer.edges {
        edges.entry(&edge.entity).or_default().push(edge);
    }
    edges
}

/// The entities in order, each once, as owned ids.
fn distinct<'a>(entities: impl IntoIterator<Item = &'a Id>) -> Vec<Id> {
    let mut seen: FastSet<&Id> = FastSet::default();
    entities
        .into_iter()
        .filter(|&entity| seen.insert(entity))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::SketchEntity;
    use crate::profile::{MatchMethod, RegionMatch, RegionRef, build_profile, match_region};
    use crate::test_support::{circle, edgeless_region, entity_ids, id, line, rectangle};

    /// The successions of the first region of `stored` matched among the regions of `entities`.
    fn successions_after(stored: &[SketchEntity], entities: &[SketchEntity]) -> Vec<Succession> {
        let reference = build_profile(stored).regions[0].reference();
        match match_region(
            &build_profile(entities).regions,
            &entity_ids(entities),
            &reference,
        ) {
            RegionMatch::Resolved { successions, .. } => successions,
            other => panic!("the region resolves, found {other:?}"),
        }
    }

    /// The successions of the rectangle matched among the regions of `entities`.
    fn successions_of(entities: &[SketchEntity]) -> Vec<Succession> {
        successions_after(&rectangle(), entities)
    }

    #[test]
    fn an_extension_on_both_sides_lists_the_entities_after_it_first() {
        let entities = [
            line("e-1", [0.0, 0.0], [80.0, 0.0]),
            line("e-6", [80.0, 0.0], [80.0, 20.0]),
            line("e-2", [80.0, 20.0], [80.0, 30.0]),
            line("e-7", [80.0, 30.0], [80.0, 50.0]),
            line("e-3", [80.0, 50.0], [0.0, 50.0]),
            line("e-4", [0.0, 50.0], [0.0, 0.0]),
        ];

        assert_eq!(
            successions_of(&entities),
            [Succession {
                entity: id("e-2"),
                now: vec![id("e-2"), id("e-7"), id("e-6")],
            }]
        );
    }

    #[test]
    fn an_extension_chains_through_new_entities_on_the_same_line() {
        let entities = [
            line("e-1", [0.0, 0.0], [80.0, 0.0]),
            line("e-2", [80.0, 0.0], [80.0, 20.0]),
            line("e-6", [80.0, 20.0], [80.0, 30.0]),
            line("e-7", [80.0, 30.0], [80.0, 50.0]),
            line("e-3", [80.0, 50.0], [0.0, 50.0]),
            line("e-4", [0.0, 50.0], [0.0, 0.0]),
        ];

        assert_eq!(
            successions_of(&entities),
            [Succession {
                entity: id("e-2"),
                now: vec![id("e-2"), id("e-6"), id("e-7")],
            }]
        );
    }

    #[test]
    fn a_replacement_by_lines_on_one_line_is_not_also_an_extension() {
        let entities = [
            line("e-1", [0.0, 0.0], [80.0, 0.0]),
            line("e-8", [80.0, 0.0], [80.0, 25.0]),
            line("e-9", [80.0, 25.0], [80.0, 50.0]),
            line("e-3", [80.0, 50.0], [0.0, 50.0]),
            line("e-4", [0.0, 50.0], [0.0, 0.0]),
        ];

        assert_eq!(
            successions_of(&entities),
            [Succession {
                entity: id("e-2"),
                now: vec![id("e-8"), id("e-9")],
            }]
        );
    }

    /// The successions of the rectangle with a circle `e-5` across its top line `e-3`, after the right piece of
    /// `e-3` is redrawn as `right` and the left piece as `left`.
    fn successions_of_redrawn_pieces(right: &str, left: &str) -> Vec<Succession> {
        let bite = circle("e-5", [40.0, 50.0], 10.0);
        let mut stored = rectangle();
        stored.push(bite.clone());
        assert_eq!(
            build_profile(&stored).regions[0].outline().to_string(),
            "e-1,e-2,e-3,~e-5,e-3,e-4"
        );
        let mut entities = rectangle();
        entities.retain(|entity| entity.id.as_str() != "e-3");
        entities.extend([
            bite,
            line(right, [80.0, 50.0], [50.0, 50.0]),
            line(left, [30.0, 50.0], [0.0, 50.0]),
        ]);

        successions_after(&stored, &entities)
    }

    #[test]
    fn the_replaced_pieces_of_an_interrupted_line_form_one_succession() {
        assert_eq!(
            successions_of_redrawn_pieces("e-8", "e-9"),
            [Succession {
                entity: id("e-3"),
                now: vec![id("e-8"), id("e-9")],
            }]
        );
    }

    #[test]
    fn the_replaced_pieces_follow_the_loop_rather_than_their_ids() {
        assert_eq!(
            successions_of_redrawn_pieces("e-9", "e-8"),
            [Succession {
                entity: id("e-3"),
                now: vec![id("e-9"), id("e-8")],
            }]
        );
    }

    #[test]
    fn a_region_without_outer_edges_has_no_successions() {
        let regions = [edgeless_region("e-1,e-2", 100.0, [0.0, 0.0])];
        let reference = RegionRef {
            key: "e-1,e-3".parse().expect("valid key"),
            fp: "c=0,0|a=100".parse().expect("valid fingerprint"),
            outline: Some("e-1,e-3".parse().expect("valid loop")),
        };

        assert_eq!(
            match_region(&regions, &[id("e-1"), id("e-2")], &reference),
            RegionMatch::Resolved {
                indices: vec![0],
                method: MatchMethod::Boundary,
                successions: Vec::new(),
            }
        );
    }

    #[test]
    fn run_between_wraps_around() {
        let names = [id("e-1"), id("e-2"), id("e-3"), id("e-4")];
        let sequence: Vec<&Id> = names.iter().collect();

        assert_eq!(
            run_between(&sequence, 2, 1).collect::<Vec<_>>(),
            [&names[3], &names[0]]
        );
    }

    #[test]
    fn run_between_one_position_and_itself_is_every_other_entry() {
        let names = [id("e-1"), id("e-2"), id("e-3")];
        let sequence: Vec<&Id> = names.iter().collect();

        assert_eq!(
            run_between(&sequence, 1, 1).collect::<Vec<_>>(),
            [&names[2], &names[0]]
        );
    }

    #[test]
    fn distinct_keeps_the_first_occurrence() {
        let names = [id("e-2"), id("e-1")];

        assert_eq!(
            distinct([&names[0], &names[1], &names[0]]),
            [id("e-2"), id("e-1")]
        );
    }
}
