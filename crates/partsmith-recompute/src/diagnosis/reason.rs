//! Why a reference was lost, and which feature a followed reference followed (recompute.md section
//! 3.10, "The lost name", "Reason", "Ambiguous losses" and "Follows").

use std::collections::{BTreeMap, BTreeSet};

use partsmith_features::{BodyChange, BodyState, Loss};
use partsmith_model::Id;
use partsmith_naming::{ElementKind, ElementName, Follow, Merged, NameSet, NamingMap, Reference};

use super::face_name::{FaceName, Generator, copy_group, has_copy_group, maker_of, parts_of};
use super::memo::Probe;
use super::{LossReason, Removal};
use crate::graph::NodeState;

/// Feature types that make copies of a body (`CopyGone`).
const COPY_TYPES: [&str; 3] = ["linear_pattern", "circular_pattern", "mirror"];

impl<'r> Probe<'_, 'r> {
    /// The reason of a loss that is not ambiguous (rules 1 to 9).
    pub(crate) fn loss_reason(&mut self, loss: &Loss) -> LossReason {
        let lost = &loss.lost;
        let unmet = lost.parts.is_empty() && lost.pieces.is_empty() && lost.unmet;
        let lost_name = lost
            .parts
            .first()
            .or(lost.pieces.first())
            .or(unmet.then_some(&loss.reference.query));
        let Some(lost_name) = lost_name else {
            return LossReason::Unknown;
        };
        let maker = if unmet {
            self.latest_maker(&loss.reference.query)
        } else {
            maker_of(lost_name)
        };
        let Some(maker) = maker else {
            return LossReason::Unknown;
        };
        if !unmet && let Some(reason) = self.reason_before_the_remover(loss, lost_name, &maker) {
            return reason;
        }
        if let Some(reason) = self.removed(loss, lost_name, &maker, unmet) {
            return reason;
        }
        self.left_this_body(loss, &maker)
            .unwrap_or(LossReason::Unknown)
    }

    /// The feature after whose change an ambiguous loss became ambiguous: the feature that starts
    /// the first part of the query with a sketch generator and relatives in the body just before
    /// the feature explained, when it is active, comes before it and did not fail.
    pub(crate) fn ambiguous_after(&mut self, loss: &Loss) -> Option<Id> {
        let (state, _) = self.view.state_before(&loss.body, self.explained)?;
        let feature = parts_of(&loss.reference.query)
            .into_iter()
            .find_map(|part| {
                let face = FaceName::of(&part)?;
                let generator = face.sketch_generator()?;
                has_relatives(state.naming(), &face, generator)
                    .then(|| Id::new(face.feature).ok())?
            })?;
        let fact = self.record(&feature);
        (fact.state == Some(NodeState::Active) && fact.before && !fact.failed).then_some(feature)
    }

    /// The feature a followed or widened reference followed (section 3.10, "Follows").
    pub(crate) fn followed_by(
        &mut self,
        body: &Id,
        query: &ElementName,
        follow: Option<Follow>,
        via: &[ElementName],
        elements: &[Reference],
    ) -> Option<Id> {
        match follow {
            None | Some(Follow::Replaced | Follow::Extended) => {
                let writer = self.first_writer_recording(body, |map| {
                    map.successions().0.keys().any(|key| via.contains(key))
                })?;
                let sketch = self.view.first_sketch(writer)?.clone();
                self.record(&sketch);
                Some(sketch)
            }
            Some(Follow::Merged) => {
                let writer = self.first_writer_recording(body, |map| {
                    map.aliases().0.keys().any(|key| via.contains(key))
                })?;
                Some(self.view.id(writer).clone())
            }
            Some(Follow::Continued) => {
                let rim = elements
                    .iter()
                    .flat_map(|element| parts_of(&element.query))
                    .find_map(|face| {
                        let parts = FaceName::of(&face)?;
                        (parts.role == "rim").then(|| Id::new(parts.feature).ok())?
                    })?;
                self.record(&rim);
                Some(rim)
            }
            Some(Follow::Related) => {
                let (state, _) = self.view.state_before(body, self.explained)?;
                let names = state.name_set();
                let part = parts_of(query)
                    .into_iter()
                    .find(|part| names.family(part, &Merged::default()).is_empty())?;
                let feature = Id::new(FaceName::of(&part)?.feature).ok()?;
                self.record(&feature);
                Some(feature)
            }
        }
    }

    /// The latest in timeline order of the features that make the face terms of `query`.
    fn latest_maker(&mut self, query: &ElementName) -> Option<Id> {
        let makers: Vec<Id> = parts_of(query).iter().filter_map(maker_of).collect();
        makers
            .iter()
            .filter_map(|maker| Some((self.view.position(maker)?, maker)))
            .max_by_key(|(position, _)| *position)
            .map(|(_, maker)| maker.clone())
            .or_else(|| makers.first().cloned())
    }

    /// Rules 1 to 6: what the record that made the lost name, the other bodies, its copies, its
    /// sketch and its file say.
    fn reason_before_the_remover(
        &mut self,
        loss: &Loss,
        lost_name: &ElementName,
        maker: &Id,
    ) -> Option<LossReason> {
        let fact = self.record(maker);
        let feature = maker.clone();
        match fact.state {
            None => return Some(LossReason::NotInDocument { feature }),
            Some(NodeState::Suppressed) => return Some(LossReason::Suppressed { feature }),
            Some(NodeState::RolledBack) if fact.after_marker => {
                return Some(LossReason::RolledBack { feature });
            }
            _ => {}
        }
        if !fact.before {
            return Some(LossReason::ComesLater { feature });
        }
        if fact.failed {
            let root = self.root_of(maker);
            return Some(LossReason::UpstreamFailed { feature, root });
        }
        let position = self.view.position(maker)?;
        self.in_other_body(loss)
            .or_else(|| self.copy_gone(position, lost_name))
            .or_else(|| self.sketch_generator_gone(position, lost_name))
            .or_else(|| {
                let fallback = FaceName::of(lost_name).is_some_and(|face| face.is_fallback());
                (self.view.kind(position) == "import" && fallback).then(|| LossReason::NotInFile {
                    feature: maker.clone(),
                })
            })
    }

    /// The first feature of the failure chain that starts at `feature` whose error is its own:
    /// from a feature whose error is `UpstreamFailed { feature: h }`, the chain moves on to `h`.
    pub(crate) fn root_of(&mut self, feature: &Id) -> Id {
        let mut current = feature.clone();
        let mut seen = BTreeSet::new();
        while let Some(next) = self.record(&current).upstream {
            if !seen.insert(current.clone()) {
                break;
            }
            current = next;
        }
        current
    }

    /// Rule 3: every part of the query is present by name in another body just before the feature
    /// explained (and, for an edge or vertex, an element of that body joins them).
    fn in_other_body(&mut self, loss: &Loss) -> Option<LossReason> {
        let query = &loss.reference.query;
        let parts = parts_of(query);
        let others = self.bodies().into_iter().filter(|body| *body != loss.body);
        for body in others {
            let Some(state) = self.state_before(&body, self.explained) else {
                continue;
            };
            let names = state.name_set();
            let families = families(names, &parts, &loss.reference.merged);
            if families.iter().any(BTreeSet::is_empty) {
                continue;
            }
            if query.kind() != ElementKind::Face && !joins(state.naming(), query.kind(), &families)
            {
                continue;
            }
            let joiner = self.joiner(&loss.body, &body);
            let by_suppressed = joiner
                .as_ref()
                .is_some_and(|by| self.record(by).state == Some(NodeState::Suppressed));
            return Some(LossReason::OtherBody {
                body,
                by: joiner,
                by_suppressed,
                present: true,
            });
        }
        None
    }

    /// Rule 4: the pattern or mirror that made the lost name no longer makes its copy.
    fn copy_gone(&self, maker: usize, lost_name: &ElementName) -> Option<LossReason> {
        if !COPY_TYPES.contains(&self.view.kind(maker)) {
            return None;
        }
        let feature = self.view.id(maker);
        let (copier, instance) = *FaceName::of(lost_name)?.copies.last()?;
        if copier != feature.as_str() {
            return None;
        }
        let group = copy_group(feature, instance);
        let made = self.view.updates(maker).iter().any(|update| {
            update
                .naming
                .names(ElementKind::Face)
                .any(|(_, face)| has_copy_group(face.as_str(), &group))
        });
        (!made).then(|| LossReason::CopyGone {
            feature: feature.clone(),
            instance,
        })
    }

    /// Rule 5: the sketch entity or hole point the lost name was made from is gone.
    fn sketch_generator_gone(
        &mut self,
        maker: usize,
        lost_name: &ElementName,
    ) -> Option<LossReason> {
        let generator = FaceName::of(lost_name)?.sketch_generator()?;
        let sketch = self.view.first_sketch(maker)?.clone();
        let sketch_position = self.position(&sketch)?;
        let result = self.view.sketch_result(sketch_position)?;
        let feature = self.view.id(maker).clone();
        if self.view.kind(maker) == "hole" {
            if super::point_in(result, generator).is_none() {
                return Some(LossReason::PointGone {
                    feature,
                    sketch,
                    point: generator.to_string(),
                    deleted: true,
                });
            }
            let listed = self
                .view
                .params(maker)
                .and_then(|params| params.get("points"))
                .and_then(serde_json::Value::as_array)
                .is_some_and(|points| points.iter().any(|point| point.as_str() == Some(generator)));
            return (!listed).then(|| LossReason::PointGone {
                feature,
                sketch,
                point: generator.to_string(),
                deleted: false,
            });
        }
        let entity = generator.split('.').next().unwrap_or(generator);
        let present = result
            .solved
            .entities
            .iter()
            .any(|solved| solved.id.as_str() == entity);
        if present {
            return None;
        }
        Some(LossReason::EntityDeleted {
            feature,
            sketch,
            entity: Id::new(entity).ok()?,
        })
    }

    /// Rule 7: the writer of the body that removed the lost name, and how.
    fn removed(
        &mut self,
        loss: &Loss,
        lost_name: &ElementName,
        maker: &Id,
        unmet: bool,
    ) -> Option<LossReason> {
        let body = &loss.body;
        let presence = Presence::new(loss, lost_name, unmet);
        let writers: Vec<usize> = self
            .view
            .bodies
            .writers_before(body, self.explained)
            .map(|(position, _)| position)
            .collect();
        let mut remover = None;
        for &writer in writers.iter().rev() {
            if presence.has(self.output_of(body, writer)?) {
                continue;
            }
            if self.input_has(writer, &presence) {
                remover = Some(writer);
                break;
            }
        }
        let remover = match remover {
            Some(remover) => remover,
            None => {
                let maker_position = self.position(maker)?;
                let removed_its_own = writers.contains(&maker_position)
                    && !presence.has(self.output_of(body, maker_position)?);
                if !removed_its_own {
                    return None;
                }
                maker_position
            }
        };
        let by = self.view.id(remover).clone();
        self.record(&by);
        Some(match self.how(loss, lost_name, maker, remover, unmet) {
            Some(how) => LossReason::Removed { by, how },
            None => LossReason::Unknown,
        })
    }

    /// The state of `body` just after its writer at `writer`: the state just before the record
    /// after it.
    fn output_of(&self, body: &Id, writer: usize) -> Option<&'r BodyState> {
        self.view
            .state_before(body, writer + 1)
            .map(|(state, _)| state)
    }

    /// True when one of the bodies the writer at `writer` declares has the lost name just before
    /// it.
    fn input_has(&mut self, writer: usize, presence: &Presence<'_>) -> bool {
        let declared: Vec<Id> = self
            .view
            .dependencies(writer)
            .map(|dependencies| {
                dependencies
                    .bodies_read
                    .iter()
                    .chain(&dependencies.bodies_written)
                    .map(|link| link.id.clone())
                    .collect()
            })
            .unwrap_or_default();
        declared.iter().any(|body| {
            self.state_before(body, writer)
                .is_some_and(|state| presence.has(state))
        })
    }

    /// How the remover at `remover` removed the lost name (rule 7, items 1 to 8).
    fn how(
        &self,
        loss: &Loss,
        lost_name: &ElementName,
        maker: &Id,
        remover: usize,
        unmet: bool,
    ) -> Option<Removal> {
        let body = &loss.body;
        let kind = self.view.kind(remover);
        if self.view.id(remover) == maker {
            if unmet {
                return Some(Removal::NoLongerMeet {
                    between: self.between(loss, remover),
                });
            }
            let absence = self.view.output(remover).and_then(|output| {
                output
                    .absent
                    .iter()
                    .find(|absent| {
                        *lost_name == absent.name
                            || lost_name.piece_free().as_ref() == Some(&absent.name)
                    })
                    .map(|absent| absent.why)
            });
            return Some(Removal::NoLongerMade { absence });
        }
        let blend = match kind {
            "fillet" => Some(Removal::Rounded),
            "chamfer" => Some(Removal::Beveled),
            _ => None,
        };
        if blend.is_some() && self.blends_query(loss, remover) {
            return blend;
        }
        if kind == "shell" {
            return Some(Removal::Hollowed);
        }
        if loss.lost.pieces.contains(lost_name) {
            return Some(Removal::PieceGone);
        }
        let change = self.view.update(remover, body).map(|update| update.change);
        if matches!(change, Some(BodyChange::Cut | BodyChange::Intersect)) || kind == "split_body" {
            return Some(Removal::CutAway);
        }
        if change == Some(BodyChange::Join) {
            return Some(Removal::Covered);
        }
        if unmet {
            return Some(Removal::NoLongerMeet {
                between: self.between(loss, remover),
            });
        }
        blend
    }

    /// True when the fillet or chamfer at `blend` made a face from the query: `<feature>/fillet/<n>`
    /// or `<feature>/chamfer/<n>` whose `n` is the query, or whose face terms pair one to one with
    /// members of the families of the query's parts just before it.
    fn blends_query(&self, loss: &Loss, blend: usize) -> bool {
        let body = &loss.body;
        let query = &loss.reference.query;
        let feature = self.view.id(blend).as_str();
        let (Some(output), Some((input, _))) = (
            self.output_of(body, blend),
            self.view.state_before(body, blend),
        ) else {
            return false;
        };
        let families = families(input.name_set(), &parts_of(query), &loss.reference.merged);
        output.naming().names(ElementKind::Face).any(|(_, face)| {
            let Some(parts) = FaceName::of(face) else {
                return false;
            };
            let Some(Generator::List(generator)) = parts.generator else {
                return false;
            };
            if parts.feature != feature || !matches!(parts.role, "fillet" | "chamfer") {
                return false;
            }
            generator == query.as_str()
                || ElementName::parse(generator).is_ok_and(|rounded| {
                    rounded.kind() == query.kind() && pairs(&rounded.face_terms(), &families)
                })
        })
    }

    /// The face that now runs where the edge was (rule 7, `between`): among the faces of the
    /// remover's output that touch a member of the family of every part, the first in byte order
    /// that also touches every other such face.
    fn between(&self, loss: &Loss, remover: usize) -> Option<ElementName> {
        let state = self.output_of(&loss.body, remover)?;
        let names = state.name_set();
        let families = families(
            names,
            &parts_of(&loss.reference.query),
            &loss.reference.merged,
        );
        let touching = touching_faces(state.naming());
        let touches = |face: &ElementName, family: &BTreeSet<ElementName>| {
            touching.get(face).is_some_and(|neighbors| {
                neighbors.iter().any(|neighbor| family.contains(*neighbor))
            })
        };
        let mut faces: Vec<&ElementName> = state
            .naming()
            .names(ElementKind::Face)
            .map(|(_, face)| face)
            .filter(|face| families.iter().all(|family| touches(face, family)))
            .collect();
        faces.sort();
        faces
            .iter()
            .find(|face| {
                faces.iter().all(|other| {
                    other == *face
                        || touching
                            .get(*face)
                            .is_some_and(|neighbors| neighbors.contains(*other))
                })
            })
            .map(|face| (*face).clone())
    }

    /// Rule 8: the feature that made the lost name now writes other bodies but not this one.
    fn left_this_body(&mut self, loss: &Loss, maker: &Id) -> Option<LossReason> {
        let fact = self.record(maker);
        if fact.state != Some(NodeState::Active) || !fact.before {
            return None;
        }
        let position = self.view.position(maker)?;
        let written = &self.view.dependencies(position)?.bodies_written;
        if written.is_empty() || written.iter().any(|link| link.id == loss.body) {
            return None;
        }
        Some(LossReason::OtherBody {
            body: written[0].id.clone(),
            by: Some(maker.clone()),
            by_suppressed: false,
            present: false,
        })
    }

    /// The first writer of `body` before the feature explained whose output map satisfies
    /// `records`.
    fn first_writer_recording(
        &mut self,
        body: &Id,
        records: impl Fn(&NamingMap) -> bool,
    ) -> Option<usize> {
        let writers: Vec<usize> = self
            .view
            .bodies
            .writers_before(body, self.explained)
            .map(|(position, _)| position)
            .collect();
        writers.into_iter().find(|&writer| {
            self.output_of(body, writer)
                .is_some_and(|state| records(state.naming()))
        })
    }
}

/// What "a map has the lost name" means for one loss (rule 7).
struct Presence<'l> {
    /// The lost name.
    lost_name: &'l ElementName,
    /// The stored reference.
    reference: &'l Reference,
    /// The anchors of the lost name, for a piece of the query: only a member that touches every
    /// anchor present in the map counts.
    anchors: &'l [ElementName],
    /// True for a query whose faces no longer meet: a map has it when it names an element that
    /// joins its parts.
    unmet: bool,
}

impl<'l> Presence<'l> {
    /// The presence test of the lost name of `loss`.
    fn new(loss: &'l Loss, lost_name: &'l ElementName, unmet: bool) -> Presence<'l> {
        let reference = &loss.reference;
        // The query's own `#n` is matched without its anchors, whose test needs vertex infos.
        let anchors = if loss.lost.pieces.contains(lost_name) && *lost_name != reference.query {
            reference
                .anchors
                .0
                .get(lost_name)
                .map_or(&[][..], Vec::as_slice)
        } else {
            &[]
        };
        Presence {
            lost_name,
            reference,
            anchors,
            unmet,
        }
    }

    /// True when the naming map of `state` has the lost name.
    fn has(&self, state: &BodyState) -> bool {
        let names = state.name_set();
        if self.unmet {
            let query = &self.reference.query;
            let families = families(names, &parts_of(query), &self.reference.merged);
            return families.iter().all(|family| !family.is_empty())
                && joins(state.naming(), query.kind(), &families);
        }
        let anchors: Vec<&ElementName> = self
            .anchors
            .iter()
            .filter(|anchor| !names.family(anchor, &Merged::default()).is_empty())
            .collect();
        names
            .family(self.lost_name, &self.reference.merged)
            .into_iter()
            .any(|member| anchors.iter().all(|anchor| names.touches(member, anchor)))
    }
}

/// The family of each part in `names`, as sets of face names.
fn families(names: &NameSet, parts: &[ElementName], merged: &Merged) -> Vec<BTreeSet<ElementName>> {
    parts
        .iter()
        .map(|part| names.family(part, merged).into_iter().cloned().collect())
        .collect()
}

/// True when `map` names an element of `kind` whose face terms pair one to one with members of
/// `families`.
fn joins(map: &NamingMap, kind: ElementKind, families: &[BTreeSet<ElementName>]) -> bool {
    map.names(kind)
        .any(|(_, element)| pairs(&element.face_terms(), families))
}

/// True when every one of `terms` can be paired with a distinct family of `families` that holds
/// it, and both have the same count.
fn pairs(terms: &[ElementName], families: &[BTreeSet<ElementName>]) -> bool {
    fn assign(
        terms: &[ElementName],
        families: &[BTreeSet<ElementName>],
        used: &mut [bool],
    ) -> bool {
        let Some((term, rest)) = terms.split_first() else {
            return true;
        };
        for (index, family) in families.iter().enumerate() {
            if used[index] || !family.contains(term) {
                continue;
            }
            used[index] = true;
            if assign(rest, families, used) {
                return true;
            }
            used[index] = false;
        }
        false
    }
    terms.len() == families.len() && assign(terms, families, &mut vec![false; families.len()])
}

/// The faces each face of `map` touches: two faces touch when an edge name lists both.
fn touching_faces(map: &NamingMap) -> BTreeMap<&ElementName, BTreeSet<&ElementName>> {
    let faces: BTreeMap<&str, &ElementName> = map
        .names(ElementKind::Face)
        .map(|(_, face)| (face.as_str(), face))
        .collect();
    let mut touching: BTreeMap<&ElementName, BTreeSet<&ElementName>> = BTreeMap::new();
    for (_, edge) in map.names(ElementKind::Edge) {
        let terms: Vec<&ElementName> = edge
            .face_terms()
            .iter()
            .filter_map(|term| faces.get(term.as_str()).copied())
            .collect();
        for &face in &terms {
            touching
                .entry(face)
                .or_default()
                .extend(terms.iter().filter(|&&other| other != face));
        }
    }
    touching
}

/// True when `map` has a relative of `face` (naming.md section 3.9 step 4): a face of the same
/// feature, sketch generator and `@` groups under another role.
fn has_relatives(map: &NamingMap, face: &FaceName<'_>, generator: &str) -> bool {
    map.names(ElementKind::Face).any(|(_, other)| {
        FaceName::of(other).is_some_and(|other| {
            other.feature == face.feature
                && other.role != face.role
                && other.sketch_generator() == Some(generator)
                && other.copies == face.copies
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(text: &str) -> ElementName {
        ElementName::parse(text).expect("a valid name")
    }

    fn family(names: &[&str]) -> BTreeSet<ElementName> {
        names.iter().map(|text| name(text)).collect()
    }

    #[test]
    fn terms_pair_one_to_one_with_families() {
        let families = [family(&["f-a/top", "f-a/top#1"]), family(&["f-a/front"])];

        assert!(pairs(&[name("f-a/front"), name("f-a/top#1")], &families));
        assert!(!pairs(&[name("f-a/front"), name("f-a/back")], &families));
        assert!(!pairs(&[name("f-a/front")], &families));
    }
}
