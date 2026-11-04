//! Frames on the model (ADR 0023): placements and directions in body coordinates, which a
//! descriptor's `fallback` and `axis_direction` are in (engine.md section 3.9), the face plane a
//! drag commit, a fix or a re-attach by pick stores, in the coordinates that keep the sketch's
//! motion (sections 3.6 and 3.22), and the placement of a sketch on a flat face.

use partsmith_features::{BodyState, PlacementExt};
use partsmith_model::Id;
use partsmith_naming::{Motion, Reference, Resolution, resolve};
use partsmith_occt_sys::{GeometryKind, SubShapeKind};
use partsmith_sketch::{HintFrame, Placement, SketchPlane};

/// The motion that undoes `motion`, which takes a placement or a direction to body coordinates;
/// `None` when `motion` moves nothing, so that nothing needs undoing.
pub(crate) fn undoing(motion: &Motion) -> Option<Motion> {
    (*motion != Motion::IDENTITY).then(|| motion.inverse())
}

/// A placement in body coordinates: `motion`, which moved the face it lies on, undone (its
/// origin as a point, its axes as directions).
pub(crate) fn placement_in_body(placement: &Placement, motion: &Motion) -> Placement {
    undoing(motion).map_or(*placement, |undo| placement.moved(&undo))
}

/// The motion of a face sketch (features/sketch.md section 3.3), which the faces built from it
/// inherit: `face_motion`, the motion of its stored face in the state just before it, when its
/// fallback is in body coordinates, and the identity for world coordinates.
pub(crate) fn sketch_motion(frame: HintFrame, face_motion: &Motion) -> Motion {
    match frame {
        HintFrame::Body => *face_motion,
        HintFrame::World => Motion::IDENTITY,
    }
}

/// The largest difference of one component of two motions that match (engine.md section 3.22):
/// a matrix entry, or a coordinate of the translation in mm.
const MOTION_TOLERANCE: f64 = 1e-9;

/// True when each of the 12 components of the two motions differs by at most
/// `MOTION_TOLERANCE` (engine.md section 3.22). A datum's motion is computed from its frames and
/// can differ from its face's motion by rounding.
pub(crate) fn motions_match(first: &Motion, second: &Motion) -> bool {
    components(first)
        .zip(components(second))
        .all(|(one, other)| (one - other).abs() <= MOTION_TOLERANCE)
}

/// True when `motion` matches the identity (`motions_match`): a sketch with this motion has none.
pub(crate) fn is_identity(motion: &Motion) -> bool {
    motions_match(motion, &Motion::IDENTITY)
}

/// The 12 components of a motion: the axes its matrix turns the unit axes to, then its
/// translation, where it moves the origin.
fn components(motion: &Motion) -> impl Iterator<Item = f64> {
    let axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
        .map(|axis| motion.apply_direction(axis));
    axes.into_iter()
        .chain([motion.apply_point([0.0; 3])])
        .flatten()
}

/// True when a face sketch whose fallback is in `frame`, on a face whose motion is
/// `face_motion`, has the motion `kept` (`sketch_motion`, compared by `motions_match`).
pub(crate) fn frame_keeps(frame: HintFrame, face_motion: &Motion, kept: &Motion) -> bool {
    motions_match(&sketch_motion(frame, face_motion), kept)
}

/// The coordinates a face sketch stores its fallback in to keep its motion `kept`, so that what
/// later features store on the faces built from it keeps its meaning (engine.md section 3.22, ADR
/// 0023): body coordinates when `face_motion`, the motion of the face in the state the sketch is
/// placed in (`BodyState::motion` of its query), matches `kept`; else world coordinates when
/// `kept` matches the identity; `None` when neither keeps it.
pub(crate) fn keeping_frame(face_motion: &Motion, kept: &Motion) -> Option<HintFrame> {
    [HintFrame::Body, HintFrame::World]
        .into_iter()
        .find(|frame| frame_keeps(*frame, face_motion, kept))
}

/// The face plane a sketch stores to stand at `placement` on `face` of `body` while keeping its
/// motion `kept` (`keeping_frame`): the placement as its fallback, in body coordinates or as it
/// is in world coordinates; `None` when no coordinates keep the motion.
pub(crate) fn kept_face_plane(
    body: &Id,
    face: &Reference,
    placement: &Placement,
    face_motion: &Motion,
    kept: &Motion,
) -> Option<SketchPlane> {
    let fallback_frame = keeping_frame(face_motion, kept)?;
    let fallback = match fallback_frame {
        HintFrame::Body => placement_in_body(placement, face_motion),
        HintFrame::World => *placement,
    };
    Some(SketchPlane::Face {
        body: body.clone(),
        face: face.clone(),
        fallback,
        fallback_frame,
    })
}

/// The hint a face sketch is placed with in `state` (features/sketch.md section 3.3): its
/// fallback, first moved by the motion of the stored face when the fallback is in body
/// coordinates. It is also where the sketch stands when the face cannot place it.
pub(crate) fn placement_hint(
    state: &BodyState,
    face: &Reference,
    fallback: &Placement,
    frame: HintFrame,
) -> Placement {
    match frame {
        HintFrame::World => *fallback,
        HintFrame::Body => fallback.moved(&state.motion(&face.query)),
    }
}

/// The face placement rule (features.md section 3.6) on the first face `face` resolves to in
/// `state`, when it resolves and that face is flat: the plane through its area centroid with its
/// outward unit normal, its origin and x axis taken from `hint` (`Placement::on_plane`), so the
/// placement is right-handed.
pub(crate) fn face_placement(
    state: &BodyState,
    face: &Reference,
    hint: &Placement,
) -> Option<Placement> {
    let shape = state.shape()?;
    let set = state.element_set().ok()?;
    let Resolution::Resolved { elements, .. } = resolve(face, &set) else {
        return None;
    };
    let index = state.naming().index(elements.first()?)?;
    let geometry = shape.element(SubShapeKind::Face, index).ok()?;
    if geometry.kind != GeometryKind::Plane {
        return None;
    }
    let normal = geometry.direction?;
    let centroid = geometry.centroid;
    Placement::on_plane(
        [centroid.x, centroid.y, centroid.z],
        [normal.x, normal.y, normal.z],
        hint,
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::id;

    /// A translation by (100, 0, 0) after a quarter turn about Z: p -> R p + (100, 0, 0).
    fn turned_and_moved() -> Motion {
        Motion::new(
            [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            [100.0, 0.0, 0.0],
        )
    }

    fn top_at(z: f64) -> Placement {
        Placement {
            origin: [0.0, 0.0, z],
            x_axis: [1.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        }
    }

    #[test]
    fn without_a_motion_a_placement_is_already_in_body_coordinates() {
        let placement = Placement {
            origin: [-0.0, 3.0, 40.0],
            ..top_at(40.0)
        };

        let in_body = placement_in_body(&placement, &Motion::IDENTITY);

        assert_eq!(in_body.origin[0].to_bits(), (-0.0_f64).to_bits());
        assert_eq!(in_body, placement);
    }

    #[test]
    fn a_placement_in_body_coordinates_undoes_the_motion() {
        let motion = turned_and_moved();
        let moved = top_at(40.0).moved(&motion);

        let in_body = placement_in_body(&moved, &motion);

        for axis in 0..3 {
            assert!((in_body.origin[axis] - top_at(40.0).origin[axis]).abs() < 1e-12);
            assert!((in_body.x_axis[axis] - top_at(40.0).x_axis[axis]).abs() < 1e-12);
            assert!((in_body.normal[axis] - top_at(40.0).normal[axis]).abs() < 1e-12);
        }
    }

    #[test]
    fn undoing_a_motion_turns_a_direction_back_and_ignores_the_translation() {
        let undo = undoing(&turned_and_moved()).expect("the motion moves");

        let in_body = undo.apply_direction([0.0, 1.0, 0.0]);

        assert!((in_body[0] - 1.0).abs() < 1e-12);
        assert!(in_body[1].abs() < 1e-12);
        assert!(in_body[2].abs() < 1e-12);
    }

    #[test]
    fn nothing_undoes_the_identity() {
        assert_eq!(undoing(&Motion::IDENTITY), None);
    }

    /// A translation by (100, 0, 0).
    fn moved_by_100() -> Motion {
        Motion::new(
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [100.0, 0.0, 0.0],
        )
    }

    /// The top of `box.ptsm`, as a face reference.
    fn top() -> Reference {
        Reference::new(
            "f-box1/top".parse().expect("a valid name"),
            "plane|n=0,0,1".parse().expect("a valid fingerprint"),
        )
        .expect("a face reference")
    }

    /// The face plane on the top of `b-1` with this fallback.
    fn on_top(fallback: Placement, fallback_frame: HintFrame) -> SketchPlane {
        SketchPlane::Face {
            body: id("b-1"),
            face: top(),
            fallback,
            fallback_frame,
        }
    }

    #[test]
    fn a_sketch_moves_with_its_face_only_in_body_coordinates() {
        let motion = moved_by_100();

        assert_eq!(sketch_motion(HintFrame::Body, &motion), motion);
        assert_eq!(sketch_motion(HintFrame::World, &motion), Motion::IDENTITY);
    }

    #[test]
    fn a_face_plane_on_a_face_with_the_sketch_s_motion_is_in_body_coordinates() {
        let motion = moved_by_100();
        let placement = Placement {
            origin: [100.0, 0.0, 40.0],
            ..top_at(40.0)
        };

        let plane = kept_face_plane(&id("b-1"), &top(), &placement, &motion, &motion);

        assert_eq!(plane, Some(on_top(top_at(40.0), HintFrame::Body)));
    }

    #[test]
    fn a_face_plane_without_motions_is_in_body_coordinates() {
        let identity = Motion::IDENTITY;

        let plane = kept_face_plane(&id("b-1"), &top(), &top_at(40.0), &identity, &identity);

        assert_eq!(plane, Some(on_top(top_at(40.0), HintFrame::Body)));
    }

    #[test]
    fn a_sketch_without_a_motion_on_a_moved_face_stays_in_world_coordinates() {
        // Body coordinates would give the sketch the face's motion, and every face built from it
        // with it, so what later features store on those faces would move.
        let plane = kept_face_plane(
            &id("b-1"),
            &top(),
            &top_at(40.0),
            &moved_by_100(),
            &Motion::IDENTITY,
        );

        assert_eq!(plane, Some(on_top(top_at(40.0), HintFrame::World)));
    }

    /// A translation by `(0, 0, z)`.
    fn lifted_by(z: f64) -> Motion {
        Motion::new(
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [0.0, 0.0, z],
        )
    }

    #[test]
    fn a_face_motion_that_matches_the_sketch_s_within_1e_9_keeps_body_coordinates() {
        // engine.md section 3.22: a datum on the top of a cube lifted by 0.1 mm has a translation of
        // 0.09999999999999964 mm, computed from its frames, where the face's motion is 0.1 mm.
        let face_motion = lifted_by(0.1);
        let datum_motion = lifted_by(0.099_999_999_999_999_64);

        assert_eq!(
            keeping_frame(&face_motion, &datum_motion),
            Some(HintFrame::Body)
        );
    }

    #[test]
    fn a_face_motion_further_than_1e_9_from_the_sketch_s_does_not_match() {
        let face_motion = lifted_by(0.1);
        let kept = lifted_by(0.1 + 2e-9);

        assert_eq!(keeping_frame(&face_motion, &kept), None);
    }

    #[test]
    fn a_turn_that_matches_within_1e_9_in_every_matrix_entry_keeps_body_coordinates() {
        let turn = turned_and_moved();
        let rounded = Motion::new(
            [[1e-10, -1.0, 0.0], [1.0, 1e-10, 0.0], [0.0, 0.0, 1.0]],
            [100.0, 0.0, 0.0],
        );

        assert_eq!(keeping_frame(&turn, &rounded), Some(HintFrame::Body));
    }

    #[test]
    fn a_sketch_motion_within_1e_9_of_the_identity_keeps_world_coordinates() {
        // The identity is matched with the same comparison: a sketch whose motion is the identity
        // to rounding stays in world coordinates on a face that moved.
        let kept = lifted_by(4e-16);

        assert_eq!(
            keeping_frame(&moved_by_100(), &kept),
            Some(HintFrame::World)
        );
    }

    #[test]
    fn a_sketch_motion_further_than_1e_9_from_the_identity_has_no_world_coordinates() {
        let kept = lifted_by(1e-6);

        assert_eq!(keeping_frame(&moved_by_100(), &kept), None);
    }

    #[test]
    fn no_face_plane_keeps_a_motion_its_face_does_not_have() {
        let plane = kept_face_plane(
            &id("b-1"),
            &top(),
            &top_at(40.0),
            &Motion::IDENTITY,
            &moved_by_100(),
        );

        assert_eq!(plane, None);
    }
}
