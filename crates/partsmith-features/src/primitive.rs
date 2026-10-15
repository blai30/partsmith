//! The placement params every primitive shares (features.md section 3.14).

use partsmith_model::{Expr, FieldHint, FieldUnit, SelectionKind};
use partsmith_sketch::Placement;
use schemars::Schema;
use serde_json::Value;

use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::FeatureError;
use crate::params::{PlacementExt, PlaneRef};
use crate::python::{PythonNames, PythonValue};

/// Field paths of the three position coordinates.
const POSITION_FIELDS: [&str; 3] = ["position[0]", "position[1]", "position[2]"];

/// The default `position`: `["0", "0", "0"]`.
pub fn default_position() -> [Expr; 3] {
    [Expr::new("0"), Expr::new("0"), Expr::new("0")]
}

/// True for the default `position`; for `skip_serializing_if`.
pub fn is_default_position(position: &[Expr; 3]) -> bool {
    *position == default_position()
}

/// Writes the default `["0", "0", "0"]` into the schema of a `position` field, or of any array of
/// three lengths that `default_position` and `is_default_position` give its default; for
/// `#[schemars(transform = ...)]`. schemars leaves out a default that `skip_serializing_if` would
/// skip, but a host sets one item of such an array only by sending all three, so it starts them
/// from the default (features.md section 3.3).
pub fn add_position_default(schema: &mut Schema) {
    let default = default_position().map(|expr| Value::String(expr.as_str().to_owned()));
    schema.insert("default".to_owned(), Value::Array(default.into()));
}

/// The `plane` and `position` params a primitive declares after its size fields and before `op`:
/// the plane it stands on (default origin XY) and the offset of its origin in the plane frame
/// (default `["0", "0", "0"]`), both omitted when default.
#[derive(Clone, Copy, Debug)]
pub struct PrimitivePlacement<'a> {
    /// The `plane` param.
    pub plane: &'a PlaneRef,
    /// The `position` param: x along the plane's x-axis, y along its y-axis, z along its normal.
    pub position: &'a [Expr; 3],
}

impl PrimitivePlacement<'_> {
    /// Declares what the placement reads: a datum plane's feature or a face's body (a body already
    /// written stays written), then the three position lengths.
    pub fn add_dependencies(&self, dependencies: &mut Dependencies) {
        dependencies.add_plane("plane", self.plane);
        for (field, expr) in POSITION_FIELDS.iter().zip(self.position) {
            dependencies.add_expression(field, expr, FieldUnit::Length);
        }
    }

    /// The primitive's local frame: the plane's placement with its origin moved to the position.
    pub fn frame(&self, context: &BuildContext<'_>) -> Result<Placement, FeatureError> {
        let plane = context.plane("plane", self.plane)?;
        let mut offset = [0.0; 3];
        for ((value, field), expr) in offset.iter_mut().zip(POSITION_FIELDS).zip(self.position) {
            *value = context.eval_length(field, expr)?;
        }
        Ok(Placement {
            origin: plane.local_to_world(offset),
            ..plane
        })
    }

    /// The `plane=` and `position=` keywords of the dedicated Python form, each only when it is
    /// not the default. None when the plane's variable is missing.
    pub fn python_keywords(&self, names: &dyn PythonNames) -> Option<Vec<(String, PythonValue)>> {
        let mut keywords = Vec::new();
        if !self.plane.is_default() {
            keywords.push(("plane".to_string(), PythonValue::plane(self.plane, names)?));
        }
        if !is_default_position(self.position) {
            let coordinates = self
                .position
                .iter()
                .map(|expr| PythonValue::expression(expr, names))
                .collect();
            keywords.push(("position".to_string(), PythonValue::Tuple(coordinates)));
        }
        Some(keywords)
    }

    /// The manifest hints of `plane` and `position`, none of them essential.
    pub fn hints() -> Vec<FieldHint> {
        let hint = |path: &str, label: &str| FieldHint {
            path: path.to_string(),
            label: label.to_string(),
            essential: false,
            unit: None,
            selection: None,
            options: Vec::new(),
        };
        let mut hints = vec![FieldHint {
            selection: Some(SelectionKind::Plane),
            ..hint("plane", "Plane")
        }];
        for (path, label) in POSITION_FIELDS
            .iter()
            .zip(["Position X", "Position Y", "Position Z"])
        {
            hints.push(FieldHint {
                unit: Some(FieldUnit::Length),
                ..hint(path, label)
            });
        }
        hints
    }
}

#[cfg(test)]
mod tests {
    use partsmith_sketch::OriginPlane;

    use super::*;
    use crate::test_support::{ScriptNames, id};

    fn datum(feature: &str) -> PlaneRef {
        PlaneRef::Datum {
            feature: id(feature),
        }
    }

    #[test]
    fn default_position_is_three_zeros() {
        assert!(is_default_position(&default_position()));
        assert!(!is_default_position(&[
            Expr::new("0"),
            Expr::new("0 mm"),
            Expr::new("0")
        ]));
    }

    #[test]
    fn position_default_is_written_into_the_schema() {
        let mut schema = Schema::try_from(serde_json::json!({
            "type": "array",
            "minItems": 3,
            "maxItems": 3
        }))
        .expect("an object is a schema");

        add_position_default(&mut schema);

        assert_eq!(
            schema.as_value(),
            &serde_json::json!({
                "type": "array",
                "minItems": 3,
                "maxItems": 3,
                "default": ["0", "0", "0"]
            })
        );
    }

    #[test]
    fn position_default_replaces_a_default_already_there() {
        let mut schema = Schema::try_from(serde_json::json!({"default": ["1", "2", "3"]}))
            .expect("an object is a schema");

        add_position_default(&mut schema);

        assert_eq!(
            schema.as_value(),
            &serde_json::json!({"default": ["0", "0", "0"]})
        );
    }

    #[test]
    fn dependencies_are_the_plane_then_the_position() {
        let plane = datum("f-dp1");
        let position = [Expr::new("1"), Expr::new("2"), Expr::new("3")];
        let mut dependencies = Dependencies::default();

        PrimitivePlacement {
            plane: &plane,
            position: &position,
        }
        .add_dependencies(&mut dependencies);

        assert_eq!(dependencies.features[0].field, "plane.feature");
        let fields: Vec<&str> = dependencies
            .expressions
            .iter()
            .map(|expression| expression.field.as_str())
            .collect();
        assert_eq!(fields, POSITION_FIELDS);
    }

    #[test]
    fn default_placement_writes_no_keywords() {
        let plane = PlaneRef::default();
        let position = default_position();

        let keywords = PrimitivePlacement {
            plane: &plane,
            position: &position,
        }
        .python_keywords(&ScriptNames::default());

        assert_eq!(keywords, Some(Vec::new()));
    }

    #[test]
    fn placed_primitives_write_plane_and_position() {
        let plane = datum("f-dp1");
        let position = [Expr::new("20"), Expr::new("0"), Expr::new("5 mm")];

        let keywords = PrimitivePlacement {
            plane: &plane,
            position: &position,
        }
        .python_keywords(&ScriptNames::default())
        .expect("the datum has a variable");

        let rendered: Vec<String> = keywords
            .iter()
            .map(|(name, value)| format!("{name}={}", value.render()))
            .collect();
        assert_eq!(rendered, ["plane=plane_1", "position=(20, 0, 5*mm)"]);
    }

    #[test]
    fn a_missing_datum_variable_has_no_dedicated_form() {
        let plane = datum("f-dp9");
        let position = default_position();

        let keywords = PrimitivePlacement {
            plane: &plane,
            position: &position,
        }
        .python_keywords(&ScriptNames::default());

        assert_eq!(keywords, None);
    }

    #[test]
    fn hints_label_plane_and_positions() {
        let hints = PrimitivePlacement::hints();

        let labels: Vec<&str> = hints.iter().map(|hint| hint.label.as_str()).collect();
        assert_eq!(labels, ["Plane", "Position X", "Position Y", "Position Z"]);
        assert_eq!(hints[0].selection, Some(SelectionKind::Plane));
        assert!(
            hints[1..]
                .iter()
                .all(|hint| hint.unit == Some(FieldUnit::Length))
        );
        assert!(hints.iter().all(|hint| !hint.essential));
    }

    #[test]
    fn origin_planes_other_than_xy_are_not_default() {
        let plane = PlaneRef::Origin {
            plane: OriginPlane::Xz,
        };

        assert!(!plane.is_default());
    }
}
