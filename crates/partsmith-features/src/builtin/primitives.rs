//! What the five primitive types (box, cylinder, sphere, cone, torus) share beyond their placement
//! params (features.md section 3.14): the manifest, dependencies, range checks, naming the
//! kernel's primitive faces, combining the tool with the body, and the dedicated Python call.

use partsmith_model::{Expr, Id};
use partsmith_naming::{ElementName, FaceSource, roles};
use partsmith_occt_sys::{KernelError, Primitive, PrimitiveFace};

use crate::{
    BodyOp, Bound, BuildContext, Category, Dependencies, FeatureError, FeatureManifest,
    FeatureOutput, FieldHint, FieldUnit, OptionHint, OutputData, PrimitivePlacement, PythonCall,
    PythonNames, PythonValue, SelectionKind, Tool, apply_op, name_shape,
};

/// One size param of a primitive: its field path and its manifest label.
#[derive(Clone, Copy, Debug)]
pub(super) struct SizeField {
    /// Field path, for example `size[0]` or `radius`.
    pub(super) path: &'static str,
    /// Manifest label, for example `Length`.
    pub(super) label: &'static str,
}

/// The manifest of a primitive (category create, version 1): its essential size fields, the
/// essential `op`, the placement fields and `body`, in that order.
pub(super) fn manifest(label: &str, icon: &str, sizes: &[SizeField]) -> FeatureManifest {
    let mut fields: Vec<FieldHint> = sizes
        .iter()
        .map(|size| FieldHint {
            path: size.path.to_string(),
            label: size.label.to_string(),
            essential: true,
            unit: Some(FieldUnit::Length),
            selection: None,
            options: Vec::new(),
        })
        .collect();
    fields.push(FieldHint {
        path: "op".to_string(),
        label: "Operation".to_string(),
        essential: true,
        unit: None,
        selection: None,
        options: [
            (BodyOp::New, "New body"),
            (BodyOp::Join, "Join"),
            (BodyOp::Cut, "Cut"),
            (BodyOp::Intersect, "Intersect"),
        ]
        .into_iter()
        .map(|(op, label)| OptionHint {
            value: op_name(op).to_string(),
            label: label.to_string(),
        })
        .collect(),
    });
    fields.extend(PrimitivePlacement::hints());
    fields.push(FieldHint {
        path: "body".to_string(),
        label: "Body".to_string(),
        essential: false,
        unit: None,
        selection: Some(SelectionKind::Body),
        options: Vec::new(),
    });
    FeatureManifest {
        label: label.to_string(),
        category: Category::Create,
        icon: icon.to_string(),
        version: 1,
        fields,
    }
}

/// The dependencies of a primitive: it writes `body`, reads its sizes as lengths in field order,
/// then whatever its placement reads (a placement face on `body` keeps `body` written).
pub(super) fn dependencies(
    body: &Id,
    sizes: &[(SizeField, &Expr)],
    placement: PrimitivePlacement<'_>,
) -> Dependencies {
    let mut dependencies = Dependencies::default();
    dependencies.add_body_written(body, "body");
    for (size, expr) in sizes {
        dependencies.add_expression(size.path, expr, FieldUnit::Length);
    }
    placement.add_dependencies(&mut dependencies);
    dependencies
}

/// The size at `field` in millimeters, which must be greater than 0.
pub(super) fn positive_length(
    context: &BuildContext<'_>,
    field: &str,
    expr: &Expr,
) -> Result<f64, FeatureError> {
    let value = context.eval_length(field, expr)?;
    if value > 0.0 {
        Ok(value)
    } else {
        Err(out_of_range(context, field, value, Bound::GreaterThan(0.0), None))
    }
}

/// The size at `field` in millimeters, which must be at least 0.
pub(super) fn non_negative_length(
    context: &BuildContext<'_>,
    field: &str,
    expr: &Expr,
) -> Result<f64, FeatureError> {
    let value = context.eval_length(field, expr)?;
    if value >= 0.0 {
        Ok(value)
    } else {
        Err(out_of_range(context, field, value, Bound::AtLeast(0.0), None))
    }
}

/// The `OutOfRange` error of the length `value` at `field`. `bound_name` says what the bound is
/// when it comes from another value, for example "the ring radius".
pub(super) fn out_of_range(
    context: &BuildContext<'_>,
    field: &str,
    value: f64,
    bound: Bound,
    bound_name: Option<&str>,
) -> FeatureError {
    FeatureError::OutOfRange {
        field: context.field(field),
        kind: FieldUnit::Length,
        value,
        bound,
        bound_name: bound_name.map(str::to_string),
    }
}

/// A kernel failure as a feature error.
pub(super) fn kernel(error: KernelError) -> FeatureError {
    FeatureError::Kernel { error }
}

/// Names the faces of a kernel primitive `<feature>/<role>` from their `PrimitiveFace` roles
/// (naming.md section 3.6.4) and combines it with `body` by `op` (features.md section 3.5).
pub(super) fn combine(
    context: &BuildContext<'_>,
    body: &Id,
    op: BodyOp,
    primitive: Primitive,
) -> Result<FeatureOutput, FeatureError> {
    let faces = primitive
        .faces
        .iter()
        .map(|face| {
            ElementName::generated(context.feature(), role(*face), None).map(FaceSource::New)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| FeatureError::Internal {
            message: format!("naming the primitive's faces failed: {error}"),
        })?;
    let naming = name_shape(&primitive.shape, faces, &[])?;
    let tool = Tool {
        shape: primitive.shape,
        naming,
    };
    let (update, warnings) = apply_op(context, body, op, vec![tool])?;
    Ok(FeatureOutput {
        data: OutputData::Bodies(vec![update]),
        warnings,
    })
}

/// The dedicated Python call of a primitive (features.md section 3.14): the sizes as positional
/// expressions in field order, then `plane=` and `position=` when not the default, then `op=`
/// when it differs from the op the script uses for `body` by default. None when the placement's
/// plane has no variable.
pub(super) fn python(
    function: &str,
    sizes: &[&Expr],
    body: &Id,
    op: BodyOp,
    placement: PrimitivePlacement<'_>,
    names: &dyn PythonNames,
) -> Option<PythonCall> {
    let mut call = PythonCall::new(function);
    call.positional = sizes
        .iter()
        .map(|expr| PythonValue::expression(expr, names))
        .collect();
    call.keywords = placement.python_keywords(names)?;
    if op != names.default_op(body) {
        call.keywords.push((
            "op".to_string(),
            PythonValue::Str(op_name(op).to_string()),
        ));
    }
    Some(call)
}

/// The face role of a kernel primitive face (naming.md section 3.6.4).
fn role(face: PrimitiveFace) -> &'static str {
    match face {
        PrimitiveFace::Left => roles::LEFT,
        PrimitiveFace::Right => roles::RIGHT,
        PrimitiveFace::Front => roles::FRONT,
        PrimitiveFace::Back => roles::BACK,
        PrimitiveFace::Bottom => roles::BOTTOM,
        PrimitiveFace::Top => roles::TOP,
        PrimitiveFace::Side => roles::SIDE,
        PrimitiveFace::Surface => roles::SURFACE,
    }
}

/// The serialized name of an op, as files and Python write it.
fn op_name(op: BodyOp) -> &'static str {
    match op {
        BodyOp::New => "new",
        BodyOp::Join => "join",
        BodyOp::Cut => "cut",
        BodyOp::Intersect => "intersect",
    }
}

/// Builds primitive params in a millimeter document through the same steps recompute takes, and
/// writes their dedicated Python, for the unit tests of the primitive types.
#[cfg(test)]
pub(super) mod testing {
    use partsmith_model::{EmptyContext, FeatureRecord, Id, ParameterValues, Scope, Units};
    use partsmith_sketch::RegionRef;

    use crate::test_support::{Fixture, id};
    use crate::{BodyOp, BodyState, Feature, FeatureError, FeatureOutput, PythonNames, Values};

    /// Builds `params` as feature `f-prim1` of type `F`, with every body it declares empty unless
    /// `bodies` gives its state.
    pub(crate) fn build<F: Feature>(
        params: &F::Params,
        bodies: Vec<(&str, BodyState)>,
    ) -> Result<FeatureOutput, FeatureError> {
        let dependencies = F::dependencies(params);
        let mut fixture = Fixture::new("f-prim1");
        fixture.manifest = F::manifest();
        let parameters = ParameterValues::default();
        let scope = Scope {
            units: Units::Mm,
            parameters: &parameters,
            context: &EmptyContext,
        };
        fixture.values = Values::evaluate(&dependencies.expressions, &scope, &fixture.manifest);
        for link in dependencies
            .bodies_written
            .iter()
            .chain(&dependencies.bodies_read)
        {
            fixture.bodies.insert(link.id.clone(), BodyState::empty());
        }
        for (body, state) in bodies {
            fixture.bodies.insert(id(body), state);
        }
        F::build(params, &fixture.context())
    }

    /// The typed params of a record's JSON params.
    pub(crate) fn params<F: Feature>(record: serde_json::Value) -> F::Params {
        let record: FeatureRecord =
            serde_json::from_value(record).expect("records written in tests are well-formed");
        serde_json::from_value(serde_json::Value::Object(record.params))
            .expect("params written in tests deserialize")
    }

    /// Script names with millimeter units, the datum plane `f-dp1` as `plane_1`, the body `b-1`
    /// as `block`, and `default_op` for every body.
    pub(crate) struct ScriptNames {
        /// The op the script uses when `op` is omitted.
        pub(crate) default_op: BodyOp,
    }

    impl PythonNames for ScriptNames {
        fn units(&self) -> Units {
            Units::Mm
        }

        fn body(&self, body: &Id) -> Option<String> {
            (body.as_str() == "b-1").then(|| "block".to_string())
        }

        fn feature(&self, feature: &Id) -> Option<String> {
            (feature.as_str() == "f-dp1").then(|| "plane_1".to_string())
        }

        fn entity(
            &self,
            _sketch: &Id,
            _entity: &Id,
        ) -> Option<String> {
            None
        }

        fn parameter(&self, _name: &str) -> Option<String> {
            None
        }

        fn regions(&self, _sketch: &Id) -> Option<Vec<RegionRef>> {
            None
        }

        fn default_op(&self, _body: &Id) -> BodyOp {
            self.default_op
        }
    }

    /// The dedicated Python call of `params` rendered at indentation 0, or None.
    pub(crate) fn python<F: Feature>(params: &F::Params, default_op: BodyOp) -> Option<String> {
        F::python(params, &ScriptNames { default_op }).map(|call| call.render(0))
    }
}

#[cfg(test)]
mod tests {
    use partsmith_occt_sys::{Plane, make_box, make_sphere};
    use serde_json::json;

    use super::*;
    use crate::test_support::{Fixture, face_names, id};
    use crate::{BodyChange, BodyState, PlaneRef, default_position};

    const SIZES: [SizeField; 2] = [
        SizeField {
            path: "radius",
            label: "Radius",
        },
        SizeField {
            path: "height",
            label: "Height",
        },
    ];

    #[test]
    fn every_kernel_face_role_has_a_naming_role() {
        let names: Vec<&str> = [
            PrimitiveFace::Left,
            PrimitiveFace::Right,
            PrimitiveFace::Front,
            PrimitiveFace::Back,
            PrimitiveFace::Bottom,
            PrimitiveFace::Top,
            PrimitiveFace::Side,
            PrimitiveFace::Surface,
        ]
        .into_iter()
        .map(role)
        .collect();

        assert_eq!(
            names,
            ["left", "right", "front", "back", "bottom", "top", "side", "surface"]
        );
    }

    #[test]
    fn op_names_match_their_serialized_form() {
        for op in [BodyOp::New, BodyOp::Join, BodyOp::Cut, BodyOp::Intersect] {
            assert_eq!(
                serde_json::to_value(op).expect("ops serialize"),
                json!(op_name(op))
            );
        }
    }

    #[test]
    fn manifest_lists_sizes_then_op_then_placement_then_body() {
        let manifest = manifest("Cylinder", "cylinder", &SIZES);

        let paths: Vec<&str> = manifest
            .fields
            .iter()
            .map(|field| field.path.as_str())
            .collect();
        assert_eq!(
            paths,
            [
                "radius",
                "height",
                "op",
                "plane",
                "position[0]",
                "position[1]",
                "position[2]",
                "body"
            ]
        );
    }

    #[test]
    fn manifest_sizes_are_essential_lengths() {
        let manifest = manifest("Cylinder", "cylinder", &SIZES);

        let radius = &manifest.fields[0];
        assert_eq!(radius.label, "Radius");
        assert!(radius.essential);
        assert_eq!(radius.unit, Some(FieldUnit::Length));
    }

    #[test]
    fn manifest_op_offers_the_four_body_ops() {
        let manifest = manifest("Cylinder", "cylinder", &SIZES);

        let op = &manifest.fields[2];
        assert!(op.essential);
        let options: Vec<(&str, &str)> = op
            .options
            .iter()
            .map(|option| (option.value.as_str(), option.label.as_str()))
            .collect();
        assert_eq!(
            options,
            [
                ("new", "New body"),
                ("join", "Join"),
                ("cut", "Cut"),
                ("intersect", "Intersect")
            ]
        );
    }

    #[test]
    fn manifest_body_is_a_body_selection_behind_more() {
        let manifest = manifest("Cylinder", "cylinder", &SIZES);

        let body = manifest.fields.last().expect("the manifest has fields");
        assert_eq!(body.label, "Body");
        assert!(!body.essential);
        assert_eq!(body.selection, Some(SelectionKind::Body));
    }

    #[test]
    fn manifest_is_version_one_in_create() {
        let manifest = manifest("Cylinder", "cylinder", &SIZES);

        assert_eq!(manifest.label, "Cylinder");
        assert_eq!(manifest.icon, "cylinder");
        assert_eq!(manifest.category, Category::Create);
        assert_eq!(manifest.version, 1);
    }

    #[test]
    fn dependencies_write_the_body_then_read_sizes_then_placement() {
        let radius = Expr::new("5");
        let height = Expr::new("10");
        let plane = PlaneRef::Datum {
            feature: id("f-dp1"),
        };
        let position = default_position();

        let dependencies = dependencies(
            &id("b-1"),
            &[(SIZES[0], &radius), (SIZES[1], &height)],
            PrimitivePlacement {
                plane: &plane,
                position: &position,
            },
        );

        assert_eq!(dependencies.bodies_written[0].id, id("b-1"));
        assert!(dependencies.bodies_read.is_empty());
        assert_eq!(dependencies.features[0].id, id("f-dp1"));
        let fields: Vec<&str> = dependencies
            .expressions
            .iter()
            .map(|usage| usage.field.as_str())
            .collect();
        assert_eq!(
            fields,
            ["radius", "height", "position[0]", "position[1]", "position[2]"]
        );
    }

    #[test]
    fn a_placement_face_on_the_target_body_keeps_it_written() {
        let radius = Expr::new("5");
        let plane: PlaneRef = serde_json::from_value(json!({
            "kind": "face",
            "body": "b-1",
            "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,30|a=200"}
        }))
        .expect("a face plane deserializes");
        let position = default_position();

        let dependencies = dependencies(
            &id("b-1"),
            &[(SIZES[0], &radius)],
            PrimitivePlacement {
                plane: &plane,
                position: &position,
            },
        );

        assert_eq!(dependencies.bodies_written.len(), 1);
        assert!(dependencies.bodies_read.is_empty());
    }

    #[test]
    fn combine_names_each_face_by_its_role() {
        let fixture = Fixture::new("f-box9").with_body("b-1", BodyState::empty());
        let primitive = make_box(&Plane::XY, 1.0, 2.0, 3.0).expect("the box builds");

        let output = combine(&fixture.context(), &id("b-1"), BodyOp::New, primitive)
            .expect("op new on an empty body builds");

        let OutputData::Bodies(updates) = output.data else {
            panic!("a primitive outputs body updates");
        };
        assert_eq!(
            face_names(&updates[0].naming),
            [
                "f-box9/back",
                "f-box9/bottom",
                "f-box9/front",
                "f-box9/left",
                "f-box9/right",
                "f-box9/top"
            ]
        );
    }

    #[test]
    fn combine_applies_the_op_to_the_body() {
        let fixture = Fixture::new("f-sph9").with_body("b-1", BodyState::empty());
        let primitive = make_sphere(&Plane::XY, 2.0).expect("the sphere builds");

        let output = combine(&fixture.context(), &id("b-1"), BodyOp::New, primitive)
            .expect("op new on an empty body builds");

        let OutputData::Bodies(updates) = output.data else {
            panic!("a primitive outputs body updates");
        };
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].body, id("b-1"));
        assert_eq!(updates[0].change, BodyChange::New);
        assert!(output.warnings.is_empty());
    }

    #[test]
    fn combine_reports_the_body_rules() {
        let fixture = Fixture::new("f-sph9").with_body("b-1", BodyState::empty());
        let primitive = make_sphere(&Plane::XY, 2.0).expect("the sphere builds");

        let error = combine(&fixture.context(), &id("b-1"), BodyOp::Join, primitive)
            .expect_err("join needs a solid");

        assert_eq!(error.code(), "body_empty");
    }
}
