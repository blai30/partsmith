//! The document recipe and its records (model.md section 2.5).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::canonical::normalize_numbers;
use crate::expr::evaluate_parameters;
use crate::names::{LabelError, LabelField, is_valid_feature_type, validate_label};
use crate::{
    BodyRole, Color, EvaluationContext, Expr, FORMAT, FORMAT_VERSION, Id, ParameterValues, Units,
};

/// The document recipe. Geometry is never stored; it is regenerated from this.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Document {
    /// Always "partsmith".
    pub format: String,
    /// Format version; equals FORMAT_VERSION for every document in memory.
    pub format_version: u32,
    /// Unit of bare numbers in length expressions, and the display unit for lengths.
    pub units: Units,
    /// Printer profile id whose values `printer.*` names resolve to; `None` when no printer is chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub printer: Option<String>,
    /// Parameters in the order the user sees them.
    #[serde(default)]
    pub parameters: Vec<Parameter>,
    /// Declared bodies in browser order.
    #[serde(default)]
    pub bodies: Vec<Body>,
    /// Feature records in timeline order.
    #[serde(default)]
    pub features: Vec<FeatureRecord>,
    /// First rolled-back feature: it and every later feature do not run. `None` means nothing is rolled back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback: Option<Id>,
}

impl Document {
    /// An empty, structurally valid document in the given units.
    pub fn new(units: Units) -> Document {
        Document {
            format: FORMAT.to_string(),
            format_version: FORMAT_VERSION,
            units,
            printer: None,
            parameters: Vec::new(),
            bodies: Vec::new(),
            features: Vec::new(),
            rollback: None,
        }
    }

    /// The parameter with this id.
    pub fn parameter(&self, id: &Id) -> Option<&Parameter> {
        self.parameters.iter().find(|parameter| parameter.id == *id)
    }

    /// The parameter with this name (exact, case-sensitive).
    pub fn parameter_by_name(&self, name: &str) -> Option<&Parameter> {
        self.parameters
            .iter()
            .find(|parameter| parameter.name == name)
    }

    /// The body with this id.
    pub fn body(&self, id: &Id) -> Option<&Body> {
        self.bodies.iter().find(|body| body.id == *id)
    }

    /// The feature with this id.
    pub fn feature(&self, id: &Id) -> Option<&FeatureRecord> {
        self.features.iter().find(|feature| feature.id == *id)
    }

    /// Timeline position of the feature with this id.
    pub fn feature_index(&self, id: &Id) -> Option<usize> {
        self.features.iter().position(|feature| feature.id == *id)
    }

    /// Features before the rollback feature, or all features (model.md section 3.8).
    pub fn active_features(&self) -> &[FeatureRecord] {
        let end = self
            .rollback
            .as_ref()
            .and_then(|rollback| self.feature_index(rollback))
            .unwrap_or(self.features.len());
        &self.features[..end]
    }

    /// True when a parameter, body or feature has this id.
    pub fn contains_id(&self, id: &Id) -> bool {
        self.parameter(id).is_some() || self.body(id).is_some() || self.feature(id).is_some()
    }

    /// Evaluates every parameter (expressions.md section 3.13).
    pub fn evaluate_parameters(&self, context: &dyn EvaluationContext) -> ParameterValues {
        evaluate_parameters(&self.parameters, self.units, context)
    }

    /// Rewrites every number inside feature params to its normalized form (file-format.md section 3.3).
    pub fn normalize(&mut self) {
        for feature in &mut self.features {
            feature.normalize();
        }
    }
}

impl Default for Document {
    /// Same as `Document::new(Units::Mm)`.
    fn default() -> Document {
        Document::new(Units::Mm)
    }
}

/// A named expression that other expressions use by name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    /// Permanent id.
    pub id: Id,
    /// Name used in expressions; a valid parameter name, unique in the document.
    pub name: String,
    /// Expression source text.
    pub expr: Expr,
    /// Free-text note shown next to the parameter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

impl Parameter {
    /// Creates a parameter without a comment.
    pub fn new(id: Id, name: impl Into<String>, expr: impl Into<Expr>) -> Parameter {
        Parameter {
            id,
            name: name.into(),
            expr: expr.into(),
            comment: None,
        }
    }
}

/// A declared solid body that features build into.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Body {
    /// Permanent id.
    pub id: Id,
    /// Display name; a valid label.
    pub name: String,
    /// Material name such as "PETG"; a valid label when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// Display and export color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Shown in the viewport; default true, omitted when true.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub visible: bool,
    /// Print role; default `Part`, omitted when `Part`.
    #[serde(default, skip_serializing_if = "BodyRole::is_part")]
    pub role: BodyRole,
}

impl Body {
    /// Creates a visible part body with no material and no color.
    pub fn new(id: Id, name: impl Into<String>) -> Body {
        Body {
            id,
            name: name.into(),
            material: None,
            color: None,
            visible: true,
            role: BodyRole::Part,
        }
    }

    /// What is wrong with the name, then with the material (model.md section 3.6).
    /// `Document::check_structure` checks the same rules in place.
    pub(crate) fn label_errors(&self) -> impl Iterator<Item = LabelError> + '_ {
        let labels = [
            (LabelField::BodyName, Some(&self.name)),
            (LabelField::Material, self.material.as_ref()),
        ];
        labels
            .into_iter()
            .filter_map(|(field, text)| validate_label(field, text?).err())
    }
}

/// A structural rule a feature record breaks (model.md section 3.6).
#[derive(Debug, PartialEq)]
pub(crate) enum RecordProblem<'record> {
    /// The type is not a valid feature type name.
    InvalidType,
    /// The name breaks the label rules.
    InvalidName(LabelError),
    /// A params key is one the record itself uses.
    ReservedKey(&'record String),
}

/// One timeline entry. Its params are stored generically and owned by the feature registry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FeatureRecord {
    /// Permanent id.
    pub id: Id,
    /// Feature type name such as "extrude"; serialized as "type".
    #[serde(rename = "type")]
    pub kind: String,
    /// Display name in the timeline; a valid label when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Skipped by recompute; default false, omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub suppressed: bool,
    /// Feature params, flattened into the record. Key order is significant (file-format.md section 3.2).
    #[serde(flatten)]
    pub params: serde_json::Map<String, serde_json::Value>,
}

impl FeatureRecord {
    /// Keys the record itself uses; params may not contain them.
    pub const RESERVED_KEYS: [&'static str; 4] = ["id", "type", "name", "suppressed"];

    /// Creates an unsuppressed, unnamed record with empty params.
    pub fn new(id: Id, kind: impl Into<String>) -> FeatureRecord {
        FeatureRecord {
            id,
            kind: kind.into(),
            name: None,
            suppressed: false,
            params: serde_json::Map::new(),
        }
    }

    /// What is wrong with the record: its type, then its name, then each reserved params key in
    /// params order. `Document::check_structure` checks the same rules in place.
    pub(crate) fn problems(&self) -> impl Iterator<Item = RecordProblem<'_>> {
        let invalid_type =
            (!is_valid_feature_type(&self.kind)).then_some(RecordProblem::InvalidType);
        let invalid_name = self
            .name
            .as_ref()
            .and_then(|name| validate_label(LabelField::FeatureName, name).err())
            .map(RecordProblem::InvalidName);
        invalid_type
            .into_iter()
            .chain(invalid_name)
            .chain(self.reserved_params_keys().map(RecordProblem::ReservedKey))
    }

    /// The params keys that the record itself uses, in params order.
    pub(crate) fn reserved_params_keys(&self) -> impl Iterator<Item = &String> {
        self.params
            .keys()
            .filter(|key| FeatureRecord::RESERVED_KEYS.contains(&key.as_str()))
    }

    /// Rewrites every number in the params to its normalized form.
    pub(crate) fn normalize(&mut self) {
        for value in self.params.values_mut() {
            normalize_numbers(value);
        }
    }
}

/// Default of `Body::visible`.
fn default_true() -> bool {
    true
}

/// True when `value` is true; omits the default of `Body::visible`.
fn is_true(value: &bool) -> bool {
    *value
}

/// True when `value` is false; omits flags that default to false, such as
/// `FeatureRecord::suppressed` and `FieldHint::essential`.
pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn id(text: &str) -> Id {
        Id::new(text).expect("valid id")
    }

    #[test]
    fn default_document_is_in_millimeters() {
        assert_eq!(Document::default(), Document::new(Units::Mm));
    }

    #[test]
    fn contains_id_checks_every_record_kind() {
        let mut document = Document::default();
        document
            .parameters
            .push(Parameter::new(id("p-1"), "a", "1"));
        document.features.push(FeatureRecord::new(id("f-1"), "box"));

        assert!(document.contains_id(&id("p-1")));
        assert!(document.contains_id(&id("f-1")));
        assert!(!document.contains_id(&id("b-1")));
    }

    #[test]
    fn hidden_bodies_serialize_visible_false() {
        let mut body = Body::new(id("b-1"), "Lid");
        body.visible = false;

        assert_eq!(
            serde_json::to_value(&body).expect("a body serializes"),
            json!({"id": "b-1", "name": "Lid", "visible": false})
        );
    }

    #[test]
    fn missing_visible_deserializes_as_true() {
        let body: Body = serde_json::from_value(json!({"id": "b-1", "name": "Lid"}))
            .expect("the body deserializes");

        assert!(body.visible);
        assert_eq!(body.role, BodyRole::Part);
    }

    #[test]
    fn feature_records_put_params_after_record_keys() {
        let mut record = FeatureRecord::new(id("f-1"), "box");
        record.suppressed = true;
        record.params.insert("size".to_string(), json!(2));

        assert_eq!(
            serde_json::to_string(&record).expect("a record serializes"),
            r#"{"id":"f-1","type":"box","suppressed":true,"size":2}"#
        );
    }

    #[test]
    fn reserved_keys_are_listed_in_params_order() {
        let mut record = FeatureRecord::new(id("f-1"), "box");
        record.params.insert("type".to_string(), json!(1));
        record.params.insert("size".to_string(), json!(1));
        record.params.insert("id".to_string(), json!(1));

        assert_eq!(
            record.reserved_params_keys().collect::<Vec<_>>(),
            ["type", "id"]
        );
    }

    #[test]
    fn record_problems_come_type_then_name_then_keys() {
        let mut record = FeatureRecord::new(id("f-1"), "Box");
        record.name = Some(" ".to_string());
        record.params.insert("id".to_string(), json!(1));
        let key = "id".to_string();

        assert_eq!(
            record.problems().collect::<Vec<_>>(),
            [
                RecordProblem::InvalidType,
                RecordProblem::InvalidName(LabelError::Empty {
                    field: LabelField::FeatureName
                }),
                RecordProblem::ReservedKey(&key)
            ]
        );
    }

    #[test]
    fn body_label_errors_come_name_then_material() {
        let mut body = Body::new(id("b-1"), "");
        body.material = Some(String::new());

        assert_eq!(
            body.label_errors().collect::<Vec<_>>(),
            [
                LabelError::Empty {
                    field: LabelField::BodyName
                },
                LabelError::Empty {
                    field: LabelField::Material
                }
            ]
        );
    }

    #[test]
    fn normalize_rewrites_params_numbers() {
        let mut document = Document::default();
        let mut record = FeatureRecord::new(id("f-1"), "box");
        record.params.insert("size".to_string(), json!([2.0, -0.0]));
        document.features.push(record);

        document.normalize();

        let size = &document.features[0].params["size"];
        assert!(size[0].is_u64() && size[1].is_u64(), "{size:?}");
    }

    #[test]
    fn documents_reject_unknown_keys() {
        let result = serde_json::from_value::<Document>(json!({
            "format": "partsmith", "format_version": 1, "units": "mm", "extra": 1
        }));

        assert!(result.is_err());
    }
}
