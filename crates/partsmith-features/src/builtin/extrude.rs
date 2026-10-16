//! The `extrude` feature: sweeps closed regions of a sketch along its normal and combines the solid
//! with a body (features/extrude.md).

use partsmith_model::{Category, Expr, FeatureManifest, FieldUnit, Id, SelectionKind};
use partsmith_naming::{ElementKind, ElementName, NamingMap, roles};
use partsmith_occt_sys::{Point, Vector, extrude};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    THROUGH_ALL_MARGIN, apply_tool, body_hint, body_op_hint, call, essential, face_name,
    farthest_corner, flag, has_face, hint, is_false, is_positive_length, length_in_range, manifest,
    op_keyword, options_hint, positive_length, profile_arguments, require_selection, result_faces,
    selection_hint, swept_tool, unit_hint,
};
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::{Bound, FeatureError, Noun};
use crate::feature::Feature;
use crate::output::{Absence, AbsentFace, FeatureOutput};
use crate::params::{BodyOp, PlacementExt, ProfileRef};
use crate::python::{PythonCall, PythonNames, PythonValue};

/// Extrudes closed sketch regions along the sketch normal and combines the result with a body.
#[derive(Clone, Copy, Debug)]
pub struct Extrude;

/// Params of an extrude feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExtrudeParams {
    /// Target body.
    pub body: Id,
    /// Closed regions of one sketch.
    pub profile: ProfileRef,
    /// How far the extrusion goes.
    pub extent: ExtrudeExtent,
    /// Extrude against the sketch normal. Default false, omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub reverse: bool,
    /// How the extrusion combines with the body. Required.
    pub op: BodyOp,
}

/// How far an extrusion goes, measured along its direction from the sketch plane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExtrudeExtent {
    /// From the sketch plane to `expr` along the direction. Length, greater than 0.
    Distance {
        /// The distance.
        expr: Expr,
    },
    /// Total length `expr`, centered on the sketch plane. Length, greater than 0.
    Symmetric {
        /// The total length.
        expr: Expr,
    },
    /// `expr` along the direction and `opposite_expr` against it. Lengths, at least 0, not both 0.
    TwoSides {
        /// The distance along the direction.
        expr: Expr,
        /// The distance against the direction.
        opposite_expr: Expr,
    },
    /// From the sketch plane through the whole target body along the direction.
    ThroughAll,
}

impl Feature for Extrude {
    const TYPE: &'static str = "extrude";
    type Params = ExtrudeParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Extrude",
            Category::Create,
            "extrude",
            vec![
                essential(selection_hint(
                    "profile",
                    "Profile",
                    SelectionKind::SketchRegion,
                )),
                essential(options_hint(
                    "extent.kind",
                    "Extent",
                    &[
                        ("distance", "Distance"),
                        ("symmetric", "Symmetric"),
                        ("two_sides", "Two sides"),
                        ("through_all", "Through all"),
                    ],
                )),
                essential(unit_hint("extent.expr", "Distance", FieldUnit::Length)),
                essential(unit_hint(
                    "extent.opposite_expr",
                    "Opposite distance",
                    FieldUnit::Length,
                )),
                body_op_hint(),
                hint("reverse", "Reverse"),
                body_hint(),
            ],
        )
    }

    fn validate(params: &ExtrudeParams) -> Result<(), FeatureError> {
        require_selection(
            &params.profile.regions,
            Extrude::manifest,
            "profile",
            Noun::Region,
        )
    }

    fn dependencies(params: &ExtrudeParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_feature(&params.profile.sketch, "profile.sketch");
        dependencies.add_body_written(&params.body, "body");
        match &params.extent {
            ExtrudeExtent::Distance { expr } | ExtrudeExtent::Symmetric { expr } => {
                dependencies.add_expression("extent.expr", expr, FieldUnit::Length);
            }
            ExtrudeExtent::TwoSides {
                expr,
                opposite_expr,
            } => {
                dependencies.add_expression("extent.expr", expr, FieldUnit::Length);
                dependencies.add_expression(
                    "extent.opposite_expr",
                    opposite_expr,
                    FieldUnit::Length,
                );
            }
            ExtrudeExtent::ThroughAll => {}
        }
        dependencies
    }

    fn build(
        params: &ExtrudeParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let profile = context.profile("profile", &params.profile)?;
        let plane = profile.sketch.placement.to_kernel();
        let symmetric = matches!(params.extent, ExtrudeExtent::Symmetric { .. });
        let direction = if params.reverse && !symmetric {
            plane.normal.scaled(-1.0)
        } else {
            plane.normal
        };
        let (start, end) = interval(params, context, plane.origin, direction)?;
        let tool = swept_tool(context, &profile, |faces| {
            extrude(faces, direction, start, end)
        })?;
        let caps = match params.op {
            BodyOp::Cut => Some(CutCaps::of(context.feature(), &tool.naming)?),
            _ => None,
        };
        let messages = profile.into_messages();
        let mut output = apply_tool(context, &params.body, params.op, tool, messages)?;
        if let Some(caps) = caps {
            output.absent = caps.absent_from(&output);
        }
        Ok(output)
    }

    fn python(params: &ExtrudeParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let (sketch, regions) = profile_arguments(&params.profile, names)?;
        let mut positional = vec![sketch];
        let mut keywords: Vec<(String, PythonValue)> = regions.into_iter().collect();
        match &params.extent {
            ExtrudeExtent::Distance { expr } => {
                positional.push(PythonValue::expression(expr, names));
            }
            ExtrudeExtent::Symmetric { expr } => {
                positional.push(PythonValue::expression(expr, names));
                keywords.push(flag("symmetric"));
            }
            ExtrudeExtent::TwoSides { .. } => return None,
            ExtrudeExtent::ThroughAll => {}
        }
        if params.reverse {
            keywords.push(flag("reverse"));
        }
        if params.extent == ExtrudeExtent::ThroughAll {
            keywords.push(flag("through_all"));
        }
        keywords.extend(op_keyword(names, &params.body, params.op));
        Some(call("extrude", positional, keywords))
    }
}

/// The interval `[start, end]` swept along `direction`, measured from the sketch plane through
/// `origin` (features/extrude.md section 3.3).
fn interval(
    params: &ExtrudeParams,
    context: &BuildContext<'_>,
    origin: Point,
    direction: Vector,
) -> Result<(f64, f64), FeatureError> {
    let not_negative =
        |field: &str, expr: &Expr| length_in_range(context, field, expr, Bound::AtLeast(0.0));
    match &params.extent {
        ExtrudeExtent::Distance { expr } => {
            Ok((0.0, positive_length(context, "extent.expr", expr)?))
        }
        ExtrudeExtent::Symmetric { expr } => {
            let length = positive_length(context, "extent.expr", expr)?;
            Ok((-length / 2.0, length / 2.0))
        }
        ExtrudeExtent::TwoSides {
            expr,
            opposite_expr,
        } => {
            let along = not_negative("extent.expr", expr)?;
            let against = not_negative("extent.opposite_expr", opposite_expr)?;
            // Sides that together stay below the tolerance extrude nothing the kernel can build.
            if !is_positive_length(along + against) {
                return Err(FeatureError::BothSidesZero);
            }
            Ok((-against, along))
        }
        ExtrudeExtent::ThroughAll => {
            if params.op == BodyOp::New {
                return Err(FeatureError::ThroughAllNeedsBody);
            }
            let state = context.body(&params.body)?;
            let Some(shape) = state.shape() else {
                return Err(FeatureError::BodyEmpty {
                    field: context.field("body"),
                    body: params.body.clone(),
                    use_new: true,
                });
            };
            let reach = farthest_corner(&shape.bounding_box()?, origin, direction);
            if reach <= 0.0 {
                return Err(FeatureError::ThroughAllMisses {
                    body: params.body.clone(),
                });
            }
            Ok((0.0, reach + THROUGH_ALL_MARGIN))
        }
    }
}

/// The caps of a cut's tool, read before the cut, to find the end caps its result does not have
/// (features/extrude.md section 3.3 step 6).
struct CutCaps {
    /// The tool's end caps: `<feature>/end-cap`, or `end-cap#n` for several.
    end_caps: Vec<ElementName>,
    /// `<feature>/start-cap`, the split base of every start cap.
    start_cap: ElementName,
}

impl CutCaps {
    /// The caps of the tool `feature` named.
    fn of(feature: &Id, tool: &NamingMap) -> Result<CutCaps, FeatureError> {
        let end_cap = face_name(feature, roles::END_CAP, None)?;
        let end_caps = tool
            .names(ElementKind::Face)
            .map(|(_, name)| name)
            .filter(|name| name.split_base() == end_cap)
            .cloned()
            .collect();
        Ok(CutCaps {
            end_caps,
            start_cap: face_name(feature, roles::START_CAP, None)?,
        })
    }

    /// Each end cap the cut's result does not have (no face of that name or split from it):
    /// `Through` when no start cap is in the result either, else `EndOutside`. A cut without
    /// effect records none, since its `NoEffect` warning tells why none of its faces are there.
    fn absent_from(&self, output: &FeatureOutput) -> Vec<AbsentFace> {
        let Some(result) = result_faces(output) else {
            return Vec::new();
        };
        let why = if result
            .iter()
            .any(|name| name.split_base() == self.start_cap)
        {
            Absence::EndOutside
        } else {
            Absence::Through
        };
        self.end_caps
            .iter()
            .filter(|cap| !has_face(&result, cap))
            .map(|cap| AbsentFace {
                name: cap.clone(),
                why,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use partsmith_model::FeatureRecord;
    use serde_json::json;

    use super::*;
    use crate::registry::Registry;
    use crate::test_support::{ScriptNames, id};

    fn rectangle_profile() -> serde_json::Value {
        json!({"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]})
    }

    /// Script names in which the rectangle's region is every region of `f-sk1`.
    fn rectangle_names() -> ScriptNames {
        let mut names = ScriptNames::default();
        names.regions.insert(
            id("f-sk1"),
            vec![
                serde_json::from_value(json!({"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}))
                    .expect("region"),
            ],
        );
        names
    }

    #[test]
    fn python_writes_the_spec_example() {
        let record: FeatureRecord = serde_json::from_value(json!({
            "id": "f-ext1", "type": "extrude", "body": "b-1", "profile": rectangle_profile(),
            "extent": {"kind": "distance", "expr": "30 mm"}, "op": "new"
        }))
        .expect("a valid record");

        let python = Registry::with_builtins().python(&record, &rectangle_names());

        assert_eq!(python.render(0), r#"extrude(sketch_1, 30*mm, id="f-ext1")"#);
    }

    #[test]
    fn python_writes_the_sketch_and_distance() {
        let names = rectangle_names();
        let params = |extent: serde_json::Value, op: &str| -> ExtrudeParams {
            serde_json::from_value(json!({"body": "b-1", "profile": rectangle_profile(), "extent": extent, "reverse": true, "op": op})).expect("valid params")
        };

        let distance = Extrude::python(
            &params(json!({"kind": "distance", "expr": "30 mm"}), "new"),
            &names,
        )
        .expect("a dedicated form");
        let through = Extrude::python(&params(json!({"kind": "through_all"}), "cut"), &names)
            .expect("a dedicated form");
        let two_sides = Extrude::python(
            &params(
                json!({"kind": "two_sides", "expr": "1", "opposite_expr": "2"}),
                "new",
            ),
            &names,
        );

        assert_eq!(distance.render(0), "extrude(sketch_1, 30*mm, reverse=True)");
        assert_eq!(
            through.render(0),
            r#"extrude(sketch_1, reverse=True, through_all=True, op="cut")"#
        );
        assert_eq!(two_sides, None);
    }

    #[test]
    fn python_writes_regions_that_are_not_every_region() {
        let names = {
            let mut names = ScriptNames::default();
            names.regions.insert(id("f-sk1"), Vec::new());
            names
        };
        let params: ExtrudeParams = serde_json::from_value(json!({"body": "b-1", "profile": rectangle_profile(), "extent": {"kind": "symmetric", "expr": "4"}, "op": "new"})).expect("valid params");

        let python = Extrude::python(&params, &names).expect("a dedicated form");

        // The call is longer than 88 characters on one line, so it takes one argument per line.
        assert_eq!(
            python.render(0),
            [
                "extrude(",
                "    sketch_1,",
                "    4,",
                r#"    regions=[Region("e-1,e-2,e-3,e-4", "c=40,25|a=4000")],"#,
                "    symmetric=True,",
                ")",
            ]
            .join("\n")
        );
    }
}
