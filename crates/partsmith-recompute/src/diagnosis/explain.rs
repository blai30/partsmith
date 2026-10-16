//! Explanations rendered from findings at report time, with current labels and units (recompute.md
//! section 3.10, "Rendering" and `fixes`).

use partsmith_features::{
    Absence, Explanation, FieldRef, Fix, Loss, Names, Place, describe_copy, describe_face,
    made_from_line, sketch_coordinates,
};
use partsmith_model::{FeatureManifest, Id, Units};
use partsmith_naming::{ElementName, MergedName, Reference};
use serde_json::Value;

use super::face_name::{FaceName, Generator, parts_of};
use super::view::RunView;
use super::{LossReason, Removal, point_in};

/// Renders the explanations of the losses of one feature.
pub(crate) struct Explainer<'e> {
    /// The run.
    pub(crate) view: &'e RunView<'e>,
    /// Current names, records and sketch results.
    pub(crate) names: &'e dyn Names,
    /// Document units.
    pub(crate) units: Units,
    /// The position of the feature explained.
    pub(crate) explained: usize,
    /// Its manifest, which labels the fields its fixes name, when its type is registered.
    pub(crate) manifest: Option<&'e FeatureManifest>,
}

/// One loss with what the diagnosis found about it.
pub(crate) struct Explained<'a> {
    /// The loss.
    pub(crate) loss: &'a Loss,
    /// The field of the reference, the list for a list entry.
    pub(crate) field: &'a FieldRef,
    /// Why it was lost; None for an ambiguous loss.
    pub(crate) reason: Option<&'a LossReason>,
    /// For an ambiguous loss, the feature after whose change it became ambiguous.
    pub(crate) after: Option<Id>,
    /// True when the reference is the only entry of its field.
    pub(crate) only_entry: bool,
}

impl Explainer<'_> {
    /// The explanation of one loss.
    pub(crate) fn explanation(&self, explained: &Explained<'_>) -> Explanation {
        let Some(reason) = explained.reason else {
            return Explanation {
                text: String::new(),
                after: explained.after.clone(),
                fixes: Vec::new(),
                upstream: false,
            };
        };
        Explanation {
            text: self.text(explained.loss, reason),
            after: None,
            fixes: self.fixes(explained, reason),
            upstream: matches!(reason, LossReason::UpstreamFailed { .. }),
        }
    }

    /// The reason clause that follows the element's description.
    fn text(&self, loss: &Loss, reason: &LossReason) -> String {
        match reason {
            LossReason::Unknown => {
                "can no longer be found, and Partsmith could not tell why".to_string()
            }
            LossReason::NotInDocument { .. }
                if self.described_as_missing(&loss.reference.query) =>
            {
                "is gone".to_string()
            }
            LossReason::NotInDocument { .. } => {
                "is made by a feature that is not in this document".to_string()
            }
            LossReason::Suppressed { feature } => {
                format!("belongs to {}, which is suppressed", self.quoted(feature))
            }
            LossReason::ComesLater { feature } => format!(
                "is made by {}, which now comes after {}",
                self.quoted(feature),
                self.this()
            ),
            LossReason::RolledBack { feature } => format!(
                "is made by {}, which is after the rollback marker",
                self.quoted(feature)
            ),
            LossReason::UpstreamFailed { feature, root } if feature == root => {
                format!("is missing because {} failed", self.quoted(feature))
            }
            LossReason::UpstreamFailed { feature, root } => format!(
                "is missing because {} cannot be built while {} has an error",
                self.quoted(feature),
                self.quoted(root)
            ),
            LossReason::OtherBody {
                body,
                by,
                by_suppressed,
                present: true,
            } => {
                let joined = match by {
                    Some(by) if *by_suppressed => format!(
                        " while {}, which joins it to this body, is suppressed",
                        self.quoted(by)
                    ),
                    Some(by) => format!(", put there by {}", self.quoted(by)),
                    None => String::new(),
                };
                format!("is now in {}{joined}", self.body(body))
            }
            LossReason::OtherBody {
                body,
                by,
                present: false,
                ..
            } => format!(
                "is no longer in {}, because {} now makes {}",
                self.body(&loss.body),
                by.as_ref()
                    .map_or_else(|| self.quoted_unknown(), |by| self.quoted(by)),
                self.body(body)
            ),
            LossReason::Removed { by, how } => self.removed_text(loss, by, how),
            LossReason::EntityDeleted { sketch, .. } => {
                let kind = if made_from_line(&loss.reference) {
                    "line"
                } else {
                    "curve"
                };
                format!(
                    "is gone because that {kind} was deleted from {}",
                    self.quoted(sketch)
                )
            }
            LossReason::PointGone {
                sketch,
                deleted: true,
                ..
            } => format!(
                "is gone because the point of that hole was deleted from {}",
                self.quoted(sketch)
            ),
            LossReason::PointGone {
                feature,
                sketch,
                point,
                deleted: false,
            } => {
                let at = self
                    .names
                    .sketch_result(sketch)
                    .and_then(|result| point_in(result, point))
                    .map_or_else(
                        || "that point".to_string(),
                        |position| {
                            format!("the hole at {}", sketch_coordinates(position, self.units))
                        },
                    );
                format!(
                    "is gone because {} no longer makes {at}: that point was removed from its Points",
                    self.quoted(feature)
                )
            }
            LossReason::CopyGone { feature, instance } => format!(
                "is gone because {} no longer makes its {}",
                self.quoted(feature),
                describe_copy(self.names, feature, *instance)
            ),
            LossReason::NotInFile { feature } => format!(
                "is not in the new version of the file imported by {}",
                self.quoted(feature)
            ),
            LossReason::Followed { .. } => String::new(),
        }
    }

    /// The reason clause of an element a feature between removed.
    fn removed_text(&self, loss: &Loss, by: &Id, how: &Removal) -> String {
        let by = self.quoted(by);
        match how {
            Removal::CutAway => format!("was cut away by {by}, which comes before {}", self.this()),
            Removal::Covered => format!("is now inside the part, covered by {by}"),
            Removal::Rounded => format!("was rounded by {by}, which comes before {}", self.this()),
            Removal::Beveled => format!("was beveled by {by}, which comes before {}", self.this()),
            Removal::Hollowed => format!("was removed by {by}, which hollows the part"),
            Removal::PieceGone => {
                let piece = loss
                    .lost
                    .pieces
                    .first()
                    .or(loss.lost.parts.first())
                    .unwrap_or(&loss.reference.query);
                let whole = piece.piece_free().unwrap_or_else(|| piece.clone());
                format!(
                    "no longer exists: the part of {} it was on was removed by {by}",
                    self.describe(loss, &whole)
                )
            }
            Removal::NoLongerMade { absence } => {
                let why = match absence {
                    Some(Absence::Through) => ", which goes all the way through the body",
                    Some(Absence::EndOutside) => ", whose end now reaches the outside of the body",
                    Some(Absence::FullTurn) => ", which turns all the way round",
                    None => "",
                };
                format!("is no longer made by {by}{why}")
            }
            Removal::NoLongerMeet { between } => match between {
                Some(between) => match self.rounding_sketch(between) {
                    Some(sketch) => format!(
                        "is already round: {} now rounds that corner",
                        self.quoted(&sketch)
                    ),
                    None => format!(
                        "no longer exists, because its faces no longer meet: {} now lies between them",
                        self.describe(loss, between)
                    ),
                },
                None => "no longer exists, because its faces no longer meet".to_string(),
            },
        }
    }

    /// The fixes of a reason, in the order of features.md section 4.4: `Unsuppress`, `Move`,
    /// `UseBody`, `Show`, and `Suppress` of a feature whose corner is already round.
    fn fixes(&self, explained: &Explained<'_>, reason: &LossReason) -> Vec<Fix> {
        match reason {
            LossReason::Suppressed { feature } => vec![Fix::Unsuppress {
                feature: feature.clone(),
            }],
            LossReason::ComesLater { feature } => self.move_fix(feature).into_iter().collect(),
            LossReason::OtherBody {
                body,
                by,
                by_suppressed,
                present: true,
            } => {
                let mut fixes = Vec::new();
                if *by_suppressed && let Some(by) = by {
                    fixes.push(Fix::Unsuppress {
                        feature: by.clone(),
                    });
                }
                let path = body_field(&explained.field.path);
                let field = match self.manifest {
                    Some(manifest) => FieldRef::new(&path, manifest),
                    None => FieldRef {
                        label: path.clone(),
                        path,
                    },
                };
                fixes.push(Fix::UseBody {
                    field,
                    body: body.clone(),
                });
                fixes
            }
            LossReason::UpstreamFailed { root, .. } => vec![Fix::Show {
                feature: root.clone(),
                entity: None,
            }],
            LossReason::Removed {
                how:
                    Removal::NoLongerMeet {
                        between: Some(between),
                    },
                ..
            } if explained.only_entry && self.rounding_sketch(between).is_some() => {
                vec![Fix::Suppress {
                    feature: self.view.id(self.explained).clone(),
                }]
            }
            _ => Vec::new(),
        }
    }

    /// The move that puts the feature explained after `maker`, the feature that makes its element:
    /// this feature directly below it when no record between them uses this feature, else the
    /// maker directly above this feature when the maker uses no record between them, else none,
    /// so a move never leaves a record above a feature it uses.
    fn move_fix(&self, maker: &Id) -> Option<Fix> {
        let this = self.view.id(self.explained);
        let maker_position = self.view.position(maker)?;
        if maker_position <= self.explained {
            return None;
        }
        let between = self.explained + 1..maker_position;
        if !between.clone().any(|user| self.uses(user, this)) {
            return Some(Fix::Move {
                feature: this.clone(),
                place: Place::Below(maker.clone()),
            });
        }
        let maker_uses_between = between
            .clone()
            .any(|used| self.uses(maker_position, self.view.id(used)));
        (!maker_uses_between).then(|| Fix::Move {
            feature: maker.clone(),
            place: Place::Above(this.clone()),
        })
    }

    /// True when the record at `user` uses `used`: one of its feature links, or a generator of one
    /// of its stored references, names it (engine.md section 3.24).
    fn uses(&self, user: usize, used: &Id) -> bool {
        let links = self
            .view
            .dependencies(user)
            .is_some_and(|dependencies| dependencies.features.iter().any(|link| link.id == *used));
        links
            || self.view.timeline.steps[user]
                .record
                .params
                .values()
                .any(|value| references_name(value, used))
    }

    /// The sketch whose arc or circle made `face`, a lateral face of an extrude or revolve, also
    /// through the face-name generators of a shell's inner and rim faces.
    fn rounding_sketch(&self, face: &ElementName) -> Option<Id> {
        let mut face = face.clone();
        loop {
            let parts = FaceName::of(&face)?;
            match parts.generator? {
                Generator::Face(generator) if matches!(parts.role, "inner" | "rim") => {
                    face = ElementName::parse(generator).ok()?;
                }
                Generator::Sketch(entity) if parts.role == "lateral" => {
                    let sweep = self.view.position(&Id::new(parts.feature).ok()?)?;
                    if !matches!(self.view.kind(sweep), "extrude" | "revolve") {
                        return None;
                    }
                    let sketch = self.view.first_sketch(sweep)?;
                    let result = self.names.sketch_result(sketch)?;
                    // An entity is a circle or an arc exactly when it has a center point.
                    return point_in(result, &format!("{entity}.center")).map(|_| sketch.clone());
                }
                _ => return None,
            }
        }
    }

    /// True when the element the query names reads as an element of a feature that is not in the
    /// document (features.md section 3.12, "Missing features").
    fn described_as_missing(&self, query: &ElementName) -> bool {
        parts_of(query).iter().any(|part| {
            FaceName::of(part).is_some_and(|face| {
                Id::new(face.feature)
                    .map_or(true, |feature| self.names.feature_label(&feature).is_none())
            })
        })
    }

    /// The words for the face `face` in the loss's body just before the feature explained.
    fn describe(&self, loss: &Loss, face: &ElementName) -> String {
        let state = self
            .view
            .state_before(&loss.body, self.explained)
            .map(|(state, _)| state);
        describe_face(face.as_str(), state, self.names, self.units)
    }

    /// `{this}`: `this` and the feature's type label in lowercase.
    fn this(&self) -> String {
        let feature = self.view.id(self.explained);
        match self.names.type_label(feature) {
            Some(label) => format!("this {}", label.to_lowercase()),
            None => "this feature".to_string(),
        }
    }

    /// A feature labeled in single quotes, or `a feature that is not in this document`.
    fn quoted(&self, feature: &Id) -> String {
        self.names
            .feature_label(feature)
            .map_or_else(|| self.quoted_unknown(), |label| format!("'{label}'"))
    }

    /// How a feature that is not in the document is named.
    fn quoted_unknown(&self) -> String {
        "a feature that is not in this document".to_string()
    }

    /// `body '{name}'`, or `a body that is not in this document`.
    fn body(&self, body: &Id) -> String {
        self.names.body_name(body).map_or_else(
            || "a body that is not in this document".to_string(),
            |name| format!("body '{name}'"),
        )
    }
}

/// The body field next to a reference at `path`: the sibling `body` of a face or edge field
/// (`base.body` for `base.face`), else the feature's own `body`.
fn body_field(path: &str) -> String {
    match path.rsplit_once('.') {
        Some((parent, "face" | "edge")) => format!("{parent}.body"),
        _ => "body".to_string(),
    }
}

/// True when a stored reference in `params` (an object with a `q` key that reads as a reference,
/// at any depth) has a generator of `feature`, in its query or one of its merged names.
fn references_name(params: &Value, feature: &Id) -> bool {
    match params {
        Value::Object(object) => {
            let named = object.contains_key("q")
                && serde_json::from_value::<Reference>(params.clone())
                    .is_ok_and(|reference| reference_names(&reference, feature));
            named || object.values().any(|value| references_name(value, feature))
        }
        Value::Array(items) => items.iter().any(|item| references_name(item, feature)),
        _ => false,
    }
}

/// True when a generator of the query of `reference`, or of one of its merged names, is `feature`.
fn reference_names(reference: &Reference, feature: &Id) -> bool {
    let generates = |name: &ElementName| {
        name.generators()
            .iter()
            .any(|used| used.feature == *feature)
    };
    generates(&reference.query)
        || reference
            .merged
            .0
            .values()
            .flatten()
            .any(|merged| match merged {
                MergedName::Face(face) => generates(face),
                MergedName::Copies {
                    source,
                    feature: copier,
                } => copier == feature || generates(source),
            })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn id(text: &str) -> Id {
        Id::new(text).expect("a valid id")
    }

    #[test]
    fn the_body_field_of_a_face_or_edge_is_its_sibling() {
        assert_eq!(body_field("base.face"), "base.body");
        assert_eq!(body_field("axis.edge"), "axis.body");
        assert_eq!(body_field("edges"), "body");
        assert_eq!(body_field("edges[1]"), "body");
    }

    #[test]
    fn stored_references_name_the_features_of_their_generators() {
        let params = json!({
            "base": {"kind": "face", "body": "b-1", "face": {"q": "f-ext2/end-cap", "fp": "plane|n=0,0,1|c=0,0,0|a=1"}},
            "edges": [{
                "q": "edge(f-box1/top|f-lp1/x)", "fp": "line|n=1,0,0|c=0,0,0|l=1",
                "merged": {"f-box1/top": ["f-box9/top@f-mir1:*"]}
            }]
        });

        assert!(references_name(&params, &id("f-ext2")));
        assert!(references_name(&params, &id("f-lp1")));
        assert!(references_name(&params, &id("f-mir1")));
        assert!(references_name(&params, &id("f-box9")));
        assert!(!references_name(&params, &id("f-sk1")));
    }
}
