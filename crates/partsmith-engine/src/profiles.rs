//! Printer profiles of a session and the display tessellation tolerances they give (engine.md
//! sections 3.7 and 3.12).

use std::collections::BTreeMap;

use partsmith_mesh::TessellationTolerance;
use partsmith_model::{Document, Edit, canonical};
use partsmith_print::{
    PrinterProfile, ProfileError, ProfileInput, builtin_profile, builtin_profiles, generic_fdm,
};
use partsmith_protocol::{Diagnostic, Severity, TessellationQuality};
use serde_json::{Map, Value};

use crate::error::EngineError;

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

/// Why a stored profile, or the profile of a `set_printer` edit, is refused (sections 3.2 and 3.3).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum StoredProfileError {
    /// `PrinterProfile::from_written` rejects it.
    Invalid(ProfileError),
    /// Its id is the id of a built-in profile.
    BuiltIn(String),
}

impl StoredProfileError {
    /// The error of the profile of the `set_printer` edit at `index`.
    pub(crate) fn in_edit(self, index: u32) -> EngineError {
        match self {
            StoredProfileError::Invalid(error) => EngineError::InvalidProfile {
                message: error.to_string(),
                index: Some(index),
            },
            StoredProfileError::BuiltIn(id) => EngineError::ProfileIdReserved {
                id,
                index: Some(index),
            },
        }
    }

    /// The `parse_error` of a document text whose stored profile this refuses, at `/printer_profile`.
    pub(crate) fn in_document(self) -> EngineError {
        let message = match &self {
            StoredProfileError::Invalid(ProfileError::Json { .. }) => {
                "The printer profile stored in this document has a value of the wrong form. Fix that value in the file to open it.".to_string()
            }
            StoredProfileError::Invalid(error) => error.to_string(),
            StoredProfileError::BuiltIn(id) => format!(
                "This document stores a copy of the built-in printer \"{id}\". Remove \"printer_profile\" from it: a built-in printer is named by its id alone."
            ),
        };
        let reason = match self {
            StoredProfileError::Invalid(error) => error.to_string(),
            StoredProfileError::BuiltIn(_) => message.clone(),
        };
        EngineError::Parse {
            message,
            detail: format!("printer_profile: {reason}"),
            line: None,
            column: None,
            path: Some("/printer_profile".to_string()),
        }
    }
}

/// Reads a stored profile, or the profile of a `set_printer` edit, with print.md's `from_written`, and refuses a
/// built-in id (sections 3.2 and 3.3).
pub(crate) fn read_stored(
    stored: &Map<String, Value>,
) -> Result<PrinterProfile, StoredProfileError> {
    let profile = PrinterProfile::from_written(stored).map_err(StoredProfileError::Invalid)?;
    if builtin_profile(profile.id.as_str()).is_some() {
        return Err(StoredProfileError::BuiltIn(profile.id.to_string()));
    }
    Ok(profile)
}

/// The written form of a valid profile as a JSON map, members in declaration order.
pub(crate) fn written_form(profile: &PrinterProfile) -> Map<String, Value> {
    match canonical::to_value(profile) {
        Ok(Value::Object(members)) => members,
        _ => unreachable!("a valid profile holds only finite numbers and serializes to an object"),
    }
}

/// Checks the profile a loaded document stores and rewrites its members into the written form's order (section
/// 3.2); a document without one is unchanged.
pub(crate) fn check_stored(document: &mut Document) -> Result<(), EngineError> {
    if let Some(stored) = &mut document.printer_profile {
        let profile = read_stored(stored).map_err(StoredProfileError::in_document)?;
        *stored = written_form(&profile);
    }
    Ok(())
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
            Diagnostic::new(
                Severity::Warning,
                "unknown_printer_profile",
                format!(
                    "Printer profile \"{id}\" is not among your printers. Printer values and checks use the default FDM profile until you add that printer or pick another one."
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

    /// The profile a document uses (section 3.7): the profile it stores, else the installed profile
    /// its `printer` names, else the default FDM profile (also when `printer` is `None`).
    pub(crate) fn resolve(&self, document: &Document) -> ResolvedProfile {
        // The engine checks a stored profile when it opens a text or applies an edit. Reading it
        // again succeeds.
        if let Some(profile) = document
            .printer_profile
            .as_ref()
            .and_then(|stored| read_stored(stored).ok())
        {
            return ResolvedProfile {
                profile,
                missing: None,
            };
        }
        match document.printer.as_deref() {
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

    /// The profile `id` names for a command on `document` (section 3.12): the profile the document
    /// stores when `id` is its `printer`, else the session or built-in profile with that id.
    pub(crate) fn find_for(&self, document: &Document, id: &str) -> Option<PrinterProfile> {
        match &document.printer_profile {
            Some(stored) if document.printer.as_deref() == Some(id) => read_stored(stored).ok(),
            _ => self.find(id),
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
            return Err(EngineError::ProfileIdReserved {
                id: input.id,
                index: None,
            });
        }
        let profile = PrinterProfile::try_from(input).map_err(invalid_profile)?;
        self.session.insert(profile.id.to_string(), profile.clone());
        Ok(profile)
    }

    /// Removes the session profile with this id; true when there was one.
    pub(crate) fn remove(&mut self, id: &str) -> bool {
        self.session.remove(id).is_some()
    }

    /// Completes a `set_printer` edit (section 3.3): a profile it carries is read and rewritten in
    /// the written form's order; without one, the session profile its `printer` names is stored.
    /// `index` is the edit's position in its batch, for the errors of its profile.
    pub(crate) fn complete(
        &self,
        printer: Option<String>,
        profile: Option<Map<String, Value>>,
        index: u32,
    ) -> Result<Edit, EngineError> {
        let profile = match profile {
            Some(stored) => Some(read_stored(&stored).map_err(|error| error.in_edit(index))?),
            None => printer
                .as_deref()
                .and_then(|id| self.session.get(id))
                .cloned(),
        };
        Ok(Edit::SetPrinter {
            printer,
            profile: profile.as_ref().map(written_form),
        })
    }
}

/// The `invalid_profile` error of a profile the print crate rejects, outside an edit.
pub(crate) fn invalid_profile(error: ProfileError) -> EngineError {
    EngineError::InvalidProfile {
        message: error.to_string(),
        index: None,
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

    /// A document whose `printer` is `printer`.
    fn naming(printer: Option<&str>) -> Document {
        Document {
            printer: printer.map(str::to_string),
            ..Document::default()
        }
    }

    fn input(id: &str) -> ProfileInput {
        serde_json::from_value(serde_json::json!({"id": id, "name": "Mine", "technology": "fdm"}))
            .expect("the profile input is valid")
    }

    #[test]
    fn documents_without_a_printer_use_the_default_profile() {
        let resolved = Profiles::default().resolve(&naming(None));

        assert_eq!(resolved.profile, generic_fdm());
        assert_eq!(resolved.diagnostic(), None);
    }

    #[test]
    fn unknown_printers_fall_back_with_a_warning() {
        let resolved = Profiles::default().resolve(&naming(Some("nope")));

        assert_eq!(resolved.profile, generic_fdm());
        let diagnostic = resolved.diagnostic().expect("a warning");
        assert_eq!(diagnostic.code, "unknown_printer_profile");
        assert!(diagnostic.message.contains("\"nope\""));
    }

    #[test]
    fn built_in_printers_resolve() {
        let resolved = Profiles::default().resolve(&naming(Some("generic-resin")));

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
                id: "generic-fdm".to_string(),
                index: None,
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
