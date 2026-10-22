//! The STEP fixtures that `feature_import.rs` shares with the import budget of `budgets.rs`.

use std::sync::Arc;

use partsmith_occt_sys::{Plane, StepBody, make_box, write_step};

use super::{Assets, step_asset_name};

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

/// The fixture `BYTES`: a STEP file with one body named `Block` spanning [0,10]x[0,20]x[0,30].
pub fn block_bytes() -> Arc<[u8]> {
    let bodies = [block("Block", &Plane::XY, [10.0, 20.0, 30.0])];
    Arc::from(write_step(&bodies).expect("the kernel writes STEP"))
}

/// The asset map `{A: bytes}`.
pub fn assets(bytes: Arc<[u8]>) -> Assets {
    Assets::from([(step_asset_name(), bytes)])
}
