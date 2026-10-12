//! The built-in feature types (features.md section 3.13), one module per type.
//!
//! Each module implements one type through the public `Feature` trait, exactly as a plugin
//! would, and `registry::register_builtins` registers them all. To add a type, declare its module
//! here and add its registration to `BUILTINS`.

pub mod r#box;
pub(crate) mod common;

use crate::registry::{Registry, RegistryError};

/// Registers one feature type, for example `Registry::register::<extrude::Extrude>`.
pub(crate) type Registration = fn(&mut Registry) -> Result<(), RegistryError>;

/// The registration of every built-in feature type.
pub(crate) const BUILTINS: &[Registration] = &[Registry::register::<r#box::BoxFeature>];
