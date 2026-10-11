//! Feature manifests: pure data the feature registry defines and the protocol carries (model.md section 2.12).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Static description of a feature type for the UI, schema and registry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FeatureManifest {
    /// Display name, for example "Extrude". Also the base of default feature labels.
    pub label: String,
    /// Toolbar and palette grouping.
    pub category: Category,
    /// Icon name in the UI icon set, kebab-case, for example "linear-pattern".
    pub icon: String,
    /// Params version of this type, starting at 1. Bumped only with a document migration.
    pub version: u32,
    /// Inspector fields in display order.
    pub fields: Vec<FieldHint>,
}

/// Toolbar and palette group of a feature type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Sketches.
    Sketch,
    /// Features that add or remove material from scratch or from a sketch.
    Create,
    /// Features that change existing bodies.
    Modify,
    /// Mirrors and patterns.
    Pattern,
    /// Construction geometry such as datum planes.
    Construct,
}

/// How the inspector shows one params field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FieldHint {
    /// Path of the field inside params, for example `extent.expr` or `size[0]` (grammar in features.md section 3.2).
    pub path: String,
    /// Label shown next to the field, for example "Distance".
    pub label: String,
    /// Shown without the "More" toggle. Default false, omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub essential: bool,
    /// Value kind of an `Expr` field. Omitted for non-expression fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<FieldUnit>,
    /// What the viewport picks for a reference field. Omitted for typed fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<SelectionKind>,
    /// Labels for the values of an enumeration or a tag field, in display order. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<OptionHint>,
}

/// Value kind of an expression field, which sets its input unit, its expected dimension and its validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FieldUnit {
    /// A length in millimeters (expected dimension `LENGTH`).
    Length,
    /// An angle in radians (expected dimension `ANGLE`).
    Angle,
    /// A plain number (expected dimension `NONE`).
    Scalar,
    /// A plain non-negative whole number (expected dimension `NONE`).
    Count,
}

/// What a reference field accepts from the viewport or the browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SelectionKind {
    /// A body.
    Body,
    /// A sketch feature.
    Sketch,
    /// One or more closed regions of a sketch.
    SketchRegion,
    /// One or more points of a sketch (point entities, line and arc ends, arc and circle centers).
    SketchPoint,
    /// Faces of a body.
    Face,
    /// Edges of a body.
    Edge,
    /// An origin plane, a datum plane or a flat face.
    Plane,
    /// An origin axis, a sketch line, a straight edge, or a cylindrical, conical, spherical or toroidal face.
    Axis,
}

/// Display label for one value of an enumeration field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OptionHint {
    /// The serialized value, for example "two_sides".
    pub value: String,
    /// The label, for example "Two sides".
    pub label: String,
}

/// True when `value` is false; omits the default of `FieldHint::essential`.
fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn essential_and_unit_are_written_when_set() {
        let hint = FieldHint {
            path: "extent.expr".to_string(),
            label: "Distance".to_string(),
            essential: true,
            unit: Some(FieldUnit::Length),
            selection: None,
            options: Vec::new(),
        };

        assert_eq!(
            serde_json::to_value(&hint).expect("a hint serializes"),
            json!({"path": "extent.expr", "label": "Distance", "essential": true, "unit": "length"})
        );
    }

    #[test]
    fn omitted_fields_take_their_defaults() {
        let hint: FieldHint = serde_json::from_value(json!({"path": "op", "label": "Operation"}))
            .expect("a minimal hint deserializes");

        assert!(!hint.essential);
        assert_eq!(hint.unit, None);
        assert!(hint.options.is_empty());
    }

    #[test]
    fn categories_and_selections_are_snake_case() {
        assert_eq!(
            serde_json::to_value(Category::Construct).expect("serializes"),
            json!("construct")
        );
        assert_eq!(
            serde_json::to_value(SelectionKind::SketchPoint).expect("serializes"),
            json!("sketch_point")
        );
    }
}
