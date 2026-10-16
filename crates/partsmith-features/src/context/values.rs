//! The evaluated values of a feature's declared expressions (features.md section 3.4).

use std::collections::BTreeMap;

use partsmith_model::{Dimension, FeatureManifest, FieldUnit, Scope};

use crate::deps::ExprUse;
use crate::error::{FeatureError, FieldRef};

/// Evaluated values of one feature's declared expressions.
#[derive(Clone, Debug, Default)]
pub struct Values {
    /// The uses in declaration order, each with its converted value or error.
    entries: Vec<(ExprUse, Result<f64, FeatureError>)>,
    /// Position in `entries` of the first use of each field.
    by_field: BTreeMap<String, usize>,
}

impl Values {
    /// Evaluates each use with `expr.evaluate(scope, Some(expected))` and converts it (section 3.4).
    pub fn evaluate(uses: &[ExprUse], scope: &Scope<'_>, manifest: &FeatureManifest) -> Values {
        let mut values = Values::default();
        for declared in uses {
            let value = evaluate_use(declared, scope, manifest);
            values
                .by_field
                .entry(declared.field.clone())
                .or_insert(values.entries.len());
            values.entries.push((declared.clone(), value));
        }
        values
    }

    /// Every use with its converted value or error, in declaration order. Recompute hashes them into cache keys (recompute.md section 3.4).
    pub fn iter(&self) -> impl Iterator<Item = (&ExprUse, &Result<f64, FeatureError>)> + '_ {
        self.entries
            .iter()
            .map(|(declared, value)| (declared, value))
    }

    /// The first use declared at `field` with its value.
    pub(super) fn get(&self, field: &str) -> Option<&(ExprUse, Result<f64, FeatureError>)> {
        self.by_field
            .get(field)
            .and_then(|&position| self.entries.get(position))
    }
}

/// The dimension an expression in a field of `unit` evaluates to (section 3.4).
pub(crate) fn expected_dimension(unit: FieldUnit) -> Dimension {
    match unit {
        FieldUnit::Length => Dimension::LENGTH,
        FieldUnit::Angle => Dimension::ANGLE,
        FieldUnit::Scalar | FieldUnit::Count => Dimension::NONE,
    }
}

/// Evaluates one declared use and converts it (section 3.4).
fn evaluate_use(
    declared: &ExprUse,
    scope: &Scope<'_>,
    manifest: &FeatureManifest,
) -> Result<f64, FeatureError> {
    declared
        .expr
        .evaluate(scope, Some(expected_dimension(declared.kind)))
        .map(|quantity| quantity.value)
        .map_err(|error| FeatureError::Expression {
            field: FieldRef::new(&declared.field, manifest),
            expr: declared.expr.clone(),
            error,
        })
}

#[cfg(test)]
mod tests {
    use partsmith_model::FieldUnit;

    use crate::test_support::values;

    #[test]
    fn counts_are_stored_as_evaluated() {
        let values = values(&[("count", "2.5", FieldUnit::Count)]);

        let (_, value) = values.iter().next().expect("one use");
        assert_eq!(value, &Ok(2.5));
    }

    #[test]
    fn values_keep_declaration_order() {
        let values = values(&[("b", "2", FieldUnit::Length), ("a", "1", FieldUnit::Scalar)]);

        let fields: Vec<&str> = values
            .iter()
            .map(|(declared, _)| declared.field.as_str())
            .collect();
        assert_eq!(fields, ["b", "a"]);
    }

    #[test]
    fn scalars_are_plain_numbers() {
        let values = values(&[("factor", "1.5", FieldUnit::Scalar)]);

        let (_, value) = values.iter().next().expect("one use");
        assert_eq!(value, &Ok(1.5));
    }

    #[test]
    fn a_repeated_field_reads_its_first_use() {
        let values = values(&[("a", "1", FieldUnit::Scalar), ("a", "2", FieldUnit::Scalar)]);

        let (declared, _) = values.get("a").expect("a is declared");
        assert_eq!(declared.expr.as_str(), "1");
    }
}
