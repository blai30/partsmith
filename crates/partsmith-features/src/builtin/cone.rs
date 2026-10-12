//! The `cone` feature: a right circular cone or frustum standing on a plane (features/cone.md).

use partsmith_model::{Category, Expr, FeatureManifest, FieldUnit, Id};
use partsmith_occt_sys::{make_cone, make_cylinder};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    apply_tool, body_hint, body_op_hint, call, check_range, essential, manifest, op_keyword,
    positive_length, primitive_tool, unit_hint,
};
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::{Bound, FeatureError};
use crate::feature::Feature;
use crate::naming::kernel;
use crate::output::FeatureOutput;
use crate::params::{BodyOp, PlacementExt, PlaneRef};
use crate::primitive::{PrimitivePlacement, default_position, is_default_position};
use crate::python::{PythonCall, PythonNames, PythonValue};

/// Radii closer than this, in millimeters, make a cylinder: the kernel's cone needs two different
/// radii.
const EQUAL_RADII_TOLERANCE: f64 = 1e-9;

/// A right circular cone or frustum.
#[derive(Clone, Copy, Debug)]
pub struct Cone;

/// Params of a cone feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ConeParams {
    /// Target body.
    pub body: Id,
    /// Radius at z = 0. Length, at least 0.
    pub bottom_radius: Expr,
    /// Radius at z = height. Length, at least 0. Not both radii 0.
    pub top_radius: Expr,
    /// Height along the placement normal. Length, greater than 0.
    pub height: Expr,
    /// Placement plane. Default origin XY, omitted when default.
    #[serde(default, skip_serializing_if = "PlaneRef::is_default")]
    pub plane: PlaneRef,
    /// Placement offset. Default ["0", "0", "0"], omitted when default.
    #[serde(
        default = "default_position",
        skip_serializing_if = "is_default_position"
    )]
    pub position: [Expr; 3],
    /// How the cone combines with the body. Required.
    pub op: BodyOp,
}

impl ConeParams {
    /// The shared placement params.
    fn placement(&self) -> PrimitivePlacement<'_> {
        PrimitivePlacement {
            plane: &self.plane,
            position: &self.position,
        }
    }
}

/// A radius that must be at least 0.
fn radius(context: &BuildContext<'_>, field: &str, expr: &Expr) -> Result<f64, FeatureError> {
    let value = context.eval_length(field, expr)?;
    check_range(
        context,
        field,
        FieldUnit::Length,
        value,
        Bound::AtLeast(0.0),
        None,
    )
}

impl Feature for Cone {
    const TYPE: &'static str = "cone";
    type Params = ConeParams;

    fn manifest() -> FeatureManifest {
        let mut fields = vec![
            essential(unit_hint(
                "bottom_radius",
                "Bottom radius",
                FieldUnit::Length,
            )),
            essential(unit_hint("top_radius", "Top radius", FieldUnit::Length)),
            essential(unit_hint("height", "Height", FieldUnit::Length)),
            body_op_hint(),
        ];
        fields.extend(PrimitivePlacement::hints());
        fields.push(body_hint());
        manifest("Cone", Category::Create, "cone", fields)
    }

    fn dependencies(params: &ConeParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_body_written(&params.body, "body");
        dependencies.add_expression("bottom_radius", &params.bottom_radius, FieldUnit::Length);
        dependencies.add_expression("top_radius", &params.top_radius, FieldUnit::Length);
        dependencies.add_expression("height", &params.height, FieldUnit::Length);
        params.placement().add_dependencies(&mut dependencies);
        dependencies
    }

    fn build(
        params: &ConeParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let bottom = radius(context, "bottom_radius", &params.bottom_radius)?;
        let top = radius(context, "top_radius", &params.top_radius)?;
        if bottom == 0.0 && top == 0.0 {
            return Err(FeatureError::ConeRadiiZero);
        }
        let height = positive_length(context, "height", &params.height)?;
        let frame = params.placement().frame(context)?.to_kernel();
        let primitive = if (bottom - top).abs() <= EQUAL_RADII_TOLERANCE {
            make_cylinder(&frame, bottom, height)
        } else {
            make_cone(&frame, bottom, top, height)
        }
        .map_err(kernel)?;
        let tool = primitive_tool(context.feature(), primitive)?;
        apply_tool(context, &params.body, params.op, tool, Vec::new())
    }

    fn python(params: &ConeParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let sizes = [&params.bottom_radius, &params.top_radius, &params.height]
            .into_iter()
            .map(|expr| PythonValue::expression(expr, names))
            .collect();
        let mut keywords = params.placement().python_keywords(names)?;
        keywords.extend(op_keyword(names, &params.body, params.op));
        Some(call("cone", sizes, keywords))
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use partsmith_occt_sys::GeometryKind;
    use serde_json::json;

    use super::*;
    use crate::test_support::{ScriptNames, run};

    fn cone(bottom: &str, top: &str, height: &str) -> serde_json::Value {
        json!({"id": "f-cone1", "type": "cone", "body": "b-1", "bottom_radius": bottom, "top_radius": top, "height": height, "op": "new"})
    }

    #[test]
    fn a_frustum_has_three_faces() {
        let run = run(&["b-1"], &[cone("10", "5", "10")]);

        run.assert_ok("f-cone1");
        run.assert_volume("b-1", 1832.595715);
        run.assert_faces("b-1", 3);
        run.assert_bounding_box("b-1", [-10.0, -10.0, 0.0], [10.0, 10.0, 10.0]);
        run.assert_face_area("b-1", "f-cone1/side", 526.861105);
        run.assert_face_area("b-1", "f-cone1/bottom", 100.0 * PI);
        run.assert_face_area("b-1", "f-cone1/top", 25.0 * PI);
    }

    #[test]
    fn a_pointed_cone_has_no_top() {
        let run = run(&["b-1"], &[cone("10", "0", "10")]);

        run.assert_volume("b-1", 1047.197551);
        assert_eq!(run.face_names("b-1"), ["f-cone1/bottom", "f-cone1/side"]);
    }

    #[test]
    fn an_inverted_cone_has_no_bottom() {
        let run = run(&["b-1"], &[cone("0", "5", "10")]);

        run.assert_volume("b-1", 261.799388);
        assert_eq!(run.face_names("b-1"), ["f-cone1/side", "f-cone1/top"]);
        run.assert_bounding_box("b-1", [-5.0, -5.0, 0.0], [5.0, 5.0, 10.0]);
    }

    #[test]
    fn equal_radii_make_a_cylinder() {
        let run = run(&["b-1"], &[cone("5", "5", "10")]);

        run.assert_volume("b-1", 250.0 * PI);
        run.assert_faces("b-1", 3);
        assert_eq!(run.face("b-1", "f-cone1/side").kind, GeometryKind::Cylinder);
    }

    #[test]
    fn both_radii_zero_is_an_error() {
        let run = run(&["b-1"], &[cone("0", "0", "10")]);

        assert_eq!(
            run.messages("f-cone1"),
            ["Cone bottom and top radius cannot both be 0. Give one of them a size."]
        );
    }

    #[test]
    fn a_negative_radius_is_out_of_range() {
        let run = run(&["b-1"], &[cone("10", "-1", "10")]);

        assert_eq!(
            run.messages("f-cone1"),
            ["Top radius must be at least 0 mm, but it is -1 mm. Use 0 mm or more."]
        );
    }

    #[test]
    fn a_zero_height_is_out_of_range() {
        let run = run(&["b-1"], &[cone("10", "5", "0")]);

        assert_eq!(
            run.error("f-cone1")
                .field()
                .map(|field| field.path.as_str()),
            Some("height")
        );
    }

    #[test]
    fn python_writes_the_three_sizes() {
        let params: ConeParams = serde_json::from_value(json!({"body": "b-1", "bottom_radius": "10", "top_radius": "5", "height": "10", "op": "new"})).expect("valid params");

        let python = Cone::python(&params, &ScriptNames::default()).expect("a dedicated form");

        assert_eq!(python.render(0), "cone(10, 5, 10)");
    }
}
