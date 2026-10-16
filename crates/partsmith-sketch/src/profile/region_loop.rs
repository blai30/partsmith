//! Region loops: a region's outer boundary as a cyclic sequence of key entries (spec section 3.13.8).

use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::key::{KeyEdge, loop_text, parse_entries};
use crate::error::SketchError;

/// The pattern of a region loop text: entries separated by ",".
const REGION_LOOP_PATTERN: &str = r"^~?[A-Za-z0-9_-]{1,64}(,~?[A-Za-z0-9_-]{1,64})*$";

/// An outer loop as a cyclic sequence of key entries in traversal order, written like "e-1,e-2,~e-5,e-3"
/// (section 3.13.8).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegionLoop {
    entries: Vec<KeyEdge>,
}

impl RegionLoop {
    /// Builds a canonical loop (section 3.13.8); fails with `InvalidRegionKey { text: "" }` when `entries` is empty.
    pub fn new(mut entries: Vec<KeyEdge>) -> Result<RegionLoop, SketchError> {
        entries.dedup();
        // The sequence is cyclic, so the last entry follows the first.
        while entries.len() > 1 && entries.first() == entries.last() {
            entries.pop();
        }
        if entries.is_empty() {
            return Err(SketchError::InvalidRegionKey {
                text: String::new(),
            });
        }
        let start = smallest_rotation(&entries);
        entries.rotate_left(start);
        Ok(RegionLoop { entries })
    }

    /// The entries in traversal order, starting where the canonical text starts.
    pub fn entries(&self) -> &[KeyEdge] {
        &self.entries
    }
}

/// The start of the rotation whose text is the smallest in byte order; the first such start on ties.
fn smallest_rotation(entries: &[KeyEdge]) -> usize {
    (1..entries.len()).fold(0, |best, start| {
        if compare_rotations(entries, start, best) == Ordering::Less {
            start
        } else {
            best
        }
    })
}

/// Compares the texts of the rotations of `entries` that start at `first` and at `second`.
///
/// Every id character sorts after the "," that separates entries and before the "~" that marks a reversed
/// entry, so comparing entry by entry, unreversed before reversed and then by id, orders the texts.
fn compare_rotations(entries: &[KeyEdge], first: usize, second: usize) -> Ordering {
    let count = entries.len();
    (0..count)
        .map(|offset| {
            let first_entry = &entries[(first + offset) % count];
            let second_entry = &entries[(second + offset) % count];
            (first_entry.reversed, &first_entry.entity)
                .cmp(&(second_entry.reversed, &second_entry.entity))
        })
        .find(|order| order.is_ne())
        .unwrap_or(Ordering::Equal)
}

impl fmt::Display for RegionLoop {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&loop_text(&self.entries))
    }
}

impl FromStr for RegionLoop {
    type Err = SketchError;

    /// Accepts any non-empty comma-separated entries and canonicalizes them.
    fn from_str(text: &str) -> Result<RegionLoop, SketchError> {
        let invalid = || SketchError::InvalidRegionKey {
            text: text.to_owned(),
        };
        let entries = parse_entries(text).ok_or_else(invalid)?;
        RegionLoop::new(entries).map_err(|_| invalid())
    }
}

impl Serialize for RegionLoop {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RegionLoop {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<RegionLoop, D::Error> {
        Cow::<str>::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

impl JsonSchema for RegionLoop {
    fn schema_name() -> Cow<'static, str> {
        "RegionLoop".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::RegionLoop").into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({"type": "string", "pattern": REGION_LOOP_PATTERN})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::key_edge;

    #[test]
    fn reversed_entries_sort_after_every_unreversed_one() {
        let region_loop = RegionLoop::new(vec![key_edge("e-1", true), key_edge("e-2", false)])
            .expect("two entries form a loop");

        assert_eq!(region_loop.to_string(), "e-2,~e-1");
    }

    #[test]
    fn a_shorter_id_sorts_before_a_longer_one_it_starts() {
        let region_loop = RegionLoop::new(vec![key_edge("e-1-a", false), key_edge("e-1", false)])
            .expect("two entries form a loop");

        assert_eq!(region_loop.to_string(), "e-1,e-1-a");
    }

    #[test]
    fn one_entity_repeated_all_the_way_round_is_one_entry() {
        let region_loop = RegionLoop::new(vec![key_edge("e-4", false); 3]).expect("a loop");

        assert_eq!(region_loop.entries(), [key_edge("e-4", false)]);
    }

    #[test]
    fn a_periodic_loop_keeps_its_period() {
        let region_loop: RegionLoop = "e-2,e-1,e-2,e-1".parse().expect("a valid loop");

        assert_eq!(region_loop.to_string(), "e-1,e-2,e-1,e-2");
    }

    #[test]
    fn an_entry_with_two_markers_is_invalid() {
        assert_eq!(
            "e-1,~~e-2".parse::<RegionLoop>(),
            Err(SketchError::InvalidRegionKey {
                text: "e-1,~~e-2".to_owned()
            })
        );
    }
}
