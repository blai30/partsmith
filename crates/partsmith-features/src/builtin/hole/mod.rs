//! The `hole` feature: simple, counterbore and countersink holes drilled at sketch points along the
//! sketch normal (features/hole.md).
//!
//! Each point gets its own tool, the full revolution of a half cross-section whose curves are
//! tagged by role, so every tool face is named `<feature>/<role>/<point>` (naming.md section
//! 3.6.8, `tool.rs`), sized by the checked values (`size.rs`). The tools whose boxes meet the
//! body's are cut from it with one kernel call, whose history tells which of them overlap it;
//! only tools that leave no face of their own there are measured against the body with a kernel
//! `common` (`overlap.rs`).

mod overlap;
mod size;
mod tool;

use partsmith_mesh::BoundingBox;
use partsmith_model::{Category, Expr, FeatureManifest, FieldUnit, Id, SelectionKind};
use partsmith_naming::{ElementName, Generator, roles};
use partsmith_occt_sys::{Point, Vector};
use partsmith_sketch::{PointRef, PointRole};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use self::overlap::{Candidate, Decision, cut_candidates};
use self::size::HoleSize;
use self::tool::{Frame, hole_tool};
use super::common::{
    bodies, body_hint, call, check_canceled, essential, face_name, farthest_corner, flag, has_face,
    hint, is_false, manifest, one_or_list, options_hint, require_selection, result_faces,
    selection_hint, solid_body, unchanged, unit_hint,
};
use crate::context::{BodyState, BuildContext};
use crate::deps::Dependencies;
use crate::error::{FeatureError, FeatureWarning, NoEffectHow, Noun};
use crate::feature::Feature;
use crate::output::{
    Absence, AbsentFace, BodyChange, Direction, FeatureOutput, SketchResult, Tool, apply_op,
    reverse_hits,
};
use crate::params::{BodyOp, PlacementExt};
use crate::python::{PythonCall, PythonNames, PythonValue};

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
        require_selection(&params.points, Hole::manifest, "points", Noun::Point)
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

    /// Checks the values, builds one tool per sketch point and cuts those that may touch the body
    /// with one kernel call (features/hole.md section 3.3). The body's bounding box is read once
    /// for every point; the cut is the result when every cut tool overlaps the body.
    fn build(
        params: &HoleParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let (state, body) = solid_body(context, "body", &params.body)?;
        let mut warnings = Vec::new();
        let size = HoleSize::checked(params, context, &mut warnings)?;
        let sketch = context.sketch("sketch", &params.sketch)?;
        let plane = sketch.placement.to_kernel();
        let direction = if params.reverse {
            plane.normal
        } else {
            plane.normal.scaled(-1.0)
        };
        let points = distinct_points(params, context, sketch, &mut warnings)?;
        let body_bounds = body.bounding_box()?;
        let drill = |direction: Vector, point: &PointRef, position: Point| {
            let length = size.depth.or_else(|| {
                size.through_length(farthest_corner(&body_bounds, position, direction))
            });
            let frame = Frame {
                position,
                radial: plane.x_axis,
                direction,
            };
            length
                .map(|length| {
                    hole_tool(
                        context.feature(),
                        point,
                        &frame,
                        &size.section(length),
                        &sketch.motion,
                    )
                })
                .transpose()
        };
        let drilling = drill_candidates(context, &points, &body_bounds, |point, position| {
            drill(direction, point, position)
        })?;
        let along = Direction {
            direction: [direction.x, direction.y, direction.z],
            sketch: Some((&params.sketch, &sketch.placement)),
        };
        let decision = cut_candidates(
            context,
            &params.body,
            state,
            body,
            &drilling.candidates,
            Some(along),
        )?;
        warnings.extend(misses(
            context,
            params,
            &points,
            &drilling.candidate_of_point,
            &decision.overlaps,
            |point| drill(direction.scaled(-1.0), point.point, point.world),
        )?);
        let overlaps = decision.overlaps.clone();
        let mut output = finish(
            context,
            &params.body,
            state,
            drilling.candidates,
            decision,
            FinishInputs {
                direction: along,
                entries: params.points.len(),
                warnings,
            },
        )?;
        output.absent = absent_ends(&drilling.ends, &overlaps, &output);
        Ok(output)
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

/// One distinct point of a hole, where it lies in its sketch and in the world.
struct HolePoint<'a> {
    /// The point reference.
    point: &'a PointRef,
    /// Its sketch coordinates.
    local: [f64; 2],
    /// Its world position.
    world: Point,
}

/// Each distinct point of `params.points`, in order, with its position on `sketch`. A point the
/// sketch no longer has is skipped with a `PointMissing` warning, so one deleted point never fails
/// the other holes; when none is left, the first is `UnknownEntity` at its entry (features/hole.md
/// section 3.3 step 4).
fn distinct_points<'a>(
    params: &'a HoleParams,
    context: &BuildContext<'_>,
    sketch: &SketchResult,
    warnings: &mut Vec<FeatureWarning>,
) -> Result<Vec<HolePoint<'a>>, FeatureError> {
    let mut points: Vec<HolePoint<'a>> = Vec::new();
    for (index, point) in params.points.iter().enumerate() {
        let Some(local) = sketch.point(point) else {
            warnings.push(FeatureWarning::PointMissing {
                field: context.field(&format!("points[{index}]")),
                position: index,
                sketch: params.sketch.clone(),
                point: point.to_string(),
            });
            continue;
        };
        if !points.iter().any(|used| used.point == point) {
            points.push(HolePoint {
                point,
                local,
                world: crate::point(sketch.to_world(local)),
            });
        }
    }
    if points.is_empty() {
        let first = params
            .points
            .first()
            .ok_or_else(|| FeatureError::internal("a hole without points passed validation"))?;
        return Err(FeatureError::UnknownEntity {
            field: context.field("points[0]"),
            sketch: params.sketch.clone(),
            entity: first.to_string(),
            noun: Noun::Point,
        });
    }
    Ok(points)
}

/// A `HoleMissesBody` for each point whose tool does not overlap the body (`overlaps` of its candidate, which
/// `candidate_of_point` gives, none without one), by its sketch coordinates (features/hole.md section 3.3 step 5). On this warning
/// path only, `drilled_back` builds the point's tool the other way round to decide whether Reverse works, and only
/// when no point overlaps: Reverse turns every point, so it would make one that reaches the body miss.
fn misses(
    context: &BuildContext<'_>,
    params: &HoleParams,
    points: &[HolePoint<'_>],
    candidate_of_point: &[Option<usize>],
    overlaps: &[bool],
    drilled_back: impl Fn(&HolePoint<'_>) -> Result<Option<Tool>, FeatureError>,
) -> Result<Vec<FeatureWarning>, FeatureError> {
    let reversible = !overlaps.contains(&true);
    let mut warnings = Vec::new();
    for (point, candidate) in points.iter().zip(candidate_of_point) {
        if candidate.is_some_and(|position| overlaps[position]) {
            continue;
        }
        let reversed = if reversible {
            drilled_back(point)?
        } else {
            None
        };
        let reverse_hits = match reversed {
            Some(reversed) => reverse_hits(context, &params.body, &[reversed])?,
            None => false,
        };
        warnings.push(FeatureWarning::HoleMissesBody {
            sketch: params.sketch.clone(),
            point: point.point.to_string(),
            position: point.local,
            body: params.body.clone(),
            reverse_hits,
        });
    }
    Ok(warnings)
}

/// The tools of a hole's points that may overlap the body.
struct Drilling {
    /// For each point, the position in `candidates` of its tool, when it has one.
    candidate_of_point: Vec<Option<usize>>,
    /// The tools whose bounding boxes meet the body's, in point order.
    candidates: Vec<Candidate>,
    /// The end and entry faces of each candidate's tool, in the same order.
    ends: Vec<ToolEnds>,
}

/// The faces of one hole's tool that tell how far it reaches (features/hole.md section 3.3 step 6).
struct ToolEnds {
    /// The face where the tool ends: `<feature>/bottom/<point>`, or `<feature>/tip/<point>` with a tip.
    end: ElementName,
    /// The face where it starts, `<feature>/entry/<point>`.
    entry: ElementName,
}

impl ToolEnds {
    /// The end and entry of the tool `feature` drills at `point`, whose naming map is `tool`.
    fn of(feature: &Id, point: &PointRef, tool: &Tool) -> Result<ToolEnds, FeatureError> {
        let point = point.to_string();
        let name = |role| face_name(feature, role, Some(Generator::Sketch(&point)));
        let tip = name(roles::TIP)?;
        let end = if tool.naming.index(&tip).is_some() {
            tip
        } else {
            name(roles::BOTTOM)?
        };
        Ok(ToolEnds {
            end,
            entry: name(roles::ENTRY)?,
        })
    }
}

/// The end faces that the result of a hole's cut does not have, for each tool that overlaps the body: `Through`
/// when its entry is not in the result either, else `EndOutside` (features/hole.md section 3.3 step 6). A hole
/// without effect records none, since its `NoEffect` warning tells why none of its faces are there.
fn absent_ends(ends: &[ToolEnds], overlaps: &[bool], output: &FeatureOutput) -> Vec<AbsentFace> {
    let Some(result) = result_faces(output) else {
        return Vec::new();
    };
    ends.iter()
        .zip(overlaps)
        .filter(|(tool, overlaps)| **overlaps && !has_face(&result, &tool.end))
        .map(|(tool, _)| AbsentFace {
            name: tool.end.clone(),
            why: if has_face(&result, &tool.entry) {
                Absence::EndOutside
            } else {
                Absence::Through
            },
        })
        .collect()
}

/// The tool `drill` builds for each point (its faces with the point sketch's motion), kept as a
/// candidate when its bounding box meets `body_bounds` (features/hole.md section 3.3 steps 4, 5
/// and 7). A through-all hole whose body lies behind its point gets no tool.
fn drill_candidates(
    context: &BuildContext<'_>,
    points: &[HolePoint<'_>],
    body_bounds: &BoundingBox,
    drill: impl Fn(&PointRef, Point) -> Result<Option<Tool>, FeatureError>,
) -> Result<Drilling, FeatureError> {
    let mut drilling = Drilling {
        candidate_of_point: Vec::with_capacity(points.len()),
        candidates: Vec::new(),
        ends: Vec::new(),
    };
    for point in points {
        check_canceled(context)?;
        let mut candidate = None;
        if let Some(tool) = drill(point.point, point.world)? {
            let bounds = tool.shape.bounding_box()?;
            if bounds.intersects(body_bounds) {
                candidate = Some(drilling.candidates.len());
                drilling
                    .ends
                    .push(ToolEnds::of(context.feature(), point.point, &tool)?);
                drilling.candidates.push(Candidate { tool, bounds });
            }
        }
        drilling.candidate_of_point.push(candidate);
    }
    Ok(drilling)
}

/// What `finish` adds to the hole's output besides the decision's cut.
struct FinishInputs<'a> {
    /// The drilling direction with the point sketch, for the body rules.
    direction: Direction<'a>,
    /// The number of entries of `points`, which decides whether `NoEffect` repeats the misses.
    entries: usize,
    /// The warnings raised so far.
    warnings: Vec<FeatureWarning>,
}

/// The output once `decision` says which candidates overlap the body, after the warnings raised
/// so far (features/hole.md section 3.3 step 6): the decision's cut when it is the result, else
/// the body cut again with the overlapping tools only, or the body unchanged when none overlaps,
/// with `NoEffect` (`Missed`) when `points` holds two or more entries; a single point's
/// `HoleMissesBody` already says why (features.md section 3.15).
fn finish(
    context: &BuildContext<'_>,
    body: &Id,
    state: &BodyState,
    candidates: Vec<Candidate>,
    decision: Decision,
    inputs: FinishInputs<'_>,
) -> Result<FeatureOutput, FeatureError> {
    let FinishInputs {
        direction,
        entries,
        mut warnings,
    } = inputs;
    let outcome = match decision.result {
        Some(result) => result,
        None => {
            let tools: Vec<Tool> = candidates
                .into_iter()
                .zip(&decision.overlaps)
                .filter(|(_, overlaps)| **overlaps)
                .map(|(candidate, _)| candidate.tool)
                .collect();
            if tools.is_empty() {
                if entries >= 2 {
                    warnings.push(FeatureWarning::NoEffect {
                        body: body.clone(),
                        how: NoEffectHow::Missed,
                        reverse_hits: false,
                        attach: None,
                    });
                }
                return Ok(bodies(
                    vec![unchanged(body, state, BodyChange::Cut)],
                    warnings,
                ));
            }
            apply_op(context, body, BodyOp::Cut, tools, Some(direction))
        }
    };
    let (update, op_warnings) = outcome?;
    warnings.extend(op_warnings);
    Ok(bodies(vec![update], warnings))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::{ScriptNames, id};

    /// `HOLE(points, extra)` of the spec: diameter 6, plus `extra` members.
    pub(super) fn record(points: serde_json::Value, extra: serde_json::Value) -> serde_json::Value {
        let mut record = json!({"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk1", "points": points, "diameter": "6"});
        if let (Some(target), Some(source)) = (record.as_object_mut(), extra.as_object()) {
            target.extend(source.clone());
        }
        record
    }

    pub(super) fn params(record: serde_json::Value) -> HoleParams {
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
