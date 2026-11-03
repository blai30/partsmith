//! The STEP fixtures of `feature_import.rs`, shared with the import budget of `budgets.rs` and,
//! through `box_step_asset`, with the document tests that import a box (`feature_messages.rs` and
//! `feature_sketch.rs`); `void_listed_twice` and `self_void`, the STEP text edits that make the
//! solids of features/import.md's `VOID_TWICE`, `BOTH`, `SELF_VOID` and `TWO_SELF_VOID` invalid
//! after repair, with those fixtures, which the copy features' documents import too
//! (`feature_mirror.rs`, `feature_linear_pattern.rs` and `feature_circular_pattern.rs`).

use std::sync::Arc;

use partsmith_features::Fix;
use partsmith_occt_sys::{Plane, Point, Shape, StepBody, SubShapeKind, cut, make_box, write_step};

use super::{
    Assets, Run, assert_bounding_box, assert_error_field, assert_face_count, assert_solid_count,
    asset_name, id, message,
};

/// A STEP body named `name` holding `shape`, without a color.
pub fn step_body(name: &str, shape: Shape) -> StepBody {
    StepBody {
        shape,
        name: name.to_owned(),
        color: None,
    }
}

/// A STEP body named `name` holding a `make_box` solid of this size on `placement`.
pub fn block(name: &str, placement: &Plane, size: [f64; 3]) -> StepBody {
    let [length, width, height] = size;
    let primitive =
        make_box(placement, length, width, height).expect("the kernel makes a valid box");
    step_body(name, primitive.shape)
}

/// The STEP file the kernel writes for `bodies`.
pub fn step_bytes(bodies: &[StepBody]) -> Arc<[u8]> {
    Arc::from(write_step(bodies).expect("the kernel writes STEP"))
}

/// The solid of `BYTES`: a `make_box` solid spanning [0,10]x[0,20]x[0,30].
pub fn block_solid() -> Shape {
    block("Block", &Plane::XY, [10.0, 20.0, 30.0]).shape
}

/// The fixture `BYTES`: a STEP file with one body named `Block` holding `block_solid`.
pub fn block_bytes() -> Arc<[u8]> {
    step_bytes(&[step_body("Block", block_solid())])
}

/// The fixture `TWO`: a STEP file with two 10 mm cubes named `A` and `B`, the second moved to x = 50.
pub fn two_cubes_bytes() -> Arc<[u8]> {
    let moved = Plane {
        origin: Point::new(50.0, 0.0, 0.0),
        ..Plane::XY
    };
    step_bytes(&[
        block("A", &Plane::XY, [10.0, 10.0, 10.0]),
        block("B", &moved, [10.0, 10.0, 10.0]),
    ])
}

/// The asset map `{A: bytes}`.
pub fn assets(bytes: Arc<[u8]>) -> Assets {
    Assets::from([(asset_name('0'), bytes)])
}

/// The asset map of a STEP file with one body named `name`, a `make_box` solid of `size` with its
/// corner at the origin, under `asset_name('0')`.
pub fn box_step_asset(name: &str, size: [f64; 3]) -> Assets {
    assets(step_bytes(&[block(name, &Plane::XY, size)]))
}

/// The number of the entity a line of a STEP text defines (`#<n> = ...`), if it defines one.
fn defined_entity(line: &str) -> Option<u64> {
    let (reference, _) = line.trim_start().split_once('=')?;
    reference.trim().strip_prefix('#')?.parse().ok()
}

/// The fixture edit of `VOID_TWICE` and `BOTH` (features/import.md section 6, kernel.md tests 140
/// and 141) on `step`, the STEP text `write_step` wrote for one solid with one void: the void of
/// its `BREP_WITH_VOIDS('',#o,(#v))` entity listed twice, as the entity
/// `#n = ORIENTED_CLOSED_SHELL('',*,#s,.F.);`, `n` one more than the largest entity number, on
/// its own line before the last `ENDSEC;`, and `(#v)` replaced by `(#v,#n)`. The kernel's tests
/// make the same edit (`void_listed_twice` in `crates/partsmith-occt-sys/tests/exchange.rs`, the
/// reference), with the same checks of the text it edits.
pub fn void_listed_twice(step: &str) -> String {
    let mut entities = step.match_indices("BREP_WITH_VOIDS(");
    let (start, _) = entities.next().expect("the solid is written with its void");
    assert!(
        entities.next().is_none(),
        "the STEP text has one BREP_WITH_VOIDS entity"
    );
    let length = step[start..]
        .find(");")
        .expect("the BREP_WITH_VOIDS entity is closed");
    let entity = &step[start..=start + length];
    let open = entity.rfind('(').expect("the entity lists its void");
    let void = entity[open + 1..].trim_end_matches(')').trim().to_owned();
    let void_number: u64 = void
        .strip_prefix('#')
        .and_then(|digits| digits.parse().ok())
        .unwrap_or_else(|| panic!("the solid has one void, not {void:?}"));
    let void_shell = step
        .lines()
        .find(|line| defined_entity(line) == Some(void_number))
        .and_then(|line| line.split_once('='))
        .map(|(_, value)| value.trim())
        .expect("the void is defined on one line");
    assert!(
        void_shell.starts_with("ORIENTED_CLOSED_SHELL('',*,") && void_shell.ends_with(",.F.);"),
        "the void is a reversed oriented closed shell: {void_shell}"
    );
    let copy = step
        .lines()
        .filter_map(defined_entity)
        .max()
        .expect("the STEP text defines entities")
        + 1;
    let listed_twice = entity.replacen(&format!("({void})"), &format!("({void},#{copy})"), 1);
    assert_ne!(listed_twice, entity, "the void list is ({void})");
    let end_of_data = step.rfind("ENDSEC;").expect("the STEP text has sections");
    let edited = format!(
        "{}#{copy} = {void_shell}\n{}",
        &step[..end_of_data],
        &step[end_of_data..]
    );
    edited.replacen(entity, &listed_twice, 1)
}

/// `HOLLOW` of kernel.md test 140: the block of `BYTES` with the closed cavity [2,8]x[2,18]x[2,28].
pub fn hollow() -> Shape {
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
pub fn hollow_bytes_with_the_void_twice(shape: Shape) -> Arc<[u8]> {
    let bytes = write_step(&[step_body("Hollow", shape)]).expect("the kernel writes STEP");
    let step = String::from_utf8(bytes).expect("STEP text is UTF-8");
    Arc::from(void_listed_twice(&step).into_bytes())
}

/// The fixture `VOID_TWICE`: `HOLLOW` written as one body named `Hollow`, its void listed twice,
/// so its solid is still invalid after repair.
pub fn void_twice_bytes() -> Arc<[u8]> {
    hollow_bytes_with_the_void_twice(hollow())
}

/// The fixture edit of `SELF_VOID` (features/import.md section 6) on `step`, the STEP text of
/// `solids` solids without a void: the edit engine.md test 162 makes, applied to the first solid.
/// Its entity `#s = MANIFOLD_SOLID_BREP('',#o);` becomes `#s = BREP_WITH_VOIDS('',#o,(#n));`, and
/// `#n = ORIENTED_CLOSED_SHELL('',*,#o,.F.);`, `n` one more than the largest entity number, is
/// added on its own line before the last `ENDSEC;`. A copy of the engine tests' `self_void`, with
/// its checks: one `MANIFOLD_SOLID_BREP` entity per solid and no `BREP_WITH_VOIDS`.
pub fn self_void(step: &str, solids: usize) -> String {
    assert_eq!(
        step.matches("MANIFOLD_SOLID_BREP(").count(),
        solids,
        "the STEP text has one MANIFOLD_SOLID_BREP entity per solid"
    );
    assert!(
        !step.contains("BREP_WITH_VOIDS("),
        "the STEP text has no BREP_WITH_VOIDS entity"
    );
    let solid = step
        .lines()
        .find(|line| line.contains("MANIFOLD_SOLID_BREP("))
        .expect("the solid is defined on one line");
    let (number, value) = solid.split_once('=').expect("the solid's line defines it");
    let outer_shell = value
        .trim()
        .strip_prefix("MANIFOLD_SOLID_BREP('',")
        .and_then(|rest| rest.strip_suffix(");"))
        .unwrap_or_else(|| panic!("the solid names its one shell: {solid}"));
    let void = step
        .lines()
        .filter_map(defined_entity)
        .max()
        .expect("the STEP text defines entities")
        + 1;
    let end_of_data = step.rfind("ENDSEC;").expect("the STEP text has sections");
    let edited = format!(
        "{}#{void} = ORIENTED_CLOSED_SHELL('',*,{outer_shell},.F.);\n{}",
        &step[..end_of_data],
        &step[end_of_data..]
    );
    let with_void = format!(
        "{} = BREP_WITH_VOIDS('',{outer_shell},(#{void}));",
        number.trim()
    );
    edited.replacen(solid, &with_void, 1)
}

/// `bytes`, the STEP file of `solids` solids, with the first edited by `self_void`.
fn with_self_void(bytes: &[u8], solids: usize) -> Arc<[u8]> {
    let step = String::from_utf8(bytes.to_vec()).expect("STEP text is UTF-8");
    Arc::from(self_void(&step, solids).into_bytes())
}

/// The fixture `SELF_VOID`: `BYTES` with the block's outer shell listed as its void too, so its
/// one solid stays invalid after repair and bounds no material.
pub fn self_void_bytes() -> Arc<[u8]> {
    with_self_void(&block_bytes(), 1)
}

/// The fixture `TWO_SELF_VOID`: `TWO` with its first cube edited as in `SELF_VOID`. The import
/// gives one body of two solids, one without volume and one with 1000 mm³.
pub fn two_self_void_bytes() -> Arc<[u8]> {
    with_self_void(&two_cubes_bytes(), 2)
}

/// The `IMP(op)` fixture of features/import.md: the import `f-imp1` of the asset `A` into b-1.
pub fn import_record(op: &str) -> String {
    format!(
        r#"{{"id": "f-imp1", "type": "import", "body": "b-1", "asset": "{}", "op": "{op}"}}"#,
        asset_name('0')
    )
}

/// Asserts that `feature`, a copy feature, failed with `body_not_valid` on its field `body`,
/// rendered `text`, with `Show` of the import `f-imp1` as its one fix and `detail` as its detail
/// (features.md sections 3.5 and 4.1).
pub fn assert_body_not_valid(run: &Run, feature: &str, text: &str, detail: Option<&str>) {
    let error = run.assert_error(feature, "body_not_valid");
    assert_error_field(error, "body");
    assert_eq!(run.messages(feature), [text]);
    let message = message(run, feature, "body_not_valid");
    assert_eq!(message.pointer.as_deref(), Some("/body"));
    assert_eq!(
        message.fixes,
        [Fix::Show {
            feature: id("f-imp1"),
            entity: None,
        }]
    );
    assert_eq!(message.detail.as_deref(), detail);
}

/// Asserts that `body` holds the solid of `SELF_VOID` as the import read it, within `min` and
/// `max`: one solid of 6 faces, volume 0 within 1e-9, not valid (features/import.md test 17).
pub fn assert_self_void_block(run: &Run, body: &str, min: [f64; 3], max: [f64; 3]) {
    let shape = run.shape(body);
    assert_solid_count(shape, 1);
    assert_face_count(shape, 6);
    let volume = shape.volume().expect("the kernel measures the solid");
    assert!(volume.abs() <= 1e-9, "volume {volume} is 0 within 1e-9");
    assert!(
        !shape.is_valid().expect("the kernel checks the solid"),
        "the solid is still invalid after repair"
    );
    assert_bounding_box(shape, min, max);
}

/// Asserts that `body` holds the two solids of `TWO_SELF_VOID` as the import read them: 1000 mm³
/// in all, not valid, within [0,0,0]-[60,10,10].
pub fn assert_two_self_void(run: &Run, body: &str) {
    let shape = run.shape(body);
    assert_solid_count(shape, 2);
    let volume = shape.volume().expect("the kernel measures the solids");
    assert!(
        (volume - 1000.0).abs() <= 1e-6 * 1000.0,
        "volume {volume} is the second cube's 1000"
    );
    assert!(
        !shape.is_valid().expect("the kernel checks the solids"),
        "the first solid is still invalid after repair"
    );
    assert_bounding_box(shape, [0.0, 0.0, 0.0], [60.0, 10.0, 10.0]);
}

/// The number of solids `body` holds at the end of the run, 0 for an empty body.
pub fn solid_count(run: &Run, body: &str) -> u32 {
    run.body(body)
        .shape()
        .map_or(0, |shape| shape.count(SubShapeKind::Solid))
}
