//! How the model learns what feature params declare and refer to (model.md section 2.7).

use std::borrow::Cow;

use serde_json::{Map, Value};

use crate::{Expr, FeatureRecord, FieldUnit, Id};

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

/// One expression stored in feature params, with its location and value kind.
#[derive(Clone, Debug, PartialEq)]
pub struct ExprLink {
    /// JSON Pointer (RFC 6901) of the expression string within the params, for example "/extent/expr".
    pub pointer: String,
    /// The expression.
    pub expr: Expr,
    /// The value kind the feature evaluates it as (model.md section 2.12), which says whether its
    /// bare numbers take the document units (`Length`).
    pub unit: FieldUnit,
}

impl ExprLink {
    /// True when the link's pointer leads to a JSON string in `params` equal to its expression.
    /// Edits rewrite only such links and leave the others alone (model.md sections 3.12 and 3.13).
    pub(crate) fn is_found_in(&self, params: &Map<String, Value>) -> bool {
        first_token(&self.pointer)
            .and_then(|(key, rest)| params.get(key.as_ref())?.pointer(rest))
            .is_some_and(|slot| slot.as_str() == Some(self.expr.as_str()))
    }

    /// Replaces the link's expression in `params` with `replacement` when the link is found there
    /// (`is_found_in`); otherwise leaves `params` alone.
    pub(crate) fn replace_in(&self, params: &mut Map<String, Value>, replacement: &Expr) {
        let slot = first_token(&self.pointer)
            .and_then(|(key, rest)| params.get_mut(key.as_ref())?.pointer_mut(rest));
        if let Some(slot) = slot
            && slot.as_str() == Some(self.expr.as_str())
        {
            *slot = Value::String(replacement.as_str().to_string());
        }
    }
}

/// The params key that the first reference token of `pointer` names, unescaped, and the rest of
/// the pointer; `None` for the empty pointer (the whole params, never a string) and for text
/// that is not a pointer.
fn first_token(pointer: &str) -> Option<(Cow<'_, str>, &str)> {
    let tokens = pointer.strip_prefix('/')?;
    let (token, rest) = tokens.split_at(tokens.find('/').unwrap_or(tokens.len()));
    let key = if token.contains('~') {
        Cow::Owned(token.replace("~1", "/").replace("~0", "~"))
    } else {
        Cow::Borrowed(token)
    };
    Some((key, rest))
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
    use serde_json::json;

    use super::*;

    fn link(pointer: &str, source: &str) -> ExprLink {
        ExprLink {
            pointer: pointer.to_string(),
            expr: Expr::new(source),
            unit: FieldUnit::Length,
        }
    }

    fn params() -> Map<String, Value> {
        json!({"extent": {"expr": "2"}, "a/b~c": "3", "sizes": ["4"], "count": 5})
            .as_object()
            .cloned()
            .expect("an object")
    }

    #[test]
    fn opaque_features_have_no_links() {
        let record = FeatureRecord::new(Id::new("f-1").expect("valid id"), "box");

        assert_eq!(OpaqueFeatures.inspect(&record), Ok(FeatureLinks::default()));
    }

    #[test]
    fn links_are_found_through_nested_escaped_and_array_pointers() {
        let params = params();

        assert!(link("/extent/expr", "2").is_found_in(&params));
        assert!(link("/a~1b~0c", "3").is_found_in(&params));
        assert!(link("/sizes/0", "4").is_found_in(&params));
    }

    #[test]
    fn links_to_other_text_numbers_or_nothing_are_not_found() {
        let params = params();

        assert!(!link("/extent/expr", "2 mm").is_found_in(&params));
        assert!(!link("/count", "5").is_found_in(&params));
        assert!(!link("/sizes/1", "4").is_found_in(&params));
        assert!(!link("", "2").is_found_in(&params));
        assert!(!link("extent", "2").is_found_in(&params));
    }

    #[test]
    fn replacing_writes_only_where_the_link_is_found() {
        let mut params = params();

        link("/sizes/0", "4").replace_in(&mut params, &Expr::new("4 mm"));
        link("/extent/expr", "9").replace_in(&mut params, &Expr::new("9 mm"));
        assert_eq!(params["sizes"], json!(["4 mm"]));
        assert_eq!(params["extent"], json!({"expr": "2"}));
    }
}
