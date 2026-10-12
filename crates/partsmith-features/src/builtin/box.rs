//! The `box` feature: a rectangular block standing on a plane, centered on the placement origin in
//! x and y (features/box.md).

use partsmith_model::{Category, Expr, FeatureManifest, FieldUnit, Id};
use partsmith_occt_sys::make_box;
use partsmith_sketch::Placement;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    apply_tool, body_hint, body_op_hint, call, essential, manifest, op_keyword, positive_length,
    primitive_tool, unit_hint,
};
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::naming::kernel;
use crate::output::FeatureOutput;
use crate::params::{BodyOp, PlacementExt, PlaneRef};
use crate::primitive::{PrimitivePlacement, default_position, is_default_position};
use crate::python::{PythonCall, PythonNames, PythonValue};

/// Field paths of the three sizes.
const SIZE_FIELDS: [&str; 3] = ["size[0]", "size[1]", "size[2]"];

/// A rectangular block. The type name in files is `box`; `Box` is the standard library's pointer.
#[derive(Clone, Copy, Debug)]
pub struct BoxFeature;

/// Params of a box feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BoxParams {
    /// Target body.
    pub body: Id,
    /// Size along the placement x, y and z axes. Lengths, greater than 0.
    pub size: [Expr; 3],
    /// Placement plane. Default origin XY, omitted when default.
    #[serde(default, skip_serializing_if = "PlaneRef::is_default")]
    pub plane: PlaneRef,
    /// Placement offset. Default ["0", "0", "0"], omitted when default.
    #[serde(
        default = "default_position",
        skip_serializing_if = "is_default_position"
    )]
    pub position: [Expr; 3],
    /// How the box combines with the body. Required.
    pub op: BodyOp,
}

impl BoxParams {
    /// The shared placement params.
    fn placement(&self) -> PrimitivePlacement<'_> {
        PrimitivePlacement {
            plane: &self.plane,
            position: &self.position,
        }
    }
}

impl Feature for BoxFeature {
    const TYPE: &'static str = "box";
    type Params = BoxParams;

    fn manifest() -> FeatureManifest {
        let mut fields: Vec<_> = SIZE_FIELDS
            .iter()
            .zip(["Length", "Width", "Height"])
            .map(|(path, label)| essential(unit_hint(path, label, FieldUnit::Length)))
            .collect();
        fields.push(body_op_hint());
        fields.extend(PrimitivePlacement::hints());
        fields.push(body_hint());
        manifest("Box", Category::Create, "box", fields)
    }

    fn dependencies(params: &BoxParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_body_written(&params.body, "body");
        for (field, expr) in SIZE_FIELDS.iter().zip(&params.size) {
            dependencies.add_expression(field, expr, FieldUnit::Length);
        }
        params.placement().add_dependencies(&mut dependencies);
        dependencies
    }

    fn build(
        params: &BoxParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let mut size = [0.0; 3];
        for ((value, field), expr) in size.iter_mut().zip(SIZE_FIELDS).zip(&params.size) {
            *value = positive_length(context, field, expr)?;
        }
        let [length, width, height] = size;
        let frame = params.placement().frame(context)?;
        // The kernel's box starts at its placement origin; center it in x and y.
        let corner = Placement {
            origin: frame.local_to_world([-length / 2.0, -width / 2.0, 0.0]),
            ..frame
        };
        let primitive = make_box(&corner.to_kernel(), length, width, height).map_err(kernel)?;
        let tool = primitive_tool(context.feature(), primitive)?;
        apply_tool(context, &params.body, params.op, tool, Vec::new())
    }

    fn python(params: &BoxParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let sizes = params
            .size
            .iter()
            .map(|expr| PythonValue::expression(expr, names))
            .collect();
        let mut keywords = params.placement().python_keywords(names)?;
        keywords.extend(op_keyword(names, &params.body, params.op));
        Some(call("box", sizes, keywords))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::{ScriptNames, assert_point, run};

    fn block(extra: serde_json::Value) -> serde_json::Value {
        let mut record = json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "20", "30"], "op": "new"});
        if let (Some(record), Some(extra)) = (record.as_object_mut(), extra.as_object()) {
            record.extend(extra.clone());
        }
        record
    }

    #[test]
    fn a_default_box_is_centered_in_x_and_y() {
        let run = run(&["b-1"], &[block(json!({}))]);

        run.assert_ok("f-box1");
        run.assert_volume("b-1", 6000.0);
        run.assert_faces("b-1", 6);
        run.assert_bounding_box("b-1", [-5.0, -10.0, 0.0], [5.0, 10.0, 30.0]);
        run.assert_face_area("b-1", "f-box1/bottom", 200.0);
        run.assert_face_area("b-1", "f-box1/front", 300.0);
        run.assert_face_area("b-1", "f-box1/left", 600.0);
        assert_point(
            "top",
            run.face("b-1", "f-box1/top").centroid,
            [0.0, 0.0, 30.0],
        );
    }

    #[test]
    fn the_position_offsets_the_box() {
        let run = run(&["b-1"], &[block(json!({"position": ["1", "2", "3"]}))]);

        run.assert_bounding_box("b-1", [-4.0, -8.0, 3.0], [6.0, 12.0, 33.0]);
    }

    #[test]
    fn a_box_on_xz_stands_along_minus_y() {
        let run = run(
            &["b-1"],
            &[block(json!({"plane": {"kind": "origin", "plane": "XZ"}}))],
        );

        run.assert_bounding_box("b-1", [-5.0, -30.0, -10.0], [5.0, 0.0, 10.0]);
        assert_point(
            "top",
            run.face("b-1", "f-box1/top").centroid,
            [0.0, -30.0, 0.0],
        );
        assert_point(
            "front",
            run.face("b-1", "f-box1/front").centroid,
            [0.0, -15.0, -10.0],
        );
    }

    #[test]
    fn a_box_on_another_body_reads_it() {
        let run = run(
            &["b-1", "b-2"],
            &[
                block(json!({})),
                json!({"id": "f-box2", "type": "box", "body": "b-2", "size": ["4", "4", "4"], "plane": {"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,30|a=200"}}, "op": "new"}),
            ],
        );

        run.assert_bounding_box("b-2", [-2.0, -2.0, 30.0], [2.0, 2.0, 34.0]);
        let dependencies = run
            .registry
            .dependencies(&run.document.features[1])
            .expect("valid params");
        assert_eq!(dependencies.bodies_read[0].id.as_str(), "b-1");
        assert_eq!(dependencies.bodies_written[0].id.as_str(), "b-2");
    }

    #[test]
    fn a_zero_size_is_out_of_range() {
        let run = run(&["b-1"], &[block(json!({"size": ["0", "20", "30"]}))]);

        assert_eq!(
            run.messages("f-box1"),
            ["Length must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."]
        );
        assert_eq!(
            run.error("f-box1").field().map(|field| field.path.as_str()),
            Some("size[0]")
        );
    }

    #[test]
    fn overlapping_boxes_join_into_six_faces() {
        let run = run(
            &["b-1"],
            &[
                json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}),
                json!({"id": "f-box2", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "position": ["5", "0", "0"], "op": "join"}),
            ],
        );

        run.assert_volume("b-1", 1500.0);
        run.assert_faces("b-1", 6);
        run.assert_bounding_box("b-1", [-5.0, -5.0, 0.0], [10.0, 5.0, 10.0]);
    }

    #[test]
    fn a_square_hole_cuts_through() {
        let run = run(
            &["b-1"],
            &[
                json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "op": "new"}),
                json!({"id": "f-box2", "type": "box", "body": "b-1", "size": ["10", "10", "20"], "position": ["0", "0", "-5"], "op": "cut"}),
            ],
        );

        run.assert_volume("b-1", 15000.0);
        run.assert_faces("b-1", 10);
    }

    #[test]
    fn python_writes_sizes_then_placement_then_op() {
        let names = ScriptNames::default();
        let params: BoxParams = serde_json::from_value(json!({"body": "b-1", "size": ["10", "10", "5 mm"], "plane": {"kind": "datum", "feature": "f-dp1"}, "position": ["20", "0", "0"], "op": "cut"})).expect("valid params");

        let python = BoxFeature::python(&params, &names).expect("a dedicated form");

        assert_eq!(
            python.render(0),
            r#"box(10, 10, 5*mm, plane=plane_1, position=(20, 0, 0), op="cut")"#
        );
    }
}
