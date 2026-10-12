//! The reserved references `origin`, `x_axis` and `y_axis` (spec section 3.16).

use crate::{ORIGIN, X_AXIS, Y_AXIS};

/// True for `origin`, `x_axis` and `y_axis`.
pub(crate) fn is_reserved(name: &str) -> bool {
    name == ORIGIN || is_axis(name)
}

/// True for `x_axis` and `y_axis`.
pub(crate) fn is_axis(name: &str) -> bool {
    name == X_AXIS || name == Y_AXIS
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
    fn origin_is_not_an_axis() {
        assert!(!is_axis("origin"));
        assert_eq!(axis_line("origin"), None);
    }

    #[test]
    fn axes_run_along_u_and_v() {
        assert_eq!(axis_line("x_axis"), Some(([0.0, 0.0], [1.0, 0.0])));
        assert_eq!(axis_line("y_axis"), Some(([0.0, 0.0], [0.0, 1.0])));
    }
}
