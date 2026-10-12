//! What the primitive features share besides their placement (features.md section 3.14): the
//! manifest, dependencies, tool and Python form of a solid made from size fields, a placement and
//! an op.

use partsmith_model::{Category, Expr, FeatureManifest, FieldUnit, Id};
use partsmith_occt_sys::{KernelError, Primitive};
use partsmith_sketch::Placement;

use super::common::{
    apply_tool, body_hint, body_op_hint, call, essential, manifest, op_keyword, primitive_tool,
    unit_hint,
};
use crate::PrimitivePlacement;
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::FeatureError;
use crate::output::FeatureOutput;
use crate::params::BodyOp;
use crate::python::{PythonCall, PythonNames, PythonValue};

/// The manifest of a primitive (category create, version 1): its `(path, label)` sizes as
/// essential lengths in field order, `op`, the placement hints, then `body`.
pub(crate) fn primitive_manifest(
    label: &str,
    icon: &str,
    sizes: &[(&str, &str)],
) -> FeatureManifest {
    let mut fields: Vec<_> = sizes
        .iter()
        .map(|(path, label)| essential(unit_hint(path, label, FieldUnit::Length)))
        .collect();
    fields.push(body_op_hint());
    fields.extend(PrimitivePlacement::hints());
    fields.push(body_hint());
    manifest(label, Category::Create, icon, fields)
}

/// A primitive's params as the steps all primitives share read them.
pub(crate) struct PrimitiveParams<'a> {
    /// The target body.
    pub(crate) body: &'a Id,
    /// The size fields in field order: path and expression.
    pub(crate) sizes: Vec<(&'static str, &'a Expr)>,
    /// The `plane` and `position` params.
    pub(crate) placement: PrimitivePlacement<'a>,
    /// How the primitive combines with the body.
    pub(crate) op: BodyOp,
}

impl PrimitiveParams<'_> {
    /// What the primitive reads and writes: the body it writes, the sizes as lengths in field
    /// order, then the placement.
    pub(crate) fn dependencies(&self) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_body_written(self.body, "body");
        for (field, expr) in &self.sizes {
            dependencies.add_expression(field, expr, FieldUnit::Length);
        }
        self.placement.add_dependencies(&mut dependencies);
        dependencies
    }

    /// Evaluates the placement, builds the kernel solid that `make` returns in that local frame,
    /// names its faces `<feature>/<role>` and combines it with the body. Callers check the sizes
    /// first, so no kernel work happens before every size is checked.
    pub(crate) fn build(
        &self,
        context: &BuildContext<'_>,
        make: impl FnOnce(&Placement) -> Result<Primitive, KernelError>,
    ) -> Result<FeatureOutput, FeatureError> {
        let frame = self.placement.frame(context)?;
        let solid = make(&frame)?;
        let tool = primitive_tool(context.feature(), solid)?;
        apply_tool(context, self.body, self.op, tool, Vec::new())
    }

    /// The dedicated Python form `function(sizes..., plane=..., position=..., op=...)`, with the
    /// keywords only when they are not the default. None when the plane has no variable.
    pub(crate) fn python(&self, function: &str, names: &dyn PythonNames) -> Option<PythonCall> {
        let sizes = self
            .sizes
            .iter()
            .map(|(_, expr)| PythonValue::expression(expr, names))
            .collect();
        let mut keywords = self.placement.python_keywords(names)?;
        keywords.extend(op_keyword(names, self.body, self.op));
        Some(call(function, sizes, keywords))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use partsmith_occt_sys::make_sphere;

    use super::*;
    use crate::builtin::cylinder::Cylinder;
    use crate::default_position;
    use crate::params::{PlacementExt, PlaneRef};
    use crate::test_support::{ScriptNames, build, fixture, id, params};

    /// `CYL` of cylinder.md with this radius.
    fn cylinder(radius: &str) -> serde_json::Value {
        json!({"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": radius, "height": "10", "op": "new"})
    }

    /// The error of a cylinder of this radius.
    fn cylinder_error(radius: &str) -> FeatureError {
        let params = params::<Cylinder>(cylinder(radius));
        build::<Cylinder>("f-cyl1", &params, Vec::new(), Vec::new())
            .expect_err("the radius is out of range")
    }

    /// Params with the sizes `radius` and `height`, then this placement and op.
    fn with_parts<'a>(
        body: &'a Id,
        sizes: &'a [Expr; 2],
        plane: &'a PlaneRef,
        position: &'a [Expr; 3],
        op: BodyOp,
    ) -> PrimitiveParams<'a> {
        PrimitiveParams {
            body,
            sizes: vec![("radius", &sizes[0]), ("height", &sizes[1])],
            placement: PrimitivePlacement { plane, position },
            op,
        }
    }

    #[test]
    fn the_manifest_lists_sizes_then_op_then_placement_then_body() {
        let manifest = primitive_manifest("Cylinder", "cylinder", &[("radius", "Radius")]);

        let paths: Vec<&str> = manifest
            .fields
            .iter()
            .map(|field| field.path.as_str())
            .collect();
        assert_eq!(
            paths,
            [
                "radius",
                "op",
                "plane",
                "position[0]",
                "position[1]",
                "position[2]",
                "body"
            ]
        );
        assert_eq!(manifest.fields[0].unit, Some(FieldUnit::Length));
        assert!(manifest.fields[0].essential && manifest.fields[1].essential);
        assert!(!manifest.fields[2..].iter().any(|field| field.essential));
        assert_eq!(
            (manifest.label.as_str(), manifest.icon.as_str()),
            ("Cylinder", "cylinder")
        );
        assert_eq!(manifest.category, Category::Create);
    }

    #[test]
    fn dependencies_are_the_body_then_the_sizes_then_the_position() {
        let body = id("b-1");
        let sizes = [Expr::new("5"), Expr::new("h")];
        let plane = PlaneRef::default();
        let position = default_position();

        let dependencies = with_parts(&body, &sizes, &plane, &position, BodyOp::New).dependencies();

        let written: Vec<&str> = dependencies
            .bodies_written
            .iter()
            .map(|link| link.id.as_str())
            .collect();
        assert_eq!(written, ["b-1"]);
        let fields: Vec<&str> = dependencies
            .expressions
            .iter()
            .map(|expression| expression.field.as_str())
            .collect();
        assert_eq!(
            fields,
            [
                "radius",
                "height",
                "position[0]",
                "position[1]",
                "position[2]"
            ]
        );
        assert!(
            dependencies
                .expressions
                .iter()
                .all(|expression| expression.kind == FieldUnit::Length)
        );
    }

    #[test]
    fn python_writes_sizes_then_placement_then_op() {
        let body = id("b-1");
        let sizes = [Expr::new("5"), Expr::new("10 mm")];
        let plane = PlaneRef::Datum {
            feature: id("f-dp1"),
        };
        let position = [Expr::new("1"), Expr::new("0"), Expr::new("0")];

        let python = with_parts(&body, &sizes, &plane, &position, BodyOp::Cut)
            .python("cylinder", &ScriptNames::default())
            .expect("the datum has a variable");

        assert_eq!(
            python.render(0),
            r#"cylinder(5, 10*mm, plane=plane_1, position=(1, 0, 0), op="cut")"#
        );
    }

    #[test]
    fn python_has_no_dedicated_form_without_the_plane_variable() {
        let body = id("b-1");
        let sizes = [Expr::new("5"), Expr::new("10")];
        let plane = PlaneRef::Datum {
            feature: id("f-dp9"),
        };
        let position = default_position();

        let python = with_parts(&body, &sizes, &plane, &position, BodyOp::New)
            .python("cylinder", &ScriptNames::default());

        assert_eq!(python, None);
    }

    #[test]
    fn a_size_below_the_tolerance_is_out_of_range_like_zero() {
        let tiny = cylinder_error("0.0000001");
        let zero = cylinder_error("0");

        assert_eq!(tiny.code(), "out_of_range");
        assert_eq!(tiny.to_string(), zero.to_string());
        assert_eq!(
            tiny.to_string(),
            "Radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm."
        );
    }

    #[test]
    fn a_size_at_the_tolerance_builds() {
        let params = params::<Cylinder>(cylinder("0.000001"));

        let built = build::<Cylinder>("f-cyl1", &params, Vec::new(), Vec::new());

        assert!(built.is_ok(), "{built:?}");
    }

    #[test]
    fn a_kernel_failure_is_a_kernel_error() {
        let params = params::<Cylinder>(cylinder("5"));
        let fixture = fixture::<Cylinder>("f-cyl1", &params, Vec::new(), Vec::new());
        let body = id("b-1");
        let sizes = [Expr::new("5"), Expr::new("10")];
        let plane = PlaneRef::default();
        let position = default_position();

        // A radius below the kernel's modeling tolerance, which the size check never lets through.
        let error = with_parts(&body, &sizes, &plane, &position, BodyOp::New)
            .build(&fixture.context(), |frame| {
                make_sphere(&frame.to_kernel(), 0.0000005)
            })
            .expect_err("the kernel rejects the radius");

        assert_eq!(error.code(), "kernel");
    }
}
