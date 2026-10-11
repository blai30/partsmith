//! How the model learns what feature params declare and refer to (model.md section 2.7).

use crate::{Expr, FeatureRecord, Id};

/// What a feature record declares and refers to, as reported by the feature registry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FeatureLinks {
    /// Ids the params define, such as sketch entity and constraint ids, in params order.
    pub declared_ids: Vec<Id>,
    /// Bodies the feature writes, then bodies it reads, each in params order.
    pub bodies: Vec<Id>,
    /// Other features whose output the feature reads directly (sketches, datums), in params order.
    pub features: Vec<Id>,
    /// Every expression stored in the params, evaluated by build or not, in params order.
    pub expressions: Vec<ExprLink>,
}

/// One expression stored in feature params, with its location.
#[derive(Clone, Debug, PartialEq)]
pub struct ExprLink {
    /// JSON Pointer (RFC 6901) of the expression string within the params, for example "/extent/expr".
    pub pointer: String,
    /// The expression.
    pub expr: Expr,
}

/// Reports the links of a feature record; implemented by the feature registry.
pub trait FeatureInspector: Send + Sync {
    /// Returns the record's links, or a user-facing message saying why its params are invalid.
    fn inspect(&self, feature: &FeatureRecord) -> Result<FeatureLinks, String>;
}

/// Inspector that treats every record as having no links; used where no registry is available.
#[derive(Clone, Copy, Debug, Default)]
pub struct OpaqueFeatures;

impl FeatureInspector for OpaqueFeatures {
    /// Always `Ok(FeatureLinks::default())`.
    fn inspect(&self, _feature: &FeatureRecord) -> Result<FeatureLinks, String> {
        Ok(FeatureLinks::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_features_have_no_links() {
        let record = FeatureRecord::new(Id::new("f-1").expect("valid id"), "box");

        assert_eq!(OpaqueFeatures.inspect(&record), Ok(FeatureLinks::default()));
    }
}
