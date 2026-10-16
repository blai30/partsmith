//! The `mirror` feature: reflects a body across a plane and fuses the image into the source, or
//! puts it into another, empty body (docs/spec/features/mirror.md).

use partsmith_model::{Category, FeatureManifest, Id, SelectionKind};
use partsmith_naming::Instance;
use partsmith_occt_sys::Transform;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{essential, into_keyword, manifest, selection_hint, target_hint};
use super::copy::{self, Source};
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::output::FeatureOutput;
use crate::params::{CopyOp, PlacementExt, PlaneRef};
use crate::python::{PythonCall, PythonNames, PythonValue};

/// The instance number of the mirror image (naming.md section 3.6.10).
const IMAGE: Instance = Instance {
    first: 1,
    second: 0,
};

/// Reflects a body across a plane.
#[derive(Clone, Copy, Debug, Default)]
pub struct Mirror;

/// Params of a mirror feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MirrorParams {
    /// The body to mirror.
    pub body: Id,
    /// The mirror plane.
    pub plane: PlaneRef,
    /// Where the image goes. Default join, omitted when join.
    #[serde(default, skip_serializing_if = "CopyOp::is_default")]
    pub op: CopyOp,
    /// The empty body that receives the image. Required with op new, not allowed with op join.
    /// Omitted when None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Id>,
}

impl Feature for Mirror {
    const TYPE: &'static str = "mirror";
    type Params = MirrorParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Mirror",
            Category::Pattern,
            "mirror",
            vec![
                essential(selection_hint("body", "Body", SelectionKind::Body)),
                essential(selection_hint(
                    "plane",
                    "Mirror plane",
                    SelectionKind::Plane,
                )),
                essential(copy::op_hint()),
                essential(target_hint()),
            ],
        )
    }

    fn validate(params: &MirrorParams) -> Result<(), FeatureError> {
        copy::validate_target(
            params.op,
            &params.body,
            params.target.as_ref(),
            &Mirror::manifest(),
        )
    }

    /// The source body (written with op join, read with op new), what the plane reads, then the
    /// target of op new (mirror.md section 3.2).
    fn dependencies(params: &MirrorParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        copy::add_source(&mut dependencies, params.op, &params.body);
        dependencies.add_plane("plane", &params.plane);
        copy::add_target(&mut dependencies, params.op, params.target.as_ref());
        dependencies
    }

    /// The source reflected through the plane, resolved before this feature so a face of the
    /// source may be the plane, then joined with the source or put into the target (mirror.md
    /// section 3.3).
    fn build(
        params: &MirrorParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let source = Source::new(context, &params.body)?;
        let plane = context.plane("plane", &params.plane)?;
        let image = source.copy(context, &Transform::mirror(&plane.value.to_kernel()), IMAGE)?;
        source.place(
            context,
            params.op,
            params.target.as_ref(),
            vec![image],
            plane.warnings,
        )
    }

    /// `mirror(plane, into=<target part>)`: `into` only with op new (mirror.md section 2). None
    /// when the plane or the target has no variable.
    fn python(params: &MirrorParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let mut call = PythonCall::new("mirror");
        call.positional
            .push(PythonValue::plane(&params.plane, names)?);
        call.keywords = into_keyword(names, params.op, params.target.as_ref())?;
        Some(call)
    }
}

#[cfg(test)]
mod tests {
    use partsmith_naming::ElementKind;
    use partsmith_occt_sys::{GeometryKind, SubShapeKind};
    use serde_json::{Value, json};

    use super::*;
    use crate::context::BodyState;
    use crate::output::{BodyChange, DatumResult, OutputData};
    use crate::test_support::{
        assert_bounds, assert_volume, box_body, build, face_names, id, params_with, python, update,
        updated_shape, updates,
    };

    const YZ: &str = r#"{"kind": "origin", "plane": "YZ"}"#;

    fn mirror(plane: &str, extra: Value) -> MirrorParams {
        let plane: Value = serde_json::from_str(plane).expect("valid plane JSON");
        params_with::<Mirror>(
            json!({"id": "f-mir1", "type": "mirror", "body": "b-1", "plane": plane}),
            extra,
        )
    }

    /// A 10 mm cube in `b-1` spanning [x,-5,0]-[x+10,5,10].
    fn cube_at(x: f64) -> Vec<(&'static str, BodyState)> {
        vec![("b-1", box_body("f-box1", [x, -5.0, 0.0], 10.0))]
    }

    #[test]
    fn canonical_params_omit_op_join() {
        let params = mirror(YZ, json!({"op": "join"}));

        assert_eq!(
            serde_json::to_string(&params).expect("serializes"),
            r#"{"body":"b-1","plane":{"kind":"origin","plane":"YZ"}}"#
        );
    }

    #[test]
    fn validation_follows_the_copy_rules() {
        let error = Mirror::validate(&mirror(YZ, json!({"op": "new"}))).expect_err("no target");

        assert_eq!(error.code(), "target_required");
        assert_eq!(error.field().expect("names a field").path, "target");
    }

    #[test]
    fn op_new_reads_the_source_and_writes_the_target() {
        let dependencies = Mirror::dependencies(&mirror(YZ, json!({"op": "new", "target": "b-2"})));

        assert_eq!(dependencies.bodies_read[0].id, id("b-1"));
        assert_eq!(dependencies.bodies_written[0].id, id("b-2"));
    }

    #[test]
    fn a_face_of_the_source_stays_written() {
        let plane = r#"{"kind": "face", "body": "b-1", "face": {"q": "f-box1/right", "fp": "plane|n=1,0,0"}}"#;

        let dependencies = Mirror::dependencies(&mirror(plane, json!({})));

        assert_eq!(dependencies.bodies_written.len(), 1);
        assert!(dependencies.bodies_read.is_empty());
    }

    #[test]
    fn a_datum_plane_is_a_feature_dependency() {
        let plane = r#"{"kind": "datum", "feature": "f-dp1"}"#;

        let dependencies = Mirror::dependencies(&mirror(plane, json!({})));

        assert_eq!(dependencies.features[0].id, id("f-dp1"));
        assert_eq!(dependencies.features[0].field, "plane.feature");
    }

    #[test]
    fn join_fuses_the_image_into_the_source() {
        let output = build::<Mirror>("f-mir1", &mirror(YZ, json!({})), cube_at(5.0), Vec::new())
            .expect("the mirror builds");

        let shape = updated_shape(&output, "b-1");
        assert_volume(shape, 2000.0);
        assert_bounds(shape, [-15.0, -5.0, 0.0], [15.0, 5.0, 10.0]);
        assert_eq!(update(&output, "b-1").change, BodyChange::Join);
    }

    #[test]
    fn the_image_is_named_as_instance_one_and_faces_outward() {
        let output = build::<Mirror>("f-mir1", &mirror(YZ, json!({})), cube_at(5.0), Vec::new())
            .expect("the mirror builds");

        let naming = &update(&output, "b-1").naming;
        assert!(face_names(naming).contains(&"f-box1/top@f-mir1:1".to_string()));
        let name = partsmith_naming::ElementName::parse("f-box1/right@f-mir1:1").expect("valid");
        let index = naming.index(&name).expect("the image has a right face");
        let geometry = updated_shape(&output, "b-1")
            .element(SubShapeKind::Face, index)
            .expect("the face has geometry");
        assert_eq!(geometry.kind, GeometryKind::Plane);
        let normal = geometry.direction.expect("a plane has a normal");
        assert!(
            (normal.x + 1.0).abs() < 1e-9,
            "outward normal is -X, got {normal:?}"
        );
        assert!((geometry.centroid.x + 15.0).abs() < 1e-9);
    }

    #[test]
    fn op_new_puts_the_image_into_the_target_only() {
        let params = mirror(YZ, json!({"op": "new", "target": "b-2"}));

        let output = build::<Mirror>("f-mir1", &params, cube_at(5.0), Vec::new())
            .expect("the mirror builds");

        assert_eq!(updates(&output).len(), 1);
        let target = update(&output, "b-2");
        assert_eq!(target.change, BodyChange::New);
        assert_eq!(target.naming.count(ElementKind::Face), 6);
        assert_bounds(
            updated_shape(&output, "b-2"),
            [-15.0, -5.0, 0.0],
            [-5.0, 5.0, 10.0],
        );
    }

    #[test]
    fn a_datum_plane_places_the_mirror() {
        let plane = r#"{"kind": "datum", "feature": "f-dp1"}"#;
        let datum = crate::output::FeatureOutput::new(
            OutputData::Datum(DatumResult::Plane(
                partsmith_sketch::OriginPlane::Yz.placement().offset(20.0),
            )),
            Vec::new(),
        );

        let output = build::<Mirror>(
            "f-mir1",
            &mirror(plane, json!({})),
            cube_at(5.0),
            vec![("f-dp1", datum)],
        )
        .expect("the mirror builds");

        assert_bounds(
            updated_shape(&output, "b-1"),
            [5.0, -5.0, 0.0],
            [35.0, 5.0, 10.0],
        );
    }

    #[test]
    fn a_symmetric_body_has_no_effect() {
        let output = build::<Mirror>("f-mir1", &mirror(YZ, json!({})), cube_at(-5.0), Vec::new())
            .expect("the mirror builds");

        assert_eq!(output.warnings[0].code(), "no_effect");
    }

    #[test]
    fn an_empty_source_has_nothing_to_mirror() {
        let error = build::<Mirror>("f-mir1", &mirror(YZ, json!({})), Vec::new(), Vec::new())
            .expect_err("b-1 is empty");

        assert_eq!(error.code(), "body_empty");
    }

    #[test]
    fn python_writes_the_plane_and_into_for_op_new() {
        assert_eq!(
            python::<Mirror>(&mirror(YZ, json!({}))).as_deref(),
            Some("mirror(Plane.YZ)")
        );
        assert_eq!(
            python::<Mirror>(&mirror(
                r#"{"kind": "datum", "feature": "f-dp1"}"#,
                json!({"op": "new", "target": "b-2"})
            ))
            .as_deref(),
            Some("mirror(plane_1, into=part_2)")
        );
    }

    #[test]
    fn python_needs_the_plane_variable() {
        assert_eq!(
            python::<Mirror>(&mirror(
                r#"{"kind": "datum", "feature": "f-dp9"}"#,
                json!({})
            )),
            None
        );
    }
}
