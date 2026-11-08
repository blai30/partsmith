//! The union of regions of one profile (spec section 3.19).

#[cfg(test)]
mod tests {
    use super::merge_pieces;
    use crate::{Id, ProfileCurve, ProfileEdge};

    fn piece(entity: &str, reversed: bool, curve: ProfileCurve) -> ProfileEdge {
        ProfileEdge {
            entity: Id::new(entity).expect("valid id"),
            reversed,
            curve,
        }
    }

    fn segment(entity: &str, start: [f64; 2], end: [f64; 2]) -> ProfileEdge {
        piece(entity, false, ProfileCurve::Line { start, end })
    }

    /// A counterclockwise arc of the unit circle around the origin.
    fn unit_arc(entity: &str, start: [f64; 2], end: [f64; 2]) -> ProfileEdge {
        piece(
            entity,
            false,
            ProfileCurve::Arc {
                center: [0.0, 0.0],
                radius: 1.0,
                start,
                end,
                ccw: true,
            },
        )
    }

    #[test]
    fn a_loop_without_continuing_pieces_is_unchanged() {
        let edges = vec![
            segment("e-1", [0.0, 0.0], [1.0, 0.0]),
            segment("e-2", [1.0, 0.0], [0.0, 1.0]),
            segment("e-3", [0.0, 1.0], [0.0, 0.0]),
        ];

        assert_eq!(merge_pieces(edges.clone()), edges);
    }

    #[test]
    fn consecutive_pieces_of_a_line_become_one_segment() {
        let edges = vec![
            segment("e-1", [0.0, 0.0], [1.0, 0.0]),
            segment("e-1", [1.0, 0.0], [2.0, 0.0]),
            segment("e-2", [2.0, 0.0], [0.0, 1.0]),
            segment("e-3", [0.0, 1.0], [0.0, 0.0]),
        ];

        assert_eq!(
            merge_pieces(edges),
            [
                segment("e-1", [0.0, 0.0], [2.0, 0.0]),
                segment("e-2", [2.0, 0.0], [0.0, 1.0]),
                segment("e-3", [0.0, 1.0], [0.0, 0.0]),
            ]
        );
    }

    #[test]
    fn the_last_piece_merges_with_the_first() {
        let edges = vec![
            segment("e-1", [1.0, 0.0], [2.0, 0.0]),
            segment("e-2", [2.0, 0.0], [0.0, 1.0]),
            segment("e-3", [0.0, 1.0], [0.0, 0.0]),
            segment("e-1", [0.0, 0.0], [1.0, 0.0]),
        ];

        assert_eq!(
            merge_pieces(edges),
            [
                segment("e-2", [2.0, 0.0], [0.0, 1.0]),
                segment("e-3", [0.0, 1.0], [0.0, 0.0]),
                segment("e-1", [0.0, 0.0], [2.0, 0.0]),
            ]
        );
    }

    #[test]
    fn arcs_of_one_entity_merge_and_close_into_a_circle() {
        let edges = vec![
            unit_arc("e-5", [1.0, 0.0], [0.0, 1.0]),
            unit_arc("e-5", [0.0, 1.0], [-1.0, 0.0]),
            unit_arc("e-5", [-1.0, 0.0], [1.0, 0.0]),
        ];

        assert_eq!(
            merge_pieces(edges),
            [piece(
                "e-5",
                false,
                ProfileCurve::Circle {
                    center: [0.0, 0.0],
                    radius: 1.0,
                    ccw: true,
                }
            )]
        );
    }

    #[test]
    fn a_run_of_arcs_never_closes_inside_a_longer_loop() {
        let edges = vec![
            segment("e-1", [2.0, 0.0], [1.0, 0.0]),
            unit_arc("e-5", [1.0, 0.0], [0.0, 1.0]),
            unit_arc("e-5", [0.0, 1.0], [-1.0, 0.0]),
            unit_arc("e-5", [-1.0, 0.0], [1.0, 0.0]),
            segment("e-2", [1.0, 0.0], [2.0, 0.0]),
        ];

        assert_eq!(
            merge_pieces(edges),
            [
                segment("e-1", [2.0, 0.0], [1.0, 0.0]),
                unit_arc("e-5", [1.0, 0.0], [-1.0, 0.0]),
                unit_arc("e-5", [-1.0, 0.0], [1.0, 0.0]),
                segment("e-2", [1.0, 0.0], [2.0, 0.0]),
            ]
        );
    }

    #[test]
    fn consecutive_arcs_of_one_entity_become_one_arc() {
        let edges = vec![
            unit_arc("e-5", [1.0, 0.0], [0.0, 1.0]),
            unit_arc("e-5", [0.0, 1.0], [-1.0, 0.0]),
            segment("e-6", [-1.0, 0.0], [1.0, 0.0]),
        ];

        assert_eq!(
            merge_pieces(edges),
            [
                unit_arc("e-5", [1.0, 0.0], [-1.0, 0.0]),
                segment("e-6", [-1.0, 0.0], [1.0, 0.0]),
            ]
        );
    }

    #[test]
    fn pieces_of_other_entities_or_directions_stay_separate() {
        let edges = vec![
            segment("e-1", [0.0, 0.0], [1.0, 0.0]),
            segment("e-2", [1.0, 0.0], [2.0, 0.0]),
            piece(
                "e-2",
                true,
                ProfileCurve::Line {
                    start: [2.0, 0.0],
                    end: [3.0, 0.0],
                },
            ),
            segment("e-3", [3.0, 0.0], [0.0, 0.0]),
        ];

        assert_eq!(merge_pieces(edges.clone()), edges);
    }
}
