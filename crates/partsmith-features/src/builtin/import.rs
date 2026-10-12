//! The `import` feature: puts the solids of a STEP asset into a body (docs/spec/features/import.md).

use partsmith_model::{Category, FeatureManifest, Id};
use partsmith_naming::{FaceSource, roles};
use partsmith_occt_sys::{Shape, SubShapeKind, compound, read_step};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{apply_tool, body_hint, body_op_hint, essential, face_name, hint, manifest};
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::{FeatureError, FeatureWarning};
use crate::feature::Feature;
use crate::naming::{kernel, name_shape};
use crate::output::{FeatureOutput, Tool};
use crate::params::BodyOp;

/// The extension of a STEP asset name.
const STEP_EXTENSION: &str = ".step";

/// The number of hex digits of a BLAKE3 hash, which names an asset.
const HASH_DIGITS: usize = 64;

/// Puts the solids of a STEP asset into a body.
#[derive(Clone, Copy, Debug, Default)]
pub struct Import;

/// Params of an import feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ImportParams {
    /// Target body.
    pub body: Id,
    /// The STEP asset: the lowercase hex BLAKE3 hash of the file's bytes followed by ".step"
    /// (engine.md section 3.14).
    pub asset: String,
    /// How the imported solids combine with the body. Required.
    pub op: BodyOp,
}

impl Feature for Import {
    const TYPE: &'static str = "import";
    type Params = ImportParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Import",
            Category::Create,
            "import",
            vec![
                essential(hint("asset", "File")),
                body_op_hint(),
                body_hint(),
            ],
        )
    }

    /// The asset names a STEP file of the assets folder: `^[0-9a-f]{64}\.step$` (import.md
    /// section 3.1).
    fn validate(params: &ImportParams) -> Result<(), FeatureError> {
        if is_step_asset(&params.asset) {
            Ok(())
        } else {
            Err(FeatureError::InvalidParams {
                detail:
                    "'asset' must name a file in the assets folder, such as <64 hex digits>.step"
                        .to_string(),
            })
        }
    }

    /// Writes the body and reads the asset (import.md section 3.2).
    fn dependencies(params: &ImportParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_body_written(&params.body, "body");
        dependencies.add_asset(&params.asset, "asset");
        dependencies
    }

    /// Reads the STEP bytes, groups every solid of the file into one tool whose faces are all
    /// `<id>/face`, numbered by naming.md's geometric order, and combines it with the body by
    /// `op`. The kernel's notes about skipped or repaired content come first as warnings
    /// (import.md section 3.3).
    fn build(
        params: &ImportParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let bytes = context.asset("asset", &params.asset)?;
        let imported = read_step(bytes).map_err(kernel)?;
        let warnings = imported
            .warnings
            .into_iter()
            .map(|message| FeatureWarning::ImportWarning { message })
            .collect();
        let solids: Vec<Shape> = imported.bodies.into_iter().map(|body| body.shape).collect();
        let tool = imported_tool(context, solids)?;
        apply_tool(context, &params.body, params.op, tool, warnings)
    }
}

/// True for a STEP asset name: 64 lowercase hex digits followed by `.step`.
fn is_step_asset(asset: &str) -> bool {
    asset.strip_suffix(STEP_EXTENSION).is_some_and(|hash| {
        hash.len() == HASH_DIGITS
            && hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

/// The tool of the imported solids: the only solid as it is, several grouped in read order, with
/// every face named by the fallback rule (naming.md section 3.6.14).
fn imported_tool(context: &BuildContext<'_>, mut solids: Vec<Shape>) -> Result<Tool, FeatureError> {
    let shape = match solids.len() {
        0 => {
            return Err(FeatureError::internal(
                "the kernel read a STEP file without solids and without an error",
            ));
        }
        1 => solids.remove(0),
        _ => compound(&solids).map_err(kernel)?.shape,
    };
    let name = face_name(context.feature(), roles::FACE, None)?;
    let faces = vec![FaceSource::New(name); shape.count(SubShapeKind::Face) as usize];
    let naming = name_shape(&shape, faces, &[])?;
    Ok(Tool { shape, naming })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use partsmith_model::FeatureRecord;
    use partsmith_occt_sys::{Plane, Point, StepBody, make_box, write_step};
    use serde_json::json;

    use super::*;
    use crate::builtin::copy::testing::{
        assert_bounds, assert_volume, fixture, params, python, update, updated_shape,
    };
    use crate::output::BodyChange;
    use crate::registry::Registry;
    use crate::test_support::{box_body, face_names};

    fn asset() -> String {
        format!("{}.step", "0".repeat(64))
    }

    fn import(op: &str) -> ImportParams {
        params::<Import>(json!({
            "id": "f-imp1", "type": "import", "body": "b-1", "asset": asset(), "op": op
        }))
    }

    /// STEP bytes of boxes of these corners and sizes.
    fn step(boxes: &[([f64; 3], [f64; 3])]) -> Arc<[u8]> {
        let bodies: Vec<StepBody> = boxes
            .iter()
            .map(|(corner, [length, width, height])| {
                let placement = Plane {
                    origin: Point::new(corner[0], corner[1], corner[2]),
                    ..Plane::XY
                };
                StepBody {
                    shape: make_box(&placement, *length, *width, *height)
                        .expect("the box builds")
                        .shape,
                    name: "Block".to_string(),
                    color: None,
                }
            })
            .collect();
        Arc::from(write_step(&bodies).expect("the kernel writes STEP"))
    }

    /// Builds the import with these asset bytes and `b-1` in `state`.
    fn run(
        params: &ImportParams,
        bytes: Arc<[u8]>,
        state: Option<crate::context::BodyState>,
    ) -> Result<FeatureOutput, FeatureError> {
        let bodies = state.into_iter().map(|state| ("b-1", state)).collect();
        let mut fixture = fixture::<Import>("f-imp1", params, bodies, Vec::new());
        fixture.assets.insert(asset(), bytes);
        Import::build(params, &fixture.context())
    }

    #[test]
    fn registers_with_its_manifest() {
        let mut registry = Registry::new();

        registry
            .register::<Import>()
            .expect("the import passes the registration checks");

        assert_eq!(registry.types().collect::<Vec<_>>(), ["import"]);
    }

    #[test]
    fn asset_names_are_hashes_with_the_step_extension() {
        assert!(is_step_asset(&asset()));
        assert!(is_step_asset(&format!(
            "{}.step",
            "0123456789abcdef".repeat(4)
        )));
        assert!(!is_step_asset("box.step"));
        assert!(!is_step_asset(&format!("{}.step", "A".repeat(64))));
        assert!(!is_step_asset(&format!("{}.stp", "0".repeat(64))));
        assert!(!is_step_asset(&format!("{}.step", "0".repeat(63))));
    }

    #[test]
    fn an_invalid_asset_name_is_invalid_params() {
        let record: FeatureRecord = serde_json::from_value(json!({
            "id": "f-imp1", "type": "import", "body": "b-1", "asset": "box.step", "op": "new"
        }))
        .expect("well-formed");
        let mut registry = Registry::new();
        registry.register::<Import>().expect("registers");

        let error = registry
            .validate_and_canonicalize(&record)
            .expect_err("box.step is not an asset name");

        assert_eq!(
            error,
            FeatureError::InvalidParams {
                detail:
                    "'asset' must name a file in the assets folder, such as <64 hex digits>.step"
                        .to_string()
            }
        );
    }

    #[test]
    fn dependencies_write_the_body_and_read_the_asset() {
        let dependencies = Import::dependencies(&import("new"));

        assert_eq!(dependencies.bodies_written[0].field, "body");
        assert_eq!(dependencies.assets[0].name, asset());
        assert_eq!(dependencies.assets[0].field, "asset");
        assert!(dependencies.expressions.is_empty());
    }

    #[test]
    fn faces_are_numbered_in_geometric_order() {
        let output = run(
            &import("new"),
            step(&[([0.0, 0.0, 0.0], [10.0, 20.0, 30.0])]),
            None,
        )
        .expect("the import builds");

        let shape = updated_shape(&output, "b-1");
        assert_volume(shape, 6000.0);
        assert_bounds(shape, [0.0, 0.0, 0.0], [10.0, 20.0, 30.0]);
        let naming = &update(&output, "b-1").naming;
        assert_eq!(
            face_names(naming),
            (1..=6)
                .map(|number| format!("f-imp1/face#{number}"))
                .collect::<Vec<_>>()
        );
        let top = partsmith_naming::ElementName::parse("f-imp1/face#4").expect("valid name");
        let index = naming.index(&top).expect("face #4 exists");
        let geometry = shape
            .element(SubShapeKind::Face, index)
            .expect("the face has geometry");
        assert!(
            (geometry.centroid.z - 30.0).abs() < 1e-9,
            "face #4 is the top"
        );
        assert_eq!(update(&output, "b-1").change, BodyChange::New);
    }

    #[test]
    fn several_solids_become_lumps_of_one_body() {
        let output = run(
            &import("new"),
            step(&[
                ([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
                ([50.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
            ]),
            None,
        )
        .expect("the import builds");

        let shape = updated_shape(&output, "b-1");
        assert_volume(shape, 2000.0);
        assert_eq!(shape.count(SubShapeKind::Solid), 2);
        assert_eq!(shape.count(SubShapeKind::Face), 12);
    }

    #[test]
    fn join_combines_with_the_body() {
        let output = run(
            &import("join"),
            step(&[([0.0, 0.0, 0.0], [10.0, 20.0, 30.0])]),
            Some(box_body("f-box1", [-5.0, -5.0, 0.0], 10.0)),
        )
        .expect("the import builds");

        assert_volume(updated_shape(&output, "b-1"), 6750.0);
        assert_eq!(update(&output, "b-1").change, BodyChange::Join);
    }

    #[test]
    fn bytes_that_are_not_step_are_a_kernel_error() {
        let error = run(
            &import("new"),
            Arc::from(b"not a step file".as_slice()),
            None,
        )
        .expect_err("the bytes are not STEP");

        assert_eq!(error.to_string(), "The file is not a valid STEP file.");
    }

    #[test]
    fn import_has_no_dedicated_python_form() {
        assert_eq!(python::<Import>(&import("new")), None);
    }
}
