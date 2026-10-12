//! The `torus` feature: a ring torus centered on the placement origin (features/torus.md).

use partsmith_model::{Category, Expr, FeatureManifest, FieldUnit, Id};
use partsmith_occt_sys::make_torus;
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

/// A ring torus.
#[derive(Clone, Copy, Debug)]
pub struct Torus;

/// Params of a torus feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TorusParams {
    /// Target body.
    pub body: Id,
    /// Distance from the axis to the tube center. Length, greater than `minor_radius`.
    pub major_radius: Expr,
    /// Tube radius. Length, greater than 0.
    pub minor_radius: Expr,
    /// Placement plane. Default origin XY, omitted when default.
    #[serde(default, skip_serializing_if = "PlaneRef::is_default")]
    pub plane: PlaneRef,
    /// Placement offset (the center). Default ["0", "0", "0"], omitted when default.
    #[serde(
        default = "default_position",
        skip_serializing_if = "is_default_position"
    )]
    pub position: [Expr; 3],
    /// How the torus combines with the body. Required.
    pub op: BodyOp,
}

impl TorusParams {
    /// The shared placement params.
    fn placement(&self) -> PrimitivePlacement<'_> {
        PrimitivePlacement {
            plane: &self.plane,
            position: &self.position,
        }
    }
}

impl Feature for Torus {
    const TYPE: &'static str = "torus";
    type Params = TorusParams;

    fn manifest() -> FeatureManifest {
        let mut fields = vec![
            essential(unit_hint("major_radius", "Ring radius", FieldUnit::Length)),
            essential(unit_hint("minor_radius", "Tube radius", FieldUnit::Length)),
            body_op_hint(),
        ];
        fields.extend(PrimitivePlacement::hints());
        fields.push(body_hint());
        manifest("Torus", Category::Create, "torus", fields)
    }

    fn dependencies(params: &TorusParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_body_written(&params.body, "body");
        dependencies.add_expression("major_radius", &params.major_radius, FieldUnit::Length);
        dependencies.add_expression("minor_radius", &params.minor_radius, FieldUnit::Length);
        params.placement().add_dependencies(&mut dependencies);
        dependencies
    }

    fn build(
        params: &TorusParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let major = positive_length(context, "major_radius", &params.major_radius)?;
        let minor = positive_length(context, "minor_radius", &params.minor_radius)?;
        check_range(
            context,
            "minor_radius",
            FieldUnit::Length,
            minor,
            Bound::LessThan(major),
            Some("the ring radius"),
        )?;
        let frame = params.placement().frame(context)?;
        let primitive = make_torus(&frame.to_kernel(), major, minor).map_err(kernel)?;
        let tool = primitive_tool(context.feature(), primitive)?;
        apply_tool(context, &params.body, params.op, tool, Vec::new())
    }

    fn python(params: &TorusParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let sizes = vec![
            PythonValue::expression(&params.major_radius, names),
            PythonValue::expression(&params.minor_radius, names),
        ];
        let mut keywords = params.placement().python_keywords(names)?;
        keywords.extend(op_keyword(names, &params.body, params.op));
        Some(call("torus", sizes, keywords))
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use partsmith_occt_sys::GeometryKind;
    use serde_json::json;

    use super::*;
    use crate::error::Bound;
    use crate::test_support::{ScriptNames, run};

    fn torus(extra: serde_json::Value) -> serde_json::Value {
        let mut record = json!({"id": "f-tor1", "type": "torus", "body": "b-1", "major_radius": "20", "minor_radius": "5", "op": "new"});
        if let (Some(record), Some(extra)) = (record.as_object_mut(), extra.as_object()) {
            record.extend(extra.clone());
        }
        record
    }

    #[test]
    fn a_default_torus_lies_in_xy() {
        let run = run(&["b-1"], &[torus(json!({}))]);

        run.assert_ok("f-tor1");
        run.assert_volume("b-1", 2.0 * PI * PI * 20.0 * 25.0);
        run.assert_faces("b-1", 1);
        run.assert_bounding_box("b-1", [-25.0, -25.0, -5.0], [25.0, 25.0, 5.0]);
        assert_eq!(run.face("b-1", "f-tor1/surface").kind, GeometryKind::Torus);
        run.assert_face_area("b-1", "f-tor1/surface", 3947.841760);
    }

    #[test]
    fn a_torus_on_xz_has_its_axis_along_y() {
        let run = run(
            &["b-1"],
            &[torus(json!({"plane": {"kind": "origin", "plane": "XZ"}}))],
        );

        run.assert_bounding_box("b-1", [-25.0, -5.0, -25.0], [25.0, 5.0, 25.0]);
    }

    #[test]
    fn the_tube_must_be_smaller_than_the_ring() {
        let run = run(&["b-1"], &[torus(json!({"minor_radius": "20"}))]);

        assert_eq!(
            run.messages("f-tor1"),
            [
                "Tube radius must be less than the ring radius (20 mm), but it is 20 mm. Use a value less than 20 mm."
            ]
        );
    }

    #[test]
    fn a_zero_tube_is_out_of_range() {
        let run = run(&["b-1"], &[torus(json!({"minor_radius": "0"}))]);

        assert!(matches!(
            run.error("f-tor1"),
            FeatureError::OutOfRange { bound: Bound::GreaterThan(0.0), field, .. } if field.path == "minor_radius"
        ));
    }

    #[test]
    fn python_writes_both_radii() {
        let params: TorusParams = serde_json::from_value(
            json!({"body": "b-1", "major_radius": "20", "minor_radius": "5", "op": "join"}),
        )
        .expect("valid params");

        let python = Torus::python(&params, &ScriptNames::default()).expect("a dedicated form");

        assert_eq!(python.render(0), r#"torus(20, 5, op="join")"#);
    }
}
