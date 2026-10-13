//! Printer profiles of a session and the display tessellation tolerances they give (engine.md
//! sections 3.7 and 3.12).

use std::collections::BTreeMap;

use partsmith_mesh::TessellationTolerance;
use partsmith_print::{
    PrinterProfile, ProfileInput, builtin_profile, builtin_profiles, generic_fdm,
};
use partsmith_protocol::{Diagnostic, Severity, TessellationQuality};

use crate::error::EngineError;
use crate::status::diagnostic;

/// Smallest display or export chord deviation, in mm.
const MIN_CHORD: f64 = 0.001;
/// Largest display or export chord deviation, in mm.
const MAX_CHORD: f64 = 1.0;

/// The session profiles; built-in profiles come from `partsmith-print`.
#[derive(Debug, Clone, Default)]
pub(crate) struct Profiles {
    /// Session profiles by id.
    session: BTreeMap<String, PrinterProfile>,
}

/// The profile a document uses, and whether its `printer` names an installed one.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedProfile {
    /// The profile.
    pub(crate) profile: PrinterProfile,
    /// The document's `printer` id when it names no installed profile.
    pub(crate) missing: Option<String>,
}

impl ResolvedProfile {
    /// The `unknown_printer_profile` warning when the document's printer is not installed.
    pub(crate) fn diagnostic(&self) -> Option<Diagnostic> {
        self.missing.as_ref().map(|id| {
            diagnostic(
                Severity::Warning,
                "unknown_printer_profile",
                format!(
                    "Printer profile \"{id}\" is not installed, so the default FDM profile is used. Install it or pick another printer."
                ),
            )
        })
    }
}

impl Profiles {
    /// The session profile with this id, else the built-in one.
    pub(crate) fn find(&self, id: &str) -> Option<PrinterProfile> {
        self.session
            .get(id)
            .cloned()
            .or_else(|| builtin_profile(id))
    }

    /// The profile of a document's `printer`: the installed profile with that id, else the
    /// default FDM profile (also when `printer` is `None`).
    pub(crate) fn resolve(&self, printer: Option<&str>) -> ResolvedProfile {
        match printer {
            None => ResolvedProfile {
                profile: generic_fdm(),
                missing: None,
            },
            Some(id) => match self.find(id) {
                Some(profile) => ResolvedProfile {
                    profile,
                    missing: None,
                },
                None => ResolvedProfile {
                    profile: generic_fdm(),
                    missing: Some(id.to_string()),
                },
            },
        }
    }

    /// Built-in profiles then session profiles, each sorted by id.
    pub(crate) fn list(&self) -> Vec<PrinterProfile> {
        let mut builtins = builtin_profiles();
        builtins.sort_by(|first, second| first.id.cmp(&second.id));
        builtins.extend(self.session.values().cloned());
        builtins
    }

    /// Converts, validates and stores a session profile; returns it.
    pub(crate) fn put(&mut self, input: ProfileInput) -> Result<PrinterProfile, EngineError> {
        if builtin_profile(&input.id).is_some() {
            return Err(EngineError::ProfileIdReserved { id: input.id });
        }
        let profile =
            PrinterProfile::try_from(input).map_err(|error| EngineError::InvalidProfile {
                message: error.to_string(),
            })?;
        self.session.insert(profile.id.to_string(), profile.clone());
        Ok(profile)
    }
}

/// The tessellation tolerance of `quality` for `profile` (section 3.7): the profile's base
/// tolerance, with the chord times 4 and the angle times 2 for draft and both halved for fine,
/// then the chord clamped to 0.001 to 1 mm.
pub(crate) fn tolerance(
    profile: &PrinterProfile,
    quality: TessellationQuality,
) -> TessellationTolerance {
    let base = profile.tessellation_tolerance();
    let (chord_factor, angle_factor) = match quality {
        TessellationQuality::Draft => (4.0, 2.0),
        TessellationQuality::Standard => (1.0, 1.0),
        TessellationQuality::Fine => (0.5, 0.5),
    };
    TessellationTolerance {
        chord_deviation: (base.chord_deviation * chord_factor).clamp(MIN_CHORD, MAX_CHORD),
        angular_deviation: base.angular_deviation * angle_factor,
    }
}

/// True when both values of an explicit tolerance are finite and positive and the angle is at
/// most π/2 (section 3.7).
pub(crate) fn is_valid_tolerance(tolerance: &TessellationTolerance) -> bool {
    let positive = |value: f64| value.is_finite() && value > 0.0;
    positive(tolerance.chord_deviation)
        && positive(tolerance.angular_deviation)
        && tolerance.angular_deviation <= std::f64::consts::FRAC_PI_2
}

#[cfg(test)]
mod tests {
    use partsmith_print::{Technology, generic_resin};

    use super::*;

    fn input(id: &str) -> ProfileInput {
        serde_json::from_value(serde_json::json!({"id": id, "name": "Mine", "technology": "fdm"}))
            .expect("the profile input is valid")
    }

    #[test]
    fn documents_without_a_printer_use_the_default_profile() {
        let resolved = Profiles::default().resolve(None);

        assert_eq!(resolved.profile, generic_fdm());
        assert_eq!(resolved.diagnostic(), None);
    }

    #[test]
    fn unknown_printers_fall_back_with_a_warning() {
        let resolved = Profiles::default().resolve(Some("nope"));

        assert_eq!(resolved.profile, generic_fdm());
        let diagnostic = resolved.diagnostic().expect("a warning");
        assert_eq!(diagnostic.code, "unknown_printer_profile");
        assert!(diagnostic.message.contains("\"nope\""));
    }

    #[test]
    fn built_in_printers_resolve() {
        let resolved = Profiles::default().resolve(Some("generic-resin"));

        assert_eq!(resolved.profile, generic_resin());
    }

    #[test]
    fn session_profiles_follow_the_built_in_ones() {
        let mut profiles = Profiles::default();
        profiles.put(input("aaa")).expect("a new id is accepted");

        let ids: Vec<String> = profiles
            .list()
            .iter()
            .map(|profile| profile.id.to_string())
            .collect();

        assert_eq!(ids, ["generic-fdm", "generic-resin", "aaa"]);
        assert_eq!(
            profiles.find("aaa").map(|profile| profile.technology),
            Some(Technology::Fdm)
        );
    }

    #[test]
    fn built_in_ids_are_reserved() {
        let result = Profiles::default().put(input("generic-fdm"));

        assert_eq!(
            result,
            Err(EngineError::ProfileIdReserved {
                id: "generic-fdm".to_string()
            })
        );
    }

    #[test]
    fn invalid_profiles_are_rejected_with_the_print_message() {
        let result = Profiles::default().put(input("bad id"));

        assert!(matches!(result, Err(EngineError::InvalidProfile { .. })));
    }

    #[test]
    fn qualities_scale_the_base_tolerance() {
        let profile = generic_fdm();

        let standard = tolerance(&profile, TessellationQuality::Standard);
        let draft = tolerance(&profile, TessellationQuality::Draft);
        let fine = tolerance(&profile, TessellationQuality::Fine);

        assert!((standard.chord_deviation - 0.05).abs() < 1e-15);
        assert!((draft.chord_deviation - 0.2).abs() < 1e-15);
        assert!((fine.chord_deviation - 0.025).abs() < 1e-15);
        assert_eq!(draft.angular_deviation, 2.0 * standard.angular_deviation);
        assert_eq!(fine.angular_deviation, standard.angular_deviation / 2.0);
    }

    #[test]
    fn chords_are_clamped() {
        let mut coarse = generic_fdm();
        coarse.layer_height = 2.0;
        let mut fine = generic_resin();
        fine.layer_height = 0.001;

        assert_eq!(
            tolerance(&coarse, TessellationQuality::Draft).chord_deviation,
            MAX_CHORD
        );
        assert_eq!(
            tolerance(&fine, TessellationQuality::Fine).chord_deviation,
            MIN_CHORD
        );
    }

    #[test]
    fn explicit_tolerances_must_be_positive_and_at_most_a_right_angle() {
        let valid = TessellationTolerance {
            chord_deviation: 0.1,
            angular_deviation: 0.5,
        };

        assert!(is_valid_tolerance(&valid));
        assert!(!is_valid_tolerance(&TessellationTolerance {
            chord_deviation: 0.0,
            ..valid
        }));
        assert!(!is_valid_tolerance(&TessellationTolerance {
            angular_deviation: 2.0,
            ..valid
        }));
        assert!(!is_valid_tolerance(&TessellationTolerance {
            chord_deviation: f64::NAN,
            ..valid
        }));
    }
}
