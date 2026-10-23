//! The changes between the clean snapshot of a message's key and now that can reach the
//! message's feature, and the one that started the problem (engine.md section 3.21, "Changes"
//! and "Primary change").

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use partsmith_features::{FieldHint, Registry};
use partsmith_model::{Document, FeatureRecord, Id};
use partsmith_recompute::{LossReason, Message};

use super::Tombstone;
use super::baseline::{Baseline, Graph, UsedParameters};
use crate::params::{field_pointer, stored_references, value_at};
use crate::sketch::{SKETCH_TYPE, point_entity, stored_drawing};

/// The manifest fields of a sketch that hold its drawing, whose changes a record change names
/// entity by entity and constraint by constraint instead.
const DRAWING_FIELDS: [&str; 2] = ["entities", "constraints"];

/// One change between the clean snapshot and now that can reach the message's feature.
#[derive(Debug, Clone)]
pub(super) enum Change {
    /// A feature upstream now that the clean snapshot's recipe did not have.
    Added(Id),
    /// A feature upstream in the clean snapshot that the recipe no longer has.
    Deleted(Id),
    /// A feature suppressed since.
    Suppressed(Id),
    /// A feature unsuppressed since.
    Unsuppressed(Id),
    /// A feature whose record changed.
    Record(RecordChange),
    /// A feature that moved, changing its order relative to the message's feature.
    Moved(Id),
    /// The rollback marker moved across a feature the message names.
    Rollback(Id),
    /// A parameter whose expression changed.
    Parameter(Id),
    /// The document units.
    Units,
}

impl Change {
    /// The feature the change belongs to; `None` for a parameter or the units, which belong to
    /// the document.
    pub(super) fn feature(&self) -> Option<&Id> {
        match self {
            Change::Added(feature)
            | Change::Deleted(feature)
            | Change::Suppressed(feature)
            | Change::Unsuppressed(feature)
            | Change::Moved(feature)
            | Change::Rollback(feature) => Some(feature),
            Change::Record(record) => Some(&record.feature),
            Change::Parameter(_) | Change::Units => None,
        }
    }

    /// The order of the changes of one feature: added or deleted first, then suppressed or
    /// unsuppressed, then its record change, then its move, then the rollback marker.
    fn rank(&self) -> u8 {
        match self {
            Change::Added(_) | Change::Deleted(_) => 0,
            Change::Suppressed(_) | Change::Unsuppressed(_) => 1,
            Change::Record(_) => 2,
            Change::Moved(_) => 3,
            Change::Rollback(_) | Change::Parameter(_) | Change::Units => 4,
        }
    }
}

/// The change of one feature's record: its sketch entities and constraints and its manifest
/// fields whose value differs, each kind in the order its words name the first.
#[derive(Debug, Clone)]
pub(super) struct RecordChange {
    /// The feature.
    pub(super) feature: Id,
    /// For a sketch, its changed entities (the one the message names first, then in stored
    /// order), then its changed constraints in stored order.
    pub(super) items: Vec<SketchItem>,
    /// Its manifest fields whose value at their pointer differs, in manifest order.
    pub(super) fields: Vec<FieldHint>,
}

/// One changed entity or constraint of a sketch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SketchItem {
    /// An entity the clean snapshot had and the sketch no longer has.
    EntityDeleted(Id),
    /// An entity the clean snapshot did not have.
    EntityAdded(Id),
    /// An entity whose stored geometry changed.
    EntityMoved(Id),
    /// An entity made construction geometry (`true`) or part of the shape.
    EntityConstruction(Id, bool),
    /// A constraint the clean snapshot did not have.
    ConstraintAdded(Id),
    /// A constraint the clean snapshot had and the sketch no longer has.
    ConstraintDeleted(Id),
    /// A dimension whose expression changed.
    Dimension(Id),
    /// A dimension made driving (`true`) or a reference.
    Driving(Id, bool),
}

/// A message's clean snapshot compared with now, from the graphs of their recipes.
pub(super) struct Comparison<'a> {
    /// The feature of the message.
    pub(super) feature: &'a Id,
    /// The message.
    pub(super) message: &'a Message,
    /// The graph of the clean snapshot's recipe.
    pub(super) clean: Graph<'a>,
    /// The graph of the recipe now.
    pub(super) now: Graph<'a>,
    /// The features that left the document, with the label and position they had.
    pub(super) tombstones: &'a BTreeMap<Id, Tombstone>,
    /// The features the message names, in order: those its reason names (`reason_features`),
    /// then its `root`.
    pub(super) named: Vec<Id>,
}

impl<'a> Comparison<'a> {
    /// The comparison of the clean snapshot `clean` of `message` of `feature` with the recompute
    /// `now`.
    pub(super) fn new(
        feature: &'a Id,
        message: &'a Message,
        clean: &'a Baseline,
        now: &'a Baseline,
        tombstones: &'a BTreeMap<Id, Tombstone>,
    ) -> Comparison<'a> {
        let found = message
            .reason
            .as_ref()
            .map(reason_features)
            .unwrap_or_default()
            .into_iter()
            .chain(message.root.as_ref());
        let mut named: Vec<Id> = Vec::new();
        for named_feature in found {
            if !named.contains(named_feature) {
                named.push(named_feature.clone());
            }
        }
        Comparison {
            feature,
            message,
            clean: clean.graph(),
            now: now.graph(),
            tombstones,
            named,
        }
    }

    /// The graphs of both recipes: the clean snapshot's, then now's.
    pub(super) fn graphs(&self) -> [&Graph<'a>; 2] {
        [&self.clean, &self.now]
    }

    /// The feature registry, for manifests.
    pub(super) fn registry(&self) -> &'a Registry {
        self.now.baseline().registry()
    }

    /// Every change that can reach the message's feature (section 3.21, "Changes"), feature by
    /// feature in id order, then the parameters in document order, then the units.
    pub(super) fn changes(&self) -> Vec<Change> {
        let clean = self.clean.baseline();
        let now = self.now.baseline();
        let upstream_clean = self.clean.upstream(self.feature, None);
        let upstream_now = self.now.upstream(self.feature, None);
        let moved = self.moved_features();
        let mut considered: BTreeSet<&Id> = upstream_clean.union(&upstream_now).copied().collect();
        considered.insert(self.feature);
        let mut changes = Vec::new();
        for &feature in &considered {
            match (clean.record(feature), now.record(feature)) {
                (None, Some(_)) if upstream_now.contains(feature) => {
                    changes.push(Change::Added(feature.clone()));
                }
                (Some(_), None) if upstream_clean.contains(feature) => {
                    changes.push(Change::Deleted(feature.clone()));
                }
                (Some(before), Some(after)) => {
                    if before.suppressed != after.suppressed {
                        changes.push(if after.suppressed {
                            Change::Suppressed(feature.clone())
                        } else {
                            Change::Unsuppressed(feature.clone())
                        });
                    }
                    if let Some(record) = self.record_change(feature, before, after) {
                        changes.push(Change::Record(record));
                    }
                    if moved.contains(feature) && self.order_changed(feature) {
                        changes.push(Change::Moved(feature.clone()));
                    }
                }
                _ => {}
            }
        }
        if let Some(rollback) = self.rollback() {
            changes.push(rollback);
        }
        changes.extend(
            self.parameter_changes(&considered)
                .into_iter()
                .map(Change::Parameter),
        );
        let relies_on_units = self.graphs().iter().any(|graph| {
            considered
                .iter()
                .any(|feature| graph.relies_on_units(feature))
        });
        if clean.units() != now.units() && relies_on_units {
            changes.push(Change::Units);
        }
        changes
    }

    /// The position of the primary change among `changes` (section 3.21, "Primary change"):
    /// the first change of the first group that has one.
    pub(super) fn primary(&self, changes: &[Change]) -> Option<usize> {
        let first_of = |feature: &Id| {
            changes
                .iter()
                .enumerate()
                .filter(|(_, change)| change.feature() == Some(feature))
                .min_by_key(|(_, change)| change.rank())
                .map(|(position, _)| position)
        };
        let first_among = |features: &[Id]| features.iter().find_map(first_of);
        let units = || {
            changes
                .iter()
                .position(|change| matches!(change, Change::Units))
        };
        first_among(&self.named)
            .or_else(|| self.parameter_used_directly(changes, &self.named))
            .or_else(|| first_of(self.feature))
            .or_else(|| self.parameter_used_directly(changes, std::slice::from_ref(self.feature)))
            .or_else(|| units().filter(|_| self.relies_on_units(self.feature)))
            .or_else(|| first_among(&self.direct()))
            .or_else(|| self.latest_feature_change(changes))
            .or_else(|| {
                changes
                    .iter()
                    .position(|change| matches!(change, Change::Parameter(_)))
            })
            .or_else(units)
    }

    /// How many changes besides the primary one are counted (section 3.21, `{others}`): every
    /// other change once, except those that reach the message's feature only through the
    /// feature of the primary change.
    pub(super) fn other_count(&self, changes: &[Change], primary: usize) -> usize {
        let Some(through) = changes[primary].feature() else {
            return changes.len() - 1;
        };
        let mut reached: BTreeSet<&Id> = self.clean.upstream(self.feature, Some(through));
        reached.extend(self.now.upstream(self.feature, Some(through)));
        reached.insert(self.feature);
        changes
            .iter()
            .enumerate()
            .filter(|&(position, change)| {
                position != primary
                    && match change.feature() {
                        Some(feature) => feature == through || reached.contains(feature),
                        None => self.reaches(change, &reached),
                    }
            })
            .count()
    }

    /// True when a change of the document reaches one of `features`: a parameter one of them
    /// uses, or the units when one of them relies on them.
    fn reaches(&self, change: &Change, features: &BTreeSet<&Id>) -> bool {
        features.iter().any(|feature| match change {
            Change::Parameter(parameter) => self.uses(feature, |used| used.all.contains(parameter)),
            _ => self.relies_on_units(feature),
        })
    }

    /// True when `feature` uses parameters that `matching` accepts, in the clean snapshot or now.
    fn uses(&self, feature: &Id, matching: impl Fn(&UsedParameters) -> bool) -> bool {
        self.graphs()
            .iter()
            .any(|graph| graph.parameters(feature).is_some_and(&matching))
    }

    /// True when the document units decide a size of `feature`, in the clean snapshot or now.
    fn relies_on_units(&self, feature: &Id) -> bool {
        self.graphs()
            .iter()
            .any(|graph| graph.relies_on_units(feature))
    }

    /// What the message's feature reads directly, in the clean snapshot or now: its feature
    /// links, then its body providers.
    fn direct(&self) -> Vec<Id> {
        let mut direct: Vec<Id> = Vec::new();
        let links = [self.now.links(self.feature), self.clean.links(self.feature)];
        let providers = [
            self.now.providers(self.feature),
            self.clean.providers(self.feature),
        ];
        let found = links
            .iter()
            .flatten()
            .chain(providers.into_iter().flatten());
        for feature in found {
            if !direct.contains(feature) {
                direct.push(feature.clone());
            }
        }
        direct
    }

    /// The first parameter change, in document order, of a parameter one of `features` uses
    /// directly, in the clean snapshot or now.
    fn parameter_used_directly(&self, changes: &[Change], features: &[Id]) -> Option<usize> {
        changes.iter().position(|change| {
            let Change::Parameter(parameter) = change else {
                return false;
            };
            features
                .iter()
                .any(|feature| self.uses(feature, |used| used.direct.contains(parameter)))
        })
    }

    /// The change of the feature latest in timeline order (a deleted feature at its tombstone's
    /// position), the first of that feature's changes.
    fn latest_feature_change(&self, changes: &[Change]) -> Option<usize> {
        let position = |feature: &Id| {
            self.now.baseline().position(feature).or_else(|| {
                self.tombstones
                    .get(feature)
                    .map(|tombstone| tombstone.position)
            })
        };
        changes
            .iter()
            .enumerate()
            .filter_map(|(index, change)| {
                Some((index, position(change.feature()?)?, change.rank()))
            })
            .max_by_key(|&(_, position, rank)| (position, Reverse(rank)))
            .map(|(index, ..)| index)
    }

    /// The change of `feature`'s record between the clean snapshot (`before`) and now
    /// (`after`): its sketch entities and constraints, and its manifest fields; `None` when
    /// neither changed.
    fn record_change(
        &self,
        feature: &Id,
        before: &FeatureRecord,
        after: &FeatureRecord,
    ) -> Option<RecordChange> {
        if before.params == after.params || before.kind != after.kind {
            return None;
        }
        let manifest = self.registry().get(&after.kind)?.manifest();
        let sketch = after.kind == SKETCH_TYPE;
        let fields: Vec<FieldHint> = manifest
            .fields
            .iter()
            .filter(|hint| !(sketch && DRAWING_FIELDS.contains(&hint.path.as_str())))
            .filter(|hint| !hint.path.contains('*'))
            .filter(|hint| {
                let pointer = field_pointer(&hint.path);
                value_at(&before.params, &pointer) != value_at(&after.params, &pointer)
            })
            .cloned()
            .collect();
        let items = if sketch {
            self.sketch_items(feature, before, after)
        } else {
            Vec::new()
        };
        (!fields.is_empty() || !items.is_empty()).then(|| RecordChange {
            feature: feature.clone(),
            items,
            fields,
        })
    }

    /// The changed entities and constraints of a sketch's record: entities deleted, moved or
    /// whose construction flag changed, in the clean snapshot's order, then added ones, with the
    /// entity the message names first; then constraints deleted, or dimensions whose expression
    /// or driving flag changed, then added ones.
    fn sketch_items(
        &self,
        sketch: &Id,
        before: &FeatureRecord,
        after: &FeatureRecord,
    ) -> Vec<SketchItem> {
        let (Some(old), Some(new)) = (stored_drawing(before), stored_drawing(after)) else {
            return Vec::new();
        };
        let mut items: Vec<SketchItem> = Vec::new();
        for entity in &old.entities {
            match new.entity(&entity.id) {
                None => items.push(SketchItem::EntityDeleted(entity.id.clone())),
                Some(now) if now.geometry != entity.geometry => {
                    items.push(SketchItem::EntityMoved(entity.id.clone()));
                }
                Some(now) if now.construction != entity.construction => {
                    items.push(SketchItem::EntityConstruction(
                        entity.id.clone(),
                        now.construction,
                    ));
                }
                Some(_) => {}
            }
        }
        for entity in &new.entities {
            if old.entity(&entity.id).is_none() {
                items.push(SketchItem::EntityAdded(entity.id.clone()));
            }
        }
        if let Some(named) = self.named_entity(sketch)
            && let Some(position) = items
                .iter()
                .position(|item| item_entity(item) == Some(&named))
        {
            let item = items.remove(position);
            items.insert(0, item);
        }
        for constraint in &old.constraints {
            let Some(now) = new.constraint(&constraint.id) else {
                items.push(SketchItem::ConstraintDeleted(constraint.id.clone()));
                continue;
            };
            if let (Some(after), Some(before)) = (now.dimension(), constraint.dimension()) {
                if after.expr != before.expr {
                    items.push(SketchItem::Dimension(constraint.id.clone()));
                } else if after.driving != before.driving {
                    items.push(SketchItem::Driving(constraint.id.clone(), after.driving));
                }
            }
        }
        for constraint in &new.constraints {
            if old.constraint(&constraint.id).is_none() {
                items.push(SketchItem::ConstraintAdded(constraint.id.clone()));
            }
        }
        items
    }

    /// The entity of `sketch` the message's reason names: the entity of an `EntityDeleted`
    /// reason, the point's entity of a `PointGone` reason.
    fn named_entity(&self, sketch: &Id) -> Option<Id> {
        match self.message.reason.as_ref()? {
            LossReason::EntityDeleted {
                sketch: named,
                entity,
                ..
            } if named == sketch => Some(entity.clone()),
            LossReason::PointGone {
                sketch: named,
                point,
                ..
            } if named == sketch => point_entity(point),
            _ => None,
        }
    }

    /// The rollback change: when the marker moved across a feature the message names, which
    /// ran on one side of the change and is rolled back on the other.
    fn rollback(&self) -> Option<Change> {
        let clean = self.clean.baseline();
        let now = self.now.baseline();
        if clean.document.rollback == now.document.rollback {
            return None;
        }
        let rolled_back = |document: &Document, feature: &Id| {
            document
                .active_features()
                .iter()
                .all(|record| record.id != *feature)
        };
        self.named
            .iter()
            .find(|feature| {
                now.record(feature).is_some()
                    && rolled_back(&clean.document, feature) != rolled_back(&now.document, feature)
            })
            .cloned()
            .map(Change::Rollback)
    }

    /// The parameters used by `features`, in the clean snapshot or now, whose expression
    /// changed, in document order.
    fn parameter_changes(&self, features: &BTreeSet<&Id>) -> Vec<Id> {
        let before = &self.clean.baseline().document;
        self.now
            .baseline()
            .document
            .parameters
            .iter()
            .filter(|parameter| {
                before
                    .parameter(&parameter.id)
                    .is_some_and(|old| old.expr != parameter.expr)
            })
            .filter(|parameter| {
                features
                    .iter()
                    .any(|feature| self.uses(feature, |used| used.all.contains(&parameter.id)))
            })
            .map(|parameter| parameter.id.clone())
            .collect()
    }

    /// The features that moved: those both recipes have that lie off a longest common
    /// subsequence of their two timelines, which keeps the message's feature on it when one of
    /// the longest does, so a feature dragged past others is the one that moved.
    fn moved_features(&self) -> BTreeSet<Id> {
        let clean = self.clean.baseline();
        let now = self.now.baseline();
        let same_timeline = clean
            .document
            .features
            .iter()
            .map(|record| &record.id)
            .eq(now.document.features.iter().map(|record| &record.id));
        if same_timeline {
            return BTreeSet::new();
        }
        let before = order_in_both(clean, now);
        let after = order_in_both(now, clean);
        if before == after {
            return BTreeSet::new();
        }
        let kept = longest_common_order(&before, &after, self.feature);
        before
            .into_iter()
            .filter(|feature| !kept.contains(*feature))
            .cloned()
            .collect()
    }

    /// True when `feature` and `other` are in a different order now than in the clean snapshot.
    pub(super) fn order_swapped(&self, feature: &Id, other: &Id) -> bool {
        let clean = self.clean.baseline();
        let now = self.now.baseline();
        (clean.position(feature) < clean.position(other))
            != (now.position(feature) < now.position(other))
    }

    /// True when `feature`'s order relative to the message's feature changed; for the message's
    /// feature itself, its order relative to a feature it links to or that starts a name in its
    /// references.
    fn order_changed(&self, feature: &Id) -> bool {
        let clean = self.clean.baseline();
        let now = self.now.baseline();
        let swapped = |other: &Id| {
            now.record(other).is_some()
                && clean.record(other).is_some()
                && self.order_swapped(feature, other)
        };
        if feature != self.feature {
            return swapped(self.feature);
        }
        let mut related = self.clean.links(feature);
        related.extend(self.now.links(feature));
        for record in [clean.record(feature), now.record(feature)]
            .into_iter()
            .flatten()
        {
            related.extend(name_starts(record));
        }
        related.iter().any(swapped)
    }
}

/// The features a reason names, in the order a cause looks for their changes: its `feature`,
/// the feature that removed or that the reference followed (`by`), the failed feature
/// upstream (`root`) and the sketch of a sketch entity or point.
fn reason_features(reason: &LossReason) -> Vec<&Id> {
    match reason {
        LossReason::UpstreamFailed { feature, root } => vec![feature, root],
        LossReason::EntityDeleted {
            feature, sketch, ..
        }
        | LossReason::PointGone {
            feature, sketch, ..
        } => vec![feature, sketch],
        LossReason::NotInDocument { feature }
        | LossReason::Suppressed { feature }
        | LossReason::ComesLater { feature }
        | LossReason::RolledBack { feature }
        | LossReason::CopyGone { feature, .. }
        | LossReason::NotInFile { feature } => vec![feature],
        LossReason::OtherBody { by, .. } => by.iter().collect(),
        LossReason::Removed { by, .. } | LossReason::Followed { by } => vec![by],
        LossReason::Unknown => Vec::new(),
    }
}

/// The entity of an entity item.
fn item_entity(item: &SketchItem) -> Option<&Id> {
    match item {
        SketchItem::EntityDeleted(entity)
        | SketchItem::EntityAdded(entity)
        | SketchItem::EntityMoved(entity)
        | SketchItem::EntityConstruction(entity, _) => Some(entity),
        _ => None,
    }
}

/// The features that start the face names of the references a record stores.
fn name_starts(record: &FeatureRecord) -> Vec<Id> {
    stored_references(&record.params)
        .iter()
        .flat_map(|stored| stored.reference.query.face_parts())
        .filter_map(|face| {
            let feature = face.face_name()?.feature().to_string();
            Id::new(feature).ok()
        })
        .collect()
}

/// The features of `timeline`'s recipe that `other`'s has too, in `timeline`'s order.
fn order_in_both<'b>(timeline: &'b Baseline, other: &Baseline) -> Vec<&'b Id> {
    timeline
        .document
        .features
        .iter()
        .map(|record| &record.id)
        .filter(|feature| other.position(feature).is_some())
        .collect()
}

/// The features of a longest common subsequence of `before` and `after`, two orders of the same
/// features; among the longest, one that keeps `kept`, when one does.
fn longest_common_order(before: &[&Id], after: &[&Id], kept: &Id) -> BTreeSet<Id> {
    // Each match weighs one more than the length of the longest order, and `kept` one more than
    // that, so the heaviest subsequence is a longest one, keeping `kept` when it can.
    let unit = u64::try_from(before.len()).unwrap_or(u64::MAX / 4) + 1;
    let weight = |feature: &Id| if feature == kept { unit + 1 } else { unit };
    let columns = after.len() + 1;
    let mut best = vec![0_u64; (before.len() + 1) * columns];
    for row in 1..=before.len() {
        for column in 1..=after.len() {
            let index = row * columns + column;
            best[index] = if before[row - 1] == after[column - 1] {
                best[index - columns - 1] + weight(before[row - 1])
            } else {
                best[index - columns].max(best[index - 1])
            };
        }
    }
    let mut kept_features = BTreeSet::new();
    let (mut row, mut column) = (before.len(), after.len());
    while row > 0 && column > 0 {
        let index = row * columns + column;
        if before[row - 1] == after[column - 1] {
            kept_features.insert(before[row - 1].clone());
            row -= 1;
            column -= 1;
        } else if best[index - columns] >= best[index - 1] {
            row -= 1;
        } else {
            column -= 1;
        }
    }
    kept_features
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::id;

    /// The ids of these names.
    fn ids(names: &[&str]) -> Vec<Id> {
        names.iter().map(|name| id(name)).collect()
    }

    /// References to each id, in order.
    fn order(features: &[Id]) -> Vec<&Id> {
        features.iter().collect()
    }

    #[test]
    fn a_feature_dragged_past_others_is_the_one_that_moved() {
        let before = ids(&["f-sk1", "f-ext1", "f-dp1", "f-sk2", "f-ext2", "f-fil1"]);
        let after = ids(&["f-sk1", "f-ext1", "f-fil1", "f-dp1", "f-sk2", "f-ext2"]);
        let kept = longest_common_order(&order(&before), &order(&after), &id("f-ext2"));

        assert!(!kept.contains(&id("f-fil1")));
        assert_eq!(kept.len(), 5);
    }

    #[test]
    fn of_two_swapped_features_the_kept_one_stays() {
        let before = ids(&["f-box1", "f-fil1", "f-box2"]);
        let after = ids(&["f-box1", "f-box2", "f-fil1"]);
        let kept = longest_common_order(&order(&before), &order(&after), &id("f-fil1"));

        assert_eq!(kept, [id("f-box1"), id("f-fil1")].into_iter().collect());
    }
}
