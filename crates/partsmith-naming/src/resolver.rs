//! Resolving references after a rebuild (naming.md sections 2.8 and 3.9).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::aliases::Aliases;
use crate::element_info::ElementInfo;
use crate::kind::ElementKind;
use crate::name::ElementName;
use crate::reference::Reference;
use crate::score::score;

/// A fingerprint-only match must score at least this much.
pub const CONFIDENCE_THRESHOLD: f64 = 0.85;

/// The best candidate must beat the second best by at least this much to be chosen.
pub const AMBIGUITY_MARGIN: f64 = 0.05;

/// Outcome of resolving one reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Resolution {
    /// The reference denotes these elements: one element, or every surviving piece of a split element. Sorted by name.
    Resolved {
        /// The denoted elements, sorted by name.
        elements: Vec<ElementName>,
        /// The resolver step that matched.
        method: MatchMethod,
        /// How sure the match is, in [0, 1].
        confidence: f64,
    },
    /// Several candidates fit equally well. Sorted by score descending, then name.
    Ambiguous {
        /// The candidates that fit, best first.
        candidates: Vec<Candidate>,
    },
    /// Nothing fits; `nearest` is the best fingerprint candidate, if any element of the kind exists.
    Missing {
        /// The best-scoring element of the reference's kind, if there is one.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        nearest: Option<Candidate>,
    },
}

/// A candidate element and its fingerprint score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Candidate {
    /// The candidate's name.
    pub name: ElementName,
    /// Its score against the reference fingerprint.
    pub score: f64,
}

/// Which resolver step produced the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MatchMethod {
    /// The name matched exactly.
    Exact,
    /// The name matched through splits, merges or un-splits (history chain).
    History,
    /// Only the fingerprint matched.
    Fingerprint,
}

/// Resolves a reference against the elements of one body state (algorithm in section 3.9).
pub fn resolve(reference: &Reference, elements: &[ElementInfo], aliases: &Aliases) -> Resolution {
    let kind = reference.query.kind();
    let resolver = Resolver {
        reference,
        kind,
        aliases,
        candidates: elements.iter().filter(|info| info.kind == kind).collect(),
    };
    resolver
        .exact()
        .or_else(|| resolver.split())
        .or_else(|| resolver.history())
        .unwrap_or_else(|| resolver.by_fingerprint())
}

/// A candidate name with its score.
type Scored<'a> = (f64, &'a ElementName);

/// The state of one `resolve` call.
struct Resolver<'a> {
    /// The reference being resolved.
    reference: &'a Reference,
    /// The query's element kind.
    kind: ElementKind,
    /// Aliases of merged faces in the body state.
    aliases: &'a Aliases,
    /// The elements of the query's kind.
    candidates: Vec<&'a ElementInfo>,
}

impl<'a> Resolver<'a> {
    /// Step 1: a candidate has exactly the query name.
    fn exact(&self) -> Option<Resolution> {
        let query = &self.reference.query;
        let exact = self
            .candidates
            .iter()
            .copied()
            .find(|candidate| candidate.name == *query)?;
        if !query.is_order_dependent() {
            return Some(resolved(query, MatchMethod::Exact, 1.0));
        }
        // An order-dependent name is confirmed by its fingerprint against its family.
        let mut members = self.family();
        if !members.iter().any(|member| member.name == *query) {
            members.push(exact);
        }
        let ranked = self.ranked(members);
        let (best, _) = top_two(&ranked);
        let exact_score = self.score(exact);
        if exact_score >= best - AMBIGUITY_MARGIN {
            return Some(resolved(query, MatchMethod::Exact, exact_score));
        }
        Some(choose(&ranked))
    }

    /// Step 2: candidates descending from the query through splits.
    fn split(&self) -> Option<Resolution> {
        let query = &self.reference.query;
        let unsplit_list_query =
            self.kind != ElementKind::Face && query.split_base_str() == query.as_str();
        let terms = if unsplit_list_query {
            query.face_terms()
        } else {
            Vec::new()
        };
        let mut descendants: Vec<&ElementName> = self
            .candidates
            .iter()
            .filter(|candidate| {
                candidate.name.is_split_descendant_of(query)
                    || (unsplit_list_query && adjacency_descends(&candidate.adjacency, &terms))
            })
            .map(|candidate| &candidate.name)
            .collect();
        if descendants.is_empty() {
            return None;
        }
        descendants.sort_unstable();
        descendants.dedup();
        Some(Resolution::Resolved {
            elements: descendants.into_iter().cloned().collect(),
            method: MatchMethod::History,
            confidence: 1.0,
        })
    }

    /// Step 3: the query's family, through aliases and un-splits.
    fn history(&self) -> Option<Resolution> {
        let ranked = self.ranked(self.family());
        match ranked.as_slice() {
            [] => None,
            [(only_score, only)] => Some(resolved(only, MatchMethod::History, *only_score)),
            _ => Some(choose(&ranked)),
        }
    }

    /// Steps 4 and 5: the fingerprint alone, or nothing.
    fn by_fingerprint(&self) -> Resolution {
        let ranked = self.ranked(self.candidates.iter().copied());
        let Some(&(best, best_name)) = ranked.first() else {
            return Resolution::Missing { nearest: None };
        };
        let (_, second) = top_two(&ranked);
        if best < CONFIDENCE_THRESHOLD {
            return Resolution::Missing {
                nearest: Some(candidate(&(best, best_name))),
            };
        }
        if best - second >= AMBIGUITY_MARGIN || ranked.len() == 1 {
            return resolved(best_name, MatchMethod::Fingerprint, best);
        }
        let floor = CONFIDENCE_THRESHOLD.max(best - AMBIGUITY_MARGIN);
        ambiguous(&ranked, floor)
    }

    /// The candidates in the query's family (section 3.9, "canonical base").
    fn family(&self) -> Vec<&'a ElementInfo> {
        let query = &self.reference.query;
        if self.kind == ElementKind::Face {
            let base = self.canonical_base(query);
            return self
                .candidates
                .iter()
                .copied()
                .filter(|candidate| candidate.name.split_base_str() == base.as_str())
                .collect();
        }
        let mut bases: Vec<ElementName> = query
            .face_terms()
            .iter()
            .map(|term| self.canonical_base(term))
            .collect();
        bases.sort_unstable();
        self.candidates
            .iter()
            .copied()
            .filter(|candidate| {
                let mut adjacent: Vec<&str> = candidate
                    .adjacency
                    .iter()
                    .map(ElementName::split_base_str)
                    .collect();
                adjacent.sort_unstable();
                adjacent.len() == bases.len()
                    && adjacent
                        .iter()
                        .zip(&bases)
                        .all(|(adjacent, base)| *adjacent == base.as_str())
            })
            .collect()
    }

    /// The split base of a face name with aliases followed.
    fn canonical_base(&self, face: &ElementName) -> ElementName {
        self.aliases.chase(&face.split_base())
    }

    /// The score of `candidate` against the reference fingerprint.
    fn score(&self, candidate: &ElementInfo) -> f64 {
        score(
            &self.reference.fingerprint,
            &candidate.fingerprint,
            self.kind,
        )
    }

    /// `members` scored, best first: by score descending, then by name.
    fn ranked(&self, members: impl IntoIterator<Item = &'a ElementInfo>) -> Vec<Scored<'a>> {
        let mut ranked: Vec<Scored<'a>> = members
            .into_iter()
            .map(|member| (self.score(member), &member.name))
            .collect();
        ranked.sort_by(|first, second| second.0.total_cmp(&first.0).then(first.1.cmp(second.1)));
        ranked
    }
}

/// The best and second best scores of a ranked list (0 when absent).
fn top_two(ranked: &[Scored<'_>]) -> (f64, f64) {
    let score_at = |position: usize| ranked.get(position).map_or(0.0, |(score, _)| *score);
    (score_at(0), score_at(1))
}

/// The best member when it beats the second by the margin, else the members within the margin of
/// the best.
fn choose(ranked: &[Scored<'_>]) -> Resolution {
    let (best, second) = top_two(ranked);
    match ranked.first() {
        Some((_, best_name)) if best - second >= AMBIGUITY_MARGIN => {
            resolved(best_name, MatchMethod::History, best)
        }
        _ => ambiguous(ranked, best - AMBIGUITY_MARGIN),
    }
}

/// `Resolved` with one element.
fn resolved(element: &ElementName, method: MatchMethod, confidence: f64) -> Resolution {
    Resolution::Resolved {
        elements: vec![element.clone()],
        method,
        confidence,
    }
}

/// `Ambiguous` with the ranked candidates scoring at least `floor`.
fn ambiguous(ranked: &[Scored<'_>], floor: f64) -> Resolution {
    Resolution::Ambiguous {
        candidates: ranked
            .iter()
            .filter(|(score, _)| *score >= floor)
            .map(candidate)
            .collect(),
    }
}

/// A scored name as a `Candidate`.
fn candidate(scored: &Scored<'_>) -> Candidate {
    Candidate {
        name: scored.1.clone(),
        score: scored.0,
    }
}

/// True when the adjacency names pair one-to-one with the face terms, each name equal to its term
/// or a strict split descendant of it, with at least one strict (section 3.9, step 2).
fn adjacency_descends(adjacency: &[ElementName], terms: &[ElementName]) -> bool {
    if adjacency.len() != terms.len() {
        return false;
    }
    // A paired name is never shorter than its term and is longer exactly when it is a strict
    // descendant. Both lists have the same total length when they hold the same names, so every
    // pairing is then all equal; when they differ, no pairing is all equal.
    let mut sorted_adjacency: Vec<&ElementName> = adjacency.iter().collect();
    sorted_adjacency.sort_unstable();
    let mut sorted_terms: Vec<&ElementName> = terms.iter().collect();
    sorted_terms.sort_unstable();
    sorted_adjacency != sorted_terms && has_perfect_matching(adjacency, terms)
}

/// True when every adjacency name can be paired with a distinct term it equals or descends from
/// (augmenting paths over the small bipartite graph).
fn has_perfect_matching(adjacency: &[ElementName], terms: &[ElementName]) -> bool {
    let mut owners: Vec<Option<usize>> = vec![None; terms.len()];
    (0..adjacency.len()).all(|name| {
        let mut visited = vec![false; terms.len()];
        augment(name, adjacency, terms, &mut owners, &mut visited)
    })
}

/// Tries to pair adjacency name `name` with a term, moving earlier pairings along an augmenting path.
fn augment(
    name: usize,
    adjacency: &[ElementName],
    terms: &[ElementName],
    owners: &mut [Option<usize>],
    visited: &mut [bool],
) -> bool {
    for term in 0..terms.len() {
        let compatible =
            adjacency[name] == terms[term] || adjacency[name].is_split_descendant_of(&terms[term]);
        if visited[term] || !compatible {
            continue;
        }
        visited[term] = true;
        let free = match owners[term] {
            None => true,
            Some(other) => augment(other, adjacency, terms, owners, visited),
        };
        if free {
            owners[term] = Some(name);
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::Fingerprint;

    fn name(text: &str) -> ElementName {
        ElementName::parse(text).expect("the test name is valid")
    }

    fn names(texts: &[&str]) -> Vec<ElementName> {
        texts.iter().map(|text| name(text)).collect()
    }

    fn info(name_text: &str, fingerprint_text: &str, adjacency: &[&str]) -> ElementInfo {
        let element = name(name_text);
        ElementInfo {
            kind: element.kind(),
            name: element,
            fingerprint: Fingerprint::parse(fingerprint_text).expect("valid fingerprint"),
            adjacency: names(adjacency),
        }
    }

    fn reference(query: &str, fingerprint_text: &str) -> Reference {
        Reference::new(
            name(query),
            Fingerprint::parse(fingerprint_text).expect("valid fingerprint"),
        )
        .expect("matching kinds")
    }

    #[test]
    fn adjacency_descends_with_one_split_term() {
        assert!(adjacency_descends(
            &names(&["f-a/x", "f-b/y#2"]),
            &names(&["f-a/x", "f-b/y"])
        ));
    }

    #[test]
    fn identical_adjacency_does_not_descend() {
        assert!(!adjacency_descends(
            &names(&["f-a/x", "f-b/y"]),
            &names(&["f-a/x", "f-b/y"])
        ));
    }

    #[test]
    fn adjacency_of_other_length_does_not_descend() {
        assert!(!adjacency_descends(
            &names(&["f-a/x#1", "f-a/x#2", "f-b/y"]),
            &names(&["f-a/x", "f-b/y"])
        ));
    }

    #[test]
    fn unrelated_adjacency_does_not_descend() {
        assert!(!adjacency_descends(
            &names(&["f-a/x#1", "f-c/z"]),
            &names(&["f-a/x", "f-b/y"])
        ));
    }

    #[test]
    fn matching_moves_earlier_pairings() {
        // f-a/x#1 fits both terms and is paired with f-a/x first; f-a/x#2 fits only f-a/x, so the
        // only valid pairing moves f-a/x#1 over to its equal term. First come, first served fails.
        assert!(adjacency_descends(
            &names(&["f-a/x#1", "f-a/x#2"]),
            &names(&["f-a/x", "f-a/x#1"])
        ));
    }

    #[test]
    fn matching_fails_when_two_names_need_one_term() {
        assert!(!adjacency_descends(
            &names(&["f-a/x#2", "f-a/x#3"]),
            &names(&["f-a/x", "f-a/x#1"])
        ));
    }

    #[test]
    fn top_two_of_empty_and_single_lists() {
        let only = name("f-a/x");

        assert_eq!(top_two(&[]), (0.0, 0.0));
        assert_eq!(top_two(&[(0.7, &only)]), (0.7, 0.0));
    }

    #[test]
    fn single_family_member_resolves_even_with_low_score() {
        let scope = vec![info("f-a/x", "plane|n=0,0,1|c=0,0,0|a=1", &[])];

        let resolution = resolve(
            &reference("f-a/x#3", "plane|n=0,0,-1|c=500,0,0|a=1000"),
            &scope,
            &Aliases::default(),
        );

        let Resolution::Resolved {
            elements, method, ..
        } = resolution
        else {
            panic!("expected a resolved reference, got {resolution:?}");
        };
        assert_eq!(elements, names(&["f-a/x"]));
        assert_eq!(method, MatchMethod::History);
    }

    #[test]
    fn edge_split_by_its_own_suffix_resolves_to_pieces() {
        let scope = vec![
            info(
                "edge(f-a/x|f-b/y)#1",
                "line|c=0,0,0|l=1",
                &["f-a/x", "f-b/y"],
            ),
            info(
                "edge(f-a/x|f-b/y)#2",
                "line|c=5,0,0|l=1",
                &["f-a/x", "f-b/y"],
            ),
        ];

        let resolution = resolve(
            &reference("edge(f-a/x|f-b/y)", "line|c=2,0,0|l=2"),
            &scope,
            &Aliases::default(),
        );

        assert_eq!(
            resolution,
            Resolution::Resolved {
                elements: names(&["edge(f-a/x|f-b/y)#1", "edge(f-a/x|f-b/y)#2"]),
                method: MatchMethod::History,
                confidence: 1.0
            }
        );
    }

    #[test]
    fn candidates_of_other_kinds_are_ignored() {
        let scope = vec![info("f-a/x", "plane|c=0,0,0", &[])];

        let resolution = resolve(
            &reference("vertex(f-a/x)", "point|c=0,0,0"),
            &scope,
            &Aliases::default(),
        );

        assert_eq!(resolution, Resolution::Missing { nearest: None });
    }

    #[test]
    fn fingerprint_ambiguity_respects_threshold() {
        let scope = vec![
            info("f-a/x", "plane|n=0,0,1|c=0,0,0|a=1", &[]),
            info("f-b/x", "plane|n=0,0,1|c=0,0,0.1|a=1", &[]),
            info("f-c/x", "plane|n=0,0,1|c=0,0,5|a=1", &[]),
        ];

        let resolution = resolve(
            &reference("f-q/x", "plane|n=0,0,1|c=0,0,0.05|a=1"),
            &scope,
            &Aliases::default(),
        );

        let Resolution::Ambiguous { candidates } = resolution else {
            panic!("expected an ambiguous reference, got {resolution:?}");
        };
        let candidate_names: Vec<&str> = candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect();
        assert_eq!(candidate_names, ["f-a/x", "f-b/x"]);
    }

    #[test]
    fn missing_serializes_without_nearest() {
        let json = serde_json::to_string(&Resolution::Missing { nearest: None })
            .expect("a resolution serializes");

        assert_eq!(json, r#"{"kind":"missing"}"#);
    }

    #[test]
    fn match_method_serializes_snake_case() {
        let json = serde_json::to_string(&MatchMethod::Fingerprint).expect("a method serializes");

        assert_eq!(json, "\"fingerprint\"");
    }
}
