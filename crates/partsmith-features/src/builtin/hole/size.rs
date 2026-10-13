//! The checked sizes of a hole and its half cross-section (features/hole.md section 3.3 steps 2
//! and 4).

use std::f64::consts::PI;

use partsmith_model::{Expr, FieldUnit, LINEAR_TOLERANCE};

use super::tool::Role;
use super::{HoleExtent, HoleParams, HoleType};
use crate::builtin::common::{
    THROUGH_ALL_MARGIN, angle_in_range, check_range, is_positive_length, out_of_range,
    positive_length,
};
use crate::context::BuildContext;
use crate::error::{Bound, FeatureError, FeatureWarning, IgnoredReason};

/// The head of a hole with its evaluated sizes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Head {
    /// No head.
    Simple,
    /// A counterbore of this radius whose floor is at `depth`.
    Counterbore {
        /// Radius of the counterbore.
        radius: f64,
        /// Depth of the counterbore floor.
        depth: f64,
    },
    /// A countersink of this top radius whose cone meets the bore at `depth`.
    Countersink {
        /// Radius at the top of the cone.
        radius: f64,
        /// Depth where the cone meets the bore.
        depth: f64,
    },
}

/// The evaluated and checked sizes of a hole.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct HoleSize {
    /// Radius of the bore.
    radius: f64,
    /// The head.
    head: Head,
    /// Depth to the shoulder of a blind hole; None for a through-all hole.
    pub(super) depth: Option<f64>,
    /// Included tip angle of a blind hole with a drill point; None for a flat end.
    tip: Option<f64>,
}

impl HoleSize {
    /// Evaluates the values and checks them in the order of features/hole.md section 3.3 step 2:
    /// the diameter, the depth, the head, then the tip, each length by the positive length rule of
    /// features.md section 3.4. A tip angle on a through-all hole adds an `Ignored` warning and is
    /// not evaluated.
    pub(super) fn checked(
        params: &HoleParams,
        context: &BuildContext<'_>,
        warnings: &mut Vec<FeatureWarning>,
    ) -> Result<HoleSize, FeatureError> {
        let diameter = context.eval_length("diameter", &params.diameter)?;
        // The entry and bottom segments of the cross-section are as long as the bore radius.
        if !is_positive_length(diameter / 2.0) {
            return Err(out_of_range(
                context,
                "diameter",
                FieldUnit::Length,
                diameter,
                Bound::GreaterThan(0.0),
                None,
            ));
        }
        let depth = match &params.extent {
            HoleExtent::Distance { expr } => Some(positive_length(context, "extent.expr", expr)?),
            HoleExtent::ThroughAll => None,
        };
        let wider = |field: &str, expr: &Expr| {
            let head_diameter = context.eval_length(field, expr)?;
            // The step in from the head radius to the bore radius is a segment of its own.
            if is_positive_length((head_diameter - diameter) / 2.0) {
                Ok(head_diameter)
            } else {
                Err(out_of_range(
                    context,
                    field,
                    FieldUnit::Length,
                    head_diameter,
                    Bound::GreaterThan(diameter),
                    Some("the hole diameter"),
                ))
            }
        };
        let head = match &params.hole_type {
            HoleType::Simple => Head::Simple,
            HoleType::Counterbore {
                diameter: head_diameter,
                depth: head_depth,
            } => {
                let head_diameter = wider("hole_type.diameter", head_diameter)?;
                let head_depth = positive_length(context, "hole_type.depth", head_depth)?;
                if let Some(depth) = depth
                    && !ends_above_shoulder(head_depth, depth)
                {
                    return Err(out_of_range(
                        context,
                        "hole_type.depth",
                        FieldUnit::Length,
                        head_depth,
                        Bound::LessThan(depth),
                        Some("the hole depth"),
                    ));
                }
                Head::Counterbore {
                    radius: head_diameter / 2.0,
                    depth: head_depth,
                }
            }
            HoleType::Countersink {
                diameter: head_diameter,
                angle: head_angle,
            } => {
                let head_diameter = wider("hole_type.diameter", head_diameter)?;
                let head_angle = included_angle(context, "hole_type.angle", head_angle)?;
                let slope = (head_angle / 2.0).tan();
                let head_depth = (head_diameter - diameter) / 2.0 / slope;
                // The message names the head diameter whose cone ends exactly at the shoulder.
                if let Some(depth) = depth
                    && !ends_above_shoulder(head_depth, depth)
                {
                    return Err(out_of_range(
                        context,
                        "hole_type.diameter",
                        FieldUnit::Length,
                        head_diameter,
                        Bound::LessThan(diameter + 2.0 * depth * slope),
                        Some("the largest that fits the hole depth"),
                    ));
                }
                Head::Countersink {
                    radius: head_diameter / 2.0,
                    depth: head_depth,
                }
            }
        };
        let tip = match (&params.tip_angle, depth) {
            (None, _) => None,
            (Some(_), None) => {
                warnings.push(FeatureWarning::Ignored {
                    field: context.field("tip_angle"),
                    reason: IgnoredReason::TipAngleThroughAll,
                });
                None
            }
            (Some(expr), Some(_)) => Some(included_angle(context, "tip_angle", expr)?),
        };
        Ok(HoleSize {
            radius: diameter / 2.0,
            head,
            depth,
            tip,
        })
    }

    /// Depth of the head's lower end: the counterbore floor or the end of the countersink cone.
    fn head_depth(&self) -> f64 {
        match self.head {
            Head::Simple => 0.0,
            Head::Counterbore { depth, .. } | Head::Countersink { depth, .. } => depth,
        }
    }

    /// The length of a through-all hole at a point whose farthest body corner is `farthest` along
    /// the drilling direction: 1 mm past that corner and past the head, so a head deeper than the
    /// body still leaves a bore below it. None when the corner is not ahead of the point
    /// (features/hole.md section 3.3 step 4).
    pub(super) fn through_length(&self, farthest: f64) -> Option<f64> {
        (farthest > 0.0).then(|| farthest.max(self.head_depth()) + THROUGH_ALL_MARGIN)
    }

    /// The half cross-section of a hole whose bore ends at `length`, as the end points of its
    /// segments in (radius, depth) coordinates, each with the role of the segment that ends there.
    /// It starts at the origin, runs out along the entry, down the head and the bore, in along the
    /// end, and back up the axis.
    pub(super) fn section(&self, length: f64) -> Vec<(Role, [f64; 2])> {
        let bore = self.radius;
        let mut corners = Vec::new();
        match self.head {
            Head::Simple => corners.push((Role::Entry, [bore, 0.0])),
            Head::Counterbore { radius, depth } => {
                corners.push((Role::Entry, [radius, 0.0]));
                corners.push((Role::Counterbore, [radius, depth]));
                corners.push((Role::CounterboreFloor, [bore, depth]));
            }
            Head::Countersink { radius, depth } => {
                corners.push((Role::Entry, [radius, 0.0]));
                corners.push((Role::Countersink, [bore, depth]));
            }
        }
        corners.push((Role::Bore, [bore, length]));
        match self.tip {
            None => corners.push((Role::Bottom, [0.0, length])),
            Some(tip) => corners.push((Role::Tip, [0.0, length + bore / (tip / 2.0).tan()])),
        }
        corners.push((Role::Axis, [0.0, 0.0]));
        corners
    }
}

/// True when a head whose lower end is at `head_depth` ends at least `LINEAR_TOLERANCE` above the
/// shoulder at `depth`, so the bore between them is long enough for the kernel (features/hole.md
/// section 3.3 step 2).
fn ends_above_shoulder(head_depth: f64, depth: f64) -> bool {
    head_depth <= depth - LINEAR_TOLERANCE
}

/// The included angle at `field` of a countersink or a drill-point tip: greater than 0 and less
/// than 180 deg (features/hole.md section 3.3 step 2).
fn included_angle(
    context: &BuildContext<'_>,
    field: &str,
    expr: &Expr,
) -> Result<f64, FeatureError> {
    let angle = angle_in_range(context, field, expr, Bound::GreaterThan(0.0))?;
    check_range(
        context,
        field,
        FieldUnit::Angle,
        angle,
        Bound::LessThan(PI),
        None,
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::builtin::hole::Hole;
    use crate::builtin::hole::tests::{params, record};
    use crate::test_support::fixture;

    /// The error of checking the values of `HOLE(["e-p1"], extra)`.
    fn size_error(extra: serde_json::Value) -> FeatureError {
        let hole = params(record(json!(["e-p1"]), extra));
        let fixture = fixture::<Hole>("f-hole1", &hole, Vec::new(), Vec::new());
        HoleSize::checked(&hole, &fixture.context(), &mut Vec::new())
            .expect_err("a value is out of range")
    }

    fn size(head: Head, depth: Option<f64>, tip: Option<f64>) -> HoleSize {
        HoleSize {
            radius: 3.0,
            head,
            depth,
            tip,
        }
    }

    #[test]
    fn a_simple_flat_section_is_a_rectangle_on_the_axis() {
        let section = size(Head::Simple, Some(5.0), None).section(5.0);

        assert_eq!(
            section,
            [
                (Role::Entry, [3.0, 0.0]),
                (Role::Bore, [3.0, 5.0]),
                (Role::Bottom, [0.0, 5.0]),
                (Role::Axis, [0.0, 0.0]),
            ]
        );
    }

    #[test]
    fn a_tip_ends_at_its_apex_on_the_axis() {
        let tip = 118.0_f64.to_radians();

        let section = size(Head::Simple, Some(5.0), Some(tip)).section(5.0);

        let (role, apex) = section[2];
        assert_eq!(role, Role::Tip);
        assert_eq!(apex[0], 0.0);
        assert!((apex[1] - 6.802582).abs() < 1e-6, "{apex:?}");
    }

    #[test]
    fn a_counterbore_steps_in_at_its_floor() {
        let head = Head::Counterbore {
            radius: 5.5,
            depth: 4.0,
        };

        let section = size(head, None, None).section(11.0);

        assert_eq!(
            section,
            [
                (Role::Entry, [5.5, 0.0]),
                (Role::Counterbore, [5.5, 4.0]),
                (Role::CounterboreFloor, [3.0, 4.0]),
                (Role::Bore, [3.0, 11.0]),
                (Role::Bottom, [0.0, 11.0]),
                (Role::Axis, [0.0, 0.0]),
            ]
        );
    }

    #[test]
    fn a_countersink_narrows_to_the_bore() {
        let head = Head::Countersink {
            radius: 6.0,
            depth: 3.0,
        };

        let section = size(head, None, None).section(11.0);

        assert_eq!(section[0], (Role::Entry, [6.0, 0.0]));
        assert_eq!(section[1], (Role::Countersink, [3.0, 3.0]));
        assert_eq!(section[2], (Role::Bore, [3.0, 11.0]));
    }

    #[test]
    fn through_all_goes_one_millimeter_past_the_body_and_the_head() {
        let head = Head::Counterbore {
            radius: 5.5,
            depth: 4.0,
        };

        assert_eq!(
            size(Head::Simple, None, None).through_length(10.0),
            Some(11.0)
        );
        assert_eq!(size(head, None, None).through_length(2.0), Some(5.0));
    }

    #[test]
    fn through_all_needs_a_corner_ahead_of_the_point() {
        let simple = size(Head::Simple, None, None);

        assert_eq!(simple.through_length(0.0), None);
        assert_eq!(simple.through_length(-3.0), None);
        assert_eq!(simple.through_length(1e-9), Some(1.0 + 1e-9));
    }

    #[test]
    fn a_head_must_end_a_tolerance_above_the_shoulder() {
        assert!(ends_above_shoulder(4.0, 5.0));
        assert!(!ends_above_shoulder(4.9999999, 5.0));
        assert!(!ends_above_shoulder(5.0, 5.0));
    }

    #[test]
    fn the_depth_is_checked_before_the_head() {
        let error = size_error(json!({
            "hole_type": {"kind": "counterbore", "diameter": "5", "depth": "4"},
            "extent": {"kind": "distance", "expr": "0"}
        }));

        assert_eq!(
            error.field().map(|field| field.path.as_str()),
            Some("extent.expr")
        );
    }

    #[test]
    fn a_counterbore_within_the_tolerance_of_the_shoulder_is_too_deep() {
        let error = size_error(json!({
            "hole_type": {"kind": "counterbore", "diameter": "11", "depth": "4.9999999"},
            "extent": {"kind": "distance", "expr": "5"}
        }));

        assert_eq!(
            error.to_string(),
            "Counterbore depth must be less than the hole depth (5 mm), but it is 5 mm. Use a value less than 5 mm."
        );
    }

    #[test]
    fn a_countersink_within_the_tolerance_of_the_shoulder_is_too_wide() {
        let error = size_error(json!({
            "hole_type": {"kind": "countersink", "diameter": "15.9999999", "angle": "90 deg"},
            "extent": {"kind": "distance", "expr": "5"}
        }));

        assert_eq!(
            error.to_string(),
            "Head diameter must be less than the largest that fits the hole depth (16 mm), but it is 16 mm. Use a value less than 16 mm."
        );
    }
}
