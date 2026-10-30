//! Shapes the unit tests of several modules share.

use crate::{
    PlanarCurve, PlanarLoop, PlanarRegion, Plane, Point, Point2, Shape, TaggedCurve, make_box,
    make_profile,
};

/// A 10 mm cube with its lowest corner at `corner`.
pub(crate) fn cube_with_corner(corner: Point) -> Shape {
    let placement = Plane {
        origin: corner,
        ..Plane::XY
    };
    make_box(&placement, 10.0, 10.0, 10.0).expect("cube").shape
}

/// A 10 mm cube with its corner at `(x, 0, 0)`.
pub(crate) fn cube_at(x: f64) -> Shape {
    cube_with_corner(Point::new(x, 0.0, 0.0))
}

/// The region bounded by the square with 10 mm sides whose lower left corner is at `(left, 0)`:
/// four lines counterclockwise from that corner, tagged 1 to 4.
pub(crate) fn square_region(left: f64) -> PlanarRegion {
    let corners = [
        (left, 0.0),
        (left + 10.0, 0.0),
        (left + 10.0, 10.0),
        (left, 10.0),
    ];
    let curves = (0..corners.len())
        .map(|position| {
            let (start, end) = (corners[position], corners[(position + 1) % corners.len()]);
            TaggedCurve {
                tag: u32::try_from(position + 1).expect("four sides"),
                curve: PlanarCurve::Line {
                    start: Point2 {
                        x: start.0,
                        y: start.1,
                    },
                    end: Point2 { x: end.0, y: end.1 },
                },
            }
        })
        .collect();
    PlanarRegion {
        outer: PlanarLoop { curves },
        holes: Vec::new(),
    }
}

/// A face of `Plane::XY` bounded by `square_region(left)`, a shape without a solid.
pub(crate) fn square_face_at(left: f64) -> Shape {
    make_profile(&Plane::XY, &[square_region(left)])
        .expect("the square builds")
        .shape
}
