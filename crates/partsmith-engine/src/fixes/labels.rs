//! The label of each fix a message offers and how it runs (engine.md section 3.22): every label
//! starts with a verb and says exactly what changes, naming elements and regions by the words
//! their repair gives them.

use partsmith_features::{FieldRef, Fix, Names, Place, SelectionKind, joined};
use partsmith_model::{Expr, ExprNodeKind, FeatureRecord, Id};
use partsmith_naming::{ElementKind, Reference};
use partsmith_protocol::{Repair, RepairCandidate};
use partsmith_recompute::Message;
use partsmith_sketch::RegionRef;
use serde_json::Value;

use super::{Offer, OfferRun};
use crate::params::{field_path, is_list_entry, reference_at};
use crate::run::FeatureView;
use crate::words::region_words;

/// The code of the warning whose reference now covers every element its replacement makes
/// (features.md section 3.8), whose keep and use fixes read `Keep all` and `Use only`.
const WIDENED: &str = "reference_widened";

/// What labeling the fixes of one message reads: the feature, the message and its repairs.
pub(super) struct Labels<'a> {
    /// The feature of the message.
    pub(super) feature: &'a FeatureView<'a>,
    /// The message.
    pub(super) message: &'a Message,
    /// The message's repairs, whose words name the elements and regions fixes store.
    pub(super) repairs: &'a [Repair],
}

impl Labels<'_> {
    /// The offer of a fix, or None when no label can say exactly what it does, or when this
    /// phase builds no edits for it.
    pub(super) fn offer<'m>(&self, fix: &'m Fix) -> Option<Offer<'m>> {
        let apply = |label: String| {
            Some(Offer {
                label,
                run: OfferRun::Apply(fix),
            })
        };
        match fix {
            Fix::SetField {
                feature,
                field,
                value,
            } => apply(self.set_field(feature.as_ref(), field, value)?),
            Fix::UseReferences {
                pointer,
                references,
            } => {
                let verb = if self.widened() { "Use only" } else { "Use" };
                apply(format!("{verb} {}", self.references(pointer, references)?))
            }
            // A field that holds one reference cannot keep every element a widening covers.
            Fix::KeepMatch { pointer } if self.widened() => {
                if !is_list_entry(&self.feature.record.params, pointer) {
                    return None;
                }
                apply(format!(
                    "Keep all {}",
                    self.element_repair(pointer)?.1.len()
                ))
            }
            Fix::KeepMatch { .. } => apply("Keep this match".to_string()),
            Fix::RemoveEntry { pointer } => {
                apply(format!("Remove from {}", self.field_label(pointer)))
            }
            Fix::UseRegions { pointer, regions } => {
                apply(format!("Use {}", self.regions(pointer, regions)?))
            }
            Fix::KeepRegions { .. } => apply("Keep these regions".to_string()),
            Fix::UseBody { body, .. } => apply(format!("Use body '{}'", self.body_name(body))),
            Fix::Unsuppress { feature } => {
                apply(format!("Unsuppress '{}'", self.feature.run.label(feature)))
            }
            Fix::Suppress { feature } => {
                apply(format!("Suppress '{}'", self.feature.run.label(feature)))
            }
            Fix::Move { feature, place } => {
                let (side, other) = match place {
                    Place::Below(other) => ("below", other),
                    Place::Above(other) => ("above", other),
                };
                apply(format!(
                    "Move '{}' {side} '{}'",
                    self.feature.run.label(feature),
                    self.feature.run.label(other)
                ))
            }
            Fix::Pick { pointer } => Some(Offer {
                label: format!("Pick {} again", self.noun(pointer)),
                run: OfferRun::Pick {
                    feature: self.feature.id().clone(),
                    path: pointer.clone(),
                },
            }),
            Fix::Show { feature, entity } => Some(Offer {
                label: self.show(feature, entity.as_ref()),
                run: OfferRun::Show {
                    feature: feature.clone(),
                    entity: entity.clone(),
                },
            }),
            // Later phases of the reference stability plan build these fixes' edits: frames on
            // the model (attaching a sketch, turning its drawing back), the sketch fixes of the
            // newcomer errors (constraint titles, connecting ends), new bodies and renamed keys.
            // Until then no message offers them.
            Fix::NewBody
            | Fix::Attach { .. }
            | Fix::TurnDrawingBack { .. }
            | Fix::ConnectEnds { .. }
            | Fix::DeleteConstraint { .. }
            | Fix::MakeReference { .. }
            | Fix::RenameKey { .. } => None,
        }
    }

    /// True for a `reference_widened` message.
    fn widened(&self) -> bool {
        self.message.code == WIDENED
    }

    /// The label of a `SetField` fix (section 3.22): `Clear`, `Turn on` or `Turn off`, `Set
    /// {label} to {option label}`, `Use {value}`, or `Replace {expression} with {value}` when the
    /// field holds an expression that is not a plain value; for another feature `Set {label} of
    /// '{feature}' to {value}`. None when that feature is not in the document.
    fn set_field(&self, feature: Option<&Id>, field: &FieldRef, value: &Value) -> Option<String> {
        let label = &field.label;
        let other = feature.filter(|feature| *feature != self.feature.id());
        if let Some(other) = other {
            let record = self.feature.run.document.feature(other)?;
            return Some(format!(
                "Set {label} of '{}' to {}",
                self.feature.run.label(other),
                self.value_words(record, field, value)
            ));
        }
        Some(match value {
            Value::Null => format!("Clear {label}"),
            Value::Bool(true) => format!("Turn on {label}"),
            Value::Bool(false) => format!("Turn off {label}"),
            Value::String(text) => match self.option_label(self.feature.record, field, text) {
                Some(option) => format!("Set {label} to {option}"),
                None => match self
                    .feature
                    .record_value(&field.pointer())
                    .and_then(Value::as_str)
                    .filter(|current| !is_plain_value(current))
                {
                    Some(current) => format!("Replace {current} with {text}"),
                    None => format!("Use {text}"),
                },
            },
            other => format!("Use {other}"),
        })
    }

    /// A value a fix writes in words: an option's label, `on` or `off`, or the expression.
    fn value_words(&self, record: &FeatureRecord, field: &FieldRef, value: &Value) -> String {
        match value {
            Value::Bool(true) => "on".to_string(),
            Value::Bool(false) => "off".to_string(),
            Value::String(text) => self
                .option_label(record, field, text)
                .unwrap_or_else(|| text.clone()),
            other => other.to_string(),
        }
    }

    /// The label of the option `value` of an option field of `record`.
    fn option_label(
        &self,
        record: &FeatureRecord,
        field: &FieldRef,
        value: &str,
    ) -> Option<String> {
        let manifest = self.feature.run.registry.get(&record.kind)?.manifest();
        FieldRef::hint(&field.path, manifest)?
            .options
            .iter()
            .find(|option| option.value == value)
            .map(|option| option.label.clone())
    }

    /// The manifest label of the field at `pointer` of the feature; its path when the type has
    /// no manifest.
    fn field_label(&self, pointer: &str) -> String {
        let path = field_path(pointer);
        match self.feature.run.registry.get(&self.feature.record.kind) {
            Some(feature) => FieldRef::new(&path, feature.manifest()).label,
            None => path,
        }
    }

    /// The stored reference and the candidates of the message's element repair at `pointer`.
    fn element_repair(&self, pointer: &str) -> Option<(&Reference, &[RepairCandidate])> {
        self.repairs.iter().find_map(|repair| match repair {
            Repair::Element {
                path,
                reference,
                candidates,
                ..
            } if path == pointer => Some((reference, candidates.as_slice())),
            _ => None,
        })
    }

    /// The words of the references a fix stores at `pointer`: their candidates' descriptions in
    /// the repair there, joined by `and`.
    fn references(&self, pointer: &str, references: &[Reference]) -> Option<String> {
        let (_, candidates) = self.element_repair(pointer)?;
        let words = references
            .iter()
            .map(|reference| {
                candidates
                    .iter()
                    .find(|candidate| candidate.reference.query == reference.query)
                    .map(|candidate| candidate.description.clone())
            })
            .collect::<Option<Vec<String>>>()?;
        Some(joined(&words))
    }

    /// The words of the regions a fix stores at `pointer`: the description of the candidate set
    /// of the region repair there that has them, else the regions' own words.
    fn regions(&self, pointer: &str, regions: &[RegionRef]) -> Option<String> {
        self.repairs.iter().find_map(|repair| match repair {
            Repair::Region {
                path,
                sketch,
                candidates,
                ..
            } if path == pointer => Some(
                candidates
                    .iter()
                    .find(|candidate| candidate.regions == regions)
                    .map(|candidate| candidate.description.clone())
                    .unwrap_or_else(|| {
                        region_words(
                            &self.feature.run.label(sketch),
                            regions,
                            self.feature.run.document.units,
                        )
                    }),
            ),
            _ => None,
        })
    }

    /// A body's name; its id when the document does not have it.
    fn body_name(&self, body: &Id) -> String {
        self.feature
            .run
            .names()
            .body_name(body)
            .unwrap_or_else(|| body.to_string())
    }

    /// What `Pick {noun} again` picks at `pointer` (section 3.22): by the kind of the reference
    /// stored there (the message's element repair, else the record's own reference), else by the
    /// selection kind of the field's manifest hint, else the field's label.
    fn noun(&self, pointer: &str) -> String {
        let stored = self
            .element_repair(pointer)
            .map(|(reference, _)| reference.query.kind())
            .or_else(|| {
                reference_at(&self.feature.record.params, pointer)
                    .map(|reference| reference.query.kind())
            });
        if let Some(kind) = stored {
            return element_noun(kind).to_string();
        }
        let path = field_path(pointer);
        self.feature
            .run
            .registry
            .get(&self.feature.record.kind)
            .and_then(|feature| FieldRef::hint(&path, feature.manifest()))
            .and_then(|hint| hint.selection)
            .map_or_else(
                || self.field_label(pointer),
                |selection| selection_noun(selection).to_string(),
            )
    }

    /// The label of a `Show` fix: `Show '{feature}'`, or with an entity `Show the {line, circle,
    /// arc or point} in '{feature}'`.
    fn show(&self, feature: &Id, entity: Option<&Id>) -> String {
        let label = self.feature.run.label(feature);
        let kind = entity.and_then(|entity| {
            let record = self.feature.run.document.feature(feature)?;
            record
                .params
                .get("entities")?
                .as_array()?
                .iter()
                .find(|item| item.get("id").and_then(Value::as_str) == Some(entity.as_str()))?
                .get("type")?
                .as_str()
                .map(str::to_string)
        });
        match kind {
            Some(kind) => format!("Show the {kind} in '{label}'"),
            None => format!("Show '{label}'"),
        }
    }
}

/// True when an expression is a plain value: a number, with or without a unit and a sign.
fn is_plain_value(expression: &str) -> bool {
    let Ok(parsed) = Expr::new(expression).parse() else {
        return false;
    };
    match &parsed.root.kind {
        ExprNodeKind::Number { .. } => true,
        ExprNodeKind::Negate(operand) => matches!(operand.kind, ExprNodeKind::Number { .. }),
        _ => false,
    }
}

/// `a face`, `an edge` or `a vertex`.
fn element_noun(kind: ElementKind) -> &'static str {
    match kind {
        ElementKind::Face => "a face",
        ElementKind::Edge => "an edge",
        ElementKind::Vertex => "a vertex",
    }
}

/// What a field of this selection kind picks, with its article.
fn selection_noun(selection: SelectionKind) -> &'static str {
    match selection {
        SelectionKind::Body => "a body",
        SelectionKind::Sketch => "a sketch",
        SelectionKind::SketchRegion => "a region",
        SelectionKind::SketchPoint => "a point",
        SelectionKind::Face => "a face",
        SelectionKind::Edge => "an edge",
        SelectionKind::Plane => "a plane",
        SelectionKind::Axis => "an axis",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_with_or_without_a_unit_are_plain_values() {
        assert!(is_plain_value("3"));
        assert!(is_plain_value("2.3 mm"));
        assert!(is_plain_value("-5 mm"));
        assert!(!is_plain_value("n / 2"));
        assert!(!is_plain_value("r * 2"));
        assert!(!is_plain_value("height"));
        assert!(!is_plain_value("2 *"));
    }

    #[test]
    fn nouns_have_their_article() {
        assert_eq!(element_noun(ElementKind::Edge), "an edge");
        assert_eq!(selection_noun(SelectionKind::SketchRegion), "a region");
        assert_eq!(selection_noun(SelectionKind::Axis), "an axis");
    }
}
