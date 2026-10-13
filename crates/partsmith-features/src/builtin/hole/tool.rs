//! The tool of one hole: the revolution of its half cross-section, its faces named by the role of
//! the segment that sweeps them (features/hole.md section 3.3 step 4, naming.md section 3.6.8).

use std::f64::consts::TAU;

use partsmith_model::Id;
use partsmith_naming::{FaceSource, Generator, roles};
use partsmith_occt_sys::{
    Axis, PlanarCurve, PlanarLoop, PlanarRegion, Plane, Point, Point2, SweepFace, TaggedCurve,
    Vector, make_profile, revolve,
};
use partsmith_sketch::PointRef;

use crate::builtin::common::face_name;
use crate::error::FeatureError;
use crate::name_shape;
use crate::output::Tool;

/// The role of one segment of a hole's half cross-section, which names the face it sweeps. The
/// discriminant is the tag of the segment's curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Role {
    /// The segment on the axis, which sweeps no face.
    Axis = 0,
    /// The disc closing the tool on the sketch plane.
    Entry = 1,
    /// The counterbore wall.
    Counterbore = 2,
    /// The counterbore's annular shoulder.
    CounterboreFloor = 3,
    /// The countersink cone.
    Countersink = 4,
    /// The wall of the hole diameter.
    Bore = 5,
    /// The flat end of a blind hole.
    Bottom = 6,
    /// The conical drill-point end of a blind hole.
    Tip = 7,
}

impl Role {
    /// Every role in tag order: the tag of a curve is the position of its role here.
    const ALL: [Role; 8] = [
        Role::Axis,
        Role::Entry,
        Role::Counterbore,
        Role::CounterboreFloor,
        Role::Countersink,
        Role::Bore,
        Role::Bottom,
        Role::Tip,
    ];

    /// The tag of curves with this role.
    pub(super) fn tag(self) -> u32 {
        self as u32
    }

    /// The role of a curve tag.
    fn of_tag(tag: u32) -> Option<Role> {
        Role::ALL.get(usize::try_from(tag).ok()?).copied()
    }

    /// The role word of the faces it sweeps; None for the axis, which sweeps none.
    fn word(self) -> Option<&'static str> {
        match self {
            Role::Axis => None,
            Role::Entry => Some(roles::ENTRY),
            Role::Counterbore => Some(roles::COUNTERBORE),
            Role::CounterboreFloor => Some(roles::COUNTERBORE_FLOOR),
            Role::Countersink => Some(roles::COUNTERSINK),
            Role::Bore => Some(roles::BORE),
            Role::Bottom => Some(roles::BOTTOM),
            Role::Tip => Some(roles::TIP),
        }
    }
}

/// Where one hole is drilled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Frame {
    /// The sketch point in world space, on the axis of the hole.
    pub(super) position: Point,
    /// The unit direction of increasing radius in the cross-section: the sketch x axis, so the
    /// seam of the tool lies on the same side for every hole.
    pub(super) radial: Vector,
    /// The unit drilling direction.
    pub(super) direction: Vector,
}

impl Frame {
    /// The plane of the cross-section: through the point, spanned by the radial direction (its x
    /// axis) and the drilling direction (its y axis), so plane coordinates are (radius, depth).
    fn plane(&self) -> Plane {
        Plane {
            origin: self.position,
            normal: self.radial.cross(self.direction),
            x_axis: self.radial,
        }
    }

    /// The axis of the hole: through the point along the drilling direction.
    fn axis(&self) -> Axis {
        Axis {
            origin: self.position,
            direction: self.direction,
        }
    }
}

/// The tool of one hole: the full revolution of its half cross-section about its axis, with every
/// face named `<feature>/<role>/<point>`.
pub(super) fn hole_tool(
    feature: &Id,
    point: &PointRef,
    frame: &Frame,
    section: &[(Role, [f64; 2])],
) -> Result<Tool, FeatureError> {
    let mut curves = Vec::with_capacity(section.len());
    let mut start = [0.0, 0.0];
    for &(role, end) in section {
        curves.push(TaggedCurve {
            tag: role.tag(),
            curve: PlanarCurve::Line {
                start: plane_point(start),
                end: plane_point(end),
            },
        });
        start = end;
    }
    let region = PlanarRegion {
        outer: PlanarLoop { curves },
        holes: Vec::new(),
    };
    let profile = make_profile(&frame.plane(), &[region])?;
    let sweep = revolve(&profile, &frame.axis(), TAU)?;
    let point_text = point.to_string();
    let faces = sweep
        .faces
        .iter()
        .map(|face| {
            let word = match face {
                SweepFace::Lateral { tag } => Role::of_tag(*tag).and_then(Role::word),
                SweepFace::StartCap | SweepFace::EndCap => None,
            }
            .ok_or_else(|| {
                FeatureError::internal(format!("the hole tool has a face without a role: {face:?}"))
            })?;
            face_name(feature, word, Some(Generator::Sketch(&point_text))).map(FaceSource::New)
        })
        .collect::<Result<Vec<_>, FeatureError>>()?;
    let naming = name_shape(&sweep.shape, faces, &[])?.naming;
    Ok(Tool {
        shape: sweep.shape,
        naming,
    })
}

/// The kernel point of plane coordinates.
fn plane_point([x, y]: [f64; 2]) -> Point2 {
    Point2 { x, y }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_round_trip_through_their_tags() {
        for role in Role::ALL {
            assert_eq!(Role::of_tag(role.tag()), Some(role));
        }
        assert_eq!(Role::of_tag(8), None);
    }

    #[test]
    fn the_cross_section_plane_has_depth_along_its_y_axis() {
        let frame = Frame {
            position: Point::new(1.0, 2.0, 3.0),
            radial: Vector::new(1.0, 0.0, 0.0),
            direction: Vector::new(0.0, 0.0, -1.0),
        };

        let plane = frame.plane();

        assert_eq!(plane.y_axis(), frame.direction);
        assert_eq!(plane.origin, frame.position);
    }
}
