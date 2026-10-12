//! Param types shared by several features, and placement helpers (features.md section 2.5).

use partsmith_model::Id;
use partsmith_naming::Reference;
use partsmith_occt_sys::{Plane, Point, Vector};
use partsmith_sketch::{OriginPlane, Placement, RegionRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How a solid feature combines its tool shape with its target body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BodyOp {
    /// Start the body. The body must be empty at this point.
    New,
    /// Add the tool to the body.
    Join,
    /// Remove the tool from the body.
    Cut,
    /// Keep only the overlap of body and tool.
    Intersect,
}

/// Where a mirror or pattern puts its copies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CopyOp {
    /// Fuse the copies into the source body (default).
    #[default]
    Join,
    /// Put the copies into the empty `target` body and leave the source unchanged.
    New,
}

impl CopyOp {
    /// True for the default, `Join`; for `skip_serializing_if`.
    pub fn is_default(&self) -> bool {
        *self == CopyOp::Join
    }
}

/// Closed regions of one sketch used as a profile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfileRef {
    /// The sketch feature.
    pub sketch: Id,
    /// The regions, at least one, in pick order (sketch.md `RegionRef`: key text and fingerprint text).
    pub regions: Vec<RegionRef>,
}

/// A plane given by an origin plane, a datum plane, or a flat face of a body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlaneRef {
    /// One of the three origin planes (sketch.md `OriginPlane`, serialized "XY", "XZ", "YZ").
    Origin {
        /// Which origin plane.
        plane: OriginPlane,
    },
    /// A datum plane feature.
    Datum {
        /// The datum feature.
        feature: Id,
    },
    /// A flat face of a body, resolved at this point in the timeline.
    Face {
        /// The body the face is resolved in.
        body: Id,
        /// The face.
        face: Reference,
    },
}

impl Default for PlaneRef {
    /// The origin plane XY, the default plane of primitives (section 3.14).
    fn default() -> PlaneRef {
        PlaneRef::Origin {
            plane: OriginPlane::Xy,
        }
    }
}

impl PlaneRef {
    /// True for the default, origin XY; for `skip_serializing_if`.
    pub fn is_default(&self) -> bool {
        *self == PlaneRef::default()
    }
}

/// An axis given by an origin axis, a sketch line, a straight edge, or a round face.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AxisRef {
    /// One of the three origin axes, through the origin.
    Origin {
        /// Which origin axis.
        axis: OriginAxis,
    },
    /// A line entity of a sketch (construction lines allowed), directed from its start to its end.
    SketchLine {
        /// The sketch feature.
        sketch: Id,
        /// The line entity.
        line: Id,
    },
    /// A straight edge of a body.
    Edge {
        /// The body the edge is resolved in.
        body: Id,
        /// The edge.
        edge: Reference,
    },
    /// The axis of a cylindrical, conical, spherical or toroidal face of a body.
    Face {
        /// The body the face is resolved in.
        body: Id,
        /// The face.
        face: Reference,
    },
}

/// The origin axes. Serialized "X", "Y", "Z".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum OriginAxis {
    /// The x axis.
    X,
    /// The y axis.
    Y,
    /// The z axis.
    Z,
}

impl OriginAxis {
    /// The unit direction of the axis.
    pub fn direction(self) -> [f64; 3] {
        match self {
            OriginAxis::X => [1.0, 0.0, 0.0],
            OriginAxis::Y => [0.0, 1.0, 0.0],
            OriginAxis::Z => [0.0, 0.0, 1.0],
        }
    }
}

/// Helpers on the sketch crate's `Placement` (origin, x_axis, normal; y_axis = normal × x_axis).
pub trait PlacementExt {
    /// The same placement moved by `distance` along its normal.
    fn offset(&self, distance: f64) -> Placement;
    /// The same origin and x-axis with the normal reversed (so the y-axis reverses too).
    fn flipped(&self) -> Placement;
    /// World position of local coordinates (x along x_axis, y along y_axis, z along normal).
    fn local_to_world(&self, local: [f64; 3]) -> [f64; 3];
    /// The kernel `Plane` with the same origin, normal and x-axis.
    fn to_kernel(&self) -> Plane;
}

impl PlacementExt for Placement {
    fn offset(&self, distance: f64) -> Placement {
        Placement {
            origin: self.local_to_world([0.0, 0.0, distance]),
            ..*self
        }
    }

    fn flipped(&self) -> Placement {
        Placement {
            normal: self.normal.map(|value| -value),
            ..*self
        }
    }

    fn local_to_world(&self, local: [f64; 3]) -> [f64; 3] {
        let y_axis = self.y_axis();
        [0, 1, 2].map(|i| {
            self.origin[i]
                + local[0] * self.x_axis[i]
                + local[1] * y_axis[i]
                + local[2] * self.normal[i]
        })
    }

    fn to_kernel(&self) -> Plane {
        Plane {
            origin: point(self.origin),
            normal: vector(self.normal),
            x_axis: vector(self.x_axis),
        }
    }
}

/// The kernel point at world coordinates `[x, y, z]`.
pub fn point([x, y, z]: [f64; 3]) -> Point {
    Point::new(x, y, z)
}

/// The kernel vector with world components `[x, y, z]`.
pub fn vector([x, y, z]: [f64; 3]) -> Vector {
    Vector::new(x, y, z)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn body_ops_are_snake_case() {
        assert_eq!(
            serde_json::to_value(BodyOp::Intersect).expect("serializes"),
            json!("intersect")
        );
    }

    #[test]
    fn copy_op_defaults_to_join() {
        assert_eq!(CopyOp::default(), CopyOp::Join);
        assert!(CopyOp::Join.is_default());
        assert!(!CopyOp::New.is_default());
    }

    #[test]
    fn plane_refs_are_tagged_with_kind() {
        let plane: PlaneRef = serde_json::from_value(json!({"kind": "origin", "plane": "XZ"}))
            .expect("an origin plane deserializes");

        assert_eq!(
            plane,
            PlaneRef::Origin {
                plane: OriginPlane::Xz
            }
        );
    }

    #[test]
    fn plane_refs_reject_unknown_keys() {
        let result: Result<PlaneRef, _> =
            serde_json::from_value(json!({"kind": "datum", "feature": "f-dp1", "offset": "1"}));

        assert!(result.is_err());
    }

    #[test]
    fn default_plane_is_origin_xy() {
        assert!(PlaneRef::default().is_default());
        assert!(
            !PlaneRef::Origin {
                plane: OriginPlane::Yz
            }
            .is_default()
        );
    }

    #[test]
    fn axis_refs_use_snake_case_kinds() {
        let axis = AxisRef::SketchLine {
            sketch: Id::new("f-sk1").expect("valid id"),
            line: Id::new("e-9").expect("valid id"),
        };

        assert_eq!(
            serde_json::to_value(axis).expect("serializes"),
            json!({"kind": "sketch_line", "sketch": "f-sk1", "line": "e-9"})
        );
    }

    #[test]
    fn origin_axes_are_uppercase_letters() {
        assert_eq!(
            serde_json::to_value(OriginAxis::Y).expect("serializes"),
            json!("Y")
        );
        assert_eq!(OriginAxis::Y.direction(), [0.0, 1.0, 0.0]);
    }

    #[test]
    fn profile_refs_keep_sketch_then_regions() {
        let profile: ProfileRef = serde_json::from_value(json!({
            "regions": [{"fp": "c=40,25|a=4000", "key": "e-1,e-2,e-3,e-4"}],
            "sketch": "f-sk1"
        }))
        .expect("a profile deserializes");

        assert_eq!(
            serde_json::to_string(&profile).expect("serializes"),
            r#"{"sketch":"f-sk1","regions":[{"key":"e-1,e-2,e-3,e-4","fp":"c=40,25|a=4000"}]}"#
        );
    }

    #[test]
    fn offset_moves_along_the_normal() {
        let placement = OriginPlane::Xz.placement().offset(5.0);

        assert_eq!(placement.origin, [0.0, -5.0, 0.0]);
        assert_eq!(placement.normal, [0.0, -1.0, 0.0]);
    }

    #[test]
    fn flipped_keeps_origin_and_x_axis() {
        let placement = OriginPlane::Yz.placement().flipped();

        assert_eq!(placement.origin, [0.0, 0.0, 0.0]);
        assert_eq!(placement.x_axis, [0.0, 1.0, 0.0]);
        assert_eq!(placement.normal, [-1.0, 0.0, 0.0]);
    }

    #[test]
    fn local_to_world_of_xy_is_the_identity() {
        let world = OriginPlane::Xy.placement().local_to_world([1.0, 2.0, 3.0]);

        assert_eq!(world, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn to_kernel_of_an_origin_plane_is_the_kernel_constant() {
        assert_eq!(OriginPlane::Xy.placement().to_kernel(), Plane::XY);
        assert_eq!(OriginPlane::Xz.placement().to_kernel(), Plane::XZ);
        assert_eq!(OriginPlane::Yz.placement().to_kernel(), Plane::YZ);
    }

    #[test]
    fn points_take_coordinates_in_x_y_z_order() {
        assert_eq!(point([1.0, -2.0, 3.5]), Point::new(1.0, -2.0, 3.5));
    }

    #[test]
    fn vectors_take_components_in_x_y_z_order() {
        assert_eq!(vector([0.0, 0.5, -1.0]), Vector::new(0.0, 0.5, -1.0));
    }
}
