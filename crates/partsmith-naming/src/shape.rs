//! Face names taken apart for matching (naming.md sections 3.1 and 3.9): the one scanner of a face
//! name (`FaceParts`, which the public `FaceName` view reads with too), piece-free names, the runs
//! of split numbers, generators, terms, skeletons and relatives.
//!
//! Trailing suffix groups after a face-name generator belong to the generator (section 3.1 rule 6:
//! the rest of the name is the generator, as written), so `f-sh1/rim/f-ext1/end-cap#2` has the
//! generator `f-ext1/end-cap#2`. After an edge or vertex generator they belong to the outer name,
//! so the `@` groups of `f-c/chamfer/edge(a|b)@f-p:1` are part of its skeleton.

use std::ops::Range;
use std::sync::Arc;

use crate::grammar;
use crate::roles;

/// A face name taken apart. The text must be a structurally valid face name; its face terms need
/// not be sorted (piece-free names of nested names may not be).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Shape {
    /// The name as written.
    text: Arc<str>,
    /// The name with every `#n` group removed, wherever it stands.
    piece_free: Arc<str>,
    /// Byte offset of the `/` after the feature id.
    feature_end: usize,
    /// Byte range of the role.
    role: Range<usize>,
    /// What the face was made from.
    source: Source,
    /// Byte offset where the name's own suffix groups start.
    groups_start: usize,
    /// The numbers of the name's own `#n` groups, in order, each with its run: the number of `@`
    /// groups before it.
    splits: Vec<(u32, u32)>,
    /// For a name with terms: its feature, role, generator kind and `@` groups.
    skeleton: Option<Arc<str>>,
}

/// A name that ends in an instance group, `<source>@<feature>:<instance>` (section 3.5.1), taken
/// apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InstanceName<'a> {
    /// The name the copy was made of.
    pub(crate) source: &'a str,
    /// The pattern or mirror feature that made the copy.
    pub(crate) feature: &'a str,
    /// The instance number, such as `2` or `1.3`.
    pub(crate) instance: &'a str,
}

impl<'a> InstanceName<'a> {
    /// `text` taken apart when its last group is an instance group with nothing after it (no `#n`
    /// follows); `None` otherwise.
    pub(crate) fn of(text: &'a str) -> Option<InstanceName<'a>> {
        let at = text.rfind('@')?;
        let group = &text[at..];
        if !grammar::is_instance(group) {
            return None;
        }
        let (feature, instance) = group[1..].split_once(':')?;
        Some(InstanceName {
            source: &text[..at],
            feature,
            instance,
        })
    }
}

/// What a face was made from.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    /// Nothing: `<feature>/<role>`.
    Nothing,
    /// A sketch reference, at this byte range of the name.
    Sketch(Range<usize>),
    /// A face name: the rest of the name.
    Face(Box<Shape>),
    /// An edge or vertex name: its face terms in written order.
    List(Vec<Shape>),
}

/// A face name read once (section 3.1): where its feature, role, generator and own suffix groups
/// are. `Shape::of` and the public `FaceName` view both read names through it.
pub(crate) struct FaceParts<'t> {
    /// Byte offset of the `/` after the feature id.
    pub(crate) feature_end: usize,
    /// Byte range of the role.
    pub(crate) role: Range<usize>,
    /// What the face was made from, as written after the role's `/`.
    pub(crate) generator: GeneratorParts<'t>,
    /// Byte offset where the name's own suffix groups start: the end of the name after a face-name
    /// generator, whose suffix groups belong to it (rule 6).
    pub(crate) groups_start: usize,
}

/// The generator of a face name as written: it starts after the role's `/` and ends where the
/// name's own suffix groups start.
pub(crate) enum GeneratorParts<'t> {
    /// None: `<feature>/<role>`.
    Nothing,
    /// A sketch reference.
    Sketch,
    /// A face name, which runs to the end of the name.
    Face,
    /// An edge or vertex name.
    List {
        /// `edge(` or `vertex(`.
        prefix: &'static str,
        /// Its face terms in written order.
        terms: Vec<&'t str>,
    },
}

impl FaceParts<'_> {
    /// Reads the face name `text`.
    pub(crate) fn of(text: &str) -> FaceParts<'_> {
        let bytes = text.as_bytes();
        let feature_end = text.find('/').unwrap_or(text.len());
        let role_start = (feature_end + 1).min(text.len());
        let role_end = role_start
            + bytes[role_start..]
                .iter()
                .take_while(|byte| !matches!(byte, b'/' | b'#' | b'@'))
                .count();
        let (generator, groups_start) = if bytes.get(role_end) == Some(&b'/') {
            generator_after(text, role_end + 1)
        } else {
            (GeneratorParts::Nothing, role_end)
        };
        FaceParts {
            feature_end,
            role: role_start..role_end,
            generator,
            groups_start,
        }
    }
}

/// The generator starting at byte `start` of the face name `text`, and the byte offset where the
/// name's own suffix groups start.
fn generator_after(text: &str, start: usize) -> (GeneratorParts<'_>, usize) {
    let rest = &text[start..];
    if let Some((_, prefix)) = grammar::leading_list_prefix(rest.as_bytes()) {
        let (terms, length) = list_terms(&rest[prefix.len()..]);
        let groups_start = start + prefix.len() + length + 1;
        return (GeneratorParts::List { prefix, terms }, groups_start);
    }
    let id_length = rest
        .bytes()
        .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        .count();
    if rest.as_bytes().get(id_length) == Some(&b'/') {
        return (GeneratorParts::Face, text.len());
    }
    let mut end = start + id_length;
    if text.as_bytes().get(end) == Some(&b'.') {
        end += 1 + text[end + 1..]
            .bytes()
            .take_while(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
            .count();
    }
    (GeneratorParts::Sketch, end)
}

impl Shape {
    /// Takes apart the face name `text`.
    pub(crate) fn of(text: Arc<str>) -> Shape {
        let parts = FaceParts::of(&text);
        let generator_start = parts.role.end + 1;
        let (source, list_prefix) = match parts.generator {
            GeneratorParts::Nothing => (Source::Nothing, None),
            GeneratorParts::Sketch => (Source::Sketch(generator_start..parts.groups_start), None),
            GeneratorParts::Face => {
                let generator = Shape::of(Arc::from(&text[generator_start..]));
                (Source::Face(Box::new(generator)), None)
            }
            GeneratorParts::List { prefix, terms } => {
                let terms = terms
                    .into_iter()
                    .map(|term| Shape::of(Arc::from(term)))
                    .collect();
                (Source::List(terms), Some(prefix))
            }
        };
        let groups_start = parts.groups_start;
        let splits = splits_of(&text[groups_start..]);
        let skeleton = match (&source, list_prefix) {
            (Source::Face(_), _) => Some(format!("{}<face>", &text[..generator_start])),
            (Source::List(_), Some(prefix)) => Some(format!(
                "{}{prefix}*){}",
                &text[..generator_start],
                piece_free_text(&text[groups_start..])
            )),
            _ => None,
        }
        .map(Arc::from);
        Shape {
            piece_free: shared_piece_free(&text),
            feature_end: parts.feature_end,
            role: parts.role,
            text,
            source,
            groups_start,
            splits,
            skeleton,
        }
    }

    /// The name as written.
    pub(crate) fn text(&self) -> &Arc<str> {
        &self.text
    }

    /// The name with every `#n` group removed.
    pub(crate) fn piece_free_text(&self) -> &Arc<str> {
        &self.piece_free
    }

    /// The feature id that starts the name.
    pub(crate) fn feature(&self) -> &str {
        &self.text[..self.feature_end]
    }

    /// The role.
    pub(crate) fn role(&self) -> &str {
        &self.text[self.role.clone()]
    }

    /// The sketch generator, if the name has one.
    pub(crate) fn sketch(&self) -> Option<&str> {
        match &self.source {
            Source::Sketch(range) => Some(&self.text[range.clone()]),
            _ => None,
        }
    }

    /// True for a fallback name: role `face` (section 3.6.14).
    pub(crate) fn is_fallback(&self) -> bool {
        self.role() == roles::FACE
    }

    /// True for a name without an element generator.
    pub(crate) fn is_flat(&self) -> bool {
        matches!(self.source, Source::Nothing | Source::Sketch(_))
    }

    /// The terms of a name with an element generator: the generator when it is a face name, else
    /// the face terms of the generator. Empty for a flat name.
    pub(crate) fn terms(&self) -> &[Shape] {
        match &self.source {
            Source::Face(generator) => std::slice::from_ref(generator),
            Source::List(terms) => terms,
            Source::Nothing | Source::Sketch(_) => &[],
        }
    }

    /// The face-name generator of a name such as a shell rim `<feature>/rim/<face>`.
    pub(crate) fn face_generator(&self) -> Option<&Shape> {
        match &self.source {
            Source::Face(generator) => Some(generator),
            _ => None,
        }
    }

    /// The skeleton of a name with terms (section 3.9): its feature, role, generator kind and `@`
    /// groups. `None` for a flat name.
    pub(crate) fn skeleton(&self) -> Option<&Arc<str>> {
        self.skeleton.as_ref()
    }

    /// The key relatives share (section 3.9 step 4): feature, sketch generator and `@` groups,
    /// without the role. `None` for names that have no relatives.
    pub(crate) fn relative_key(&self) -> Option<String> {
        if self.sketch().is_none() || self.is_fallback() {
            return None;
        }
        let role_end_in_piece_free = self.role.end;
        Some(format!(
            "{}{}",
            &self.piece_free[..=self.feature_end],
            &self.piece_free[role_end_in_piece_free..]
        ))
    }

    /// The same name with every `#n` group removed, taken apart.
    pub(crate) fn piece_free(&self) -> Shape {
        if self.text == self.piece_free {
            return self.clone();
        }
        Shape::of(Arc::clone(&self.piece_free))
    }

    /// True when `self` extends `other` (section 3.1): equal piece-free names, `other`'s split runs
    /// each a prefix of `self`'s, and terms that extend `other`'s one to one.
    pub(crate) fn extends(&self, other: &Shape) -> bool {
        if self.text == other.text {
            return true;
        }
        if self.piece_free != other.piece_free || !runs_extend(&self.splits, &other.splits) {
            return false;
        }
        match (&self.source, &other.source) {
            (Source::Face(longer), Source::Face(shorter)) => longer.extends(shorter),
            (Source::List(longer), Source::List(shorter)) => {
                pairs_one_to_one(longer.len(), shorter.len(), |left, right| {
                    longer[left].extends(&shorter[right])
                })
            }
            _ => true,
        }
    }

    /// The features and sketch generators the name depends on, with repetitions (section 2.2,
    /// `ElementName::generators`).
    pub(crate) fn generators(&self) -> Vec<(&str, Option<&str>)> {
        let mut found = vec![(self.feature(), self.sketch())];
        found.extend(
            instance_groups(&self.text[self.groups_start..])
                .map(|group| (group[1..].split(':').next().unwrap_or_default(), None)),
        );
        for term in self.terms() {
            found.extend(term.generators());
        }
        found
    }
}

/// The face terms of a list name's face list, which starts `text`, and the byte length of the list
/// up to its closing parenthesis.
fn list_terms(text: &str) -> (Vec<&str>, usize) {
    let mut terms = Vec::new();
    let mut depth = 0_usize;
    let mut start = 0;
    for (position, byte) in text.bytes().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' if depth == 0 => {
                terms.push(&text[start..position]);
                return (terms, position);
            }
            b')' => depth -= 1,
            b'|' if depth == 0 => {
                terms.push(&text[start..position]);
                start = position + 1;
            }
            _ => {}
        }
    }
    terms.push(&text[start..]);
    (terms, text.len())
}

/// The `@<feature>:<instance>` groups of a run of suffix groups, in order, each as written from its
/// `@`.
pub(crate) fn instance_groups(groups: &str) -> impl Iterator<Item = &str> {
    groups.match_indices('@').map(|(start, _)| {
        let group = &groups[start..];
        let end = group[1..]
            .find(['#', '@'])
            .map_or(group.len(), |end| end + 1);
        &group[..end]
    })
}

/// `text` with every `#n` group removed; shares `text` when it has none.
pub(crate) fn shared_piece_free(text: &Arc<str>) -> Arc<str> {
    if text.contains('#') {
        Arc::from(piece_free_text(text))
    } else {
        Arc::clone(text)
    }
}

/// The piece-free text of a name (section 3.1): `text` with every `#n` group removed, wherever it
/// stands.
pub fn piece_free_text(text: &str) -> String {
    let mut kept = String::with_capacity(text.len());
    let mut in_split = false;
    for character in text.chars() {
        match character {
            '#' => in_split = true,
            digit if in_split && digit.is_ascii_digit() => {}
            other => {
                in_split = false;
                kept.push(other);
            }
        }
    }
    kept
}

/// The split numbers of a run of suffix groups, each with its run index.
fn splits_of(groups: &str) -> Vec<(u32, u32)> {
    let mut splits = Vec::new();
    let mut run = 0;
    let bytes = groups.as_bytes();
    let mut position = 0;
    while position < bytes.len() {
        match bytes[position] {
            b'#' => {
                let digits = bytes[position + 1..]
                    .iter()
                    .take_while(|byte| byte.is_ascii_digit())
                    .count();
                let number = groups[position + 1..position + 1 + digits]
                    .parse()
                    .unwrap_or(u32::MAX);
                splits.push((run, number));
                position += 1 + digits;
            }
            b'@' => {
                run += 1;
                position += 1;
            }
            _ => position += 1,
        }
    }
    splits
}

/// True when every run of split numbers of `shorter` is a prefix of the same run of `longer`.
fn runs_extend(longer: &[(u32, u32)], shorter: &[(u32, u32)]) -> bool {
    let mut longer = longer.iter().peekable();
    let mut previous_run = None;
    for &(run, number) in shorter {
        if previous_run != Some(run) {
            // Skip the rest of the previous run of `longer`, and every earlier run.
            while longer
                .next_if(|&&(longer_run, _)| longer_run < run)
                .is_some()
            {}
            previous_run = Some(run);
        }
        match longer.next() {
            Some(&(longer_run, longer_number)) if longer_run == run && longer_number == number => {}
            _ => return false,
        }
    }
    true
}

/// True when every one of `left` items can be paired with a distinct one of `right` items for
/// which `pair(left, right)` holds, and both have the same count (augmenting paths).
pub(crate) fn pairs_one_to_one(
    left: usize,
    right: usize,
    pair: impl Fn(usize, usize) -> bool,
) -> bool {
    if left != right {
        return false;
    }
    let compatible: Vec<Vec<bool>> = (0..left)
        .map(|item| (0..right).map(|other| pair(item, other)).collect())
        .collect();
    let mut owners: Vec<Option<usize>> = vec![None; right];
    (0..left).all(|item| {
        let mut visited = vec![false; right];
        augment(item, &compatible, &mut owners, &mut visited)
    })
}

/// Pairs `item` with a free compatible partner, moving earlier pairings along an augmenting path.
fn augment(
    item: usize,
    compatible: &[Vec<bool>],
    owners: &mut [Option<usize>],
    visited: &mut [bool],
) -> bool {
    for partner in 0..owners.len() {
        if visited[partner] || !compatible[item][partner] {
            continue;
        }
        visited[partner] = true;
        let free = match owners[partner] {
            None => true,
            Some(other) => augment(other, compatible, owners, visited),
        };
        if free {
            owners[partner] = Some(item);
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(text: &str) -> Shape {
        Shape::of(Arc::from(text))
    }

    #[test]
    fn flat_name_has_feature_role_and_no_terms() {
        let flat = shape("f-ext1/end-cap#2@f-p:1#1");

        assert_eq!(flat.feature(), "f-ext1");
        assert_eq!(flat.role(), "end-cap");
        assert!(flat.is_flat());
        assert!(flat.terms().is_empty());
        assert_eq!(flat.piece_free_text().as_ref(), "f-ext1/end-cap@f-p:1");
    }

    #[test]
    fn sketch_generator_keeps_its_role() {
        let bore = shape("f-hole1/bore/e-1.center#2");

        assert_eq!(bore.sketch(), Some("e-1.center"));
        assert_eq!(bore.relative_key().as_deref(), Some("f-hole1//e-1.center"));
    }

    #[test]
    fn face_generator_takes_the_rest_of_the_name() {
        let rim = shape("f-sh1/rim/f-ext1/end-cap#2");

        let generator = rim.face_generator().expect("a face-name generator");

        assert_eq!(generator.text().as_ref(), "f-ext1/end-cap#2");
        assert_eq!(rim.skeleton().map(AsRef::as_ref), Some("f-sh1/rim/<face>"));
    }

    #[test]
    fn edge_generator_terms_and_skeleton_keep_instance_groups() {
        let chamfer = shape("f-c/chamfer/edge(f-a/x#1|f-b/y)@f-p:1#2");

        let terms: Vec<&str> = chamfer
            .terms()
            .iter()
            .map(|term| term.text().as_ref())
            .collect();

        assert_eq!(terms, ["f-a/x#1", "f-b/y"]);
        assert_eq!(
            chamfer.skeleton().map(AsRef::as_ref),
            Some("f-c/chamfer/edge(*)@f-p:1")
        );
        assert_eq!(chamfer.splits, [(1, 2)]);
    }

    #[test]
    fn extends_appends_to_runs_only() {
        assert!(shape("f-a/x#2#1").extends(&shape("f-a/x#2")));
        assert!(!shape("f-a/x#2#1").extends(&shape("f-a/x#1")));
        assert!(shape("f-a/x#1@f-p:1").extends(&shape("f-a/x@f-p:1")));
        assert!(!shape("f-a/x@f-p:1").extends(&shape("f-a/x")));
        assert!(!shape("f-a/x").extends(&shape("f-a/x#1")));
    }

    #[test]
    fn extends_pairs_generator_terms() {
        let piece = shape("f-c/chamfer/edge(f-a/x#1|f-b/y)");

        assert!(piece.extends(&shape("f-c/chamfer/edge(f-a/x|f-b/y)")));
        assert!(!piece.extends(&shape("f-c/chamfer/edge(f-a/x#2|f-b/y)")));
    }

    #[test]
    fn generators_list_features_sketches_and_copies() {
        let edge_face = shape("f-f/fillet/edge(f-e/lateral/e-2@f-lp1:1|f-box1/top)");

        assert_eq!(
            edge_face.generators(),
            [
                ("f-f", None),
                ("f-e", Some("e-2")),
                ("f-lp1", None),
                ("f-box1", None)
            ]
        );
    }

    #[test]
    fn an_instance_name_splits_at_its_last_group() {
        assert_eq!(
            InstanceName::of("f-a/x#1@f-m:1@f-lp1:0.3"),
            Some(InstanceName {
                source: "f-a/x#1@f-m:1",
                feature: "f-lp1",
                instance: "0.3",
            })
        );
        assert_eq!(InstanceName::of("f-a/x@f-lp1:2#1"), None);
        assert_eq!(
            InstanceName::of("f-c/chamfer/edge(f-a/x@f-lp1:2|f-b/y)"),
            None
        );
        assert_eq!(InstanceName::of("f-a/x"), None);
    }

    #[test]
    fn piece_free_names_drop_every_number() {
        assert_eq!(
            piece_free_text("f-ext1/end-cap#1@f-lp1:1#2"),
            "f-ext1/end-cap@f-lp1:1"
        );
        assert_eq!(piece_free_text("f-box1/top"), "f-box1/top");
    }

    #[test]
    fn runs_extend_compares_each_run() {
        assert!(runs_extend(&[(0, 2), (0, 1), (1, 3)], &[(0, 2), (1, 3)]));
        assert!(!runs_extend(&[(0, 2), (1, 3)], &[(0, 2), (0, 1)]));
        assert!(runs_extend(&[(1, 1)], &[]));
    }

    #[test]
    fn pairing_moves_earlier_pairs() {
        // Item 0 fits both partners and is paired with partner 0 first; item 1 fits only partner 0,
        // so the only valid pairing moves item 0 to partner 1.
        let fits = [[true, true], [true, false]];

        assert!(pairs_one_to_one(2, 2, |item, partner| fits[item][partner]));
        assert!(!pairs_one_to_one(2, 1, |_, _| true));
    }
}
