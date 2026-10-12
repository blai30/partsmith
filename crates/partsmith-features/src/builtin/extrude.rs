//! The `extrude` feature: sweeps closed regions of one sketch along the sketch normal and combines
//! the solid with a body (docs/spec/features/extrude.md).
//!
//! The naming of swept profiles (naming.md sections 3.6.2 and 3.6.3), the profile and op arguments
//! of the Python forms, the through-all distance and the manifest hint helpers are shared with the
//! other features of this group (`sketch`, `revolve`, `hole`, `datum_plane`).

use partsmith_model::{
    Category, Expr, FeatureManifest, FieldHint, FieldUnit, Id, LINEAR_TOLERANCE, OptionHint,
    SelectionKind,
};
use partsmith_naming::{ElementName, FaceSource, Generator, roles};
use partsmith_occt_sys::{SweepFace, SweepResult, Vector, extrude};
use partsmith_sketch::Placement;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    BodyOp, Bound, BuildContext, Dependencies, Feature, FeatureError, FeatureOutput, Noun,
    OutputData, ProfileRef, PythonCall, PythonNames, PythonValue, Tool, apply_op, name_shape,
};

/// How far past the farthest corner of the body's bounding box a through-all sweep goes, in
/// millimeters (spec section 3.3, decision D21).
pub(super) const THROUGH_ALL_MARGIN: f64 = 1.0;

/// Extrudes closed sketch regions along the sketch normal and combines the result with a body.
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
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
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
        FeatureManifest {
            label: "Extrude".to_string(),
            category: Category::Create,
            icon: "extrude".to_string(),
            version: 1,
            fields: vec![
                FieldHint {
                    selection: Some(SelectionKind::SketchRegion),
                    ..hint("profile", "Profile", true)
                },
                FieldHint {
                    options: options(&[
                        ("distance", "Distance"),
                        ("symmetric", "Symmetric"),
                        ("two_sides", "Two sides"),
                        ("through_all", "Through all"),
                    ]),
                    ..hint("extent.kind", "Extent", true)
                },
                FieldHint {
                    unit: Some(FieldUnit::Length),
                    ..hint("extent.expr", "Distance", true)
                },
                FieldHint {
                    unit: Some(FieldUnit::Length),
                    ..hint("extent.opposite_expr", "Opposite distance", true)
                },
                op_hint(),
                hint("reverse", "Reverse", false),
                FieldHint {
                    selection: Some(SelectionKind::Body),
                    ..hint("body", "Body", false)
                },
            ],
        }
    }

    /// The profile's sketch, the written body and the extent lengths (spec section 3.2).
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

    /// Sweeps the union of the resolved regions over the extent and applies the op (spec
    /// section 3.3).
    fn build(
        params: &ExtrudeParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let profile = context.profile("profile", &params.profile)?;
        let mut warnings = profile.warnings.clone();
        let placement = profile.sketch.placement;
        let direction = direction(&placement, params);
        let (start, end) = interval(params, context, &placement, direction)?;
        let faces = profile.faces()?;
        if context.is_canceled() {
            return Err(FeatureError::Canceled);
        }
        let sweep = extrude(&faces.profile, vector(direction), start, end)
            .map_err(|error| FeatureError::Kernel { error })?;
        let tool = sweep_tool(context.feature(), &faces.entities, sweep)?;
        let (update, op_warnings) = apply_op(context, &params.body, params.op, vec![tool])?;
        warnings.extend(op_warnings);
        Ok(FeatureOutput {
            data: OutputData::Bodies(vec![update]),
            warnings,
        })
    }

    /// At least one region (spec section 3.1).
    fn validate(params: &ExtrudeParams) -> Result<(), FeatureError> {
        if params.profile.regions.is_empty() {
            return Err(FeatureError::EmptySelection {
                field: crate::FieldRef::new("profile", &Extrude::manifest()),
                noun: Noun::Region,
            });
        }
        Ok(())
    }

    /// `extrude(profile, distance, regions=..., symmetric=True, reverse=True, through_all=True,
    /// op=...)`. A two-sided extent has no dedicated form (spec section 2, Python).
    fn python(params: &ExtrudeParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let (distance, symmetric, through_all) = match &params.extent {
            ExtrudeExtent::Distance { expr } => (Some(expr), false, false),
            ExtrudeExtent::Symmetric { expr } => (Some(expr), true, false),
            ExtrudeExtent::TwoSides { .. } => return None,
            ExtrudeExtent::ThroughAll => (None, false, true),
        };
        let (sketch, regions) = python_profile(&params.profile, names)?;
        let mut call = PythonCall::new("extrude");
        call.positional.push(sketch);
        if let Some(expr) = distance {
            call.positional.push(PythonValue::expression(expr, names));
        }
        call.keywords.extend(regions);
        for (flag, set) in [
            ("symmetric", symmetric),
            ("reverse", params.reverse),
            ("through_all", through_all),
        ] {
            if set {
                call.keywords
                    .push((flag.to_string(), PythonValue::Bool(true)));
            }
        }
        call.keywords
            .extend(python_op(params.op, &params.body, names));
        Some(call)
    }
}

/// The unit extrusion direction: the sketch normal, reversed when `reverse` is set, except for a
/// symmetric extent, which is the same solid with the same names either way (spec section 3.3
/// step 2).
fn direction(placement: &Placement, params: &ExtrudeParams) -> [f64; 3] {
    let symmetric = matches!(params.extent, ExtrudeExtent::Symmetric { .. });
    if params.reverse && !symmetric {
        placement.normal.map(|component| -component)
    } else {
        placement.normal
    }
}

/// The interval `[start, end]` along `direction`, measured from the sketch plane, with the range
/// checks of spec section 3.3 step 3 in their order.
fn interval(
    params: &ExtrudeParams,
    context: &BuildContext<'_>,
    placement: &Placement,
    direction: [f64; 3],
) -> Result<(f64, f64), FeatureError> {
    match &params.extent {
        ExtrudeExtent::Distance { expr } => {
            let length = positive_length(context, "extent.expr", expr)?;
            Ok((0.0, length))
        }
        ExtrudeExtent::Symmetric { expr } => {
            let length = positive_length(context, "extent.expr", expr)?;
            Ok((-length / 2.0, length / 2.0))
        }
        ExtrudeExtent::TwoSides {
            expr,
            opposite_expr,
        } => {
            let along = context.eval_length("extent.expr", expr)?;
            let against = context.eval_length("extent.opposite_expr", opposite_expr)?;
            for (field, value) in [("extent.expr", along), ("extent.opposite_expr", against)] {
                if value < 0.0 {
                    return Err(out_of_range(context, field, value, Bound::AtLeast(0.0)));
                }
            }
            if along + against <= 0.0 {
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
                    body: params.body.clone(),
                    use_new: true,
                });
            };
            let bounds = shape
                .bounding_box()
                .map_err(|error| FeatureError::Kernel { error })?;
            let farthest = farthest_corner(bounds.min, bounds.max, placement.origin, direction);
            if farthest <= LINEAR_TOLERANCE {
                return Err(FeatureError::ThroughAllMisses {
                    body: params.body.clone(),
                });
            }
            Ok((0.0, farthest + THROUGH_ALL_MARGIN))
        }
    }
}

/// The length at `field`, which must be greater than 0.
fn positive_length(
    context: &BuildContext<'_>,
    field: &str,
    expr: &Expr,
) -> Result<f64, FeatureError> {
    let length = context.eval_length(field, expr)?;
    if length <= 0.0 {
        return Err(out_of_range(context, field, length, Bound::GreaterThan(0.0)));
    }
    Ok(length)
}

/// The `OutOfRange` error of a length at `field`.
fn out_of_range(context: &BuildContext<'_>, field: &str, value: f64, bound: Bound) -> FeatureError {
    FeatureError::OutOfRange {
        field: context.field(field),
        kind: FieldUnit::Length,
        value,
        bound,
        bound_name: None,
    }
}

/// The largest signed distance `(corner - origin) · direction` over the eight corners of the box
/// from `min` to `max`: how far a sweep from `origin` along the unit `direction` must go to pass
/// the whole box.
pub(super) fn farthest_corner(
    min: [f64; 3],
    max: [f64; 3],
    origin: [f64; 3],
    direction: [f64; 3],
) -> f64 {
    // Each coordinate contributes its larger product with the direction, independently.
    (0..3)
        .map(|axis| {
            let low = (min[axis] - origin[axis]) * direction[axis];
            let high = (max[axis] - origin[axis]) * direction[axis];
            low.max(high)
        })
        .sum()
}

/// The tool of a sweep of a profile union, its faces named `<feature>/lateral/<entity>` after the
/// sketch entity of their profile edge, `<feature>/start-cap` and `<feature>/end-cap` (naming.md
/// sections 3.6.2 and 3.6.3). Tag `i` of the sweep names `entities[i]`.
pub(super) fn sweep_tool(
    feature: &Id,
    entities: &[Id],
    sweep: SweepResult,
) -> Result<Tool, FeatureError> {
    let faces = sweep
        .faces
        .iter()
        .map(|face| {
            let name = match face {
                SweepFace::Lateral { tag } => {
                    let entity = usize::try_from(*tag)
                        .ok()
                        .and_then(|index| entities.get(index))
                        .ok_or_else(|| FeatureError::Internal {
                            message: format!("the sweep names profile tag {tag}, which has no sketch entity"),
                        })?;
                    ElementName::generated(
                        feature,
                        roles::LATERAL,
                        Some(Generator::Sketch(entity.as_str())),
                    )
                }
                SweepFace::StartCap => ElementName::generated(feature, roles::START_CAP, None),
                SweepFace::EndCap => ElementName::generated(feature, roles::END_CAP, None),
            };
            name.map(FaceSource::New)
                .map_err(|error| FeatureError::Internal {
                    message: format!("naming a swept face failed: {error}"),
                })
        })
        .collect::<Result<Vec<_>, FeatureError>>()?;
    let naming = name_shape(&sweep.shape, faces, &[])?;
    Ok(Tool {
        shape: sweep.shape,
        naming,
    })
}

/// The profile argument of a sweep's Python form: the sketch's variable, and the `regions=`
/// keyword when the stored regions are not all of the sketch's regions in solved order
/// (features.md section 3.11). None when a script name is missing.
pub(super) fn python_profile(
    profile: &ProfileRef,
    names: &dyn PythonNames,
) -> Option<(PythonValue, Option<(String, PythonValue)>)> {
    let sketch = PythonValue::Name(names.feature(&profile.sketch)?);
    let regions = (names.regions(&profile.sketch)? != profile.regions).then(|| {
        let listed = profile.regions.iter().map(PythonValue::region).collect();
        ("regions".to_string(), PythonValue::List(listed))
    });
    Some((sketch, regions))
}

/// The `op=` keyword of a solid feature's Python form, when `op` differs from the default op of
/// `body` (features.md section 3.11).
pub(super) fn python_op(
    op: BodyOp,
    body: &Id,
    names: &dyn PythonNames,
) -> Option<(String, PythonValue)> {
    (op != names.default_op(body)).then(|| {
        let text = match op {
            BodyOp::New => "new",
            BodyOp::Join => "join",
            BodyOp::Cut => "cut",
            BodyOp::Intersect => "intersect",
        };
        ("op".to_string(), PythonValue::Str(text.to_string()))
    })
}

/// The manifest hint of a solid feature's `op`, with the labels of every `BodyOp`.
pub(super) fn op_hint() -> FieldHint {
    FieldHint {
        options: options(&[
            ("new", "New body"),
            ("join", "Join"),
            ("cut", "Cut"),
            ("intersect", "Intersect"),
        ]),
        ..hint("op", "Operation", true)
    }
}

/// A manifest hint without unit, selection or options.
pub(super) fn hint(path: &str, label: &str, essential: bool) -> FieldHint {
    FieldHint {
        path: path.to_string(),
        label: label.to_string(),
        essential,
        unit: None,
        selection: None,
        options: Vec::new(),
    }
}

/// Option labels from `(value, label)` pairs, in display order.
pub(super) fn options(pairs: &[(&str, &str)]) -> Vec<OptionHint> {
    pairs
        .iter()
        .map(|(value, label)| OptionHint {
            value: (*value).to_string(),
            label: (*label).to_string(),
        })
        .collect()
}

/// The kernel vector of world components.
pub(super) fn vector([x, y, z]: [f64; 3]) -> Vector {
    Vector::new(x, y, z)
}

#[cfg(test)]
mod tests {
    use partsmith_model::Units;
    use partsmith_sketch::{OriginPlane, RegionRef};
    use serde_json::json;

    use super::*;
    use crate::Registry;

    /// Script names with `f-sk1` as `sketch_1`, the parameter `height`, every region of `f-sk1`
    /// being the rectangle of features.md section 6.1, and default op new.
    struct Names;

    impl PythonNames for Names {
        fn units(&self) -> Units {
            Units::Mm
        }

        fn body(&self, _body: &Id) -> Option<String> {
            Some("block".to_string())
        }

        fn feature(&self, feature: &Id) -> Option<String> {
            (feature.as_str() == "f-sk1").then(|| "sketch_1".to_string())
        }

        fn entity(&self, _sketch: &Id, _entity: &Id) -> Option<String> {
            None
        }

        fn parameter(&self, name: &str) -> Option<String> {
            (name == "height").then(|| "height".to_string())
        }

        fn regions(&self, sketch: &Id) -> Option<Vec<RegionRef>> {
            (sketch.as_str() == "f-sk1").then(|| vec![rectangle_region()])
        }

        fn default_op(&self, _body: &Id) -> BodyOp {
            BodyOp::New
        }
    }

    fn rectangle_region() -> RegionRef {
        serde_json::from_value(json!({"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}))
            .expect("the region ref is valid")
    }

    /// The extrude of `P-RECT` with this extent and op, plus `extra` members.
    fn params(extent: serde_json::Value, op: &str, extra: serde_json::Value) -> ExtrudeParams {
        let mut record = json!({
            "body": "b-1",
            "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]},
            "extent": extent,
            "op": op
        });
        if let (Some(target), Some(source)) = (record.as_object_mut(), extra.as_object()) {
            target.extend(source.clone());
        }
        serde_json::from_value(record).expect("the extrude params are valid")
    }

    fn python(params: &ExtrudeParams) -> Option<String> {
        Extrude::python(params, &Names).map(|call| call.render(0))
    }

    #[test]
    fn registers_through_the_public_registry() {
        let mut registry = Registry::new();

        assert_eq!(registry.register::<Extrude>(), Ok(()));
    }

    #[test]
    fn reverse_is_omitted_when_false() {
        let extrude = params(
            json!({"kind": "distance", "expr": "30 mm"}),
            "new",
            json!({"reverse": false}),
        );

        let text = serde_json::to_value(&extrude).expect("serializes");

        assert!(text.get("reverse").is_none(), "{text}");
    }

    #[test]
    fn extents_reject_unknown_keys() {
        let extent: Result<ExtrudeExtent, _> =
            serde_json::from_value(json!({"kind": "distance", "expr": "1", "taper": "2 deg"}));

        assert!(extent.is_err());
    }

    #[test]
    fn dependencies_declare_both_sides_of_a_two_sided_extent() {
        let dependencies = Extrude::dependencies(&params(
            json!({"kind": "two_sides", "expr": "20 mm", "opposite_expr": "10 mm"}),
            "new",
            json!({}),
        ));

        let fields: Vec<&str> = dependencies
            .expressions
            .iter()
            .map(|usage| usage.field.as_str())
            .collect();
        assert_eq!(fields, ["extent.expr", "extent.opposite_expr"]);
        assert_eq!(dependencies.features[0].field, "profile.sketch");
        assert_eq!(dependencies.bodies_written[0].field, "body");
    }

    #[test]
    fn dependencies_of_through_all_have_no_expression() {
        let dependencies =
            Extrude::dependencies(&params(json!({"kind": "through_all"}), "cut", json!({})));

        assert!(dependencies.expressions.is_empty());
    }

    #[test]
    fn validate_rejects_an_empty_profile() {
        let mut extrude = params(json!({"kind": "distance", "expr": "1"}), "new", json!({}));
        extrude.profile.regions.clear();

        let error = Extrude::validate(&extrude).expect_err("a profile needs a region");

        assert_eq!(error.code(), "empty_selection");
        assert_eq!(error.to_string(), "Select at least one region for Profile.");
    }

    #[test]
    fn direction_follows_the_normal_unless_reversed() {
        let placement = OriginPlane::Xy.placement();
        let forward = params(json!({"kind": "distance", "expr": "1"}), "new", json!({}));
        let reversed = params(
            json!({"kind": "distance", "expr": "1"}),
            "new",
            json!({"reverse": true}),
        );

        assert_eq!(direction(&placement, &forward), [0.0, 0.0, 1.0]);
        assert_eq!(direction(&placement, &reversed), [0.0, 0.0, -1.0]);
    }

    #[test]
    fn symmetric_direction_ignores_reverse() {
        let placement = OriginPlane::Xz.placement();
        let symmetric = params(
            json!({"kind": "symmetric", "expr": "1"}),
            "new",
            json!({"reverse": true}),
        );

        assert_eq!(direction(&placement, &symmetric), placement.normal);
    }

    #[test]
    fn farthest_corner_takes_the_far_side_of_each_axis() {
        let farthest = farthest_corner(
            [-20.0, -20.0, -10.0],
            [20.0, 20.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, -1.0],
        );

        assert_eq!(farthest, 10.0);
    }

    #[test]
    fn farthest_corner_is_not_positive_when_the_box_is_behind() {
        let farthest = farthest_corner(
            [-20.0, -20.0, -10.0],
            [20.0, 20.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
        );

        assert_eq!(farthest, 0.0);
    }

    #[test]
    fn python_writes_a_distance_with_the_sketch_variable() {
        let extrude = params(json!({"kind": "distance", "expr": "30 mm"}), "new", json!({}));

        assert_eq!(
            python(&extrude),
            Some("extrude(sketch_1, 30*mm)".to_string())
        );
    }

    #[test]
    fn python_writes_symmetric_and_reverse_flags() {
        let extrude = params(
            json!({"kind": "symmetric", "expr": "height"}),
            "new",
            json!({"reverse": true}),
        );

        assert_eq!(
            python(&extrude),
            Some("extrude(sketch_1, height, symmetric=True, reverse=True)".to_string())
        );
    }

    #[test]
    fn python_writes_through_all_without_a_distance_and_a_non_default_op() {
        let extrude = params(json!({"kind": "through_all"}), "cut", json!({}));

        assert_eq!(
            python(&extrude),
            Some("extrude(sketch_1, through_all=True, op=\"cut\")".to_string())
        );
    }

    #[test]
    fn python_has_no_form_for_two_sides() {
        let extrude = params(
            json!({"kind": "two_sides", "expr": "20 mm", "opposite_expr": "10 mm"}),
            "new",
            json!({}),
        );

        assert_eq!(python(&extrude), None);
    }

    #[test]
    fn python_lists_regions_that_differ_from_the_sketch() {
        let extrude = params(
            json!({"kind": "distance", "expr": "5"}),
            "new",
            json!({"profile": {"sketch": "f-sk1", "regions": [{"key": "e-5", "fp": "c=0,0|a=1"}]}}),
        );

        assert_eq!(
            python(&extrude),
            Some("extrude(sketch_1, 5, regions=[Region(\"e-5\", \"c=0,0|a=1\")])".to_string())
        );
    }

    #[test]
    fn python_needs_the_sketch_variable() {
        let extrude = params(
            json!({"kind": "distance", "expr": "5"}),
            "new",
            json!({"profile": {"sketch": "f-sk9", "regions": [{"key": "e-5", "fp": "c=0,0|a=1"}]}}),
        );

        assert_eq!(python(&extrude), None);
    }
}
