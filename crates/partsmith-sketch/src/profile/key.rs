//! Region keys, fingerprints and stored region references (spec sections 3.13.5 and 3.13.6).

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use partsmith_model::Id;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::SketchError;
use crate::number::Rounded;

/// The pattern of a region key text: loops of entries separated by ";", entries separated by ",".
const REGION_KEY_PATTERN: &str = r"^~?[A-Za-z0-9_-]{1,64}(,~?[A-Za-z0-9_-]{1,64})*(;~?[A-Za-z0-9_-]{1,64}(,~?[A-Za-z0-9_-]{1,64})*)*$";

/// The pattern of a region fingerprint text.
const FINGERPRINT_PATTERN: &str =
    r"^c=-?[0-9]+(\.[0-9]+)?,-?[0-9]+(\.[0-9]+)?\|a=[0-9]+(\.[0-9]+)?$";

/// Canonical region key, written like "e-1,e-2,~e-5;~e-9" (section 3.13.5).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegionKey {
    outer: Vec<KeyEdge>,
    holes: Vec<Vec<KeyEdge>>,
}

/// One entry of a region key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyEdge {
    /// Boundary entity.
    pub entity: Id,
    /// True when the boundary runs against the entity's direction; written with a "~" prefix.
    pub reversed: bool,
}

impl RegionKey {
    /// Builds a canonical key: sorts and deduplicates each loop's entries and sorts the holes.
    pub fn new(outer: Vec<KeyEdge>, holes: Vec<Vec<KeyEdge>>) -> RegionKey {
        let canonical = |mut entries: Vec<KeyEdge>| {
            entries.sort();
            entries.dedup();
            entries
        };
        let mut holes: Vec<Vec<KeyEdge>> = holes.into_iter().map(canonical).collect();
        holes.sort_by_cached_key(|hole| loop_text(hole));
        RegionKey {
            outer: canonical(outer),
            holes,
        }
    }

    /// Entries of the outer loop, sorted.
    pub fn outer(&self) -> &[KeyEdge] {
        &self.outer
    }

    /// Entries of each hole loop, holes sorted by text.
    pub fn holes(&self) -> &[Vec<KeyEdge>] {
        &self.holes
    }
}

impl fmt::Display for KeyEdge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.reversed {
            formatter.write_str("~")?;
        }
        write!(formatter, "{}", self.entity)
    }
}

/// The text of one loop: its entries joined with ",".
pub(crate) fn loop_text(entries: &[KeyEdge]) -> String {
    entries
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<String>>()
        .join(",")
}

impl fmt::Display for RegionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&loop_text(&self.outer))?;
        for hole in &self.holes {
            write!(formatter, ";{}", loop_text(hole))?;
        }
        Ok(())
    }
}

impl FromStr for RegionKey {
    type Err = SketchError;

    fn from_str(text: &str) -> Result<RegionKey, SketchError> {
        let invalid = || SketchError::InvalidRegionKey {
            text: text.to_owned(),
        };
        let pieces: Vec<&str> = text.split(';').collect();
        let mut loops = pieces
            .iter()
            .map(|piece| parse_loop(piece))
            .collect::<Option<Vec<Vec<KeyEdge>>>>()
            .ok_or_else(invalid)?;
        // Canonical loops print as written, so the hole order can be checked on the pieces themselves.
        if pieces[1..].windows(2).any(|pair| pair[0] > pair[1]) {
            return Err(invalid());
        }
        let outer = loops.remove(0);
        Ok(RegionKey {
            outer,
            holes: loops,
        })
    }
}

/// One canonical loop: non-empty, valid ids, at most one "~" per entry, entries strictly ascending.
fn parse_loop(text: &str) -> Option<Vec<KeyEdge>> {
    let mut entries = Vec::new();
    for entry in text.split(',') {
        let (name, reversed) = match entry.strip_prefix('~') {
            Some(name) => (name, true),
            None => (entry, false),
        };
        let entity = Id::new(name).ok()?;
        entries.push(KeyEdge { entity, reversed });
    }
    entries
        .windows(2)
        .all(|pair| pair[0] < pair[1])
        .then_some(entries)
}

impl Serialize for RegionKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RegionKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<RegionKey, D::Error> {
        Cow::<str>::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

impl JsonSchema for RegionKey {
    fn schema_name() -> Cow<'static, str> {
        "RegionKey".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::RegionKey").into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({"type": "string", "pattern": REGION_KEY_PATTERN})
    }
}

/// Area and centroid of a region, written "c=<x>,<y>|a=<area>".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionFingerprint {
    /// Area in mm^2.
    pub area: f64,
    /// Centroid in sketch coordinates.
    pub centroid: [f64; 2],
}

impl fmt::Display for RegionFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "c={},{}|a={}",
            Rounded(self.centroid[0]),
            Rounded(self.centroid[1]),
            Rounded(self.area)
        )
    }
}

impl FromStr for RegionFingerprint {
    type Err = SketchError;

    fn from_str(text: &str) -> Result<RegionFingerprint, SketchError> {
        let parse = || -> Option<RegionFingerprint> {
            let (centroid, area) = text.strip_prefix("c=")?.split_once("|a=")?;
            let (u, v) = centroid.split_once(',')?;
            let number = |part: &str| part.parse::<f64>().ok().filter(|value| value.is_finite());
            let fingerprint = RegionFingerprint {
                area: number(area)?,
                centroid: [number(u)?, number(v)?],
            };
            (fingerprint.area >= 0.0).then_some(fingerprint)
        };
        parse().ok_or_else(|| SketchError::InvalidFingerprint {
            text: text.to_owned(),
        })
    }
}

impl Serialize for RegionFingerprint {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RegionFingerprint {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<RegionFingerprint, D::Error> {
        Cow::<str>::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

impl JsonSchema for RegionFingerprint {
    fn schema_name() -> Cow<'static, str> {
        "RegionFingerprint".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::RegionFingerprint").into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({"type": "string", "pattern": FINGERPRINT_PATTERN})
    }
}

/// A stored reference to a region of a sketch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegionRef {
    /// The region's key.
    pub key: RegionKey,
    /// The region's fingerprint when the reference was made.
    pub fp: RegionFingerprint,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(entity: &str, reversed: bool) -> KeyEdge {
        KeyEdge {
            entity: Id::new(entity).expect("valid id"),
            reversed,
        }
    }

    #[test]
    fn holes_sort_by_text_not_by_entries() {
        // By entries (e-1, true) sorts before (e-2, false); by text "e-2" sorts before "~e-1".
        let key = RegionKey::new(
            vec![entry("e-9", false)],
            vec![vec![entry("e-1", true)], vec![entry("e-2", false)]],
        );

        assert_eq!(key.to_string(), "e-9;e-2;~e-1");
    }

    #[test]
    fn equal_holes_are_accepted() {
        let key: RegionKey = "e-1;~e-2;~e-2"
            .parse()
            .expect("non-decreasing holes are canonical");

        assert_eq!(key.holes().len(), 2);
    }

    #[test]
    fn fingerprint_with_a_missing_part_fails() {
        assert!("c=1,2|a=".parse::<RegionFingerprint>().is_err());
        assert!("c=1|a=1".parse::<RegionFingerprint>().is_err());
    }

    #[test]
    fn fingerprint_text_matches_its_schema_pattern_shape() {
        let fingerprint = RegionFingerprint {
            area: 1e20,
            centroid: [-1e-3, 2.5],
        };

        assert_eq!(
            fingerprint.to_string(),
            "c=-0.001,2.5|a=100000000000000000000"
        );
    }
}
