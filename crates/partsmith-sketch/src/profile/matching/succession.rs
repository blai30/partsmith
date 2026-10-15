//! Successions of a `Boundary` match: which entities now carry the place of a stored boundary entity (spec
//! section 3.14).

use partsmith_model::Id;

use super::super::{ProfileEdge, Region, RegionLoop};
use super::{Stored, Succession, lies_on};

/// The successions of the one region a stored reference matched by boundary, sorted by entity: replacements
/// first (only with a stored outline), then extensions among the new entities no replacement uses.
pub(super) fn successions(region: &Region, stored: &Stored<'_>) -> Vec<Succession> {
    // A hand-built region can have an empty outer loop, which has no entities to follow.
    let Some(outline) = region.try_outline() else {
        return Vec::new();
    };
    let current = entities_of(&outline);
    let mut found = match &stored.reference.outline {
        Some(previous) => replacements(&current, &entities_of(previous), stored),
        // The stored key does not say where a removed entity was, so nothing tells a replacement from a
        // deletion plus an unrelated addition.
        None => Vec::new(),
    };
    let replacing: Vec<&Id> = found
        .iter()
        .flat_map(|succession| &succession.now)
        .collect();
    let extensions = extensions(region, &current, stored, &replacing);
    found.extend(extensions);
    found.sort_by(|first, second| first.entity.cmp(&second.entity));
    found
}

/// Each removed entity that is the only entry between two consecutive surviving entities of the stored outline,
/// where the current loop has only new entities between them. The runs that replace the pieces of one removed
/// entity (a line another curve interrupted) form its one succession, in the order of the current loop.
fn replacements(current: &[&Id], previous: &[&Id], stored: &Stored<'_>) -> Vec<Succession> {
    let survivors: Vec<usize> = (0..current.len())
        .filter(|&position| !stored.is_added(current[position]))
        .collect();
    // An entity that occurs twice, such as a line another curve interrupts, leaves the runs undetermined.
    let once = |sequence: &[&Id], entity: &Id| occurrences(sequence, entity).count() == 1;
    let mut found: Vec<(&Id, Vec<&Id>)> = Vec::new();
    for (rank, &position) in survivors.iter().enumerate() {
        let next = survivors[(rank + 1) % survivors.len()];
        let (first, second) = (current[position], current[next]);
        if !(once(current, first) && once(current, second))
            || !(once(previous, first) && once(previous, second))
        {
            continue;
        }
        let new_run = run_between(current, position, next);
        let old_run = run_between(
            previous,
            position_of(previous, first),
            position_of(previous, second),
        );
        let [removed] = old_run.as_slice() else {
            continue;
        };
        let is_removed = !current.contains(removed);
        let all_new = !new_run.is_empty() && new_run.iter().all(|entity| stored.is_added(entity));
        if !(is_removed && all_new) {
            continue;
        }
        match found.iter_mut().find(|(entity, _)| entity == removed) {
            Some((_, now)) => now.extend(new_run),
            None => found.push((*removed, new_run)),
        }
    }
    found
        .into_iter()
        .map(|(entity, now)| Succession {
            entity: entity.clone(),
            now: distinct(now),
        })
        .collect()
}

/// Each surviving entity that new entities next to it continue on its line or circle, with those entities in
/// traversal order starting after it.
fn extensions(
    region: &Region,
    current: &[&Id],
    stored: &Stored<'_>,
    replacing: &[&Id],
) -> Vec<Succession> {
    let count = current.len();
    let mut found = Vec::new();
    let mut seen: Vec<&Id> = Vec::new();
    for (first_position, &survivor) in current.iter().enumerate() {
        if stored.is_added(survivor) || seen.contains(&survivor) {
            continue;
        }
        seen.push(survivor);
        let Some(carrier) = edges_of(region, survivor).next() else {
            continue;
        };
        let continues = |entity: &Id| {
            stored.is_added(entity)
                && !replacing.contains(&entity)
                && edges_of(region, entity).all(|edge| lies_on(&edge.curve, &carrier.curve))
        };
        let mut chained: Vec<usize> = Vec::new();
        for position in occurrences(current, survivor) {
            for step in [1, count - 1] {
                let mut next = (position + step) % count;
                while next != position && continues(current[next]) && !chained.contains(&next) {
                    chained.push(next);
                    next = (next + step) % count;
                }
            }
        }
        if chained.is_empty() {
            continue;
        }
        chained.sort_by_key(|&position| (position + count - first_position) % count);
        let mut now = vec![survivor];
        now.extend(chained.into_iter().map(|position| current[position]));
        found.push(Succession {
            entity: survivor.clone(),
            now: distinct(now),
        });
    }
    found
}

/// The entities of a loop's entries, in its order.
fn entities_of(region_loop: &RegionLoop) -> Vec<&Id> {
    region_loop
        .entries()
        .iter()
        .map(|entry| &entry.entity)
        .collect()
}

/// The entries strictly between positions `from` and `to` of a cyclic sequence; every other entry when they are
/// the same position.
fn run_between<'a>(sequence: &[&'a Id], from: usize, to: usize) -> Vec<&'a Id> {
    let count = sequence.len();
    (1..count)
        .map(|offset| (from + offset) % count)
        .take_while(|&position| position != to)
        .map(|position| sequence[position])
        .collect()
}

/// The positions where `entity` occurs in `sequence`.
fn occurrences<'a>(sequence: &'a [&Id], entity: &'a Id) -> impl Iterator<Item = usize> + 'a {
    (0..sequence.len()).filter(move |&position| sequence[position] == entity)
}

/// The position of an entity that occurs in `sequence`.
fn position_of(sequence: &[&Id], entity: &Id) -> usize {
    occurrences(sequence, entity)
        .next()
        .expect("the entity occurs in the sequence")
}

/// The edges of an entity in the region's outer loop.
fn edges_of<'a>(region: &'a Region, entity: &'a Id) -> impl Iterator<Item = &'a ProfileEdge> {
    region
        .outer
        .edges
        .iter()
        .filter(move |edge| &edge.entity == entity)
}

/// The entities in order, each once, as owned ids.
fn distinct(entities: Vec<&Id>) -> Vec<Id> {
    let mut unique: Vec<Id> = Vec::with_capacity(entities.len());
    for entity in entities {
        if !unique.contains(entity) {
            unique.push(entity.clone());
        }
    }
    unique
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::SketchEntity;
    use crate::profile::{
        MatchMethod, ProfileLoop, RegionMatch, RegionRef, build_profile, match_region,
    };
    use crate::test_support::{circle, id, line};

    /// The successions of the rectangle `e-1` to `e-4` (80 x 50) matched among the regions of `entities`.
    fn successions_of(entities: &[SketchEntity]) -> Vec<Succession> {
        let rectangle = [
            line("e-1", [0.0, 0.0], [80.0, 0.0]),
            line("e-2", [80.0, 0.0], [80.0, 50.0]),
            line("e-3", [80.0, 50.0], [0.0, 50.0]),
            line("e-4", [0.0, 50.0], [0.0, 0.0]),
        ];
        let reference = build_profile(&rectangle).regions[0].reference();
        let ids: Vec<Id> = entities.iter().map(|entity| entity.id.clone()).collect();
        match match_region(&build_profile(entities).regions, &ids, &reference) {
            RegionMatch::Resolved { successions, .. } => successions,
            other => panic!("the rectangle resolves, found {other:?}"),
        }
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

    #[test]
    fn the_replaced_pieces_of_an_interrupted_line_form_one_succession() {
        let stored = [
            line("e-1", [0.0, 0.0], [80.0, 0.0]),
            line("e-2", [80.0, 0.0], [80.0, 50.0]),
            line("e-3", [80.0, 50.0], [0.0, 50.0]),
            line("e-4", [0.0, 50.0], [0.0, 0.0]),
            circle("e-5", [40.0, 50.0], 10.0),
        ];
        let reference = build_profile(&stored).regions[0].reference();
        assert_eq!(
            reference.outline,
            Some("e-1,e-2,e-3,~e-5,e-3,e-4".parse().expect("valid loop"))
        );
        let entities = [
            line("e-1", [0.0, 0.0], [80.0, 0.0]),
            line("e-2", [80.0, 0.0], [80.0, 50.0]),
            line("e-4", [0.0, 50.0], [0.0, 0.0]),
            circle("e-5", [40.0, 50.0], 10.0),
            line("e-8", [80.0, 50.0], [50.0, 50.0]),
            line("e-9", [30.0, 50.0], [0.0, 50.0]),
        ];
        let regions = build_profile(&entities).regions;
        let ids: Vec<Id> = entities.iter().map(|entity| entity.id.clone()).collect();

        let RegionMatch::Resolved { successions, .. } = match_region(&regions, &ids, &reference)
        else {
            panic!("the region resolves");
        };

        assert_eq!(
            successions,
            [Succession {
                entity: id("e-3"),
                now: vec![id("e-8"), id("e-9")],
            }]
        );
    }

    #[test]
    fn a_region_without_outer_edges_has_no_successions() {
        let regions = [Region {
            key: "e-1,e-2".parse().expect("valid key"),
            outer: ProfileLoop { edges: Vec::new() },
            holes: Vec::new(),
            area: 100.0,
            centroid: [0.0, 0.0],
        }];
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

        assert_eq!(run_between(&sequence, 2, 1), [&names[3], &names[0]]);
    }

    #[test]
    fn run_between_one_position_and_itself_is_every_other_entry() {
        let names = [id("e-1"), id("e-2"), id("e-3")];
        let sequence: Vec<&Id> = names.iter().collect();

        assert_eq!(run_between(&sequence, 1, 1), [&names[2], &names[0]]);
    }

    #[test]
    fn distinct_keeps_the_first_occurrence() {
        let names = [id("e-2"), id("e-1")];

        assert_eq!(
            distinct(vec![&names[0], &names[1], &names[0]]),
            [id("e-2"), id("e-1")]
        );
    }
}
