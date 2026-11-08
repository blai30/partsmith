//! Chain keys, fingerprints and stored chain references (spec sections 2.10, 3.18.1 and 3.18.5).

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::SketchError;
use crate::number::Rounded;
use crate::profile::{KeyEdge, decimal, loop_text, parse_entries};

/// The pattern of a chain key text: entries separated by ",".
const CHAIN_KEY_PATTERN: &str = r"^~?[A-Za-z0-9_-]{1,64}(,~?[A-Za-z0-9_-]{1,64})*$";

/// The pattern of a chain fingerprint text.
const CHAIN_FINGERPRINT_PATTERN: &str = r"^s=-?[0-9]+(\.[0-9]+)?,-?[0-9]+(\.[0-9]+)?\|e=-?[0-9]+(\.[0-9]+)?,-?[0-9]+(\.[0-9]+)?\|l=[0-9]+(\.[0-9]+)?$";

/// An ordered chain of sketch curves, written like "e-1,~e-2,e-3" (section 3.18.1).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChainKey {
    /// The entries in chain order, each entity once.
    entries: Vec<KeyEdge>,
}

impl ChainKey {
    /// A key from entries in chain order; fails with `InvalidChainKey` when `entries` is empty or names an entity
    /// twice.
    pub fn new(entries: Vec<KeyEdge>) -> Result<ChainKey, SketchError> {
        let mut entities = BTreeSet::new();
        if entries.is_empty() || !entries.iter().all(|entry| entities.insert(&entry.entity)) {
            return Err(SketchError::InvalidChainKey {
                text: loop_text(&entries),
            });
        }
        Ok(ChainKey { entries })
    }

    /// The entries in chain order.
    pub fn entries(&self) -> &[KeyEdge] {
        &self.entries
    }

    /// The same curves run the other way: the entries in reverse order, each with its `reversed` flag flipped.
    pub fn reversed(&self) -> ChainKey {
        ChainKey {
            entries: self
                .entries
                .iter()
                .rev()
                .map(|entry| KeyEdge {
                    entity: entry.entity.clone(),
                    reversed: !entry.reversed,
                })
                .collect(),
        }
    }
}

impl fmt::Display for ChainKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&loop_text(&self.entries))
    }
}

impl FromStr for ChainKey {
    type Err = SketchError;

    fn from_str(text: &str) -> Result<ChainKey, SketchError> {
        parse_entries(text)
            .and_then(|entries| ChainKey::new(entries).ok())
            .ok_or_else(|| SketchError::InvalidChainKey {
                text: text.to_owned(),
            })
    }
}

impl Serialize for ChainKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ChainKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<ChainKey, D::Error> {
        Cow::<str>::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

impl JsonSchema for ChainKey {
    fn schema_name() -> Cow<'static, str> {
        "ChainKey".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::ChainKey").into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({"type": "string", "pattern": CHAIN_KEY_PATTERN})
    }
}

/// Start, end and length of a chain, written "s=<u>,<v>|e=<u>,<v>|l=<length>" (section 3.18.5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChainFingerprint {
    /// The start in sketch coordinates.
    pub start: [f64; 2],
    /// The end in sketch coordinates; the start for a closed chain.
    pub end: [f64; 2],
    /// The length in mm.
    pub length: f64,
}

impl fmt::Display for ChainFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "s={},{}|e={},{}|l={}",
            Rounded(self.start[0]),
            Rounded(self.start[1]),
            Rounded(self.end[0]),
            Rounded(self.end[1]),
            Rounded(self.length)
        )
    }
}

impl FromStr for ChainFingerprint {
    type Err = SketchError;

    /// Accepts the grammar of `CHAIN_FINGERPRINT_PATTERN`: plain decimals, a sign only on the coordinates.
    fn from_str(text: &str) -> Result<ChainFingerprint, SketchError> {
        let point = |text: &str| -> Option<[f64; 2]> {
            let (u, v) = text.split_once(',')?;
            Some([decimal(u, true)?, decimal(v, true)?])
        };
        let parse = || -> Option<ChainFingerprint> {
            let (start, rest) = text.strip_prefix("s=")?.split_once("|e=")?;
            let (end, length) = rest.split_once("|l=")?;
            Some(ChainFingerprint {
                start: point(start)?,
                end: point(end)?,
                length: decimal(length, false)?,
            })
        };
        parse().ok_or_else(|| SketchError::InvalidChainFingerprint {
            text: text.to_owned(),
        })
    }
}

impl Serialize for ChainFingerprint {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ChainFingerprint {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<ChainFingerprint, D::Error> {
        Cow::<str>::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

impl JsonSchema for ChainFingerprint {
    fn schema_name() -> Cow<'static, str> {
        "ChainFingerprint".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::ChainFingerprint").into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({"type": "string", "pattern": CHAIN_FINGERPRINT_PATTERN})
    }
}

/// A stored reference to a chain of a sketch (section 3.18.5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChainRef {
    /// The chain's key.
    pub key: ChainKey,
    /// The chain's fingerprint when the reference was made.
    pub fp: ChainFingerprint,
    /// True when the chain was closed when the reference was made. Omitted from JSON when false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub closed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::key_edge;

    #[test]
    fn a_key_naming_an_entity_twice_keeps_the_text_of_its_entries() {
        let error = ChainKey::new(vec![
            key_edge("e-2", false),
            key_edge("e-1", true),
            key_edge("e-2", true),
        ]);

        assert_eq!(
            error,
            Err(SketchError::InvalidChainKey {
                text: "e-2,~e-1,~e-2".to_owned()
            })
        );
    }

    #[test]
    fn a_fingerprint_with_an_exponent_or_a_signed_length_fails() {
        for text in ["s=1e3,0|e=0,0|l=1", "s=0,0|e=0,0|l=+1", "s=0,0|e=0|l=1"] {
            assert!(text.parse::<ChainFingerprint>().is_err(), "{text}");
        }
    }
}
