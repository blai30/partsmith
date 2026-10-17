//! The `sketch` feature: places a sketch, evaluates its driving dimensions, solves it and outputs
//! its geometry and closed regions (features/sketch.md).

use partsmith_model::{Category, FeatureManifest, FieldUnit, Id, SelectionKind};
use partsmith_naming::{Motion, Reference};
use partsmith_sketch::{
    DimensionValues, HintFrame, NativeSolver, Placement, SketchParams, SketchPlane, SolveIssue,
    Solver, ValueKind, build_profile,
};

use super::common::{essential, hint, manifest, selection_hint};
use crate::context::{BuildContext, WithWarnings};
use crate::deps::Dependencies;
use crate::error::{FallbackCause, FeatureError, FeatureWarning, Placed};
use crate::feature::Feature;
use crate::output::{DatumResult, FeatureOutput, OutputData, SketchResult};
use crate::params::PlacementExt;

/// Field path of the face of a sketch on a face.
const FACE_FIELD: &str = "plane.face";

/// A 2D sketch placed on a plane.
#[derive(Clone, Copy, Debug)]
pub struct SketchFeature;

/// The unit of a dimension's expression.
fn field_unit(kind: ValueKind) -> FieldUnit {
    match kind {
        ValueKind::Length => FieldUnit::Length,
        ValueKind::Angle => FieldUnit::Angle,
    }
}

/// The field path of the expression of constraint `index`.
fn dimension_field(index: usize) -> String {
    format!("constraints[{index}].expr")
}

impl Feature for SketchFeature {
    const TYPE: &'static str = "sketch";
    type Params = SketchParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Sketch",
            Category::Sketch,
            "sketch",
            vec![
                essential(selection_hint("plane", "Plane", SelectionKind::Plane)),
                hint("entities", "Entities"),
                hint("constraints", "Constraints"),
                hint("constraints[*].expr", "Dimension"),
            ],
        )
    }

    fn validate(params: &SketchParams) -> Result<(), FeatureError> {
        params
            .validate()
            .map_err(|error| FeatureError::SketchInvalid { error })
    }

    fn dependencies(params: &SketchParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        match &params.plane {
            SketchPlane::Origin { .. } => {}
            SketchPlane::Datum { feature } => dependencies.add_feature(feature, "plane.feature"),
            SketchPlane::Face { body, .. } => dependencies.add_body_read(body, "plane.body"),
        }
        for (index, constraint) in params.constraints.iter().enumerate() {
            if let (Some(dimension), Some(kind)) = (constraint.dimension(), constraint.value_kind())
            {
                dependencies.add_expression(
                    &dimension_field(index),
                    &dimension.expr,
                    field_unit(kind),
                );
            }
        }
        dependencies.declared_ids = params
            .entities
            .iter()
            .map(|entity| entity.id.clone())
            .chain(
                params
                    .constraints
                    .iter()
                    .map(|constraint| constraint.id.clone()),
            )
            .collect();
        dependencies
    }

    fn build(
        params: &SketchParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let (placement, motion, mut warnings) = match &params.plane {
            SketchPlane::Origin { plane } => (plane.placement(), Motion::IDENTITY, Vec::new()),
            SketchPlane::Datum { feature } => match context.datum("plane", feature)? {
                DatumResult::Plane { placement, motion } => (*placement, *motion, Vec::new()),
            },
            SketchPlane::Face {
                body,
                face,
                fallback,
                fallback_frame,
            } => {
                // A fallback in world coordinates does not follow the face's motion (naming.md section 3.6).
                let motion = match fallback_frame {
                    HintFrame::Body => context.body(body)?.motion(&face.query),
                    HintFrame::World => Motion::IDENTITY,
                };
                let placed =
                    face_sketch_placement(context, body, face, fallback, *fallback_frame, &motion)?;
                (placed.value, motion, placed.warnings)
            }
        };
        let values = driving_values(params, context)?;
        let solved = NativeSolver
            .solve(params, &values)
            .map_err(|error| FeatureError::SketchInvalid { error })?;
        if !solved.conflicting.is_empty() {
            return Err(FeatureError::SketchConflict {
                constraints: solved.conflicting.clone(),
            });
        }
        if solved.failed {
            return Err(FeatureError::SketchNotSolved);
        }
        if !solved.redundant.is_empty() {
            let issues = solved
                .issues(params)
                .into_iter()
                .filter(|issue| matches!(issue, SolveIssue::Redundant { .. }))
                .collect();
            warnings.push(FeatureWarning::RedundantConstraints { issues });
        }
        let profile = build_profile(&solved.entities);
        Ok(FeatureOutput::new(
            OutputData::Sketch(SketchResult {
                placement,
                motion,
                solved,
                profile,
            }),
            warnings,
        ))
    }
}

/// The placement of a sketch on `face` of `body` (features/sketch.md section 3.3 step 1), with its stored
/// `fallback` in the coordinates `frame` names and the sketch's `motion`, the first that applies: on the face, with
/// its reference's warnings and notices and `SketchTurnedOver` when the face now faces the other way; for a missing
/// face, where it was just before the earlier writer that removed it, moved with the body since; else the saved
/// `fallback`, moved by `motion` for `HintFrame::Body`. The last two warn `PlaneFallback` with where the sketch was
/// placed.
fn face_sketch_placement(
    context: &BuildContext<'_>,
    body: &Id,
    face: &Reference,
    fallback: &Placement,
    frame: HintFrame,
    motion: &Motion,
) -> Result<WithWarnings<Placement>, FeatureError> {
    let face_fallback = match context.face_placement(FACE_FIELD, body, face, fallback, frame)? {
        Ok(placed) => {
            let mut warnings = placed.warnings;
            if placed.turned_over {
                warnings.push(FeatureWarning::SketchTurnedOver {
                    field: context.field(FACE_FIELD),
                    body: body.clone(),
                    reference: face.clone(),
                });
            }
            return Ok(WithWarnings {
                value: placed.placement,
                warnings,
            });
        }
        Err(face_fallback) => face_fallback,
    };
    // An empty body has no loss and no earlier state with the face to look for.
    let missing = face_fallback.cause == FallbackCause::Missing && face_fallback.loss.is_some();
    let before_removal = if missing {
        context.face_placement_before_removal(body, face, fallback, frame)?
    } else {
        None
    };
    let (value, placed) = match before_removal {
        Some((placement, by)) => (placement, Placed::BeforeRemoval { by }),
        None => {
            let saved = match frame {
                HintFrame::Body => fallback.moved(motion),
                HintFrame::World => *fallback,
            };
            (saved, Placed::Saved)
        }
    };
    Ok(WithWarnings {
        value,
        warnings: vec![FeatureWarning::PlaneFallback {
            field: context.field(FACE_FIELD),
            body: body.clone(),
            reference: face.clone(),
            loss: face_fallback.loss,
            cause: face_fallback.cause,
            placed,
        }],
    })
}

/// The values of the driving dimensions, keyed by constraint id. Reference dimensions are not
/// evaluated, so a broken one never fails the sketch.
fn driving_values(
    params: &SketchParams,
    context: &BuildContext<'_>,
) -> Result<DimensionValues, FeatureError> {
    let mut values = DimensionValues::new();
    for (index, constraint) in params.constraints.iter().enumerate() {
        let (Some(dimension), Some(kind)) = (constraint.dimension(), constraint.value_kind())
        else {
            continue;
        };
        if !dimension.driving {
            continue;
        }
        let field = dimension_field(index);
        let value = match kind {
            ValueKind::Length => context.eval_length(&field, &dimension.expr)?,
            ValueKind::Angle => context.eval_angle(&field, &dimension.expr)?,
        };
        values.insert(constraint.id.clone(), value);
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::id;

    /// Sketch params of one line on `plane` with these constraints.
    fn line_sketch(plane: serde_json::Value, constraints: serde_json::Value) -> SketchParams {
        serde_json::from_value(json!({
            "plane": plane,
            "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}],
            "constraints": constraints
        }))
        .expect("valid sketch params")
    }

    fn on_xy() -> serde_json::Value {
        json!({"kind": "origin", "plane": "XY"})
    }

    #[test]
    fn dimension_fields_are_constraint_expressions() {
        assert_eq!(dimension_field(3), "constraints[3].expr");
        assert_eq!(field_unit(ValueKind::Length), FieldUnit::Length);
        assert_eq!(field_unit(ValueKind::Angle), FieldUnit::Angle);
    }

    #[test]
    fn dependencies_declare_every_dimension_and_every_id() {
        let params = line_sketch(
            on_xy(),
            json!([
                {"id": "c-1", "type": "horizontal", "line": "e-1"},
                {"id": "c-2", "type": "distance", "line": "e-1", "expr": "width"},
                {"id": "c-3", "type": "distance", "line": "e-1", "expr": "wdth", "driving": false}
            ]),
        );

        let dependencies = SketchFeature::dependencies(&params);

        let fields: Vec<&str> = dependencies
            .expressions
            .iter()
            .map(|expression| expression.field.as_str())
            .collect();
        assert_eq!(fields, ["constraints[1].expr", "constraints[2].expr"]);
        assert_eq!(
            dependencies.declared_ids,
            [id("e-1"), id("c-1"), id("c-2"), id("c-3")]
        );
        assert!(dependencies.features.is_empty());
        assert!(dependencies.bodies_read.is_empty());
    }

    #[test]
    fn a_sketch_on_a_datum_reads_the_datum() {
        let params = line_sketch(json!({"kind": "datum", "feature": "f-dp1"}), json!([]));

        let dependencies = SketchFeature::dependencies(&params);

        assert_eq!(dependencies.features[0].id, id("f-dp1"));
        assert_eq!(dependencies.features[0].field, "plane.feature");
    }

    #[test]
    fn a_sketch_on_a_face_reads_the_body() {
        let params = line_sketch(
            json!({"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1"},
                "fallback": {"origin": [0, 0, 0], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}}),
            json!([]),
        );

        let dependencies = SketchFeature::dependencies(&params);

        assert_eq!(dependencies.bodies_read[0].id, id("b-1"));
        assert_eq!(dependencies.bodies_read[0].field, "plane.body");
        assert!(dependencies.bodies_written.is_empty());
    }
}
