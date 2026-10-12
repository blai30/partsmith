//! Pieces the built-in features share: manifest hints, range checks, body states, tool naming and
//! Python arguments.

use partsmith_model::{
    Category, Expr, FeatureManifest, FieldHint, FieldUnit, Id, OptionHint, SelectionKind,
};
use partsmith_naming::{ElementName, FaceSource, Generator, Reference, roles};
use partsmith_occt_sys::{Primitive, PrimitiveFace, Shape, SweepFace, SweepResult};

use crate::context::{BodyState, BuildContext};
use crate::error::{Bound, FeatureError, FeatureWarning};
use crate::naming::{name_shape, naming};
use crate::output::{BodyChange, BodyUpdate, FeatureOutput, OutputData, Tool, apply_op};
use crate::params::{BodyOp, CopyOp, ProfileRef};
use crate::python::{PythonCall, PythonNames, PythonValue};

/// The options of a `BodyOp` field, with their labels.
pub(crate) const BODY_OP_OPTIONS: [(&str, &str); 4] = [
    ("new", "New body"),
    ("join", "Join"),
    ("cut", "Cut"),
    ("intersect", "Intersect"),
];

/// The options of a `CopyOp` field, with their labels.
pub(crate) const COPY_OP_OPTIONS: [(&str, &str); 2] = [("join", "Join"), ("new", "New body")];

/// For `skip_serializing_if` on flags that default to false.
pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}

/// A version 1 manifest (features.md section 3.13).
pub(crate) fn manifest(
    label: &str,
    category: Category,
    icon: &str,
    fields: Vec<FieldHint>,
) -> FeatureManifest {
    FeatureManifest {
        label: label.to_string(),
        category,
        icon: icon.to_string(),
        version: 1,
        fields,
    }
}

/// A hint with only a path and a label.
pub(crate) fn hint(path: &str, label: &str) -> FieldHint {
    FieldHint {
        path: path.to_string(),
        label: label.to_string(),
        essential: false,
        unit: None,
        selection: None,
        options: Vec::new(),
    }
}

/// The same hint, shown without the "More" toggle.
pub(crate) fn essential(hint: FieldHint) -> FieldHint {
    FieldHint {
        essential: true,
        ..hint
    }
}

/// A hint for an expression field of `unit`.
pub(crate) fn unit_hint(path: &str, label: &str, unit: FieldUnit) -> FieldHint {
    FieldHint {
        unit: Some(unit),
        ..hint(path, label)
    }
}

/// A hint for a field the user fills by selecting `selection`.
pub(crate) fn selection_hint(path: &str, label: &str, selection: SelectionKind) -> FieldHint {
    FieldHint {
        selection: Some(selection),
        ..hint(path, label)
    }
}

/// A hint for an enumeration or tag field with `(value, label)` options.
pub(crate) fn options_hint(path: &str, label: &str, options: &[(&str, &str)]) -> FieldHint {
    FieldHint {
        options: options
            .iter()
            .map(|(value, label)| OptionHint {
                value: value.to_string(),
                label: label.to_string(),
            })
            .collect(),
        ..hint(path, label)
    }
}

/// The essential `op` hint of a solid feature.
pub(crate) fn body_op_hint() -> FieldHint {
    essential(options_hint("op", "Operation", &BODY_OP_OPTIONS))
}

/// The `body` hint of a feature that names its body in a `with <part>:` block.
pub(crate) fn body_hint() -> FieldHint {
    selection_hint("body", "Body", SelectionKind::Body)
}

/// True when `value` satisfies `bound`.
fn satisfies(value: f64, bound: Bound) -> bool {
    match bound {
        Bound::GreaterThan(limit) => value > limit,
        Bound::AtLeast(limit) => value >= limit,
        Bound::LessThan(limit) => value < limit,
        Bound::AtMost(limit) => value <= limit,
    }
}

/// `value` when it satisfies `bound`, else `OutOfRange` at `field` (features.md section 4.1).
/// `bound_name` says what the bound is when it comes from another value.
pub(crate) fn check_range(
    context: &BuildContext<'_>,
    field: &str,
    kind: FieldUnit,
    value: f64,
    bound: Bound,
    bound_name: Option<&str>,
) -> Result<f64, FeatureError> {
    if satisfies(value, bound) {
        Ok(value)
    } else {
        Err(FeatureError::OutOfRange {
            field: context.field(field),
            kind,
            value,
            bound,
            bound_name: bound_name.map(str::to_string),
        })
    }
}

/// The declared length at `field`, which must be greater than 0.
pub(crate) fn positive_length(
    context: &BuildContext<'_>,
    field: &str,
    expr: &Expr,
) -> Result<f64, FeatureError> {
    let value = context.eval_length(field, expr)?;
    check_range(
        context,
        field,
        FieldUnit::Length,
        value,
        Bound::GreaterThan(0.0),
        None,
    )
}

/// The state and shape of a declared body that holds a solid. An empty body is `BodyEmpty` for a
/// feature that works on an existing solid (`use_new: false`).
pub(crate) fn solid_body<'a>(
    context: &BuildContext<'a>,
    body: &Id,
) -> Result<(&'a BodyState, &'a Shape), FeatureError> {
    let state = context.body(body)?;
    match state.shape() {
        Some(shape) => Ok((state, shape)),
        None => Err(FeatureError::BodyEmpty {
            body: body.clone(),
            use_new: false,
        }),
    }
}

/// An update that keeps a body's state exactly: the same shape and naming map.
pub(crate) fn unchanged(body: &Id, state: &BodyState, change: BodyChange) -> BodyUpdate {
    BodyUpdate {
        body: body.clone(),
        shape: state.shape().cloned(),
        naming: state.naming().clone(),
        change,
    }
}

/// A body-updating output.
pub(crate) fn bodies(updates: Vec<BodyUpdate>, warnings: Vec<FeatureWarning>) -> FeatureOutput {
    FeatureOutput {
        data: OutputData::Bodies(updates),
        warnings,
    }
}

/// Applies a solid feature's tool to its body with the shared body semantics and outputs the
/// update, after the warnings raised so far.
pub(crate) fn apply_tool(
    context: &BuildContext<'_>,
    body: &Id,
    op: BodyOp,
    tool: Tool,
    mut warnings: Vec<FeatureWarning>,
) -> Result<FeatureOutput, FeatureError> {
    let (update, op_warnings) = apply_op(context, body, op, vec![tool])?;
    warnings.extend(op_warnings);
    Ok(bodies(vec![update], warnings))
}

/// A new face name `<feature>/<role>`, or `<feature>/<role>/<generator>` (naming.md section 3.2).
pub(crate) fn face_name(
    feature: &Id,
    role: &str,
    generator: Option<Generator<'_>>,
) -> Result<ElementName, FeatureError> {
    ElementName::generated(feature, role, generator).map_err(naming)
}

/// The tool of a primitive: each face named `<feature>/<role>` from its kernel role (naming.md
/// section 3.6.4).
pub(crate) fn primitive_tool(feature: &Id, primitive: Primitive) -> Result<Tool, FeatureError> {
    let faces = primitive
        .faces
        .iter()
        .map(|face| {
            let role = match face {
                PrimitiveFace::Bottom => roles::BOTTOM,
                PrimitiveFace::Top => roles::TOP,
                PrimitiveFace::Front => roles::FRONT,
                PrimitiveFace::Back => roles::BACK,
                PrimitiveFace::Left => roles::LEFT,
                PrimitiveFace::Right => roles::RIGHT,
                PrimitiveFace::Side => roles::SIDE,
                PrimitiveFace::Surface => roles::SURFACE,
            };
            face_name(feature, role, None).map(FaceSource::New)
        })
        .collect::<Result<Vec<_>, FeatureError>>()?;
    let naming = name_shape(&primitive.shape, faces, &[])?;
    Ok(Tool {
        shape: primitive.shape,
        naming,
    })
}

/// The tool of an extrusion or revolution of a profile: lateral faces named
/// `<feature>/lateral/<entity>` after the sketch entity of their curve tag (tag `i` is
/// `entities[i]`), caps `<feature>/start-cap` and `<feature>/end-cap` (naming.md sections 3.6.2
/// and 3.6.3).
pub(crate) fn sweep_tool(
    feature: &Id,
    sweep: SweepResult,
    entities: &[Id],
) -> Result<Tool, FeatureError> {
    let faces = sweep
        .faces
        .iter()
        .map(|face| {
            let name = match face {
                SweepFace::Lateral { tag } => {
                    let entity = entities.get(*tag as usize).ok_or_else(|| {
                        FeatureError::internal(format!("the profile has no curve tag {tag}"))
                    })?;
                    face_name(
                        feature,
                        roles::LATERAL,
                        Some(Generator::Sketch(entity.as_str())),
                    )
                }
                SweepFace::StartCap => face_name(feature, roles::START_CAP, None),
                SweepFace::EndCap => face_name(feature, roles::END_CAP, None),
            }?;
            Ok(FaceSource::New(name))
        })
        .collect::<Result<Vec<_>, FeatureError>>()?;
    let naming = name_shape(&sweep.shape, faces, &[])?;
    Ok(Tool {
        shape: sweep.shape,
        naming,
    })
}

/// The snake_case name of a body op, as stored and as Python writes it.
pub(crate) fn body_op_name(op: BodyOp) -> &'static str {
    match op {
        BodyOp::New => "new",
        BodyOp::Join => "join",
        BodyOp::Cut => "cut",
        BodyOp::Intersect => "intersect",
    }
}

/// `op="<op>"` when `op` differs from the op a solid feature function uses for `body` when it is
/// omitted (features.md section 3.11).
pub(crate) fn op_keyword(
    names: &dyn PythonNames,
    body: &Id,
    op: BodyOp,
) -> Option<(String, PythonValue)> {
    (op != names.default_op(body)).then(|| {
        (
            "op".to_string(),
            PythonValue::Str(body_op_name(op).to_string()),
        )
    })
}

/// A keyword set to `True`.
pub(crate) fn flag(name: &str) -> (String, PythonValue) {
    (name.to_string(), PythonValue::Bool(true))
}

/// One value alone, several as a list (python-api.md section 3.8 rule 9).
pub(crate) fn one_or_list(mut values: Vec<PythonValue>) -> PythonValue {
    if values.len() == 1 {
        values.remove(0)
    } else {
        PythonValue::List(values)
    }
}

/// Stored references as Python: one `Ref(...)` alone, several as a list.
pub(crate) fn references(references: &[Reference]) -> PythonValue {
    one_or_list(
        references
            .iter()
            .map(|reference| PythonValue::reference(reference, None))
            .collect(),
    )
}

/// The variable of a body's part. None when the script has none.
pub(crate) fn part(names: &dyn PythonNames, body: &Id) -> Option<PythonValue> {
    names.body(body).map(PythonValue::Name)
}

/// `into=<target part>` of a copy feature with op new; nothing for op join. None when the target's
/// part has no variable or op new has no target.
pub(crate) fn into_keyword(
    names: &dyn PythonNames,
    op: CopyOp,
    target: Option<&Id>,
) -> Option<Vec<(String, PythonValue)>> {
    match op {
        CopyOp::Join => Some(Vec::new()),
        CopyOp::New => Some(vec![("into".to_string(), part(names, target?)?)]),
    }
}

/// The Python form of a profile (features.md section 3.11): the sketch's variable, and the
/// keyword `regions=[Region(...), ...]` only when the stored regions are not every region of the
/// sketch in solved order. None when the sketch has no variable or did not solve.
pub(crate) fn profile_arguments(
    profile: &ProfileRef,
    names: &dyn PythonNames,
) -> Option<(PythonValue, Option<(String, PythonValue)>)> {
    let sketch = PythonValue::Name(names.feature(&profile.sketch)?);
    let solved = names.regions(&profile.sketch)?;
    let regions = (solved != profile.regions).then(|| {
        (
            "regions".to_string(),
            PythonValue::List(profile.regions.iter().map(PythonValue::region).collect()),
        )
    });
    Some((sketch, regions))
}

/// A call of `function` with these positional and keyword arguments.
pub(crate) fn call(
    function: &str,
    positional: Vec<PythonValue>,
    keywords: Vec<(String, PythonValue)>,
) -> PythonCall {
    PythonCall {
        function: function.to_string(),
        positional,
        keywords,
        extra: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use partsmith_occt_sys::{Plane, make_cylinder};
    use partsmith_sketch::RegionRef;

    use super::*;
    use crate::test_support::{Fixture, ScriptNames, id};

    #[test]
    fn bounds_are_strict_or_inclusive() {
        assert!(satisfies(1.0, Bound::GreaterThan(0.0)));
        assert!(!satisfies(0.0, Bound::GreaterThan(0.0)));
        assert!(satisfies(0.0, Bound::AtLeast(0.0)));
        assert!(!satisfies(5.0, Bound::LessThan(5.0)));
        assert!(satisfies(5.0, Bound::AtMost(5.0)));
    }

    #[test]
    fn a_value_out_of_range_names_its_field_and_bound() {
        let fixture = Fixture::new("f-test1");

        let error = check_range(
            &fixture.context(),
            "hole_type.depth",
            FieldUnit::Length,
            6.0,
            Bound::LessThan(5.0),
            Some("the hole depth"),
        )
        .expect_err("6 is not less than 5");

        assert_eq!(
            error.to_string(),
            "hole_type.depth must be less than the hole depth (5 mm), but it is 6 mm. Use a value less than 5 mm."
        );
    }

    #[test]
    fn an_empty_body_has_nothing_to_work_on() {
        let fixture = Fixture::new("f-test1").with_body("b-1", BodyState::empty());

        let result = solid_body(&fixture.context(), &id("b-1"));

        assert_eq!(
            result.map(|_| ()),
            Err(FeatureError::BodyEmpty {
                body: id("b-1"),
                use_new: false
            })
        );
    }

    #[test]
    fn primitive_faces_are_named_by_role() {
        let primitive = make_cylinder(&Plane::XY, 5.0, 10.0).expect("the cylinder builds");

        let tool = primitive_tool(&id("f-cyl1"), primitive).expect("the faces are named");

        let mut names: Vec<String> = tool
            .naming
            .names(partsmith_naming::ElementKind::Face)
            .map(|(_, name)| name.to_string())
            .collect();
        names.sort();
        assert_eq!(names, ["f-cyl1/bottom", "f-cyl1/side", "f-cyl1/top"]);
    }

    #[test]
    fn the_default_op_is_not_written() {
        let names = ScriptNames::default();

        assert_eq!(op_keyword(&names, &id("b-1"), BodyOp::New), None);
        assert_eq!(
            op_keyword(&names, &id("b-1"), BodyOp::Cut),
            Some(("op".to_string(), PythonValue::Str("cut".to_string())))
        );
    }

    #[test]
    fn one_value_stands_alone_and_several_make_a_list() {
        let one = one_or_list(vec![PythonValue::Number("1".to_string())]);
        let two = one_or_list(vec![
            PythonValue::Number("1".to_string()),
            PythonValue::Number("2".to_string()),
        ]);

        assert_eq!(one.render(), "1");
        assert_eq!(two.render(), "[1, 2]");
    }

    #[test]
    fn every_region_in_solved_order_is_not_written() {
        let region: RegionRef =
            serde_json::from_value(serde_json::json!({"key": "e-1", "fp": "c=0,0|a=1"}))
                .expect("a region ref");
        let mut names = ScriptNames::default();
        names.regions.insert(id("f-sk1"), vec![region.clone()]);
        let profile = ProfileRef {
            sketch: id("f-sk1"),
            regions: vec![region],
        };

        let (sketch, regions) = profile_arguments(&profile, &names).expect("the sketch solved");

        assert_eq!(sketch.render(), "sketch_1");
        assert_eq!(regions, None);
    }

    #[test]
    fn a_sketch_that_did_not_solve_has_no_dedicated_profile() {
        let names = ScriptNames::default();
        let profile = ProfileRef {
            sketch: id("f-sk9"),
            regions: Vec::new(),
        };

        assert_eq!(profile_arguments(&profile, &names), None);
    }
}
