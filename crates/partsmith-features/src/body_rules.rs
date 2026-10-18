//! The measurements behind the body rules (features.md sections 3.5 and 3.15): why a join, cut or intersect changed
//! nothing, the attach face of a directed feature, closed hollows, separate pieces and the Reverse check.

use partsmith_mesh::BoundingBox;
use partsmith_model::LINEAR_TOLERANCE;
use partsmith_naming::{ElementKind, GeometryKind, Reference};
use partsmith_occt_sys::{Shape, SubShapeKind, common};

use crate::context::{BodyState, BuildContext};
use crate::error::{FeatureError, NoEffectHow};
use crate::output::{Tool, VOLUME_TOLERANCE};
use crate::params::BodyOp;

/// How far a direction may be from a world axis, per component, for boxes to be compared along it (section 3.15).
const AXIS_TOLERANCE: f64 = 1e-9;

/// The largest angle in radians between a face's outward normal and the feature's direction for the face to be an
/// attach face (section 3.15).
const ATTACH_ANGLE: f64 = 1e-9;

/// Why a join, cut or intersect of `tools` with `body` changed nothing (section 3.15): `Inside` for a join, `Covers`
/// for an intersect, and for a cut where the tools' combined box lies along `direction` relative to the body's box.
pub(crate) fn no_effect_how(
    op: BodyOp,
    body: &Shape,
    tools: &[Tool],
    direction: Option<[f64; 3]>,
) -> Result<NoEffectHow, FeatureError> {
    match op {
        BodyOp::Join | BodyOp::New => Ok(NoEffectHow::Inside),
        BodyOp::Intersect => Ok(NoEffectHow::Covers),
        BodyOp::Cut => cut_how(body, tools, direction),
    }
}

/// `Away`, `Short` or `Beside` of a cut that changed nothing (section 3.15).
fn cut_how(
    body: &Shape,
    tools: &[Tool],
    direction: Option<[f64; 3]>,
) -> Result<NoEffectHow, FeatureError> {
    let Some((axis, sign)) = direction.and_then(world_axis) else {
        return Ok(NoEffectHow::Beside);
    };
    let body_box = body.bounding_box()?;
    let mut tool_box: Option<BoundingBox> = None;
    for tool in tools {
        let bounds = tool.shape.bounding_box()?;
        tool_box = Some(match tool_box {
            Some(combined) => combined.union(&bounds),
            None => bounds,
        });
    }
    let Some(tool_box) = tool_box else {
        return Ok(NoEffectHow::Beside);
    };
    let overlaps_across = (0..3)
        .filter(|&other| other != axis)
        .all(|other| overlap(&body_box, &tool_box, other) > LINEAR_TOLERANCE);
    if !overlaps_across {
        return Ok(NoEffectHow::Beside);
    }
    let [tool_start, tool_end] = extent(&tool_box, axis, sign);
    let [body_start, body_end] = extent(&body_box, axis, sign);
    // A gap the tolerance cannot tell from zero is at the surface, so it is measured as zero.
    Ok(if tool_start >= body_end - LINEAR_TOLERANCE {
        NoEffectHow::Away {
            gap: (tool_start - body_end).max(0.0),
        }
    } else if tool_end <= body_start + LINEAR_TOLERANCE {
        NoEffectHow::Short {
            gap: (body_start - tool_end).max(0.0),
        }
    } else {
        NoEffectHow::Beside
    })
}

/// The world axis `direction` lies along within `AXIS_TOLERANCE`, as its index and sign.
fn world_axis(direction: [f64; 3]) -> Option<(usize, f64)> {
    let axis = (0..3).find(|&axis| (direction[axis].abs() - 1.0).abs() <= AXIS_TOLERANCE)?;
    let others_vanish = (0..3)
        .filter(|&other| other != axis)
        .all(|other| direction[other].abs() <= AXIS_TOLERANCE);
    others_vanish.then(|| (axis, direction[axis].signum()))
}

/// The length over which two boxes overlap along the world axis `axis` (negative when they are apart).
fn overlap(first: &BoundingBox, second: &BoundingBox, axis: usize) -> f64 {
    first.max[axis].min(second.max[axis]) - first.min[axis].max(second.min[axis])
}

/// The extent of a box along the world axis `axis` pointing the way of `sign`, smallest first.
fn extent(bounds: &BoundingBox, axis: usize, sign: f64) -> [f64; 2] {
    if sign > 0.0 {
        [bounds.min[axis], bounds.max[axis]]
    } else {
        [-bounds.max[axis], -bounds.min[axis]]
    }
}

/// The attach face of a body (section 3.15): the planar face whose outward normal equals `direction` within
/// `ATTACH_ANGLE` and whose centroid lies farthest along it, when no other such face lies within `LINEAR_TOLERANCE`
/// of it along `direction`; as `Reference::from_element` stores it.
pub(crate) fn attach_face(
    state: &BodyState,
    direction: [f64; 3],
) -> Result<Option<Reference>, FeatureError> {
    if state.is_empty() {
        return Ok(None);
    }
    let set = state.element_set()?;
    let mut facing: Vec<(f64, usize)> = set
        .elements()
        .iter()
        .enumerate()
        .filter(|(_, info)| {
            info.kind == ElementKind::Face
                && info.fingerprint.kind == GeometryKind::Plane
                && info
                    .fingerprint
                    .direction
                    .is_some_and(|normal| angle(normal, direction) <= ATTACH_ANGLE)
        })
        .filter_map(|(index, info)| {
            let centroid = info.fingerprint.centroid?;
            Some((dot(centroid, direction), index))
        })
        .collect();
    facing.sort_by(|first, second| second.0.total_cmp(&first.0));
    let Some(&(farthest, index)) = facing.first() else {
        return Ok(None);
    };
    let unique = facing
        .get(1)
        .is_none_or(|&(next, _)| farthest - next > LINEAR_TOLERANCE);
    Ok(unique.then(|| Reference::from_element(&set.elements()[index], &set)))
}

/// The number of closed cavities of a shape: its shells minus its solids, one outer shell per solid (kernel.md
/// section 3.6 item 6).
pub(crate) fn inner_shells(shape: &Shape) -> u32 {
    shape
        .count(SubShapeKind::Shell)
        .saturating_sub(shape.count(SubShapeKind::Solid))
}

/// True when the tools overlap the body by more than `VOLUME_TOLERANCE` times its volume (section 3.15): a kernel
/// `common` of the body with each tool until one overlaps it, so one call for the usual single tool.
pub fn reverse_hits(
    context: &BuildContext<'_>,
    body: &partsmith_model::Id,
    tools: &[Tool],
) -> Result<bool, FeatureError> {
    let state = context.body(body)?;
    let Some(target) = state.shape() else {
        return Ok(false);
    };
    let volume = state.volume()?;
    for tool in tools {
        context.check_canceled()?;
        let overlap = common(target, std::slice::from_ref(&tool.shape))?;
        let shared = overlap.shape.count(SubShapeKind::Solid) > 0
            && overlap.shape.volume()? > VOLUME_TOLERANCE * volume;
        if shared {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The angle between two unit vectors, accurate for small angles.
fn angle(first: [f64; 3], second: [f64; 3]) -> f64 {
    let cross = [
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    ];
    let sine = dot(cross, cross).sqrt();
    sine.atan2(dot(first, second))
}

/// The dot product.
fn dot(first: [f64; 3], second: [f64; 3]) -> f64 {
    first[0] * second[0] + first[1] * second[1] + first[2] * second[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(min: [f64; 3], max: [f64; 3]) -> BoundingBox {
        BoundingBox { min, max }
    }

    #[test]
    fn only_directions_along_a_world_axis_compare_boxes() {
        assert_eq!(world_axis([0.0, 0.0, 1.0]), Some((2, 1.0)));
        assert_eq!(world_axis([-1.0, 0.0, 0.0]), Some((0, -1.0)));
        assert_eq!(world_axis([0.0, 1e-10, -1.0]), Some((2, -1.0)));
        assert_eq!(world_axis([0.6, 0.0, 0.8]), None);
    }

    #[test]
    fn extents_along_a_reversed_axis_are_mirrored() {
        let block = bounds([0.0, 0.0, 2.0], [1.0, 1.0, 5.0]);

        assert_eq!(extent(&block, 2, 1.0), [2.0, 5.0]);
        assert_eq!(extent(&block, 2, -1.0), [-5.0, -2.0]);
    }

    #[test]
    fn boxes_apart_have_a_negative_overlap() {
        let first = bounds([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let second = bounds([3.0, 0.5, 0.0], [4.0, 2.0, 1.0]);

        assert_eq!(overlap(&first, &second, 0), -2.0);
        assert_eq!(overlap(&first, &second, 1), 0.5);
    }

    #[test]
    fn small_angles_are_measured_accurately() {
        let tilted = [1e-10_f64.sin(), 0.0, 1e-10_f64.cos()];

        assert!((angle(tilted, [0.0, 0.0, 1.0]) - 1e-10).abs() < 1e-20);
        assert_eq!(angle([0.0, 0.0, 1.0], [0.0, 0.0, 1.0]), 0.0);
    }
}
