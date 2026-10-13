//! The `fillet` feature: rounds edges of a body with a constant radius (features/fillet.md).

use partsmith_model::{Category, Expr, FeatureManifest, FieldUnit, Id, SelectionKind};
use partsmith_naming::{Reference, roles};
use partsmith_occt_sys::{EdgeRadius, fillet};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    body_hint, essential, manifest, positive_length, references, require_selection, selection_hint,
    solid_body, unit_hint,
};
use super::dressup::{BlendTarget, blend};
use crate::{
    BuildContext, Dependencies, Feature, FeatureError, FeatureOutput, Noun, PythonCall,
    PythonNames, PythonValue,
};

/// Rounds edges of a body with a constant radius.
#[derive(Clone, Copy, Debug)]
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
        manifest(
            "Fillet",
            Category::Modify,
            "fillet",
            vec![
                essential(selection_hint("edges", "Edges", SelectionKind::Edge)),
                essential(unit_hint("radius", "Radius", FieldUnit::Length)),
                body_hint(),
            ],
        )
    }

    fn dependencies(params: &FilletParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_body_written(&params.body, "body");
        dependencies.add_expression("radius", &params.radius, FieldUnit::Length);
        dependencies
    }

    fn validate(params: &FilletParams) -> Result<(), FeatureError> {
        require_selection(&params.edges, Self::manifest, "edges", Noun::Edge)
    }

    fn build(
        params: &FilletParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let (state, shape) = solid_body(context, &params.body)?;
        let radius = positive_length(context, "radius", &params.radius)?;
        let target = BlendTarget {
            body: &params.body,
            state,
            shape,
            edges: &params.edges,
        };
        blend(
            context,
            target,
            roles::FILLET,
            radius,
            |edge| Ok(edge.index),
            |edges, radius| {
                let edges: Vec<EdgeRadius> = edges
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
        )
    }

    /// `fillet(edges, radius)`: the edges as references, the radius as an expression; the
    /// generator's `with <part>:` block gives the body.
    fn python(params: &FilletParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let mut call = PythonCall::new("fillet");
        call.positional = vec![
            references(&params.edges),
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
    use crate::test_support::{ScriptNames, id, reference};
    use crate::{FieldRef, Registry};

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
    fn the_manifest_matches_the_spec_table() {
        let registry = registry();

        let manifest = registry.get("fillet").expect("registered").manifest();

        assert_eq!(
            serde_json::to_value(manifest).expect("a manifest serializes"),
            json!({
                "label": "Fillet", "category": "modify", "icon": "fillet", "version": 1,
                "fields": [
                    {"path": "edges", "label": "Edges", "essential": true, "selection": "edge"},
                    {"path": "radius", "label": "Radius", "essential": true, "unit": "length"},
                    {"path": "body", "label": "Body", "selection": "body"},
                ]
            })
        );
    }

    #[test]
    fn the_feature_type_is_copy_and_debug() {
        fn debug_text<T: Copy + std::fmt::Debug>(value: T) -> String {
            format!("{value:?}")
        }

        assert_eq!(debug_text(Fillet), "Fillet");
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
        assert_eq!(
            error.field().map(FieldRef::pointer),
            Some("/edges".to_string())
        );
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
        let mut call = Fillet::python(&params(&[FRONT_TOP], "2 mm"), &ScriptNames::default())
            .expect("a dedicated form");
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

        let call = Fillet::python(
            &params(&[FRONT_TOP, back_top], "radius"),
            &ScriptNames::default(),
        )
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
