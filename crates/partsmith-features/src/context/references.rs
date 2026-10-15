//! Resolving stored references by meaning (features.md section 3.8): elements, follows, guesses, widenings and
//! losses with candidates a fix can store.

use std::collections::{BTreeSet, HashMap};

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
    /// Resolves `references` of `kind` in `body` (section 3.8), each lost or ambiguous one as a `ReferenceLost`
    /// warning. A list's entries are named `field[i]`, a single reference `field`. Candidates that `usable` rejects,
    /// and in a list those another entry resolves to, are dropped from every loss and guess. An empty body is
    /// `BodyEmpty` of the body field next to `field`.
    pub(super) fn resolve_references(
        &self,
        field: &str,
        body: &Id,
        references: &[Reference],
        kind: ElementKind,
        list: bool,
        usable: &Usable<'_>,
    ) -> Result<Resolved, FeatureError> {
        let state = self.body(body)?;
        if state.is_empty() {
            return Err(FeatureError::BodyEmpty {
                field: self.field(&body_field(field)),
                body: body.clone(),
                use_new: false,
            });
        }
        let set = state.element_set()?;
        let resolutions: Vec<Resolution> = references
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
        let resolver = EntryResolver {
            body,
            state,
            set: &set,
            infos: set
                .elements()
                .iter()
                .map(|info| (&info.name, info))
                .collect(),
            references,
            resolutions: &resolutions,
            usable,
        };
        let mut resolved = Resolved {
            elements: Vec::new(),
            sources: Vec::new(),
            warnings: Vec::new(),
            notices: Vec::new(),
        };
        for position in 0..references.len() {
            let entry_field = if list {
                self.field(&format!("{field}[{position}]"))
            } else {
                self.field(field)
            };
            resolver.entry(position, entry_field, &mut resolved)?;
        }
        Ok(resolved)
    }
}

/// Resolves the entries of one field.
struct EntryResolver<'r> {
    /// The body the references are resolved in.
    body: &'r Id,
    /// Its state just before the feature.
    state: &'r BodyState,
    /// The resolver's view of the state.
    set: &'r ElementSet,
    /// The element infos of the state by name.
    infos: HashMap<&'r ElementName, &'r ElementInfo>,
    /// The stored references.
    references: &'r [Reference],
    /// Their resolutions, in the same order.
    resolutions: &'r [Resolution],
    /// Whether the field can use a candidate.
    usable: &'r Usable<'r>,
}

impl EntryResolver<'_> {
    /// Adds the effect of the entry at `position` (section 3.8).
    fn entry(
        &self,
        position: usize,
        field: FieldRef,
        resolved: &mut Resolved,
    ) -> Result<(), FeatureError> {
        let reference = &self.references[position];
        match &self.resolutions[position] {
            Resolution::Resolved {
                elements,
                method,
                follow,
                via,
                alternatives,
                ..
            } => {
                let sub_shapes = self.sub_shapes(elements)?;
                for sub_shape in &sub_shapes {
                    if !resolved.elements.contains(sub_shape) {
                        resolved.elements.push(*sub_shape);
                        resolved.sources.push(position);
                    }
                }
                if *method == MatchMethod::Guess {
                    resolved.warnings.push(FeatureWarning::ReferenceGuessed {
                        field,
                        position,
                        body: self.body.clone(),
                        reference: reference.clone(),
                        elements: self.picked(elements),
                        alternatives: self.candidates(position, alternatives)?,
                    });
                    return Ok(());
                }
                let Some(follow) = *follow else {
                    return Ok(());
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
                            first: self.first_of_succession(elements, via),
                        });
                    }
                    Follow::Merged if self.every_merged_part_picked(position, elements) => {}
                    _ => resolved.notices.push(followed()),
                }
            }
            Resolution::Ambiguous { candidates } => {
                let loss = self.loss(position, true, Lost::default(), candidates)?;
                resolved
                    .warnings
                    .push(FeatureWarning::ReferenceLost { field, loss });
            }
            Resolution::Missing { lost, candidates } => {
                let loss = self.loss(position, false, lost.clone(), candidates)?;
                resolved
                    .warnings
                    .push(FeatureWarning::ReferenceLost { field, loss });
            }
        }
        Ok(())
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
            .filter_map(|name| self.infos.get(name))
            .map(|info| Reference::from_element(info, self.set))
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
            let Some(info) = self.infos.get(&candidate.name) else {
                continue;
            };
            if taken.contains(&candidate.name) || !(self.usable)(info)? {
                continue;
            }
            kept.push(LossCandidate {
                reference: Reference::from_element(info, self.set),
                meaning: candidate.meaning,
                distance: candidate.distance,
                score: candidate.score,
            });
        }
        Ok(kept)
    }

    /// The element made from the first entity of the succession of the first `via` key alone (section 3.8): the
    /// element that is, or touches, the first face of that succession; the first element when none does.
    fn first_of_succession(&self, elements: &[ElementName], via: &[ElementName]) -> Reference {
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
        chosen.and_then(|name| self.infos.get(name)).map_or_else(
            || self.references[0].clone(),
            |info| Reference::from_element(info, self.set),
        )
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
    let parts = faces_of(query);
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
    for face in elements.iter().flat_map(faces_of) {
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

/// The faces of an element name: the name itself for a face, its face terms for an edge or vertex.
fn faces_of(name: &ElementName) -> Vec<ElementName> {
    match name.kind() {
        ElementKind::Face => vec![name.clone()],
        ElementKind::Edge | ElementKind::Vertex => name.face_terms(),
    }
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
        GeometryKind::Line => {
            let Some(direction) = first.direction else {
                return false;
            };
            rest.iter().all(|other| {
                other.direction.is_some_and(|other_direction| {
                    parallel(direction, other_direction)
                        && distance_to_line(other.centroid, first.centroid, direction)
                            <= LINEAR_TOLERANCE
                })
            })
        }
        GeometryKind::Circle => same_round(first, rest, |other, center| {
            distance(other, center) <= LINEAR_TOLERANCE
        }),
        GeometryKind::Cylinder => {
            let Some(direction) = first.direction else {
                return false;
            };
            same_round(first, rest, |other, center| {
                distance_to_line(other, center, direction) <= LINEAR_TOLERANCE
            })
        }
        GeometryKind::Plane => {
            let Some(normal) = first.direction else {
                return false;
            };
            rest.iter().all(|other| {
                other.direction.is_some_and(|other_normal| {
                    normal.dot(other_normal) > 0.0
                        && parallel(normal, other_normal)
                        && offset(other.centroid, first.centroid).dot(normal).abs()
                            <= LINEAR_TOLERANCE
                })
            })
        }
        _ => false,
    }
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

/// The distance between two points.
fn distance(first: Point, second: Point) -> f64 {
    offset(first, second).length()
}

/// The distance from `point` to the line through `through` along `direction`.
fn distance_to_line(point: Point, through: Point, direction: Vector) -> f64 {
    let length = direction.length();
    if length == 0.0 {
        return distance(point, through);
    }
    offset(point, through).cross(direction).length() / length
}

#[cfg(test)]
mod tests {
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
    fn body_fields_sit_next_to_face_and_edge_fields() {
        assert_eq!(body_field("base.face"), "base.body");
        assert_eq!(body_field("axis.edge"), "axis.body");
        assert_eq!(body_field("edges"), "body");
    }
}
