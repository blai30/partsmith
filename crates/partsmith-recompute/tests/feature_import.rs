//! Acceptance tests of the `import` feature (docs/spec/features/import.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/import/`. The placeholder examples get asset bytes written with the kernel at test
//! time, as the spec's fixtures define them; the step-block examples (tests 9 and 11) get the
//! committed file `S` of `examples/import/assets/`, the bytes `partsmith rebuild` reads.

use crate::feature_support;

use std::sync::Arc;

use feature_support::import::{assets, block, block_bytes, step_bytes};
use feature_support::{
    Assets, Run, aliases, assert_bounding_box, assert_empty, assert_examples_canonical,
    assert_face_count, assert_face_in_plane, assert_face_names, assert_messages_pass_lint,
    assert_solid_count, assert_volume, asset_name, body_changes, examples_folder, face,
    load_example,
};
use partsmith_features::FeatureError;
use partsmith_occt_sys::{
    PlanarCurve, PlanarLoop, PlanarRegion, Plane, Point, Point2, Shape, ShapeKind, StepBody,
    SubShapeKind, TaggedCurve, compound, make_profile,
};

/// The fixture `S`: the name of the committed STEP file of the block in `examples/import/assets/`.
const COMMITTED_ASSET: &str =
    "b8d947e1aae9d107df35bea403414dc9ae3df630b84426c70e92770948bdfdb4.step";

/// The six face names of the imported block, `f-imp1/face#1` to `f-imp1/face#6` (test 1).
const BLOCK_FACES: [&str; 6] = [
    "f-imp1/face#1",
    "f-imp1/face#2",
    "f-imp1/face#3",
    "f-imp1/face#4",
    "f-imp1/face#5",
    "f-imp1/face#6",
];

/// The text of the kernel's one warning for the two faces of `SKIPPED` (test 10).
const SKIPPED_TWO_SHAPES: &str = "Skipped 2 shapes that are not closed solids.";

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
    step_bytes(&bodies)
}

/// A region bounded by a square with 10 mm sides whose lower left corner is at (`left`, 0) in plane
/// coordinates, its sides tagged 1 to 4.
fn square(left: f64) -> PlanarRegion {
    let right = left + 10.0;
    let side = |tag, start: (f64, f64), end: (f64, f64)| TaggedCurve {
        tag,
        curve: PlanarCurve::Line {
            start: Point2 {
                x: start.0,
                y: start.1,
            },
            end: Point2 { x: end.0, y: end.1 },
        },
    };
    PlanarRegion {
        outer: PlanarLoop {
            curves: vec![
                side(1, (left, 0.0), (right, 0.0)),
                side(2, (right, 0.0), (right, 10.0)),
                side(3, (right, 10.0), (left, 10.0)),
                side(4, (left, 10.0), (left, 0.0)),
            ],
        },
        holes: Vec::new(),
    }
}

/// The fixture `FACES`: the shape of `make_profile` of two squares with 10 mm sides on
/// `Plane::XY`, [50,60]x[0,10] and [70,80]x[0,10], a compound of two faces that bound no solid.
fn two_faces() -> Shape {
    make_profile(&Plane::XY, &[square(50.0), square(70.0)])
        .expect("the kernel makes the two squares")
        .shape
}

/// The fixture `SKIPPED`: one part named `Block` holding the block of `BYTES` and, beside it, the
/// two faces of `FACES`.
fn skipped_bytes() -> Arc<[u8]> {
    let solid = block("Block", &Plane::XY, [10.0, 20.0, 30.0]).shape;
    let shape = compound(&[solid, two_faces()])
        .expect("the kernel groups the block and the faces")
        .shape;
    step_bytes(&[StepBody {
        shape,
        name: "Block".to_owned(),
        color: None,
    }])
}

/// The bytes of the committed file `S` in `examples/import/assets/`.
fn committed_bytes() -> Arc<[u8]> {
    let path = examples_folder()
        .join("import/assets")
        .join(COMMITTED_ASSET);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("cannot read the asset {}: {error}", path.display()));
    Arc::from(bytes)
}

/// Runs an example with the asset map `{A: bytes}`.
fn run_example(relative_path: &str, bytes: Arc<[u8]>) -> Run {
    Run::with_assets(load_example(relative_path), &assets(bytes))
}

/// Runs an example with the asset map `{S: <the committed file>}` (tests 9 and 11).
fn run_committed_example(relative_path: &str) -> Run {
    let assets = Assets::from([(COMMITTED_ASSET.to_owned(), committed_bytes())]);
    Run::with_assets(load_example(relative_path), &assets)
}

/// Asserts the shape of test 1: the imported block alone in b-1, with volume 6000, 6 faces and the
/// bounding box [0,0,0]-[10,20,30].
fn assert_block_shape(run: &Run) {
    let shape = run.shape("b-1");
    assert_volume(shape, 6000.0);
    assert_face_count(shape, 6);
    assert_bounding_box(shape, [0.0, 0.0, 0.0], [10.0, 20.0, 30.0]);
}

/// Asserts the result of test 1: every feature ok and the imported block alone in b-1.
fn assert_imported_block(run: &Run) {
    run.assert_all_ok();
    assert_block_shape(run);
}

/// Asserts the face names of test 1 and the planes they lie in: by centroid the order is left,
/// front, bottom, top, back, right.
fn assert_block_faces_in_geometric_order(run: &Run) {
    let body = run.body("b-1");
    assert_face_names(body, &BLOCK_FACES);
    // Each face lies in the plane `axis = position`.
    let planes = [
        (0, 0.0),
        (1, 0.0),
        (2, 0.0),
        (2, 30.0),
        (1, 20.0),
        (0, 10.0),
    ];
    for (name, (axis, position)) in BLOCK_FACES.into_iter().zip(planes) {
        assert_face_in_plane(&face(body, name), axis, position);
    }
}

/// Asserts the dependencies of test 1, whose import names the asset `asset`.
fn assert_import_dependencies(run: &Run, asset: &str) {
    let dependencies = run.dependencies("f-imp1");
    let assets: Vec<(&str, &str)> = dependencies
        .assets
        .iter()
        .map(|link| (link.name.as_str(), link.field.as_str()))
        .collect();
    assert_eq!(assets, [(asset, "asset")]);
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

/// Asserts the result of test 2: the block joined to the 10 mm cube.
fn assert_joined_to_the_cube(run: &Run) {
    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 6750.0);
    assert_bounding_box(shape, [-5.0, -5.0, 0.0], [10.0, 20.0, 30.0]);
    assert_eq!(body_changes(run, "f-imp1"), ["b-1 Join"]);
}

/// Asserts the result of test 7: the block cut through the plate.
fn assert_cut_through_the_plate(run: &Run) {
    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 14000.0);
    assert_face_count(shape, 10);
    assert_bounding_box(shape, [-20.0, -15.0, 0.0], [20.0, 25.0, 10.0]);
    assert_eq!(body_changes(run, "f-imp1"), ["b-1 Cut"]);
}

/// Asserts the walls of the rectangular hole of test 7 and their imported names.
fn assert_hole_walls_keep_their_imported_names(run: &Run) {
    let walls = [
        ("f-imp1/face#1", 0, 0.0),
        ("f-imp1/face#2", 1, 0.0),
        ("f-imp1/face#5", 1, 20.0),
        ("f-imp1/face#6", 0, 10.0),
    ];
    let body = run.body("b-1");
    for (name, axis, position) in walls {
        assert_face_in_plane(&face(body, name), axis, position);
    }
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

    assert_block_faces_in_geometric_order(&run);
}

#[test]
fn t001_op_new_starts_the_body() {
    let run = run_example("import/new-body.ptsm", block_bytes());

    assert_eq!(body_changes(&run, "f-imp1"), ["b-1 New"]);
}

#[test]
fn t001_op_new_dependencies() {
    let run = run_example("import/new-body.ptsm", block_bytes());

    assert_import_dependencies(&run, &asset_name('0'));
}

#[test]
fn t002_join() {
    let run = run_example("import/join.ptsm", block_bytes());

    assert_joined_to_the_cube(&run);
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

    assert_cut_through_the_plate(&run);
}

#[test]
fn t007_cut_hole_walls_keep_their_imported_names() {
    let run = run_example("import/cut-through-plate.ptsm", block_bytes());

    assert_hole_walls_keep_their_imported_names(&run);
}

#[test]
fn t009_intersect_volume_faces_and_bounds() {
    let run = run_committed_example("import/intersect-step-block.ptsm");

    run.assert_all_ok();
    let shape = run.shape("b-1");
    assert_volume(shape, 250.0);
    assert_face_count(shape, 6);
    assert_bounding_box(shape, [0.0, 0.0, 0.0], [5.0, 5.0, 10.0]);
}

#[test]
fn t009_intersect_keeps_the_overlap() {
    let run = run_committed_example("import/intersect-step-block.ptsm");

    assert_eq!(body_changes(&run, "f-imp1"), ["b-1 Intersect"]);
}

#[test]
fn t009_intersect_names_each_face_after_the_solid_it_bounded() {
    let run = run_committed_example("import/intersect-step-block.ptsm");

    let body = run.body("b-1");
    let faces = [
        ("f-imp1/face#1", 0, 0.0),
        ("f-imp1/face#2", 1, 0.0),
        ("f-box1/right", 0, 5.0),
        ("f-box1/back", 1, 5.0),
        ("f-box1/top", 2, 10.0),
        ("f-box1/bottom", 2, 0.0),
    ];
    let names: Vec<&str> = faces.iter().map(|&(name, _, _)| name).collect();
    assert_face_names(body, &names);
    for (name, axis, position) in faces {
        assert_face_in_plane(&face(body, name), axis, position);
    }
}

#[test]
fn t009_intersect_aliases_the_block_bottom_to_the_box_bottom() {
    let run = run_committed_example("import/intersect-step-block.ptsm");

    let merged = aliases(run.body("b-1"));
    let block_bottom = ("f-imp1/face#3".to_owned(), vec!["f-box1/bottom".to_owned()]);
    assert!(
        merged.contains(&block_bottom),
        "the block's bottom merged into the box's: {merged:?}"
    );
}

#[test]
fn t010_skipped_content_warns_with_the_kernel_text() {
    let run = run_example("import/new-body.ptsm", skipped_bytes());

    run.assert_warnings("f-imp1", &["import_warning"]);
    assert_eq!(run.messages("f-imp1"), [SKIPPED_TWO_SHAPES]);
    assert_messages_pass_lint(&run, "f-imp1");
}

#[test]
fn t010_skipped_content_warning_has_no_pointer_detail_or_fixes() {
    let run = run_example("import/new-body.ptsm", skipped_bytes());

    let message = &run.feature("f-imp1").messages[0];
    assert_eq!(message.code, "import_warning");
    assert_eq!(message.pointer, None);
    assert_eq!(message.detail, None);
    assert!(message.fixes.is_empty(), "fixes: {:?}", message.fixes);
}

#[test]
fn t010_skipped_content_leaves_the_block_alone() {
    let run = run_example("import/new-body.ptsm", skipped_bytes());

    assert_block_shape(&run);
    assert_face_names(run.body("b-1"), &BLOCK_FACES);
}

#[test]
fn faces_fixture_is_two_faces_without_a_solid() {
    let faces = two_faces();

    assert_eq!(faces.kind(), ShapeKind::Compound);
    assert_eq!(faces.count(SubShapeKind::Face), 2);
    assert_eq!(faces.count(SubShapeKind::Solid), 0);
}

#[test]
fn t011_committed_asset_is_named_by_its_hash() {
    let hash = blake3::hash(&committed_bytes());

    assert_eq!(format!("{}.step", hash.to_hex()), COMMITTED_ASSET);
}

#[test]
fn t011_step_block_volume_faces_and_bounds() {
    let run = run_committed_example("import/step-block.ptsm");

    assert_imported_block(&run);
}

#[test]
fn t011_step_block_names_faces_in_geometric_order() {
    let run = run_committed_example("import/step-block.ptsm");

    assert_block_faces_in_geometric_order(&run);
}

#[test]
fn t011_step_block_starts_the_body() {
    let run = run_committed_example("import/step-block.ptsm");

    assert_eq!(body_changes(&run, "f-imp1"), ["b-1 New"]);
}

#[test]
fn t011_step_block_dependencies_name_the_committed_asset() {
    let run = run_committed_example("import/step-block.ptsm");

    assert_import_dependencies(&run, COMMITTED_ASSET);
}

#[test]
fn t011_join_step_block() {
    let run = run_committed_example("import/join-step-block.ptsm");

    assert_joined_to_the_cube(&run);
}

#[test]
fn t011_cut_step_block_volume_faces_and_bounds() {
    let run = run_committed_example("import/cut-step-block.ptsm");

    assert_cut_through_the_plate(&run);
}

#[test]
fn t011_cut_step_block_hole_walls_keep_their_imported_names() {
    let run = run_committed_example("import/cut-step-block.ptsm");

    assert_hole_walls_keep_their_imported_names(&run);
}
