//! Format versions and the migration chain (file-format.md section 3.9).

use serde_json::{Number, Value};

use super::{FORMAT_VERSION, LoadError, OLDEST_FORMAT_VERSION};
use crate::canonical::write_one_line;

/// 2^64, the first whole number a `u64` cannot hold.
const UNSIGNED_LIMIT: f64 = 18_446_744_073_709_551_616.0;

/// One step of the migration chain: upgrades a document from version `from` to `from + 1`.
#[derive(Clone, Copy, Debug)]
pub struct MigrationStep {
    /// The version this step upgrades from.
    pub from: u32,
    /// Rewrites the raw JSON; returns a user-facing message on failure.
    pub migrate: fn(Value) -> Result<Value, String>,
}

/// The built-in chain: one step from version 1 that returns the document unchanged (section 3.9).
pub const MIGRATIONS: &[MigrationStep] = &[MigrationStep {
    from: 1,
    migrate: migrate_from_1,
}];

/// Brings raw document JSON to FORMAT_VERSION with the built-in chain; returns the original version if it changed.
pub fn migrate(document: Value) -> Result<(Value, Option<u32>), LoadError> {
    migrate_with(document, FORMAT_VERSION, OLDEST_FORMAT_VERSION, MIGRATIONS)
}

/// Same as `migrate` with an explicit current version, oldest version and chain (for tests and tools).
pub fn migrate_with(
    document: Value,
    current: u32,
    oldest: u32,
    steps: &[MigrationStep],
) -> Result<(Value, Option<u32>), LoadError> {
    let found = format_version(&document)?;
    if found > u64::from(current) {
        return Err(LoadError::NewerFormatVersion {
            found,
            supported: current,
        });
    }
    if found < u64::from(oldest) {
        return Err(LoadError::UnsupportedFormatVersion { found, oldest });
    }
    // `found` lies between `oldest` and `current`, so it fits a `u32`.
    let original = u32::try_from(found).expect("the version is at most the current version");
    let mut document = document;
    for version in original..current {
        let step = steps
            .iter()
            .find(|step| step.from == version)
            .ok_or(LoadError::MissingMigration { from: version })?;
        let next = version + 1;
        document = (step.migrate)(document).map_err(|message| LoadError::Migration {
            from: version,
            to: next,
            message,
        })?;
        let Some(root) = document.as_object_mut() else {
            return Err(LoadError::Migration {
                from: version,
                to: next,
                message: "The upgraded document is not a JSON object.".to_string(),
            });
        };
        root.insert("format_version".to_string(), Value::from(next));
    }
    let migrated_from = (original != current).then_some(original);
    Ok((document, migrated_from))
}

/// The step from version 1 to 2 (ADR 0025): every field version 2 adds is optional and keeps the
/// version 1 meaning when absent, so the document is returned as it is.
fn migrate_from_1(document: Value) -> Result<Value, String> {
    Ok(document)
}

/// The root `format_version`: a non-negative whole number, where integral floats such as `1.0`
/// count as whole numbers.
fn format_version(document: &Value) -> Result<u64, LoadError> {
    let value = document
        .get("format_version")
        .ok_or(LoadError::MissingFormatVersion)?;
    value.as_number().and_then(whole_number).ok_or_else(|| {
        let mut found = String::new();
        write_one_line(&mut found, value);
        LoadError::InvalidFormatVersion { found }
    })
}

/// The number as a `u64` when it is a non-negative whole number that fits one.
fn whole_number(number: &Number) -> Option<u64> {
    if let Some(whole) = number.as_u64() {
        return Some(whole);
    }
    let value = number.as_f64()?;
    // Integral values from 0 up to 2^64 convert exactly; negative zero is 0.
    (value.fract() == 0.0 && (0.0..UNSIGNED_LIMIT).contains(&value)).then_some(value as u64)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn keep(document: Value) -> Result<Value, String> {
        Ok(document)
    }

    fn replace_with_list(_: Value) -> Result<Value, String> {
        Ok(json!([]))
    }

    #[test]
    fn a_root_that_is_not_an_object_has_no_version() {
        assert_eq!(
            migrate_with(json!([1]), 1, 1, &[]),
            Err(LoadError::MissingFormatVersion)
        );
    }

    #[test]
    fn integral_floats_are_whole_versions() {
        let document = json!({"format_version": 1.0});

        assert_eq!(
            migrate_with(document.clone(), 1, 1, &[]),
            Ok((document, None))
        );
    }

    #[test]
    fn invalid_versions_are_written_in_canonical_form() {
        for (version, found) in [
            (json!(true), "true"),
            (json!(null), "null"),
            (json!([1]), "[1]"),
            (json!(2.5), "2.5"),
        ] {
            assert_eq!(
                migrate_with(json!({"format_version": version}), 1, 1, &[]),
                Err(LoadError::InvalidFormatVersion {
                    found: found.to_string()
                })
            );
        }
    }

    #[test]
    fn a_step_that_returns_a_non_object_fails() {
        let step = MigrationStep {
            from: 1,
            migrate: replace_with_list,
        };

        assert_eq!(
            migrate_with(json!({"format_version": 1}), 2, 1, &[step]),
            Err(LoadError::Migration {
                from: 1,
                to: 2,
                message: "The upgraded document is not a JSON object.".to_string()
            })
        );
    }

    #[test]
    fn large_whole_versions_are_newer() {
        assert_eq!(
            migrate_with(json!({"format_version": u64::MAX}), 3, 1, &[]),
            Err(LoadError::NewerFormatVersion {
                found: u64::MAX,
                supported: 3
            })
        );
        assert_eq!(
            migrate_with(json!({"format_version": 1e18}), 3, 1, &[]),
            Err(LoadError::NewerFormatVersion {
                found: 1_000_000_000_000_000_000,
                supported: 3
            })
        );
    }

    #[test]
    fn versions_beyond_two_to_the_64_are_invalid() {
        assert_eq!(
            migrate_with(json!({"format_version": 1e20}), 3, 1, &[]),
            Err(LoadError::InvalidFormatVersion {
                found: "1e20".to_string()
            })
        );
    }

    #[test]
    fn negative_zero_is_version_zero() {
        assert_eq!(
            migrate_with(json!({"format_version": -0.0}), 3, 1, &[]),
            Err(LoadError::UnsupportedFormatVersion {
                found: 0,
                oldest: 1
            })
        );
    }

    #[test]
    fn steps_that_keep_the_document_still_advance_the_version() {
        let step = MigrationStep {
            from: 1,
            migrate: keep,
        };

        let (migrated, original) =
            migrate_with(json!({"format_version": 1}), 2, 1, &[step]).expect("the step runs");

        assert_eq!(migrated, json!({"format_version": 2}));
        assert_eq!(original, Some(1));
    }

    #[test]
    fn the_built_in_chain_brings_version_one_to_two() {
        assert_eq!(
            migrate(json!({"format_version": 1, "units": "mm"})),
            Ok((json!({"format_version": 2, "units": "mm"}), Some(1)))
        );
    }

    #[test]
    fn the_built_in_chain_keeps_version_two() {
        assert_eq!(
            migrate(json!({"format_version": 2})),
            Ok((json!({"format_version": 2}), None))
        );
    }
}
