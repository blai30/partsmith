//! Pieces the built-in features share: manifest hints, range checks, body states, the reach of
//! through-all extents, tool naming and Python arguments.

use partsmith_mesh::BoundingBox;
use partsmith_model::{
    Category, Expr, FeatureManifest, FieldHint, FieldUnit, Id, LINEAR_TOLERANCE, OptionHint,
    SelectionKind,
};
use partsmith_naming::{
    ElementKind, ElementName, FaceSource, Generator, Motion, NamingMap, Reference, Successions,
    roles,
};
use partsmith_occt_sys::{
    KernelError, PlanarProfile, Point, Primitive, PrimitiveFace, Shape, SweepFace, SweepResult,
    Transform, Vector, compound,
};
use partsmith_sketch::{RegionRef, Succession};

use crate::context::{BodyState, BuildContext, ResolvedProfile};
use crate::error::{Bound, FeatureError, FeatureWarning, FieldRef, Noun};
use crate::output::{BodyChange, BodyUpdate, FeatureOutput, OutputData, Tool, apply_op};
use crate::params::{BodyOp, CopyOp, ProfileRef};
use crate::python::{PythonCall, PythonNames, PythonValue};
use crate::{Named, name_shape};

/// The options of a `BodyOp` field, with their labels.
pub(crate) const BODY_OP_OPTIONS: [(&str, &str); 4] = [
    ("new", "New body"),
    ("join", "Join"),
    ("cut", "Cut"),
    ("intersect", "Intersect"),
];

/// How far past the farthest corner of the body's bounding box a through-all extrusion or hole
/// goes, in millimeters (decision D21).
pub(crate) const THROUGH_ALL_MARGIN: f64 = 1.0;

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

/// The `target` hint of a feature that puts its result into another body.
pub(crate) fn target_hint() -> FieldHint {
    selection_hint("target", "Target body", SelectionKind::Body)
}

/// The `body` hint of a feature that names its body in a `with <part>:` block.
pub(crate) fn body_hint() -> FieldHint {
    selection_hint("body", "Body", SelectionKind::Body)
}

/// `EmptySelection` of `noun` at `path`, labeled from `manifest`, when `selection` is empty. For
/// `Feature::validate`, which has no context; the manifest is only built for the error.
pub(crate) fn require_selection<T>(
    selection: &[T],
    manifest: fn() -> FeatureManifest,
    path: &str,
    noun: Noun,
) -> Result<(), FeatureError> {
    if selection.is_empty() {
        return Err(FeatureError::EmptySelection {
            field: FieldRef::new(path, &manifest()),
            noun,
        });
    }
    Ok(())
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

/// `OutOfRange` at `field` for `value`, with the `bound` its message names (features.md section
/// 4.1). `bound_name` says what the bound is when it comes from another value.
pub(crate) fn out_of_range(
    context: &BuildContext<'_>,
    field: &str,
    kind: FieldUnit,
    value: f64,
    bound: Bound,
    bound_name: Option<&str>,
) -> FeatureError {
    FeatureError::OutOfRange {
        field: context.field(field),
        kind,
        value,
        bound,
        bound_name: bound_name.map(str::to_string),
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
        Err(out_of_range(context, field, kind, value, bound, bound_name))
    }
}

/// The declared length at `field` when it satisfies `bound`, else `OutOfRange` (features.md section
/// 4.1).
pub(crate) fn length_in_range(
    context: &BuildContext<'_>,
    field: &str,
    expr: &Expr,
    bound: Bound,
) -> Result<f64, FeatureError> {
    let value = context.eval_length(field, expr)?;
    check_range(context, field, FieldUnit::Length, value, bound, None)
}

/// The declared angle at `field` when it satisfies `bound`, else `OutOfRange` (features.md section
/// 4.1).
pub(crate) fn angle_in_range(
    context: &BuildContext<'_>,
    field: &str,
    expr: &Expr,
    bound: Bound,
) -> Result<f64, FeatureError> {
    let value = context.eval_angle(field, expr)?;
    check_range(context, field, FieldUnit::Angle, value, bound, None)
}

/// The declared lengths at three fields, such as the coordinates of a position, in field order.
pub(crate) fn three_lengths(
    context: &BuildContext<'_>,
    fields: [&str; 3],
    exprs: &[Expr; 3],
) -> Result<[f64; 3], FeatureError> {
    let mut lengths = [0.0; 3];
    for ((length, field), expr) in lengths.iter_mut().zip(fields).zip(exprs) {
        *length = context.eval_length(field, expr)?;
    }
    Ok(lengths)
}

/// True when `length` counts as greater than 0: at least `LINEAR_TOLERANCE`, below which the
/// kernel builds nothing (the positive length rule of features.md section 3.4).
pub(crate) fn is_positive_length(length: f64) -> bool {
    length >= LINEAR_TOLERANCE
}

/// The declared length at `field`, which must be greater than 0. A value below `LINEAR_TOLERANCE`
/// is `OutOfRange` (`GreaterThan(0)`) like 0, so it never reaches the kernel (features.md section
/// 3.4).
pub(crate) fn positive_length(
    context: &BuildContext<'_>,
    field: &str,
    expr: &Expr,
) -> Result<f64, FeatureError> {
    let value = context.eval_length(field, expr)?;
    if is_positive_length(value) {
        Ok(value)
    } else {
        Err(out_of_range(
            context,
            field,
            FieldUnit::Length,
            value,
            Bound::GreaterThan(0.0),
            None,
        ))
    }
}

/// `Canceled` once the run has been canceled. Long builds check it before each kernel step.
pub(crate) fn check_canceled(context: &BuildContext<'_>) -> Result<(), FeatureError> {
    if context.is_canceled() {
        Err(FeatureError::Canceled)
    } else {
        Ok(())
    }
}

/// The state and shape of a declared body, named at `field`, that holds a solid. An empty body is
/// `BodyEmpty` for a feature that works on an existing solid (`use_new: false`).
pub(crate) fn solid_body<'a>(
    context: &BuildContext<'a>,
    field: &str,
    body: &Id,
) -> Result<(&'a BodyState, &'a Shape), FeatureError> {
    let state = context.body(body)?;
    match state.shape() {
        Some(shape) => Ok((state, shape)),
        None => Err(FeatureError::BodyEmpty {
            field: context.field(field),
            body: body.clone(),
            use_new: false,
        }),
    }
}

/// The face names of what a feature left in its body: those of its first body update. None for an
/// output without body updates, and for one with a `NoEffect` warning, which tells why none of the
/// feature's faces are there, so the feature records no face absent.
pub(crate) fn result_faces(output: &FeatureOutput) -> Option<Vec<&ElementName>> {
    let no_effect = output
        .warnings
        .iter()
        .any(|warning| matches!(warning, FeatureWarning::NoEffect { .. }));
    let OutputData::Bodies(updates) = &output.data else {
        return None;
    };
    let update = updates.first().filter(|_| !no_effect)?;
    Some(
        update
            .naming
            .names(ElementKind::Face)
            .map(|(_, name)| name)
            .collect(),
    )
}

/// True when `faces` has `face` or a piece split from it.
pub(crate) fn has_face(faces: &[&ElementName], face: &ElementName) -> bool {
    faces
        .iter()
        .any(|name| *name == face || name.is_split_descendant_of(face))
}

/// An update that keeps a body's state exactly: the same shape and naming map, with what has been
/// measured on it.
pub(crate) fn unchanged(body: &Id, state: &BodyState, change: BodyChange) -> BodyUpdate {
    BodyUpdate {
        body: body.clone(),
        shape: state.shape().cloned(),
        naming: state.naming().clone(),
        change,
        measured: state.measurements(),
    }
}

/// The kernel transform `transform` as a naming motion: its linear part, then its translation. Copies and moves name
/// their faces with it (features.md section 3.9).
pub(crate) fn naming_motion(transform: &Transform) -> Motion {
    let [first, second, third] = transform.matrix;
    Motion::new(
        [
            [first[0], first[1], first[2]],
            [second[0], second[1], second[2]],
            [third[0], third[1], third[2]],
        ],
        [first[3], second[3], third[3]],
    )
}

/// Names `shape`, made by a fillet, chamfer, shell or split body from the body whose naming map is `input`, with
/// `input` inherited and no successions. Its new faces take the base motion of the body's map (features.md section
/// 3.9): the motion of a name the map never saw, the feature's own `<feature>/face`.
pub(crate) fn name_on_body(
    feature: &Id,
    shape: &Shape,
    faces: Vec<FaceSource>,
    input: &NamingMap,
) -> Result<Named, FeatureError> {
    name_shape(
        shape,
        faces,
        &[input],
        &Successions::default(),
        &input.motion(&face_name(feature, roles::FACE, None)?),
    )
}

/// A body-updating output that records no absent face.
pub(crate) fn bodies(updates: Vec<BodyUpdate>, warnings: Vec<FeatureWarning>) -> FeatureOutput {
    FeatureOutput::new(OutputData::Bodies(updates), warnings)
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

/// One solid as it is, several grouped in order into one compound (kernel `compound`).
pub(crate) fn single_or_compound(mut solids: Vec<Shape>) -> Result<Shape, FeatureError> {
    if solids.len() == 1 {
        Ok(solids.remove(0))
    } else {
        Ok(compound(&solids)?.shape)
    }
}

/// The largest signed distance `(corner - origin) · direction` over the eight corners of `bounds`,
/// a body's tight bounding box: how far a through-all extrusion or hole must reach.
pub(crate) fn farthest_corner(bounds: &BoundingBox, origin: Point, direction: Vector) -> f64 {
    let mut farthest = f64::NEG_INFINITY;
    for corner in 0..8 {
        let [x, y, z] = [0, 1, 2].map(|axis| {
            if corner & (1 << axis) == 0 {
                bounds.min[axis]
            } else {
                bounds.max[axis]
            }
        });
        let offset = Vector::new(x - origin.x, y - origin.y, z - origin.z);
        farthest = farthest.max(offset.dot(direction));
    }
    farthest
}

/// A new face name `<feature>/<role>`, or `<feature>/<role>/<generator>` (naming.md section 3.2).
pub(crate) fn face_name(
    feature: &Id,
    role: &str,
    generator: Option<Generator<'_>>,
) -> Result<ElementName, FeatureError> {
    ElementName::generated(feature, role, generator).map_err(FeatureError::from)
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
    let naming = name_shape(
        &primitive.shape,
        faces,
        &[],
        &Successions::default(),
        &Motion::IDENTITY,
    )?
    .naming;
    Ok(Tool {
        shape: primitive.shape,
        naming,
    })
}

/// The tool of an extrusion or revolution of a profile: lateral faces named
/// `<feature>/lateral/<entity>` after the sketch entity of their curve tag (tag `i` is
/// `entities[i]`), caps `<feature>/start-cap` and `<feature>/end-cap` (naming.md sections 3.6.2
/// and 3.6.3), with the lateral successions and the sketch's motion (features.md section 3.9).
fn sweep_tool(
    feature: &Id,
    sweep: SweepResult,
    entities: &[Id],
    successions: &Successions,
    motion: &Motion,
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
                    lateral_name(feature, entity)
                }
                SweepFace::StartCap => face_name(feature, roles::START_CAP, None),
                SweepFace::EndCap => face_name(feature, roles::END_CAP, None),
            }?;
            Ok(FaceSource::New(name))
        })
        .collect::<Result<Vec<_>, FeatureError>>()?;
    let naming = name_shape(&sweep.shape, faces, &[], successions, motion)?.naming;
    Ok(Tool {
        shape: sweep.shape,
        naming,
    })
}

/// The lateral face name `<feature>/lateral/<entity>` (naming.md section 3.6.2).
fn lateral_name(feature: &Id, entity: &Id) -> Result<ElementName, FeatureError> {
    face_name(
        feature,
        roles::LATERAL,
        Some(Generator::Sketch(entity.as_str())),
    )
}

/// The successions of a profile's boundary matches as successions of the feature's lateral faces:
/// `<feature>/lateral/<m> -> [<feature>/lateral/<n1>, ...]` in the order of `Succession::now`
/// (features.md section 3.9). An entity listed twice keeps its first list.
fn lateral_successions(
    feature: &Id,
    successions: &[Succession],
) -> Result<Successions, FeatureError> {
    let mut lateral = Successions::default();
    for succession in successions {
        let now = succession
            .now
            .iter()
            .map(|entity| lateral_name(feature, entity))
            .collect::<Result<Vec<_>, FeatureError>>()?;
        lateral
            .0
            .entry(lateral_name(feature, &succession.entity)?)
            .or_insert(now);
    }
    Ok(lateral)
}

/// Sweeps the faces of a resolved `profile` with the kernel `sweep` and names the tool by
/// `sweep_tool`, with the profile's successions and its sketch's motion: the shared middle of
/// extrude and revolve. A canceled run stops before the kernel sweep.
pub(crate) fn swept_tool(
    context: &BuildContext<'_>,
    profile: &ResolvedProfile<'_>,
    sweep: impl FnOnce(&PlanarProfile) -> Result<SweepResult, KernelError>,
) -> Result<Tool, FeatureError> {
    let faces = profile.faces()?;
    check_canceled(context)?;
    let swept = sweep(&faces.profile)?;
    let feature = context.feature();
    let successions = lateral_successions(feature, &profile.successions)?;
    sweep_tool(
        feature,
        swept,
        &faces.entities,
        &successions,
        &profile.sketch.motion,
    )
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
/// keyword `regions=[Region(...), ...]` only when the stored regions differ from the references of
/// every region of the sketch in solved order, outlines included. None when the sketch has no
/// variable or did not solve.
pub(crate) fn profile_arguments(
    profile: &ProfileRef,
    names: &dyn PythonNames,
) -> Option<(PythonValue, Option<(String, PythonValue)>)> {
    let sketch = PythonValue::Name(names.feature(&profile.sketch)?);
    let solved = names.regions(&profile.sketch)?;
    // A script without `regions=` stores the solved references, so a stored region without the
    // outline its solved region has (written before format version 2) is written out.
    let regions = (!stores_the_same(&profile.regions, &solved)).then(|| {
        (
            "regions".to_string(),
            PythonValue::List(profile.regions.iter().map(PythonValue::region).collect()),
        )
    });
    Some((sketch, regions))
}

/// Whether two lists of region references store the same text: keys, outlines and the
/// fingerprints' text. A solved region's fingerprint is exact, while a stored one holds the rounded
/// text (sketch.md section 3.13.6), so comparing the values would tell a reference from the region
/// it was picked from whenever its area or centroid is not a round number.
fn stores_the_same(stored: &[RegionRef], solved: &[RegionRef]) -> bool {
    stored.len() == solved.len()
        && stored.iter().zip(solved).all(|(stored, solved)| {
            stored.key == solved.key
                && stored.outline == solved.outline
                && stored.fp.to_string() == solved.fp.to_string()
        })
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
    use partsmith_occt_sys::{Plane, make_box, make_cylinder};
    use partsmith_sketch::RegionFingerprint;

    use super::*;
    use crate::test_support::{Fixture, ScriptNames, id};

    /// The farthest corner of the box x 0..10, y 0..20, z 0..30 from `origin` along `direction`.
    fn farthest_box_corner(origin: [f64; 3], direction: [f64; 3]) -> f64 {
        let shape = make_box(&Plane::XY, 10.0, 20.0, 30.0)
            .expect("the box builds")
            .shape;
        let [x, y, z] = origin;
        let [dx, dy, dz] = direction;
        let bounds = shape.bounding_box().expect("the box has bounds");
        farthest_corner(&bounds, Point::new(x, y, z), Vector::new(dx, dy, dz))
    }

    #[test]
    fn a_length_is_positive_from_the_tolerance_on() {
        assert!(is_positive_length(LINEAR_TOLERANCE));
        assert!(is_positive_length(1.0));
        assert!(!is_positive_length(LINEAR_TOLERANCE / 2.0));
        assert!(!is_positive_length(0.0));
        assert!(!is_positive_length(-1.0));
    }

    #[test]
    fn the_farthest_corner_is_measured_along_the_direction() {
        let up = farthest_box_corner([0.0, 0.0, 5.0], [0.0, 0.0, 1.0]);
        let down = farthest_box_corner([0.0, 0.0, 5.0], [0.0, 0.0, -1.0]);

        assert!((up - 25.0).abs() < 1e-9, "{up}");
        assert!((down - 5.0).abs() < 1e-9, "{down}");
    }

    #[test]
    fn a_body_behind_the_plane_has_no_positive_reach() {
        let reach = farthest_box_corner([0.0, 0.0, 40.0], [0.0, 0.0, 1.0]);

        assert!((reach + 10.0).abs() < 1e-9, "{reach}");
    }

    #[test]
    fn a_tilted_direction_measures_the_projection() {
        let diagonal = std::f64::consts::FRAC_1_SQRT_2;

        let reach = farthest_box_corner([0.0, 0.0, 0.0], [diagonal, diagonal, 0.0]);

        assert!((reach - 30.0 * diagonal).abs() < 1e-9, "{reach}");
    }

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
        let context = fixture.context();

        let result = solid_body(&context, "body", &id("b-1"));

        assert_eq!(
            result.map(|_| ()),
            Err(FeatureError::BodyEmpty {
                field: context.field("body"),
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
    fn every_region_whose_fingerprint_is_not_round_is_not_written() {
        // A circle of radius 10: the solved fingerprint is exact, the stored one rounded.
        let solved = RegionRef {
            key: "e-5".parse().expect("a region key"),
            fp: RegionFingerprint {
                area: std::f64::consts::PI * 100.0,
                centroid: [1e-17, -1e-17],
            },
            outline: Some("e-5".parse().expect("a region loop")),
        };
        let stored: RegionRef =
            serde_json::from_value(serde_json::to_value(&solved).expect("JSON"))
                .expect("a region ref");
        let mut names = ScriptNames::default();
        names.regions.insert(id("f-sk1"), vec![solved]);
        let profile = ProfileRef {
            sketch: id("f-sk1"),
            regions: vec![stored],
        };

        let (_, regions) = profile_arguments(&profile, &names).expect("the sketch solved");

        assert_eq!(regions, None);
    }

    #[test]
    fn a_stored_region_without_its_outline_is_written() {
        let solved: RegionRef = serde_json::from_value(
            serde_json::json!({"key": "e-1", "fp": "c=0,0|a=1", "outline": "e-1"}),
        )
        .expect("a region ref");
        let stored = RegionRef {
            outline: None,
            ..solved.clone()
        };
        let mut names = ScriptNames::default();
        names.regions.insert(id("f-sk1"), vec![solved]);
        let profile = ProfileRef {
            sketch: id("f-sk1"),
            regions: vec![stored],
        };

        let (_, regions) = profile_arguments(&profile, &names).expect("the sketch solved");

        let (keyword, value) = regions.expect("the stored regions differ from the solved ones");
        assert_eq!(keyword, "regions");
        assert_eq!(value.render(), r#"[Region("e-1", "c=0,0|a=1")]"#);
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
