//! The built-in feature types (features.md section 3.13), one module per type.
//!
//! Each module implements one type through the public `Feature` trait, exactly as a plugin
//! would, and `registry::register_builtins` registers them all. To add a type, declare its module
//! here and add its registration to `BUILTINS`.

pub mod boolean;
pub mod r#box;
pub mod chamfer;
pub mod circular_pattern;
pub(crate) mod common;
pub mod cone;
mod copy;
pub mod cylinder;
pub mod datum_plane;
mod dressup;
pub mod extrude;
pub mod fillet;
pub mod hole;
pub mod import;
pub mod linear_pattern;
pub mod mirror;
pub mod move_body;
mod primitives;
pub mod revolve;
pub mod shell;
pub mod sketch;
pub mod sphere;
pub mod split_body;
pub mod torus;

use crate::registry::{Registry, RegistryError};

/// Registers one feature type, for example `Registry::register::<extrude::Extrude>`.
pub(crate) type Registration = fn(&mut Registry) -> Result<(), RegistryError>;

/// The registration of every built-in feature type.
pub(crate) const BUILTINS: &[Registration] = &[
    Registry::register::<boolean::Boolean>,
    Registry::register::<r#box::BoxFeature>,
    Registry::register::<chamfer::Chamfer>,
    Registry::register::<circular_pattern::CircularPattern>,
    Registry::register::<cone::Cone>,
    Registry::register::<cylinder::Cylinder>,
    Registry::register::<datum_plane::DatumPlane>,
    Registry::register::<extrude::Extrude>,
    Registry::register::<fillet::Fillet>,
    Registry::register::<hole::Hole>,
    Registry::register::<import::Import>,
    Registry::register::<linear_pattern::LinearPattern>,
    Registry::register::<mirror::Mirror>,
    Registry::register::<move_body::MoveBody>,
    Registry::register::<revolve::Revolve>,
    Registry::register::<shell::Shell>,
    Registry::register::<sketch::SketchFeature>,
    Registry::register::<sphere::Sphere>,
    Registry::register::<split_body::SplitBody>,
    Registry::register::<torus::Torus>,
];
