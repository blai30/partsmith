//! The built-in feature types (features.md section 3.13), one module per type.
//!
//! Each module implements one type through the public `Feature` trait, exactly as a plugin
//! would. To add a type, declare its module here and add its registration to `BUILTINS`.

use crate::registry::{Registry, RegistryError};

/// Registers one feature type, for example `Registry::register::<extrude::Extrude>`.
type Registration = fn(&mut Registry) -> Result<(), RegistryError>;

/// The registration of every built-in feature type.
const BUILTINS: &[Registration] = &[];

/// Registers every built-in feature type through `Registry::register`, exactly as a plugin would.
pub fn register_builtins(registry: &mut Registry) -> Result<(), RegistryError> {
    for register in BUILTINS {
        register(registry)?;
    }
    Ok(())
}
