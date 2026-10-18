//! Which hole tools overlap the body, decided from one kernel cut and, for tools that leave no face
//! of their own, their shared volumes (features/hole.md section 3.3 steps 5 and 6).

use std::collections::BTreeSet;

use partsmith_mesh::BoundingBox;
use partsmith_model::{Id, LINEAR_TOLERANCE};
use partsmith_occt_sys::{OpResult, Shape, common, compound, cut};

use crate::builtin::common::check_canceled;
use crate::context::{BodyState, BuildContext};
use crate::error::{FeatureError, FeatureWarning};
use crate::output::{BodyUpdate, Direction, Tool, VOLUME_TOLERANCE, apply_op_result};
use crate::params::BodyOp;

/// A hole tool whose bounding box meets the body's, so only a kernel boolean can tell whether it
/// overlaps the body (features/hole.md section 3.3 step 5).
pub(super) struct Candidate {
    /// The tool.
    pub(super) tool: Tool,
    /// Its tight bounding box.
    pub(super) bounds: BoundingBox,
}

/// Which candidates overlap the body, and the cut when it is the feature's result.
pub(super) struct Decision {
    /// For each candidate, in order, whether it overlaps the body.
    pub(super) overlaps: Vec<bool>,
    /// The body rules applied to the cut with every candidate, when every candidate overlaps
    /// (its error included, such as `EmptyResult`); None when the body must be cut again with the
    /// overlapping candidates only.
    pub(super) result: Option<Result<(BodyUpdate, Vec<FeatureWarning>), FeatureError>>,
}

/// Cuts the body with every candidate in one kernel call and decides from that cut which of them
/// overlap the body (features/hole.md section 3.3 steps 5 and 6): none when the cut leaves the
/// volume unchanged; a candidate that leaves a face of its own in the result; and, by its shared
/// volume, a candidate that leaves none (it misses the body, touches it or swallows whole solids
/// of it). The cut is the result when every candidate overlaps.
pub(super) fn cut_candidates(
    context: &BuildContext<'_>,
    body: &Id,
    state: &BodyState,
    shape: &Shape,
    candidates: &[Candidate],
    direction: Option<Direction<'_>>,
) -> Result<Decision, FeatureError> {
    if candidates.is_empty() {
        return Ok(Decision {
            overlaps: Vec::new(),
            result: None,
        });
    }
    check_canceled(context)?;
    let tools: Vec<Tool> = candidates
        .iter()
        .map(|candidate| candidate.tool.clone())
        .collect();
    let shapes: Vec<Shape> = tools.iter().map(|tool| tool.shape.clone()).collect();
    let result = cut(shape, &shapes)?;
    let mut overlaps = tools_with_own_faces(&result);
    let nothing_overlaps = Decision {
        overlaps: vec![false; candidates.len()],
        result: None,
    };
    if overlaps.iter().all(|overlaps| *overlaps) {
        // The usual case: the body rules name the cut once, and their `NoEffect` is step 5's
        // unchanged volume.
        let outcome = apply_op_result(context, body, BodyOp::Cut, &tools, result, direction);
        if outcome
            .as_ref()
            .is_ok_and(|(_, warnings)| has_no_effect(warnings))
        {
            return Ok(nothing_overlaps);
        }
        return Ok(Decision {
            overlaps,
            result: Some(outcome),
        });
    }
    // Some tool left no face of its own: decide it before naming anything, since the cut is the
    // result only when every tool overlaps.
    let before = state.volume()?;
    if (result.shape.volume()? - before).abs() <= VOLUME_TOLERANCE * before {
        return Ok(nothing_overlaps);
    }
    let undecided: Vec<usize> = (0..candidates.len())
        .filter(|&position| !overlaps[position])
        .collect();
    let undecided_candidates: Vec<&Candidate> = undecided
        .iter()
        .map(|&position| &candidates[position])
        .collect();
    let shared = shared_volumes(shape, &undecided_candidates)?;
    for (&position, volume) in undecided.iter().zip(shared) {
        overlaps[position] = volume > VOLUME_TOLERANCE * before;
    }
    let every_one = overlaps.iter().all(|overlaps| *overlaps);
    let result =
        every_one.then(|| apply_op_result(context, body, BodyOp::Cut, &tools, result, direction));
    Ok(Decision { overlaps, result })
}

/// True when the body rules found that an operation left the body unchanged.
fn has_no_effect(warnings: &[FeatureWarning]) -> bool {
    warnings
        .iter()
        .any(|warning| matches!(warning, FeatureWarning::NoEffect { .. }))
}

/// For each tool of a kernel cut, in order, whether a face of the result descends from one of the
/// tool's faces and from no face of the body (the history's `modified` lists, whose first input is
/// the body and the others the tools): such a face is the tool's boundary inside the body.
fn tools_with_own_faces(result: &OpResult) -> Vec<bool> {
    let Some((body, tools)) = result.history.inputs.split_first() else {
        return Vec::new();
    };
    let body_faces: BTreeSet<u32> = body
        .faces
        .iter()
        .flat_map(|fate| fate.modified.iter().copied())
        .collect();
    tools
        .iter()
        .map(|tool| {
            tool.faces
                .iter()
                .flat_map(|fate| &fate.modified)
                .any(|face| !body_faces.contains(face))
        })
        .collect()
}

/// The volume each candidate shares with the body, in candidate order. Candidates whose bounding
/// boxes are apart are grouped into one compound and intersected with the body in one kernel
/// `common`, each solid of the result counting for the candidate whose box is nearest to its
/// center: the kernel's work grows with the body, so one boolean replaces one per hole.
/// Candidates whose boxes come closer each get their own `common`.
fn shared_volumes(body: &Shape, candidates: &[&Candidate]) -> Result<Vec<f64>, FeatureError> {
    let bounds: Vec<&BoundingBox> = candidates
        .iter()
        .map(|candidate| &candidate.bounds)
        .collect();
    if candidates.len() < 2 || !apart(&bounds) {
        return candidates
            .iter()
            .map(|candidate| {
                let shared = common(body, std::slice::from_ref(&candidate.tool.shape))?;
                Ok(shared.shape.volume()?)
            })
            .collect();
    }
    let tools: Vec<Shape> = candidates
        .iter()
        .map(|candidate| candidate.tool.shape.clone())
        .collect();
    let group = compound(&tools)?.shape;
    let shared = common(body, std::slice::from_ref(&group))?.shape;
    let mut volumes = vec![0.0; candidates.len()];
    for solid in shared.solids()? {
        let center = solid.bounding_box()?.center();
        volumes[nearest(&bounds, center)] += solid.volume()?;
    }
    Ok(volumes)
}

/// True when every two boxes are more than `LINEAR_TOLERANCE` apart along some axis, so the
/// solids inside them neither overlap nor touch.
fn apart(bounds: &[&BoundingBox]) -> bool {
    bounds.iter().enumerate().all(|(index, first)| {
        bounds[index + 1..].iter().all(|second| {
            (0..3).any(|axis| {
                first.max[axis] + LINEAR_TOLERANCE < second.min[axis]
                    || second.max[axis] + LINEAR_TOLERANCE < first.min[axis]
            })
        })
    })
}

/// The position in `bounds` of the box nearest to `point` (distance 0 inside a box); the first of
/// equally near boxes. `bounds` is not empty.
fn nearest(bounds: &[&BoundingBox], point: [f64; 3]) -> usize {
    let squared_distance = |bounds: &BoundingBox| -> f64 {
        (0..3)
            .map(|axis| {
                let outside = (bounds.min[axis] - point[axis])
                    .max(point[axis] - bounds.max[axis])
                    .max(0.0);
                outside * outside
            })
            .sum()
    };
    let mut nearest = 0;
    for (index, candidate) in bounds.iter().enumerate().skip(1) {
        if squared_distance(candidate) < squared_distance(bounds[nearest]) {
            nearest = index;
        }
    }
    nearest
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use partsmith_occt_sys::{Plane, Point, make_cylinder};

    use super::*;
    use crate::test_support::{Fixture, box_body, id, named_box, named_primitive};

    fn bounds(min: [f64; 3], max: [f64; 3]) -> BoundingBox {
        BoundingBox { min, max }
    }

    /// A cylinder tool of radius 1 standing on (x, y, z), 12 high, with its bounding box.
    fn rod(x: f64, y: f64, z: f64) -> Candidate {
        let placement = Plane {
            origin: Point::new(x, y, z),
            ..Plane::XY
        };
        let tool = named_primitive(
            "f-rod",
            make_cylinder(&placement, 1.0, 12.0).expect("the rod builds"),
        );
        let bounds = tool.shape.bounding_box().expect("the rod has bounds");
        Candidate { tool, bounds }
    }

    /// The shared volumes of `candidates` with the cube [0, 10]^3.
    fn shared_with_cube(candidates: &[Candidate]) -> Vec<f64> {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let candidates: Vec<&Candidate> = candidates.iter().collect();
        shared_volumes(&cube.shape, &candidates).expect("the overlaps are measured")
    }

    fn assert_volumes(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len(), "{actual:?}");
        for (actual, expected) in actual.iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
        }
    }

    #[test]
    fn boxes_are_apart_with_a_gap_along_one_axis() {
        let unit = bounds([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let beside = bounds([2.0, 0.5, 0.5], [3.0, 1.5, 1.5]);
        let touching = bounds([1.0, 0.0, 0.0], [2.0, 1.0, 1.0]);
        let overlapping = bounds([0.5, 0.5, 0.5], [1.5, 1.5, 1.5]);

        assert!(apart(&[&unit, &beside]));
        assert!(!apart(&[&unit, &touching]));
        assert!(!apart(&[&beside, &unit, &overlapping]));
        assert!(apart(&[&unit]));
    }

    #[test]
    fn the_nearest_box_holds_the_point_or_is_closest_to_it() {
        let first = bounds([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let second = bounds([5.0, 0.0, 0.0], [6.0, 1.0, 1.0]);

        assert_eq!(nearest(&[&first, &second], [5.5, 0.5, 0.5]), 1);
        assert_eq!(nearest(&[&first, &second], [2.0, 0.5, 0.5]), 0);
        assert_eq!(nearest(&[&first, &second], [4.0, 0.5, 0.5]), 1);
    }

    #[test]
    fn tools_apart_share_their_own_volumes_with_the_body() {
        let quarter = PI * 10.0 / 4.0;

        let volumes = shared_with_cube(&[
            rod(5.0, 5.0, -1.0),
            rod(0.0, 0.0, -1.0),
            rod(10.0, 5.0, 5.0),
            rod(20.0, 20.0, 0.0),
        ]);

        assert_volumes(&volumes, &[PI * 10.0, quarter, PI * 5.0 / 2.0, 0.0]);
    }

    #[test]
    fn tools_close_together_share_their_own_volumes_with_the_body() {
        let volumes = shared_with_cube(&[rod(5.0, 5.0, -1.0), rod(6.0, 5.0, 2.0)]);

        assert_volumes(&volumes, &[PI * 10.0, PI * 8.0]);
    }

    /// Decides which `candidates` overlap the cube [0, 10]^3 held by `b-1`.
    fn decide_on_cube(candidates: &[Candidate]) -> Decision {
        let state = box_body("f-box1", [0.0, 0.0, 0.0], 10.0);
        let fixture = Fixture::new("f-hole1").with_body("b-1", state.clone());
        let shape = state.shape().expect("a solid");
        cut_candidates(
            &fixture.context(),
            &id("b-1"),
            &state,
            shape,
            candidates,
            None,
        )
        .expect("the candidates are decided")
    }

    #[test]
    fn only_tools_that_drill_the_body_leave_faces_of_their_own() {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let rods = [
            rod(5.0, 5.0, -1.0),
            rod(20.0, 20.0, 0.0),
            rod(5.0, 5.0, 10.0),
        ];
        let shapes: Vec<Shape> = rods.iter().map(|rod| rod.tool.shape.clone()).collect();

        let result = cut(&cube.shape, &shapes).expect("the kernel cuts");

        assert_eq!(tools_with_own_faces(&result), [true, false, false]);
    }

    #[test]
    fn the_cut_is_the_result_when_every_tool_drills_the_body() {
        let decision = decide_on_cube(&[rod(5.0, 5.0, -1.0), rod(2.0, 2.0, 5.0)]);

        assert_eq!(decision.overlaps, [true, true]);
        let (update, warnings) = decision
            .result
            .expect("the cut is the result")
            .expect("the cut applies");
        assert!(warnings.is_empty());
        let shape = update.shape.expect("a solid");
        let expected = 1000.0 - PI * 10.0 - PI * 5.0;
        assert!((shape.volume().expect("a volume") - expected).abs() < 1e-6);
    }

    #[test]
    fn tools_without_faces_of_their_own_are_decided_by_their_shared_volume() {
        let decision = decide_on_cube(&[
            rod(5.0, 5.0, -1.0),
            rod(20.0, 20.0, 0.0),
            rod(5.0, 5.0, 10.0),
        ]);

        assert_eq!(decision.overlaps, [true, false, false]);
        assert!(decision.result.is_none(), "the body is cut again");
    }

    #[test]
    fn tools_that_leave_the_volume_unchanged_do_not_overlap() {
        let decision = decide_on_cube(&[rod(5.0, 5.0, 10.0), rod(20.0, 20.0, 0.0)]);

        assert_eq!(decision.overlaps, [false, false]);
        assert!(decision.result.is_none());
    }

    #[test]
    fn no_candidate_leaves_nothing_to_decide() {
        let decision = decide_on_cube(&[]);

        assert!(decision.overlaps.is_empty());
        assert!(decision.result.is_none());
    }
}
