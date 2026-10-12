//! The `sketch` feature: places a sketch, evaluates its driving dimensions, solves it and outputs
//! its geometry and regions (docs/spec/features/sketch.md).
//!
//! The params, the solver and the profile belong to `partsmith_sketch`; this module is the feature
//! wrapper. A sketch has no naming map (naming.md section 3.6.1), and its Python form is a block
//! the engine's script generator writes, so the type has no Python hook.

use partsmith_model::{Category, FeatureManifest, FieldHint, FieldUnit, Id, SelectionKind};
use partsmith_naming::{ElementKind, Reference};
use partsmith_occt_sys::GeometryKind;
use partsmith_sketch::{
    DimensionValues, NativeSolver, Placement, SketchParams, SketchPlane, SolveIssue, Solver,
    ValueKind, build_profile,
};

use crate::{
    BuildContext, DatumResult, Dependencies, FallbackReason, Feature, FeatureError, FeatureOutput,
    FeatureWarning, OutputData, SketchResult,
};

/// The field of a face plane's reference, which `PlaneFallback` warnings point at.
const FACE_FIELD: &str = "plane.face";

/// A 2D sketch placed on a plane.
pub struct SketchFeature;

impl Feature for SketchFeature {
    const TYPE: &'static str = "sketch";
    type Params = SketchParams;

    fn manifest() -> FeatureManifest {
        FeatureManifest {
            label: "Sketch".to_string(),
            category: Category::Sketch,
            icon: "sketch".to_string(),
            version: 1,
            fields: vec![
                FieldHint {
                    selection: Some(SelectionKind::Plane),
                    ..hint("plane", "Plane", true)
                },
                hint("entities", "Entities", false),
                hint("constraints", "Constraints", false),
                hint("constraints[*].expr", "Dimension", false),
            ],
        }
    }

    /// The datum or face body of the plane, one expression per dimensional constraint (driving or
    /// reference) and every entity and constraint id (spec section 3.2).
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
                dependencies.add_expression(&dimension_field(index), &dimension.expr, unit(kind));
            }
        }
        dependencies.declared_ids = params
            .entities
            .iter()
            .map(|entity| entity.id.clone())
            .chain(params.constraints.iter().map(|constraint| constraint.id.clone()))
            .collect();
        dependencies
    }

    /// Places the sketch, evaluates its driving dimensions, solves it and builds its profile
    /// (spec section 3.3).
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
            } => match face_placement(context, body, face, fallback)? {
                Ok(placement) => placement,
                Err(reason) => {
                    warnings.push(FeatureWarning::PlaneFallback {
                        field: context.field(FACE_FIELD),
                        body: body.clone(),
                        reference: face.clone(),
                        reason,
                    });
                    *fallback
                }
            },
        };
        let values = driving_values(params, context)?;
        let solved = NativeSolver
            .solve(params, &values)
            .map_err(|error| FeatureError::SketchInvalid { error })?;
        if !solved.conflicting.is_empty() {
            return Err(FeatureError::SketchConflict {
                constraints: solved.conflicting,
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

    /// The sketch crate's validation; its first error is `SketchInvalid` (spec section 3.1).
    fn validate(params: &SketchParams) -> Result<(), FeatureError> {
        params
            .validate()
            .map_err(|error| FeatureError::SketchInvalid { error })
    }
}

/// The placement on the resolved face, or why the sketch keeps its fallback placement (spec
/// section 3.3 step 1): an empty body or a lost face is `Missing`, a face that matches several
/// faces is `Ambiguous`, and a face that is not flat is `NotPlanar`.
fn face_placement(
    context: &BuildContext<'_>,
    body: &Id,
    face: &Reference,
    fallback: &Placement,
) -> Result<Result<Placement, FallbackReason>, FeatureError> {
    let found = match context.resolve(FACE_FIELD, body, face, ElementKind::Face) {
        Ok(found) => found,
        Err(FeatureError::BodyEmpty { .. }) => return Ok(Err(FallbackReason::Missing)),
        Err(FeatureError::ReferenceNotFound { candidates, .. }) => {
            let reason = if candidates.is_empty() {
                FallbackReason::Missing
            } else {
                FallbackReason::Ambiguous
            };
            return Ok(Err(reason));
        }
        Err(error) => return Err(error),
    };
    let first = found.first().ok_or_else(|| FeatureError::Internal {
        message: "a resolved face reference has no element".to_string(),
    })?;
    let shape = context
        .body(body)?
        .shape()
        .ok_or_else(|| FeatureError::Internal {
            message: "a body with a resolved face has no shape".to_string(),
        })?;
    let geometry = shape
        .element(first.kind, first.index)
        .map_err(|error| FeatureError::Kernel { error })?;
    if geometry.kind != GeometryKind::Plane {
        return Ok(Err(FallbackReason::NotPlanar));
    }
    let normal = geometry.direction.ok_or_else(|| FeatureError::Internal {
        message: "the kernel gave a plane without a normal".to_string(),
    })?;
    let centroid = geometry.centroid;
    Placement::on_plane(
        [centroid.x, centroid.y, centroid.z],
        [normal.x, normal.y, normal.z],
        fallback,
    )
    .map(Ok)
    .map_err(|error| FeatureError::Internal {
        message: format!("placing the sketch on its face failed: {error}"),
    })
}

/// The evaluated driving dimensions, keyed by constraint id. Reference dimensions are not
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

/// The field path of the expression of the constraint at `index`.
fn dimension_field(index: usize) -> String {
    format!("constraints[{index}].expr")
}

/// The expression unit of a dimension's value kind.
fn unit(kind: ValueKind) -> FieldUnit {
    match kind {
        ValueKind::Length => FieldUnit::Length,
        ValueKind::Angle => FieldUnit::Angle,
    }
}

/// A manifest hint without unit, selection or options.
fn hint(path: &str, label: &str, essential: bool) -> FieldHint {
    FieldHint {
        path: path.to_string(),
        label: label.to_string(),
        essential,
        unit: None,
        selection: None,
        options: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use partsmith_model::{EmptyContext, ParameterValues, Scope, Units};
    use partsmith_sketch::{OriginPlane, SketchError};
    use serde_json::json;

    use super::*;
    use crate::context::BodyState;
    use crate::test_support::{Fixture, box_body};
    use crate::{Registry, Values};

    fn params(value: serde_json::Value) -> SketchParams {
        serde_json::from_value(value).expect("the sketch params are valid")
    }

    /// A line `e-1` from (0, 0) to (80, 0), fixed at its start, horizontal, with a driving
    /// distance `expr` and `extra` constraints after it.
    fn line_dimension(expr: &str, extra: &[serde_json::Value]) -> SketchParams {
        let mut constraints = vec![
            json!({"id": "c-1", "type": "fix", "target": "e-1.start"}),
            json!({"id": "c-2", "type": "horizontal", "line": "e-1"}),
            json!({"id": "c-3", "type": "distance", "line": "e-1", "expr": expr}),
        ];
        constraints.extend(extra.iter().cloned());
        params(json!({
            "plane": {"kind": "origin", "plane": "XY"},
            "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}],
            "constraints": constraints
        }))
    }

    /// The rectangle `RECT(0, 0, 80, 50)` on `plane`.
    fn rectangle(plane: serde_json::Value) -> SketchParams {
        params(json!({
            "plane": plane,
            "entities": [
                {"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]},
                {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]},
                {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]},
                {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}
            ]
        }))
    }

    /// Builds `params` in a millimeter document, with these bodies declared.
    fn built(
        params: &SketchParams,
        bodies: Vec<(&str, BodyState)>,
    ) -> Result<FeatureOutput, FeatureError> {
        let mut fixture = Fixture::new("f-sk1");
        fixture.manifest = SketchFeature::manifest();
        for (body, state) in bodies {
            fixture = fixture.with_body(body, state);
        }
        let parameters = ParameterValues::default();
        let scope = Scope {
            units: Units::Mm,
            parameters: &parameters,
            context: &EmptyContext,
        };
        fixture.values = Values::evaluate(
            &SketchFeature::dependencies(params).expressions,
            &scope,
            &fixture.manifest,
        );
        SketchFeature::build(params, &fixture.context())
    }

    fn sketch(output: &FeatureOutput) -> &SketchResult {
        match &output.data {
            OutputData::Sketch(sketch) => sketch,
            other => panic!("expected a sketch, got {other:?}"),
        }
    }

    fn face_plane(query: &str, fingerprint: &str) -> serde_json::Value {
        json!({
            "kind": "face",
            "body": "b-1",
            "face": {"q": query, "fp": fingerprint},
            "fallback": {"origin": [3, 4, 0], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}
        })
    }

    #[test]
    fn registers_through_the_public_registry() {
        let mut registry = Registry::new();

        assert_eq!(registry.register::<SketchFeature>(), Ok(()));
    }

    #[test]
    fn dimension_hints_label_every_constraint_expression() {
        let field = crate::FieldRef::new("constraints[7].expr", &SketchFeature::manifest());

        assert_eq!(field.label, "Dimension");
    }

    #[test]
    fn dependencies_list_reference_dimensions_with_their_constraint_index() {
        let sketch = line_dimension(
            "width",
            &[json!({"id": "c-4", "type": "distance", "line": "e-1", "expr": "wdth", "driving": false})],
        );

        let uses: Vec<(String, String, FieldUnit)> = SketchFeature::dependencies(&sketch)
            .expressions
            .into_iter()
            .map(|usage| (usage.field, usage.expr.as_str().to_string(), usage.kind))
            .collect();

        assert_eq!(
            uses,
            [
                (
                    "constraints[2].expr".to_string(),
                    "width".to_string(),
                    FieldUnit::Length
                ),
                (
                    "constraints[3].expr".to_string(),
                    "wdth".to_string(),
                    FieldUnit::Length
                ),
            ]
        );
    }

    #[test]
    fn dependencies_declare_entity_ids_then_constraint_ids() {
        let declared: Vec<String> = SketchFeature::dependencies(&line_dimension("5", &[]))
            .declared_ids
            .iter()
            .map(|id| id.to_string())
            .collect();

        assert_eq!(declared, ["e-1", "c-1", "c-2", "c-3"]);
    }

    #[test]
    fn dependencies_declare_the_datum_of_the_plane() {
        let dependencies =
            SketchFeature::dependencies(&rectangle(json!({"kind": "datum", "feature": "f-dp1"})));

        assert_eq!(dependencies.features.len(), 1);
        assert_eq!(dependencies.features[0].id.as_str(), "f-dp1");
        assert!(dependencies.bodies_read.is_empty());
    }

    #[test]
    fn dependencies_read_the_body_of_a_face_plane() {
        let dependencies = SketchFeature::dependencies(&rectangle(face_plane(
            "f-box1/top",
            "plane|n=0,0,1",
        )));

        assert_eq!(dependencies.bodies_read.len(), 1);
        assert_eq!(dependencies.bodies_read[0].id.as_str(), "b-1");
        assert!(dependencies.features.is_empty());
    }

    #[test]
    fn validate_reports_the_sketch_crate_error() {
        let sketch = params(json!({
            "plane": {"kind": "origin", "plane": "XY"},
            "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [10, 0]}],
            "constraints": [{"id": "c-1", "type": "horizontal", "line": "e-9"}]
        }));

        let error = SketchFeature::validate(&sketch).expect_err("e-9 does not exist");

        assert!(
            matches!(
                error,
                FeatureError::SketchInvalid {
                    error: SketchError::UnknownEntity { .. }
                }
            ),
            "{error:?}"
        );
    }

    #[test]
    fn build_places_an_origin_sketch_and_finds_its_region() {
        let output = built(
            &rectangle(json!({"kind": "origin", "plane": "XZ"})),
            Vec::new(),
        )
        .expect("the rectangle builds");

        assert_eq!(sketch(&output).placement, OriginPlane::Xz.placement());
        assert_eq!(sketch(&output).profile.regions.len(), 1);
        assert!(output.warnings.is_empty());
    }

    #[test]
    fn build_solves_driving_dimensions() {
        let output = built(&line_dimension("100 mm", &[]), Vec::new()).expect("the line solves");

        let end = sketch(&output)
            .point(&"e-1.end".parse().expect("valid point reference"))
            .expect("the line has an end");
        assert!((end[0] - 100.0).abs() < 1e-9 && end[1].abs() < 1e-9, "{end:?}");
    }

    #[test]
    fn build_does_not_evaluate_reference_dimensions() {
        let sketch = line_dimension(
            "100 mm",
            &[json!({"id": "c-4", "type": "distance", "line": "e-1", "expr": "wdth", "driving": false})],
        );

        let output = built(&sketch, Vec::new()).expect("a broken reference dimension is ignored");

        assert!(output.warnings.is_empty());
    }

    #[test]
    fn build_reports_conflicting_constraints() {
        let sketch = line_dimension(
            "100 mm",
            &[json!({"id": "c-4", "type": "distance", "line": "e-1", "expr": "50 mm"})],
        );

        let error = built(&sketch, Vec::new()).expect_err("the dimensions conflict");

        match error {
            FeatureError::SketchConflict { constraints } => {
                assert!(constraints.iter().any(|id| id.as_str() == "c-4"), "{constraints:?}");
            }
            other => panic!("expected a conflict, got {other:?}"),
        }
    }

    #[test]
    fn build_warns_about_redundant_constraints() {
        let sketch = line_dimension(
            "100 mm",
            &[json!({"id": "c-4", "type": "distance", "line": "e-1", "expr": "100 mm"})],
        );

        let output = built(&sketch, Vec::new()).expect("a redundant dimension still solves");

        assert_eq!(output.warnings.len(), 1);
        assert_eq!(output.warnings[0].code(), "redundant_constraints");
    }

    #[test]
    fn build_reports_a_value_the_solver_rejects() {
        let error = built(&line_dimension("-5", &[]), Vec::new()).expect_err("-5 is not a length");

        assert!(
            matches!(
                error,
                FeatureError::SketchInvalid {
                    error: SketchError::ValueNotPositive { .. }
                }
            ),
            "{error:?}"
        );
    }

    #[test]
    fn build_projects_the_fallback_onto_a_resolved_face() {
        let output = built(
            &rectangle(face_plane("f-box1/top", "plane|n=0,0,1|c=5,5,10|a=100")),
            vec![("b-1", box_body("f-box1", [0.0, 0.0, 0.0], 10.0))],
        )
        .expect("the sketch builds on the face");

        let placement = sketch(&output).placement;
        assert_eq!(placement.origin, [3.0, 4.0, 10.0]);
        assert_eq!(placement.x_axis, [1.0, 0.0, 0.0]);
        assert_eq!(placement.normal, [0.0, 0.0, 1.0]);
        assert!(output.warnings.is_empty());
    }

    #[test]
    fn build_keeps_the_fallback_when_the_face_is_lost() {
        let params = rectangle(face_plane(
            "f-box1/nothing",
            "plane|n=0,0,1|c=500,500,500|a=1",
        ));
        let output = built(
            &params,
            vec![("b-1", box_body("f-box1", [0.0, 0.0, 0.0], 10.0))],
        )
        .expect("a lost face falls back");

        assert_fallback(&output, &params, FallbackReason::Missing);
    }

    #[test]
    fn build_keeps_the_fallback_when_the_body_is_empty() {
        let params = rectangle(face_plane("f-box1/top", "plane|n=0,0,1|c=5,5,10|a=100"));
        let output = built(&params, vec![("b-1", BodyState::empty())])
            .expect("an empty body falls back");

        assert_fallback(&output, &params, FallbackReason::Missing);
    }

    /// Asserts that the sketch kept its fallback placement with one `PlaneFallback` warning.
    fn assert_fallback(output: &FeatureOutput, params: &SketchParams, expected: FallbackReason) {
        let SketchPlane::Face { fallback, .. } = &params.plane else {
            panic!("the sketch is on a face");
        };
        assert_eq!(sketch(output).placement, *fallback);
        match output.warnings.as_slice() {
            [FeatureWarning::PlaneFallback { field, reason, .. }] => {
                assert_eq!(*reason, expected);
                assert_eq!(field.pointer(), "/plane/face");
            }
            other => panic!("expected one plane fallback warning, got {other:?}"),
        }
    }
}
