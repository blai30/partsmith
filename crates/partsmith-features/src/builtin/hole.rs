//! The `hole` feature: simple, counterbore and countersink holes drilled at sketch points along the
//! sketch normal (docs/spec/features/hole.md).
//!
//! Each point gets its own tool, the revolution of a half cross-section whose curves are tagged by
//! role, so every tool face is named `<feature>/<role>/<point>` (naming.md section 3.6.8). The
//! tools are cut from the body with one kernel call.

use std::f64::consts::{PI, TAU};

use partsmith_model::{
    Category, Expr, FeatureManifest, FieldHint, FieldUnit, Id, LINEAR_TOLERANCE, SelectionKind,
};
use partsmith_naming::{ElementName, FaceSource, Generator, roles};
use partsmith_occt_sys::{
    Axis, PlanarCurve, PlanarLoop, PlanarRegion, Plane, Point, Point2, Shape, SweepFace,
    TaggedCurve, common, make_profile, revolve,
};
use partsmith_sketch::{PointRef, PointRole};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::extrude::{THROUGH_ALL_MARGIN, farthest_corner, hint, options, vector};
use crate::{
    BodyChange, BodyOp, BodyUpdate, Bound, BuildContext, Dependencies, Feature, FeatureError,
    FeatureOutput, FeatureWarning, IgnoredReason, Noun, OutputData, PythonCall, PythonNames,
    PythonValue, Tool, apply_op, name_shape,
};

/// Relative common volume at or below which a tool does not touch the body (spec section 3.3 step
/// 5, the volume rule of features.md section 3.5).
const VOLUME_TOLERANCE: f64 = 1e-9;

/// Cuts round holes at sketch points.
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
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
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
        let length = |path: &str, label: &str| FieldHint {
            unit: Some(FieldUnit::Length),
            ..hint(path, label, true)
        };
        FeatureManifest {
            label: "Hole".to_string(),
            category: Category::Create,
            icon: "hole".to_string(),
            version: 1,
            fields: vec![
                FieldHint {
                    selection: Some(SelectionKind::SketchPoint),
                    ..hint("points", "Points", true)
                },
                length("diameter", "Diameter"),
                FieldHint {
                    options: options(&[
                        ("simple", "Simple"),
                        ("counterbore", "Counterbore"),
                        ("countersink", "Countersink"),
                    ]),
                    ..hint("hole_type.kind", "Type", true)
                },
                length("hole_type.diameter", "Head diameter"),
                length("hole_type.depth", "Counterbore depth"),
                FieldHint {
                    unit: Some(FieldUnit::Angle),
                    ..hint("hole_type.angle", "Countersink angle", true)
                },
                FieldHint {
                    options: options(&[("distance", "Depth"), ("through_all", "Through all")]),
                    ..hint("extent.kind", "Extent", true)
                },
                length("extent.expr", "Depth"),
                FieldHint {
                    unit: Some(FieldUnit::Angle),
                    ..hint("tip_angle", "Tip angle", false)
                },
                hint("reverse", "Reverse", false),
                FieldHint {
                    selection: Some(SelectionKind::Sketch),
                    ..hint("sketch", "Sketch", false)
                },
                FieldHint {
                    selection: Some(SelectionKind::Body),
                    ..hint("body", "Body", false)
                },
            ],
        }
    }

    /// The sketch, the cut body and the expressions in the order of spec section 3.2.
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
    /// all at once (spec section 3.3).
    fn build(params: &HoleParams, context: &BuildContext<'_>) -> Result<FeatureOutput, FeatureError> {
        let state = context.body(&params.body)?;
        let Some(body) = state.shape() else {
            return Err(FeatureError::BodyEmpty {
                body: params.body.clone(),
                use_new: false,
            });
        };
        let mut warnings = Vec::new();
        let size = HoleSize::checked(params, context, &mut warnings)?;
        let sketch = context.sketch("sketch", &params.sketch)?;
        let placement = sketch.placement;
        let direction = if params.reverse {
            placement.normal
        } else {
            placement.normal.map(|component| -component)
        };
        let mut points: Vec<(&PointRef, [f64; 3])> = Vec::new();
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
                points.push((point, sketch.to_world(position)));
            }
        }

        let body_bounds = body
            .bounding_box()
            .map_err(|error| FeatureError::Kernel { error })?;
        let body_volume = body.volume().map_err(|error| FeatureError::Kernel { error })?;
        let mut tools = Vec::new();
        for (point, position) in points {
            if context.is_canceled() {
                return Err(FeatureError::Canceled);
            }
            let length = match size.depth {
                Some(depth) => Some(depth),
                None => {
                    let farthest =
                        farthest_corner(body_bounds.min, body_bounds.max, position, direction);
                    (farthest > LINEAR_TOLERANCE).then(|| size.through_length(farthest))
                }
            };
            let tool = match length {
                Some(length) => {
                    let frame = Frame {
                        position,
                        radial: placement.x_axis,
                        direction,
                    };
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

        let update = if tools.is_empty() {
            warnings.push(FeatureWarning::NoEffect {
                body: params.body.clone(),
            });
            BodyUpdate {
                body: params.body.clone(),
                shape: Some(body.clone()),
                naming: state.naming().clone(),
                change: BodyChange::Cut,
            }
        } else {
            let (update, op_warnings) = apply_op(context, &params.body, BodyOp::Cut, tools)?;
            warnings.extend(op_warnings);
            update
        };
        Ok(FeatureOutput {
            data: OutputData::Bodies(vec![update]),
            warnings,
        })
    }

    /// At least one point (spec section 3.1).
    fn validate(params: &HoleParams) -> Result<(), FeatureError> {
        if params.points.is_empty() {
            return Err(FeatureError::EmptySelection {
                field: crate::FieldRef::new("points", &Hole::manifest()),
                noun: Noun::Point,
            });
        }
        Ok(())
    }

    /// `hole(points, diameter, depth, counterbore=(d, depth), countersink=(d, angle),
    /// tip_angle=..., reverse=True)`, without `depth` for a through-all hole (spec section 2,
    /// Python).
    fn python(params: &HoleParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let mut points = params
            .points
            .iter()
            .map(|point| python_point(&params.sketch, point, names))
            .collect::<Option<Vec<_>>>()?;
        let points = if points.len() == 1 {
            points.remove(0)
        } else {
            PythonValue::List(points)
        };
        let mut call = PythonCall::new("hole");
        call.positional.push(points);
        call.positional
            .push(PythonValue::expression(&params.diameter, names));
        if let HoleExtent::Distance { expr } = &params.extent {
            call.positional.push(PythonValue::expression(expr, names));
        }
        let pair = |first: &Expr, second: &Expr| {
            PythonValue::Tuple(vec![
                PythonValue::expression(first, names),
                PythonValue::expression(second, names),
            ])
        };
        match &params.hole_type {
            HoleType::Simple => {}
            HoleType::Counterbore { diameter, depth } => call
                .keywords
                .push(("counterbore".to_string(), pair(diameter, depth))),
            HoleType::Countersink { diameter, angle } => call
                .keywords
                .push(("countersink".to_string(), pair(diameter, angle))),
        }
        if let Some(tip_angle) = &params.tip_angle {
            call.keywords.push((
                "tip_angle".to_string(),
                PythonValue::expression(tip_angle, names),
            ));
        }
        if params.reverse {
            call.keywords
                .push(("reverse".to_string(), PythonValue::Bool(true)));
        }
        Some(call)
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
    /// A counterbore of this radius and depth.
    Counterbore {
        /// Radius of the counterbore.
        radius: f64,
        /// Depth of the counterbore floor.
        depth: f64,
    },
    /// A countersink of this top radius narrowing to the bore at `depth`.
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
    /// Evaluates the values and checks them in the order of spec section 3.3 step 2. A tip angle
    /// on a through-all hole adds an `Ignored` warning and is not evaluated.
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

        let check = |field: &str, kind, value, bound, bound_name: Option<&str>| {
            let ok = match bound {
                Bound::GreaterThan(limit) => value > limit,
                Bound::LessThan(limit) => value < limit,
                Bound::AtLeast(limit) => value >= limit,
                Bound::AtMost(limit) => value <= limit,
            };
            if ok {
                Ok(())
            } else {
                Err(FeatureError::OutOfRange {
                    field: context.field(field),
                    kind,
                    value,
                    bound,
                    bound_name: bound_name.map(str::to_string),
                })
            }
        };
        let length = FieldUnit::Length;
        let angle = FieldUnit::Angle;
        check("diameter", length, diameter, Bound::GreaterThan(0.0), None)?;
        let head = match (&params.hole_type, head) {
            (HoleType::Counterbore { .. }, Some((head_diameter, head_depth))) => {
                check(
                    "hole_type.diameter",
                    length,
                    head_diameter,
                    Bound::GreaterThan(diameter),
                    Some("the hole diameter"),
                )?;
                check("hole_type.depth", length, head_depth, Bound::GreaterThan(0.0), None)?;
                if let Some(depth) = depth {
                    check(
                        "hole_type.depth",
                        length,
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
                check(
                    "hole_type.diameter",
                    length,
                    head_diameter,
                    Bound::GreaterThan(diameter),
                    Some("the hole diameter"),
                )?;
                check("hole_type.angle", angle, head_angle, Bound::GreaterThan(0.0), None)?;
                check("hole_type.angle", angle, head_angle, Bound::LessThan(PI), None)?;
                let slope = (head_angle / 2.0).tan();
                let sink = (head_diameter - diameter) / 2.0 / slope;
                if let Some(depth) = depth
                    && sink >= depth
                {
                    check(
                        "hole_type.diameter",
                        length,
                        head_diameter,
                        Bound::LessThan(diameter + 2.0 * depth * slope),
                        Some("the largest that fits the hole depth"),
                    )?;
                }
                Head::Countersink {
                    radius: head_diameter / 2.0,
                    depth: sink,
                }
            }
            _ => Head::Simple,
        };
        if let Some(depth) = depth {
            check("extent.expr", length, depth, Bound::GreaterThan(0.0), None)?;
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
                check("tip_angle", angle, tip, Bound::GreaterThan(0.0), None)?;
                check("tip_angle", angle, tip, Bound::LessThan(PI), None)?;
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
    /// the drilling direction: 1 mm past it (spec section 3.3 step 4), and at least 1 mm past the
    /// head, so a head deeper than the body still leaves a bore that closes the cross-section.
    fn through_length(&self, farthest: f64) -> f64 {
        (farthest + THROUGH_ALL_MARGIN).max(self.head_depth() + THROUGH_ALL_MARGIN)
    }

    /// The half cross-section of a hole whose bore ends at `length`, as the end points of its
    /// segments in (radius, depth) coordinates, each with the role of the segment that ends there.
    /// It starts at the origin, runs out along the entry, down the head and bore, in along the end,
    /// and back up the axis.
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
    /// Every role, in tag order: a curve's tag is its role's position here.
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
        // The position always exists and is below 8.
        Role::ALL.iter().position(|role| *role == self).unwrap_or(0) as u32
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
    position: [f64; 3],
    /// The unit direction of increasing radius in the cross-section: the sketch x axis.
    radial: [f64; 3],
    /// The unit drilling direction.
    direction: [f64; 3],
}

impl Frame {
    /// The plane of the cross-section: through the point, spanned by the radial direction (its x
    /// axis) and the drilling direction (its y axis), so plane coordinates are (radius, depth).
    fn plane(&self) -> Plane {
        let [x, y, z] = self.radial;
        let [dx, dy, dz] = self.direction;
        Plane {
            origin: point(self.position),
            normal: vector([y * dz - z * dy, z * dx - x * dz, x * dy - y * dx]),
            x_axis: vector(self.radial),
        }
    }

    /// The axis of the hole: through the point along the drilling direction.
    fn axis(&self) -> Axis {
        Axis {
            origin: point(self.position),
            direction: vector(self.direction),
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
    let kernel = |error| FeatureError::Kernel { error };
    let mut curves = Vec::with_capacity(section.len());
    let mut start = [0.0, 0.0];
    for &(role, end) in section {
        curves.push(TaggedCurve {
            tag: role.tag(),
            curve: PlanarCurve::Line {
                start: point2(start),
                end: point2(end),
            },
        });
        start = end;
    }
    let region = PlanarRegion {
        outer: PlanarLoop { curves },
        holes: Vec::new(),
    };
    let profile = make_profile(&frame.plane(), &[region]).map_err(kernel)?;
    let sweep = revolve(&profile, &frame.axis(), TAU).map_err(kernel)?;
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
            ElementName::generated(feature, word, Some(Generator::Sketch(&point_text)))
                .map(FaceSource::New)
                .map_err(|error| FeatureError::Internal {
                    message: format!("naming a hole face failed: {error}"),
                })
        })
        .collect::<Result<Vec<_>, FeatureError>>()?;
    let naming = name_shape(&sweep.shape, faces, &[])?;
    Ok(Tool {
        shape: sweep.shape,
        naming,
    })
}

/// True when the tool overlaps the body: their common volume is more than a billionth of the
/// body's (spec section 3.3 step 5). Disjoint bounding boxes need no kernel boolean.
fn touches(body: &Shape, body_volume: f64, tool: &Shape) -> Result<bool, FeatureError> {
    let kernel = |error| FeatureError::Kernel { error };
    let body_bounds = body.bounding_box().map_err(kernel)?;
    let tool_bounds = tool.bounding_box().map_err(kernel)?;
    let overlap = (0..3).all(|axis| {
        body_bounds.min[axis] <= tool_bounds.max[axis]
            && tool_bounds.min[axis] <= body_bounds.max[axis]
    });
    if !overlap {
        return Ok(false);
    }
    let shared = common(body, std::slice::from_ref(tool)).map_err(kernel)?;
    let volume = shared.shape.volume().map_err(kernel)?;
    Ok(volume > VOLUME_TOLERANCE * body_volume)
}

/// The kernel point of world coordinates.
fn point([x, y, z]: [f64; 3]) -> Point {
    Point::new(x, y, z)
}

/// The kernel plane point of plane coordinates.
fn point2([x, y]: [f64; 2]) -> Point2 {
    Point2 { x, y }
}

#[cfg(test)]
mod tests {
    use partsmith_model::Units;
    use partsmith_sketch::RegionRef;
    use serde_json::json;

    use super::*;
    use crate::Registry;

    /// Script names with every point entity `e-p<n>` as `point_<n>`, `e-1` as `line_1` and
    /// `e-5` as `circle_1`.
    struct Names;

    impl PythonNames for Names {
        fn units(&self) -> Units {
            Units::Mm
        }

        fn body(&self, _body: &Id) -> Option<String> {
            Some("block".to_string())
        }

        fn feature(&self, _feature: &Id) -> Option<String> {
            Some("sketch_1".to_string())
        }

        fn entity(&self, _sketch: &Id, entity: &Id) -> Option<String> {
            match entity.as_str() {
                "e-1" => Some("line_1".to_string()),
                "e-5" => Some("circle_1".to_string()),
                other => other
                    .strip_prefix("e-p")
                    .map(|number| format!("point_{number}")),
            }
        }

        fn parameter(&self, _name: &str) -> Option<String> {
            None
        }

        fn regions(&self, _sketch: &Id) -> Option<Vec<RegionRef>> {
            None
        }

        fn default_op(&self, _body: &Id) -> BodyOp {
            BodyOp::New
        }
    }

    /// `HOLE(points, extra)` of the spec: diameter 6, plus `extra` members.
    fn params(points: serde_json::Value, extra: serde_json::Value) -> HoleParams {
        let mut record = json!({"body": "b-1", "sketch": "f-sk1", "points": points, "diameter": "6"});
        if let (Some(target), Some(source)) = (record.as_object_mut(), extra.as_object()) {
            target.extend(source.clone());
        }
        serde_json::from_value(record).expect("the hole params are valid")
    }

    fn python(params: &HoleParams) -> Option<String> {
        Hole::python(params, &Names).map(|call| call.render(0))
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
        let hole = params(
            json!(["e-p1"]),
            json!({"hole_type": {"kind": "simple"}, "extent": {"kind": "through_all"}}),
        );

        let text = serde_json::to_value(&hole).expect("serializes");

        assert!(text.get("hole_type").is_none(), "{text}");
        assert!(text.get("tip_angle").is_none(), "{text}");
    }

    #[test]
    fn dependencies_list_expressions_in_spec_order() {
        let hole = params(
            json!(["e-p1"]),
            json!({
                "hole_type": {"kind": "countersink", "diameter": "12", "angle": "90 deg"},
                "extent": {"kind": "distance", "expr": "8"},
                "tip_angle": "118 deg"
            }),
        );

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
        let dependencies =
            Hole::dependencies(&params(json!(["e-p1"]), json!({"extent": {"kind": "through_all"}})));

        assert_eq!(dependencies.features[0].id.as_str(), "f-sk1");
        assert_eq!(dependencies.features[0].field, "sketch");
        assert_eq!(dependencies.bodies_written[0].id.as_str(), "b-1");
        assert!(dependencies.bodies_read.is_empty());
    }

    #[test]
    fn validate_rejects_no_points() {
        let hole = params(json!([]), json!({"extent": {"kind": "through_all"}}));

        let error = Hole::validate(&hole).expect_err("a hole needs a point");

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

        assert_eq!(size(Head::Simple, None, None).through_length(10.0), 11.0);
        assert_eq!(size(head, None, None).through_length(2.0), 5.0);
    }

    #[test]
    fn the_cross_section_plane_has_depth_along_its_y_axis() {
        let frame = Frame {
            position: [1.0, 2.0, 3.0],
            radial: [1.0, 0.0, 0.0],
            direction: [0.0, 0.0, -1.0],
        };

        let plane = frame.plane();

        assert_eq!(plane.y_axis(), vector(frame.direction));
        assert_eq!(plane.origin, Point::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn python_writes_the_spec_example() {
        let hole = params(
            json!(["e-p1", "e-p2"]),
            json!({
                "diameter": "3.4 mm",
                "hole_type": {"kind": "counterbore", "diameter": "6.5 mm", "depth": "3.5 mm"},
                "extent": {"kind": "through_all"}
            }),
        );

        assert_eq!(
            python(&hole),
            Some("hole([point_1, point_2], 3.4*mm, counterbore=(6.5*mm, 3.5*mm))".to_string())
        );
    }

    #[test]
    fn python_writes_one_point_alone_with_its_role_and_a_depth() {
        let hole = params(
            json!(["e-1.start"]),
            json!({"extent": {"kind": "distance", "expr": "8 mm"}, "tip_angle": "118 deg", "reverse": true}),
        );

        assert_eq!(
            python(&hole),
            Some("hole(line_1.start, 6, 8*mm, tip_angle=118*deg, reverse=True)".to_string())
        );
    }

    #[test]
    fn python_writes_a_countersink_pair() {
        let hole = params(
            json!(["e-5.center"]),
            json!({
                "hole_type": {"kind": "countersink", "diameter": "12", "angle": "90 deg"},
                "extent": {"kind": "through_all"}
            }),
        );

        assert_eq!(
            python(&hole),
            Some("hole(circle_1.center, 6, countersink=(12, 90*deg))".to_string())
        );
    }

    #[test]
    fn python_needs_every_point_variable() {
        let hole = params(json!(["origin"]), json!({"extent": {"kind": "through_all"}}));

        assert_eq!(python(&hole), None);
    }
}
