//! The reserved references `origin`, `x_axis` and `y_axis` (spec section 3.16).

use crate::point_ref::{PointRef, PointRole};
use crate::{ORIGIN, X_AXIS, Y_AXIS};

/// True for `origin`, `x_axis` and `y_axis`.
pub(crate) fn is_reserved(name: &str) -> bool {
    name == ORIGIN || is_axis(name)
}

/// True for the point reference `origin` (the bare name; `origin` has no other points).
pub(crate) fn is_origin(point: &PointRef) -> bool {
    point.entity.as_str() == ORIGIN && point.role == PointRole::Point
}

/// True for `x_axis` and `y_axis`.
pub(crate) fn is_axis(name: &str) -> bool {
    name == X_AXIS || name == Y_AXIS
}

/// The words messages use for a reserved name ("the origin", "the x axis", "the y axis"), or `None` for any other
/// name (spec section 4).
pub(crate) fn words(name: &str) -> Option<&'static str> {
    match name {
        ORIGIN => Some("the origin"),
        X_AXIS => Some("the x axis"),
        Y_AXIS => Some("the y axis"),
        _ => None,
    }
}

/// The constant start and end of an axis, or `None` for any other name.
pub(crate) fn axis_line(name: &str) -> Option<([f64; 2], [f64; 2])> {
    match name {
        X_AXIS => Some(([0.0, 0.0], [1.0, 0.0])),
        Y_AXIS => Some(([0.0, 0.0], [0.0, 1.0])),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_names() {
        assert!(is_reserved("origin"));
        assert!(is_reserved("y_axis"));
        assert!(!is_reserved("origin2"));
    }

    #[test]
    fn only_the_bare_origin_is_the_origin_point() {
        let point = |text: &str| text.parse::<PointRef>().expect("valid point reference");

        assert!(is_origin(&point("origin")));
        assert!(!is_origin(&point("origin.start")));
        assert!(!is_origin(&point("e-1")));
    }

    #[test]
    fn origin_is_not_an_axis() {
        assert!(!is_axis("origin"));
        assert_eq!(axis_line("origin"), None);
    }

    #[test]
    fn reserved_names_have_words_and_other_names_none() {
        assert_eq!(words("origin"), Some("the origin"));
        assert_eq!(words("x_axis"), Some("the x axis"));
        assert_eq!(words("y_axis"), Some("the y axis"));
        assert_eq!(words("e-1"), None);
    }

    #[test]
    fn axes_run_along_u_and_v() {
        assert_eq!(axis_line("x_axis"), Some(([0.0, 0.0], [1.0, 0.0])));
        assert_eq!(axis_line("y_axis"), Some(([0.0, 0.0], [0.0, 1.0])));
    }
}
