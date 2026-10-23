//! The STEP fixtures of `feature_import.rs`, shared with the import budget of `budgets.rs` and,
//! through `box_step_asset`, with the document tests that import a box (`feature_messages.rs` and
//! `feature_sketch.rs`).

use std::sync::Arc;

use partsmith_occt_sys::{Plane, StepBody, make_box, write_step};

use super::{Assets, asset_name};

/// A STEP body named `name` holding a `make_box` solid of this size on `placement`.
pub fn block(name: &str, placement: &Plane, size: [f64; 3]) -> StepBody {
    let [length, width, height] = size;
    let primitive =
        make_box(placement, length, width, height).expect("the kernel makes a valid box");
    StepBody {
        shape: primitive.shape,
        name: name.to_owned(),
        color: None,
    }
}

/// The STEP file the kernel writes for `bodies`.
pub fn step_bytes(bodies: &[StepBody]) -> Arc<[u8]> {
    Arc::from(write_step(bodies).expect("the kernel writes STEP"))
}

/// The fixture `BYTES`: a STEP file with one body named `Block` spanning [0,10]x[0,20]x[0,30].
pub fn block_bytes() -> Arc<[u8]> {
    step_bytes(&[block("Block", &Plane::XY, [10.0, 20.0, 30.0])])
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
