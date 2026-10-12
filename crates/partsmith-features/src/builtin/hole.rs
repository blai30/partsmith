//! The `hole` feature: simple, counterbore and countersink holes drilled at sketch points along the
//! sketch normal (features/hole.md).
//!
//! Each point gets its own tool, the full revolution of a half cross-section whose curves are
//! tagged by role, so every tool face is named `<feature>/<role>/<point>` (naming.md section
//! 3.6.8). The tools are cut from the body with one kernel call.

use std::f64::consts::{PI, TAU};

use partsmith_model::{Category, Expr, FeatureManifest, FieldUnit, Id, SelectionKind};
use partsmith_naming::{FaceSource, Generator, roles};
use partsmith_occt_sys::{
    Axis, PlanarCurve, PlanarLoop, PlanarRegion, Plane, Point, Point2, Shape, SweepFace,
    TaggedCurve, Vector, common, make_profile, revolve,
};
use partsmith_sketch::{PointRef, PointRole};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    THROUGH_ALL_MARGIN, bodies, body_hint, call, check_range, empty_selection, essential,
    face_name, farthest_corner, flag, hint, is_false, manifest, one_or_list, options_hint,
    selection_hint, solid_body, unchanged, unit_hint,
};
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::{Bound, FeatureError, FeatureWarning, IgnoredReason, Noun};
use crate::feature::Feature;
use crate::name_shape;
use crate::output::{BodyChange, FeatureOutput, Tool, apply_op};
use crate::params::{BodyOp, PlacementExt};
use crate::python::{PythonCall, PythonNames, PythonValue};

/// Relative common volume at or below which a tool does not touch the body (features/hole.md
/// section 3.3 step 5). The same rule as "the volume does not change" in features.md section 3.5,
/// whose constant the framework keeps private.
const VOLUME_TOLERANCE: f64 = 1e-9;

/// Cuts round holes at sketch points.
#[derive(Clone, Copy, Debug)]
pub struct Hole;

/// Params of a hole feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HoleParams {
    /// The body to cut.
    pub body: Id,
    /// The sketch whose points place the holes; its plane sets the drilling direction.
    pub sketch: Id,
    /// Sketch points, at least one, as point refs (`e-p1`, `e-1.start`, `e-5.center`).
    pub points: Vec<PointRef>,
    /// Hole diameter. Length, greater than 0.
    pub diameter: Expr,
    /// Simple, counterbore or countersink. Default simple, omitted when simple.
    #[serde(default, skip_serializing_if = "HoleType::is_simple")]
    pub hole_type: HoleType,
    /// Depth or through all.
    pub extent: HoleExtent,
    /// Included angle of a drill-point tip. Angle, greater than 0 and less than 180 deg. None (omitted) is a flat bottom.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tip_angle: Option<Expr>,
    /// Drill along the sketch normal instead of against it. Default false, omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub reverse: bool,
}

/// The head of a hole.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HoleType {
    /// A plain cylinder.
    #[default]
    Simple,
    /// A wider cylinder of `diameter` and `depth` at the top. Lengths.
    Counterbore {
        /// The counterbore diameter.
        diameter: Expr,
        /// The counterbore depth.
        depth: Expr,
    },
    /// A cone of top `diameter` and included `angle` at the top. Length and angle.
    Countersink {
        /// The diameter at the top of the cone.
        diameter: Expr,
        /// The included angle of the cone.
        angle: Expr,
    },
}

impl HoleType {
    /// True for the default, `Simple`; for `skip_serializing_if`.
    pub fn is_simple(&self) -> bool {
        *self == HoleType::Simple
    }
}

/// How deep a hole goes, from the sketch plane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HoleExtent {
    /// To the shoulder at depth `expr` (the tip, if any, goes beyond). Length, greater than 0.
    Distance {
        /// The depth.
        expr: Expr,
    },
    /// Through the whole body.
    ThroughAll,
}

impl Feature for Hole {
    const TYPE: &'static str = "hole";
    type Params = HoleParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Hole",
            Category::Create,
            "hole",
            vec![
                essential(selection_hint(
                    "points",
                    "Points",
                    SelectionKind::SketchPoint,
                )),
                essential(unit_hint("diameter", "Diameter", FieldUnit::Length)),
                essential(options_hint(
                    "hole_type.kind",
                    "Type",
                    &[
                        ("simple", "Simple"),
                        ("counterbore", "Counterbore"),
                        ("countersink", "Countersink"),
                    ],
                )),
                essential(unit_hint(
                    "hole_type.diameter",
                    "Head diameter",
                    FieldUnit::Length,
                )),
                essential(unit_hint(
                    "hole_type.depth",
                    "Counterbore depth",
                    FieldUnit::Length,
                )),
                essential(unit_hint(
                    "hole_type.angle",
                    "Countersink angle",
                    FieldUnit::Angle,
                )),
                essential(options_hint(
                    "extent.kind",
                    "Extent",
                    &[("distance", "Depth"), ("through_all", "Through all")],
                )),
                essential(unit_hint("extent.expr", "Depth", FieldUnit::Length)),
                unit_hint("tip_angle", "Tip angle", FieldUnit::Angle),
                hint("reverse", "Reverse"),
                selection_hint("sketch", "Sketch", SelectionKind::Sketch),
                body_hint(),
            ],
        )
    }

    fn validate(params: &HoleParams) -> Result<(), FeatureError> {
        if params.points.is_empty() {
            return Err(empty_selection(&Hole::manifest(), "points", Noun::Point));
        }
        Ok(())
    }

    /// The sketch, the cut body and the expressions in the order of features/hole.md section 3.2.
    fn dependencies(params: &HoleParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_feature(&params.sketch, "sketch");
        dependencies.add_body_written(&params.body, "body");
        dependencies.add_expression("diameter", &params.diameter, FieldUnit::Length);
        match &params.hole_type {
            HoleType::Simple => {}
            HoleType::Counterbore { diameter, depth } => {
                dependencies.add_expression("hole_type.diameter", diameter, FieldUnit::Length);
                dependencies.add_expression("hole_type.depth", depth, FieldUnit::Length);
            }
            HoleType::Countersink { diameter, angle } => {
                dependencies.add_expression("hole_type.diameter", diameter, FieldUnit::Length);
                dependencies.add_expression("hole_type.angle", angle, FieldUnit::Angle);
            }
        }
        if let HoleExtent::Distance { expr } = &params.extent {
            dependencies.add_expression("extent.expr", expr, FieldUnit::Length);
        }
        if let Some(tip_angle) = &params.tip_angle {
            dependencies.add_expression("tip_angle", tip_angle, FieldUnit::Angle);
        }
        dependencies
    }

    /// Checks the values, builds one tool per sketch point that touches the body and cuts them
    /// all with one kernel call (features/hole.md section 3.3).
    fn build(
        params: &HoleParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let (state, body) = solid_body(context, &params.body)?;
        let mut warnings = Vec::new();
        let size = HoleSize::checked(params, context, &mut warnings)?;
        let sketch = context.sketch("sketch", &params.sketch)?;
        let plane = sketch.placement.to_kernel();
        let direction = if params.reverse {
            plane.normal
        } else {
            plane.normal.scaled(-1.0)
        };
        let mut points: Vec<(&PointRef, Point)> = Vec::new();
        for (index, point) in params.points.iter().enumerate() {
            let position = sketch
                .point(point)
                .ok_or_else(|| FeatureError::UnknownEntity {
                    field: context.field(&format!("points[{index}]")),
                    sketch: params.sketch.clone(),
                    entity: point.to_string(),
                    noun: Noun::Point,
                })?;
            if !points.iter().any(|(used, _)| *used == point) {
                let [x, y, z] = sketch.to_world(position);
                points.push((point, Point::new(x, y, z)));
            }
        }

        let body_volume = body
            .volume()
            .map_err(|error| FeatureError::Kernel { error })?;
        let mut tools = Vec::new();
        for (point, position) in points {
            if context.is_canceled() {
                return Err(FeatureError::Canceled);
            }
            let length = match size.depth {
                Some(depth) => Some(depth),
                None => size.through_length(farthest_corner(body, position, direction)?),
            };
            let frame = Frame {
                position,
                radial: plane.x_axis,
                direction,
            };
            let tool = match length {
                Some(length) => {
                    let tool = hole_tool(context.feature(), point, &frame, &size.section(length))?;
                    touches(body, body_volume, &tool.shape)?.then_some(tool)
                }
                None => None,
            };
            match tool {
                Some(tool) => tools.push(tool),
                None => warnings.push(FeatureWarning::HoleMissesBody {
                    point: point.to_string(),
                    body: params.body.clone(),
                }),
            }
        }

        if tools.is_empty() {
            warnings.push(FeatureWarning::NoEffect {
                body: params.body.clone(),
            });
            return Ok(bodies(
                vec![unchanged(&params.body, state, BodyChange::Cut)],
                warnings,
            ));
        }
        let (update, op_warnings) = apply_op(context, &params.body, BodyOp::Cut, tools)?;
        warnings.extend(op_warnings);
        Ok(bodies(vec![update], warnings))
    }

    /// `hole(points, diameter, depth, counterbore=(diameter, depth), countersink=(diameter,
    /// angle), tip_angle=..., reverse=True)`, without `depth` for a through-all hole
    /// (features/hole.md section 2, Python).
    fn python(params: &HoleParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let points = params
            .points
            .iter()
            .map(|point| python_point(&params.sketch, point, names))
            .collect::<Option<Vec<_>>>()?;
        let mut positional = vec![
            one_or_list(points),
            PythonValue::expression(&params.diameter, names),
        ];
        if let HoleExtent::Distance { expr } = &params.extent {
            positional.push(PythonValue::expression(expr, names));
        }
        let pair = |first: &Expr, second: &Expr| {
            PythonValue::Tuple(vec![
                PythonValue::expression(first, names),
                PythonValue::expression(second, names),
            ])
        };
        let mut keywords = Vec::new();
        match &params.hole_type {
            HoleType::Simple => {}
            HoleType::Counterbore { diameter, depth } => {
                keywords.push(("counterbore".to_string(), pair(diameter, depth)));
            }
            HoleType::Countersink { diameter, angle } => {
                keywords.push(("countersink".to_string(), pair(diameter, angle)));
            }
        }
        if let Some(tip_angle) = &params.tip_angle {
            keywords.push((
                "tip_angle".to_string(),
                PythonValue::expression(tip_angle, names),
            ));
        }
        if params.reverse {
            keywords.push(flag("reverse"));
        }
        Some(call("hole", positional, keywords))
    }
}

/// A sketch point in Python: `<entity variable>.<role>`, or the variable of a point entity.
fn python_point(sketch: &Id, point: &PointRef, names: &dyn PythonNames) -> Option<PythonValue> {
    let variable = names.entity(sketch, &point.entity)?;
    let role = match point.role {
        PointRole::Point => return Some(PythonValue::Name(variable)),
        PointRole::Start => "start",
        PointRole::End => "end",
        PointRole::Center => "center",
    };
    Some(PythonValue::Name(format!("{variable}.{role}")))
}

/// The head of a hole with its evaluated sizes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Head {
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
struct HoleSize {
    /// Radius of the bore.
    radius: f64,
    /// The head.
    head: Head,
    /// Depth to the shoulder of a blind hole; None for a through-all hole.
    depth: Option<f64>,
    /// Included tip angle of a blind hole with a drill point; None for a flat end.
    tip: Option<f64>,
}

impl HoleSize {
    /// Evaluates the values and checks them in the order of features/hole.md section 3.3 step 2.
    /// A tip angle on a through-all hole adds an `Ignored` warning and is not evaluated.
    fn checked(
        params: &HoleParams,
        context: &BuildContext<'_>,
        warnings: &mut Vec<FeatureWarning>,
    ) -> Result<HoleSize, FeatureError> {
        let diameter = context.eval_length("diameter", &params.diameter)?;
        let head = match &params.hole_type {
            HoleType::Simple => None,
            HoleType::Counterbore { diameter, depth } => Some((
                context.eval_length("hole_type.diameter", diameter)?,
                context.eval_length("hole_type.depth", depth)?,
            )),
            HoleType::Countersink { diameter, angle } => Some((
                context.eval_length("hole_type.diameter", diameter)?,
                context.eval_angle("hole_type.angle", angle)?,
            )),
        };
        let depth = match &params.extent {
            HoleExtent::Distance { expr } => Some(context.eval_length("extent.expr", expr)?),
            HoleExtent::ThroughAll => None,
        };

        let length = |field: &str, value: f64, bound: Bound, bound_name: Option<&str>| {
            check_range(context, field, FieldUnit::Length, value, bound, bound_name)
        };
        let angle = |field: &str, value: f64, bound: Bound| {
            check_range(context, field, FieldUnit::Angle, value, bound, None)
        };
        length("diameter", diameter, Bound::GreaterThan(0.0), None)?;
        let wider = |head_diameter: f64| {
            length(
                "hole_type.diameter",
                head_diameter,
                Bound::GreaterThan(diameter),
                Some("the hole diameter"),
            )
        };
        let head = match (&params.hole_type, head) {
            (HoleType::Counterbore { .. }, Some((head_diameter, head_depth))) => {
                wider(head_diameter)?;
                length("hole_type.depth", head_depth, Bound::GreaterThan(0.0), None)?;
                if let Some(depth) = depth {
                    length(
                        "hole_type.depth",
                        head_depth,
                        Bound::LessThan(depth),
                        Some("the hole depth"),
                    )?;
                }
                Head::Counterbore {
                    radius: head_diameter / 2.0,
                    depth: head_depth,
                }
            }
            (HoleType::Countersink { .. }, Some((head_diameter, head_angle))) => {
                wider(head_diameter)?;
                angle("hole_type.angle", head_angle, Bound::GreaterThan(0.0))?;
                angle("hole_type.angle", head_angle, Bound::LessThan(PI))?;
                let slope = (head_angle / 2.0).tan();
                // The cone must end above the shoulder. The check compares the head diameter with
                // the bound its message names, so the message and the decision always agree.
                if let Some(depth) = depth {
                    length(
                        "hole_type.diameter",
                        head_diameter,
                        Bound::LessThan(diameter + 2.0 * depth * slope),
                        Some("the largest that fits the hole depth"),
                    )?;
                }
                Head::Countersink {
                    radius: head_diameter / 2.0,
                    depth: (head_diameter - diameter) / 2.0 / slope,
                }
            }
            _ => Head::Simple,
        };
        if let Some(depth) = depth {
            length("extent.expr", depth, Bound::GreaterThan(0.0), None)?;
        }
        let tip = match (&params.tip_angle, depth) {
            (None, _) => None,
            (Some(_), None) => {
                warnings.push(FeatureWarning::Ignored {
                    field: context.field("tip_angle"),
                    reason: IgnoredReason::TipAngleThroughAll,
                });
                None
            }
            (Some(expr), Some(_)) => {
                let tip = context.eval_angle("tip_angle", expr)?;
                angle("tip_angle", tip, Bound::GreaterThan(0.0))?;
                angle("tip_angle", tip, Bound::LessThan(PI))?;
                Some(tip)
            }
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
    /// the drilling direction: 1 mm past it, and None when that distance is not positive
    /// (features/hole.md section 3.3 step 4). It is also at least 1 mm past the head, so a head
    /// deeper than the body still leaves a bore that closes the cross-section.
    fn through_length(&self, farthest: f64) -> Option<f64> {
        (farthest > 0.0)
            .then(|| (farthest + THROUGH_ALL_MARGIN).max(self.head_depth() + THROUGH_ALL_MARGIN))
    }

    /// The half cross-section of a hole whose bore ends at `length`, as the end points of its
    /// segments in (radius, depth) coordinates, each with the role of the segment that ends there.
    /// It starts at the origin, runs out along the entry, down the head and the bore, in along the
    /// end, and back up the axis.
    fn section(&self, length: f64) -> Vec<(Role, [f64; 2])> {
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

/// The role of one segment of a hole's half cross-section, which names the face it sweeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    /// The segment on the axis, which sweeps no face.
    Axis,
    /// The disc closing the tool on the sketch plane.
    Entry,
    /// The counterbore wall.
    Counterbore,
    /// The counterbore's annular shoulder.
    CounterboreFloor,
    /// The countersink cone.
    Countersink,
    /// The wall of the hole diameter.
    Bore,
    /// The flat end of a blind hole.
    Bottom,
    /// The conical drill-point end of a blind hole.
    Tip,
}

impl Role {
    /// Every role in tag order: the tag of a curve is the position of its role here.
    const ALL: [Role; 8] = [
        Role::Axis,
        Role::Entry,
        Role::Counterbore,
        Role::CounterboreFloor,
        Role::Countersink,
        Role::Bore,
        Role::Bottom,
        Role::Tip,
    ];

    /// The tag of curves with this role.
    fn tag(self) -> u32 {
        match self {
            Role::Axis => 0,
            Role::Entry => 1,
            Role::Counterbore => 2,
            Role::CounterboreFloor => 3,
            Role::Countersink => 4,
            Role::Bore => 5,
            Role::Bottom => 6,
            Role::Tip => 7,
        }
    }

    /// The role of a curve tag.
    fn of_tag(tag: u32) -> Option<Role> {
        Role::ALL.get(usize::try_from(tag).ok()?).copied()
    }

    /// The role word of the faces it sweeps; None for the axis, which sweeps none.
    fn word(self) -> Option<&'static str> {
        match self {
            Role::Axis => None,
            Role::Entry => Some(roles::ENTRY),
            Role::Counterbore => Some(roles::COUNTERBORE),
            Role::CounterboreFloor => Some(roles::COUNTERBORE_FLOOR),
            Role::Countersink => Some(roles::COUNTERSINK),
            Role::Bore => Some(roles::BORE),
            Role::Bottom => Some(roles::BOTTOM),
            Role::Tip => Some(roles::TIP),
        }
    }
}

/// Where one hole is drilled.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Frame {
    /// The sketch point in world space, on the axis of the hole.
    position: Point,
    /// The unit direction of increasing radius in the cross-section: the sketch x axis, so the
    /// seam of the tool lies on the same side for every hole.
    radial: Vector,
    /// The unit drilling direction.
    direction: Vector,
}

impl Frame {
    /// The plane of the cross-section: through the point, spanned by the radial direction (its x
    /// axis) and the drilling direction (its y axis), so plane coordinates are (radius, depth).
    fn plane(&self) -> Plane {
        Plane {
            origin: self.position,
            normal: self.radial.cross(self.direction),
            x_axis: self.radial,
        }
    }

    /// The axis of the hole: through the point along the drilling direction.
    fn axis(&self) -> Axis {
        Axis {
            origin: self.position,
            direction: self.direction,
        }
    }
}

/// The tool of one hole: the full revolution of its half cross-section about its axis, with every
/// face named `<feature>/<role>/<point>`.
fn hole_tool(
    feature: &Id,
    point: &PointRef,
    frame: &Frame,
    section: &[(Role, [f64; 2])],
) -> Result<Tool, FeatureError> {
    let mut curves = Vec::with_capacity(section.len());
    let mut start = [0.0, 0.0];
    for &(role, end) in section {
        curves.push(TaggedCurve {
            tag: role.tag(),
            curve: PlanarCurve::Line {
                start: plane_point(start),
                end: plane_point(end),
            },
        });
        start = end;
    }
    let region = PlanarRegion {
        outer: PlanarLoop { curves },
        holes: Vec::new(),
    };
    let profile =
        make_profile(&frame.plane(), &[region]).map_err(|error| FeatureError::Kernel { error })?;
    let sweep =
        revolve(&profile, &frame.axis(), TAU).map_err(|error| FeatureError::Kernel { error })?;
    let point_text = point.to_string();
    let faces = sweep
        .faces
        .iter()
        .map(|face| {
            let word = match face {
                SweepFace::Lateral { tag } => Role::of_tag(*tag).and_then(Role::word),
                SweepFace::StartCap | SweepFace::EndCap => None,
            }
            .ok_or_else(|| FeatureError::Internal {
                message: format!("the hole tool has a face without a role: {face:?}"),
            })?;
            face_name(feature, word, Some(Generator::Sketch(&point_text))).map(FaceSource::New)
        })
        .collect::<Result<Vec<_>, FeatureError>>()?;
    let naming = name_shape(&sweep.shape, faces, &[])?;
    Ok(Tool {
        shape: sweep.shape,
        naming,
    })
}

/// True when the tool overlaps the body: their common volume is more than a billionth of the
/// body's (features/hole.md section 3.3 step 5). Disjoint bounding boxes need no kernel boolean.
fn touches(body: &Shape, body_volume: f64, tool: &Shape) -> Result<bool, FeatureError> {
    let body_bounds = body
        .bounding_box()
        .map_err(|error| FeatureError::Kernel { error })?;
    let tool_bounds = tool
        .bounding_box()
        .map_err(|error| FeatureError::Kernel { error })?;
    let overlap = (0..3).all(|axis| {
        body_bounds.min[axis] <= tool_bounds.max[axis]
            && tool_bounds.min[axis] <= body_bounds.max[axis]
    });
    if !overlap {
        return Ok(false);
    }
    let shared =
        common(body, std::slice::from_ref(tool)).map_err(|error| FeatureError::Kernel { error })?;
    let volume = shared
        .shape
        .volume()
        .map_err(|error| FeatureError::Kernel { error })?;
    Ok(volume > VOLUME_TOLERANCE * body_volume)
}

/// The kernel point of plane coordinates.
fn plane_point([x, y]: [f64; 2]) -> Point2 {
    Point2 { x, y }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::registry::Registry;
    use crate::test_support::{ScriptNames, id};

    /// `HOLE(points, extra)` of the spec: diameter 6, plus `extra` members.
    fn record(points: serde_json::Value, extra: serde_json::Value) -> serde_json::Value {
        let mut record = json!({"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk1", "points": points, "diameter": "6"});
        if let (Some(target), Some(source)) = (record.as_object_mut(), extra.as_object()) {
            target.extend(source.clone());
        }
        record
    }

    fn params(record: serde_json::Value) -> HoleParams {
        serde_json::from_value(record).expect("the hole params are valid")
    }

    fn through_all() -> serde_json::Value {
        json!({"extent": {"kind": "through_all"}})
    }

    /// Script names with the points `e-p1` and `e-p2` as `point_1` and `point_2`, `e-1` as
    /// `line_1` and `e-5` as `circle_1`.
    fn names() -> ScriptNames {
        let mut names = ScriptNames::default();
        for (entity, variable) in [
            ("e-p1", "point_1"),
            ("e-p2", "point_2"),
            ("e-1", "line_1"),
            ("e-5", "circle_1"),
        ] {
            names
                .entities
                .insert((id("f-sk1"), id(entity)), variable.to_string());
        }
        names
    }

    fn python(record: serde_json::Value) -> Option<String> {
        Hole::python(&params(record), &names()).map(|call| call.render(0))
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
    fn registers_through_the_public_registry() {
        let mut registry = Registry::new();

        assert_eq!(registry.register::<Hole>(), Ok(()));
    }

    #[test]
    fn a_simple_type_is_omitted() {
        let hole = params(record(
            json!(["e-p1"]),
            json!({"hole_type": {"kind": "simple"}, "extent": {"kind": "through_all"}}),
        ));

        let text = serde_json::to_value(&hole).expect("serializes");

        assert!(text.get("hole_type").is_none(), "{text}");
        assert!(text.get("tip_angle").is_none(), "{text}");
    }

    #[test]
    fn dependencies_list_expressions_in_spec_order() {
        let hole = params(record(
            json!(["e-p1"]),
            json!({
                "hole_type": {"kind": "countersink", "diameter": "12", "angle": "90 deg"},
                "extent": {"kind": "distance", "expr": "8"},
                "tip_angle": "118 deg"
            }),
        ));

        let uses: Vec<(String, FieldUnit)> = Hole::dependencies(&hole)
            .expressions
            .into_iter()
            .map(|usage| (usage.field, usage.kind))
            .collect();

        assert_eq!(
            uses,
            [
                ("diameter".to_string(), FieldUnit::Length),
                ("hole_type.diameter".to_string(), FieldUnit::Length),
                ("hole_type.angle".to_string(), FieldUnit::Angle),
                ("extent.expr".to_string(), FieldUnit::Length),
                ("tip_angle".to_string(), FieldUnit::Angle),
            ]
        );
    }

    #[test]
    fn dependencies_read_the_sketch_and_write_the_body() {
        let dependencies = Hole::dependencies(&params(record(json!(["e-p1"]), through_all())));

        assert_eq!(dependencies.features[0].id, id("f-sk1"));
        assert_eq!(dependencies.features[0].field, "sketch");
        assert_eq!(dependencies.bodies_written[0].id, id("b-1"));
        assert!(dependencies.bodies_read.is_empty());
    }

    #[test]
    fn validate_rejects_no_points() {
        let error = Hole::validate(&params(record(json!([]), through_all())))
            .expect_err("a hole needs a point");

        assert_eq!(error.to_string(), "Select at least one point for Points.");
    }

    #[test]
    fn roles_round_trip_through_their_tags() {
        for role in Role::ALL {
            assert_eq!(Role::of_tag(role.tag()), Some(role));
        }
        assert_eq!(Role::of_tag(8), None);
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
    fn the_cross_section_plane_has_depth_along_its_y_axis() {
        let frame = Frame {
            position: Point::new(1.0, 2.0, 3.0),
            radial: Vector::new(1.0, 0.0, 0.0),
            direction: Vector::new(0.0, 0.0, -1.0),
        };

        let plane = frame.plane();

        assert_eq!(plane.y_axis(), frame.direction);
        assert_eq!(plane.origin, frame.position);
    }

    #[test]
    fn python_writes_the_spec_example() {
        assert_eq!(
            python(record(
                json!(["e-p1", "e-p2"]),
                json!({
                    "diameter": "3.4 mm",
                    "hole_type": {"kind": "counterbore", "diameter": "6.5 mm", "depth": "3.5 mm"},
                    "extent": {"kind": "through_all"}
                }),
            )),
            Some("hole([point_1, point_2], 3.4*mm, counterbore=(6.5*mm, 3.5*mm))".to_string())
        );
    }

    #[test]
    fn python_writes_one_point_alone_with_its_role_and_a_depth() {
        assert_eq!(
            python(record(
                json!(["e-1.start"]),
                json!({"extent": {"kind": "distance", "expr": "8 mm"}, "tip_angle": "118 deg", "reverse": true}),
            )),
            Some("hole(line_1.start, 6, 8*mm, tip_angle=118*deg, reverse=True)".to_string())
        );
    }

    #[test]
    fn python_writes_a_countersink_pair() {
        assert_eq!(
            python(record(
                json!(["e-5.center"]),
                json!({
                    "hole_type": {"kind": "countersink", "diameter": "12", "angle": "90 deg"},
                    "extent": {"kind": "through_all"}
                }),
            )),
            Some("hole(circle_1.center, 6, countersink=(12, 90*deg))".to_string())
        );
    }

    #[test]
    fn python_needs_every_point_variable() {
        assert_eq!(python(record(json!(["origin"]), through_all())), None);
    }
}
