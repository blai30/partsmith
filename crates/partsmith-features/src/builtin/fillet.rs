//! The `fillet` feature: rounds edges of a body with a constant radius (features/fillet.md).

use partsmith_model::{
    Category, Expr, FeatureManifest, FieldHint, FieldUnit, Id, SelectionKind,
};
use partsmith_naming::{Reference, roles};
use partsmith_occt_sys::{EdgeRadius, fillet};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::dressup::{
    modified, name_blend, python_references, run_or_search, select_edges, unchanged,
};
use crate::{
    Bound, BuildContext, Dependencies, Feature, FeatureError, FeatureOutput, FieldRef, Noun,
    PythonCall, PythonNames, PythonValue,
};

/// Rounds edges of a body with a constant radius.
pub struct Fillet;

/// Params of a fillet feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FilletParams {
    /// The body whose edges are rounded.
    pub body: Id,
    /// Edges to round, at least one.
    pub edges: Vec<Reference>,
    /// Fillet radius. Length, greater than 0.
    pub radius: Expr,
}

impl Feature for Fillet {
    const TYPE: &'static str = "fillet";
    type Params = FilletParams;

    fn manifest() -> FeatureManifest {
        let hint = |path: &str, label: &str, essential: bool| FieldHint {
            path: path.to_string(),
            label: label.to_string(),
            essential,
            unit: None,
            selection: None,
            options: Vec::new(),
        };
        FeatureManifest {
            label: "Fillet".to_string(),
            category: Category::Modify,
            icon: "fillet".to_string(),
            version: 1,
            fields: vec![
                FieldHint {
                    selection: Some(SelectionKind::Edge),
                    ..hint("edges", "Edges", true)
                },
                FieldHint {
                    unit: Some(FieldUnit::Length),
                    ..hint("radius", "Radius", true)
                },
                FieldHint {
                    selection: Some(SelectionKind::Body),
                    ..hint("body", "Body", false)
                },
            ],
        }
    }

    fn dependencies(params: &FilletParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_body_written(&params.body, "body");
        dependencies.add_expression("radius", &params.radius, FieldUnit::Length);
        dependencies
    }

    fn validate(params: &FilletParams) -> Result<(), FeatureError> {
        if params.edges.is_empty() {
            return Err(FeatureError::EmptySelection {
                field: FieldRef::new("edges", &Self::manifest()),
                noun: Noun::Edge,
            });
        }
        Ok(())
    }

    fn build(
        params: &FilletParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let state = context.body(&params.body)?;
        let Some(shape) = state.shape() else {
            return Err(FeatureError::BodyEmpty {
                body: params.body.clone(),
                use_new: false,
            });
        };
        let radius = context.eval_length("radius", &params.radius)?;
        if radius <= 0.0 {
            return Err(FeatureError::OutOfRange {
                field: context.field("radius"),
                kind: FieldUnit::Length,
                value: radius,
                bound: Bound::GreaterThan(0.0),
                bound_name: None,
            });
        }
        let selection = select_edges(context, &params.body, shape, &params.edges)?;
        if selection.edges.is_empty() {
            return Ok(unchanged(&params.body, state, selection.warnings));
        }
        let result = run_or_search(
            context,
            radius,
            |radius| {
                let edges: Vec<EdgeRadius> = selection
                    .edges
                    .iter()
                    .map(|&edge| EdgeRadius { edge, radius })
                    .collect();
                fillet(shape, &edges)
            },
            |max, cause| FeatureError::FilletTooLarge {
                field: context.field("radius"),
                radius,
                max,
                cause,
            },
        )?;
        let naming = name_blend(context.feature(), roles::FILLET, state.naming(), &result)?;
        Ok(modified(
            &params.body,
            result.shape,
            naming,
            selection.warnings,
        ))
    }

    /// `fillet(edges, radius)`: the edges as references, the radius as an expression; the
    /// generator's `with <part>:` block gives the body.
    fn python(params: &FilletParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let mut call = PythonCall::new("fillet");
        call.positional = vec![
            python_references(&params.edges),
            PythonValue::expression(&params.radius, names),
        ];
        Some(call)
    }
}

#[cfg(test)]
mod tests {
    use partsmith_model::{FeatureRecord, Units};
    use serde_json::json;

    use super::*;
    use crate::builtin::dressup::test_fixtures::{ScriptNames, reference};
    use crate::{Registry, test_support::id};

    /// `E-FT` of fillet.md section 6.
    const FRONT_TOP: &str =
        r#"{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}"#;

    fn params(edges: &[&str], radius: &str) -> FilletParams {
        FilletParams {
            body: id("b-1"),
            edges: edges.iter().map(|edge| reference(edge)).collect(),
            radius: Expr::new(radius),
        }
    }

    fn registry() -> Registry {
        let mut registry = Registry::new();
        registry
            .register::<Fillet>()
            .expect("the fillet passes the registration checks");
        registry
    }

    #[test]
    fn the_manifest_matches_the_params() {
        let registry = registry();

        let manifest = registry.get("fillet").expect("registered").manifest();

        assert_eq!(manifest.label, "Fillet");
        assert_eq!(manifest.category, Category::Modify);
        assert_eq!(manifest.icon, "fillet");
        assert_eq!(manifest.version, 1);
    }

    #[test]
    fn params_canonicalize_in_struct_order() {
        let record: FeatureRecord = serde_json::from_value(json!({
            "id": "f-fil1", "type": "fillet", "radius": "2 mm", "body": "b-1",
            "edges": [{"fp": "line|n=1,0,0|c=0,-10,20|l=20", "q": "edge(f-box1/front|f-box1/top)"}]
        }))
        .expect("a record");

        let validated = registry()
            .validate_and_canonicalize(&record)
            .expect("valid");

        let keys: Vec<&str> = validated.record.params.keys().map(String::as_str).collect();
        assert_eq!(keys, ["body", "edges", "radius"]);
        assert!(validated.unknown_keys.is_empty());
    }

    #[test]
    fn no_edges_is_an_empty_selection() {
        let error = Fillet::validate(&params(&[], "2")).expect_err("no edges");

        assert_eq!(error.code(), "empty_selection");
        assert_eq!(error.to_string(), "Select at least one edge for Edges.");
    }

    #[test]
    fn dependencies_write_the_body_and_read_the_radius() {
        let dependencies = Fillet::dependencies(&params(&[FRONT_TOP], "2 mm"));

        assert_eq!(dependencies.bodies_written.len(), 1);
        assert_eq!(dependencies.bodies_written[0].id, id("b-1"));
        assert_eq!(dependencies.bodies_written[0].field, "body");
        assert!(dependencies.bodies_read.is_empty());
        assert!(dependencies.features.is_empty());
        assert_eq!(dependencies.expressions.len(), 1);
        assert_eq!(dependencies.expressions[0].field, "radius");
        assert_eq!(dependencies.expressions[0].expr, Expr::new("2 mm"));
        assert_eq!(dependencies.expressions[0].kind, FieldUnit::Length);
    }

    #[test]
    fn python_writes_one_edge_alone_and_wraps_long_calls() {
        let mut call =
            Fillet::python(&params(&[FRONT_TOP], "2 mm"), &ScriptNames).expect("a dedicated form");
        call.keywords
            .push(("id".to_string(), PythonValue::Str("f-fil1".to_string())));

        assert_eq!(
            call.render(0),
            "fillet(\n    Ref(\"edge(f-box1/front|f-box1/top)\", \"line|n=1,0,0|c=0,-10,20|l=20\"),\n    2*mm,\n    id=\"f-fil1\",\n)"
        );
    }

    #[test]
    fn python_writes_several_edges_as_a_list() {
        let back_top =
            r#"{"q": "edge(f-box1/back|f-box1/top)", "fp": "line|n=1,0,0|c=0,10,20|l=20"}"#;

        let call = Fillet::python(&params(&[FRONT_TOP, back_top], "radius"), &ScriptNames)
            .expect("a dedicated form");

        assert!(matches!(&call.positional[0], PythonValue::List(items) if items.len() == 2));
        assert_eq!(call.positional[1], PythonValue::Str("radius".to_string()));
        assert!(call.keywords.is_empty());
    }

    #[test]
    fn a_too_large_radius_suggests_the_largest_that_works() {
        let error = FeatureError::FilletTooLarge {
            field: FieldRef::new("radius", &Fillet::manifest()),
            radius: 3.0,
            max: Some(2.396),
            cause: partsmith_occt_sys::KernelError {
                operation: partsmith_occt_sys::Operation::Fillet,
                kind: partsmith_occt_sys::KernelErrorKind::Failed,
                exception: "StdFail_NotDone".to_string(),
                message: String::new(),
            },
        };

        assert_eq!(error.suggestion(Units::Mm), Some("2.3 mm".to_string()));
        assert_eq!(
            error.field().map(FieldRef::pointer),
            Some("/radius".to_string())
        );
    }
}
