//! The direction a cap of an extrude or revolve faces in this run, read from its record and its sketch's placement,
//! for a cap that is not in the state (features.md section 3.12, "Direction words").

use partsmith_model::{Expr, FieldUnit, LINEAR_TOLERANCE};
use partsmith_naming::GeometryKind;
use partsmith_sketch::{Geometry, RegionFingerprint};
use serde_json::Value;

use super::maker::{Maker, entity_geometry};
use super::{cross, dot};
use crate::error::message::Names;
use crate::output::SketchResult;
use crate::params::AxisRef;

/// The outward normal of the cap with `role` (`start-cap` or `end-cap`) as `maker` makes it in this run, also when
/// it is suppressed or failed, as long as its sketch has a result. None for other faces and features, and when the
/// record does not give the direction.
pub(super) fn cap_direction(names: &dyn Names, maker: &Maker<'_>, role: &str) -> Option<[f64; 3]> {
    let start = match role {
        "start-cap" => true,
        "end-cap" => false,
        _ => return None,
    };
    let sketch = names.sketch_result(&maker.sketch()?)?;
    match maker.kind()? {
        "extrude" => Some(extrude_cap(maker, sketch, start)),
        "revolve" => revolve_cap(names, maker, sketch, start),
        _ => None,
    }
}

/// An extrude's end cap faces along its direction (the sketch normal, reversed by Reverse except for a symmetric
/// extent, features/extrude.md section 3.4), its start cap against it.
fn extrude_cap(maker: &Maker<'_>, sketch: &SketchResult, start: bool) -> [f64; 3] {
    let symmetric = maker.param(&["extent", "kind"]).and_then(Value::as_str) == Some("symmetric");
    let along = if reversed(maker) && !symmetric {
        -1.0
    } else {
        1.0
    };
    let sign = if start { -along } else { along };
    sketch.placement.normal.map(|component| component * sign)
}

/// A revolve's start cap faces against the turning direction at the sketch plane, its end cap along the turning
/// direction after the turn by its angle (features/revolve.md section 3.3); a full turn has no angle, so it gives its
/// end cap no direction. The axis lies in the sketch plane, so the profile turns along the sketch normal, times the
/// side of the axis it lies on, measured along the normal crossed with the axis, and times the sense of the turn.
fn revolve_cap(
    names: &dyn Names,
    maker: &Maker<'_>,
    sketch: &SketchResult,
    start: bool,
) -> Option<[f64; 3]> {
    let full = maker.param(&["extent", "kind"]).and_then(Value::as_str) == Some("full");
    if full && !start {
        return None;
    }
    let (origin, axis) = revolve_axis(names, maker)?;
    let region: RegionFingerprint = maker
        .param(&["profile", "regions"])?
        .get(0)?
        .get("fp")?
        .as_str()?
        .parse()
        .ok()?;
    let inside = sketch.to_world(region.centroid);
    let normal = sketch.placement.normal;
    let offset = [0, 1, 2].map(|i| inside[i] - origin[i]);
    let side = dot(offset, cross(normal, axis));
    if side.abs() <= LINEAR_TOLERANCE {
        return None;
    }
    let sense = if reversed(maker) && !full { -1.0 } else { 1.0 };
    let turning = normal.map(|component| component * side.signum() * sense);
    if start {
        return Some(turning.map(|component| -component));
    }
    let angle = Expr::new(maker.param(&["extent", "expr"])?.as_str()?);
    let turn = names.value(&angle, FieldUnit::Angle)? * sense;
    // The turning direction is perpendicular to the axis, so turning it by `turn` about the axis is Rodrigues'
    // rotation without its axial term.
    let across = cross(axis, turning);
    Some([0, 1, 2].map(|i| turning[i] * turn.cos() + across[i] * turn.sin()))
}

/// The revolve's axis in world coordinates, as a point on it and its unit direction: an origin axis, a sketch line from
/// its start to its end, or a straight edge by its stored fingerprint, as the build reads it (features.md section
/// 3.6). None for the axis of a face, whose stored fingerprint does not give a point on the axis.
fn revolve_axis(names: &dyn Names, maker: &Maker<'_>) -> Option<([f64; 3], [f64; 3])> {
    let axis: AxisRef = serde_json::from_value(maker.param(&["axis"])?.clone()).ok()?;
    match axis {
        AxisRef::Origin { axis } => Some(([0.0; 3], axis.direction())),
        AxisRef::SketchLine { sketch, line } => {
            let placed = names.sketch_result(&sketch)?;
            let Geometry::Line { start, end } = entity_geometry(names, &sketch, line.as_str())?
            else {
                return None;
            };
            let from = placed.to_world(start);
            let to = placed.to_world(end);
            let along = [0, 1, 2].map(|i| to[i] - from[i]);
            let length = dot(along, along).sqrt();
            (length > LINEAR_TOLERANCE).then(|| (from, along.map(|component| component / length)))
        }
        AxisRef::Edge { edge, .. } => {
            let fingerprint = &edge.fingerprint;
            if fingerprint.kind != GeometryKind::Line {
                return None;
            }
            Some((fingerprint.centroid?, fingerprint.direction?))
        }
        AxisRef::Face { .. } => None,
    }
}

/// True when the record turns Reverse on.
fn reversed(maker: &Maker<'_>) -> bool {
    maker
        .param(&["reverse"])
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
