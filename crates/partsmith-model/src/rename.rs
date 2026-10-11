//! The edit set that renames a parameter everywhere (model.md section 3.13).

use std::mem;

use serde_json::Value;

use crate::names::validate_parameter_name;
use crate::{Document, Edit, EditError, Expr, FeatureInspector, Id, Parameter, RecordKind};

impl Document {
    /// The edits that rename parameter `id` to `new_name` and rewrite every expression that uses
    /// it (model.md section 3.13), as one transaction for `UndoLog::commit`. The document is not changed.
    pub fn rename_parameter_edits(
        &self,
        id: &Id,
        new_name: &str,
        inspector: &dyn FeatureInspector,
    ) -> Result<Vec<Edit>, EditError> {
        let renamed = self.parameter(id).ok_or_else(|| EditError::NotFound {
            kind: RecordKind::Parameter,
            id: id.clone(),
        })?;
        validate_parameter_name(new_name).map_err(EditError::InvalidParameterName)?;
        let name_taken = self
            .parameters
            .iter()
            .any(|parameter| parameter.id != *id && parameter.name == new_name);
        if name_taken {
            return Err(EditError::NameInUse {
                name: new_name.to_string(),
            });
        }
        let old_name = renamed.name.as_str();
        if old_name == new_name {
            return Ok(Vec::new());
        }
        // An expression that does not parse stays as it is.
        let rewrite = |expr: &Expr| {
            expr.rename_parameter(old_name, new_name)
                .unwrap_or_else(|_| expr.clone())
        };

        let mut edits = vec![Edit::UpdateParameter {
            parameter: Parameter {
                name: new_name.to_string(),
                expr: rewrite(&renamed.expr),
                ..renamed.clone()
            },
        }];
        for parameter in self
            .parameters
            .iter()
            .filter(|parameter| parameter.id != *id)
        {
            let expr = rewrite(&parameter.expr);
            if expr != parameter.expr {
                edits.push(Edit::UpdateParameter {
                    parameter: Parameter {
                        expr,
                        ..parameter.clone()
                    },
                });
            }
        }
        for feature in &self.features {
            let Ok(links) = inspector.inspect(feature) else {
                continue;
            };
            let changes: Vec<(String, Expr, Expr)> = links
                .expressions
                .into_iter()
                .filter_map(|link| {
                    let rewritten = rewrite(&link.expr);
                    (rewritten != link.expr).then_some((link.pointer, link.expr, rewritten))
                })
                .collect();
            if changes.is_empty() {
                continue;
            }
            let mut updated = feature.clone();
            let mut params = Value::Object(mem::take(&mut updated.params));
            for (pointer, old, new) in changes {
                if let Some(slot) = params.pointer_mut(&pointer)
                    && slot.as_str() == Some(old.as_str())
                {
                    *slot = Value::String(new.as_str().to_string());
                }
            }
            // Only string slots are replaced, so the params stay an object.
            if let Value::Object(params) = params {
                updated.params = params;
            }
            edits.push(Edit::UpdateFeature { feature: updated });
        }
        Ok(edits)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{ExprLink, FeatureLinks, FeatureRecord};

    fn id(text: &str) -> Id {
        Id::new(text).expect("valid id")
    }

    /// Reports one expression link with a fixed pointer and text.
    struct OneLink {
        pointer: &'static str,
        source: &'static str,
    }

    impl FeatureInspector for OneLink {
        fn inspect(&self, _feature: &FeatureRecord) -> Result<FeatureLinks, String> {
            Ok(FeatureLinks {
                expressions: vec![ExprLink {
                    pointer: self.pointer.to_string(),
                    expr: Expr::new(self.source),
                }],
                ..FeatureLinks::default()
            })
        }
    }

    fn document() -> Document {
        let mut document = Document::default();
        document
            .parameters
            .push(Parameter::new(id("p-a"), "a", "1"));
        let mut record = FeatureRecord::new(id("f-1"), "box");
        record.params = json!({"extent": {"expr": "a * 2"}, "a/b": "a"})
            .as_object()
            .cloned()
            .expect("an object");
        document.features.push(record);
        document
    }

    fn renamed_params(edits: &[Edit]) -> Option<serde_json::Map<String, Value>> {
        edits.iter().find_map(|edit| match edit {
            Edit::UpdateFeature { feature } => Some(feature.params.clone()),
            _ => None,
        })
    }

    #[test]
    fn nested_pointers_are_rewritten() {
        let inspector = OneLink {
            pointer: "/extent/expr",
            source: "a * 2",
        };

        let edits = document()
            .rename_parameter_edits(&id("p-a"), "b", &inspector)
            .expect("the rename is valid");

        assert_eq!(
            renamed_params(&edits).map(Value::Object),
            Some(json!({"extent": {"expr": "b * 2"}, "a/b": "a"}))
        );
    }

    #[test]
    fn escaped_pointers_are_followed() {
        let inspector = OneLink {
            pointer: "/a~1b",
            source: "a",
        };

        let edits = document()
            .rename_parameter_edits(&id("p-a"), "b", &inspector)
            .expect("the rename is valid");

        assert_eq!(
            renamed_params(&edits).map(|params| params["a/b"].clone()),
            Some(json!("b"))
        );
    }

    #[test]
    fn a_link_whose_text_differs_is_left_alone() {
        let inspector = OneLink {
            pointer: "/extent/expr",
            source: "a + 5",
        };

        let edits = document()
            .rename_parameter_edits(&id("p-a"), "b", &inspector)
            .expect("the rename is valid");

        assert_eq!(
            renamed_params(&edits).map(|params| params["extent"].clone()),
            Some(json!({"expr": "a * 2"}))
        );
    }

    #[test]
    fn the_renamed_parameter_keeps_its_comment() {
        let mut document = document();
        document.parameters[0].comment = Some("Width".to_string());

        let edits = document
            .rename_parameter_edits(&id("p-a"), "b", &crate::OpaqueFeatures)
            .expect("the rename is valid");

        assert_eq!(
            edits,
            [Edit::UpdateParameter {
                parameter: Parameter {
                    id: id("p-a"),
                    name: "b".to_string(),
                    expr: Expr::new("1"),
                    comment: Some("Width".to_string()),
                }
            }]
        );
    }
}
