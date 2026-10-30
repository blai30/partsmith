//! Acceptance tests of the `import` feature (docs/spec/features/import.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/import/`, except the joins of tests 15 and 16, whose documents are written here. The
//! placeholder examples and those joins get asset bytes written with the kernel at test time, as the spec's fixtures define them (`VOID_TWICE` and `BOTH` by editing the STEP text the
//! kernel wrote); the step-block examples (tests 9 and 11) get the committed file `S` of
//! `examples/import/assets/`, the bytes `partsmith rebuild` reads.

use crate::feature_support;

use std::sync::Arc;

use feature_support::import::{
    assets, block, block_bytes, block_solid, step_body, step_bytes, void_listed_twice,
};
use feature_support::{
    Assets, Run, aliases, assert_bounding_box, assert_empty, assert_examples_canonical,
    assert_face_count, assert_face_in_plane, assert_face_names, assert_messages_pass_lint,
    assert_shell_count, assert_solid_count, assert_volume, assert_volume_faces_and_bounds,
    asset_name, body_changes, document, examples_folder, face, load_example,
};
use partsmith_features::FeatureError;
use partsmith_occt_sys::{
    PlanarCurve, PlanarLoop, PlanarRegion, Plane, Point, Point2, Shape, ShapeKind, SubShapeKind,
    TaggedCurve, compound, cut, make_box, make_profile, write_step,
};
use partsmith_recompute::Message;

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

/// The message of test 10: the two faces of `SKIPPED` left out.
const TWO_SHAPES_LEFT_OUT: &str =
    "This import left out 2 shapes of the STEP file that are not closed solids.";

/// The message of tests 12 and 14: the face of `ONE_FACE` or `BOTH` left out.
const ONE_SHAPE_LEFT_OUT: &str =
    "This import left out 1 shape of the STEP file that is not a closed solid.";

/// The message of tests 13 to 15: the solid of `VOID_TWICE` or `BOTH` kept invalid, the same for
/// every op.
const ONE_SOLID_KEPT: &str = "This import kept 1 solid of the STEP file that is not valid and could not be repaired, so the shape this import gives may be wrong, and features that change it later may fail or give a wrong shape.";

/// The detail of test 10: the kernel's sentence for the two faces of `SKIPPED`.
const TWO_SHAPES_DETAIL: &str = "read_step: Skipped 2 shapes that are not closed solids.";

/// The detail of test 12: the kernel's sentence for the face of `ONE_FACE`.
const ONE_SHAPE_DETAIL: &str = "read_step: Skipped 1 shape that is not a closed solid.";

/// The detail of test 13: the kernel's sentence for the solid of `VOID_TWICE`.
const ONE_SOLID_DETAIL: &str =
    "read_step: Solid 1 is not valid after repair; booleans with it may fail.";

/// The detail of both warnings of test 14: the kernel's sentences in its order.
const BOTH_DETAIL: &str = "read_step: Solid 1 is not valid after repair; booleans with it may fail. Skipped 1 shape that is not a closed solid.";

/// The plate of features/import.md tests 7 and 15, spanning [-20,-15,0]-[20,25,10].
const PLATE: &str = r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "position": ["0", "5", "0"], "op": "new"}"#;

/// The box of test 16, spanning [45,55]x[45,55]x[0,10], apart from the imported block.
const BOX_APART: &str = r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "position": ["50", "50", "0"], "op": "new"}"#;

/// The face names of the solid of `VOID_TWICE`, `f-imp1/face#1` to `f-imp1/face#12` (test 13).
fn hollow_face_names() -> Vec<String> {
    (1..=12)
        .map(|number| format!("f-imp1/face#{number}"))
        .collect()
}

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

/// The shape of `make_profile` on `Plane::XY` of one `square` for each of `lefts`.
fn squares(lefts: &[f64]) -> Shape {
    let regions: Vec<PlanarRegion> = lefts.iter().map(|&left| square(left)).collect();
    make_profile(&Plane::XY, &regions)
        .expect("the kernel makes the squares")
        .shape
}

/// The fixture `FACES`: the shape of `make_profile` of two squares with 10 mm sides on
/// `Plane::XY`, [50,60]x[0,10] and [70,80]x[0,10], a compound of two faces that bound no solid.
fn two_faces() -> Shape {
    squares(&[50.0, 70.0])
}

/// One part named `Block` holding the block of `BYTES` and, beside it, `extra`.
fn block_beside(extra: Shape) -> Arc<[u8]> {
    let shape = compound(&[block_solid(), extra])
        .expect("the kernel groups the block and what lies beside it")
        .shape;
    step_bytes(&[step_body("Block", shape)])
}

/// The fixture `SKIPPED`: one part named `Block` holding the block of `BYTES` and, beside it, the
/// two faces of `FACES`.
fn skipped_bytes() -> Arc<[u8]> {
    block_beside(two_faces())
}

/// The fixture `FACE` of kernel.md test 139: the shape of `make_profile` of the square
/// [50,60]x[0,10] alone on `Plane::XY`, one face.
fn one_face() -> Shape {
    squares(&[50.0])
}

/// The fixture `ONE_FACE`: `SKIPPED` with only the first square, [50,60]x[0,10], beside the block.
fn one_face_bytes() -> Arc<[u8]> {
    block_beside(one_face())
}

/// `HOLLOW` of kernel.md test 140: the block of `BYTES` with the closed cavity [2,8]x[2,18]x[2,28].
fn hollow() -> Shape {
    let solid = block_solid();
    let cavity_corner = Plane {
        origin: Point::new(2.0, 2.0, 2.0),
        ..Plane::XY
    };
    let cavity = make_box(&cavity_corner, 6.0, 16.0, 26.0)
        .expect("the kernel makes the cavity box")
        .shape;
    cut(&solid, &[cavity])
        .expect("the kernel cuts the cavity out")
        .shape
}

/// The STEP text `write_step` gives for `shape` as one body named `Hollow`, with its void listed
/// twice (kernel.md tests 140 and 141).
fn hollow_bytes_with_the_void_twice(shape: Shape) -> Arc<[u8]> {
    let bytes = write_step(&[step_body("Hollow", shape)]).expect("the kernel writes STEP");
    let step = String::from_utf8(bytes).expect("STEP text is UTF-8");
    Arc::from(void_listed_twice(&step).into_bytes())
}

/// The fixture `VOID_TWICE`: `HOLLOW` written as one body named `Hollow`, its void listed twice,
/// so its solid is still invalid after repair.
fn void_twice_bytes() -> Arc<[u8]> {
    hollow_bytes_with_the_void_twice(hollow())
}

/// The fixture `BOTH`: `VOID_TWICE` made from `HOLLOW` with the square of `ONE_FACE` in the same
/// part.
fn both_bytes() -> Arc<[u8]> {
    let shape = compound(&[hollow(), one_face()])
        .expect("the kernel groups the hollow block and the face")
        .shape;
    hollow_bytes_with_the_void_twice(shape)
}

/// Runs `box_record` in b-1, then `IMP("join")` with the asset map `{A: bytes}` (tests 15 and 16).
fn run_join(box_record: &str, bytes: Arc<[u8]>) -> Run {
    let import = format!(
        r#"{{"id": "f-imp1", "type": "import", "body": "b-1", "asset": "{}", "op": "join"}}"#,
        asset_name('0')
    );
    Run::with_assets(document(&["b-1"], &[box_record, &import]), &assets(bytes))
}

/// The messages of the import, failing the test when it has none, so that the assertions over
/// each never pass for an import that does not warn.
fn import_messages(run: &Run) -> &[Message] {
    let messages = &run.feature("f-imp1").messages;
    assert!(!messages.is_empty(), "the import warns; {}", run.summary());
    messages
}

/// Asserts the messages of the import: there is one at least, and each passes the message lint and
/// holds none of the sentences of its detail (features.md test 14).
fn assert_messages_hold_no_kernel_sentence(run: &Run) {
    assert_messages_pass_lint(run, "f-imp1");
    for message in import_messages(run) {
        let detail = message
            .detail
            .as_deref()
            .expect("the kernel's sentences are in the detail");
        let sentences = detail
            .strip_prefix("read_step: ")
            .unwrap_or_else(|| panic!("the detail names read_step: {detail:?}"));
        for sentence in sentences.split_inclusive(". ").map(str::trim) {
            assert!(
                !message.text.contains(sentence),
                "{:?} holds the kernel's sentence {sentence:?}",
                message.text
            );
        }
    }
}

/// Asserts that the import has a message at least, and that each is an import warning with
/// `detail`, no pointer and no fixes.
fn assert_import_warning_details(run: &Run, detail: &str) {
    for message in import_messages(run) {
        assert_eq!(message.code, "import_warning");
        assert_eq!(message.detail.as_deref(), Some(detail));
        assert_eq!(message.pointer, None);
        assert!(message.fixes.is_empty(), "fixes: {:?}", message.fixes);
    }
}

/// Asserts the solid of test 13 in b-1: the solid of `VOID_TWICE` as the kernel read it.
fn assert_invalid_hollow_block(run: &Run) {
    let shape = run.shape("b-1");
    assert_solid_count(shape, 1);
    assert_face_count(shape, 12);
    // 6000 minus the cavity's 2496, twice (kernel.md test 140).
    assert_volume(shape, 1008.0);
    assert_bounding_box(shape, [0.0, 0.0, 0.0], [10.0, 20.0, 30.0]);
    assert!(
        !shape.is_valid().expect("the kernel checks the solid"),
        "the solid is still invalid after repair"
    );
    assert_face_names(run.body("b-1"), &hollow_face_names());
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
    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        6000.0,
        6,
        [0.0, 0.0, 0.0],
        [10.0, 20.0, 30.0],
    );
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
    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        14000.0,
        10,
        [-20.0, -15.0, 0.0],
        [20.0, 25.0, 10.0],
    );
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
    assert_volume_faces_and_bounds(
        run.shape("b-1"),
        250.0,
        6,
        [0.0, 0.0, 0.0],
        [5.0, 5.0, 10.0],
    );
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
fn t010_skipped_content_warns_once_with_its_count() {
    let run = run_example("import/new-body.ptsm", skipped_bytes());

    run.assert_warnings("f-imp1", &["import_warning"]);
    assert_eq!(run.messages("f-imp1"), [TWO_SHAPES_LEFT_OUT]);
    assert_messages_hold_no_kernel_sentence(&run);
}

#[test]
fn t010_skipped_content_warning_has_the_kernels_sentence_in_its_detail() {
    let run = run_example("import/new-body.ptsm", skipped_bytes());

    assert_import_warning_details(&run, TWO_SHAPES_DETAIL);
}

#[test]
fn t010_skipped_content_leaves_the_block_alone() {
    let run = run_example("import/new-body.ptsm", skipped_bytes());

    assert_block_shape(&run);
    assert_face_names(run.body("b-1"), &BLOCK_FACES);
}

#[test]
fn t012_one_skipped_shape_warns_in_the_singular() {
    let run = run_example("import/new-body.ptsm", one_face_bytes());

    run.assert_warnings("f-imp1", &["import_warning"]);
    assert_eq!(run.messages("f-imp1"), [ONE_SHAPE_LEFT_OUT]);
    assert_messages_hold_no_kernel_sentence(&run);
}

#[test]
fn t012_one_skipped_shape_warning_has_the_kernels_sentence_in_its_detail() {
    let run = run_example("import/new-body.ptsm", one_face_bytes());

    assert_import_warning_details(&run, ONE_SHAPE_DETAIL);
}

#[test]
fn t012_one_skipped_shape_leaves_the_block_alone() {
    let run = run_example("import/new-body.ptsm", one_face_bytes());

    assert_block_shape(&run);
    assert_face_names(run.body("b-1"), &BLOCK_FACES);
}

#[test]
fn t013_an_invalid_solid_warns_that_its_shape_may_be_wrong() {
    let run = run_example("import/new-body.ptsm", void_twice_bytes());

    run.assert_warnings("f-imp1", &["import_warning"]);
    assert_eq!(run.messages("f-imp1"), [ONE_SOLID_KEPT]);
    assert_messages_hold_no_kernel_sentence(&run);
}

#[test]
fn t013_an_invalid_solid_warning_has_the_kernels_sentence_in_its_detail() {
    let run = run_example("import/new-body.ptsm", void_twice_bytes());

    assert_import_warning_details(&run, ONE_SOLID_DETAIL);
}

#[test]
fn t013_the_invalid_solid_is_kept_as_the_kernel_read_it() {
    let run = run_example("import/new-body.ptsm", void_twice_bytes());

    assert_invalid_hollow_block(&run);
}

#[test]
fn t013_the_invalid_solid_starts_the_body() {
    let run = run_example("import/new-body.ptsm", void_twice_bytes());

    assert_eq!(body_changes(&run, "f-imp1"), ["b-1 New"]);
}

#[test]
fn t014_both_warn_about_the_invalid_solid_first() {
    let run = run_example("import/new-body.ptsm", both_bytes());

    run.assert_warnings("f-imp1", &["import_warning", "import_warning"]);
    assert_eq!(run.messages("f-imp1"), [ONE_SOLID_KEPT, ONE_SHAPE_LEFT_OUT]);
    assert_messages_hold_no_kernel_sentence(&run);
}

#[test]
fn t014_both_warnings_carry_every_kernel_sentence_in_its_order() {
    let run = run_example("import/new-body.ptsm", both_bytes());

    assert_import_warning_details(&run, BOTH_DETAIL);
}

#[test]
fn t014_both_keep_the_invalid_solid_alone() {
    let run = run_example("import/new-body.ptsm", both_bytes());

    assert_invalid_hollow_block(&run);
}

#[test]
fn t015_an_invalid_solid_joined_warns_as_with_op_new() {
    let run = run_join(PLATE, void_twice_bytes());

    run.assert_warnings("f-imp1", &["import_warning"]);
    assert_eq!(run.messages("f-imp1"), [ONE_SOLID_KEPT]);
    assert_messages_hold_no_kernel_sentence(&run);
}

#[test]
fn t015_an_invalid_solid_joined_warning_has_the_kernels_sentence_in_its_detail() {
    let run = run_join(PLATE, void_twice_bytes());

    assert_import_warning_details(&run, ONE_SOLID_DETAIL);
}

#[test]
fn t015_an_invalid_solid_joined_changes_the_body_by_a_join() {
    let run = run_join(PLATE, void_twice_bytes());

    assert_eq!(body_changes(&run, "f-imp1"), ["b-1 Join"]);
}

#[test]
fn t016_import_warnings_come_before_the_body_rules_warnings() {
    let run = run_join(BOX_APART, one_face_bytes());

    run.assert_warnings("f-imp1", &["import_warning", "separate_solid"]);
    assert_eq!(run.messages("f-imp1")[0], ONE_SHAPE_LEFT_OUT);
}

#[test]
fn t016_the_block_is_joined_apart_from_the_box() {
    let run = run_join(BOX_APART, one_face_bytes());

    let shape = run.shape("b-1");
    assert_solid_count(shape, 2);
    assert_volume(shape, 7000.0);
}

#[test]
fn one_face_fixture_is_one_face_without_a_solid() {
    let face = one_face();

    assert_eq!(face.count(SubShapeKind::Face), 1);
    assert_eq!(face.count(SubShapeKind::Solid), 0);
}

#[test]
fn hollow_fixture_is_the_block_with_a_closed_cavity() {
    let shape = hollow();

    assert_solid_count(&shape, 1);
    assert_shell_count(&shape, 2);
    assert_volume(&shape, 3504.0);
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
