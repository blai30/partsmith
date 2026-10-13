//! The motions of a naming map: a base motion and motions of their own per face name (naming.md
//! section 3.3, step 8).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::NamingError;
use crate::motion::Motion;
use crate::name::ElementName;
use crate::shape::piece_free_text;

/// How far Move body features and copy transforms moved the faces of a map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Motions {
    /// The motion of every name without one of its own: the body as a whole.
    pub(crate) base: Motion,
    /// Motions of their own, by full or piece-free face name.
    pub(crate) own: BTreeMap<ElementName, Motion>,
}

impl Default for Motions {
    fn default() -> Motions {
        Motions::of_base(Motion::IDENTITY)
    }
}

impl Motions {
    /// Motions with this base and nothing of their own.
    pub(crate) fn of_base(base: Motion) -> Motions {
        Motions {
            base,
            own: BTreeMap::new(),
        }
    }

    /// Motions with this base and the motions `named` (full names), written canonically: per
    /// piece-free name, one entry under it when every name with it has the same motion, else one
    /// entry per full name; entries equal to the base are left out.
    pub(crate) fn canonical(base: Motion, named: BTreeMap<ElementName, Motion>) -> Motions {
        let mut by_piece_free: BTreeMap<String, Vec<(ElementName, Motion)>> = BTreeMap::new();
        for (name, motion) in named {
            let piece_free = piece_free_text(name.shared_text()).to_string();
            by_piece_free
                .entry(piece_free)
                .or_default()
                .push((name, motion));
        }
        let mut own = BTreeMap::new();
        for (piece_free, names) in by_piece_free {
            let first = names[0].1;
            let shared = names.iter().all(|(_, motion)| *motion == first);
            match ElementName::parse(&piece_free) {
                Ok(key) if shared => {
                    if first != base {
                        own.insert(key, first);
                    }
                }
                _ => own.extend(names.into_iter().filter(|(_, motion)| *motion != base)),
            }
        }
        Motions { base, own }
    }

    /// The motion of a face name: its own under the full name, else under its piece-free name,
    /// else the base.
    pub(crate) fn of(&self, face: &ElementName) -> Motion {
        if let Some(motion) = self.own.get(face) {
            return *motion;
        }
        face.piece_free()
            .and_then(|piece_free| self.own.get(&piece_free))
            .copied()
            .unwrap_or(self.base)
    }

    /// True when the base is the identity and no name has a motion of its own.
    pub(crate) fn is_identity(&self) -> bool {
        self.base == Motion::IDENTITY && self.own.is_empty()
    }

    /// The same motions with `after` applied after each.
    pub(crate) fn then(&self, after: &Motion) -> Motions {
        Motions {
            base: self.base.then(after),
            own: self
                .own
                .iter()
                .map(|(name, motion)| (name.clone(), motion.then(after)))
                .collect(),
        }
    }

    /// The same motions with every name renamed by `rename`.
    pub(crate) fn renamed(
        &self,
        rename: impl Fn(&ElementName) -> Result<ElementName, NamingError>,
    ) -> Result<Motions, NamingError> {
        Ok(Motions {
            base: self.base,
            own: self
                .own
                .iter()
                .map(|(name, motion)| Ok((rename(name)?, *motion)))
                .collect::<Result<_, NamingError>>()?,
        })
    }
}
