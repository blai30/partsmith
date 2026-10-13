//! Canonical names (naming.md section 3.9, "Canonical names"): sets of forms, how they meet, and
//! the families of parts, matched through the indexes of a `NameSet`.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::chase::Links;
use crate::name::ElementName;
use crate::name_set::NameSet;
use crate::reference::{Merged, MergedName};
use crate::shape::{Shape, pairs_one_to_one};

/// A name in a set of forms, with the keys whose chase gave it.
#[derive(Debug, Clone)]
pub(crate) struct Form {
    /// The name taken apart.
    pub(crate) shape: Arc<Shape>,
    /// The alias and succession keys whose chase gave the name, each with the kinds of history on
    /// the way. Empty for a name taken as it is.
    pub(crate) via: Vec<(ElementName, Links)>,
}

/// The alias and succession keys a match went through, each with the kinds of history used.
pub(crate) type Via = BTreeMap<ElementName, Links>;

/// A skeleton with one set of forms per term.
#[derive(Debug, Clone)]
pub(crate) struct Pattern {
    /// Feature, role, generator kind and `@` groups.
    pub(crate) skeleton: Arc<str>,
    /// The forms of each term.
    pub(crate) terms: Vec<Forms>,
}

/// A set of forms: face names and patterns.
#[derive(Debug, Clone, Default)]
pub(crate) struct Forms {
    /// Distinct names, sorted by piece-free name, then by name.
    pub(crate) names: Vec<Form>,
    /// Patterns.
    pub(crate) patterns: Vec<Pattern>,
}

impl Forms {
    /// The number of forms.
    pub(crate) fn len(&self) -> usize {
        self.names.len() + self.patterns.len()
    }

    /// The names with this piece-free name.
    fn with_piece_free(&self, piece_free: &str) -> &[Form] {
        let start = self
            .names
            .partition_point(|form| form.shape.piece_free_text().as_ref() < piece_free);
        let length = self.names[start..]
            .iter()
            .take_while(|form| form.shape.piece_free_text().as_ref() == piece_free)
            .count();
        &self.names[start..start + length]
    }
}

/// Collects forms, keeping each name once with every key that gave it.
#[derive(Default)]
pub(crate) struct FormsBuilder {
    /// Names by text.
    names: BTreeMap<Arc<str>, Form>,
    /// Patterns.
    patterns: Vec<Pattern>,
}

impl FormsBuilder {
    /// Adds a name.
    pub(crate) fn add(&mut self, form: Form) {
        match self.names.get_mut(form.shape.text()) {
            Some(known) => {
                for entry in form.via {
                    if !known.via.contains(&entry) {
                        known.via.push(entry);
                    }
                }
            }
            None => {
                self.names.insert(Arc::clone(form.shape.text()), form);
            }
        }
    }

    /// Adds names.
    pub(crate) fn extend(&mut self, forms: impl IntoIterator<Item = Form>) {
        for form in forms {
            self.add(form);
        }
    }

    /// Adds a pattern.
    pub(crate) fn add_pattern(&mut self, pattern: Pattern) {
        self.patterns.push(pattern);
    }

    /// The set of forms.
    pub(crate) fn build(self) -> Forms {
        let mut names: Vec<Form> = self.names.into_values().collect();
        names.sort_by(|first, second| {
            first
                .shape
                .piece_free_text()
                .cmp(second.shape.piece_free_text())
                .then_with(|| first.shape.text().cmp(second.shape.text()))
        });
        Forms {
            names,
            patterns: self.patterns,
        }
    }
}

/// True when a name of one set extends a name of the other.
fn names_meet(first: &Forms, second: &Forms) -> bool {
    let (fewer, more) = if first.names.len() <= second.names.len() {
        (first, second)
    } else {
        (second, first)
    };
    fewer.names.iter().any(|form| {
        more.with_piece_free(form.shape.piece_free_text())
            .iter()
            .any(|other| form.shape.extends(&other.shape) || other.shape.extends(&form.shape))
    })
}

impl NameSet {
    /// The canonical set of a part, or of a face name nested in one, with the merged names
    /// `merged` records for it and for the face names nested in it (section 3.9).
    pub(crate) fn canonical(&self, name: &Shape, merged: &Merged) -> Forms {
        let mut forms = FormsBuilder::default();
        if name.is_flat() {
            if self.is_key(name.text()) {
                forms.extend(self.chase_forms(name));
            } else {
                forms.extend(self.chase_forms(&name.piece_free()));
            }
            forms.extend(self.chase_of_keys_extending(name));
        } else {
            let pattern = Pattern {
                skeleton: Arc::clone(name.skeleton().expect("a name with terms has a skeleton")),
                terms: name
                    .terms()
                    .iter()
                    .map(|term| self.canonical(term, merged))
                    .collect(),
            };
            for key in self.keys_with_skeleton(&pattern.skeleton) {
                if self.fits(key, &pattern) {
                    forms.extend(self.chase_forms(key));
                }
            }
            forms.add_pattern(pattern);
        }
        let recorded = merged
            .0
            .iter()
            .find(|(face, _)| face.as_str() == name.text().as_ref())
            .map(|(_, names)| names.as_slice())
            .unwrap_or_default();
        for merged_name in recorded {
            match merged_name {
                MergedName::Face(face) => {
                    forms.extend(self.merged_forms(&Shape::of(Arc::clone(face.shared_text()))));
                }
                MergedName::Copies { source, feature } => {
                    for copy in self.known_copies(source, feature.as_str()) {
                        forms.extend(self.merged_forms(&copy));
                    }
                }
            }
        }
        forms.build()
    }

    /// The forms of a merged name taken as written: its chase when it is a key, else the name, and
    /// the chase of every key that strictly extends it (section 3.9).
    fn merged_forms(&self, name: &Shape) -> Vec<Form> {
        let mut forms = if self.is_key(name.text()) {
            self.chase_forms(name)
        } else {
            vec![Form {
                shape: Arc::new(name.clone()),
                via: Vec::new(),
            }]
        };
        forms.extend(self.chase_of_keys_extending(name));
        forms
    }

    /// The chase of every key that strictly extends `name`.
    fn chase_of_keys_extending(&self, name: &Shape) -> Vec<Form> {
        self.keys_with_piece_free(name.piece_free_text())
            .filter(|key| key.text() != name.text() && key.extends(name))
            .flat_map(|key| self.chase_forms(key))
            .collect()
    }

    /// True when the name `name` fits `pattern`: it has the pattern's skeleton and terms that pair
    /// one to one with its term sets, the canonical set of each term meeting its set.
    pub(crate) fn fits(&self, name: &Shape, pattern: &Pattern) -> bool {
        if name.skeleton() != Some(&pattern.skeleton) {
            return false;
        }
        let terms = name.terms();
        let canonical: Vec<Forms> = terms
            .iter()
            .map(|term| self.canonical(term, &Merged::default()))
            .collect();
        pairs_one_to_one(canonical.len(), pattern.terms.len(), |term, set| {
            self.meet(&canonical[term], &pattern.terms[set])
        })
    }

    /// The pattern of a name with terms: its skeleton with the canonical set of each term.
    pub(crate) fn pattern_of(&self, name: &Shape) -> Pattern {
        Pattern {
            skeleton: Arc::clone(name.skeleton().expect("a name with terms has a skeleton")),
            terms: name
                .terms()
                .iter()
                .map(|term| self.canonical(term, &Merged::default()))
                .collect(),
        }
    }

    /// True when two patterns have the same skeleton and term sets that pair one to one, each pair
    /// meeting.
    pub(crate) fn patterns_meet(&self, first: &Pattern, second: &Pattern) -> bool {
        first.skeleton == second.skeleton
            && pairs_one_to_one(first.terms.len(), second.terms.len(), |left, right| {
                self.meet(&first.terms[left], &second.terms[right])
            })
    }

    /// True when two sets of forms meet (section 3.9): a name of one extends a name of the other, a
    /// name of one fits a pattern of the other, or they have patterns that meet.
    pub(crate) fn meet(&self, first: &Forms, second: &Forms) -> bool {
        names_meet(first, second)
            || self.a_name_fits(first, second)
            || self.a_name_fits(second, first)
            || first.patterns.iter().any(|left| {
                second
                    .patterns
                    .iter()
                    .any(|right| self.patterns_meet(left, right))
            })
    }

    /// True when a name of `names` fits a pattern of `patterns`.
    fn a_name_fits(&self, names: &Forms, patterns: &Forms) -> bool {
        !patterns.patterns.is_empty()
            && names.names.iter().any(|form| {
                !form.shape.is_flat()
                    && patterns
                        .patterns
                        .iter()
                        .any(|pattern| self.fits(&form.shape, pattern))
            })
    }

    /// The faces of the set whose forms meet `canonical`, as face ids in byte order of their names.
    pub(crate) fn family_of(&self, canonical: &Forms) -> Vec<u32> {
        let mut members = Vec::new();
        for form in &canonical.names {
            if form.shape.is_flat() {
                for &face in self.faces_with_piece_free(form.shape.piece_free_text()) {
                    let shape = self.face_shape(face);
                    if shape.is_flat() && (shape.extends(&form.shape) || form.shape.extends(shape))
                    {
                        members.push(face);
                    }
                }
            } else {
                self.push_faces_meeting(&self.pattern_of(&form.shape), &mut members);
            }
        }
        for pattern in &canonical.patterns {
            self.push_faces_meeting(pattern, &mut members);
        }
        members.sort_unstable();
        members.dedup();
        members
    }

    /// Adds the faces whose pattern meets `pattern`.
    fn push_faces_meeting(&self, pattern: &Pattern, members: &mut Vec<u32>) {
        let mut candidates = self.faces_with_term_like(pattern);
        candidates.sort_unstable();
        candidates.dedup();
        for face in candidates {
            let meets = self
                .face_pattern(face)
                .is_some_and(|face_pattern| self.patterns_meet(pattern, face_pattern));
            if meets {
                members.push(face);
            }
        }
    }

    /// The keys whose chase gave a form a face of the set was matched by, against `canonical`, with
    /// the kinds of history used: the "via" of section 3.9.
    pub(crate) fn matched_via(&self, face: u32, canonical: &Forms, via: &mut Via) {
        let face_forms = match self.face_pattern(face) {
            Some(pattern) => Forms {
                names: Vec::new(),
                patterns: vec![pattern.clone()],
            },
            None => Forms {
                names: vec![Form {
                    shape: Arc::clone(self.face_shape(face)),
                    via: Vec::new(),
                }],
                patterns: Vec::new(),
            },
        };
        self.collect_matches(&face_forms, canonical, via);
    }

    /// Adds the keys of the forms of `first` and `second` that meet each other.
    fn collect_matches(&self, first: &Forms, second: &Forms, via: &mut Via) {
        for left in &first.names {
            for right in second.with_piece_free(left.shape.piece_free_text()) {
                if left.shape.extends(&right.shape) || right.shape.extends(&left.shape) {
                    push_via(via, &left.via);
                    push_via(via, &right.via);
                }
            }
        }
        let first_patterns = self.patterns_with_names(first);
        let second_patterns = self.patterns_with_names(second);
        for (left, left_via) in &first_patterns {
            for (right, right_via) in &second_patterns {
                if !self.patterns_meet(left, right) {
                    continue;
                }
                push_via(via, left_via);
                push_via(via, right_via);
                for left_term in &left.terms {
                    for right_term in &right.terms {
                        if self.meet(left_term, right_term) {
                            self.collect_matches(left_term, right_term, via);
                        }
                    }
                }
            }
        }
    }

    /// The patterns of a set of forms and of its names with terms, each with the keys that gave it.
    fn patterns_with_names<'a>(
        &self,
        forms: &'a Forms,
    ) -> Vec<(Pattern, &'a [(ElementName, Links)])> {
        let mut patterns: Vec<(Pattern, &[(ElementName, Links)])> = forms
            .patterns
            .iter()
            .map(|pattern| (pattern.clone(), &[][..]))
            .collect();
        patterns.extend(
            forms
                .names
                .iter()
                .filter(|form| !form.shape.is_flat())
                .map(|form| (self.pattern_of(&form.shape), form.via.as_slice())),
        );
        patterns
    }
}

/// Adds keys and the kinds of history they used.
fn push_via(via: &mut Via, more: &[(ElementName, Links)]) {
    for (key, links) in more {
        let known = via.entry(key.clone()).or_default();
        *known = *known | *links;
    }
}
