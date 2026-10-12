//! Feature warnings, their messages and repair data (features.md sections 2.8 and 4.2).

use std::fmt;

use partsmith_model::{Id, Units};
use partsmith_naming::{ElementKind, ElementName, Reference};
use partsmith_sketch::{RegionRef, SolveIssue};

use super::field::FieldRef;
use super::message::{
    FallbackReason, IdNames, IgnoredReason, Names, Renderer, Repair, plural,
};

/// Something a feature built around: a lost reference, a remapped region, an unchanged body.
/// Stores ids, never display names, so it renders correctly after a rename. `Display` renders the
/// message with ids in place of names and values in millimeters.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum FeatureWarning {
    /// A reference of a list matches nothing and was skipped.
    ReferenceMissing {
        /// The list entry, for example `edges[1]`.
        field: FieldRef,
        /// The entry's position in the list.
        position: usize,
        /// The body it is resolved in.
        body: Id,
        /// The stored reference.
        reference: Reference,
        /// The kind of element the list selects.
        kind: ElementKind,
    },
    /// A reference of a list matches several elements and was skipped.
    ReferenceAmbiguous {
        /// The list entry, for example `edges[1]`.
        field: FieldRef,
        /// The entry's position in the list.
        position: usize,
        /// The body it is resolved in.
        body: Id,
        /// The stored reference.
        reference: Reference,
        /// The kind of element the list selects.
        kind: ElementKind,
        /// The elements it matches, best first.
        candidates: Vec<ElementName>,
    },
    /// A profile region was found by its outer boundary or fingerprint instead of its key.
    RegionRemapped {
        /// The region entry, for example `profile.regions[0]`.
        field: FieldRef,
        /// The entry's position in the region list.
        position: usize,
        /// The sketch.
        sketch: Id,
        /// The stored region ref.
        region: RegionRef,
    },
    /// A profile region matches several regions and was skipped.
    RegionAmbiguous {
        /// The region entry, for example `profile.regions[0]`.
        field: FieldRef,
        /// The entry's position in the region list.
        position: usize,
        /// The sketch.
        sketch: Id,
        /// The stored region ref.
        region: RegionRef,
    },
    /// A profile region matches nothing and was skipped.
    RegionMissing {
        /// The region entry, for example `profile.regions[0]`.
        field: FieldRef,
        /// The entry's position in the region list.
        position: usize,
        /// The sketch.
        sketch: Id,
        /// The stored region ref.
        region: RegionRef,
    },
    /// A sketch on a face kept its fallback placement.
    PlaneFallback {
        /// The face field, for example `plane.face`.
        field: FieldRef,
        /// The body of the face.
        body: Id,
        /// The stored face reference.
        reference: Reference,
        /// Why the face was not used.
        reason: FallbackReason,
    },
    /// The feature left the body's volume unchanged.
    NoEffect {
        /// The body.
        body: Id,
    },
    /// A hole does not touch its body.
    HoleMissesBody {
        /// The sketch point of the hole.
        point: String,
        /// The body.
        body: Id,
    },
    /// Part of the input was ignored.
    Ignored {
        /// The field that was partly ignored.
        field: FieldRef,
        /// Why.
        reason: IgnoredReason,
    },
    /// The params have keys this version does not know. Added by recompute.
    UnknownParams {
        /// The unknown keys in their original order.
        keys: Vec<String>,
    },
    /// Sketch constraints repeat what others determine.
    RedundantConstraints {
        /// The `SolveIssue::Redundant` issues.
        issues: Vec<SolveIssue>,
    },
    /// The kernel skipped or repaired content of an imported file.
    ImportWarning {
        /// The kernel's warning text.
        message: String,
    },
}

impl FeatureWarning {
    /// The message with current names and units.
    pub fn render(&self, names: &dyn Names, units: Units) -> String {
        let renderer = Renderer { names, units };
        match self {
            FeatureWarning::ReferenceMissing { field, kind, .. } => format!(
                "One of the {} selected for {} can no longer be found, so it was skipped. Re-pick it in the repair panel.",
                plural(*kind),
                field.label
            ),
            FeatureWarning::ReferenceAmbiguous {
                field,
                kind,
                candidates,
                ..
            } => format!(
                "One of the {plural} selected for {} now matches {} {plural}, so it was skipped. Pick the one you mean in the repair panel.",
                field.label,
                candidates.len(),
                plural = plural(*kind)
            ),
            FeatureWarning::RegionRemapped { sketch, .. } => format!(
                "A profile region of '{}' changed, so the closest matching region was used. Check the result, or re-pick the profile.",
                renderer.feature(sketch)
            ),
            FeatureWarning::RegionAmbiguous { sketch, .. } => format!(
                "A profile region of '{}' now matches several regions, so it was skipped. Re-pick the profile.",
                renderer.feature(sketch)
            ),
            FeatureWarning::RegionMissing { sketch, .. } => format!(
                "A profile region of '{}' can no longer be found, so it was skipped. Re-pick the profile.",
                renderer.feature(sketch)
            ),
            FeatureWarning::PlaneFallback { reason, .. } => match reason {
                FallbackReason::Missing => "The face this sketch is on can no longer be found, so the sketch stays at its last position. Re-attach it in the repair panel.",
                FallbackReason::Ambiguous => "The face this sketch is on now matches several faces, so the sketch stays at its last position. Pick the face in the repair panel.",
                FallbackReason::NotPlanar => "The face this sketch is on is no longer flat, so the sketch stays at its last position. Attach it to a flat face.",
            }
            .to_string(),
            FeatureWarning::NoEffect { body } => format!(
                "This feature does not change body '{}'. Check its position, size and direction.",
                renderer.body(body)
            ),
            FeatureWarning::HoleMissesBody { point, body } => format!(
                "The hole at point '{point}' does not touch body '{}'. Move the point, or turn on Reverse.",
                renderer.body(body)
            ),
            FeatureWarning::Ignored {
                field,
                reason: IgnoredReason::SmoothEdge,
            } => format!(
                "One of the edges selected for {} is smooth (its faces meet without a corner), so it was skipped.",
                field.label
            ),
            FeatureWarning::Ignored {
                field,
                reason: IgnoredReason::TipAngleThroughAll,
            } => format!(
                "{} is ignored because the hole goes through all.",
                field.label
            ),
            FeatureWarning::UnknownParams { keys } => {
                let keys: Vec<String> = keys.iter().map(|key| format!("'{key}'")).collect();
                format!(
                    "This feature has settings this version of Partsmith does not know ({}). They are ignored and kept in the file.",
                    keys.join(", ")
                )
            }
            FeatureWarning::RedundantConstraints { issues } => issues
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" "),
            FeatureWarning::ImportWarning { message } => message.clone(),
        }
    }

    /// Stable snake_case code.
    pub fn code(&self) -> &'static str {
        match self {
            FeatureWarning::ReferenceMissing { .. } => "reference_missing",
            FeatureWarning::ReferenceAmbiguous { .. } => "reference_ambiguous",
            FeatureWarning::RegionRemapped { .. } => "region_remapped",
            FeatureWarning::RegionAmbiguous { .. } => "region_ambiguous",
            FeatureWarning::RegionMissing { .. } => "region_missing",
            FeatureWarning::PlaneFallback { .. } => "plane_fallback",
            FeatureWarning::NoEffect { .. } => "no_effect",
            FeatureWarning::HoleMissesBody { .. } => "hole_misses_body",
            FeatureWarning::Ignored { .. } => "ignored",
            FeatureWarning::UnknownParams { .. } => "unknown_params",
            FeatureWarning::RedundantConstraints { .. } => "redundant_constraints",
            FeatureWarning::ImportWarning { .. } => "import_warning",
        }
    }

    /// The field the warning is about, if any.
    pub fn field(&self) -> Option<&FieldRef> {
        match self {
            FeatureWarning::ReferenceMissing { field, .. }
            | FeatureWarning::ReferenceAmbiguous { field, .. }
            | FeatureWarning::RegionRemapped { field, .. }
            | FeatureWarning::RegionAmbiguous { field, .. }
            | FeatureWarning::RegionMissing { field, .. }
            | FeatureWarning::PlaneFallback { field, .. }
            | FeatureWarning::Ignored { field, .. } => Some(field),
            FeatureWarning::NoEffect { .. }
            | FeatureWarning::HoleMissesBody { .. }
            | FeatureWarning::UnknownParams { .. }
            | FeatureWarning::RedundantConstraints { .. }
            | FeatureWarning::ImportWarning { .. } => None,
        }
    }

    /// A replacement expression for `field()`, in display units (section 4.4). None for every M1 warning.
    pub fn suggestion(&self, units: Units) -> Option<String> {
        let _ = units;
        None
    }

    /// Repair data, if the warning is about a lost reference or region.
    pub fn repair(&self) -> Option<Repair> {
        match self {
            FeatureWarning::ReferenceMissing {
                field,
                body,
                reference,
                ..
            } => Some(element_repair(field, body, reference, false, &[])),
            FeatureWarning::ReferenceAmbiguous {
                field,
                body,
                reference,
                candidates,
                ..
            } => Some(element_repair(field, body, reference, true, candidates)),
            FeatureWarning::PlaneFallback {
                field,
                body,
                reference,
                reason,
            } => Some(element_repair(
                field,
                body,
                reference,
                *reason == FallbackReason::Ambiguous,
                &[],
            )),
            FeatureWarning::RegionRemapped {
                field,
                sketch,
                region,
                ..
            }
            | FeatureWarning::RegionMissing {
                field,
                sketch,
                region,
                ..
            } => Some(region_repair(field, sketch, region, false)),
            FeatureWarning::RegionAmbiguous {
                field,
                sketch,
                region,
                ..
            } => Some(region_repair(field, sketch, region, true)),
            FeatureWarning::NoEffect { .. }
            | FeatureWarning::HoleMissesBody { .. }
            | FeatureWarning::Ignored { .. }
            | FeatureWarning::UnknownParams { .. }
            | FeatureWarning::RedundantConstraints { .. }
            | FeatureWarning::ImportWarning { .. } => None,
        }
    }
}

impl fmt::Display for FeatureWarning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.render(&IdNames, Units::Mm))
    }
}

/// Repair data for an element reference at `field`.
fn element_repair(
    field: &FieldRef,
    body: &Id,
    reference: &Reference,
    ambiguous: bool,
    candidates: &[ElementName],
) -> Repair {
    Repair::Element {
        pointer: field.pointer(),
        body: body.clone(),
        reference: reference.clone(),
        ambiguous,
        candidates: candidates.to_vec(),
    }
}

/// Repair data for a region ref at `field`.
fn region_repair(field: &FieldRef, sketch: &Id, region: &RegionRef, ambiguous: bool) -> Repair {
    Repair::Region {
        pointer: field.pointer(),
        sketch: sketch.clone(),
        region: region.clone(),
        ambiguous,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn id(text: &str) -> Id {
        Id::new(text).expect("test ids are valid")
    }

    fn field(path: &str, label: &str) -> FieldRef {
        FieldRef {
            path: path.to_string(),
            label: label.to_string(),
        }
    }

    fn reference() -> Reference {
        serde_json::from_value(json!({"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0"}))
            .expect("the reference is valid")
    }

    fn region() -> RegionRef {
        serde_json::from_value(json!({"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}))
            .expect("the region ref is valid")
    }

    #[test]
    fn missing_reference_renders_and_repairs_at_its_entry() {
        let warning = FeatureWarning::ReferenceMissing {
            field: field("edges[1]", "Edges"),
            position: 1,
            body: id("b-1"),
            reference: reference(),
            kind: ElementKind::Edge,
        };

        assert_eq!(
            warning.to_string(),
            "One of the edges selected for Edges can no longer be found, so it was skipped. Re-pick it in the repair panel."
        );
        assert_eq!(warning.code(), "reference_missing");
        assert!(matches!(
            warning.repair(),
            Some(Repair::Element { pointer, ambiguous: false, .. }) if pointer == "/edges/1"
        ));
    }

    #[test]
    fn ambiguous_reference_counts_its_candidates() {
        let candidates = vec![
            ElementName::parse("f-box1/top#1").expect("valid name"),
            ElementName::parse("f-box1/top#2").expect("valid name"),
        ];
        let warning = FeatureWarning::ReferenceAmbiguous {
            field: field("faces[0]", "Faces"),
            position: 0,
            body: id("b-1"),
            reference: reference(),
            kind: ElementKind::Face,
            candidates: candidates.clone(),
        };

        assert_eq!(
            warning.to_string(),
            "One of the faces selected for Faces now matches 2 faces, so it was skipped. Pick the one you mean in the repair panel."
        );
        assert!(matches!(
            warning.repair(),
            Some(Repair::Element { ambiguous: true, candidates: listed, .. }) if listed == candidates
        ));
    }

    #[test]
    fn region_warnings_repair_their_region_entry() {
        let warning = FeatureWarning::RegionAmbiguous {
            field: field("profile.regions[0]", "Profile"),
            position: 0,
            sketch: id("f-sk1"),
            region: region(),
        };

        assert_eq!(
            warning.to_string(),
            "A profile region of 'f-sk1' now matches several regions, so it was skipped. Re-pick the profile."
        );
        assert!(matches!(
            warning.repair(),
            Some(Repair::Region { pointer, ambiguous: true, .. }) if pointer == "/profile/regions/0"
        ));
    }

    #[test]
    fn plane_fallback_messages_per_reason() {
        let warning = |reason| FeatureWarning::PlaneFallback {
            field: field("plane.face", "Plane"),
            body: id("b-1"),
            reference: reference(),
            reason,
        };

        assert_eq!(
            warning(FallbackReason::NotPlanar).to_string(),
            "The face this sketch is on is no longer flat, so the sketch stays at its last position. Attach it to a flat face."
        );
        assert!(matches!(
            warning(FallbackReason::Ambiguous).repair(),
            Some(Repair::Element { ambiguous: true, pointer, .. }) if pointer == "/plane/face"
        ));
    }

    #[test]
    fn unknown_params_quote_each_key() {
        let warning = FeatureWarning::UnknownParams {
            keys: vec!["taper_angle".to_string(), "thin".to_string()],
        };

        assert_eq!(
            warning.to_string(),
            "This feature has settings this version of Partsmith does not know ('taper_angle', 'thin'). They are ignored and kept in the file."
        );
        assert_eq!(warning.field(), None);
        assert_eq!(warning.suggestion(Units::Mm), None);
    }

    #[test]
    fn ignored_tip_angle_names_the_field() {
        let warning = FeatureWarning::Ignored {
            field: field("tip_angle", "Tip angle"),
            reason: IgnoredReason::TipAngleThroughAll,
        };

        assert_eq!(
            warning.to_string(),
            "Tip angle is ignored because the hole goes through all."
        );
    }

    #[test]
    fn redundant_constraints_join_their_messages() {
        let issue = |constraint: &str| SolveIssue::Redundant {
            constraint: id(constraint),
            type_name: "horizontal",
            dimension: false,
        };
        let warning = FeatureWarning::RedundantConstraints {
            issues: vec![issue("c-3"), issue("c-9")],
        };

        assert_eq!(
            warning.to_string(),
            format!("{} {}", issue("c-3"), issue("c-9"))
        );
    }

    #[test]
    fn no_effect_names_the_body() {
        struct Bracket;

        impl Names for Bracket {
            fn body_name(&self, _body: &Id) -> Option<String> {
                Some("Bracket".to_string())
            }

            fn feature_label(&self, _feature: &Id) -> Option<String> {
                None
            }
        }

        let warning = FeatureWarning::NoEffect { body: id("b-1") };

        assert_eq!(
            warning.render(&Bracket, Units::Mm),
            "This feature does not change body 'Bracket'. Check its position, size and direction."
        );
    }
}
