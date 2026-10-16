//! Step 4 of the resolver (naming.md section 3.9), for edges and vertices: relatives of the same
//! feature and sketch generator.

use std::collections::BTreeSet;

use crate::resolver::parts::FacePart;
use crate::resolver::steps::{Resolver, element_name, followed};
use crate::resolver::{Candidate, Follow, MatchMethod, Resolution};

impl Resolver<'_> {
    /// Step 4 (edges and vertices): relatives of the same feature and sketch generator.
    pub(super) fn related(&self) -> Option<Resolution> {
        let relatives: Vec<Vec<u32>> = self
            .faces
            .iter()
            .map(|part| self.relatives_of(part))
            .collect();
        let absent: Vec<usize> = (0..self.faces.len())
            .filter(|&position| !self.faces[position].is_present())
            .collect();
        match absent.as_slice() {
            [missing] => self.related_to_absent_part(*missing, &relatives[*missing]),
            [] => self.ambiguous_by_relatives(&relatives),
            _ => None,
        }
    }

    /// Step 4 when only the part at `missing` is not present: the candidates that pair with the
    /// other parts and with one of its `relatives`.
    fn related_to_absent_part(&self, missing: usize, relatives: &[u32]) -> Option<Resolution> {
        let allowed: Vec<&[u32]> = self
            .faces
            .iter()
            .enumerate()
            .map(|(position, part)| {
                if position == missing {
                    relatives
                } else {
                    part.family.as_slice()
                }
            })
            .collect();
        match self.set.pairing(self.kind, &allowed).as_slice() {
            [] => None,
            [only] => Some(followed(
                vec![element_name(self.set, *only)],
                MatchMethod::Related,
                Follow::Related,
            )),
            several => Some(Resolution::Ambiguous {
                candidates: self
                    .candidates(several.iter().copied(), None)
                    .into_iter()
                    .map(|candidate| Candidate {
                        meaning: true,
                        ..candidate
                    })
                    .collect(),
            }),
        }
    }

    /// Step 4 when every part is present but no candidate pairs with all of them: the candidates
    /// that keep one part and whose other faces are relatives, given per part in `relatives`.
    fn ambiguous_by_relatives(&self, relatives: &[Vec<u32>]) -> Option<Resolution> {
        let every_relative: BTreeSet<u32> = relatives.iter().flatten().copied().collect();
        let kept: BTreeSet<u32> = self
            .faces
            .iter()
            .flat_map(|part| part.family.iter().copied())
            .collect();
        let mut positions: Vec<usize> = every_relative
            .iter()
            .flat_map(|&face| self.set.incident(self.kind, face))
            .map(|&position| position as usize)
            .collect();
        positions.sort_unstable();
        positions.dedup();
        positions.retain(|&position| {
            keeps_one_part(self.set.adjacent_faces(position), &kept, &every_relative)
        });
        (!positions.is_empty()).then(|| Resolution::Ambiguous {
            candidates: self.candidates(positions, None),
        })
    }

    /// The relatives of a part: faces of the same feature with the same sketch generator and `@`
    /// groups under another role.
    fn relatives_of(&self, part: &FacePart) -> Vec<u32> {
        let Some(key) = part.shape.relative_key() else {
            return Vec::new();
        };
        self.names
            .faces_with_relative_key(&key)
            .iter()
            .copied()
            .filter(|&face| self.names.face_shape(face).role() != part.shape.role())
            .collect()
    }
}

/// True when one face of `adjacent` is in `kept` and every other one is a relative.
fn keeps_one_part(adjacent: &[u32], kept: &BTreeSet<u32>, relatives: &BTreeSet<u32>) -> bool {
    adjacent.len() > 1
        && (0..adjacent.len()).any(|position| {
            kept.contains(&adjacent[position])
                && adjacent
                    .iter()
                    .enumerate()
                    .all(|(other, face)| other == position || relatives.contains(face))
        })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::aliases::{Aliases, Successions};
    use crate::element_set::ElementSet;
    use crate::fingerprint::Fingerprint;
    use crate::reference::Reference;
    use crate::testing::{element_info, name};

    #[test]
    fn one_kept_face_with_relatives_keeps_one_part() {
        let kept = BTreeSet::from([1]);
        let relatives = BTreeSet::from([2, 3]);

        assert!(keeps_one_part(&[1, 2], &kept, &relatives));
        assert!(!keeps_one_part(&[1, 4], &kept, &relatives));
        assert!(!keeps_one_part(&[1], &kept, &relatives));
        assert!(!keeps_one_part(&[2, 3], &kept, &relatives));
    }

    #[test]
    fn a_one_face_edge_follows_a_relative_of_its_only_face() {
        // A counterbore hole that became a simple hole: the seam of its counterbore is gone, and
        // the seam of its bore, made from the same point, is its relative (section 3.9 step 4).
        let set = ElementSet::new(
            Arc::from(vec![
                element_info("f-hole1/bore/e-p1"),
                element_info("edge(f-hole1/bore/e-p1)"),
            ]),
            Aliases::default(),
            Successions::default(),
        )
        .expect("distinct names");
        let reference = Reference::new(
            name("edge(f-hole1/counterbore/e-p1)"),
            Fingerprint::parse("line|n=0,0,1|c=0,0,0").expect("valid fingerprint"),
        )
        .expect("the kinds agree");

        let resolution = Resolver::new(&reference, &set).resolve();

        assert_eq!(
            resolution,
            Resolution::Resolved {
                elements: vec![name("edge(f-hole1/bore/e-p1)")],
                method: MatchMethod::Related,
                confidence: 1.0,
                follow: Some(Follow::Related),
                via: Vec::new(),
                alternatives: Vec::new(),
            }
        );
    }
}
