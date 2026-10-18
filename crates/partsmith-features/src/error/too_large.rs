//! The messages and fixes of the "too large" errors of fillet, chamfer and shell, with the face or round faces that
//! limit them and the sentence for a number without a unit (features.md section 4.3).

use partsmith_model::{Expr, ExprNodeKind, Id, Unit, Units};
use partsmith_naming::Reference;
use serde_json::Value;

use super::field::FieldRef;
use super::fix::{Fix, Place};
use super::message::{rounded_up_length, suggested_limit};
use super::render::{Renderer, capitalized};
use crate::builtin::fillet::Fillet;
use crate::feature::Feature;

/// How much thicker than the shell's wall a fillet's radius must be so that the wall is no longer stopped by its
/// rounds (section 4.3: "1.5 times the thickness").
const ROUNDS_OVER_WALL: f64 = 1.5;

/// What a "too large" error measured (section 4.3).
#[derive(Clone, Debug, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "Limit keeps the payload types the spec gives, as FeatureError does"
)]
pub enum Limit {
    /// The narrowest face next to a selected edge, narrower than the requested size.
    Face {
        /// Position of the edge's reference in the feature's edge list.
        position: usize,
        /// The edge, by name and fingerprint (`Reference::new`), for its description.
        edge: Reference,
        /// The face, by name and fingerprint, for its description.
        face: Reference,
        /// Its width across the edge in mm (kernel.md `width_across`).
        width: f64,
        /// True when the feature verified that the other edges work at the requested size without this one.
        removable: bool,
    },
    /// The round faces of the body whose radius is smaller than the shell thickness, made by one feature.
    RoundFaces {
        /// The feature that made them (from their names).
        feature: Id,
        /// Their smallest radius in mm.
        radius: f64,
        /// True when that feature is a fillet that wrote this body before the shell.
        fillet: bool,
    },
}

/// Which feature's value is too large, as its messages name it.
#[derive(Clone, Copy, Debug)]
pub(super) enum Subject<'a> {
    /// A fillet's radius.
    Fillet,
    /// A chamfer's distance (the larger of two distances).
    Chamfer,
    /// A shell's thickness, with the body it hollows.
    Shell {
        /// The body.
        body: &'a Id,
    },
}

/// One "too large" error as its message and fixes read it (section 4.3).
#[derive(Clone, Copy, Debug)]
pub(super) struct TooLarge<'a> {
    /// The feature's value.
    pub(super) subject: Subject<'a>,
    /// The field of the value: `radius`, `distance.expr`, `distance` (two distances) or `thickness`.
    pub(super) field: &'a FieldRef,
    /// The value's expression; None for two chamfer distances, which have no one value to replace.
    pub(super) expr: Option<&'a Expr>,
    /// The requested value in millimeters.
    pub(super) requested: f64,
    /// The largest value that worked, when the search found a useful one.
    pub(super) max: Option<f64>,
    /// What limits it, when it was measured.
    pub(super) limit: Option<&'a Limit>,
}

impl TooLarge<'_> {
    /// The message (section 4.3): what happened, then the bare-number sentence, else the limit or round-face
    /// sentence, else that Partsmith could not tell why.
    pub(super) fn text(&self, renderer: &Renderer<'_>) -> String {
        let value = format!("{} {}", self.noun(), renderer.length(self.requested));
        let reason = self
            .bare_number_sentence(renderer)
            .or_else(|| self.limit_sentence(renderer));
        // The reason as a second sentence, when there is one.
        let second_sentence =
            |reason: Option<String>| reason.map_or_else(String::new, |reason| format!(" {reason}"));
        match (self.max, self.subject, reason) {
            (Some(max), Subject::Shell { body }, reason) => format!(
                "{value} is too thick for {}: the largest that works is about {}.{}",
                renderer.body_phrase(body),
                renderer.limit(max),
                second_sentence(reason)
            ),
            (Some(max), _, reason) => format!(
                "{value} is too large for the selected edges: the largest that works is about {}.{}",
                renderer.limit(max),
                second_sentence(reason)
            ),
            (None, Subject::Shell { body }, reason) => format!(
                "{value} does not work on {}, even when thinner. {}",
                renderer.body_phrase(body),
                reason.unwrap_or_else(|| "Partsmith could not tell why.".to_string())
            ),
            (None, _, Some(reason)) => {
                format!("{value} does not work on the selected edges, even when smaller. {reason}")
            }
            (None, _, None) => format!(
                "{value} does not work on the selected edges, even when smaller, and Partsmith could not tell why."
            ),
        }
    }

    /// A replacement expression for the field (section 4.4): `{expr} mm` when the bare-number rule applies, else
    /// the suggested limit; only with a `max` and a field that holds one value.
    pub(super) fn suggestion(&self, units: Units) -> Option<String> {
        let max = self.max?;
        let expr = self.expr?;
        Some(bare_number_in_millimeters(expr, units).unwrap_or_else(|| suggested_limit(max, units)))
    }

    /// The fixes in the order of section 4.3: the bare-number fix, then the round-face fixes of a shell, then the
    /// limit and the removal of the limiting edge, else `Suppress`.
    pub(super) fn fixes(&self, renderer: &Renderer<'_>, feature: &Id) -> Vec<Fix> {
        let mut fixes = Vec::new();
        if let Some(bare) = self
            .expr
            .and_then(|expr| bare_number_in_millimeters(expr, renderer.units()))
        {
            fixes.push(Fix::set_field(self.field.clone(), bare));
        }
        if let (
            Subject::Shell { .. },
            Some(Limit::RoundFaces {
                feature: maker,
                fillet: true,
                ..
            }),
        ) = (self.subject, self.limit)
        {
            fixes.extend(self.larger_rounds(renderer, maker));
            fixes.push(Fix::Move {
                feature: feature.clone(),
                place: Place::Above(maker.clone()),
            });
        }
        if let (Some(max), Some(_)) = (self.max, self.expr) {
            fixes.push(Fix::set_field(self.field.clone(), renderer.limit(max)));
        }
        if let Some(Limit::Face {
            position,
            removable: true,
            ..
        }) = self.limit
        {
            // The limiting edge's entry of the edge list of a fillet or chamfer.
            fixes.push(Fix::RemoveEntry {
                pointer: format!("/edges/{position}"),
            });
        }
        if self.max.is_none() {
            fixes.push(Fix::Suppress {
                feature: feature.clone(),
            });
        }
        fixes
    }

    /// `Fillet radius`, `Chamfer distance` or `Shell thickness`.
    fn noun(&self) -> &'static str {
        match self.subject {
            Subject::Fillet => "Fillet radius",
            Subject::Chamfer => "Chamfer distance",
            Subject::Shell { .. } => "Shell thickness",
        }
    }

    /// `'{expr}' has no unit, so it means {expr} {unit} in this document.` in a document whose units are not
    /// millimeters, for a value written as a number without a unit (corpus R48).
    fn bare_number_sentence(&self, renderer: &Renderer<'_>) -> Option<String> {
        let units = renderer.units();
        let expr = self
            .expr
            .filter(|expr| units != Units::Mm && expr.is_bare_number())?;
        Some(format!(
            "'{expr}' has no unit, so it means {expr} {} in this document.",
            units.symbol()
        ))
    }

    /// The sentence that states what limits the value: the narrowest face next to an edge, or a shell's round faces.
    fn limit_sentence(&self, renderer: &Renderer<'_>) -> Option<String> {
        match self.limit? {
            Limit::Face {
                edge, face, width, ..
            } => {
                let words = renderer.describe_stored(&[edge, face]);
                Some(format!(
                    "{} lies next to {}, which is only {} across.",
                    capitalized(&words[0]),
                    words[1],
                    renderer.length(*width)
                ))
            }
            Limit::RoundFaces {
                feature, radius, ..
            } => Some(format!(
                "Its rounded faces made by {} have radius {}, smaller than the wall.",
                renderer.quoted_feature(feature),
                renderer.length(*radius)
            )),
        }
    }

    /// `SetField` of the Radius of the fillet `maker` to 1.5 times the wall, rounded up to two significant digits,
    /// when that Radius holds a plain value, so no parameter link is cut (section 4.3).
    fn larger_rounds(&self, renderer: &Renderer<'_>, maker: &Id) -> Option<Fix> {
        let radius = renderer.record(maker)?.params.get("radius")?.as_str()?;
        let plain = Expr::new(radius)
            .parse()
            .is_ok_and(|parsed| matches!(parsed.root.kind, ExprNodeKind::Number { .. }));
        plain.then(|| Fix::SetField {
            feature: Some(maker.clone()),
            field: FieldRef::new("radius", &Fillet::manifest()),
            value: Value::String(rounded_up_length(
                ROUNDS_OVER_WALL * self.requested,
                renderer.units(),
            )),
        })
    }
}

/// `{expr} mm`, the size the user meant, for an expression that is a number without a unit in a document whose units
/// are not millimeters (expressions.md section 3.17).
fn bare_number_in_millimeters(expr: &Expr, units: Units) -> Option<String> {
    if units == Units::Mm {
        return None;
    }
    expr.with_unit(Unit::Millimeter)
        .map(|expr| expr.as_str().to_string())
}
