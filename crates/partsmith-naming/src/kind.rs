//! Element kinds and geometry kinds (naming.md section 2.1).

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The topological kind of an element.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ElementKind {
    /// A face.
    Face,
    /// An edge.
    Edge,
    /// A vertex.
    Vertex,
}

impl fmt::Display for ElementKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            ElementKind::Face => "face",
            ElementKind::Edge => "edge",
            ElementKind::Vertex => "vertex",
        })
    }
}

/// The geometric kind of an element, as written at the start of a fingerprint.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum GeometryKind {
    /// Planar face.
    Plane,
    /// Cylindrical face.
    Cylinder,
    /// Conical face.
    Cone,
    /// Spherical face.
    Sphere,
    /// Toroidal face.
    Torus,
    /// Surface of revolution that is not one of the kinds above.
    Revolution,
    /// Surface of linear extrusion that is not one of the kinds above.
    Extrusion,
    /// B-spline surface.
    BsplineSurface,
    /// Bezier surface.
    BezierSurface,
    /// Offset surface.
    OffsetSurface,
    /// Any other surface.
    OtherSurface,
    /// Straight edge.
    Line,
    /// Circular edge (full circle or arc).
    Circle,
    /// Elliptical edge.
    Ellipse,
    /// Hyperbolic edge.
    Hyperbola,
    /// Parabolic edge.
    Parabola,
    /// B-spline edge.
    BsplineCurve,
    /// Bezier edge.
    BezierCurve,
    /// Offset curve edge.
    OffsetCurve,
    /// Any other curve, including degenerated edges.
    OtherCurve,
    /// A vertex.
    Point,
}

impl GeometryKind {
    /// Every geometry kind, in declaration order.
    pub(crate) const ALL: [GeometryKind; 21] = [
        GeometryKind::Plane,
        GeometryKind::Cylinder,
        GeometryKind::Cone,
        GeometryKind::Sphere,
        GeometryKind::Torus,
        GeometryKind::Revolution,
        GeometryKind::Extrusion,
        GeometryKind::BsplineSurface,
        GeometryKind::BezierSurface,
        GeometryKind::OffsetSurface,
        GeometryKind::OtherSurface,
        GeometryKind::Line,
        GeometryKind::Circle,
        GeometryKind::Ellipse,
        GeometryKind::Hyperbola,
        GeometryKind::Parabola,
        GeometryKind::BsplineCurve,
        GeometryKind::BezierCurve,
        GeometryKind::OffsetCurve,
        GeometryKind::OtherCurve,
        GeometryKind::Point,
    ];

    /// The element kind this geometry kind belongs to (surfaces are faces, curves are edges, `Point` is a vertex).
    pub fn element_kind(self) -> ElementKind {
        match self {
            GeometryKind::Plane
            | GeometryKind::Cylinder
            | GeometryKind::Cone
            | GeometryKind::Sphere
            | GeometryKind::Torus
            | GeometryKind::Revolution
            | GeometryKind::Extrusion
            | GeometryKind::BsplineSurface
            | GeometryKind::BezierSurface
            | GeometryKind::OffsetSurface
            | GeometryKind::OtherSurface => ElementKind::Face,
            GeometryKind::Line
            | GeometryKind::Circle
            | GeometryKind::Ellipse
            | GeometryKind::Hyperbola
            | GeometryKind::Parabola
            | GeometryKind::BsplineCurve
            | GeometryKind::BezierCurve
            | GeometryKind::OffsetCurve
            | GeometryKind::OtherCurve => ElementKind::Edge,
            GeometryKind::Point => ElementKind::Vertex,
        }
    }

    /// The token used in fingerprint text, for example `bspline_surface`.
    pub fn token(self) -> &'static str {
        match self {
            GeometryKind::Plane => "plane",
            GeometryKind::Cylinder => "cylinder",
            GeometryKind::Cone => "cone",
            GeometryKind::Sphere => "sphere",
            GeometryKind::Torus => "torus",
            GeometryKind::Revolution => "revolution",
            GeometryKind::Extrusion => "extrusion",
            GeometryKind::BsplineSurface => "bspline_surface",
            GeometryKind::BezierSurface => "bezier_surface",
            GeometryKind::OffsetSurface => "offset_surface",
            GeometryKind::OtherSurface => "other_surface",
            GeometryKind::Line => "line",
            GeometryKind::Circle => "circle",
            GeometryKind::Ellipse => "ellipse",
            GeometryKind::Hyperbola => "hyperbola",
            GeometryKind::Parabola => "parabola",
            GeometryKind::BsplineCurve => "bspline_curve",
            GeometryKind::BezierCurve => "bezier_curve",
            GeometryKind::OffsetCurve => "offset_curve",
            GeometryKind::OtherCurve => "other_curve",
            GeometryKind::Point => "point",
        }
    }

    /// The kind whose token is `token`, if any.
    pub(crate) fn from_token(token: &str) -> Option<GeometryKind> {
        GeometryKind::ALL
            .into_iter()
            .find(|kind| kind.token() == token)
    }

    /// True when a fingerprint of this kind carries a direction (naming.md section 3.7.1).
    pub(crate) fn has_direction(self) -> bool {
        matches!(
            self,
            GeometryKind::Plane
                | GeometryKind::Cylinder
                | GeometryKind::Cone
                | GeometryKind::Torus
                | GeometryKind::Revolution
                | GeometryKind::Extrusion
                | GeometryKind::Line
                | GeometryKind::Circle
                | GeometryKind::Ellipse
                | GeometryKind::Hyperbola
                | GeometryKind::Parabola
        )
    }

    /// True when `Fingerprint::new` needs a direction for this kind.
    pub(crate) fn requires_direction(self) -> bool {
        matches!(self, GeometryKind::Plane | GeometryKind::Line)
    }

    /// True when the direction is a signed normal rather than an axis, so it is never canonicalized.
    pub(crate) fn keeps_direction_sign(self) -> bool {
        self == GeometryKind::Plane
    }

    /// True when a fingerprint of this kind carries an area or length.
    pub(crate) fn has_size(self) -> bool {
        self.element_kind() != ElementKind::Vertex
    }

    /// True when a fingerprint of this kind carries a radius.
    pub(crate) fn has_radius(self) -> bool {
        matches!(
            self,
            GeometryKind::Cylinder
                | GeometryKind::Sphere
                | GeometryKind::Torus
                | GeometryKind::Circle
        )
    }
}

impl fmt::Display for GeometryKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.token())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_kind_displays_as_lowercase_word() {
        assert_eq!(ElementKind::Face.to_string(), "face");
        assert_eq!(ElementKind::Edge.to_string(), "edge");
        assert_eq!(ElementKind::Vertex.to_string(), "vertex");
    }

    #[test]
    fn element_kind_serializes_as_snake_case() {
        let json = serde_json::to_string(&ElementKind::Vertex).expect("a kind serializes");

        assert_eq!(json, "\"vertex\"");
    }

    #[test]
    fn geometry_kind_displays_as_token() {
        assert_eq!(GeometryKind::BsplineSurface.to_string(), "bspline_surface");
    }

    #[test]
    fn geometry_kind_json_matches_token() {
        for kind in GeometryKind::ALL {
            let json = serde_json::to_string(&kind).expect("a kind serializes");

            assert_eq!(json, format!("\"{}\"", kind.token()));
        }
    }

    #[test]
    fn tokens_are_distinct() {
        let mut tokens: Vec<&str> = GeometryKind::ALL.iter().map(|kind| kind.token()).collect();
        tokens.sort_unstable();
        tokens.dedup();

        assert_eq!(tokens.len(), GeometryKind::ALL.len());
    }

    #[test]
    fn from_token_inverts_token() {
        for kind in GeometryKind::ALL {
            assert_eq!(GeometryKind::from_token(kind.token()), Some(kind));
        }
    }

    #[test]
    fn from_token_rejects_unknown_token() {
        assert_eq!(GeometryKind::from_token("plain"), None);
    }

    #[test]
    fn surfaces_are_faces_curves_are_edges_points_are_vertices() {
        assert_eq!(GeometryKind::OtherSurface.element_kind(), ElementKind::Face);
        assert_eq!(GeometryKind::OtherCurve.element_kind(), ElementKind::Edge);
        assert_eq!(GeometryKind::Point.element_kind(), ElementKind::Vertex);
    }

    #[test]
    fn plane_and_line_require_direction() {
        let required: Vec<GeometryKind> = GeometryKind::ALL
            .into_iter()
            .filter(|kind| kind.requires_direction())
            .collect();

        assert_eq!(required, [GeometryKind::Plane, GeometryKind::Line]);
    }

    #[test]
    fn sphere_and_free_form_kinds_have_no_direction() {
        for kind in [
            GeometryKind::Sphere,
            GeometryKind::BsplineSurface,
            GeometryKind::OtherCurve,
            GeometryKind::Point,
        ] {
            assert!(!kind.has_direction(), "{kind}");
        }
    }

    #[test]
    fn only_points_have_no_size() {
        let sizeless: Vec<GeometryKind> = GeometryKind::ALL
            .into_iter()
            .filter(|kind| !kind.has_size())
            .collect();

        assert_eq!(sizeless, [GeometryKind::Point]);
    }

    #[test]
    fn radius_kinds_are_cylinder_sphere_torus_and_circle() {
        let with_radius: Vec<GeometryKind> = GeometryKind::ALL
            .into_iter()
            .filter(|kind| kind.has_radius())
            .collect();

        assert_eq!(
            with_radius,
            [
                GeometryKind::Cylinder,
                GeometryKind::Sphere,
                GeometryKind::Torus,
                GeometryKind::Circle
            ]
        );
    }

    #[test]
    fn only_planes_keep_direction_sign() {
        assert!(GeometryKind::Plane.keeps_direction_sign());
        assert!(!GeometryKind::Cylinder.keeps_direction_sign());
        assert!(!GeometryKind::Line.keeps_direction_sign());
    }
}
