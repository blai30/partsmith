//! Sketch planes and placements (spec sections 2.2 and 3.1).

use partsmith_model::Id;
use partsmith_naming::Reference;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{PlacementProblem, SketchError};
use crate::vector::{MIN_LENGTH, dot, length, scale, subtract, vector_product};

/// Largest deviation from unit length or perpendicularity a valid placement may have.
const FRAME_TOLERANCE: f64 = 1e-9;

/// Shortest projected axis candidate `Placement::on_plane` accepts.
const MIN_AXIS_LENGTH: f64 = 1e-6;

/// Where a sketch lies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[expect(
    clippy::large_enum_variant,
    reason = "one plane exists per sketch, so the space a small variant leaves unused costs nothing, and boxing would change the shape the spec gives"
)]
pub enum SketchPlane {
    /// One of the three origin planes.
    Origin {
        /// Which origin plane.
        plane: OriginPlane,
    },
    /// The plane produced by a datum feature, for example a `datum_plane`.
    Datum {
        /// The datum feature.
        feature: Id,
    },
    /// A planar face of `body`, plus the placement used when the face cannot be resolved and the coordinates it is in.
    Face {
        /// The body the face reference is resolved in.
        body: Id,
        /// The face.
        face: Reference,
        /// The placement recorded when the sketch was attached.
        fallback: Placement,
        /// Which coordinates `fallback` is in; omitted from JSON when `World`.
        #[serde(default, skip_serializing_if = "HintFrame::is_world")]
        fallback_frame: HintFrame,
    },
}

/// Which coordinates a face sketch's `fallback` is in (section 3.1, features.md section 3.6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HintFrame {
    /// World coordinates: the default, and the meaning of every fallback written before file format version 2.
    #[default]
    World,
    /// The coordinates of the face's own feature, before any later Move body or copy transform.
    Body,
}

impl HintFrame {
    /// True for `World`; used to omit the default when saving.
    pub fn is_world(&self) -> bool {
        *self == HintFrame::World
    }
}

/// An origin plane, serialized as "XY", "XZ" or "YZ".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum OriginPlane {
    /// Normal +Z, x axis +X.
    #[serde(rename = "XY")]
    Xy,
    /// Normal -Y, x axis +X, so sketch up is +Z.
    #[serde(rename = "XZ")]
    Xz,
    /// Normal +X, x axis +Y, so sketch up is +Z.
    #[serde(rename = "YZ")]
    Yz,
}

impl OriginPlane {
    /// The fixed placement of this plane (section 3.1).
    pub fn placement(self) -> Placement {
        let (x_axis, normal) = match self {
            OriginPlane::Xy => ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            OriginPlane::Xz => ([1.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
            OriginPlane::Yz => ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
        };
        Placement {
            origin: [0.0, 0.0, 0.0],
            x_axis,
            normal,
        }
    }
}

/// A right-handed frame that places sketch coordinates in model space.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    /// Model-space position (mm) of sketch point (0, 0).
    pub origin: [f64; 3],
    /// Unit model-space direction of sketch +u.
    pub x_axis: [f64; 3],
    /// Unit normal of the sketch plane; sketch +v is `normal x x_axis`.
    pub normal: [f64; 3],
}

impl Placement {
    /// Unit direction of sketch +v, `normal x x_axis`.
    pub fn y_axis(&self) -> [f64; 3] {
        vector_product(self.normal, self.x_axis)
    }

    /// Model-space point of sketch coordinates `(u, v)`: `origin + u * x_axis + v * y_axis`.
    pub fn to_world(&self, point: [f64; 2]) -> [f64; 3] {
        let y_axis = self.y_axis();
        [0, 1, 2].map(|i| self.origin[i] + point[0] * self.x_axis[i] + point[1] * y_axis[i])
    }

    /// Checks that all values are finite and the axes are unit length and perpendicular within 1e-9.
    pub fn validate(&self) -> Result<(), SketchError> {
        let values = self.origin.iter().chain(&self.x_axis).chain(&self.normal);
        if !values.into_iter().all(|value| value.is_finite()) {
            return Err(invalid(PlacementProblem::NonFinite));
        }
        if (length(self.normal) - 1.0).abs() > FRAME_TOLERANCE {
            return Err(invalid(PlacementProblem::NormalNotUnit));
        }
        if (length(self.x_axis) - 1.0).abs() > FRAME_TOLERANCE {
            return Err(invalid(PlacementProblem::XAxisNotUnit));
        }
        if dot(self.x_axis, self.normal).abs() > FRAME_TOLERANCE {
            return Err(invalid(PlacementProblem::NotPerpendicular));
        }
        Ok(())
    }

    /// The placement on a resolved plane, keeping origin and x axis as close to `hint` as possible (section 3.1).
    pub fn on_plane(
        plane_point: [f64; 3],
        plane_normal: [f64; 3],
        hint: &Placement,
    ) -> Result<Placement, SketchError> {
        let inputs = plane_point
            .iter()
            .chain(&plane_normal)
            .chain(&hint.origin)
            .chain(&hint.x_axis)
            .chain(&hint.normal);
        if !inputs.into_iter().all(|value| value.is_finite()) {
            return Err(invalid(PlacementProblem::NonFinite));
        }
        // A shorter normal has no direction.
        let normal_length = length(plane_normal);
        if normal_length < MIN_LENGTH {
            return Err(invalid(PlacementProblem::ZeroNormal));
        }
        let normal = scale(plane_normal, 1.0 / normal_length);
        let height = dot(subtract(hint.origin, plane_point), normal);
        let origin = subtract(hint.origin, scale(normal, height));
        let candidates = [hint.x_axis, hint.y_axis(), [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let x_axis = candidates
            .into_iter()
            .map(|candidate| subtract(candidate, scale(normal, dot(candidate, normal))))
            .find(|projected| length(*projected) >= MIN_AXIS_LENGTH)
            .map(|projected| scale(projected, 1.0 / length(projected)))
            .expect("a unit normal is perpendicular to at most one of +X and +Y, so one of them projects to a usable axis");
        Ok(Placement {
            origin,
            x_axis,
            normal,
        })
    }
}

/// The error of a placement with this problem.
fn invalid(reason: PlacementProblem) -> SketchError {
    SketchError::InvalidPlacement { reason }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn y_axis_completes_a_right_handed_frame() {
        assert_eq!(OriginPlane::Xz.placement().y_axis(), [0.0, 0.0, 1.0]);
    }

    #[test]
    fn non_finite_origin_is_invalid() {
        let placement = Placement {
            origin: [f64::INFINITY, 0.0, 0.0],
            ..OriginPlane::Xy.placement()
        };

        assert_eq!(
            placement.validate(),
            Err(SketchError::InvalidPlacement {
                reason: PlacementProblem::NonFinite
            })
        );
    }

    #[test]
    fn degenerate_hint_axes_fall_back_to_world_x() {
        let hint = Placement {
            origin: [0.0, 0.0, 0.0],
            x_axis: [0.0, 0.0, 1.0],
            normal: [0.0, 0.0, 1.0],
        };

        let placement =
            Placement::on_plane([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], &hint).expect("valid plane");

        assert_eq!(placement.x_axis, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn world_y_is_the_last_candidate() {
        let hint = Placement {
            origin: [0.0, 0.0, 0.0],
            x_axis: [1.0, 0.0, 0.0],
            normal: [1.0, 0.0, 0.0],
        };

        let placement =
            Placement::on_plane([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], &hint).expect("valid plane");

        assert_eq!(placement.x_axis, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn non_finite_hint_is_rejected() {
        let hint = Placement {
            normal: [f64::NAN, 0.0, 1.0],
            ..OriginPlane::Xy.placement()
        };

        assert_eq!(
            Placement::on_plane([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], &hint),
            Err(SketchError::InvalidPlacement {
                reason: PlacementProblem::NonFinite
            })
        );
    }
}
