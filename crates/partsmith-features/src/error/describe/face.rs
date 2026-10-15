//! What one face of a described element reads as, and what the edge and vertex rules need to know about it
//! (features.md section 3.12).

use std::collections::HashMap;

use partsmith_model::Units;
use partsmith_naming::{ElementKind, ElementName, Fingerprint, GeometryKind};

use super::Word;
use super::maker::{
    Maker, NOT_IN_DOCUMENT, entity_geometry, entity_words, is_round, sketch_coordinates,
    sketch_point, world_coordinates,
};
use super::name::{FaceName, GeneratorText, piece_free};
use crate::context::BodyState;
use crate::error::message::Names;

/// The element being described.
pub(super) struct Element<'e> {
    /// Its name.
    pub(super) name: &'e ElementName,
    /// Its stored fingerprint; None for a part of a query described by its name alone.
    pub(super) fingerprint: Option<&'e Fingerprint>,
}

impl Element<'_> {
    /// The stored centroid, in world coordinates.
    pub(super) fn centroid(&self) -> Option<[f64; 3]> {
        self.fingerprint?.centroid
    }

    /// The stored geometry kind.
    pub(super) fn kind(&self) -> Option<GeometryKind> {
        self.fingerprint.map(|fingerprint| fingerprint.kind)
    }
}

/// The faces of the body state the elements are described in, by name.
pub(super) struct Faces {
    /// Each face's fingerprint.
    by_name: HashMap<String, Fingerprint>,
    /// The face names of each piece-free name.
    by_piece_free: HashMap<String, Vec<String>>,
}

impl Faces {
    /// The faces of `state`; none without a state or when its element infos cannot be read.
    pub(super) fn of(state: Option<&BodyState>) -> Faces {
        let mut faces = Faces {
            by_name: HashMap::new(),
            by_piece_free: HashMap::new(),
        };
        let Some(elements) = state.and_then(|state| state.elements().ok()) else {
            return faces;
        };
        for info in elements
            .iter()
            .filter(|info| info.kind == ElementKind::Face)
        {
            let name = info.name.to_string();
            faces
                .by_piece_free
                .entry(piece_free(&name))
                .or_default()
                .push(name.clone());
            faces.by_name.insert(name, info.fingerprint.clone());
        }
        faces
    }

    /// The fingerprint of the face named `name` in the state.
    pub(super) fn fingerprint_of(&self, name: &str) -> Option<&Fingerprint> {
        self.by_name.get(name)
    }
}

/// The sketch entity a lateral face was made from.
#[derive(Clone, Debug)]
pub(super) enum Entity {
    /// The sketch still has it: its words and the sketch's label in quotes.
    Known {
        /// `the line from (80, 40) to (70, 50)`.
        words: String,
        /// `'Sketch 1'`.
        sketch: String,
    },
    /// The sketch no longer has it.
    Deleted {
        /// `'Sketch 1'`.
        sketch: String,
    },
}

/// One face of a described element: its words and what the edge and vertex rules read.
pub(super) struct FaceFacts {
    /// The face's words, piece and copy included: `the left part of the top of 'Extrude 1'`.
    pub(super) text: String,
    /// The direction word: from its role for a primitive, else from its normal.
    pub(super) word: Option<Word>,
    /// Its outward normal in the result, when known.
    normal: Option<[f64; 3]>,
    /// True when its feature is not in the document.
    pub(super) missing: bool,
    /// True for a fallback name, which has no meaning beyond geometric order.
    pub(super) fallback: bool,
    /// Its feature's label in quotes.
    pub(super) owner: String,
    /// The copy clause: ` of the third copy of 'Linear pattern 1'`.
    pub(super) copies: String,
    /// Its feature id and copy groups: faces with equal keys belong to one feature.
    pub(super) feature_key: String,
    /// `the cut made by 'Extrude 2'` for a face a cut left.
    pub(super) cut: Option<String>,
    /// The sketch entity a side of an extrude or revolve was made from, when it has no direction word.
    entity: Option<Entity>,
    /// True when the face is round for the rim rule.
    pub(super) round: bool,
    /// What a rim on it belongs to: `the hole at (7, 0)`, `the cut made by 'Extrude 2'`, `'Extrude 2'`.
    pub(super) rim_owner: Option<String>,
}

impl FaceFacts {
    /// The facts of the face named `face`, which is `element` or one of its face terms.
    pub(super) fn of(
        element: &Element<'_>,
        face: &str,
        faces: &Faces,
        names: &dyn Names,
        units: Units,
    ) -> FaceFacts {
        let reader = FaceReader {
            element,
            faces,
            names,
            units,
        };
        reader.facts(face)
    }

    /// The word of the side of a cut this face is on: its result normal reversed, which points out of the cut.
    pub(super) fn cut_word(&self) -> Option<Word> {
        self.normal
            .and_then(|normal| Word::of(normal.map(|component| -component)))
    }

    /// The entity clause: ` {preposition} the line from (80, 50) to (50, 50) in 'Sketch 1'`.
    pub(super) fn entity_clause(&self, preposition: &str) -> Option<String> {
        match self.entity.as_ref()? {
            Entity::Known { words, sketch } => Some(format!(" {preposition} {words} in {sketch}")),
            Entity::Deleted { .. } => None,
        }
    }

    /// `along {entity} in '{sketch}'` for a face described by its entity, with `along a line of '{sketch}'` or
    /// `along a curve of '{sketch}'` (by the edge's stored kind) when the sketch no longer has the entity.
    pub(super) fn along_entity(&self, edge: Option<GeometryKind>) -> Option<String> {
        if self.word.is_some() || self.cut.is_some() {
            return None;
        }
        Some(match self.entity.as_ref()? {
            Entity::Known { words, sketch } => format!("along {words} in {sketch}"),
            Entity::Deleted { sketch } => {
                let kind = if edge == Some(GeometryKind::Line) {
                    "line"
                } else {
                    "curve"
                };
                format!("along a {kind} of {sketch}")
            }
        })
    }
}

/// Reads the facts of the faces of one element.
struct FaceReader<'r, 'e> {
    /// The element described.
    element: &'r Element<'e>,
    /// The faces of the state.
    faces: &'r Faces,
    /// Names, records and sketch results.
    names: &'r dyn Names,
    /// Document units.
    units: Units,
}

impl FaceReader<'_, '_> {
    /// The facts of the face named `face`.
    fn facts(&self, face: &str) -> FaceFacts {
        let Some(name) = FaceName::parse(face) else {
            return self.unknown(face);
        };
        let maker = Maker::of(self.names, name.feature);
        let copies = self.copy_clause(&name);
        let mut facts = FaceFacts {
            text: String::new(),
            word: None,
            normal: None,
            missing: !maker.exists(),
            fallback: name.role == "face",
            owner: maker.quoted(),
            copies,
            feature_key: format!("{}{}", name.feature, name.copy_text()),
            cut: maker
                .is_cut()
                .then(|| format!("the cut made by {}", maker.quoted())),
            entity: None,
            round: false,
            rim_owner: None,
        };
        if facts.missing {
            facts.text = "a face of a feature that is not in this document".to_string();
            return facts;
        }
        if facts.fallback {
            facts.text = format!("a face of {}{}", facts.owner, facts.copies);
            return facts;
        }
        let fingerprint = self.fingerprint(face);
        facts.normal = fingerprint
            .filter(|fingerprint| fingerprint.kind == GeometryKind::Plane)
            .and_then(|fingerprint| fingerprint.direction)
            .or_else(|| self.estimated_normal(face))
            .or_else(|| self.cap_normal(&maker, &name));
        facts.word = if maker.is_primitive() {
            Word::of_role(name.role)
        } else {
            facts.normal.and_then(Word::of)
        };
        facts.round = self.is_round(&maker, &name, fingerprint);
        facts.entity = self.entity(&maker, &name);
        facts.rim_owner = Some(self.rim_owner(&maker, &name, &facts));
        let base = self.base_text(&maker, &name, &facts, fingerprint);
        facts.text = format!(
            "{}{base}{}",
            self.piece_prefix(&name, fingerprint),
            facts.copies
        );
        facts
    }

    /// The facts of text that is not a face name.
    fn unknown(&self, face: &str) -> FaceFacts {
        FaceFacts {
            text: format!("a face of {NOT_IN_DOCUMENT}"),
            word: None,
            normal: None,
            missing: true,
            fallback: false,
            owner: NOT_IN_DOCUMENT.to_string(),
            copies: String::new(),
            feature_key: face.to_string(),
            cut: None,
            entity: None,
            round: false,
            rim_owner: None,
        }
    }

    /// The fingerprint of `face`: the element's own when it is the element, else the state's.
    fn fingerprint(&self, face: &str) -> Option<&Fingerprint> {
        if self.element.name.as_str() == face {
            self.element.fingerprint
        } else {
            self.faces.fingerprint_of(face)
        }
    }

    /// For a face term of an edge that is not in the state, the normal estimated from the edge (section 3.12): the
    /// edge's direction crossed with the other face's normal, pointing from that face's centroid toward the edge.
    fn estimated_normal(&self, face: &str) -> Option<[f64; 3]> {
        if self.element.name.kind() != ElementKind::Edge
            || self.faces.fingerprint_of(face).is_some()
        {
            return None;
        }
        let terms = self.element.name.face_terms();
        let [first, second] = terms.as_slice() else {
            return None;
        };
        let other = if first.as_str() == face {
            second
        } else {
            first
        };
        let other = self.faces.fingerprint_of(other.as_str())?;
        if other.kind != GeometryKind::Plane {
            return None;
        }
        let direction = self.element.fingerprint?.direction?;
        let edge_centroid = self.element.centroid()?;
        let normal = other.direction?;
        let face_centroid = other.centroid?;
        let crossed = cross(direction, normal);
        let length = dot(crossed, crossed).sqrt();
        if length < 1e-9 {
            return None;
        }
        let toward = [0, 1, 2].map(|i| edge_centroid[i] - face_centroid[i]);
        let side = dot(crossed, toward);
        if side.abs() < 1e-9 {
            return None;
        }
        let sign = side.signum() / length;
        Some(crossed.map(|component| component * sign))
    }

    /// For an extrude cap that is not in the state, the direction it faces in this run (section 3.12): an end cap
    /// along the extrude's direction (the sketch normal, reversed by Reverse except for a symmetric extent), a start
    /// cap against it. A cut leaves the other side of its tool's cap, so its result normal is reversed.
    fn cap_normal(&self, maker: &Maker<'_>, name: &FaceName<'_>) -> Option<[f64; 3]> {
        if !maker.is("extrude") || self.faces.fingerprint_of(name.text).is_some() {
            return None;
        }
        let sign = match name.role {
            "end-cap" => 1.0,
            "start-cap" => -1.0,
            _ => return None,
        };
        let sketch = self.names.sketch_result(&maker.sketch()?)?;
        let reverse = maker
            .param(&["reverse"])
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let symmetric = maker
            .param(&["extent", "kind"])
            .and_then(serde_json::Value::as_str)
            == Some("symmetric");
        let along = if reverse && !symmetric { -sign } else { sign };
        let side = if maker.is_cut() { -along } else { along };
        Some(sketch.placement.normal.map(|component| component * side))
    }

    /// True when the face is round for the rim rule (section 3.12): in the state, a cylinder, cone, sphere or torus;
    /// not in the state, a hole's bore, counterbore or countersink, or a side of an extrude or revolve whose entity the
    /// sketch still has as a circle or an arc.
    fn is_round(
        &self,
        maker: &Maker<'_>,
        name: &FaceName<'_>,
        fingerprint: Option<&Fingerprint>,
    ) -> bool {
        if let Some(fingerprint) = fingerprint {
            return matches!(
                fingerprint.kind,
                GeometryKind::Cylinder
                    | GeometryKind::Cone
                    | GeometryKind::Sphere
                    | GeometryKind::Torus
            );
        }
        if maker.is("hole") {
            return matches!(name.role, "bore" | "counterbore" | "countersink");
        }
        match (maker.is_sweep(), name.role, name.generator, maker.sketch()) {
            (true, "lateral", Some(GeneratorText::Sketch(entity)), Some(sketch)) => {
                entity_geometry(self.names, &sketch, entity)
                    .is_some_and(|geometry| is_round(&geometry))
            }
            _ => false,
        }
    }

    /// The entity a side of an extrude or revolve was made from.
    fn entity(&self, maker: &Maker<'_>, name: &FaceName<'_>) -> Option<Entity> {
        let (true, "lateral", Some(GeneratorText::Sketch(entity))) =
            (maker.is_sweep(), name.role, name.generator)
        else {
            return None;
        };
        let sketch_id = maker.sketch()?;
        let sketch = Maker::of(self.names, sketch_id.as_str()).quoted();
        Some(match entity_geometry(self.names, &sketch_id, entity) {
            Some(geometry) => Entity::Known {
                words: entity_words(&geometry, self.units),
                sketch,
            },
            None => Entity::Deleted { sketch },
        })
    }

    /// What a rim on the face belongs to.
    fn rim_owner(&self, maker: &Maker<'_>, name: &FaceName<'_>, facts: &FaceFacts) -> String {
        if maker.is("hole") {
            let hole = self.hole(maker, name);
            return match name.role {
                "counterbore" => format!("the counterbore of {hole}"),
                "countersink" => format!("the countersink of {hole}"),
                _ => hole,
            };
        }
        facts.cut.clone().unwrap_or_else(|| facts.owner.clone())
    }

    /// The hole a face of a hole feature belongs to: `the hole at (7, 0)`, or `a hole of 'Hole 1' near (7, 0, 0)` when
    /// the sketch no longer has its point, with the stored centroid of the element described.
    fn hole(&self, maker: &Maker<'_>, name: &FaceName<'_>) -> String {
        let point = match (name.generator, maker.sketch()) {
            (Some(GeneratorText::Sketch(point)), Some(sketch)) => {
                sketch_point(self.names, &sketch, point)
            }
            _ => None,
        };
        match (point, self.element.centroid()) {
            (Some(point), _) => format!("the hole at {}", sketch_coordinates(point, self.units)),
            (None, Some(centroid)) => format!(
                "a hole of {} near {}",
                maker.quoted(),
                world_coordinates(centroid, self.units)
            ),
            (None, None) => format!("a hole of {}", maker.quoted()),
        }
    }

    /// The face's own words, without piece and copy.
    fn base_text(
        &self,
        maker: &Maker<'_>,
        name: &FaceName<'_>,
        facts: &FaceFacts,
        fingerprint: Option<&Fingerprint>,
    ) -> String {
        let owner = &facts.owner;
        if let Some(cut) = &facts.cut {
            return self.cut_face_text(cut, name, facts);
        }
        match maker.kind() {
            Some("box") => match name.role {
                "front" | "back" | "left" | "right" => format!("the {} side of {owner}", name.role),
                "top" | "bottom" => format!("the {} of {owner}", name.role),
                _ => format!("a face of {owner}"),
            },
            Some("cylinder" | "cone") if matches!(name.role, "top" | "bottom" | "side") => {
                format!("the {} of {owner}", name.role)
            }
            Some("sphere" | "torus") if name.role == "surface" => {
                format!("the surface of {owner}")
            }
            Some("extrude" | "revolve") => self.sweep_face_text(name, facts, fingerprint),
            Some("fillet") => match name.role {
                "fillet" => format!("the rounded face of {owner}"),
                _ => format!("a corner face of {owner}"),
            },
            Some("chamfer") => match name.role {
                "chamfer" => format!("the beveled face of {owner}"),
                _ => format!("a corner face of {owner}"),
            },
            Some("shell") => self.shell_face_text(name, facts),
            Some("hole") => {
                let hole = self.hole(maker, name);
                match name.role {
                    "bore" => format!("the wall of {hole}"),
                    "counterbore" => format!("the counterbore of {hole}"),
                    "countersink" => format!("the countersink of {hole}"),
                    "bottom" => format!("the bottom of {hole}"),
                    "tip" => format!("the tip of {hole}"),
                    "counterbore-floor" => format!("the floor of the counterbore of {hole}"),
                    _ => format!("the opening of {hole}"),
                }
            }
            Some("split_body") => format!("the cut face of {owner}"),
            _ => format!("a face of {owner}"),
        }
    }

    /// A face of an extrude or revolve that is not a cut's: by its direction word, else by its entity.
    fn sweep_face_text(
        &self,
        name: &FaceName<'_>,
        facts: &FaceFacts,
        fingerprint: Option<&Fingerprint>,
    ) -> String {
        let owner = &facts.owner;
        if let Some(word) = facts.word {
            return worded_text(word, owner);
        }
        match (name.role, &facts.entity) {
            ("end-cap", _) => format!("the end face of {owner}"),
            ("start-cap", _) => format!("the start face of {owner}"),
            (_, Some(Entity::Known { words, sketch })) => {
                format!("the side of {owner} made from {words} in {sketch}")
            }
            (_, Some(Entity::Deleted { sketch })) => {
                let flat =
                    fingerprint.is_some_and(|fingerprint| fingerprint.kind == GeometryKind::Plane);
                let kind = if flat { "line" } else { "curve" };
                format!("the side of {owner} made from a {kind} of {sketch}")
            }
            (_, None) => format!("a face of {owner}"),
        }
    }

    /// A face a cut left: the floor (a cap, or any face whose result normal is +Z), the wall along its entity, or a
    /// wall.
    fn cut_face_text(&self, cut: &str, name: &FaceName<'_>, facts: &FaceFacts) -> String {
        let floor = matches!(name.role, "start-cap" | "end-cap")
            || facts.normal.and_then(Word::of) == Some(Word::Top);
        if floor {
            return format!("the floor of {cut}");
        }
        match &facts.entity {
            Some(Entity::Known { words, .. }) => format!("the wall of {cut} along {words}"),
            _ => format!("a wall of {cut}"),
        }
    }

    /// A face of a shell: an inside face by the word of the face it was offset from, or the rim.
    fn shell_face_text(&self, name: &FaceName<'_>, facts: &FaceFacts) -> String {
        let owner = &facts.owner;
        match name.role {
            "rim" => format!("the rim of {owner}"),
            "inner" => {
                let generator = match name.generator {
                    Some(GeneratorText::Element(generator)) => self.faces.fingerprint_of(generator),
                    _ => None,
                };
                let word = generator
                    .filter(|fingerprint| fingerprint.kind == GeometryKind::Plane)
                    .and_then(|fingerprint| fingerprint.direction)
                    .or_else(|| {
                        facts
                            .normal
                            .map(|normal| normal.map(|component| -component))
                    })
                    .and_then(Word::of);
                match word {
                    Some(Word::Bottom) => format!("the inside floor of {owner}"),
                    Some(Word::Top) => format!("the inside ceiling of {owner}"),
                    Some(word) => format!("the inside {} wall of {owner}", word.text()),
                    None => format!("an inside face of {owner}"),
                }
            }
            _ => format!("a face of {owner}"),
        }
    }

    /// `the left part of ` for a piece of a face with two pieces in the state, `a part of ` for another piece,
    /// nothing for a whole face. The axis is the one along which the two pieces' centroids differ most, and the side
    /// that of the described centroid against their middle, so a stored shape that now fits the sibling reads as the
    /// sibling's side.
    fn piece_prefix(&self, name: &FaceName<'_>, fingerprint: Option<&Fingerprint>) -> &'static str {
        if !name.piece {
            return "";
        }
        let siblings = self
            .faces
            .by_piece_free
            .get(&piece_free(name.text))
            .map_or(&[][..], Vec::as_slice);
        let [first, second] = siblings else {
            return "a part of ";
        };
        if first != name.text && second != name.text {
            return "a part of ";
        }
        let centroid = |name: &str| {
            self.faces
                .fingerprint_of(name)
                .and_then(|fingerprint| fingerprint.centroid)
        };
        let (Some(own), Some(first), Some(second)) = (
            fingerprint.and_then(|fingerprint| fingerprint.centroid),
            centroid(first),
            centroid(second),
        ) else {
            return "a part of ";
        };
        let axis = (0..3)
            .max_by(|&a, &b| {
                (first[a] - second[a])
                    .abs()
                    .total_cmp(&(first[b] - second[b]).abs())
            })
            .unwrap_or(0);
        let lower = own[axis] < (first[axis] + second[axis]) / 2.0;
        match (axis, lower) {
            (0, true) => "the left part of ",
            (0, false) => "the right part of ",
            (1, true) => "the front part of ",
            (1, false) => "the back part of ",
            (_, true) => "the lower part of ",
            (_, false) => "the upper part of ",
        }
    }

    /// The copy clause of a face of a pattern or mirror copy.
    fn copy_clause(&self, name: &FaceName<'_>) -> String {
        name.copies
            .iter()
            .map(|copy| {
                let maker = Maker::of(self.names, copy.feature);
                let owner = maker.quoted();
                if maker.is("mirror") {
                    format!(" of the mirror image made by {owner}")
                } else {
                    match copy.second {
                        Some(second) => format!(
                            " of the copy in row {}, column {} of {owner}",
                            u64::from(second) + 1,
                            u64::from(copy.first) + 1
                        ),
                        None => format!(
                            " of the {} copy of {owner}",
                            ordinal(u64::from(copy.first) + 1)
                        ),
                    }
                }
            })
            .collect()
    }
}

/// `the top of 'Extrude 1'`, or `the right side of 'Extrude 1'` for a vertical face.
fn worded_text(word: Word, owner: &str) -> String {
    match word {
        Word::Top | Word::Bottom => format!("the {} of {owner}", word.text()),
        _ => format!("the {} side of {owner}", word.text()),
    }
}

/// An ordinal number in words up to the twentieth, then in digits: `second`, `twenty-first` is `21st`.
fn ordinal(number: u64) -> String {
    const WORDS: [&str; 20] = [
        "first",
        "second",
        "third",
        "fourth",
        "fifth",
        "sixth",
        "seventh",
        "eighth",
        "ninth",
        "tenth",
        "eleventh",
        "twelfth",
        "thirteenth",
        "fourteenth",
        "fifteenth",
        "sixteenth",
        "seventeenth",
        "eighteenth",
        "nineteenth",
        "twentieth",
    ];
    if let Some(word) = number
        .checked_sub(1)
        .and_then(|index| WORDS.get(usize::try_from(index).ok()?))
    {
        return (*word).to_string();
    }
    let suffix = match (number % 10, number % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{number}{suffix}")
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
    use super::*;

    #[test]
    fn ordinals_are_words_then_digits() {
        assert_eq!(ordinal(2), "second");
        assert_eq!(ordinal(20), "twentieth");
        assert_eq!(ordinal(21), "21st");
        assert_eq!(ordinal(112), "112th");
        assert_eq!(ordinal(23), "23rd");
    }

    #[test]
    fn worded_faces_say_side_for_vertical_faces() {
        assert_eq!(
            worded_text(Word::Top, "'Extrude 1'"),
            "the top of 'Extrude 1'"
        );
        assert_eq!(
            worded_text(Word::Right, "'Extrude 1'"),
            "the right side of 'Extrude 1'"
        );
    }
}
