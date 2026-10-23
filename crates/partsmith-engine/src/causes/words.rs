//! The words of a cause line (engine.md section 3.21, "Text"): each kind of change in words,
//! with the values of fields, dimensions and parameters in the clean snapshot and now.

use partsmith_features::{FieldHint, FieldUnit, Registry, entity_words, format_value};
use partsmith_model::{Expr, Id, SelectionKind};
use partsmith_sketch::{DimensionValues, SketchConstraint, SketchEntity, SketchParams, ValueKind};
use serde_json::Value;

use super::Tombstone;
use super::baseline::{Baseline, Evaluation};
use super::changes::{Change, Comparison, RecordChange, SketchItem};
use crate::fixes::is_plain_value;
use crate::params::{field_pointer, value_at};
use crate::sketch::{SKETCH_TYPE, SketchWords, stored_drawing};
use crate::state::feature_label;

/// One side of the comparison: the clean snapshot or now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Side {
    /// The clean snapshot.
    Clean,
    /// The recompute now.
    Now,
}

/// A short value of a field, dimension or parameter (section 3.21): its words in a cause's text
/// and in a revert's label.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ShortValue {
    /// The words in the text: an expression's value, followed by the expression in parentheses
    /// when it is not a plain value; an option's label; `on` or `off`; a body's name.
    pub(super) shown: String,
    /// The words in a revert's label: an expression that is not a plain value as written, else
    /// what `shown` says.
    pub(super) written: String,
}

/// The words of the changes of one comparison.
pub(super) struct Words<'a> {
    /// The comparison.
    comparison: &'a Comparison<'a>,
    /// The clean snapshot's expressions as its recompute evaluated them.
    clean: Evaluation<'a>,
    /// The expressions now.
    now: Evaluation<'a>,
}

impl<'a> Words<'a> {
    /// The words of `comparison`'s changes.
    pub(super) fn new(comparison: &'a Comparison<'a>) -> Words<'a> {
        Words {
            comparison,
            clean: comparison.clean.baseline().evaluation(),
            now: comparison.now.baseline().evaluation(),
        }
    }

    /// The comparison the words are of.
    pub(super) fn comparison(&self) -> &'a Comparison<'a> {
        self.comparison
    }

    /// The registry.
    fn registry(&self) -> &'a Registry {
        self.comparison.registry()
    }

    /// The baseline of a side.
    pub(super) fn baseline(&self, side: Side) -> &'a Baseline {
        match side {
            Side::Clean => self.comparison.clean.baseline(),
            Side::Now => self.comparison.now.baseline(),
        }
    }

    /// The evaluation of a side.
    fn evaluation(&self, side: Side) -> &Evaluation<'a> {
        match side {
            Side::Clean => &self.clean,
            Side::Now => &self.now,
        }
    }

    /// `{change}` of a change (section 3.21, "Text").
    pub(super) fn change(&self, change: &Change) -> String {
        match change {
            Change::Added(feature) => {
                let label = self.label(feature);
                let now = self.baseline(Side::Now);
                if now.position(feature) < now.position(self.comparison.feature) {
                    format!("added '{label}' before {}", self.this())
                } else {
                    format!("added '{label}'")
                }
            }
            Change::Deleted(feature) => self.deleted(feature),
            Change::Suppressed(feature) => format!("suppressed '{}'", self.label(feature)),
            Change::Unsuppressed(feature) => format!("unsuppressed '{}'", self.label(feature)),
            Change::Record(record) => self.record(record),
            Change::Moved(feature) => self.moved(feature),
            Change::Rollback(_) => match &self.baseline(Side::Now).document.rollback {
                Some(marker) => format!("moved the rollback marker above '{}'", self.label(marker)),
                None => "moved the rollback marker to the end".to_string(),
            },
            Change::Parameter(parameter) => {
                let name = self
                    .baseline(Side::Now)
                    .document
                    .parameter(parameter)
                    .map_or_else(|| parameter.to_string(), |found| found.name.clone());
                match (
                    self.parameter_value(Side::Clean, parameter),
                    self.parameter_value(Side::Now, parameter),
                ) {
                    (Some(old), Some(new)) => format!(
                        "changed parameter '{name}' from {} to {}",
                        old.shown, new.shown
                    ),
                    _ => format!("changed parameter '{name}'"),
                }
            }
            Change::Units => format!(
                "changed the document units from {} to {}",
                self.baseline(Side::Clean).units().symbol(),
                self.baseline(Side::Now).units().symbol()
            ),
        }
    }

    /// The label of a feature now.
    pub(super) fn label(&self, feature: &Id) -> String {
        feature_label(self.registry(), &self.baseline(Side::Now).document, feature)
    }

    /// `{this}`: `this` and the message's feature's type in lowercase, `this fillet`.
    fn this(&self) -> String {
        let feature = self.comparison.feature;
        let kind = self
            .baseline(Side::Now)
            .record(feature)
            .and_then(|record| self.registry().get(&record.kind))
            .map_or_else(
                || "feature".to_string(),
                |found| found.manifest().label.to_lowercase(),
            );
        format!("this {kind}")
    }

    /// The words of a deleted feature: `deleted '{feature}'` with the label it had, or `deleted
    /// the former '{feature}'` when a feature in the document has that label now.
    fn deleted(&self, feature: &Id) -> String {
        let label = match self.comparison.tombstones.get(feature) {
            Some(Tombstone { label, .. }) => label.clone(),
            None => feature_label(
                self.registry(),
                &self.baseline(Side::Clean).document,
                feature,
            ),
        };
        let now = &self.baseline(Side::Now).document;
        let taken = now
            .features
            .iter()
            .any(|record| feature_label(self.registry(), now, &record.id) == label);
        if taken {
            format!("deleted the former '{label}'")
        } else {
            format!("deleted '{label}'")
        }
    }

    /// The words of a moved feature: `moved '{feature}' above '{other}'` or `below`, `{other}`
    /// being, of the features whose order relative to it changed, the one the message names,
    /// else the message's feature, else the one nearest its new position.
    fn moved(&self, feature: &Id) -> String {
        let clean = self.baseline(Side::Clean);
        let now = self.baseline(Side::Now);
        let swapped: Vec<&Id> = now
            .document
            .features
            .iter()
            .map(|record| &record.id)
            .filter(|other| *other != feature && clean.record(other).is_some())
            .filter(|other| self.comparison.order_swapped(feature, other))
            .collect();
        let here = now.position(feature).unwrap_or_default();
        let other = self
            .comparison
            .named
            .iter()
            .find(|candidate| swapped.contains(candidate))
            .or_else(|| {
                swapped
                    .iter()
                    .copied()
                    .find(|candidate| *candidate == self.comparison.feature)
            })
            .or_else(|| {
                swapped.iter().copied().min_by_key(|candidate| {
                    now.position(candidate).unwrap_or_default().abs_diff(here)
                })
            });
        match other {
            Some(other) => {
                let side = if now.position(feature) < now.position(other) {
                    "above"
                } else {
                    "below"
                };
                format!(
                    "moved '{}' {side} '{}'",
                    self.label(feature),
                    self.label(other)
                )
            }
            None => format!("moved '{}'", self.label(feature)),
        }
    }

    /// The words of a record change: its first item.
    fn record(&self, record: &RecordChange) -> String {
        let feature = &record.feature;
        if let Some(item) = record.items.first() {
            return self.sketch_item(feature, item);
        }
        let Some(hint) = record.fields.first() else {
            return format!("changed '{}'", self.label(feature));
        };
        if self.is_asset_field(feature, hint) {
            return format!("replaced the file of '{}'", self.label(feature));
        }
        let of = if feature == self.comparison.feature {
            String::new()
        } else {
            format!(" of '{}'", self.label(feature))
        };
        match (
            self.field_value(Side::Clean, feature, hint),
            self.field_value(Side::Now, feature, hint),
        ) {
            (Some(old), Some(new)) => format!(
                "changed {}{of} from {} to {}",
                hint.label, old.shown, new.shown
            ),
            _ => format!("changed {}{of}", hint.label),
        }
    }

    /// The words of a changed entity or constraint of a sketch.
    fn sketch_item(&self, sketch: &Id, item: &SketchItem) -> String {
        let label = self.label(sketch);
        match item {
            SketchItem::EntityDeleted(entity) => {
                format!(
                    "deleted {} in '{label}'",
                    self.entity(Side::Clean, sketch, entity)
                )
            }
            SketchItem::EntityAdded(entity) => {
                format!(
                    "added {} in '{label}'",
                    self.entity(Side::Now, sketch, entity)
                )
            }
            SketchItem::EntityMoved(entity) => {
                format!(
                    "moved {} in '{label}'",
                    self.entity(Side::Clean, sketch, entity)
                )
            }
            SketchItem::EntityConstruction(entity, construction) => {
                let made = if *construction {
                    "construction geometry"
                } else {
                    "part of the shape"
                };
                format!(
                    "made {} in '{label}' {made}",
                    self.entity(Side::Clean, sketch, entity)
                )
            }
            SketchItem::ConstraintAdded(constraint) => {
                format!(
                    "added {} in '{label}'",
                    self.title(Side::Now, sketch, constraint)
                )
            }
            SketchItem::ConstraintDeleted(constraint) => {
                format!(
                    "deleted {} in '{label}'",
                    self.title(Side::Clean, sketch, constraint)
                )
            }
            // Named by its title on the side where it drives, with the value it holds there.
            SketchItem::Driving(constraint, true) => {
                format!(
                    "made {} in '{label}' a driving dimension",
                    self.title(Side::Now, sketch, constraint)
                )
            }
            SketchItem::Driving(constraint, false) => {
                format!(
                    "made {} in '{label}' a reference",
                    self.title(Side::Clean, sketch, constraint)
                )
            }
            SketchItem::Dimension(constraint) => {
                let kind = self
                    .constraint(Side::Now, sketch, constraint)
                    .map_or_else(|| "Dimension".to_string(), |found| found.type_label());
                match (
                    self.dimension_value(Side::Clean, sketch, constraint),
                    self.dimension_value(Side::Now, sketch, constraint),
                ) {
                    (Some(old), Some(new)) => format!(
                        "changed {kind} of '{label}' from {} to {}",
                        old.shown, new.shown
                    ),
                    _ => format!("changed {kind} of '{label}'"),
                }
            }
        }
    }

    /// A sketch entity in words, as one side's solution has it, else as its record stores it.
    fn entity(&self, side: Side, sketch: &Id, entity: &Id) -> String {
        self.entity_geometry(side, sketch, entity).map_or_else(
            || "an entity".to_string(),
            |found| entity_words(&found.geometry, self.baseline(Side::Now).units()),
        )
    }

    /// A sketch entity as one side's solution has it, else as its record stores it.
    pub(super) fn entity_geometry(
        &self,
        side: Side,
        sketch: &Id,
        entity: &Id,
    ) -> Option<SketchEntity> {
        let baseline = self.baseline(side);
        let solved = baseline.sketch(sketch).and_then(|result| {
            result
                .solved
                .entities
                .iter()
                .find(|found| found.id == *entity)
        });
        match solved {
            Some(found) => Some(found.clone()),
            None => stored_drawing(baseline.record(sketch)?)?
                .entity(entity)
                .cloned(),
        }
    }

    /// The drawing of a sketch on one side.
    fn drawing(&self, side: Side, sketch: &Id) -> Option<SketchParams> {
        stored_drawing(self.baseline(side).record(sketch)?)
    }

    /// A constraint of a sketch on one side.
    fn constraint(&self, side: Side, sketch: &Id, constraint: &Id) -> Option<SketchConstraint> {
        self.drawing(side, sketch)?.constraint(constraint).cloned()
    }

    /// The title of a constraint as the sketch crate renders it for one side's sketch (sketch.md
    /// section 4): `Distance 30 mm`.
    fn title(&self, side: Side, sketch: &Id, constraint: &Id) -> String {
        let Some(params) = self.drawing(side, sketch) else {
            return "a constraint".to_string();
        };
        let baseline = self.baseline(side);
        let solve = baseline.sketch(sketch).map(|result| &result.solved);
        let values = self.dimension_values(side, &params);
        let label = feature_label(self.registry(), &baseline.document, sketch);
        SketchWords::new(label, self.baseline(Side::Now).units())
            .context(&params, &values, solve)
            .title(constraint)
            .unwrap_or_else(|| "a constraint".to_string())
    }

    /// The driving dimension values of a sketch on one side, those that evaluate.
    fn dimension_values(&self, side: Side, params: &SketchParams) -> DimensionValues {
        params
            .driving_dimensions()
            .into_iter()
            .filter_map(|input| {
                let value = self.value(side, SKETCH_TYPE, input.expr, field_unit(input.kind))?;
                Some((input.constraint.clone(), value))
            })
            .collect()
    }

    /// The value of an expression in a field of `kind` of a feature of type `feature_type` on
    /// one side, as that feature evaluates it.
    fn value(&self, side: Side, feature_type: &str, expr: &Expr, kind: FieldUnit) -> Option<f64> {
        let manifest = self.registry().get(feature_type)?.manifest();
        self.evaluation(side).value(manifest, expr, kind)
    }

    /// The short value of a dimension of a sketch on one side.
    pub(super) fn dimension_value(
        &self,
        side: Side,
        sketch: &Id,
        constraint: &Id,
    ) -> Option<ShortValue> {
        let found = self.constraint(side, sketch, constraint)?;
        let dimension = found.dimension()?;
        let kind = field_unit(found.value_kind()?);
        let value = self.value(side, SKETCH_TYPE, &dimension.expr, kind)?;
        Some(self.expression_words(side, &dimension.expr, kind, value))
    }

    /// The short value of a parameter on one side.
    pub(super) fn parameter_value(&self, side: Side, parameter: &Id) -> Option<ShortValue> {
        let found = self.baseline(side).document.parameter(parameter)?;
        let quantity = self.evaluation(side).parameter(&found.name)?;
        let shown = quantity.display(self.baseline(side).units());
        Some(with_expression(&found.expr, shown))
    }

    /// The short value of a manifest field of a feature on one side (section 3.21): an
    /// expression field's value, an option field's option label, a toggle's `on` or `off`, a body
    /// field's body name in quotes; `None` for every other field and for an expression that does
    /// not evaluate.
    pub(super) fn field_value(
        &self,
        side: Side,
        feature: &Id,
        hint: &FieldHint,
    ) -> Option<ShortValue> {
        let record = self.baseline(side).record(feature)?;
        let value = value_at(&record.params, &field_pointer(&hint.path));
        if let Some(kind) = hint.unit {
            let expr = Expr::new(value?.as_str()?);
            let evaluated = self.value(side, &record.kind, &expr, kind)?;
            return Some(self.expression_words(side, &expr, kind, evaluated));
        }
        if !hint.options.is_empty() {
            // A record leaves out an option field that holds its default, the first option.
            let option = match value {
                Some(chosen) => {
                    let chosen = chosen.as_str()?;
                    hint.options.iter().find(|option| option.value == chosen)?
                }
                None => hint.options.first()?,
            };
            return Some(plain(option.label.clone()));
        }
        if hint.selection == Some(SelectionKind::Body) {
            let body = Id::new(value?.as_str()?).ok()?;
            let name = self.baseline(side).document.body(&body)?.name.clone();
            return Some(plain(format!("'{name}'")));
        }
        if hint.selection.is_none() && self.is_toggle(feature, hint) {
            let on = value.and_then(Value::as_bool).unwrap_or(false);
            return Some(plain(if on { "on" } else { "off" }.to_string()));
        }
        None
    }

    /// True when a field holds a toggle: a boolean on either side, absent meaning off.
    fn is_toggle(&self, feature: &Id, hint: &FieldHint) -> bool {
        self.comparison.graphs().iter().any(|graph| {
            graph.baseline().record(feature).is_some_and(|record| {
                value_at(&record.params, &field_pointer(&hint.path)).is_some_and(Value::is_boolean)
            })
        })
    }

    /// True when a manifest field of a feature names an asset it reads.
    pub(super) fn is_asset_field(&self, feature: &Id, hint: &FieldHint) -> bool {
        self.comparison.graphs().iter().any(|graph| {
            graph.dependencies(feature).is_some_and(|dependencies| {
                dependencies
                    .assets
                    .iter()
                    .any(|asset| asset.field == hint.path)
            })
        })
    }

    /// The words of an expression and its value in a field of `kind` on one side.
    fn expression_words(&self, side: Side, expr: &Expr, kind: FieldUnit, value: f64) -> ShortValue {
        with_expression(expr, format_value(kind, value, self.baseline(side).units()))
    }
}

/// The unit of a manifest field that holds a dimension's value of `kind`.
fn field_unit(kind: ValueKind) -> FieldUnit {
    match kind {
        ValueKind::Length => FieldUnit::Length,
        ValueKind::Angle => FieldUnit::Angle,
    }
}

/// The words of a value with its expression: the value, followed by the expression in
/// parentheses when it is not a plain value (`30 mm (d * 6)`); a revert writes such an
/// expression as it is.
fn with_expression(expr: &Expr, shown: String) -> ShortValue {
    let written_expression = expr.as_str().trim();
    if is_plain_value(written_expression) {
        plain(shown)
    } else {
        ShortValue {
            shown: format!("{shown} ({written_expression})"),
            written: written_expression.to_string(),
        }
    }
}

/// A short value that reads the same in a text and in a label.
fn plain(words: String) -> ShortValue {
    ShortValue {
        shown: words.clone(),
        written: words,
    }
}

/// `{others}` (section 3.21): empty, `, and 1 other change` or `, and {n} other changes`.
pub(super) fn others(count: usize) -> String {
    match count {
        0 => String::new(),
        1 => ", and 1 other change".to_string(),
        many => format!(", and {many} other changes"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn other_changes_are_counted_in_words() {
        assert_eq!(others(0), "");
        assert_eq!(others(1), ", and 1 other change");
        assert_eq!(others(3), ", and 3 other changes");
    }

    #[test]
    fn an_expression_that_is_not_a_plain_value_follows_its_value() {
        let plain_value = with_expression(&Expr::new("5 mm"), "5 mm".to_string());
        let named = with_expression(&Expr::new("d * 6"), "30 mm".to_string());

        assert_eq!(plain_value, plain("5 mm".to_string()));
        assert_eq!(named.shown, "30 mm (d * 6)");
        assert_eq!(named.written, "d * 6");
    }
}
