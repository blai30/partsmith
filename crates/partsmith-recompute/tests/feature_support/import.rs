//! The STEP fixtures of `feature_import.rs`, shared with the import budget of `budgets.rs` and,
//! through `box_step_asset`, with the document tests that import a box (`feature_messages.rs` and
//! `feature_sketch.rs`), and `void_listed_twice`, the STEP text edit that makes the solid of
//! features/import.md's `VOID_TWICE` and `BOTH` invalid after repair.

use std::sync::Arc;

use partsmith_occt_sys::{Plane, Shape, StepBody, make_box, write_step};

use super::{Assets, asset_name};

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
