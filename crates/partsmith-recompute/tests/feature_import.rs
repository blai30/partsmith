//! Acceptance tests of the `import` feature (docs/spec/features/import.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/import/`; the asset bytes are written with the kernel at test time, as the spec's
//! fixtures define them.

use crate::feature_support;

use std::sync::Arc;

use feature_support::import::{assets, block, block_bytes};
use feature_support::{
    Run, assert_bounding_box, assert_empty, assert_examples_canonical, assert_face_count,
    assert_face_kind, assert_face_names, assert_face_position, assert_solid_count, assert_volume,
    body_changes, face, load_example, step_asset_name,
};
use partsmith_features::FeatureError;
use partsmith_occt_sys::{GeometryKind, Plane, Point, write_step};

/// The fixture `TWO`: a STEP file with two 10 mm cubes named `A` and `B`, the second moved to x = 50.
fn two_cubes_bytes() -> Arc<[u8]> {
    let moved = Plane {
        origin: Point::new(50.0, 0.0, 0.0),
        ..Plane::XY
    };
    let bodies = [
        block("A", &Plane::XY, [10.0, 10.0, 10.0]),
        block("B", &moved, [10.0, 10.0, 10.0]),
    ];
    Arc::from(write_step(&bodies).expect("the kernel writes STEP"))
}

/// Runs an example with the asset map `{A: bytes}`.
fn run_example(relative_path: &str, bytes: Arc<[u8]>) -> Run {
    Run::with_assets(load_example(relative_path), &assets(bytes))
}

/// Asserts the result of test 1: the imported block alone in b-1.
fn assert_imported_block(run: &Run) {
    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 6000.0);
    assert_face_count(shape, 6);
    assert_bounding_box(shape, [0.0, 0.0, 0.0], [10.0, 20.0, 30.0]);
}

#[test]
fn examples_are_canonical() {
    assert_examples_canonical("import");
}

#[test]
fn t001_op_new_volume_faces_and_bounds() {
    let run = run_example("import/new-body.ptsm", block_bytes());

    assert_imported_block(&run);
}

#[test]
fn t001_op_new_names_faces_in_geometric_order() {
    let run = run_example("import/new-body.ptsm", block_bytes());

    let body = run.body("b-1");
    assert_face_names(
        body,
        &[
            "f-imp1/face#1",
            "f-imp1/face#2",
            "f-imp1/face#3",
            "f-imp1/face#4",
            "f-imp1/face#5",
            "f-imp1/face#6",
        ],
    );
    // By centroid the order is left, front, bottom, top, back, right: each face lies in the plane
    // `axis = position`.
    let planes = [
        (0, 0.0),
        (1, 0.0),
        (2, 0.0),
        (2, 30.0),
        (1, 20.0),
        (0, 10.0),
    ];
    for (number, (axis, position)) in planes.into_iter().enumerate() {
        let geometry = face(body, &format!("f-imp1/face#{}", number + 1));
        assert_face_kind(&geometry, GeometryKind::Plane);
        assert_face_position(&geometry, axis, position);
    }
}

#[test]
fn t001_op_new_starts_the_body() {
    let run = run_example("import/new-body.ptsm", block_bytes());

    assert_eq!(body_changes(&run, "f-imp1"), ["b-1 New"]);
}

#[test]
fn t001_op_new_dependencies() {
    let run = run_example("import/new-body.ptsm", block_bytes());

    let dependencies = run.dependencies("f-imp1");
    let name = step_asset_name();
    let assets: Vec<(&str, &str)> = dependencies
        .assets
        .iter()
        .map(|asset| (asset.name.as_str(), asset.field.as_str()))
        .collect();
    assert_eq!(assets, [(name.as_str(), "asset")]);
    let written: Vec<(&str, &str)> = dependencies
        .bodies_written
        .iter()
        .map(|link| (link.id.as_str(), link.field.as_str()))
        .collect();
    assert_eq!(written, [("b-1", "body")]);
    assert!(
        dependencies.expressions.is_empty(),
        "import declares no expressions: {:?}",
        dependencies.expressions,
    );
}

#[test]
fn t002_join() {
    let run = run_example("import/join.ptsm", block_bytes());

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 6750.0);
    assert_bounding_box(shape, [-5.0, -5.0, 0.0], [10.0, 20.0, 30.0]);
    assert_eq!(body_changes(&run, "f-imp1"), ["b-1 Join"]);
}

#[test]
fn t003_missing_asset() {
    let run = Run::example("import/new-body.ptsm");

    run.assert_error("f-imp1", "asset_missing");
    let message = &run.feature("f-imp1").messages[0];
    assert_eq!(message.pointer.as_deref(), Some("/asset"));
    assert_eq!(
        run.messages("f-imp1"),
        [
            "The STEP file of this import is not in this document's assets folder. Copy the assets folder that came with the document next to it, or import the file again."
        ]
    );
    let detail = message
        .detail
        .as_deref()
        .expect("the asset name is in the detail");
    assert!(
        detail.contains(
            "assets/0000000000000000000000000000000000000000000000000000000000000000.step"
        ),
        "detail: {detail}"
    );
    assert_empty(&run, "b-1");
}

#[test]
fn t003_missing_asset_builds_once_supplied() {
    let mut run = Run::example("import/new-body.ptsm");
    run.assert_error("f-imp1", "asset_missing");

    run.rerun(&assets(block_bytes()));

    assert!(
        !run.feature("f-imp1").timing.from_cache,
        "the import is built once its asset is supplied, not taken from the cache",
    );
    assert_imported_block(&run);
}

#[test]
fn t004_not_a_step_file() {
    let run = run_example(
        "import/new-body.ptsm",
        Arc::from(b"not a step file".as_slice()),
    );

    run.assert_error("f-imp1", "kernel");
    assert_eq!(
        run.messages("f-imp1"),
        [
            "Partsmith's geometry kernel could not build this import. The file is not a valid STEP file."
        ]
    );
    let detail = run.feature("f-imp1").messages[0]
        .detail
        .as_deref()
        .expect("the kernel's technical text is in the detail");
    assert!(
        detail.starts_with("read_step read_failed"),
        "detail: {detail}"
    );
    assert_empty(&run, "b-1");
}

#[test]
fn t005_invalid_asset_name() {
    let run = Run::records(
        &["b-1"],
        &[r#"{"id": "f-imp1", "type": "import", "body": "b-1", "asset": "box.step", "op": "new"}"#],
    );

    let error = run
        .recompute
        .registry()
        .validate_and_canonicalize(run.record("f-imp1"))
        .expect_err("box.step is not an asset name");
    let FeatureError::InvalidParams { detail, .. } = &error else {
        panic!("expected InvalidParams, got {error:?}");
    };
    assert_eq!(
        detail,
        "'asset' must name a file in the assets folder, such as <64 hex digits>.step"
    );
    run.assert_error("f-imp1", "invalid_params");
    assert_eq!(
        run.messages("f-imp1"),
        ["File of this import cannot be read, so it was not built. Set File again."]
    );
}

#[test]
fn t006_two_solids() {
    let run = run_example("import/new-body.ptsm", two_cubes_bytes());

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 2000.0);
    assert_face_count(shape, 12);
    assert_solid_count(shape, 2);
    assert_bounding_box(shape, [0.0, 0.0, 0.0], [60.0, 10.0, 10.0]);
}

#[test]
fn t007_cut_volume_faces_and_bounds() {
    let run = run_example("import/cut-through-plate.ptsm", block_bytes());

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 14000.0);
    assert_face_count(shape, 10);
    assert_bounding_box(shape, [-20.0, -15.0, 0.0], [20.0, 25.0, 10.0]);
    assert_eq!(body_changes(&run, "f-imp1"), ["b-1 Cut"]);
}

#[test]
fn t007_cut_hole_walls_keep_their_imported_names() {
    let run = run_example("import/cut-through-plate.ptsm", block_bytes());

    let walls = [
        ("f-imp1/face#1", 0, 0.0),
        ("f-imp1/face#2", 1, 0.0),
        ("f-imp1/face#5", 1, 20.0),
        ("f-imp1/face#6", 0, 10.0),
    ];
    for (name, axis, position) in walls {
        let wall = face(run.body("b-1"), name);
        assert_face_kind(&wall, GeometryKind::Plane);
        assert_face_position(&wall, axis, position);
    }
}
