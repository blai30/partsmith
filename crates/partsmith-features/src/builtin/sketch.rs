//! The `sketch` feature: places a sketch, evaluates its driving dimensions, solves it and outputs
//! its geometry and closed regions (features/sketch.md).

use partsmith_model::{Category, FeatureManifest, FieldUnit, Id, SelectionKind};
use partsmith_naming::{ElementKind, Reference};
use partsmith_occt_sys::GeometryKind;
use partsmith_sketch::{
    DimensionValues, NativeSolver, Placement, SketchParams, SketchPlane, SolveIssue, Solver,
    ValueKind, build_profile,
};

use super::common::{essential, hint, manifest, selection_hint};
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::{FallbackReason, FeatureError, FeatureWarning};
use crate::feature::Feature;
use crate::output::{DatumResult, FeatureOutput, OutputData, SketchResult};

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
        let mut warnings = Vec::new();
        let placement = match &params.plane {
            SketchPlane::Origin { plane } => plane.placement(),
            SketchPlane::Datum { feature } => match context.datum("plane", feature)? {
                DatumResult::Plane(placement) => *placement,
            },
            SketchPlane::Face {
                body,
                face,
                fallback,
            } => {
                let (placement, fallback_reason) = face_placement(context, body, face, fallback)?;
                if let Some(reason) = fallback_reason {
                    warnings.push(FeatureWarning::PlaneFallback {
                        field: context.field(FACE_FIELD),
                        body: body.clone(),
                        reference: face.clone(),
                        reason,
                    });
                }
                placement
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
        Ok(FeatureOutput {
            data: OutputData::Sketch(SketchResult {
                placement,
                solved,
                profile,
            }),
            warnings,
        })
    }
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

/// The placement of a sketch on a face (features/sketch.md section 3.3): the face's plane with the
/// origin and x-axis kept as close to the fallback as possible, or the fallback itself with the
/// reason when the body is empty or the face is missing, ambiguous or not flat.
fn face_placement(
    context: &BuildContext<'_>,
    body: &Id,
    face: &Reference,
    fallback: &Placement,
) -> Result<(Placement, Option<FallbackReason>), FeatureError> {
    let state = context.body(body)?;
    let Some(shape) = state.shape() else {
        return Ok((*fallback, Some(FallbackReason::Missing)));
    };
    let found = match context.resolve(FACE_FIELD, body, face, ElementKind::Face) {
        Ok(found) => found,
        Err(FeatureError::ReferenceNotFound { candidates, .. }) => {
            let reason = if candidates.is_empty() {
                FallbackReason::Missing
            } else {
                FallbackReason::Ambiguous
            };
            return Ok((*fallback, Some(reason)));
        }
        Err(error) => return Err(error),
    };
    let first = found
        .first()
        .ok_or_else(|| internal("a resolved face reference has no face"))?;
    let geometry = shape
        .element(first.kind, first.index)
        .map_err(|error| FeatureError::Kernel { error })?;
    if geometry.kind != GeometryKind::Plane {
        return Ok((*fallback, Some(FallbackReason::NotPlanar)));
    }
    let normal = geometry
        .direction
        .ok_or_else(|| internal("the kernel gave a plane without a normal"))?;
    let centroid = geometry.centroid;
    let placement = Placement::on_plane(
        [centroid.x, centroid.y, centroid.z],
        [normal.x, normal.y, normal.z],
        fallback,
    )
    .map_err(|error| internal(format!("placing the sketch failed: {error}")))?;
    Ok((placement, None))
}

/// An internal error for a failure that valid kernel output cannot cause.
fn internal(message: impl Into<String>) -> FeatureError {
    FeatureError::Internal {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::{id, run, run_document};

    /// The rectangle (0, 0)-(80, 50) of features/sketch.md on `plane`.
    fn rectangle(plane: serde_json::Value) -> serde_json::Value {
        json!({"id": "f-sk1", "type": "sketch", "plane": plane, "entities": [
            {"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]},
            {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]},
            {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]},
            {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}
        ]})
    }

    /// A line with a dimension `expr` and `extra` constraints after it.
    fn line_dimension(expr: &str, extra: &[serde_json::Value]) -> serde_json::Value {
        let mut constraints = vec![
            json!({"id": "c-1", "type": "fix", "target": "e-1.start"}),
            json!({"id": "c-2", "type": "horizontal", "line": "e-1"}),
            json!({"id": "c-3", "type": "distance", "line": "e-1", "expr": expr}),
        ];
        constraints.extend_from_slice(extra);
        json!({"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"},
            "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}],
            "constraints": constraints})
    }

    /// The face plane of features/sketch.md test 4 with fallback origin (3, 4, 0).
    fn face_plane(face: &str, fingerprint: &str) -> serde_json::Value {
        json!({"kind": "face", "body": "b-1", "face": {"q": face, "fp": fingerprint},
            "fallback": {"origin": [3, 4, 0], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}})
    }

    fn cube() -> serde_json::Value {
        json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"})
    }

    fn end_point(run: &crate::test_support::DocumentRun) -> [f64; 2] {
        run.sketch("f-sk1")
            .point(&"e-1.end".parse().expect("valid point"))
            .expect("e-1 has an end")
    }

    #[test]
    fn a_rectangle_on_xy_has_one_region() {
        let run = run(&[], &[rectangle(json!({"kind": "origin", "plane": "XY"}))]);

        run.assert_ok("f-sk1");
        let sketch = run.sketch("f-sk1");
        assert_eq!(sketch.placement.normal, [0.0, 0.0, 1.0]);
        assert_eq!(sketch.profile.regions.len(), 1);
        assert_eq!(sketch.profile.regions[0].key.to_string(), "e-1,e-2,e-3,e-4");
        assert!((sketch.profile.regions[0].area - 4000.0).abs() < 1e-9);
    }

    #[test]
    fn a_sketch_on_xz_maps_sketch_up_to_z() {
        let run = run(&[], &[rectangle(json!({"kind": "origin", "plane": "XZ"}))]);

        assert_eq!(
            run.sketch("f-sk1").to_world([10.0, 20.0]),
            [10.0, 0.0, 20.0]
        );
    }

    #[test]
    fn a_sketch_on_a_face_projects_its_fallback_origin() {
        let run = run(
            &["b-1"],
            &[
                cube(),
                rectangle(face_plane("f-box1/top", "plane|n=0,0,1|c=0,0,10|a=100")),
            ],
        );

        run.assert_ok("f-sk1");
        let placement = run.sketch("f-sk1").placement;
        assert_eq!(placement.origin, [3.0, 4.0, 10.0]);
        assert_eq!(placement.x_axis, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn a_lost_face_keeps_the_fallback() {
        let run = run(
            &["b-1"],
            &[
                cube(),
                rectangle(face_plane(
                    "f-box1/nothing",
                    "plane|n=0,0,1|c=500,500,500|a=1",
                )),
            ],
        );

        assert_eq!(run.warning_codes("f-sk1"), ["plane_fallback"]);
        assert_eq!(
            run.messages("f-sk1"),
            [
                "The face this sketch is on can no longer be found, so the sketch stays at its last position. Re-attach it in the repair panel."
            ]
        );
        assert_eq!(run.sketch("f-sk1").placement.origin, [3.0, 4.0, 0.0]);
        let warning = &run.output("f-sk1").warnings[0];
        assert_eq!(
            warning.field().map(|field| field.pointer()),
            Some("/plane/face".to_string())
        );
    }

    #[test]
    fn a_round_face_is_not_flat() {
        let run = run(
            &["b-1"],
            &[
                json!({"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "5", "height": "10", "op": "new"}),
                rectangle(face_plane(
                    "f-cyl1/side",
                    "cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5",
                )),
            ],
        );

        assert!(matches!(
            run.output("f-sk1").warnings[0],
            FeatureWarning::PlaneFallback {
                reason: FallbackReason::NotPlanar,
                ..
            }
        ));
    }

    #[test]
    fn an_empty_body_under_a_face_plane_keeps_the_fallback() {
        let run = run(
            &["b-1"],
            &[rectangle(face_plane(
                "f-box1/top",
                "plane|n=0,0,1|c=0,0,10|a=100",
            ))],
        );

        assert_eq!(run.warning_codes("f-sk1"), ["plane_fallback"]);
        assert_eq!(run.sketch("f-sk1").placement.origin, [3.0, 4.0, 0.0]);
    }

    #[test]
    fn driving_dimensions_take_parameters_and_document_units() {
        let parameters = [json!({"id": "p-w", "name": "width", "expr": "100 mm"})];
        let with_parameter = run_document(
            "mm",
            &parameters,
            &[],
            &[line_dimension("width", &[])],
            &Default::default(),
        );
        let in_inches = run_document(
            "in",
            &[],
            &[],
            &[line_dimension("2", &[])],
            &Default::default(),
        );

        let close = |actual: [f64; 2], expected: [f64; 2]| {
            (actual[0] - expected[0]).abs() < 1e-9 && (actual[1] - expected[1]).abs() < 1e-9
        };
        assert!(close(end_point(&with_parameter), [100.0, 0.0]));
        assert!(close(end_point(&in_inches), [50.8, 0.0]));
    }

    #[test]
    fn conflicting_dimensions_fail_the_sketch_and_its_users() {
        let parameters = [json!({"id": "p-w", "name": "width", "expr": "100 mm"})];
        let run = run_document(
            "mm",
            &parameters,
            &["b-1"],
            &[
                line_dimension(
                    "width",
                    &[json!({"id": "c-4", "type": "distance", "line": "e-1", "expr": "50 mm"})],
                ),
                json!({"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk1", "points": ["e-1.start"], "diameter": "3", "extent": {"kind": "through_all"}}),
            ],
            &Default::default(),
        );

        let FeatureError::SketchConflict { constraints } = run.error("f-sk1") else {
            panic!("expected a conflict, got {:?}", run.error("f-sk1"));
        };
        assert!(constraints.contains(&id("c-4")));
        assert_eq!(run.error_code("f-hole1"), "upstream_failed");
    }

    #[test]
    fn a_redundant_dimension_warns() {
        let parameters = [json!({"id": "p-w", "name": "width", "expr": "100 mm"})];
        let run = run_document(
            "mm",
            &parameters,
            &[],
            &[line_dimension(
                "width",
                &[json!({"id": "c-4", "type": "distance", "line": "e-1", "expr": "100 mm"})],
            )],
            &Default::default(),
        );

        assert_eq!(run.warning_codes("f-sk1"), ["redundant_constraints"]);
    }

    #[test]
    fn a_value_the_solver_rejects_is_invalid() {
        let run = run(&[], &[line_dimension("-5", &[])]);

        assert_eq!(run.error_code("f-sk1"), "sketch_invalid");
    }

    #[test]
    fn a_dimension_with_an_unknown_name_is_labeled_dimension() {
        let run = run(&[], &[line_dimension("wdth", &[])]);

        assert_eq!(
            run.messages("f-sk1"),
            ["Dimension: Unknown name 'wdth'. Check the spelling, or add a parameter named wdth."]
        );
        assert_eq!(
            run.error("f-sk1").field().map(|field| field.path.as_str()),
            Some("constraints[2].expr")
        );
    }

    #[test]
    fn a_sketch_is_not_a_datum_plane() {
        let run = run(
            &[],
            &[
                rectangle(json!({"kind": "origin", "plane": "XY"})),
                json!({"id": "f-sk2", "type": "sketch", "plane": {"kind": "datum", "feature": "f-sk1"}}),
            ],
        );

        assert_eq!(
            run.messages("f-sk2"),
            ["'Sketch 1' is not a datum plane. Pick a datum plane for Plane."]
        );
    }

    #[test]
    fn reference_dimensions_are_declared_but_not_evaluated() {
        let parameters = [json!({"id": "p-w", "name": "width", "expr": "100 mm"})];
        let sketch = line_dimension(
            "width",
            &[
                json!({"id": "c-4", "type": "distance", "line": "e-1", "expr": "wdth", "driving": false}),
            ],
        );
        let run = run_document("mm", &parameters, &[], &[sketch], &Default::default());

        run.assert_ok("f-sk1");
        let dependencies = run
            .registry
            .dependencies(&run.document.features[0])
            .expect("valid params");
        let fields: Vec<&str> = dependencies
            .expressions
            .iter()
            .map(|expression| expression.field.as_str())
            .collect();
        assert_eq!(fields, ["constraints[2].expr", "constraints[3].expr"]);
        assert_eq!(
            dependencies.declared_ids,
            [id("e-1"), id("c-1"), id("c-2"), id("c-3"), id("c-4")]
        );
    }

    #[test]
    fn an_unknown_entity_fails_validation() {
        let run = run(
            &[],
            &[
                json!({"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"},
                "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [10, 0]}],
                "constraints": [{"id": "c-1", "type": "horizontal", "line": "e-9"}]}),
            ],
        );

        assert!(matches!(
            run.error("f-sk1"),
            FeatureError::SketchInvalid {
                error: partsmith_sketch::SketchError::UnknownEntity { .. }
            }
        ));
    }
}
