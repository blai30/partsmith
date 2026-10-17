//! Resolving stored references by meaning (features.md section 3.8): elements, follows, guesses, widenings and
//! losses with candidates a fix can store.

use std::collections::BTreeSet;
use std::sync::Arc;

use partsmith_model::{Id, LINEAR_TOLERANCE};
use partsmith_naming::{
    Aliases, Candidate, ElementInfo, ElementKind, ElementName, ElementSet, Follow, Lost,
    MatchMethod, MergedName, Reference, Resolution,
};
use partsmith_occt_sys::{ElementGeometry, GeometryKind, Point, SubShape, Vector};

use super::{BodyState, BuildContext, Resolved};
use crate::error::{FeatureError, FeatureWarning, FieldRef, Loss, LossCandidate};
use crate::naming::sub_shape_kind;

/// Largest angle in radians between two directions that still agree (section 3.8, "one carrier").
const ANGLE_TOLERANCE: f64 = 1e-9;

/// Tells whether the field being resolved can use a candidate element (section 3.8): candidates it rejects are
/// dropped from losses and guesses, because a fix is offered only when it works.
pub type Usable<'u> = dyn Fn(&ElementInfo) -> Result<bool, FeatureError> + 'u;

impl BuildContext<'_> {
    /// The resolver of the entries of a reference field: `references` of `kind` resolved in `body` (section 3.8). An
    /// empty body is `BodyEmpty` of the body field next to `field`.
    pub(super) fn entry_resolver<'r>(
        &'r self,
        field: &str,
        body: &'r Id,
        references: &'r [Reference],
        kind: ElementKind,
        usable: &'r Usable<'r>,
    ) -> Result<EntryResolver<'r>, FeatureError> {
        let state = self.body(body)?;
        if state.is_empty() {
            return Err(FeatureError::BodyEmpty {
                field: self.field(&body_field(field)),
                body: body.clone(),
                use_new: false,
            });
        }
        let set = state.element_set()?;
        let resolutions = references
            .iter()
            .map(|reference| {
                if reference.query.kind() == kind {
                    partsmith_naming::resolve(reference, &set)
                } else {
                    Resolution::Missing {
                        lost: Lost::default(),
                        candidates: Vec::new(),
                    }
                }
            })
            .collect();
        Ok(EntryResolver {
            body,
            state,
            set,
            references,
            resolutions,
            usable,
        })
    }

    /// `Reference::from_element` of each of `elements`, kernel elements of `kind` that a reference resolved to in
    /// `body`, as a pick of each stores it, in name order: the faces of `NotCoplanar` and the elements of
    /// `NotCollinear` (section 3.6), which no name order may decide between.
    pub(super) fn picked_references(
        &self,
        body: &Id,
        kind: ElementKind,
        elements: &[SubShape],
    ) -> Result<Vec<Reference>, FeatureError> {
        let state = self.body(body)?;
        let set = state.element_set()?;
        let mut picked = elements
            .iter()
            .map(|element| {
                state
                    .naming()
                    .name(kind, element.index)
                    .and_then(|name| set.element(name))
                    .map(|info| Reference::from_element(info, &set))
                    .ok_or_else(|| {
                        FeatureError::internal(format!(
                            "the resolved {kind} {} has no element info",
                            element.index
                        ))
                    })
            })
            .collect::<Result<Vec<Reference>, FeatureError>>()?;
        picked.sort_by(|first, second| first.query.cmp(&second.query));
        Ok(picked)
    }
}

/// What one entry of a reference field resolved to (section 3.8).
pub(super) enum Outcome {
    /// Its elements, with the warning of a guess or a widening, or the notice of a follow.
    Resolved(Resolved),
    /// It is missing or ambiguous.
    Lost {
        /// The entry's field.
        field: FieldRef,
        /// The loss, boxed because it is much larger than a resolved entry.
        loss: Box<Loss>,
    },
}

impl Outcome {
    /// True when the entry gave no element.
    pub(super) fn has_no_elements(&self) -> bool {
        match self {
            Outcome::Resolved(resolved) => resolved.elements.is_empty(),
            Outcome::Lost { .. } => true,
        }
    }

    /// The loss of a lost entry.
    pub(super) fn into_loss(self) -> Option<Loss> {
        match self {
            Outcome::Resolved(_) => None,
            Outcome::Lost { loss, .. } => Some(*loss),
        }
    }
}

impl Resolved {
    /// Nothing resolved yet.
    fn empty() -> Resolved {
        Resolved {
            elements: Vec::new(),
            sources: Vec::new(),
            warnings: Vec::new(),
            notices: Vec::new(),
        }
    }

    /// The result of a field's entries, in entry order: their elements without duplicates, each with the first entry
    /// that resolved to it, and their warnings and notices, a lost entry's as its `ReferenceLost` warning.
    pub(super) fn of_entries(outcomes: Vec<Outcome>) -> Resolved {
        let mut resolved = Resolved::empty();
        for outcome in outcomes {
            match outcome {
                Outcome::Resolved(entry) => {
                    resolved.add_elements(entry.elements.into_iter().zip(entry.sources));
                    resolved.warnings.extend(entry.warnings);
                    resolved.notices.extend(entry.notices);
                }
                Outcome::Lost { field, loss } => {
                    resolved
                        .warnings
                        .push(FeatureWarning::ReferenceLost { field, loss: *loss });
                }
            }
        }
        resolved
    }

    /// Adds each element that is not in yet, with the position of the entry it was resolved from.
    fn add_elements(&mut self, elements: impl IntoIterator<Item = (SubShape, usize)>) {
        for (element, source) in elements {
            if !self.elements.contains(&element) {
                self.elements.push(element);
                self.sources.push(source);
            }
        }
    }
}

/// Resolves the entries of one field. Candidates that `usable` rejects, and in a list those another entry resolves
/// to, are dropped from every loss and guess.
pub(super) struct EntryResolver<'r> {
    /// The body the references are resolved in.
    body: &'r Id,
    /// Its state just before the feature.
    state: &'r BodyState,
    /// The resolver's view of the state.
    set: Arc<ElementSet>,
    /// The stored references.
    references: &'r [Reference],
    /// Their resolutions, in the same order.
    resolutions: Vec<Resolution>,
    /// Whether the field can use a candidate.
    usable: &'r Usable<'r>,
}

impl EntryResolver<'_> {
    /// What the entry at `position` resolves to, with `field` in its messages (section 3.8).
    pub(super) fn entry(&self, position: usize, field: FieldRef) -> Result<Outcome, FeatureError> {
        let reference = &self.references[position];
        match &self.resolutions[position] {
            Resolution::Resolved {
                elements,
                method,
                confidence,
                follow,
                via,
                alternatives,
            } => {
                let mut resolved = Resolved::empty();
                let sub_shapes = self.sub_shapes(elements)?;
                resolved.add_elements(sub_shapes.iter().map(|&sub_shape| (sub_shape, position)));
                if *method == MatchMethod::Guess {
                    resolved.warnings.push(FeatureWarning::ReferenceGuessed {
                        field,
                        position,
                        body: self.body.clone(),
                        reference: reference.clone(),
                        elements: self.picked(elements),
                        alternatives: self.candidates(position, alternatives)?,
                        score: *confidence,
                    });
                    return Ok(Outcome::Resolved(resolved));
                }
                let Some(follow) = *follow else {
                    return Ok(Outcome::Resolved(resolved));
                };
                let followed = || FeatureWarning::ReferenceFollowed {
                    field: field.clone(),
                    position,
                    body: self.body.clone(),
                    reference: reference.clone(),
                    elements: self.picked(elements),
                    follow,
                    via: via.clone(),
                };
                match follow {
                    Follow::Replaced if !self.on_one_carrier(&sub_shapes)? => {
                        resolved.warnings.push(FeatureWarning::ReferenceWidened {
                            field: field.clone(),
                            position,
                            body: self.body.clone(),
                            reference: reference.clone(),
                            elements: self.picked(elements),
                            via: via.clone(),
                            first: self.first_of_succession(elements, via)?,
                        });
                    }
                    Follow::Merged if self.every_merged_part_picked(position, elements) => {}
                    _ => resolved.notices.push(followed()),
                }
                Ok(Outcome::Resolved(resolved))
            }
            Resolution::Ambiguous { candidates } => Ok(Outcome::Lost {
                field,
                loss: Box::new(self.loss(position, true, Lost::default(), candidates)?),
            }),
            Resolution::Missing { lost, candidates } => Ok(Outcome::Lost {
                field,
                loss: Box::new(self.loss(position, false, lost.clone(), candidates)?),
            }),
        }
    }

    /// The kernel sub-shapes of resolved elements, in resolver order.
    fn sub_shapes(&self, elements: &[ElementName]) -> Result<Vec<SubShape>, FeatureError> {
        elements
            .iter()
            .map(|name| {
                let index = self.state.naming().index(name).ok_or_else(|| {
                    FeatureError::internal(format!("the resolved element {name} has no index"))
                })?;
                Ok(SubShape {
                    kind: sub_shape_kind(name.kind()),
                    index,
                })
            })
            .collect()
    }

    /// `Reference::from_element` of each element, as a pick of it stores it.
    fn picked(&self, elements: &[ElementName]) -> Vec<Reference> {
        elements
            .iter()
            .filter_map(|name| self.set.element(name))
            .map(|info| Reference::from_element(info, &self.set))
            .collect()
    }

    /// The loss of the reference at `position`, with the candidates the field can use.
    fn loss(
        &self,
        position: usize,
        ambiguous: bool,
        lost: Lost,
        candidates: &[Candidate],
    ) -> Result<Loss, FeatureError> {
        let reference = &self.references[position];
        Ok(Loss {
            position,
            body: self.body.clone(),
            reference: reference.clone(),
            kind: reference.query.kind(),
            ambiguous,
            lost,
            candidates: self.candidates(position, candidates)?,
            matches: if ambiguous { candidates.len() } else { 0 },
        })
    }

    /// The candidates of the entry at `position` the field can use and no other entry resolves to, each with the
    /// reference a pick of it stores, in candidate order.
    fn candidates(
        &self,
        position: usize,
        candidates: &[Candidate],
    ) -> Result<Vec<LossCandidate>, FeatureError> {
        let taken: BTreeSet<&ElementName> = self
            .resolutions
            .iter()
            .enumerate()
            .filter(|&(other, _)| other != position)
            .filter_map(|(_, resolution)| match resolution {
                Resolution::Resolved { elements, .. } => Some(elements),
                _ => None,
            })
            .flatten()
            .collect();
        let mut kept = Vec::new();
        for candidate in candidates {
            let Some(info) = self.set.element(&candidate.name) else {
                continue;
            };
            if taken.contains(&candidate.name) || !(self.usable)(info)? {
                continue;
            }
            kept.push(LossCandidate {
                reference: Reference::from_element(info, &self.set),
                meaning: candidate.meaning,
                distance: candidate.distance,
                score: candidate.score,
            });
        }
        Ok(kept)
    }

    /// The element made from the first entity of the succession of the first `via` key alone (section 3.8): the
    /// element that is, or touches, the first face of that succession; the first element when none does. Elements
    /// without an element info, which a resolution of the same set cannot give, are `Internal`.
    fn first_of_succession(
        &self,
        elements: &[ElementName],
        via: &[ElementName],
    ) -> Result<Reference, FeatureError> {
        let successions = &self.set.names().successions().0;
        let first_face = via
            .iter()
            .find_map(|key| successions.get(key))
            .and_then(|faces| faces.first());
        let chosen = first_face
            .and_then(|face| {
                elements.iter().find(|element| {
                    *element == face
                        || element.is_split_descendant_of(face)
                        || element.face_terms().contains(face)
                })
            })
            .or_else(|| elements.first());
        chosen
            .and_then(|name| self.set.element(name))
            .map(|info| Reference::from_element(info, &self.set))
            .ok_or_else(|| {
                FeatureError::internal("a widened reference resolved to no known element")
            })
    }

    /// True when the elements lie on one carrier (section 3.8): one element; lines on one line; circles on one
    /// circle; coplanar planes; or cylinders on one axis with one radius. Read from the kernel geometry.
    fn on_one_carrier(&self, elements: &[SubShape]) -> Result<bool, FeatureError> {
        if elements.len() <= 1 {
            return Ok(true);
        }
        let Some(shape) = self.state.shape() else {
            return Ok(false);
        };
        let geometries = elements
            .iter()
            .map(|element| Ok(shape.element(element.kind, element.index)?))
            .collect::<Result<Vec<ElementGeometry>, FeatureError>>()?;
        Ok(one_carrier(&geometries))
    }

    /// True for a merged follow whose every grown name is the query, or a face term of the query, of another
    /// reference of the same list: the user picked every part that merged (section 3.8, corpus R61 (c)).
    fn every_merged_part_picked(&self, position: usize, elements: &[ElementName]) -> bool {
        let grown = grown_names(
            self.set.names().aliases(),
            &self.references[position],
            elements,
        );
        !grown.is_empty()
            && grown.iter().all(|name| {
                self.references
                    .iter()
                    .enumerate()
                    .filter(|&(other, _)| other != position)
                    .any(|(_, other)| {
                        other.query == *name || other.query.face_terms().contains(name)
                    })
            })
    }
}

/// The body field next to a reference field (recompute.md section 3.10): the sibling `body` of a `face` or `edge`
/// field, such as `base.body` for `base.face`, else the feature's own `body`.
fn body_field(field: &str) -> String {
    match field
        .strip_suffix(".face")
        .or_else(|| field.strip_suffix(".edge"))
    {
        Some(parent) => format!("{parent}.body"),
        None => "body".to_string(),
    }
}

/// The names the grown faces of `elements` stand for that are neither parts of the reference's query nor its merged
/// names (naming.md section 3.9, "Follow"): for each face of the elements that is the target of a merge alias, its
/// own name and every alias key whose chase holds a name it extends.
fn grown_names(
    aliases: &Aliases,
    reference: &Reference,
    elements: &[ElementName],
) -> BTreeSet<ElementName> {
    let query = &reference.query;
    let parts = query.face_parts();
    let merged: Vec<&MergedName> = reference.merged.0.values().flatten().collect();
    let is_merged = |name: &ElementName| {
        merged.iter().any(|merged_name| match merged_name {
            MergedName::Face(face) => face == name,
            MergedName::Copies { source, feature } => {
                name.as_str().starts_with(&format!("{source}@{feature}:"))
            }
        })
    };
    let mut grown = BTreeSet::new();
    for face in elements.iter().flat_map(ElementName::face_parts) {
        let extends = |name: &ElementName| face == *name || face.is_split_descendant_of(name);
        let keys: Vec<&ElementName> = aliases
            .0
            .keys()
            .filter(|key| aliases.chase(key).iter().any(extends))
            .collect();
        if keys.is_empty() {
            continue;
        }
        grown.insert(face.clone());
        grown.extend(keys.into_iter().cloned());
    }
    grown.retain(|name| !parts.contains(name) && !is_merged(name));
    grown
}

/// True when the geometries lie on one carrier (section 3.8).
fn one_carrier(geometries: &[ElementGeometry]) -> bool {
    let Some((first, rest)) = geometries.split_first() else {
        return true;
    };
    let same_kind = rest.iter().all(|other| other.kind == first.kind);
    if !same_kind {
        return false;
    }
    match first.kind {
        GeometryKind::Line => rest.iter().all(|other| collinear(first, other)),
        GeometryKind::Circle => same_round(first, rest, |other, center| {
            other.distance(center) <= LINEAR_TOLERANCE
        }),
        GeometryKind::Cylinder => {
            let Some(direction) = first.direction else {
                return false;
            };
            same_round(first, rest, |other, center| {
                distance_to_line(other, center, direction) <= LINEAR_TOLERANCE
            })
        }
        GeometryKind::Plane => rest.iter().all(|other| coplanar(first, other)),
        _ => false,
    }
}

/// True when the line `other` lies on the line of `first` (section 3.6): directions within `ANGLE_TOLERANCE` up to
/// sign, and `other`'s centroid within `LINEAR_TOLERANCE` of `first`'s line. False when either has no direction.
pub(super) fn collinear(first: &ElementGeometry, other: &ElementGeometry) -> bool {
    match (first.direction, other.direction) {
        (Some(direction), Some(other_direction)) => {
            parallel(direction, other_direction)
                && distance_to_line(other.centroid, first.centroid, direction) <= LINEAR_TOLERANCE
        }
        _ => false,
    }
}

/// True when the plane `other` lies in the plane of `first` and faces the same way (section 3.6): outward normals
/// within `ANGLE_TOLERANCE`, and `other`'s centroid within `LINEAR_TOLERANCE` of `first`'s plane. False when either
/// has no normal.
pub(super) fn coplanar(first: &ElementGeometry, other: &ElementGeometry) -> bool {
    match (first.direction, other.direction) {
        (Some(normal), Some(other_normal)) => {
            normal.dot(other_normal) > 0.0
                && parallel(normal, other_normal)
                && offset(other.centroid, first.centroid).dot(normal).abs() <= LINEAR_TOLERANCE
        }
        _ => false,
    }
}

/// True when the axis through `other_through` along `other_direction` lies on the axis through `through` along
/// `direction` in the same sense (section 3.6): directions within `ANGLE_TOLERANCE` with a positive dot product,
/// and `other_through` within `LINEAR_TOLERANCE` of the first axis.
pub(super) fn same_axis(
    (through, direction): (Point, Vector),
    (other_through, other_direction): (Point, Vector),
) -> bool {
    direction.dot(other_direction) > 0.0
        && parallel(direction, other_direction)
        && distance_to_line(other_through, through, direction) <= LINEAR_TOLERANCE
}

/// True when every other circle or cylinder has a parallel axis, the first's radius, and a location `on_axis`
/// accepts against the first's location.
fn same_round(
    first: &ElementGeometry,
    rest: &[ElementGeometry],
    on_axis: impl Fn(Point, Point) -> bool,
) -> bool {
    let (Some(direction), Some(location), Some(radius)) =
        (first.direction, first.location, first.radius)
    else {
        return false;
    };
    rest.iter().all(
        |other| match (other.direction, other.location, other.radius) {
            (Some(other_direction), Some(other_location), Some(other_radius)) => {
                parallel(direction, other_direction)
                    && on_axis(other_location, location)
                    && (other_radius - radius).abs() <= LINEAR_TOLERANCE
            }
            _ => false,
        },
    )
}

/// True when two directions agree within `ANGLE_TOLERANCE`, up to sign.
fn parallel(first: Vector, second: Vector) -> bool {
    first.cross(second).length().atan2(first.dot(second).abs()) <= ANGLE_TOLERANCE
}

/// The vector from `from` to `to`.
fn offset(to: Point, from: Point) -> Vector {
    Vector::new(to.x - from.x, to.y - from.y, to.z - from.z)
}

/// The distance from `point` to the line through `through` along `direction`.
fn distance_to_line(point: Point, through: Point, direction: Vector) -> f64 {
    let length = direction.length();
    if length == 0.0 {
        return point.distance(through);
    }
    offset(point, through).cross(direction).length() / length
}

#[cfg(test)]
mod tests {
    use partsmith_naming::Successions;

    use super::*;

    fn line(direction: [f64; 3], centroid: [f64; 3]) -> ElementGeometry {
        ElementGeometry {
            kind: GeometryKind::Line,
            direction: Some(Vector::new(direction[0], direction[1], direction[2])),
            location: None,
            radius: None,
            centroid: Point::new(centroid[0], centroid[1], centroid[2]),
            measure: 1.0,
        }
    }

    #[test]
    fn collinear_lines_lie_on_one_carrier() {
        assert!(one_carrier(&[
            line([1.0, 0.0, 0.0], [20.0, 50.0, 30.0]),
            line([-1.0, 0.0, 0.0], [60.0, 50.0, 30.0]),
        ]));
    }

    #[test]
    fn parallel_lines_apart_do_not() {
        assert!(!one_carrier(&[
            line([1.0, 0.0, 0.0], [20.0, 50.0, 30.0]),
            line([1.0, 0.0, 0.0], [40.0, 40.0, 30.0]),
        ]));
    }

    #[test]
    fn a_widened_element_without_an_element_info_is_internal() {
        // An element the set has no info for cannot come from a resolution of that set; "Use only" never falls back
        // to another entry's reference.
        let set = ElementSet::new(
            Arc::from(Vec::new()),
            Aliases::default(),
            Successions::default(),
        )
        .expect("an empty element set");
        let state = BodyState::empty();
        let body = Id::new("b-1").expect("a valid id");
        let usable = |_: &ElementInfo| Ok(true);
        let resolver = EntryResolver {
            body: &body,
            state: &state,
            set: Arc::new(set),
            references: &[],
            resolutions: Vec::new(),
            usable: &usable,
        };
        let element = ElementName::parse("edge(f-ext1/end-cap|f-ext1/lateral/e-9)")
            .expect("a valid edge name");

        let error = resolver
            .first_of_succession(&[element], &[])
            .expect_err("no element info");

        assert!(matches!(error, FeatureError::Internal { .. }), "{error:?}");
    }

    #[test]
    fn body_fields_sit_next_to_face_and_edge_fields() {
        assert_eq!(body_field("base.face"), "base.body");
        assert_eq!(body_field("axis.edge"), "axis.body");
        assert_eq!(body_field("edges"), "body");
    }
}
