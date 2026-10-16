//! The resolver's state and what its steps share (naming.md section 3.9): the step order, the
//! elements face sets allow, scores, candidates, history and the result constructors. Each step is
//! its own `impl Resolver` block: `exact.rs` (step 1), `pieces.rs` (step 2), `continued.rs` (step
//! 3), `related.rs` (step 4) and `missing.rs` (step 5).

use std::cell::OnceCell;
use std::collections::BTreeSet;

use crate::canon::Via;
use crate::chase::Links;
use crate::element_set::ElementSet;
use crate::kind::ElementKind;
use crate::name::ElementName;
use crate::name_set::NameSet;
use crate::reference::{MergedName, Reference, has_own_split};
use crate::resolver::candidates::{Ranked, in_candidate_order};
use crate::resolver::parts::{FacePart, OwnPart};
use crate::resolver::{Candidate, Follow, MatchMethod, Resolution};
use crate::score::score;
use crate::shape::InstanceName;

/// The most candidates a loss or a guess by score lists.
pub(super) const CANDIDATE_LIMIT: usize = 5;

/// The state of one `resolve` call.
pub(crate) struct Resolver<'a> {
    /// The reference being resolved.
    pub(super) reference: &'a Reference,
    /// The body state.
    pub(super) set: &'a ElementSet,
    /// Its names-only view.
    pub(super) names: &'a NameSet,
    /// The query's element kind.
    pub(super) kind: ElementKind,
    /// The face parts, in written order.
    pub(super) faces: Vec<FacePart>,
    /// The query itself, for an edge or vertex with its own `#n`.
    pub(super) own: Option<OwnPart>,
    /// Which face ids give a candidate `meaning` (section 3.9 step 5), computed when needed.
    meaningful: OnceCell<Vec<bool>>,
    /// The face names the query holds, computed when needed.
    named: OnceCell<QueryNames>,
}

/// The face names a query holds: its parts and its merged names (section 3.9, "Follow").
struct QueryNames {
    /// Face parts and merged face names.
    faces: BTreeSet<String>,
    /// The source and feature of every merged `Copies` entry.
    copies: Vec<(String, String)>,
}

impl QueryNames {
    /// The names `reference` holds.
    fn of(reference: &Reference, faces: &[FacePart]) -> QueryNames {
        let mut named = QueryNames {
            faces: faces.iter().map(|part| part.name.to_string()).collect(),
            copies: Vec::new(),
        };
        for merged in reference.merged.0.values().flatten() {
            match merged {
                MergedName::Face(face) => {
                    named.faces.insert(face.to_string());
                }
                MergedName::Copies { source, feature } => {
                    named.copies.push((source.to_string(), feature.to_string()));
                }
            }
        }
        named
    }

    /// True when `text` is one of the names, or an instance name of a `Copies` entry.
    fn holds(&self, text: &str) -> bool {
        self.faces.contains(text)
            || InstanceName::of(text).is_some_and(|copy| {
                self.copies
                    .iter()
                    .any(|(source, feature)| copy.source == source && copy.feature == feature)
            })
    }
}

/// How step 1 ended.
pub(super) enum Exact {
    /// With a result.
    Done(Resolution),
    /// Without one: go to step 2.
    Pieces,
    /// The fallback names could not be confirmed by their score: every order-dependent part is
    /// piece-lost; go to step 3.
    FallbackLost,
}

/// What step 2 selected for each part.
pub(super) struct Selection {
    /// The selected faces of each face part; empty when nothing is selected.
    pub(super) faces: Vec<Vec<u32>>,
    /// The selected edges or vertices of the query's own part.
    pub(super) own: Vec<usize>,
    /// For each face part, true when it is piece-lost.
    pub(super) lost: Vec<bool>,
    /// True when the query's own part is piece-lost.
    pub(super) own_lost: bool,
    /// True when some part was selected by its pieces.
    pub(super) by_pieces: bool,
}

impl Selection {
    /// True when some part is piece-lost.
    pub(super) fn any_lost(&self) -> bool {
        self.own_lost || self.lost.contains(&true)
    }
}

impl<'a> Resolver<'a> {
    /// Takes the query apart against the body state.
    pub(crate) fn new(reference: &'a Reference, set: &'a ElementSet) -> Resolver<'a> {
        let names = set.names();
        let kind = reference.query.kind();
        let faces: Vec<FacePart> = reference
            .query
            .face_parts()
            .into_iter()
            .map(|name| FacePart::new(name, reference, names))
            .collect();
        let own = has_own_split(&reference.query)
            .then(|| OwnPart::new(reference, set, set.pairing(kind, &families(&faces))));
        Resolver {
            reference,
            set,
            names,
            kind,
            faces,
            own,
            meaningful: OnceCell::new(),
            named: OnceCell::new(),
        }
    }

    /// Runs the steps in order; the first that produces a result wins.
    pub(crate) fn resolve(&self) -> Resolution {
        let fallback_lost = match self.exact() {
            Exact::Done(resolution) => return resolution,
            Exact::Pieces => false,
            Exact::FallbackLost => true,
        };
        let selection = self.select(fallback_lost);
        if !selection.any_lost() {
            let result = self.combine(&slices(&selection.faces), &selection.own);
            if !result.is_empty() {
                return self.pieces(&result, &selection);
            }
        }
        let continuations = self.continuations(&selection);
        if let Some(resolution) = self.continued(&selection, &continuations) {
            return resolution;
        }
        if let Some(resolution) = self.decided_by_fingerprint(&selection, &continuations) {
            return resolution;
        }
        if !selection.any_lost()
            && self.kind != ElementKind::Face
            && let Some(resolution) = self.related()
        {
            return resolution;
        }
        self.missing(&selection)
    }

    /// A `History` result for these elements, with `via` and `follow`.
    pub(super) fn history(&self, elements: &[usize]) -> Resolution {
        let mut via = Via::new();
        for &element in elements {
            match self.kind {
                ElementKind::Face => {
                    let face = self.set.face_id_at(element);
                    self.names
                        .matched_via(face, &self.faces[0].canonical, &mut via);
                }
                ElementKind::Edge | ElementKind::Vertex => {
                    for &face in self.set.adjacent_faces(element) {
                        for part in &self.faces {
                            if part.family.binary_search(&face).is_ok() {
                                self.names.matched_via(face, &part.canonical, &mut via);
                            }
                        }
                    }
                }
            }
        }
        let links = via.values().fold(Links::NONE, |all, links| all | *links);
        let follow = if links.contains(Links::REPLACEMENT) {
            Some(Follow::Replaced)
        } else if links.contains(Links::EXTENSION) {
            Some(Follow::Extended)
        } else if links.contains(Links::MERGE) && elements.iter().any(|&element| self.grew(element))
        {
            Some(Follow::Merged)
        } else {
            None
        };
        Resolution::Resolved {
            elements: sorted_names(self.set, elements),
            method: MatchMethod::History,
            confidence: 1.0,
            follow,
            via: via.into_keys().collect(),
            alternatives: Vec::new(),
        }
    }

    /// True when the element grew by a merge: a face that stands for a name the query does not
    /// name, or an edge or vertex all of whose faces grew.
    pub(super) fn grew(&self, element: usize) -> bool {
        let named = |text: &str| self.names_in_query(text);
        match self.kind {
            ElementKind::Face => {
                let face = self.set.face_id_at(element);
                self.names.grew(self.names.face_shape(face), named)
            }
            ElementKind::Edge | ElementKind::Vertex => {
                let faces = self.set.adjacent_faces(element);
                !faces.is_empty()
                    && faces
                        .iter()
                        .all(|&face| self.names.grew(self.names.face_shape(face), named))
            }
        }
    }

    /// True when `text` is a part of the query or one of its merged names.
    fn names_in_query(&self, text: &str) -> bool {
        self.named
            .get_or_init(|| QueryNames::of(self.reference, &self.faces))
            .holds(text)
    }

    /// The elements of the query's kind allowed by one face set per face part: for a face query the
    /// elements among the first set, for an edge or vertex query those whose faces pair with the
    /// sets. Ascending positions.
    pub(super) fn members(&self, allowed: &[&[u32]]) -> Vec<usize> {
        match self.kind {
            ElementKind::Face => allowed
                .first()
                .into_iter()
                .flat_map(|faces| faces.iter())
                .filter_map(|&face| self.set.face_element(face))
                .collect::<BTreeSet<usize>>()
                .into_iter()
                .collect(),
            ElementKind::Edge | ElementKind::Vertex => self.set.pairing(self.kind, allowed),
        }
    }

    /// The elements one face set per face part gives (`members`), among the query's own selection
    /// `own` when the query has its own `#n`.
    pub(super) fn combine(&self, allowed: &[&[u32]], own: &[usize]) -> Vec<usize> {
        let mut combined = self.members(allowed);
        if self.own.is_some() {
            combined.retain(|member| own.binary_search(member).is_ok());
        }
        combined
    }

    /// The score of the element at `element` against the stored fingerprint.
    pub(super) fn score_of(&self, element: usize) -> f64 {
        score(
            &self.reference.fingerprint,
            &self.set.element_at(element).fingerprint,
            self.kind,
        )
    }

    /// The members with their scores, best first, then by name.
    pub(super) fn scored(&self, members: &[usize]) -> Vec<(f64, usize)> {
        let mut scored: Vec<(f64, usize)> = members
            .iter()
            .map(|&member| (self.score_of(member), member))
            .collect();
        scored.sort_by(|first, second| {
            second.0.total_cmp(&first.0).then_with(|| {
                element_name(self.set, first.1).cmp(&element_name(self.set, second.1))
            })
        });
        scored
    }

    /// Candidates for these elements, in candidate order, at most `limit`.
    pub(super) fn candidates(
        &self,
        elements: impl IntoIterator<Item = usize>,
        limit: Option<usize>,
    ) -> Vec<Candidate> {
        let ranked = elements
            .into_iter()
            .map(|element| {
                let info = self.set.element_at(element);
                Ranked::new(
                    &info.name,
                    &info.fingerprint,
                    &self.reference.fingerprint,
                    self.kind,
                    self.has_meaning(element),
                )
            })
            .collect();
        in_candidate_order(ranked, limit.unwrap_or(usize::MAX))
    }

    /// True when the element touches a face in the family of a present part, or, when no part is
    /// present, a face of a feature the parts' names depend on (section 3.9 step 5).
    pub(super) fn has_meaning(&self, element: usize) -> bool {
        let meaningful = self.meaningful.get_or_init(|| self.meaningful_faces());
        self.set
            .adjacent_faces(element)
            .iter()
            .any(|&face| meaningful[face as usize])
    }

    /// The faces that give a candidate `meaning`.
    fn meaningful_faces(&self) -> Vec<bool> {
        let mut meaningful = vec![false; self.names.face_count()];
        let present: Vec<&FacePart> = self.faces.iter().filter(|part| part.is_present()).collect();
        if present.is_empty() {
            for part in &self.faces {
                for generator in part.name.generators() {
                    for &face in self.names.faces_of_feature(generator.feature.as_str()) {
                        meaningful[face as usize] = true;
                    }
                }
            }
        } else {
            for part in present {
                for &face in &part.family {
                    meaningful[face as usize] = true;
                }
            }
        }
        meaningful
    }
}

/// The family of each face part.
pub(super) fn families(faces: &[FacePart]) -> Vec<&[u32]> {
    faces.iter().map(|part| part.family.as_slice()).collect()
}

/// The sets as slices.
pub(super) fn slices(sets: &[Vec<u32>]) -> Vec<&[u32]> {
    sets.iter().map(Vec::as_slice).collect()
}

/// The best and second best scores of a list sorted best first (0 when absent).
pub(super) fn top_two(scored: &[(f64, usize)]) -> (f64, f64) {
    let score_at = |position: usize| scored.get(position).map_or(0.0, |(score, _)| *score);
    (score_at(0), score_at(1))
}

/// The name of the element at `element`.
pub(super) fn element_name(set: &ElementSet, element: usize) -> ElementName {
    set.element_at(element).name.clone()
}

/// The names of these elements, sorted.
pub(super) fn sorted_names(set: &ElementSet, elements: &[usize]) -> Vec<ElementName> {
    let mut names: Vec<ElementName> = elements
        .iter()
        .map(|&element| element_name(set, element))
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// A result of one element with this confidence and nothing followed.
pub(super) fn resolved_with(
    element: ElementName,
    method: MatchMethod,
    confidence: f64,
) -> Resolution {
    Resolution::Resolved {
        elements: vec![element],
        method,
        confidence,
        follow: None,
        via: Vec::new(),
        alternatives: Vec::new(),
    }
}

/// A result of these elements, found by `method` with confidence 1, that followed a change.
pub(super) fn followed(
    elements: Vec<ElementName>,
    method: MatchMethod,
    follow: Follow,
) -> Resolution {
    Resolution::Resolved {
        elements,
        method,
        confidence: 1.0,
        follow: Some(follow),
        via: Vec::new(),
        alternatives: Vec::new(),
    }
}

/// A guess of one element with this confidence and the members that might be meant instead.
pub(super) fn guessed(
    element: ElementName,
    confidence: f64,
    alternatives: Vec<Candidate>,
) -> Resolution {
    Resolution::Resolved {
        elements: vec![element],
        method: MatchMethod::Guess,
        confidence,
        follow: None,
        via: Vec::new(),
        alternatives,
    }
}

#[cfg(test)]
mod tests {
    use partsmith_model::Id;

    use super::*;
    use crate::fingerprint::Fingerprint;
    use crate::reference::Merged;
    use crate::testing::{name, set_of};

    #[test]
    fn the_top_two_scores_default_to_zero() {
        assert_eq!(top_two(&[(0.9, 1), (0.8, 2), (0.1, 3)]), (0.9, 0.8));
        assert_eq!(top_two(&[(0.9, 1)]), (0.9, 0.0));
        assert_eq!(top_two(&[]), (0.0, 0.0));
    }

    #[test]
    fn query_names_hold_parts_merged_faces_and_instances_of_copies() {
        let set = set_of(&["f-a/top"], &[], &[], &[]);
        let mut reference = Reference::new(
            name("f-a/top"),
            Fingerprint::parse("plane|n=0,0,1").expect("valid fingerprint"),
        )
        .expect("the kinds agree");
        reference.merged = Merged(
            [(
                name("f-a/top"),
                vec![
                    MergedName::Face(name("f-b/top")),
                    MergedName::Copies {
                        source: name("f-c/top"),
                        feature: Id::new("f-lp1").expect("valid id"),
                    },
                ],
            )]
            .into(),
        );
        let parts = [FacePart::new(name("f-a/top"), &reference, &set)];

        let named = QueryNames::of(&reference, &parts);

        assert!(named.holds("f-a/top"));
        assert!(named.holds("f-b/top"));
        assert!(named.holds("f-c/top@f-lp1:2"));
        assert!(named.holds("f-c/top@f-lp1:1.2"));
        assert!(!named.holds("f-c/top@f-lp1:"));
        assert!(!named.holds("f-c/top@f-lp1:2#1"));
        assert!(!named.holds("f-d/top"));
    }
}
