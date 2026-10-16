//! What one face of a described element reads as, and what the edge and vertex rules need to know about it
//! (features.md section 3.12).

use std::collections::HashMap;

use partsmith_model::{Id, Units};
use partsmith_naming::{
    ElementKind, ElementName, FaceGenerator, FaceName, Fingerprint, GeometryKind, piece_free_text,
};

use super::caps::cap_direction;
use super::cuts::cut_boolean;
use super::maker::{
    Maker, NOT_IN_DOCUMENT, entity_geometry, entity_words, is_round, sketch_coordinates,
    sketch_point, world_coordinates,
};
use super::{Describer, Word, cross, dot};
use crate::context::BodyState;
use crate::error::message::Names;
use crate::output::Absence;

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
                .entry(piece_free_text(&name))
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

    /// True when the state has the face named `name`, a piece of it, or the whole face it is a piece of.
    fn has_any_piece_of(&self, name: &str) -> bool {
        self.by_piece_free.contains_key(&piece_free_text(name))
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
    /// The facts of the face named `face`, which is `element` or one of its face terms, in the state `describer`
    /// describes.
    pub(super) fn of(
        element: &Element<'_>,
        face: &ElementName,
        describer: &Describer<'_>,
    ) -> FaceFacts {
        let reader = FaceReader {
            element,
            body: describer.body,
            before: describer.before,
            faces: describer.faces,
            names: describer.names,
            units: describer.units,
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
    /// The body whose state is described.
    body: &'r Id,
    /// The feature just before which the state is; None for the end of the timeline.
    before: Option<&'r Id>,
    /// The faces of the state.
    faces: &'r Faces,
    /// Names, records and sketch results.
    names: &'r dyn Names,
    /// Document units.
    units: Units,
}

impl FaceReader<'_, '_> {
    /// The facts of the face `face`.
    fn facts(&self, face: &ElementName) -> FaceFacts {
        let Some(name) = face.face_name() else {
            return self.unknown(face.as_str());
        };
        let maker = Maker::of(self.names, name.feature());
        let copies = self.copy_clause(&name);
        let copy_groups: String = name.copies().iter().map(|copy| copy.text).collect();
        let mut facts = FaceFacts {
            text: String::new(),
            word: None,
            normal: None,
            missing: !maker.exists(),
            fallback: name.role() == "face",
            owner: maker.quoted(),
            copies,
            feature_key: format!("{}{copy_groups}", name.feature()),
            cut: self.cut(&maker, name.feature()),
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
        let fingerprint = self.fingerprint(face.as_str());
        facts.normal = fingerprint
            .filter(|fingerprint| fingerprint.kind == GeometryKind::Plane)
            .and_then(|fingerprint| fingerprint.direction)
            .or_else(|| self.estimated_normal(face.as_str()))
            .or_else(|| self.cap_normal(&maker, face, name.role(), facts.cut.is_some()));
        facts.word = if maker.is_primitive() {
            Word::of_role(name.role())
        } else {
            facts.normal.and_then(Word::of)
        };
        facts.round = self.is_round(&maker, &name, fingerprint);
        facts.entity = self.entity(&maker, &name);
        facts.rim_owner = Some(self.rim_owner(&maker, &name, &facts));
        let base = self.base_text(&maker, &name, &facts, fingerprint);
        facts.text = format!(
            "{}{base}{}",
            self.piece_prefix(face, &name, fingerprint),
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

    /// The fingerprint of `face`: the element's own when it is the element, else the state's. A face given by its
    /// name alone has no fingerprint of its own, so it reads by the face of that name in the state.
    fn fingerprint(&self, face: &str) -> Option<&Fingerprint> {
        match self.element.fingerprint {
            Some(own) if self.element.name.as_str() == face => Some(own),
            _ => self.faces.fingerprint_of(face),
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

    /// `the cut made by 'Boolean 1'` when the faces of `feature` in the element read as faces a cut leaves (section
    /// 3.12): the tool faces of a cut boolean in the body described, else the faces of a cutting `maker`.
    fn cut(&self, maker: &Maker<'_>, feature: &str) -> Option<String> {
        let boolean = if self.all_in_state(feature) {
            cut_boolean(self.names, feature, self.body, self.before)
        } else {
            None
        };
        let cutter = match boolean {
            Some(boolean) => Maker::of(self.names, boolean.as_str()).quoted(),
            None if self.leaves_cut(maker, feature) => maker.quoted(),
            None => return None,
        };
        Some(format!("the cut made by {cutter}"))
    }

    /// True when the state has every face of `feature` the element names, whole or in pieces. A tool face of a cut
    /// boolean that the state does not have never was a face the cut leaves, so it, and an edge or vertex with it as a
    /// face term, reads as the tool's own (section 3.12, corpus R30 (b)).
    fn all_in_state(&self, feature: &str) -> bool {
        self.element.name.face_parts().iter().all(|face| {
            face.face_name()
                .is_none_or(|name| name.feature() != feature)
                || self.faces.has_any_piece_of(face.as_str())
        })
    }

    /// True when the faces of `feature` in the element read as faces its cut leaves (section 3.12): `maker` is a cut,
    /// and the element has no cap of it that it records absent with `EndOutside`. Such a cap never was a face the cut
    /// leaves, so it, and an edge or vertex with it as a face term, reads as the tool's own.
    fn leaves_cut(&self, maker: &Maker<'_>, feature: &str) -> bool {
        if !maker.is_cut() {
            return false;
        }
        let Ok(feature) = Id::new(feature) else {
            return true;
        };
        let absent = self.names.absent_faces(&feature);
        !self.element.name.face_parts().iter().any(|face| {
            let whole = face.piece_free();
            absent.iter().any(|entry| {
                entry.why == Absence::EndOutside
                    && (entry.name == *face || whole.as_ref() == Some(&entry.name))
            })
        })
    }

    /// For a cap of an extrude or revolve that is not in the state, the direction it faces in this run (section 3.12).
    /// A cut leaves the other side of its tool's cap (`cut`), so its result normal is reversed.
    fn cap_normal(
        &self,
        maker: &Maker<'_>,
        face: &ElementName,
        role: &str,
        cut: bool,
    ) -> Option<[f64; 3]> {
        if self.faces.fingerprint_of(face.as_str()).is_some() {
            return None;
        }
        let made = cap_direction(self.names, maker, role)?;
        Some(if cut {
            made.map(|component| -component)
        } else {
            made
        })
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
            return matches!(name.role(), "bore" | "counterbore" | "countersink");
        }
        match (
            maker.is_sweep(),
            name.role(),
            name.generator(),
            maker.sketch(),
        ) {
            (true, "lateral", Some(FaceGenerator::Sketch(entity)), Some(sketch)) => {
                entity_geometry(self.names, &sketch, entity)
                    .is_some_and(|geometry| is_round(&geometry))
            }
            _ => false,
        }
    }

    /// The entity a side of an extrude or revolve was made from.
    fn entity(&self, maker: &Maker<'_>, name: &FaceName<'_>) -> Option<Entity> {
        let (true, "lateral", Some(FaceGenerator::Sketch(entity))) =
            (maker.is_sweep(), name.role(), name.generator())
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
            return match name.role() {
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
        let point = match (name.generator(), maker.sketch()) {
            (Some(FaceGenerator::Sketch(point)), Some(sketch)) => {
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
        let role = name.role();
        match maker.kind() {
            Some("box") => match role {
                "front" | "back" | "left" | "right" => format!("the {role} side of {owner}"),
                "top" | "bottom" => format!("the {role} of {owner}"),
                _ => format!("a face of {owner}"),
            },
            Some("cylinder" | "cone") if matches!(role, "top" | "bottom" | "side") => {
                format!("the {role} of {owner}")
            }
            Some("sphere" | "torus") if role == "surface" => {
                format!("the surface of {owner}")
            }
            Some("extrude" | "revolve") => self.sweep_face_text(name, facts, fingerprint),
            Some("fillet") => match role {
                "fillet" => format!("the rounded face of {owner}"),
                _ => format!("a corner face of {owner}"),
            },
            Some("chamfer") => match role {
                "chamfer" => format!("the beveled face of {owner}"),
                _ => format!("a corner face of {owner}"),
            },
            Some("shell") => self.shell_face_text(name, facts),
            Some("hole") => {
                let hole = self.hole(maker, name);
                match role {
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
        match (name.role(), &facts.entity) {
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
        let floor = matches!(name.role(), "start-cap" | "end-cap")
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
        match name.role() {
            "rim" => format!("the rim of {owner}"),
            "inner" => {
                let generator = match name.generator() {
                    Some(FaceGenerator::Face(generator) | FaceGenerator::List(generator)) => {
                        self.faces.fingerprint_of(generator)
                    }
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
    fn piece_prefix(
        &self,
        face: &ElementName,
        name: &FaceName<'_>,
        fingerprint: Option<&Fingerprint>,
    ) -> &'static str {
        if !name.has_own_pieces() {
            return "";
        }
        let siblings = self
            .faces
            .by_piece_free
            .get(&piece_free_text(face.as_str()))
            .map_or(&[][..], Vec::as_slice);
        let [first, second] = siblings else {
            return "a part of ";
        };
        if first != face.as_str() && second != face.as_str() {
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
        name.copies()
            .iter()
            .map(|copy| {
                let maker = Maker::of(self.names, copy.feature);
                let owner = maker.quoted();
                let mirror = maker.is("mirror");
                let second = (copy.instance.second != 0).then_some(copy.instance.second);
                let noun = copy_noun(mirror, copy.instance.first, second);
                if mirror {
                    format!(" of the {noun} made by {owner}")
                } else {
                    format!(" of the {noun} of {owner}")
                }
            })
            .collect()
    }
}

/// A copy in words (section 3.12): `mirror image` for a mirror, `copy in row 2, column 3` with a second index, else
/// `third copy` (the ordinal of the first index plus 1).
pub(super) fn copy_noun(mirror: bool, first: u32, second: Option<u32>) -> String {
    if mirror {
        return "mirror image".to_string();
    }
    match second {
        Some(second) => format!(
            "copy in row {}, column {}",
            u64::from(second) + 1,
            u64::from(first) + 1
        ),
        None => format!("{} copy", ordinal(u64::from(first) + 1)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_read_as_ordinals_grid_places_or_mirror_images() {
        assert_eq!(copy_noun(false, 2, None), "third copy");
        assert_eq!(copy_noun(false, 2, Some(1)), "copy in row 2, column 3");
        assert_eq!(copy_noun(true, 1, None), "mirror image");
    }

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
