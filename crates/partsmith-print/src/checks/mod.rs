//! The printability checks of spec section 3.9, one module each.

pub(crate) mod bed_fit;
pub(crate) mod contact;
pub(crate) mod islands;
pub(crate) mod overhangs;
pub(crate) mod small_features;
pub(crate) mod thin_walls;
pub(crate) mod trapped_volumes;
pub(crate) mod validity;

/// The length slack `ε` of spec section 3.9.0, in mm.
pub(crate) const EPSILON: f64 = 1e-6;
