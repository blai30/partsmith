//! A cause and its actions (engine.md section 3.21, "Cause actions"): the cause keeps its text and
//! what its primary change was, and its actions are made again from the current records after
//! every recompute, so a revert that would change nothing, or whose feature, parameter or
//! neighbor is gone, and `Undo` of an entry that is no longer the newest are left out.

use partsmith_features::{FieldUnit, Place};
use partsmith_model::{
    Document, Edit, Expr, FeatureRecord, Id, LINEAR_TOLERANCE, Parameter, Units,
};
use partsmith_protocol::{FixAction, FixKind, FixRef, Repair};
use partsmith_recompute::Message;
use partsmith_sketch::{EntityKind, Placement, SketchPlane};
use serde_json::{Map, Value};

use super::changes::{Change, RecordChange, SketchItem};
use super::words::{Side, Words};
use crate::fixes::{constraint_pointer, index_beside, offered_edits, updated};
use crate::frames::kept_face_plane;
use crate::ghosts::GhostShape;
use crate::params::{field_pointer, remove, set, value_at};
use crate::run::{FeatureView, RunView};
use crate::sketch::stored_drawing;
use crate::state::built_sketch;

/// The label of the cause action that stores where a sketch stood when it last built without its
/// message.
const KEEP_LABEL: &str = "Keep the sketch where it was";

/// The code of the message of a sketch that lost its face, which `Keep the sketch where it was`
/// answers.
const PLANE_FALLBACK: &str = "plane_fallback";

/// Unit directions closer than this are the same direction.
const DIRECTION_TOLERANCE: f64 = 1e-9;

/// A cause, made when its message's key appeared (section 3.21, "When a cause is made"): its
/// text and feature stay while the key lasts, and its actions are made again from what its
/// primary change was.
#[derive(Debug, Clone)]
pub(super) struct MadeCause {
    /// `This started after you ...`.
    pub(super) text: String,
    /// The feature the primary change belongs to; `None` for a parameter, units or rollback
    /// change.
    pub(super) feature: Option<Id>,
    /// The revert of the primary change, when it has one.
    pub(super) revert: Option<Revert>,
    /// What `Show` shows of the primary change, when it shows something.
    pub(super) show: Option<Show>,
    /// The undo entry whose command made the primary change.
    pub(super) entry: Option<u64>,
    /// Where the message's feature, a sketch, stood when it last built without the message.
    pub(super) clean_placement: Option<Placement>,
    /// The sketch entity the primary change deleted or moved, where it was (display sessions).
    pub(super) ghost: Option<GhostShape>,
}

/// The revert of a primary change (section 3.21, "Reverts"), built from the current records when
/// it is offered.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Revert {
    /// Fields of a record set back to their values in the clean snapshot.
    Fields {
        /// The feature.
        feature: Id,
        /// Each changed field's pointer and its value in the clean snapshot (`None`: absent).
        fields: Vec<(String, Option<Value>)>,
        /// How the label names them.
        label: FieldsLabel,
    },
    /// A dimension's expression set back.
    Dimension {
        /// The sketch.
        sketch: Id,
        /// The dimension.
        constraint: Id,
        /// Its expression in the clean snapshot.
        expr: Expr,
        /// `{type}` of its title, `Distance`.
        kind: String,
        /// Its value or expression in the clean snapshot, as a label writes it.
        old: String,
    },
    /// A parameter's expression set back.
    Parameter {
        /// The parameter.
        parameter: Id,
        /// Its expression in the clean snapshot.
        expr: Expr,
        /// Its value or expression in the clean snapshot, as a label writes it.
        old: String,
    },
    /// A feature suppressed (`true`) or unsuppressed again.
    Suppression {
        /// The feature.
        feature: Id,
        /// The suppression to store.
        suppressed: bool,
    },
    /// A feature moved back beside its neighbor in the clean snapshot.
    Move {
        /// The feature.
        feature: Id,
        /// Directly below the feature that was directly above it in the clean snapshot, or above
        /// the one below when it was first.
        place: Place,
    },
    /// Each number without a unit of a length field written with the units of the clean
    /// snapshot.
    Units {
        /// The units of the clean snapshot.
        old: Units,
    },
    /// The rollback marker set back.
    Rollback {
        /// The marker of the clean snapshot.
        marker: Option<Id>,
    },
}

/// How the label of a revert of fields names them.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum FieldsLabel {
    /// One field with a short value: `Set {label} back to {old}`.
    Value {
        /// The field's manifest label.
        label: String,
        /// Its value or expression in the clean snapshot, as a label writes it.
        old: String,
    },
    /// One field without a short value: `Restore {label} of '{feature}'`.
    Field(String),
    /// Several fields: `Restore the settings of '{feature}'`.
    Settings,
}

/// What `Show` of a primary change shows.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Show {
    /// A sketch entity, by its kind.
    Entity {
        /// The sketch.
        sketch: Id,
        /// The entity.
        entity: Id,
        /// `line`, `circle`, `arc` or `point`.
        kind: &'static str,
    },
    /// A feature.
    Feature(Id),
}

/// The state of a document's undo history that `Undo` of a cause depends on.
#[derive(Debug, Clone, Copy)]
pub(super) struct UndoState {
    /// The serial of the newest undo entry.
    pub(super) newest: Option<u64>,
    /// True while a transaction is open.
    pub(super) transaction_open: bool,
}

/// One action of a cause: its label and how it runs.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CauseOffer {
    /// `apply_fix` applies these edits as one batch labeled `label`.
    Apply {
        /// The label.
        label: String,
        /// The edits, built from the current records.
        edits: Vec<Edit>,
    },
    /// The host undoes the newest entry.
    Undo,
    /// The host shows a feature, and one of its sketch entities.
    Show {
        /// The label.
        label: String,
        /// The feature.
        feature: Id,
        /// The sketch entity.
        entity: Option<Id>,
    },
}

impl CauseOffer {
    /// The protocol action; an `apply` action carries the `FixRef` that `fix` makes.
    pub(crate) fn action(&self, fix: impl FnOnce() -> FixRef) -> FixAction {
        match self {
            CauseOffer::Apply { label, .. } => FixAction {
                label: label.clone(),
                run: FixKind::Apply { fix: fix() },
            },
            CauseOffer::Undo => FixAction {
                label: "Undo".to_string(),
                run: FixKind::Undo,
            },
            CauseOffer::Show {
                label,
                feature,
                entity,
            } => FixAction {
                label: label.clone(),
                run: FixKind::Show {
                    feature: feature.clone(),
                    entity: entity.clone(),
                },
            },
        }
    }
}

impl MadeCause {
    /// The cause whose primary change is `change`, worded by `words`.
    pub(super) fn of(
        words: &Words<'_>,
        change: &Change,
        text: String,
        entry: Option<u64>,
        display: bool,
    ) -> MadeCause {
        let feature = match change {
            Change::Parameter(_) | Change::Units | Change::Rollback(_) => None,
            other => other.feature().cloned(),
        };
        let message_feature = words.comparison().feature;
        MadeCause {
            text,
            feature,
            revert: revert(words, change),
            show: show(words, change),
            entry,
            clean_placement: words
                .baseline(Side::Clean)
                .sketch(message_feature)
                .map(|result| result.placement),
            ghost: display.then(|| ghost(words, change)).flatten(),
        }
    }

    /// The cause's actions now (section 3.21, "Cause actions"), for the message `message` of the
    /// feature `view`, whose repairs are `repairs`: the revert of the primary change, `Keep the
    /// sketch where it was`, `Undo`, then `Show` when the primary change has no revert; an action
    /// whose edits equal those of an action of the message is left out.
    pub(super) fn offers(
        &self,
        view: &FeatureView<'_>,
        message: &Message,
        repairs: &[Repair],
        undo: UndoState,
    ) -> Vec<CauseOffer> {
        let reverts = self.reverts(view);
        let has_revert = !reverts.is_empty();
        let applied: Vec<(String, Vec<Edit>)> = reverts
            .into_iter()
            .chain(
                self.keep(view, message)
                    .map(|edits| (KEEP_LABEL.to_string(), edits)),
            )
            .collect();
        let offered = if applied.is_empty() {
            Vec::new()
        } else {
            offered_edits(view, message, repairs)
        };
        let mut offers: Vec<CauseOffer> = applied
            .into_iter()
            .filter(|(_, edits)| !offered.contains(edits))
            .map(|(label, edits)| CauseOffer::Apply { label, edits })
            .collect();
        if self.entry.is_some() && self.entry == undo.newest && !undo.transaction_open {
            offers.push(CauseOffer::Undo);
        }
        if !has_revert && let Some(show) = self.show_offer(view) {
            offers.push(show);
        }
        offers
    }

    /// The revert of the primary change as it applies to the current records, with its label;
    /// one per number without a unit for a units change; none when it would change nothing or
    /// its feature, parameter or neighbor is gone.
    fn reverts(&self, view: &FeatureView<'_>) -> Vec<(String, Vec<Edit>)> {
        let run = view.run;
        let single = match self.revert.as_ref() {
            None => None,
            Some(Revert::Fields {
                feature,
                fields,
                label,
            }) => fields_revert(view, feature, fields, label),
            Some(Revert::Dimension {
                sketch,
                constraint,
                expr,
                kind,
                old,
            }) => run
                .document
                .feature(sketch)
                .and_then(|record| dimension_set_back(record, constraint, expr))
                .map(|edit| {
                    let label = format!("Set {kind} of '{}' back to {old}", run.label(sketch));
                    (label, edit)
                }),
            Some(Revert::Parameter {
                parameter,
                expr,
                old,
            }) => parameter_set_back(run.document, parameter, expr, old),
            Some(Revert::Suppression {
                feature,
                suppressed,
            }) => suppression_set_back(run, feature, *suppressed),
            Some(Revert::Move { feature, place }) => {
                moved_back(run.document, feature, place).map(|edit| {
                    let (side, beside) = match place {
                        Place::Below(beside) => ("below", beside),
                        Place::Above(beside) => ("above", beside),
                    };
                    let label = format!(
                        "Move '{}' back {side} '{}'",
                        run.label(feature),
                        run.label(beside)
                    );
                    (label, edit)
                })
            }
            Some(Revert::Units { old }) => return units_reverts(view, *old),
            Some(Revert::Rollback { marker }) => rollback_set_back(run, marker.as_ref()),
        };
        single
            .into_iter()
            .map(|(label, edit)| (label, vec![edit]))
            .collect()
    }

    /// The edits of `Keep the sketch where it was` (section 3.21, cause action 2): for a sketch
    /// that lost its face, when where it stood when it last built without the message differs
    /// from where it stands now, one `update_feature` of its plane storing that placement as
    /// its fallback in body coordinates (the motion of its stored face query in the state just
    /// before it undone) with `fallback_frame: body`.
    fn keep(&self, view: &FeatureView<'_>, message: &Message) -> Option<Vec<Edit>> {
        if message.code != PLANE_FALLBACK {
            return None;
        }
        let clean = self.clean_placement?;
        let now = built_sketch(view.report)?.placement;
        if same_placement(&clean, &now) {
            return None;
        }
        let SketchPlane::Face { body, face, .. } = stored_drawing(view.record)?.plane else {
            return None;
        };
        let motion = view.state_before(&body).motion(&face.query);
        let plane = kept_face_plane(&body, &face, &clean, &motion, &motion)?;
        let value = serde_json::to_value(plane).ok()?;
        updated(view.record, |params| set(params, "/plane", value))
            .ok()
            .map(|edit| vec![edit])
    }

    /// `Show` of the primary change: the sketch entity it changed, else the feature it changed
    /// when that is in the document and is not the message's feature.
    fn show_offer(&self, view: &FeatureView<'_>) -> Option<CauseOffer> {
        match self.show.as_ref()? {
            Show::Entity {
                sketch,
                entity,
                kind,
            } => Some(CauseOffer::Show {
                label: format!("Show the {kind}"),
                feature: sketch.clone(),
                entity: Some(entity.clone()),
            }),
            Show::Feature(feature) => (feature != view.id()
                && view.run.document.feature(feature).is_some())
            .then(|| CauseOffer::Show {
                label: format!("Show '{}'", view.run.label(feature)),
                feature: feature.clone(),
                entity: None,
            }),
        }
    }
}

/// The revert of a primary change (section 3.21, "Reverts"): deleted features, sketch entity and
/// constraint changes other than a dimension's value, and asset changes have none.
fn revert(words: &Words<'_>, change: &Change) -> Option<Revert> {
    let clean = words.baseline(Side::Clean);
    let suppression = |feature: &Id, suppressed: bool| {
        Some(Revert::Suppression {
            feature: feature.clone(),
            suppressed,
        })
    };
    match change {
        Change::Added(feature) | Change::Unsuppressed(feature) => suppression(feature, true),
        Change::Suppressed(feature) => suppression(feature, false),
        Change::Deleted(_) => None,
        Change::Record(record) => record_revert(words, record),
        Change::Moved(feature) => {
            let position = clean.position(feature)?;
            let timeline = &clean.document.features;
            let place = match position.checked_sub(1) {
                Some(above) => Place::Below(timeline[above].id.clone()),
                None => Place::Above(timeline.get(position + 1)?.id.clone()),
            };
            Some(Revert::Move {
                feature: feature.clone(),
                place,
            })
        }
        Change::Rollback(_) => Some(Revert::Rollback {
            marker: clean.document.rollback.clone(),
        }),
        Change::Parameter(parameter) => Some(Revert::Parameter {
            parameter: parameter.clone(),
            expr: clean.document.parameter(parameter)?.expr.clone(),
            old: words.parameter_value(Side::Clean, parameter)?.written,
        }),
        Change::Units => Some(Revert::Units { old: clean.units() }),
    }
}

/// The revert of a record change: of a dimension's value when that is the change named, else of
/// its changed fields, unless the first is an asset field or a sketch entity or other
/// constraint change is named.
fn record_revert(words: &Words<'_>, record: &RecordChange) -> Option<Revert> {
    let feature = &record.feature;
    if let Some(item) = record.items.first() {
        let SketchItem::Dimension(constraint) = item else {
            return None;
        };
        let before = stored_drawing(words.baseline(Side::Clean).record(feature)?)?;
        let dimension = before.constraint(constraint)?;
        return Some(Revert::Dimension {
            sketch: feature.clone(),
            constraint: constraint.clone(),
            expr: dimension.dimension()?.expr.clone(),
            kind: dimension.type_label(),
            old: words
                .dimension_value(Side::Clean, feature, constraint)?
                .written,
        });
    }
    let first = record.fields.first()?;
    if words.is_asset_field(feature, first) {
        return None;
    }
    let before = words.baseline(Side::Clean).record(feature)?;
    let mut fields: Vec<(String, Option<Value>)> = record
        .fields
        .iter()
        .map(|hint| {
            let pointer = field_pointer(&hint.path);
            match value_at(&before.params, &pointer) {
                Some(old) => (pointer, Some(old.clone())),
                None => (absent_root(&before.params, &pointer).to_string(), None),
            }
        })
        .collect();
    fields.dedup();
    let label = match record.fields.as_slice() {
        [only] => match words.field_value(Side::Clean, feature, only) {
            Some(old) => FieldsLabel::Value {
                label: only.label.clone(),
                old: old.written,
            },
            None => FieldsLabel::Field(only.label.clone()),
        },
        _ => FieldsLabel::Settings,
    };
    Some(Revert::Fields {
        feature: feature.clone(),
        fields,
        label,
    })
}

/// What `Show` shows of a primary change: the sketch entity a record change names first, else
/// the feature the change belongs to; nothing for a deleted feature or a change of the document.
fn show(words: &Words<'_>, change: &Change) -> Option<Show> {
    match change {
        Change::Record(record) => match record.items.first() {
            Some(
                SketchItem::EntityDeleted(entity)
                | SketchItem::EntityMoved(entity)
                | SketchItem::EntityConstruction(entity, _)
                | SketchItem::EntityAdded(entity),
            ) => {
                let side = if matches!(record.items.first(), Some(SketchItem::EntityAdded(_))) {
                    Side::Now
                } else {
                    Side::Clean
                };
                let geometry = words.entity_geometry(side, &record.feature, entity)?;
                Some(Show::Entity {
                    sketch: record.feature.clone(),
                    entity: entity.clone(),
                    kind: kind_word(geometry.kind()),
                })
            }
            _ => Some(Show::Feature(record.feature.clone())),
        },
        Change::Added(feature)
        | Change::Suppressed(feature)
        | Change::Unsuppressed(feature)
        | Change::Moved(feature) => Some(Show::Feature(feature.clone())),
        Change::Deleted(_) | Change::Rollback(_) | Change::Parameter(_) | Change::Units => None,
    }
}

/// The ghost of a primary change that deleted or moved a sketch entity: the entity as the clean
/// snapshot had it.
fn ghost(words: &Words<'_>, change: &Change) -> Option<GhostShape> {
    let Change::Record(record) = change else {
        return None;
    };
    let (SketchItem::EntityDeleted(entity) | SketchItem::EntityMoved(entity)) =
        record.items.first()?
    else {
        return None;
    };
    let geometry = words.entity_geometry(Side::Clean, &record.feature, entity)?;
    Some(GhostShape::of_entity(&geometry.geometry))
}

/// The word of an entity kind in a label: `line`, `circle`, `arc` or `point`.
fn kind_word(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::Point => "point",
        EntityKind::Line => "line",
        EntityKind::Circle => "circle",
        EntityKind::Arc => "arc",
    }
}

/// The shortest part of `pointer` that `params` lacks, which a revert removes so that a field
/// the clean record left out (with the object holding it) is left out again; `pointer` itself
/// when `params` has every part of it but the last.
fn absent_root<'p>(params: &Map<String, Value>, pointer: &'p str) -> &'p str {
    pointer
        .match_indices('/')
        .skip(1)
        .map(|(end, _)| &pointer[..end])
        .find(|part| value_at(params, part).is_none())
        .unwrap_or(pointer)
}

/// One `update_feature` of `record` with each field set back to its value in the clean snapshot
/// (or removed when it had none); `None` when that changes nothing or a field has no place.
fn restored(record: &FeatureRecord, fields: &[(String, Option<Value>)]) -> Option<Edit> {
    let edit = updated(record, |params| {
        fields.iter().all(|(pointer, old)| match old {
            Some(value) => set(params, pointer, value.clone()),
            None => {
                remove(params, pointer);
                true
            }
        })
    })
    .ok()?;
    let changes =
        matches!(&edit, Edit::UpdateFeature { feature } if feature.params != record.params);
    changes.then_some(edit)
}

/// The revert of fields of `feature`, labeled as `named` says, the feature's name left out of a
/// short value's label when it is the message's feature `view`; `None` when the feature is gone
/// or the revert changes nothing.
fn fields_revert(
    view: &FeatureView<'_>,
    feature: &Id,
    fields: &[(String, Option<Value>)],
    named: &FieldsLabel,
) -> Option<(String, Edit)> {
    let record = view.run.document.feature(feature)?;
    let label = match named {
        FieldsLabel::Value { label: field, old } if feature == view.id() => {
            format!("Set {field} back to {old}")
        }
        FieldsLabel::Value { label: field, old } => {
            format!("Set {field} of '{}' back to {old}", view.run.label(feature))
        }
        FieldsLabel::Field(field) => format!("Restore {field} of '{}'", view.run.label(feature)),
        FieldsLabel::Settings => format!("Restore the settings of '{}'", view.run.label(feature)),
    };
    Some((label, restored(record, fields)?))
}

/// `Set '{parameter}' back to {old}`: one `update_parameter` of `parameter` with its expression
/// set back to `expr`; `None` when the parameter is gone or already holds `expr`.
fn parameter_set_back(
    document: &Document,
    parameter: &Id,
    expr: &Expr,
    old: &str,
) -> Option<(String, Edit)> {
    let current = document.parameter(parameter)?;
    let edit = (current.expr != *expr).then(|| Edit::UpdateParameter {
        parameter: Parameter {
            expr: expr.clone(),
            ..current.clone()
        },
    })?;
    Some((format!("Set '{}' back to {old}", current.name), edit))
}

/// `Suppress '{feature}'` or `Unsuppress '{feature}'`: one `update_feature` of `feature` with
/// `suppressed`; `None` when the feature is gone or already has it.
fn suppression_set_back(
    run: &RunView<'_>,
    feature: &Id,
    suppressed: bool,
) -> Option<(String, Edit)> {
    let record = run
        .document
        .feature(feature)
        .filter(|record| record.suppressed != suppressed)?;
    let edit = Edit::UpdateFeature {
        feature: FeatureRecord {
            suppressed,
            ..record.clone()
        },
    };
    let verb = if suppressed { "Suppress" } else { "Unsuppress" };
    Some((format!("{verb} '{}'", run.label(feature)), edit))
}

/// The rollback marker set back to `marker`, above a feature or at the end; `None` when that
/// feature is gone or the marker is there already.
fn rollback_set_back(run: &RunView<'_>, marker: Option<&Id>) -> Option<(String, Edit)> {
    let document = run.document;
    let label = match marker {
        Some(feature) if document.feature(feature).is_none() => return None,
        Some(feature) => format!(
            "Move the rollback marker back above '{}'",
            run.label(feature)
        ),
        None => "Move the rollback marker back to the end".to_string(),
    };
    let edit = (document.rollback.as_ref() != marker).then(|| Edit::SetRollback {
        rollback: marker.cloned(),
    })?;
    Some((label, edit))
}

/// One `update_feature` of a sketch's record with the expression of the dimension `constraint`
/// set back to `expr`; `None` when the sketch no longer has it or it already holds `expr`.
fn dimension_set_back(record: &FeatureRecord, constraint: &Id, expr: &Expr) -> Option<Edit> {
    let pointer = format!("{}/expr", constraint_pointer(record, constraint)?);
    if value_at(&record.params, &pointer).and_then(Value::as_str) == Some(expr.as_str()) {
        return None;
    }
    let value = Value::String(expr.as_str().to_string());
    updated(record, |params| set(params, &pointer, value)).ok()
}

/// One `move_feature` of `feature` to directly below (or above) its neighbor in the clean
/// snapshot, found by id now; `None` when either is gone or it is there already.
fn moved_back(document: &Document, feature: &Id, place: &Place) -> Option<Edit> {
    let current = document.feature_index(feature)?;
    let index = index_beside(document, feature, place)?;
    (index != current).then(|| Edit::MoveFeature {
        id: feature.clone(),
        index,
    })
}

/// The reverts of a units change (section 3.21): `Use {expr} {old unit}` for each length field of
/// the message's feature whose expression is a number without a unit, in manifest order, each
/// one `update_feature` with that field written with the old unit.
fn units_reverts(view: &FeatureView<'_>, old: Units) -> Vec<(String, Vec<Edit>)> {
    let record = view.record;
    let Some(feature) = view.run.registry.get(&record.kind) else {
        return Vec::new();
    };
    feature
        .manifest()
        .fields
        .iter()
        .filter(|hint| hint.unit == Some(FieldUnit::Length) && !hint.path.contains('*'))
        .filter_map(|hint| {
            let pointer = field_pointer(&hint.path);
            let written = Expr::new(value_at(&record.params, &pointer)?.as_str()?);
            let kept = written.with_unit(old.unit())?;
            let value = Value::String(kept.as_str().to_string());
            let edit = updated(record, |params| set(params, &pointer, value)).ok()?;
            Some((format!("Use {}", kept.as_str()), vec![edit]))
        })
        .collect()
}

/// True when two placements are the same within the modeling tolerances: origins within
/// `LINEAR_TOLERANCE`, axes within `DIRECTION_TOLERANCE`.
fn same_placement(first: &Placement, second: &Placement) -> bool {
    let close = |one: [f64; 3], other: [f64; 3], tolerance: f64| {
        one.iter()
            .zip(other)
            .all(|(component, matching)| (component - matching).abs() <= tolerance)
    };
    close(first.origin, second.origin, LINEAR_TOLERANCE)
        && close(first.x_axis, second.x_axis, DIRECTION_TOLERANCE)
        && close(first.normal, second.normal, DIRECTION_TOLERANCE)
}

#[cfg(test)]
mod tests {
    use partsmith_model::Units;

    use super::*;
    use crate::test_support::id;

    /// A fillet record with these params.
    fn fillet(params: Value) -> FeatureRecord {
        let mut record = serde_json::json!({"id": "f-fil1", "type": "fillet"});
        if let (Value::Object(record), Value::Object(params)) = (&mut record, params) {
            record.extend(params);
        }
        serde_json::from_value(record).expect("a well-formed record")
    }

    #[test]
    fn restoring_fields_sets_back_their_values_and_removes_new_ones() {
        let record = fillet(serde_json::json!({"radius": "30 mm", "extra": true}));

        let edit = restored(
            &record,
            &[
                (
                    "/radius".to_string(),
                    Some(Value::String("2 mm".to_string())),
                ),
                ("/extra".to_string(), None),
            ],
        );

        let Some(Edit::UpdateFeature { feature }) = edit else {
            panic!("one update_feature");
        };
        assert_eq!(
            feature.params.get("radius"),
            Some(&serde_json::json!("2 mm"))
        );
        assert!(!feature.params.contains_key("extra"));
    }

    #[test]
    fn a_field_the_clean_record_left_out_is_removed_with_the_object_holding_it() {
        let clean = fillet(serde_json::json!({"extent": {"kind": "distance"}}));

        let nested = absent_root(&clean.params, "/hole_type/kind");
        let inner = absent_root(&clean.params, "/extent/expr");

        assert_eq!(nested, "/hole_type");
        assert_eq!(inner, "/extent/expr");
    }

    #[test]
    fn a_restore_that_changes_nothing_is_no_revert() {
        let record = fillet(serde_json::json!({"radius": "2 mm"}));

        let edit = restored(
            &record,
            &[(
                "/radius".to_string(),
                Some(Value::String("2 mm".to_string())),
            )],
        );

        assert_eq!(edit, None);
    }

    #[test]
    fn a_feature_already_beside_its_neighbor_is_not_moved() {
        let mut document = Document::new(Units::Mm);
        for feature in ["f-box1", "f-box2", "f-fil1"] {
            document
                .features
                .push(FeatureRecord::new(id(feature), "box"));
        }

        let back = moved_back(&document, &id("f-box2"), &Place::Below(id("f-fil1")));
        let stays = moved_back(&document, &id("f-box2"), &Place::Below(id("f-box1")));

        assert_eq!(
            back,
            Some(Edit::MoveFeature {
                id: id("f-box2"),
                index: 2
            })
        );
        assert_eq!(stays, None);
    }

    #[test]
    fn placements_within_tolerance_are_the_same() {
        let top = Placement {
            origin: [0.0, 0.0, 40.0],
            x_axis: [1.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        };
        let nudged = Placement {
            origin: [0.0, 0.0, 40.0 + 1e-9],
            ..top
        };
        let higher = Placement {
            origin: [0.0, 0.0, 80.0],
            ..top
        };

        assert!(same_placement(&top, &nudged));
        assert!(!same_placement(&top, &higher));
    }
}
