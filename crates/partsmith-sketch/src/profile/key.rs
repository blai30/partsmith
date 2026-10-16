//! Region keys, fingerprints and stored region references (spec sections 2.7, 3.13.5 and 3.13.6).

use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use partsmith_model::Id;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::hashing::FastHasher;
use super::region_loop::RegionLoop;
use crate::error::SketchError;
use crate::number::Rounded;

/// The pattern of a region key text: loops of entries separated by ";", entries separated by ",".
const REGION_KEY_PATTERN: &str = r"^~?[A-Za-z0-9_-]{1,64}(,~?[A-Za-z0-9_-]{1,64})*(;~?[A-Za-z0-9_-]{1,64}(,~?[A-Za-z0-9_-]{1,64})*)*$";

/// The pattern of a region fingerprint text.
const FINGERPRINT_PATTERN: &str =
    r"^c=-?[0-9]+(\.[0-9]+)?,-?[0-9]+(\.[0-9]+)?\|a=[0-9]+(\.[0-9]+)?$";

/// Canonical region key, written like "e-1,e-2,~e-5;~e-9" (section 3.13.5).
#[derive(Clone)]
pub struct RegionKey {
    outer: Vec<KeyEdge>,
    holes: Vec<Vec<KeyEdge>>,
    /// A hash of the entries kept beside them, so that two different keys almost always differ without
    /// reading their entries: region matching compares a stored key with every region's (section 5.1).
    digest: u64,
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
    pub fn new(outer: Vec<KeyEdge>, mut holes: Vec<Vec<KeyEdge>>) -> RegionKey {
        sort_holes(&mut holes, Vec::clone);
        RegionKey::from_canonical(
            canonical_loop(outer),
            holes.into_iter().map(canonical_loop).collect(),
        )
    }

    /// The key of loops that are already canonical.
    fn from_canonical(outer: Vec<KeyEdge>, holes: Vec<Vec<KeyEdge>>) -> RegionKey {
        let digest = FastHasher::of(&(&outer, &holes));
        RegionKey {
            outer,
            holes,
            digest,
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

impl PartialEq for RegionKey {
    fn eq(&self, other: &RegionKey) -> bool {
        self.digest == other.digest && self.outer == other.outer && self.holes == other.holes
    }
}

impl Eq for RegionKey {}

impl PartialOrd for RegionKey {
    fn partial_cmp(&self, other: &RegionKey) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RegionKey {
    /// Orders by the outer loop's entries, then the holes'.
    fn cmp(&self, other: &RegionKey) -> Ordering {
        (&self.outer, &self.holes).cmp(&(&other.outer, &other.holes))
    }
}

impl Hash for RegionKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.digest.hash(state);
    }
}

impl fmt::Debug for RegionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RegionKey")
            .field(&self.to_string())
            .finish()
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

/// A loop's entries as a region key lists them: sorted, without repeats.
pub(crate) fn canonical_loop(mut entries: Vec<KeyEdge>) -> Vec<KeyEdge> {
    entries.sort();
    entries.dedup();
    entries
}

/// Sorts holes into region key order, by the text of their canonical loops; holes with the same text
/// keep their order.
pub(crate) fn sort_holes<T>(holes: &mut [T], entries: impl Fn(&T) -> Vec<KeyEdge>) {
    holes.sort_by_cached_key(|hole| loop_text(&canonical_loop(entries(hole))));
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
        Ok(RegionKey::from_canonical(outer, loops))
    }
}

/// One canonical loop: non-empty, valid entries, entries strictly ascending.
fn parse_loop(text: &str) -> Option<Vec<KeyEdge>> {
    parse_entries(text).filter(|entries| entries.windows(2).all(|pair| pair[0] < pair[1]))
}

/// The entries of a loop text, separated by ",", in written order; `None` when any entry is invalid.
pub(super) fn parse_entries(text: &str) -> Option<Vec<KeyEdge>> {
    text.split(',').map(parse_entry).collect()
}

/// One entry `[~]id`: a valid id with at most one "~" before it.
fn parse_entry(text: &str) -> Option<KeyEdge> {
    let (name, reversed) = match text.strip_prefix('~') {
        Some(name) => (name, true),
        None => (text, false),
    };
    let entity = Id::new(name).ok()?;
    Some(KeyEdge { entity, reversed })
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

    /// Accepts the grammar of `FINGERPRINT_PATTERN`: plain decimals, a sign only on the centroid.
    fn from_str(text: &str) -> Result<RegionFingerprint, SketchError> {
        let parse = || -> Option<RegionFingerprint> {
            let (centroid, area) = text.strip_prefix("c=")?.split_once("|a=")?;
            let (centroid_u, centroid_v) = centroid.split_once(',')?;
            Some(RegionFingerprint {
                area: decimal(area, false)?,
                centroid: [decimal(centroid_u, true)?, decimal(centroid_v, true)?],
            })
        };
        parse().ok_or_else(|| SketchError::InvalidFingerprint {
            text: text.to_owned(),
        })
    }
}

/// The finite value of a plain decimal `-?[0-9]+(\.[0-9]+)?` (the minus only when `signed`), with -0 read as 0.
fn decimal(text: &str, signed: bool) -> Option<f64> {
    let unsigned = match text.strip_prefix('-') {
        Some(rest) if signed => rest,
        Some(_) => return None,
        None => text,
    };
    let (whole, fraction) = match unsigned.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (unsigned, None),
    };
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    if !digits(whole) || !fraction.is_none_or(digits) {
        return None;
    }
    let value: f64 = text.parse().ok()?;
    // Adding 0 turns -0 into 0 and leaves every other value unchanged.
    value.is_finite().then_some(value + 0.0)
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
    /// The region's outer loop in traversal order when the reference was made (section 3.13.8); `None` in references
    /// written before file format version 2. Omitted from JSON when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<RegionLoop>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::key_edge;

    #[test]
    fn holes_sort_by_text_not_by_entries() {
        // By entries (e-1, true) sorts before (e-2, false); by text "e-2" sorts before "~e-1".
        let key = RegionKey::new(
            vec![key_edge("e-9", false)],
            vec![vec![key_edge("e-1", true)], vec![key_edge("e-2", false)]],
        );

        assert_eq!(key.to_string(), "e-9;e-2;~e-1");
    }

    #[test]
    fn built_and_parsed_keys_are_equal_and_hash_equally() {
        let built = RegionKey::new(
            vec![key_edge("e-2", false), key_edge("e-1", true)],
            vec![vec![key_edge("e-5", true)]],
        );
        let parsed: RegionKey = "~e-1,e-2;~e-5".parse().expect("a canonical key");

        assert_eq!(built, parsed);
        assert_eq!(FastHasher::of(&built), FastHasher::of(&parsed));
    }

    #[test]
    fn keys_order_by_their_entries() {
        let first: RegionKey = "e-1,e-3".parse().expect("a canonical key");
        let second: RegionKey = "e-2".parse().expect("a canonical key");

        assert!(first < second);
    }

    #[test]
    fn debug_shows_the_key_text() {
        let key: RegionKey = "e-1;~e-2".parse().expect("a canonical key");

        assert_eq!(format!("{key:?}"), r#"RegionKey("e-1;~e-2")"#);
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
    fn fingerprint_numbers_outside_the_plain_decimal_form_fail() {
        for text in [
            "c=1e3,2|a=5",
            "c=1,2|a=+5",
            "c=.5,2|a=5",
            "c=1,2|a=5.",
            "c=1,2|a=-0",
            "c=+1,2|a=5",
            "c=1,2|a=NaN",
            "c=1, 2|a=5",
            "c=--1,2|a=5",
            "c=1,2|a=1.2.3",
        ] {
            assert_eq!(
                text.parse::<RegionFingerprint>(),
                Err(SketchError::InvalidFingerprint {
                    text: text.to_owned()
                }),
                "{text}"
            );
        }
    }

    #[test]
    fn fingerprint_accepts_long_decimals_and_reads_negative_zero_as_zero() {
        let fingerprint: RegionFingerprint = "c=-0,-0.000|a=0.1234567"
            .parse()
            .expect("plain decimals are valid");

        assert_eq!(fingerprint.centroid[0].to_bits(), 0.0_f64.to_bits());
        assert_eq!(fingerprint.centroid[1].to_bits(), 0.0_f64.to_bits());
        assert_eq!(fingerprint.area, 0.1234567);
    }

    #[test]
    fn fingerprint_too_large_to_be_finite_fails() {
        let text = format!("c=0,0|a={}", "9".repeat(400));

        assert!(text.parse::<RegionFingerprint>().is_err());
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
