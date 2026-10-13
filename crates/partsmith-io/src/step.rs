//! STEP export and import through the kernel (spec section 3.8).

use partsmith_model::Color;
use partsmith_occt_sys::{self as kernel, Shape, StepBody};

use crate::IoError;
use crate::checks::check_not_empty;
use crate::text::display_name;

/// One body to write to STEP.
#[derive(Clone, Copy, Debug)]
pub struct StepExportBody<'a> {
    /// Product name.
    pub name: &'a str,
    /// Surface color.
    pub color: Option<Color>,
    /// The exact shape; must contain at least one solid.
    pub shape: &'a Shape,
}

/// One solid read from a STEP file.
#[derive(Clone, Debug)]
pub struct ImportedSolid {
    /// Name of its part in the file, trimmed; empty when the part has none.
    pub name: String,
    /// Its color in the file, if any.
    pub color: Option<Color>,
    /// The solid, in millimeters.
    pub shape: Shape,
}

/// The result of [`read_step`].
#[derive(Clone, Debug)]
pub struct ImportedStep {
    /// Solids in file order (the order of `partsmith_occt_sys::read_step`).
    pub solids: Vec<ImportedSolid>,
    /// The kernel's warnings about skipped or repaired content, unchanged.
    pub warnings: Vec<String>,
}

/// Writes bodies to a STEP file through the kernel.
///
/// The kernel writes AP214 in millimeters with one product per body, named with the body's
/// display name and colored with its color, deterministically.
pub fn write_step(bodies: &[StepExportBody<'_>]) -> Result<Vec<u8>, IoError> {
    check_not_empty(bodies)?;
    let kernel_bodies: Vec<StepBody> = bodies
        .iter()
        .enumerate()
        .map(|(index, body)| StepBody {
            shape: body.shape.clone(),
            name: display_name(index, body.name),
            color: body.color.map(to_kernel_color),
        })
        .collect();
    Ok(kernel::write_step(&kernel_bodies)?)
}

/// Reads the solids, names and colors of a STEP file through the kernel.
///
/// Shapes are in millimeters whatever unit the file uses, in the kernel's order.
pub fn read_step(bytes: &[u8]) -> Result<ImportedStep, IoError> {
    let import = kernel::read_step(bytes)?;
    let solids = import
        .bodies
        .into_iter()
        .map(|body| ImportedSolid {
            name: body.name,
            color: body.color.map(from_kernel_color),
            shape: body.shape,
        })
        .collect();
    Ok(ImportedStep {
        solids,
        warnings: import.warnings,
    })
}

/// A model color as kernel components in `[0, 1]`.
fn to_kernel_color(color: Color) -> kernel::Color {
    let component = |channel: u8| f64::from(channel) / 255.0;
    kernel::Color {
        red: component(color.red),
        green: component(color.green),
        blue: component(color.blue),
    }
}

/// A kernel color as the nearest 8-bit model color; NaN reads as 0 and values outside `[0, 1]`
/// are clamped.
fn from_kernel_color(color: kernel::Color) -> Color {
    let channel = |component: f64| {
        let component = if component.is_nan() { 0.0 } else { component };
        (component.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    Color::new(
        channel(color.red),
        channel(color.green),
        channel(color.blue),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A kernel color with the same value in every component.
    fn gray(component: f64) -> kernel::Color {
        kernel::Color {
            red: component,
            green: component,
            blue: component,
        }
    }

    #[test]
    fn model_color_converts_to_unit_components() {
        let color = to_kernel_color(Color::new(0, 255, 51));
        assert_eq!(
            (color.red, color.green, color.blue),
            (0.0, 1.0, 51.0 / 255.0)
        );
    }

    #[test]
    fn every_channel_value_survives_the_conversion() {
        for value in 0..=u8::MAX {
            let color = Color::new(value, u8::MAX - value, value / 2);
            assert_eq!(from_kernel_color(to_kernel_color(color)), color);
        }
    }

    #[test]
    fn kernel_color_rounds_to_the_nearest_channel_value() {
        assert_eq!(from_kernel_color(gray(0.5)), Color::new(128, 128, 128));
        assert_eq!(from_kernel_color(gray(0.001)), Color::new(0, 0, 0));
        assert_eq!(from_kernel_color(gray(0.999)), Color::new(255, 255, 255));
    }

    #[test]
    fn kernel_color_outside_the_unit_range_is_clamped() {
        assert_eq!(from_kernel_color(gray(-0.5)), Color::new(0, 0, 0));
        assert_eq!(from_kernel_color(gray(7.0)), Color::new(255, 255, 255));
        assert_eq!(
            from_kernel_color(gray(f64::INFINITY)),
            Color::new(255, 255, 255)
        );
    }

    #[test]
    fn nan_component_reads_as_zero() {
        let color = kernel::Color {
            red: f64::NAN,
            green: 1.0,
            blue: f64::NAN,
        };
        assert_eq!(from_kernel_color(color), Color::new(0, 255, 0));
    }
}
