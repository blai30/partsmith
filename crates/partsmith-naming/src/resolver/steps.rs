//! The resolver's steps (naming.md section 3.9): exact, pieces, continuation, related and lost.

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
use crate::resolver::parts::{FacePart, LiveAnchors, OwnPart, face_id};
use crate::resolver::{
    AMBIGUITY_MARGIN, CONFIDENCE_THRESHOLD, Candidate, Follow, Lost, MatchMethod, Resolution,
};
use crate::score::score;
use crate::shape::pairs_one_to_one;

/// The most candidates a loss or a guess by score lists.
const CANDIDATE_LIMIT: usize = 5;

/// The state of one `resolve` call.
pub(crate) struct Resolver<'a> {
    /// The reference being resolved.
    reference: &'a Reference,
    /// The body state.
    set: &'a ElementSet,
    /// Its names-only view.
    names: &'a NameSet,
    /// The query's element kind.
    kind: ElementKind,
    /// The face parts, in written order.
    faces: Vec<FacePart>,
    /// The query itself, for an edge or vertex with its own `#n`.
    own: Option<OwnPart>,
    /// Which face ids give a candidate `meaning` (section 3.9 step 5), computed when needed.
    meaningful: OnceCell<Vec<bool>>,
    /// The face names the query holds, computed when needed.
    named: OnceCell<QueryNames>,
}

/// The face names a query holds: its parts and its merged names (section 3.9, "Follow").
struct QueryNames {
    /// Face parts and merged face names.
    faces: BTreeSet<String>,
    /// `<source>@<feature>:` of every merged `Copies` entry.
    copies: Vec<String>,
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
                    named.copies.push(format!("{source}@{feature}:"));
                }
            }
        }
        named
    }

    /// True when `text` is one of the names, or an instance name of a `Copies` entry.
    fn holds(&self, text: &str) -> bool {
        self.faces.contains(text)
            || self.copies.iter().any(|prefix| {
                text.strip_prefix(prefix.as_str()).is_some_and(|instance| {
                    !instance.is_empty()
                        && instance
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || byte == b'.')
                })
            })
    }
}

/// How step 1 ended.
enum Exact {
    /// With a result.
    Done(Resolution),
    /// Without one: go to step 2.
    Pieces,
    /// The fallback names could not be confirmed by their score: every order-dependent part is
    /// piece-lost; go to step 3.
    FallbackLost,
}

/// What step 2 selected for each part.
struct Selection {
    /// The selected faces of each face part; empty when nothing is selected.
    faces: Vec<Vec<u32>>,
    /// The selected edges or vertices of the query's own part.
    own: Vec<usize>,
    /// For each face part, true when it is piece-lost.
    lost: Vec<bool>,
    /// True when the query's own part is piece-lost.
    own_lost: bool,
    /// True when some part was selected by its pieces.
    by_pieces: bool,
}

impl Selection {
    /// True when some part is piece-lost.
    fn any_lost(&self) -> bool {
        self.own_lost || self.lost.contains(&true)
    }
}

impl<'a> Resolver<'a> {
    /// Takes the query apart against the body state.
    pub(crate) fn new(reference: &'a Reference, set: &'a ElementSet) -> Resolver<'a> {
        let names = set.names();
        let kind = reference.query.kind();
        let part_names = match kind {
            ElementKind::Face => vec![reference.query.clone()],
            ElementKind::Edge | ElementKind::Vertex => reference.query.face_terms(),
        };
        let faces: Vec<FacePart> = part_names
            .into_iter()
            .map(|name| FacePart::new(name, reference, names))
            .collect();
        let mut resolver = Resolver {
            reference,
            set,
            names,
            kind,
            faces,
            own: None,
            meaningful: OnceCell::new(),
            named: OnceCell::new(),
        };
        if has_own_split(&reference.query) {
            let families: Vec<&[u32]> = resolver
                .faces
                .iter()
                .map(|part| part.family.as_slice())
                .collect();
            let family = resolver.pairing(&families);
            resolver.own = Some(OwnPart::new(reference, set, family));
        }
        resolver
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
            let result = self.combine(&selection.faces, &selection.own);
            if !result.is_empty() {
                return self.pieces(result, &selection);
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
        self.missing(&selection, &continuations)
    }

    /// Step 1: the exact name, confirmed by anchors or against its siblings.
    fn exact(&self) -> Exact {
        let query = &self.reference.query;
        let named = self.set.position_of(self.kind, query.as_str());
        if self.faces.iter().any(|part| part.canonical.len() > 1) {
            return Exact::Pieces;
        }
        if !query.is_order_dependent() {
            return match named.or_else(|| self.only_candidate()) {
                Some(element) if named.is_some() => Exact::Done(resolved(
                    vec![element_name(self.set, element)],
                    MatchMethod::Exact,
                )),
                Some(element) => Exact::Done(self.history(vec![element])),
                None => Exact::Pieces,
            };
        }
        let ordered: Vec<&FacePart> = self
            .faces
            .iter()
            .filter(|part| part.order_dependent)
            .collect();
        let own_live = self.own.as_ref().map(|own| own.anchors.is_some());
        let every_part_live =
            ordered.iter().all(|part| part.anchors.is_some()) && own_live.unwrap_or(true);
        if ordered.iter().all(|part| !part.fallback) && every_part_live {
            return match named.filter(|&element| self.covers_live_anchors(element)) {
                Some(element) => Exact::Done(resolved(
                    vec![element_name(self.set, element)],
                    MatchMethod::Exact,
                )),
                None => Exact::Pieces,
            };
        }
        let only_fallback = self.own.is_none()
            && ordered
                .iter()
                .all(|part| part.fallback && part.anchors.is_none());
        if only_fallback {
            return self.confirm_fallback(named);
        }
        let some_part_unanchored =
            ordered.iter().any(|part| part.anchors.is_none()) || own_live == Some(false);
        match named {
            Some(element) if some_part_unanchored && self.covers_live_anchors(element) => {
                Exact::Done(self.guess_rule(element))
            }
            _ => Exact::Pieces,
        }
    }

    /// Step 1 without a candidate named by the query: the candidate without `#` whose faces are
    /// each the only member of the family of the part they pair with.
    fn only_candidate(&self) -> Option<usize> {
        let only: Vec<u32> = self
            .faces
            .iter()
            .map(|part| match part.family.as_slice() {
                [face] => Some(*face),
                _ => None,
            })
            .collect::<Option<_>>()?;
        let candidates: Vec<usize> = match self.kind {
            ElementKind::Face => self.set.face_element(only[0]).into_iter().collect(),
            ElementKind::Edge | ElementKind::Vertex => {
                let allowed: Vec<&[u32]> = only.iter().map(std::slice::from_ref).collect();
                self.pairing(&allowed)
            }
        };
        let mut unnumbered = candidates
            .into_iter()
            .filter(|&element| !element_name(self.set, element).is_order_dependent());
        let first = unnumbered.next();
        if unnumbered.next().is_some() {
            return None;
        }
        first
    }

    /// True when `element` covers the live anchors of every part that has them, each through the
    /// face it pairs with (the element itself for the query's own `#n`).
    fn covers_live_anchors(&self, element: usize) -> bool {
        let own_covered = self.own.as_ref().is_none_or(|own| {
            own.anchors
                .as_ref()
                .is_none_or(|anchors| anchors.covers(&self.set.faces_beyond_vertices(element)))
        });
        own_covered && {
            let allowed = self.covering_sets();
            let allowed: Vec<&[u32]> = allowed.iter().map(Vec::as_slice).collect();
            self.element_pairs(element, &allowed)
        }
    }

    /// Per face part: the members that cover its live anchors, or its whole family when it has
    /// none.
    fn covering_sets(&self) -> Vec<Vec<u32>> {
        self.faces
            .iter()
            .map(|part| match &part.anchors {
                Some(anchors) => part
                    .family
                    .iter()
                    .copied()
                    .filter(|&face| anchors.covers(self.names.touching(face)))
                    .collect(),
                None => part.family.clone(),
            })
            .collect()
    }

    /// True when the faces of `element` (itself for a face) pair one to one with `allowed`.
    fn element_pairs(&self, element: usize, allowed: &[&[u32]]) -> bool {
        match self.kind {
            ElementKind::Face => allowed
                .first()
                .is_some_and(|faces| faces.binary_search(&self.set.face_id_at(element)).is_ok()),
            ElementKind::Edge | ElementKind::Vertex => {
                pairs(self.set.adjacent_faces(element), allowed)
            }
        }
    }

    /// Step 1, fallback names: confirmed by their score.
    fn confirm_fallback(&self, named: Option<usize>) -> Exact {
        let families: Vec<&[u32]> = self
            .faces
            .iter()
            .map(|part| part.family.as_slice())
            .collect();
        let members = self.members(&families);
        let scored = self.scored(&members);
        let (best, second) = top_two(&scored);
        let Some(&(best_score, best_member)) = scored.first() else {
            return Exact::FallbackLost;
        };
        if best < CONFIDENCE_THRESHOLD {
            return Exact::FallbackLost;
        }
        if best - second >= AMBIGUITY_MARGIN {
            let named_score = named.map(|element| self.score_of(element));
            return Exact::Done(match (named, named_score) {
                (Some(element), Some(score)) if score >= best - AMBIGUITY_MARGIN => {
                    resolved_with(element_name(self.set, element), MatchMethod::Exact, score)
                }
                _ => resolved_with(
                    element_name(self.set, best_member),
                    MatchMethod::History,
                    best_score,
                ),
            });
        }
        let floor = (best - AMBIGUITY_MARGIN).max(CONFIDENCE_THRESHOLD);
        let fitting = scored
            .iter()
            .filter(|(score, _)| *score >= floor)
            .map(|(_, member)| *member);
        Exact::Done(Resolution::Ambiguous {
            candidates: self.candidates(fitting, None),
        })
    }

    /// Step 1, a name without anchors: used, but a guess when a sibling fits the stored fingerprint
    /// clearly better.
    fn guess_rule(&self, element: usize) -> Resolution {
        let allowed = self.covering_sets();
        let allowed: Vec<&[u32]> = allowed.iter().map(Vec::as_slice).collect();
        let mut members = self.members(&allowed);
        if let Some(anchors) = self.own.as_ref().and_then(|own| own.anchors.as_ref()) {
            members.retain(|&member| anchors.covers(&self.set.faces_beyond_vertices(member)));
        }
        let element_score = self.score_of(element);
        let better = members
            .into_iter()
            .filter(|&member| member != element)
            .filter(|&member| self.score_of(member) >= element_score + AMBIGUITY_MARGIN);
        let alternatives = self.candidates(better, None);
        if alternatives.is_empty() {
            return resolved(vec![element_name(self.set, element)], MatchMethod::Exact);
        }
        Resolution::Resolved {
            elements: vec![element_name(self.set, element)],
            method: MatchMethod::Guess,
            confidence: element_score,
            follow: None,
            via: Vec::new(),
            alternatives,
        }
    }

    /// Step 2: what each part selects of its family.
    fn select(&self, fallback_lost: bool) -> Selection {
        let mut selection = Selection {
            faces: Vec::with_capacity(self.faces.len()),
            own: Vec::new(),
            lost: vec![false; self.faces.len()],
            own_lost: false,
            by_pieces: false,
        };
        for (position, part) in self.faces.iter().enumerate() {
            if fallback_lost && part.order_dependent {
                selection.lost[position] = true;
                selection.faces.push(Vec::new());
                continue;
            }
            let selected = match &part.anchors {
                Some(anchors) => {
                    let members: Vec<usize> =
                        part.family.iter().map(|&face| face as usize).collect();
                    select_anchored(anchors, &members, |member| {
                        self.names.touching(face_id(member)).to_vec()
                    })
                    .map(|members| members.into_iter().map(face_id).collect())
                }
                None if !part.order_dependent => Some(part.family.clone()),
                None => {
                    let pieces = self.pieces_of(part);
                    selection.by_pieces |= !pieces.is_empty();
                    (!pieces.is_empty()).then_some(pieces)
                }
            };
            selection.lost[position] = selected.is_none();
            selection.faces.push(selected.unwrap_or_default());
        }
        if let Some(own) = self.own.as_ref().filter(|own| !own.family.is_empty()) {
            let selected = match &own.anchors {
                Some(anchors) => select_anchored(anchors, &own.family, |member| {
                    self.set.faces_beyond_vertices(member)
                }),
                None => {
                    let pieces: Vec<usize> = own
                        .family
                        .iter()
                        .copied()
                        .filter(|&member| {
                            let name = element_name(self.set, member);
                            name == own.name || name.is_split_descendant_of(&own.name)
                        })
                        .collect();
                    selection.by_pieces |= !pieces.is_empty();
                    (!pieces.is_empty()).then_some(pieces)
                }
            };
            selection.own_lost = selected.is_none();
            selection.own = selected.unwrap_or_default();
        }
        selection
    }

    /// The pieces of an order-dependent part without live anchors: the members that extend it, and
    /// the members that extend a name the chase of a key that extends it gave.
    fn pieces_of(&self, part: &FacePart) -> Vec<u32> {
        let mut continued: Vec<_> = Vec::new();
        if self.names.is_key(part.shape.text()) {
            continued.extend(self.names.chase_forms(&part.shape));
        }
        for key in self
            .names
            .keys_with_piece_free(part.shape.piece_free_text())
        {
            if key.text() != part.shape.text() && key.extends(&part.shape) {
                continued.extend(self.names.chase_forms(key));
            }
        }
        part.family
            .iter()
            .copied()
            .filter(|&face| {
                let shape = self.names.face_shape(face);
                shape.extends(&part.shape)
                    || continued.iter().any(|form| shape.extends(&form.shape))
            })
            .collect()
    }

    /// The elements the selections give: the selected faces of a face query, or the edges or
    /// vertices whose faces pair with the selections, among the query's own selection.
    fn combine(&self, faces: &[Vec<u32>], own: &[usize]) -> Vec<usize> {
        let allowed: Vec<&[u32]> = faces.iter().map(Vec::as_slice).collect();
        let mut combined = self.members(&allowed);
        if self.own.is_some() {
            combined.retain(|member| own.binary_search(member).is_ok());
        }
        combined
    }

    /// Step 2's result, a guess when pieces were taken by name and a sibling fits clearly better.
    fn pieces(&self, result: Vec<usize>, selection: &Selection) -> Resolution {
        let resolution = self.history(result.clone());
        if !selection.by_pieces {
            return resolution;
        }
        let allowed: Vec<Vec<u32>> = self
            .faces
            .iter()
            .zip(&selection.faces)
            .map(|(part, selected)| match part.anchors {
                Some(_) => selected.clone(),
                None => part.family.clone(),
            })
            .collect();
        let own = match &self.own {
            Some(own) if own.anchors.is_some() => selection.own.clone(),
            Some(own) => own.family.clone(),
            None => Vec::new(),
        };
        let family = self.combine(&allowed, &own);
        let best_inside = result
            .iter()
            .map(|&member| self.score_of(member))
            .fold(0.0, f64::max);
        let better = family
            .into_iter()
            .filter(|member| result.binary_search(member).is_err())
            .filter(|&member| self.score_of(member) >= best_inside + AMBIGUITY_MARGIN);
        let alternatives = self.candidates(better, None);
        match resolution {
            Resolution::Resolved {
                elements,
                follow,
                via,
                ..
            } if !alternatives.is_empty() => Resolution::Resolved {
                elements,
                method: MatchMethod::Guess,
                confidence: best_inside,
                follow,
                via,
                alternatives,
            },
            resolution => resolution,
        }
    }

    /// Step 3: the rims that continue each part that is not present or piece-lost.
    fn continuations(&self, selection: &Selection) -> Vec<Vec<u32>> {
        self.faces
            .iter()
            .zip(&selection.lost)
            .map(|(part, &lost)| {
                if part.is_present() && !lost {
                    return Vec::new();
                }
                self.names
                    .rims()
                    .iter()
                    .filter(|rim| self.names.meet(&rim.canonical, &part.canonical))
                    .filter(|rim| {
                        !part.order_dependent
                            || match &part.anchors {
                                Some(anchors) => anchors.covers(self.names.touching(rim.face)),
                                None => rim.generator.extends(&part.shape),
                            }
                    })
                    .map(|rim| rim.face)
                    .collect()
            })
            .collect()
    }

    /// Step 3's result when continuations give one.
    fn continued(&self, selection: &Selection, continuations: &[Vec<u32>]) -> Option<Resolution> {
        let allowed: Option<Vec<&[u32]>> = selection
            .faces
            .iter()
            .zip(continuations)
            .map(|(selected, continuing)| {
                if !selected.is_empty() {
                    Some(selected.as_slice())
                } else if !continuing.is_empty() {
                    Some(continuing.as_slice())
                } else {
                    None
                }
            })
            .collect();
        if !continuations
            .iter()
            .any(|continuing| !continuing.is_empty())
        {
            return None;
        }
        let allowed = allowed?;
        if self.own.is_some() && selection.own.is_empty() {
            return None;
        }
        let elements = self.combine_allowed(&allowed, &selection.own);
        (!elements.is_empty()).then(|| Resolution::Resolved {
            elements: sorted_names(self.set, elements),
            method: MatchMethod::Continued,
            confidence: 1.0,
            follow: Some(Follow::Continued),
            via: Vec::new(),
            alternatives: Vec::new(),
        })
    }

    /// Step 3: a piece-lost part without live anchors that no rim continues, decided by the stored
    /// fingerprint.
    fn decided_by_fingerprint(
        &self,
        selection: &Selection,
        continuations: &[Vec<u32>],
    ) -> Option<Resolution> {
        let decided: Vec<bool> = self
            .faces
            .iter()
            .zip(&selection.lost)
            .zip(continuations)
            .map(|((part, &lost), continuing)| {
                lost && part.anchors.is_none() && continuing.is_empty()
            })
            .collect();
        let own_decided = self
            .own
            .as_ref()
            .is_some_and(|own| selection.own_lost && own.anchors.is_none());
        if !own_decided && !decided.contains(&true) {
            return None;
        }
        let allowed: Vec<&[u32]> = self
            .faces
            .iter()
            .enumerate()
            .map(|(position, part)| {
                if decided[position] {
                    part.family.as_slice()
                } else if !selection.faces[position].is_empty() {
                    selection.faces[position].as_slice()
                } else {
                    continuations[position].as_slice()
                }
            })
            .collect();
        let own: &[usize] = match &self.own {
            Some(own) if own_decided => &own.family,
            Some(_) => &selection.own,
            None => &[],
        };
        let members = self.combine_allowed(&allowed, own);
        let scored = self.scored(&members);
        let (best, second) = top_two(&scored);
        let &(_, best_member) = scored.first()?;
        if best < CONFIDENCE_THRESHOLD || best - second < AMBIGUITY_MARGIN {
            return None;
        }
        let others = scored.iter().skip(1).map(|(_, member)| *member);
        Some(Resolution::Resolved {
            elements: vec![element_name(self.set, best_member)],
            method: MatchMethod::Guess,
            confidence: best,
            follow: None,
            via: Vec::new(),
            alternatives: self.candidates(others, Some(CANDIDATE_LIMIT)),
        })
    }

    /// Step 4 (edges and vertices): relatives of the same feature and sketch generator.
    fn related(&self) -> Option<Resolution> {
        let relatives: Vec<Vec<u32>> = self
            .faces
            .iter()
            .map(|part| self.relatives_of(part))
            .collect();
        let absent: Vec<usize> = (0..self.faces.len())
            .filter(|&position| !self.faces[position].is_present())
            .collect();
        match absent.as_slice() {
            [missing] => {
                let allowed: Vec<&[u32]> = self
                    .faces
                    .iter()
                    .enumerate()
                    .map(|(position, part)| {
                        if position == *missing {
                            relatives[position].as_slice()
                        } else {
                            part.family.as_slice()
                        }
                    })
                    .collect();
                match self.pairing(&allowed).as_slice() {
                    [] => None,
                    [only] => Some(Resolution::Resolved {
                        elements: vec![element_name(self.set, *only)],
                        method: MatchMethod::Related,
                        confidence: 1.0,
                        follow: Some(Follow::Related),
                        via: Vec::new(),
                        alternatives: Vec::new(),
                    }),
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
            [] => {
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
            _ => None,
        }
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

    /// Step 5: what is gone, and the candidates.
    fn missing(&self, selection: &Selection, continuations: &[Vec<u32>]) -> Resolution {
        let mut lost = Lost::default();
        for ((part, &piece_lost), continuing) in
            self.faces.iter().zip(&selection.lost).zip(continuations)
        {
            let gone =
                !part.is_present() && continuing.is_empty() && self.relatives_of(part).is_empty();
            if gone {
                lost.parts.push(part.name.clone());
            } else if piece_lost {
                lost.pieces.push(part.name.clone());
            }
        }
        if let Some(own) = &self.own
            && selection.own_lost
        {
            lost.pieces.push(own.name.clone());
        }
        lost.unmet = self.faces.iter().all(FacePart::is_present) && !selection.any_lost();
        let positions = self.set.positions(self.kind);
        Resolution::Missing {
            lost,
            candidates: self.candidates(positions, Some(CANDIDATE_LIMIT)),
        }
    }

    /// A `History` result for these elements, with `via` and `follow`.
    fn history(&self, elements: Vec<usize>) -> Resolution {
        let mut via = Via::new();
        for &element in &elements {
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
    fn grew(&self, element: usize) -> bool {
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
    fn members(&self, allowed: &[&[u32]]) -> Vec<usize> {
        match self.kind {
            ElementKind::Face => allowed
                .first()
                .into_iter()
                .flat_map(|faces| faces.iter())
                .filter_map(|&face| self.set.face_element(face))
                .collect::<BTreeSet<usize>>()
                .into_iter()
                .collect(),
            ElementKind::Edge | ElementKind::Vertex => self.pairing(allowed),
        }
    }

    /// `members` among the query's own selection `own` when the query has its own `#n`.
    fn combine_allowed(&self, allowed: &[&[u32]], own: &[usize]) -> Vec<usize> {
        let mut combined = self.members(allowed);
        if self.own.is_some() {
            combined.retain(|member| own.binary_search(member).is_ok());
        }
        combined
    }

    /// The edges or vertices of the query's kind whose faces pair one to one with `allowed`, one
    /// set per face part. Ascending positions.
    fn pairing(&self, allowed: &[&[u32]]) -> Vec<usize> {
        let Some(smallest) = allowed.iter().min_by_key(|faces| faces.len()) else {
            return Vec::new();
        };
        let mut positions: Vec<usize> = smallest
            .iter()
            .flat_map(|&face| self.set.incident(self.kind, face))
            .map(|&position| position as usize)
            .collect();
        positions.sort_unstable();
        positions.dedup();
        positions.retain(|&position| pairs(self.set.adjacent_faces(position), allowed));
        positions
    }

    /// The score of the element at `element` against the stored fingerprint.
    fn score_of(&self, element: usize) -> f64 {
        score(
            &self.reference.fingerprint,
            &self.set.element(element).fingerprint,
            self.kind,
        )
    }

    /// The members with their scores, best first, then by name.
    fn scored(&self, members: &[usize]) -> Vec<(f64, usize)> {
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
    fn candidates(
        &self,
        elements: impl IntoIterator<Item = usize>,
        limit: Option<usize>,
    ) -> Vec<Candidate> {
        let ranked = elements
            .into_iter()
            .map(|element| {
                let info = self.set.element(element);
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
    fn has_meaning(&self, element: usize) -> bool {
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

/// Step 2 for a part with live anchors: the member that covers them, or, when every anchor is live
/// and the members that touch one together cover them, all of those; `None` when the part is
/// piece-lost.
fn select_anchored(
    anchors: &LiveAnchors,
    members: &[usize],
    adjacent: impl Fn(usize) -> Vec<u32>,
) -> Option<Vec<usize>> {
    if let Some(covering) = anchors.covering {
        return Some(vec![covering]);
    }
    if !anchors.every_anchor_live {
        return None;
    }
    let mut touched_together: Vec<bool> = Vec::new();
    let mut touching = Vec::new();
    for &member in members {
        let touched = anchors.touched(&adjacent(member));
        if touched.contains(&true) {
            touching.push(member);
            if touched_together.is_empty() {
                touched_together = touched;
            } else {
                for (together, one) in touched_together.iter_mut().zip(touched) {
                    *together |= one;
                }
            }
        }
    }
    (!touching.is_empty() && touched_together.iter().all(|&touched| touched)).then_some(touching)
}

/// True when the faces `adjacent` pair one to one with `allowed`, each face in its set.
fn pairs(adjacent: &[u32], allowed: &[&[u32]]) -> bool {
    pairs_one_to_one(adjacent.len(), allowed.len(), |face, part| {
        allowed[part].binary_search(&adjacent[face]).is_ok()
    })
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

/// The best and second best scores of a list sorted best first (0 when absent).
fn top_two(scored: &[(f64, usize)]) -> (f64, f64) {
    let score_at = |position: usize| scored.get(position).map_or(0.0, |(score, _)| *score);
    (score_at(0), score_at(1))
}

/// The name of the element at `element`.
fn element_name(set: &ElementSet, element: usize) -> ElementName {
    set.element(element).name.clone()
}

/// The names of these elements, sorted.
fn sorted_names(set: &ElementSet, elements: Vec<usize>) -> Vec<ElementName> {
    let mut names: Vec<ElementName> = elements
        .into_iter()
        .map(|element| element_name(set, element))
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// A result with confidence 1 and nothing followed.
fn resolved(elements: Vec<ElementName>, method: MatchMethod) -> Resolution {
    Resolution::Resolved {
        elements,
        method,
        confidence: 1.0,
        follow: None,
        via: Vec::new(),
        alternatives: Vec::new(),
    }
}

/// A result of one element with this confidence.
fn resolved_with(element: ElementName, method: MatchMethod, confidence: f64) -> Resolution {
    Resolution::Resolved {
        elements: vec![element],
        method,
        confidence,
        follow: None,
        via: Vec::new(),
        alternatives: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use partsmith_model::Id;

    use super::*;
    use crate::aliases::{Aliases, Successions};
    use crate::element_info::ElementInfo;
    use crate::fingerprint::Fingerprint;
    use crate::name_set::testing::{name, set_of};
    use crate::reference::Merged;

    /// The ids of the faces `texts` of `set`, ascending.
    fn ids(set: &NameSet, texts: &[&str]) -> Vec<u32> {
        let mut ids: Vec<u32> = texts
            .iter()
            .map(|text| set.face_id(text).expect("the face is in the set"))
            .collect();
        ids.sort_unstable();
        ids
    }

    /// The top split in three pieces: `#1` touches the front, `#2` the back, `#3` nothing.
    fn split_top() -> NameSet {
        set_of(
            &[
                "f-a/top#1",
                "f-a/top#2",
                "f-a/top#3",
                "f-a/front",
                "f-a/back",
            ],
            &[("f-a/top#1", "f-a/front"), ("f-a/top#2", "f-a/back")],
            &[],
            &[],
        )
    }

    /// The live anchors `anchors` of the pieces of the top of `set`, and the pieces.
    fn live(set: &NameSet, anchors: &[&str]) -> (Option<LiveAnchors>, Vec<usize>) {
        let anchor_names: Vec<ElementName> = anchors.iter().map(|anchor| name(anchor)).collect();
        let members: Vec<usize> = ids(set, &["f-a/top#1", "f-a/top#2", "f-a/top#3"])
            .into_iter()
            .map(|face| face as usize)
            .collect();
        let anchors = LiveAnchors::of(&anchor_names, set, &members, |member| {
            set.touching(face_id(member)).to_vec()
        });
        (anchors, members)
    }

    #[test]
    fn the_member_covering_the_anchors_is_selected() {
        let set = split_top();
        let (anchors, members) = live(&set, &["f-a/front"]);
        let anchors = anchors.expect("the anchor is live");

        let selected = select_anchored(&anchors, &members, |member| {
            set.touching(face_id(member)).to_vec()
        });

        assert_eq!(
            selected,
            Some(vec![set.face_id("f-a/top#1").expect("a face") as usize])
        );
    }

    #[test]
    fn members_that_cover_the_anchors_together_are_selected_together() {
        let set = split_top();
        let (anchors, members) = live(&set, &["f-a/back", "f-a/front"]);
        let anchors = anchors.expect("the anchors are live");

        let selected = select_anchored(&anchors, &members, |member| {
            set.touching(face_id(member)).to_vec()
        });

        let expected: Vec<usize> = ids(&set, &["f-a/top#1", "f-a/top#2"])
            .into_iter()
            .map(|face| face as usize)
            .collect();
        assert_eq!(selected, Some(expected));
    }

    #[test]
    fn a_part_with_a_gone_anchor_and_no_covering_member_is_piece_lost() {
        let set = split_top();
        let (anchors, members) = live(&set, &["f-a/back", "f-a/front", "f-a/left"]);
        let anchors = anchors.expect("two anchors are live");

        let selected = select_anchored(&anchors, &members, |member| {
            set.touching(face_id(member)).to_vec()
        });

        assert_eq!(selected, None);
    }

    #[test]
    fn faces_pair_when_each_sits_in_its_own_allowed_set() {
        assert!(pairs(&[1, 4], &[&[4, 5], &[1, 2]]));
        assert!(!pairs(&[1, 2], &[&[1, 2], &[3]]));
        assert!(!pairs(&[1], &[&[1], &[1]]));
    }

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
    fn the_top_two_scores_default_to_zero() {
        assert_eq!(top_two(&[(0.9, 1), (0.8, 2), (0.1, 3)]), (0.9, 0.8));
        assert_eq!(top_two(&[(0.9, 1)]), (0.9, 0.0));
        assert_eq!(top_two(&[]), (0.0, 0.0));
    }

    /// An element info named `text` with an unspecific fingerprint of its kind; an edge lists the
    /// face terms of its name as its adjacency.
    fn element(text: &str) -> ElementInfo {
        let name = name(text);
        let kind = name.kind();
        let fingerprint = match kind {
            ElementKind::Face => "plane|n=0,0,1|c=0,0,0",
            ElementKind::Edge => "line|n=0,0,1|c=0,0,0",
            ElementKind::Vertex => "point|c=0,0,0",
        };
        ElementInfo {
            adjacency: name.face_terms(),
            kind,
            fingerprint: Fingerprint::parse(fingerprint).expect("valid fingerprint"),
            name,
            vertices: Vec::new(),
        }
    }

    #[test]
    fn a_one_face_edge_follows_a_relative_of_its_only_face() {
        // A counterbore hole that became a simple hole: the seam of its counterbore is gone, and
        // the seam of its bore, made from the same point, is its relative (section 3.9 step 4).
        let set = ElementSet::new(
            Arc::from(vec![
                element("f-hole1/bore/e-p1"),
                element("edge(f-hole1/bore/e-p1)"),
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
