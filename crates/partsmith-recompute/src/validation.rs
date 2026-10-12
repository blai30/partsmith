//! Validated feature records and their memo (recompute.md section 3.1 steps 1 and 2).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use partsmith_features::{Dependencies, FeatureError, Registry};
use partsmith_model::{Document, FeatureRecord, Id};
use serde_json::{Map, Value};

use crate::key::params_text;

/// A record that passed validation, with what recompute derives from it.
#[derive(Debug)]
pub(crate) struct Valid {
    /// Canonical params: known keys in canonical order, then the unknown keys.
    pub(crate) params: Map<String, Value>,
    /// Keys the feature type does not know, in their original order.
    pub(crate) unknown_keys: Vec<String>,
    /// What the record declares it reads and defines.
    pub(crate) dependencies: Dependencies,
    /// The known canonical params written compactly, for its cache key.
    pub(crate) params_text: String,
    /// Parameter names its expressions use directly, without repeats.
    pub(crate) parameter_names: Vec<String>,
    /// Dotted names its expressions use directly, without repeats.
    pub(crate) value_names: Vec<String>,
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
        dependencies,
        parameter_names: parameter_names.into_iter().collect(),
        value_names: value_names.into_iter().collect(),
    }))
}

/// Validation outcomes memoized per feature id and record content, so unchanged records are not
/// parsed again. One per `Recompute`, whose registry never changes.
#[derive(Debug, Default)]
pub(crate) struct Validations {
    /// The latest outcome for each feature id, with the content it was computed from.
    entries: BTreeMap<Id, Memo>,
}

/// One memoized outcome.
#[derive(Debug)]
struct Memo {
    /// The record type the outcome was computed for.
    kind: String,
    /// The params, in their stored order, the outcome was computed for.
    params: Map<String, Value>,
    /// The outcome.
    checked: Checked,
}

impl Validations {
    /// The validation outcome of `record`, computed only when its type or params changed since
    /// the last time a record with its id was checked.
    pub(crate) fn check(&mut self, registry: &Registry, record: &FeatureRecord) -> Checked {
        if let Some(memo) = self.entries.get(&record.id)
            && memo.kind == record.kind
            && same_params(&memo.params, &record.params)
        {
            return memo.checked.clone();
        }
        let checked = check(registry, record);
        self.entries.insert(
            record.id.clone(),
            Memo {
                kind: record.kind.clone(),
                params: record.params.clone(),
                checked: checked.clone(),
            },
        );
        checked
    }

    /// When more than `limit` outcomes are kept, keeps only those of `document`'s records.
    pub(crate) fn prune(&mut self, limit: usize, document: &Document) {
        if self.entries.len() > limit {
            let current: BTreeSet<&Id> = document.features.iter().map(|record| &record.id).collect();
            self.entries.retain(|feature, _| current.contains(feature));
        }
    }

    /// Number of memoized outcomes.
    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

/// True when two params maps have equal keys in the same order with equal values. The order
/// matters because the unknown keys keep it.
fn same_params(left: &Map<String, Value>, right: &Map<String, Value>) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|((left_key, left_value), (right_key, right_value))| {
                left_key == right_key && left_value == right_value
            })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::{document, record, registry};

    #[test]
    fn a_valid_record_gives_canonical_params_dependencies_and_names() {
        let registry = registry();
        let record = record(json!({
            "id": "f-d1", "type": "test_datum", "offset": "h * printer.fit.slip + w", "taper": 2
        }));

        let valid = check(&registry, &record).expect("the record is valid");

        assert_eq!(valid.unknown_keys, ["taper"]);
        assert_eq!(valid.params_text, r#"{"offset":"h * printer.fit.slip + w"}"#);
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
        assert_eq!(validations.len(), 1);
    }

    #[test]
    fn reordered_unknown_keys_are_checked_again() {
        let registry = registry();
        let mut validations = Validations::default();
        let first = record(json!({"id": "f-d1", "type": "test_datum", "offset": "1", "a": 1, "b": 2}));
        let second = record(json!({"id": "f-d1", "type": "test_datum", "offset": "1", "b": 2, "a": 1}));

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

    #[test]
    fn pruning_keeps_the_current_document_once_over_the_limit() {
        let registry = registry();
        let mut validations = Validations::default();
        let old = record(json!({"id": "f-old", "type": "test_datum", "offset": "1"}));
        let current = document(json!([{"id": "f-new", "type": "test_datum", "offset": "1"}]));
        validations.check(&registry, &old).expect("valid");
        validations
            .check(&registry, &current.features[0])
            .expect("valid");

        validations.prune(2, &current);
        assert_eq!(validations.len(), 2);
        validations.prune(1, &current);
        assert_eq!(validations.len(), 1);
    }

    #[test]
    fn params_compare_by_order_and_value() {
        let left = json!({"a": 1, "b": {"x": 1, "y": 2}});
        let same = json!({"a": 1, "b": {"y": 2, "x": 1}});
        let reordered = json!({"b": {"x": 1, "y": 2}, "a": 1});
        let changed = json!({"a": 2, "b": {"x": 1, "y": 2}});
        let as_map = |value: &Value| value.as_object().expect("an object").clone();

        assert!(same_params(&as_map(&left), &as_map(&same)));
        assert!(!same_params(&as_map(&left), &as_map(&reordered)));
        assert!(!same_params(&as_map(&left), &as_map(&changed)));
    }
}
