//! The messages and fixes of the body rules: why a feature changed nothing, separate pieces and holes beside the
//! body, with the sketch that lost its face or turned over as their root (features.md sections 3.5 and 3.15).

use partsmith_model::{Id, LINEAR_TOLERANCE};
use partsmith_naming::Reference;
use serde_json::Value;

use super::fix::Fix;
use super::message::NoEffectHow;
use super::render::{Renderer, joined};
use super::sketch_coordinates;
use crate::builtin::circular_pattern::CircularPattern;
use crate::builtin::linear_pattern::LinearPattern;
use crate::builtin::mirror::Mirror;
use crate::feature::Feature;

/// The sketch that recompute's diagnosis names as the root of a body rule's warning, and how it went wrong
/// (section 3.15).
#[derive(Clone, Copy, Debug)]
enum SketchRoot<'a> {
    /// The profile or point sketch lost the face it was on.
    LostFace(&'a Id),
    /// Its face turned over, so its drawing lies mirrored on the face.
    TurnedOver(&'a Id),
}

impl SketchRoot<'_> {
    /// The root the diagnosis gives, if any.
    fn of<'a>(renderer: &Renderer<'a>) -> Option<SketchRoot<'a>> {
        let diagnosis = renderer.diagnosis();
        diagnosis
            .lost_face
            .as_ref()
            .map(SketchRoot::LostFace)
            .or_else(|| diagnosis.turned_over.as_ref().map(SketchRoot::TurnedOver))
    }

    /// The sketch.
    fn sketch(self) -> Id {
        match self {
            SketchRoot::LostFace(sketch) | SketchRoot::TurnedOver(sketch) => sketch.clone(),
        }
    }

    /// `because '{sketch}' lost the face it was on. Fix '{sketch}' first.`, or the mirrored drawing's form.
    fn because(self, renderer: &Renderer<'_>) -> String {
        let sketch = renderer.quoted_feature(&self.sketch());
        match self {
            SketchRoot::LostFace(_) => {
                format!("because {sketch} lost the face it was on. Fix {sketch} first.")
            }
            SketchRoot::TurnedOver(_) => format!(
                "because the drawing of {sketch} lies mirrored on its face. Fix {sketch} first."
            ),
        }
    }
}

/// The message of `NoEffect` (section 3.15): why nothing changed, or, when the feature's sketch lost its face or
/// turned over in the same run, that sketch as the root.
pub(super) fn no_effect_text(renderer: &Renderer<'_>, body: &Id, how: &NoEffectHow) -> String {
    let body = renderer.body_phrase(body);
    let this = renderer.this_title();
    if let Some(root) = SketchRoot::of(renderer) {
        let changes = match how {
            NoEffectHow::Inside => "adds nothing to",
            _ => "changes nothing in",
        };
        return format!("{this} {changes} {body} {}", root.because(renderer));
    }
    match how {
        NoEffectHow::Inside => format!(
            "{this} adds nothing to {body}: {} entirely inside the body.",
            inside_subject(renderer)
        ),
        NoEffectHow::Away { gap } if *gap < LINEAR_TOLERANCE => format!(
            "This cut does not touch {body}: it starts at the surface of the body and points away from it."
        ),
        NoEffectHow::Away { gap } => format!(
            "This cut does not touch {body}: it starts {} from the body and points away from it.",
            renderer.length(*gap)
        ),
        NoEffectHow::Short { gap } if *gap < LINEAR_TOLERANCE => {
            format!("This cut does not reach {body}: it ends at the surface of the body.")
        }
        NoEffectHow::Short { gap } => format!(
            "This cut does not reach {body}: it ends {} short of it.",
            renderer.length(*gap)
        ),
        NoEffectHow::Beside => format!("This cut does not touch {body}: it lies beside it."),
        NoEffectHow::Covers => {
            format!("This intersection keeps all of {body}: its shape covers the whole body.")
        }
        NoEffectHow::Skipped { field } => format!(
            "{this} changes nothing in {body}, because every edge selected for {} was skipped.",
            field.label
        ),
        NoEffectHow::Missed => format!("{this} cuts nothing from {body}: every hole misses it."),
        NoEffectHow::NoCopies { fields } => {
            let labels: Vec<String> = fields.iter().map(|field| field.label.clone()).collect();
            let verb = if labels.len() > 1 { "are" } else { "is" };
            format!(
                "{this} makes no copies, because {} {verb} 1.",
                joined(&labels)
            )
        }
        NoEffectHow::NotMoved => format!(
            "{this} leaves {body} where it is, because its move is 0 and it turns by whole turns or not at all."
        ),
    }
}

/// The fixes of `NoEffect` (section 3.15): `Show` of the sketch that is the root; else, for a join inside the body,
/// Turn off Reverse when the tools built the other way round add to the body, then `Attach` with an attach face, and
/// Turn on Reverse for a cut whose reversed tools reach the body.
pub(super) fn no_effect_fixes(
    renderer: &Renderer<'_>,
    body: &Id,
    how: &NoEffectHow,
    reverse_hits: bool,
    attach: Option<&(Id, Reference)>,
) -> Vec<Fix> {
    if let Some(root) = SketchRoot::of(renderer) {
        return vec![Fix::show(&root.sketch())];
    }
    let reverse = reverse_hits.then(|| reverse_fix(renderer));
    match how {
        NoEffectHow::Inside => reverse
            .into_iter()
            .chain(attach_fix(body, attach))
            .collect(),
        NoEffectHow::Away { .. } | NoEffectHow::Short { .. } | NoEffectHow::Beside => {
            reverse.into_iter().collect()
        }
        _ => Vec::new(),
    }
}

/// The message of `SeparateSolid` (section 4.2), which starts with `Part of {this}` when part of the tools joined the
/// body. A sketch that lost its face or turned over makes it the root without changing the text (section 3.15).
pub(super) fn separate_solid_text(
    renderer: &Renderer<'_>,
    body: &Id,
    pieces: usize,
    part_joined: bool,
) -> String {
    let subject = if part_joined {
        format!("Part of {}", renderer.this())
    } else {
        renderer.this_title()
    };
    format!(
        "{subject} does not touch {}, so the body now has {pieces} separate pieces.",
        renderer.body_phrase(body)
    )
}

/// The fixes of `SeparateSolid` (section 3.15): `NewBody`, then `Attach` when the attach rule found a face; none when
/// part of the tools joined the body, since a new body would take that part along.
pub(super) fn separate_solid_fixes(
    body: &Id,
    part_joined: bool,
    attach: Option<&(Id, Reference)>,
) -> Vec<Fix> {
    if part_joined {
        return Vec::new();
    }
    std::iter::once(Fix::NewBody)
        .chain(attach_fix(body, attach))
        .collect()
}

/// The message of `HoleMissesBody` (section 4.2): the hole by its point's sketch coordinates, with the way out when
/// reversing misses too, or the sketch that is the root.
pub(super) fn hole_misses_text(
    renderer: &Renderer<'_>,
    sketch: &Id,
    position: [f64; 2],
    body: &Id,
    reverse_hits: bool,
) -> String {
    let hole = format!(
        "The hole at {} in {} does not touch {}",
        sketch_coordinates(position, renderer.units()),
        renderer.quoted_feature(sketch),
        renderer.body_phrase(body)
    );
    match SketchRoot::of(renderer) {
        Some(root) => format!("{hole} {}", root.because(renderer)),
        None if reverse_hits => format!("{hole}."),
        None => format!("{hole}. Move the point onto the body."),
    }
}

/// The fixes of `HoleMissesBody` (section 4.2): `Show` of the sketch that is the root, else Turn on Reverse when the
/// hole drilled the other way reaches the body.
pub(super) fn hole_misses_fixes(renderer: &Renderer<'_>, reverse_hits: bool) -> Vec<Fix> {
    match SketchRoot::of(renderer) {
        Some(root) => vec![Fix::show(&root.sketch())],
        None if reverse_hits => vec![reverse_fix(renderer)],
        None => Vec::new(),
    }
}

/// `SetField` of `reverse` to the other value than the feature's record holds (false when it has none), so on a
/// feature that is already reversed it turns Reverse off (section 3.15).
pub(super) fn reverse_fix(renderer: &Renderer<'_>) -> Fix {
    let reversed = renderer
        .this_record()
        .and_then(|record| record.params.get("reverse"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Fix::set_field(renderer.own_field("reverse", "Reverse"), !reversed)
}

/// `Attach` of the sketch to the attach face, when the attach rule found one.
fn attach_fix(body: &Id, attach: Option<&(Id, Reference)>) -> Option<Fix> {
    attach.map(|(sketch, face)| Fix::Attach {
        sketch: sketch.clone(),
        body: body.clone(),
        face: face.clone(),
    })
}

/// What lies inside the body when a join adds nothing (section 3.15): `its mirror image lies` for a mirror,
/// `its copies lie` for a pattern, else `it lies`.
fn inside_subject(renderer: &Renderer<'_>) -> &'static str {
    let Some(label) = renderer.type_label() else {
        return "it lies";
    };
    if label == Mirror::manifest().label {
        "its mirror image lies"
    } else if [
        LinearPattern::manifest().label,
        CircularPattern::manifest().label,
    ]
    .contains(&label)
    {
        "its copies lie"
    } else {
        "it lies"
    }
}
