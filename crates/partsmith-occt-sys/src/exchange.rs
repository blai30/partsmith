//! STEP and binary BRep exchange (kernel spec sections 2.12 and 3.8).

use crate::error::{KernelError, Operation, call};
use crate::ffi::bridge as sys;
use crate::shape::{Shape, SubShapeKind};
use crate::values::{Color, check_finite};

/// One body in a STEP file.
#[derive(Debug, Clone)]
pub struct StepBody {
    /// The body's shape (must contain at least one solid when writing).
    pub shape: Shape,
    /// Product name.
    pub name: String,
    /// Surface color, if any.
    pub color: Option<Color>,
}

/// The bodies read from a STEP file, plus warnings for content that was skipped or repaired.
#[derive(Debug, Clone)]
pub struct StepImport {
    /// One body per solid, in file order.
    pub bodies: Vec<StepBody>,
    /// Plain-language notes about skipped or repaired content.
    pub warnings: Vec<String>,
}

const NOTHING_TO_EXPORT_MESSAGE: &str = "There is nothing to export.";
const COLOR_RANGE_MESSAGE: &str = "Color components must be between 0 and 1.";

impl Color {
    fn components(self) -> [f64; 3] {
        [self.red, self.green, self.blue]
    }
}

/// The name written for body `position` (0-based): its own, or `Body <n>` when empty.
fn written_name(position: usize, name: &str) -> String {
    if name.is_empty() {
        format!("Body {}", position + 1)
    } else {
        name.to_owned()
    }
}

/// Writes bodies to an AP214 STEP file in millimeters.
pub fn write_step(bodies: &[StepBody]) -> Result<Vec<u8>, KernelError> {
    let operation = Operation::WriteStep;
    if bodies.is_empty() {
        return Err(KernelError::invalid_input(
            operation,
            NOTHING_TO_EXPORT_MESSAGE,
        ));
    }
    let components: Vec<f64> = bodies
        .iter()
        .filter_map(|body| body.color)
        .flat_map(Color::components)
        .collect();
    check_finite(operation, &components)?;
    if components
        .iter()
        .any(|component| !(0.0..=1.0).contains(component))
    {
        return Err(KernelError::invalid_input(operation, COLOR_RANGE_MESSAGE));
    }
    let mut data = Vec::with_capacity(bodies.len());
    for (position, body) in bodies.iter().enumerate() {
        let name = written_name(position, &body.name);
        if body.shape.count(SubShapeKind::Solid) == 0 {
            return Err(KernelError::invalid_input(
                operation,
                format!("Body \"{name}\" has no solid to export."),
            ));
        }
        let [red, green, blue] = body.color.map_or([0.0; 3], Color::components);
        data.push(sys::StepBodyData {
            shape: body.shape.handle().shape,
            name,
            has_color: body.color.is_some(),
            red,
            green,
            blue,
        });
    }
    let bytes = call(operation, || sys::write_step(&data))?;
    owned_bytes(operation, &bytes)
}

/// Copies bytes returned by the shim; a null string is an `Internal` error.
fn owned_bytes(
    operation: Operation,
    bytes: &cxx::UniquePtr<cxx::CxxString>,
) -> Result<Vec<u8>, KernelError> {
    bytes
        .as_ref()
        .map(|text| text.as_bytes().to_vec())
        .ok_or_else(|| KernelError::internal(operation, "the kernel returned no bytes"))
}

/// Reads solids, names and colors from a STEP file, converted to millimeters.
pub fn read_step(bytes: &[u8]) -> Result<StepImport, KernelError> {
    let operation = Operation::ReadStep;
    let data = call(operation, || sys::read_step(bytes))?;
    let bodies = data
        .bodies
        .into_iter()
        .map(|body| {
            Ok(StepBody {
                shape: Shape::from_ffi(operation, body.shape)?,
                name: body.name,
                color: body.has_color.then_some(Color {
                    red: body.red,
                    green: body.green,
                    blue: body.blue,
                }),
            })
        })
        .collect::<Result<_, KernelError>>()?;
    Ok(StepImport {
        bodies,
        warnings: data.warnings,
    })
}

impl Shape {
    /// Serializes to OCCT binary BRep (section 3.8.2).
    pub fn to_brep(&self) -> Result<Vec<u8>, KernelError> {
        let bytes = call(Operation::WriteBrep, || sys::shape_to_brep(self.inner()))?;
        owned_bytes(Operation::WriteBrep, &bytes)
    }

    /// Deserializes OCCT binary BRep written by `to_brep`.
    pub fn from_brep(bytes: &[u8]) -> Result<Shape, KernelError> {
        let shape = call(Operation::ReadBrep, || sys::shape_from_brep(bytes))?;
        Shape::from_ffi(Operation::ReadBrep, shape)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Plane, Point, Transform, Vector, fuse, make_box, transform};

    /// A 10 mm cube with its corner at `(x, 0, 0)`.
    fn cube_at(x: f64) -> Shape {
        let corner = Plane {
            origin: Point::new(x, 0.0, 0.0),
            ..Plane::XY
        };
        make_box(&corner, 10.0, 10.0, 10.0).expect("cube").shape
    }

    #[test]
    fn empty_names_get_their_position() {
        assert_eq!(written_name(0, ""), "Body 1");
        assert_eq!(written_name(4, ""), "Body 5");
        assert_eq!(written_name(4, "Lid"), "Lid");
    }

    #[test]
    fn body_of_two_solids_reads_as_two_bodies_with_its_name() {
        let pair = fuse(&cube_at(0.0), &[cube_at(20.0)]).expect("fused").shape;
        let bodies = [StepBody {
            shape: pair,
            name: "Pair".to_owned(),
            color: None,
        }];

        let import = read_step(&write_step(&bodies).expect("written")).expect("read");

        assert_eq!(import.bodies.len(), 2);
        assert!(import.bodies.iter().all(|body| body.name == "Pair"));
    }

    #[test]
    fn moved_body_keeps_its_position() {
        let moved = transform(
            &cube_at(0.0),
            &Transform::translation(Vector::new(100.0, 0.0, 0.0)),
        )
        .expect("moved")
        .shape;
        let bodies = [StepBody {
            shape: moved,
            name: "Moved".to_owned(),
            color: None,
        }];

        let import = read_step(&write_step(&bodies).expect("written")).expect("read");

        let bounds = import.bodies[0].shape.bounding_box().expect("bounds");
        assert!((bounds.min[0] - 100.0).abs() < 1e-6);
    }

    #[test]
    fn empty_shape_survives_the_brep_round_trip() {
        let bytes = Shape::empty().to_brep().expect("bytes");

        let reloaded = Shape::from_brep(&bytes).expect("reloaded");

        assert!(reloaded.is_empty());
    }

    #[test]
    fn truncated_brep_bytes_fail_to_read() {
        let bytes = cube_at(0.0).to_brep().expect("bytes");

        let error = Shape::from_brep(&bytes[..bytes.len() / 2]).expect_err("truncated");

        assert_eq!(error.kind, crate::KernelErrorKind::ReadFailed);
    }

    #[test]
    fn empty_body_names_the_default_name() {
        let error = write_step(&[StepBody {
            shape: Shape::empty(),
            name: String::new(),
            color: None,
        }])
        .expect_err("the body has no solid");

        assert_eq!(error.message, "Body \"Body 1\" has no solid to export.");
    }

    #[test]
    fn nan_color_is_not_finite() {
        let error = write_step(&[StepBody {
            shape: Shape::empty(),
            name: "Part".to_owned(),
            color: Some(Color {
                red: f64::NAN,
                green: 0.0,
                blue: 0.0,
            }),
        }])
        .expect_err("NaN is rejected");

        assert_eq!(error.message, crate::values::NOT_FINITE_MESSAGE);
    }

    #[test]
    fn negative_color_is_out_of_range() {
        let error = write_step(&[StepBody {
            shape: Shape::empty(),
            name: "Part".to_owned(),
            color: Some(Color {
                red: 0.5,
                green: -0.1,
                blue: 0.0,
            }),
        }])
        .expect_err("negative components are rejected");

        assert_eq!(error.message, COLOR_RANGE_MESSAGE);
    }
}
