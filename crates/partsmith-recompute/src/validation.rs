//! Validated feature records and their memo (recompute.md section 3.1 steps 1 and 2).

use std::collections::BTreeSet;
use std::sync::Arc;

use partsmith_features::{Dependencies, FeatureError, FieldRef, Link, Registry};
use partsmith_model::FeatureRecord;
use serde_json::{Map, Value};

use crate::key::params_text;
use crate::pass_lru::PassLru;

/// A record that passed validation, with what recompute derives from it.
#[derive(Debug)]
pub(crate) struct Valid {
    /// Canonical params: known keys in canonical order, then the unknown keys.
    pub(crate) params: Map<String, Value>,
    /// Keys the feature type does not know, in their original order.
    pub(crate) unknown_keys: Vec<String>,
    /// Each unknown key that nearly matches one known key, with that field (`Validated::suggestions`).
    pub(crate) suggestions: Vec<(String, FieldRef)>,
    /// What the record declares it reads and defines.
    pub(crate) dependencies: Dependencies,
    /// The known canonical params written compactly, for its cache key.
    pub(crate) params_text: String,
    /// Parameter names its expressions use directly, without repeats.
    pub(crate) parameter_names: Vec<String>,
    /// Dotted names its expressions use directly, without repeats.
    pub(crate) value_names: Vec<String>,
}

impl Valid {
    /// The body links in key order: read, then written.
    pub(crate) fn body_links(&self) -> impl Iterator<Item = &Link> {
        self.dependencies
            .bodies_read
            .iter()
            .chain(&self.dependencies.bodies_written)
    }
}

/// The outcome of validating a record: what it declares, or why it is invalid.
pub(crate) type Checked = Result<Arc<Valid>, FeatureError>;

/// Validates a record and derives its dependencies, key params and expression names.
fn check(registry: &Registry, record: &FeatureRecord) -> Checked {
    let validated = registry.validate_and_canonicalize(record)?;
    let dependencies = registry.dependencies(&validated.record)?;
    let mut parameter_names = BTreeSet::new();
    let mut value_names = BTreeSet::new();
    for declared in &dependencies.expressions {
        // An expression that does not parse names nothing; its evaluation reports the error.
        if let Ok(references) = declared.expr.references() {
            parameter_names.extend(references.parameters);
            value_names.extend(references.values);
        }
    }
    Ok(Arc::new(Valid {
        params_text: params_text(&validated.record.params, &validated.unknown_keys),
        params: validated.record.params,
        unknown_keys: validated.unknown_keys,
        suggestions: validated.suggestions,
        dependencies,
        parameter_names: parameter_names.into_iter().collect(),
        value_names: value_names.into_iter().collect(),
    }))
}

/// Validation outcomes memoized per record content, so unchanged records are not parsed again,
/// even when several documents use the same feature ids. One per `Recompute`, whose registry
/// never changes. Each pass over a document is one pass of the memo.
#[derive(Debug, Default)]
pub(crate) struct Validations {
    /// The outcome for each record content (see [`content_hash`]), with the pass that last used it.
    entries: PassLru<[u8; 32], Checked>,
}

impl Validations {
    /// Starts a pass over a document: outcomes looked up from now on count as used by it.
    pub(crate) fn begin_pass(&mut self) {
        self.entries.begin_pass();
    }

    /// The validation outcome of `record`, computed only when no record with the same id, type
    /// and params was checked before.
    pub(crate) fn check(&mut self, registry: &Registry, record: &FeatureRecord) -> Checked {
        let content = content_hash(record);
        if let Some(checked) = self.entries.touch(&content) {
            return checked.clone();
        }
        let checked = check(registry, record);
        self.entries.insert(content, checked.clone());
        checked
    }

    /// Drops the outcomes used longest ago, beyond `limit`, that the latest pass did not use.
    pub(crate) fn prune(&mut self, limit: usize) {
        self.entries.evict_to(limit);
    }

    /// Number of memoized outcomes.
    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

/// The BLAKE3 hash of what a record's validation outcome is computed from: its id, its type and
/// its params in stored order (the order matters because the unknown keys keep it). Names and
/// suppression are not part of it.
fn content_hash(record: &FeatureRecord) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    serde_json::to_writer(&mut hasher, &(&record.id, &record.kind, &record.params))
        .expect("ids, strings and JSON values always serialize, and a hasher accepts every write");
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::{record, registry};

    #[test]
    fn a_valid_record_gives_canonical_params_dependencies_and_names() {
        let registry = registry();
        let record = record(json!({
            "id": "f-d1", "type": "test_datum", "offset": "h * printer.fit.slip + w", "taper": 2
        }));

        let valid = check(&registry, &record).expect("the record is valid");

        assert_eq!(valid.unknown_keys, ["taper"]);
        assert_eq!(
            valid.params_text,
            r#"{"offset":"h * printer.fit.slip + w"}"#
        );
        assert_eq!(valid.dependencies.expressions.len(), 1);
        assert_eq!(valid.parameter_names, ["h", "w"]);
        assert_eq!(valid.value_names, ["printer.fit.slip"]);
    }

    #[test]
    fn an_expression_that_does_not_parse_names_nothing() {
        let registry = registry();
        let record = record(json!({"id": "f-d1", "type": "test_datum", "offset": "2 +"}));

        let valid = check(&registry, &record).expect("expressions are not parsed by validation");

        assert!(valid.parameter_names.is_empty());
        assert!(valid.value_names.is_empty());
    }

    #[test]
    fn an_unknown_type_is_invalid() {
        let registry = registry();
        let record = record(json!({"id": "f-g", "type": "gear_demo", "teeth": 12}));

        let error = check(&registry, &record).expect_err("the type is unknown");

        assert_eq!(error.code(), "unknown_type");
    }

    #[test]
    fn unchanged_records_reuse_the_memoized_outcome() {
        let registry = registry();
        let mut validations = Validations::default();
        let record = record(json!({"id": "f-d1", "type": "test_datum", "offset": "1"}));

        let first = validations.check(&registry, &record).expect("valid");
        let second = validations.check(&registry, &record).expect("valid");

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn a_renamed_or_suppressed_record_reuses_the_memoized_outcome() {
        let registry = registry();
        let mut validations = Validations::default();
        let original = record(json!({"id": "f-d1", "type": "test_datum", "offset": "1"}));
        let renamed = record(json!({
            "id": "f-d1", "type": "test_datum", "name": "Top", "suppressed": true, "offset": "1"
        }));

        let first = validations.check(&registry, &original).expect("valid");
        let second = validations.check(&registry, &renamed).expect("valid");

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn changed_params_are_checked_again() {
        let registry = registry();
        let mut validations = Validations::default();
        let original = record(json!({"id": "f-d1", "type": "test_datum", "offset": "1"}));
        let edited = record(json!({"id": "f-d1", "type": "test_datum", "offset": "2"}));

        validations.check(&registry, &original).expect("valid");
        let checked = validations.check(&registry, &edited).expect("valid");

        assert_eq!(checked.params_text, r#"{"offset":"2"}"#);
        assert_eq!(validations.len(), 2);
    }

    #[test]
    fn documents_that_share_ids_keep_their_outcomes() {
        let registry = registry();
        let mut validations = Validations::default();
        let in_first = record(json!({"id": "f-000000000001", "type": "test_datum", "offset": "1"}));
        let in_second =
            record(json!({"id": "f-000000000001", "type": "test_datum", "offset": "2"}));

        validations.begin_pass();
        let first = validations.check(&registry, &in_first).expect("valid");
        validations.begin_pass();
        let second = validations.check(&registry, &in_second).expect("valid");
        validations.begin_pass();
        let first_again = validations.check(&registry, &in_first).expect("valid");
        validations.begin_pass();
        let second_again = validations.check(&registry, &in_second).expect("valid");

        assert!(Arc::ptr_eq(&first, &first_again));
        assert!(Arc::ptr_eq(&second, &second_again));
    }

    #[test]
    fn records_with_other_ids_are_checked_on_their_own() {
        let registry = registry();
        let mut validations = Validations::default();
        let one = record(json!({"id": "f-1", "type": "test_datum", "offset": "1"}));
        let other = record(json!({"id": "f-2", "type": "test_datum", "offset": "1"}));

        let first = validations.check(&registry, &one).expect("valid");
        let second = validations.check(&registry, &other).expect("valid");

        assert!(!Arc::ptr_eq(&first, &second));
        assert_eq!(validations.len(), 2);
    }

    #[test]
    fn reordered_unknown_keys_are_checked_again() {
        let registry = registry();
        let mut validations = Validations::default();
        let first =
            record(json!({"id": "f-d1", "type": "test_datum", "offset": "1", "a": 1, "b": 2}));
        let second =
            record(json!({"id": "f-d1", "type": "test_datum", "offset": "1", "b": 2, "a": 1}));

        validations.check(&registry, &first).expect("valid");
        let checked = validations.check(&registry, &second).expect("valid");

        assert_eq!(checked.unknown_keys, ["b", "a"]);
    }

    #[test]
    fn a_changed_type_is_checked_again() {
        let registry = registry();
        let mut validations = Validations::default();
        let datum = record(json!({"id": "f-x", "type": "test_datum", "offset": "1"}));
        let unknown = record(json!({"id": "f-x", "type": "gear_demo", "offset": "1"}));

        validations.check(&registry, &datum).expect("valid");
        let checked = validations.check(&registry, &unknown);

        assert!(checked.is_err());
    }

    /// Checks a `test_datum` record with this id in a new pass.
    fn check_in_pass(validations: &mut Validations, registry: &Registry, feature: &str) -> Checked {
        validations.begin_pass();
        validations.check(
            registry,
            &record(json!({"id": feature, "type": "test_datum", "offset": "1"})),
        )
    }

    #[test]
    fn pruning_drops_the_least_recently_used_outcomes_first() {
        let registry = registry();
        let mut validations = Validations::default();
        let oldest = check_in_pass(&mut validations, &registry, "f-1").expect("valid");
        check_in_pass(&mut validations, &registry, "f-2").expect("valid");
        check_in_pass(&mut validations, &registry, "f-3").expect("valid");

        validations.prune(2);

        assert_eq!(validations.len(), 2);
        let checked_again = check_in_pass(&mut validations, &registry, "f-1").expect("valid");
        assert!(!Arc::ptr_eq(&oldest, &checked_again));
    }

    #[test]
    fn content_hashes_follow_id_type_and_params_in_stored_order() {
        let hash = |value: serde_json::Value| content_hash(&record(value));
        let original =
            hash(json!({"id": "f-1", "type": "test_datum", "offset": "1", "a": 1, "b": 2}));

        assert_eq!(
            original,
            hash(json!({
                "id": "f-1", "type": "test_datum", "name": "Top", "suppressed": true,
                "offset": "1", "a": 1, "b": 2
            }))
        );
        assert_ne!(
            original,
            hash(json!({"id": "f-2", "type": "test_datum", "offset": "1", "a": 1, "b": 2}))
        );
        assert_ne!(
            original,
            hash(json!({"id": "f-1", "type": "test_other", "offset": "1", "a": 1, "b": 2}))
        );
        assert_ne!(
            original,
            hash(json!({"id": "f-1", "type": "test_datum", "offset": "1", "b": 2, "a": 1}))
        );
        assert_ne!(
            original,
            hash(json!({"id": "f-1", "type": "test_datum", "offset": "2", "a": 1, "b": 2}))
        );
    }
}
