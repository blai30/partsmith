//! Faces, edges and vertices described in words (features.md section 3.12, "Element descriptions").
//!
//! A face reads by the feature that made it and its role: a direction word for flat faces along a world axis, the
//! sketch entity a side was made from, the floor and walls of a cut, the point of a hole, a piece of a split face
//! and the copy of a pattern. An edge or vertex reads by its faces, with the first of the edge rules that applies.
//! Elements that would read the same in one call get their entity or position clause.

mod caps;
mod cuts;
mod face;
mod maker;
mod name;

use std::collections::HashMap;

use partsmith_model::{Id, Units};
use partsmith_naming::{ElementKind, ElementName, Fingerprint, GeometryKind, Instance, Reference};

use super::message::Names;
use crate::context::BodyState;
use face::{Element, FaceFacts, Faces, copy_noun};
use maker::{Maker, NOT_IN_DOCUMENT, world_coordinates};
use name::{FaceName, GeneratorText};

pub use maker::sketch_coordinates;

/// The words for the faces, edges or vertices of `body` one message or candidate list names, in order (section
/// 3.12): "the front top edge of 'Box 1'". Each element is given by its name and fingerprint; `state` is the state of
/// `body` the elements were resolved in, whose faces give direction words; elements that would read the same get
/// their entity or position clause.
pub fn describe_elements(
    elements: &[(&ElementName, &Fingerprint)],
    body: &Id,
    state: Option<&BodyState>,
    names: &dyn Names,
    units: Units,
) -> Vec<String> {
    let faces = Faces::of(state);
    let describer = Describer {
        body,
        faces: &faces,
        names,
        units,
    };
    let mut descriptions: Vec<Description> = elements
        .iter()
        .map(|(name, fingerprint)| describer.describe(name, Some(fingerprint)))
        .collect();
    give_clauses(&mut descriptions);
    descriptions
        .into_iter()
        .map(|description| description.text)
        .collect()
}

/// One element's words and the clauses that tell it apart from lookalikes.
struct Description {
    /// The words.
    text: String,
    /// The entity clause, for example ` along the line from (80, 50) to (50, 50) in 'Sketch 1'`.
    entity: Option<String>,
    /// The position clause, for example ` near (65, 0, 30)`.
    position: Option<String>,
}

/// Appends to each description that reads like another its entity clause, or else its position clause, and then
/// the position clause to any that still read alike.
fn give_clauses(descriptions: &mut [Description]) {
    for round in 0..2 {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for description in descriptions.iter() {
            *counts.entry(description.text.clone()).or_default() += 1;
        }
        for description in descriptions.iter_mut() {
            if counts[&description.text] < 2 {
                continue;
            }
            let entity = description
                .entity
                .take()
                .filter(|clause| round == 0 && !description.text.contains(clause.trim_start()));
            if let Some(clause) = entity.or_else(|| description.position.take()) {
                description.text.push_str(&clause);
            }
        }
    }
}

/// Describes elements of one body state with the document's names.
struct Describer<'a> {
    /// The body whose state is described.
    body: &'a Id,
    /// The faces of the state by name.
    faces: &'a Faces,
    /// Names, records and sketch results.
    names: &'a dyn Names,
    /// Document units.
    units: Units,
}

impl Describer<'_> {
    /// The words of one element.
    fn describe(&self, name: &ElementName, fingerprint: Option<&Fingerprint>) -> Description {
        let element = Element { name, fingerprint };
        let position = element
            .centroid()
            .map(|centroid| format!(" near {}", world_coordinates(centroid, self.units)));
        match name.kind() {
            ElementKind::Face => {
                let facts = self.face(&element, name.as_str());
                Description {
                    text: facts.text.clone(),
                    entity: facts.entity_clause("made from"),
                    position,
                }
            }
            ElementKind::Edge | ElementKind::Vertex => {
                let terms: Vec<FaceFacts> = name
                    .face_terms()
                    .iter()
                    .map(|term| self.face(&element, term.as_str()))
                    .collect();
                let text = if name.kind() == ElementKind::Edge {
                    self.edge(&element, &terms)
                } else {
                    self.vertex(&element, &terms)
                };
                Description {
                    text,
                    entity: terms.iter().find_map(|term| term.entity_clause("along")),
                    position,
                }
            }
        }
    }

    /// The facts of a face of `element` (the element itself, or one of its face terms).
    fn face(&self, element: &Element<'_>, face: &str) -> FaceFacts {
        FaceFacts::of(element, face, self)
    }

    /// An edge by the first rule of section 3.12 that applies.
    fn edge(&self, element: &Element<'_>, terms: &[FaceFacts]) -> String {
        if terms.iter().any(|term| term.missing) {
            return "an edge of a feature that is not in this document".to_string();
        }
        if terms.iter().any(|term| term.fallback) {
            return self.near(element, terms, "edge");
        }
        let [first, second] = terms else {
            return self.near(element, terms, "edge");
        };
        let pair = [first, second];
        // (1) Between two faces one cut left.
        if let (Some(cut), Some(other)) = (&first.cut, &second.cut)
            && cut == other
        {
            let words = words_text(pair.iter().filter_map(|face| face.cut_word()).collect());
            return match words {
                Some(words) => format!("the {words} edge of {cut}{}", first.copies),
                None => format!("an edge of {cut}{}", first.copies),
            };
        }
        // (2) A circle edge between a flat face with a direction word and a round face.
        if element.kind() == Some(GeometryKind::Circle) {
            for (flat, round) in [(first, second), (second, first)] {
                if let (Some(word), Some(owner)) = (flat.word, &round.rim_owner)
                    && round.round
                {
                    return format!("the {} rim of {owner}{}", word.text(), round.copies);
                }
            }
        }
        // (3) Between a face a cut left and a face of the body with a direction word.
        for (cut_face, body_face) in [(first, second), (second, first)] {
            if let (Some(cut), Some(word), None) = (&cut_face.cut, body_face.word, &body_face.cut) {
                return format!("the {} edge of {cut}{}", word.text(), cut_face.copies);
            }
        }
        // (4) Between two faces of one feature that both have direction words.
        if first.feature_key == second.feature_key
            && let Some(words) = all_words(&pair)
        {
            return format!("the {words} edge of {}{}", first.owner, first.copies);
        }
        // (5) Between a face with a direction word and a face described by its entity.
        for (worded, described) in [(first, second), (second, first)] {
            if let (Some(word), Some(along)) = (worded.word, described.along_entity(element.kind()))
            {
                return format!(
                    "the {} edge of {}{} {along}",
                    word.text(),
                    described.owner,
                    described.copies
                );
            }
        }
        // (6) Otherwise, the edge between its faces.
        format!("the edge between {} and {}", first.text, second.text)
    }

    /// A vertex: its three faces' words when they belong to one feature, else the faces that meet there; a vertex on
    /// one face, such as the pole of a sphere, by its position.
    fn vertex(&self, element: &Element<'_>, terms: &[FaceFacts]) -> String {
        if terms.iter().any(|term| term.missing) {
            return "a corner of a feature that is not in this document".to_string();
        }
        if terms.len() < 2 || terms.iter().any(|term| term.fallback) {
            return self.near(element, terms, "corner");
        }
        let one_feature = terms
            .windows(2)
            .all(|pair| pair[0].feature_key == pair[1].feature_key);
        if let (true, [first, ..]) = (one_feature, terms) {
            let faces: Vec<&FaceFacts> = terms.iter().collect();
            if let Some(words) = all_words(&faces) {
                return format!("the {words} corner of {}{}", first.owner, first.copies);
            }
        }
        let texts: Vec<String> = terms.iter().map(|term| term.text.clone()).collect();
        format!("the corner where {} meet", super::render::joined(&texts))
    }

    /// The position rule: `an edge of 'Box 1' near (40, 0, 30)`, `a round edge of 'Import 1' near (30, 10, 5)`.
    fn near(&self, element: &Element<'_>, terms: &[FaceFacts], noun: &str) -> String {
        let owner = terms
            .first()
            .map_or_else(|| NOT_IN_DOCUMENT.to_string(), |term| term.owner.clone());
        let article = if noun == "edge" && element.kind() == Some(GeometryKind::Circle) {
            "a round"
        } else if noun == "edge" {
            "an"
        } else {
            "a"
        };
        match element.centroid() {
            Some(centroid) => format!(
                "{article} {noun} of {owner} near {}",
                world_coordinates(centroid, self.units)
            ),
            None => format!("{article} {noun} of {owner}"),
        }
    }
}

/// The words for one face of `body` given by its name alone (`name` as text), described in `state`, the state of
/// `body`, as a stored element without a fingerprint: the part of a query a message names (`{face}` of section 4.2),
/// or a face recompute's loss diagnosis names (recompute.md section 3.10). Text that is no face name reads as a face of
/// a feature that is not in this document.
pub fn describe_face(
    name: &str,
    body: &Id,
    state: Option<&BodyState>,
    names: &dyn Names,
    units: Units,
) -> String {
    let Ok(name) = ElementName::parse(name) else {
        return format!("a face of {NOT_IN_DOCUMENT}");
    };
    let faces = Faces::of(state);
    let describer = Describer {
        body,
        faces: &faces,
        names,
        units,
    };
    describer.describe(&name, None).text
}

/// True when the stored fingerprint shows that the entity whose face `reference` names was a line (section 4.2,
/// `{line or curve}`): for a face query, a side of an extrude or revolve whose fingerprint is a plane; for an edge
/// query, an edge between such a side and a cap of the same feature whose fingerprint is a line.
pub fn made_from_line(reference: &Reference) -> bool {
    let query = &reference.query;
    let kind = reference.fingerprint.kind;
    match query.kind() {
        ElementKind::Face => {
            kind == GeometryKind::Plane
                && FaceName::parse(query.as_str()).is_some_and(|face| face.role == "lateral")
        }
        ElementKind::Edge => {
            let terms = query.face_terms();
            let faces: Vec<FaceName<'_>> = terms
                .iter()
                .filter_map(|term| FaceName::parse(term.as_str()))
                .collect();
            let [first, second] = faces.as_slice() else {
                return false;
            };
            let side_and_cap = |side: &FaceName<'_>, cap: &FaceName<'_>| {
                side.role == "lateral"
                    && matches!(cap.role, "start-cap" | "end-cap")
                    && side.feature == cap.feature
                    && side.copy_text() == cap.copy_text()
            };
            kind == GeometryKind::Line
                && (side_and_cap(first, second) || side_and_cap(second, first))
        }
        ElementKind::Vertex => false,
    }
}

/// The copy `instance` of the pattern or mirror `feature` in words, as section 3.12 names copies: `third copy` (the
/// ordinal of the first index plus 1), `copy in row 2, column 3` for two indices (row the second index plus 1, column
/// the first plus 1), or `mirror image` for a mirror.
pub fn describe_copy(names: &dyn Names, feature: &Id, instance: Instance) -> String {
    let second = (instance.second != 0).then_some(instance.second);
    copy_noun(
        Maker::of(names, feature.as_str()).is("mirror"),
        instance.first,
        second,
    )
}

/// Whether every new face of `elements` is flat in `state` (section 4.2, `{lines or curves}`): the elements
/// themselves for a face query, else their faces that are no face of the query. None when a new face is not in the
/// state.
pub(crate) fn new_faces_are_flat(
    reference: &Reference,
    elements: &[Reference],
    state: Option<&BodyState>,
) -> Option<bool> {
    let faces = Faces::of(state);
    let query_faces = reference.query.face_terms();
    let mut flat = true;
    for element in elements {
        let new_faces = match element.query.kind() {
            ElementKind::Face => vec![element.query.clone()],
            ElementKind::Edge | ElementKind::Vertex => element
                .query
                .face_terms()
                .into_iter()
                .filter(|face| !query_faces.contains(face))
                .collect(),
        };
        for face in new_faces {
            flat &= faces.fingerprint_of(face.as_str())?.kind == GeometryKind::Plane;
        }
    }
    Some(flat)
}

/// The name a shell rim continues: the generator of `<feature>/rim/<face>`, else None.
pub(crate) fn rim_generator(face: &str) -> Option<String> {
    let face = FaceName::parse(face)?;
    match (face.role, face.generator) {
        ("rim", Some(GeneratorText::Element(generator))) => Some(generator.to_string()),
        _ => None,
    }
}

/// The name with every `#n` group removed, wherever it stands.
pub(crate) fn piece_free_text(name: &str) -> String {
    name::piece_free(name)
}

/// A direction word of a flat face along a world axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Word {
    /// -Y.
    Front,
    /// +Y.
    Back,
    /// -X.
    Left,
    /// +X.
    Right,
    /// +Z.
    Top,
    /// -Z.
    Bottom,
}

/// Largest difference per component between a normal and a world axis that still gives a direction word.
const WORD_TOLERANCE: f64 = 1e-6;

impl Word {
    /// The word of an outward normal within `WORD_TOLERANCE` of a world axis.
    fn of(normal: [f64; 3]) -> Option<Word> {
        let words = [
            (Word::Right, [1.0, 0.0, 0.0]),
            (Word::Left, [-1.0, 0.0, 0.0]),
            (Word::Back, [0.0, 1.0, 0.0]),
            (Word::Front, [0.0, -1.0, 0.0]),
            (Word::Top, [0.0, 0.0, 1.0]),
            (Word::Bottom, [0.0, 0.0, -1.0]),
        ];
        words.into_iter().find_map(|(word, axis)| {
            normal
                .iter()
                .zip(axis)
                .all(|(component, wanted)| (component - wanted).abs() <= WORD_TOLERANCE)
                .then_some(word)
        })
    }

    /// The word of a primitive face's role.
    fn of_role(role: &str) -> Option<Word> {
        match role {
            "front" => Some(Word::Front),
            "back" => Some(Word::Back),
            "left" => Some(Word::Left),
            "right" => Some(Word::Right),
            "top" => Some(Word::Top),
            "bottom" => Some(Word::Bottom),
            _ => None,
        }
    }

    /// The word as written.
    fn text(self) -> &'static str {
        match self {
            Word::Front => "front",
            Word::Back => "back",
            Word::Left => "left",
            Word::Right => "right",
            Word::Top => "top",
            Word::Bottom => "bottom",
        }
    }

    /// The pair the word belongs to: front or back, then left or right, then top or bottom.
    fn axis(self) -> u8 {
        match self {
            Word::Front | Word::Back => 0,
            Word::Left | Word::Right => 1,
            Word::Top | Word::Bottom => 2,
        }
    }
}

/// The words of every face, ordered front or back, then left or right, then top or bottom; None when a face has no
/// word or two faces give words of one pair.
fn all_words(faces: &[&FaceFacts]) -> Option<String> {
    let words: Vec<Word> = faces.iter().map(|face| face.word).collect::<Option<_>>()?;
    let count = words.len();
    let text = words_text(words)?;
    (text.split(' ').count() == count).then_some(text)
}

/// Words in pair order, joined by spaces; None when there are none. Words of a pair already given are dropped.
fn words_text(mut words: Vec<Word>) -> Option<String> {
    words.sort_by_key(|word| word.axis());
    words.dedup_by_key(|word| word.axis());
    (!words.is_empty()).then(|| {
        words
            .iter()
            .map(|word| word.text())
            .collect::<Vec<_>>()
            .join(" ")
    })
}

/// The cross product.
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The dot product.
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[cfg(test)]
mod tests {
    use partsmith_model::FeatureRecord;

    use super::*;
    use crate::SketchResult;

    /// Names that know one record, `f-c` of type `kind`, labeled `Copies 1`.
    struct OneRecord {
        /// The record.
        record: FeatureRecord,
    }

    impl OneRecord {
        fn of(kind: &str) -> OneRecord {
            OneRecord {
                record: FeatureRecord {
                    id: Id::new("f-c").expect("a valid id"),
                    kind: kind.to_string(),
                    name: None,
                    suppressed: false,
                    params: serde_json::Map::new(),
                },
            }
        }
    }

    impl Names for OneRecord {
        fn body_name(&self, _body: &Id) -> Option<String> {
            None
        }

        fn feature_label(&self, feature: &Id) -> Option<String> {
            (*feature == self.record.id).then(|| "Copies 1".to_string())
        }

        fn type_label(&self, feature: &Id) -> Option<String> {
            (*feature == self.record.id).then(|| self.record.kind.clone())
        }

        fn feature_record(&self, feature: &Id) -> Option<&FeatureRecord> {
            (*feature == self.record.id).then_some(&self.record)
        }

        fn sketch_result(&self, _sketch: &Id) -> Option<&SketchResult> {
            None
        }
    }

    #[test]
    fn copies_are_described_by_their_place_or_as_the_mirror_image() {
        let feature = Id::new("f-c").expect("a valid id");
        let copy = |first: u32, second: u32| Instance { first, second };

        let pattern = OneRecord::of("linear_pattern");
        let mirror = OneRecord::of("mirror");

        assert_eq!(describe_copy(&pattern, &feature, copy(2, 0)), "third copy");
        assert_eq!(
            describe_copy(&pattern, &feature, copy(2, 1)),
            "copy in row 2, column 3"
        );
        assert_eq!(describe_copy(&mirror, &feature, copy(1, 0)), "mirror image");
    }

    #[test]
    fn a_face_given_by_name_alone_reads_by_its_feature() {
        let names = OneRecord::of("box");

        let body = Id::new("b-1").expect("a valid id");

        assert_eq!(
            describe_face("f-c/top", &body, None, &names, Units::Mm),
            "the top of 'Copies 1'"
        );
        assert_eq!(
            describe_face("f-gone/top", &body, None, &names, Units::Mm),
            "a face of a feature that is not in this document"
        );
    }

    #[test]
    fn a_face_given_by_name_alone_reads_by_the_face_of_that_name_in_the_state() {
        let names = OneRecord::of("extrude");
        let state = crate::test_support::box_body("f-c", [0.0, 0.0, 0.0], 10.0);
        let body = Id::new("b-1").expect("a valid id");

        assert_eq!(
            describe_face("f-c/top", &body, Some(&state), &names, Units::Mm),
            "the top of 'Copies 1'"
        );
    }

    #[test]
    fn a_side_made_from_a_line_shows_in_its_stored_fingerprint() {
        let reference = |query: &str, fingerprint: &str| -> Reference {
            serde_json::from_value(serde_json::json!({"q": query, "fp": fingerprint}))
                .expect("a valid reference")
        };

        assert!(made_from_line(&reference(
            "edge(f-ext1/end-cap|f-ext1/lateral/e-5)",
            "line|n=1,0,0|c=40,0,30|l=80"
        )));
        assert!(!made_from_line(&reference(
            "edge(f-ext1/end-cap|f-ext1/lateral/e-6)",
            "circle|n=0,0,1|c=78,48,10|l=7.853982|r=5"
        )));
    }

    #[test]
    fn sketch_coordinates_are_written_without_the_unit() {
        assert_eq!(sketch_coordinates([7.0, 0.0], Units::Mm), "(7, 0)");
    }

    #[test]
    fn words_come_from_normals_along_world_axes() {
        assert_eq!(Word::of([0.0, 0.0, 1.0]), Some(Word::Top));
        assert_eq!(Word::of([0.0, -1.0, 0.0000005]), Some(Word::Front));
        assert_eq!(Word::of([0.6, 0.8, 0.0]), None);
    }

    #[test]
    fn words_read_front_or_back_then_left_or_right_then_top_or_bottom() {
        assert_eq!(
            words_text(vec![Word::Top, Word::Left, Word::Front]).as_deref(),
            Some("front left top")
        );
        assert_eq!(words_text(Vec::new()), None);
    }

    #[test]
    fn clashing_descriptions_get_their_clauses() {
        let description = |entity: Option<&str>, position: &str| Description {
            text: "the back top edge of 'Extrude 1'".to_string(),
            entity: entity.map(ToString::to_string),
            position: Some(position.to_string()),
        };
        let mut descriptions = [
            description(
                Some(" along the line from (80, 50) to (50, 50) in 'Sketch 1'"),
                " near (65, 50, 30)",
            ),
            description(None, " near (15, 50, 30)"),
            Description {
                text: "the left top edge of 'Extrude 1'".to_string(),
                entity: Some(" along x".to_string()),
                position: None,
            },
        ];

        give_clauses(&mut descriptions);

        assert_eq!(
            descriptions[0].text,
            "the back top edge of 'Extrude 1' along the line from (80, 50) to (50, 50) in 'Sketch 1'"
        );
        assert_eq!(
            descriptions[1].text,
            "the back top edge of 'Extrude 1' near (15, 50, 30)"
        );
        assert_eq!(descriptions[2].text, "the left top edge of 'Extrude 1'");
    }
}
