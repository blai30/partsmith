//! What the body features `boolean`, `mirror`, `linear_pattern`, `circular_pattern`, `move_body`,
//! `split_body` and `import` share: manifest hints, body checks and outputs, and the copy
//! features' rules for `op` and `target`, their transformed instances and how the copies are
//! combined (features.md sections 3.5 and 3.11, mirror.md section 3).

use partsmith_model::{Category, FeatureManifest, FieldHint, FieldUnit, Id, OptionHint};
use partsmith_naming::{Instance, NamingMap};
use partsmith_occt_sys::{Shape, Transform, transform};

use crate::context::{BodyState, BuildContext};
use crate::deps::Dependencies;
use crate::error::{Bound, FeatureError, FeatureWarning, FieldRef};
use crate::naming::{kernel, naming};
use crate::output::{BodyUpdate, FeatureOutput, OutputData, Tool, apply_op};
use crate::params::{BodyOp, CopyOp};
use crate::python::{PythonNames, PythonValue};

/// Most instances a pattern makes, the original included (linear_pattern.md and
/// circular_pattern.md section 3.3).
pub(super) const INSTANCE_LIMIT: u64 = 10_000;

/// The values of a copy feature's `op` with their labels.
pub(super) const COPY_OP_OPTIONS: [(&str, &str); 2] = [("join", "Join"), ("new", "New body")];

/// A version 1 manifest (features.md section 3.13).
pub(super) fn manifest(
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

/// The hint of the field at `path` labeled `label`, shown without the "More" toggle when
/// `essential`. Callers set its unit, selection or options with struct update syntax.
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

/// Option hints of `(value, label)` pairs, in display order.
pub(super) fn options(values: &[(&str, &str)]) -> Vec<OptionHint> {
    values
        .iter()
        .map(|(value, label)| OptionHint {
            value: value.to_string(),
            label: label.to_string(),
        })
        .collect()
}

/// For `skip_serializing_if` on flags that default to false.
pub(super) fn is_false(value: &bool) -> bool {
    !*value
}

/// The `OutOfRange` error of `value` at `field`.
pub(super) fn out_of_range(
    context: &BuildContext<'_>,
    field: &str,
    kind: FieldUnit,
    value: f64,
    bound: Bound,
) -> FeatureError {
    FeatureError::OutOfRange {
        field: context.field(field),
        kind,
        value,
        bound,
        bound_name: None,
    }
}

/// The state and shape of a declared body that must hold a solid. An empty body is `BodyEmpty`
/// for a feature that works on an existing solid (`use_new: false`).
pub(super) fn solid<'a>(
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

/// An output of body updates, in `bodies_written` order, with these warnings.
pub(super) fn bodies(updates: Vec<BodyUpdate>, warnings: Vec<FeatureWarning>) -> FeatureOutput {
    FeatureOutput {
        data: OutputData::Bodies(updates),
        warnings,
    }
}

/// The output of a feature that leaves `body` as it is: no update, and a `NoEffect` warning.
pub(super) fn no_effect(body: &Id) -> FeatureOutput {
    bodies(
        Vec::new(),
        vec![FeatureWarning::NoEffect { body: body.clone() }],
    )
}

/// `Canceled` once the run has been canceled. Patterns check it before each instance.
pub(super) fn check_canceled(context: &BuildContext<'_>) -> Result<(), FeatureError> {
    if context.is_canceled() {
        Err(FeatureError::Canceled)
    } else {
        Ok(())
    }
}

/// Checks the `op` and `target` of a copy feature (mirror.md section 3.1): op new needs a target,
/// op join takes none, and the target is not the source body.
pub(super) fn validate_target(
    op: CopyOp,
    body: &Id,
    target: Option<&Id>,
    manifest: &FeatureManifest,
) -> Result<(), FeatureError> {
    let field = || FieldRef::new("target", manifest);
    match (op, target) {
        (CopyOp::New, None) => Err(FeatureError::TargetRequired { field: field() }),
        (CopyOp::Join, Some(_)) => Err(FeatureError::TargetNotAllowed { field: field() }),
        (CopyOp::New, Some(target)) if target == body => Err(FeatureError::SameBody {
            field: field(),
            body: target.clone(),
        }),
        _ => Ok(()),
    }
}

/// Declares the source body of a copy feature: written with op join, which fuses the copies into
/// it, and only read with op new.
pub(super) fn add_source(dependencies: &mut Dependencies, op: CopyOp, body: &Id) {
    match op {
        CopyOp::Join => dependencies.add_body_written(body, "body"),
        CopyOp::New => dependencies.add_body_read(body, "body"),
    }
}

/// Declares the target body that a copy feature with op new writes.
pub(super) fn add_target(dependencies: &mut Dependencies, op: CopyOp, target: Option<&Id>) {
    if let (CopyOp::New, Some(target)) = (op, target) {
        dependencies.add_body_written(target, "target");
    }
}

/// The body a copy feature changes: the source with op join, the target with op new.
fn destination<'a>(
    op: CopyOp,
    body: &'a Id,
    target: Option<&'a Id>,
) -> Result<&'a Id, FeatureError> {
    match op {
        CopyOp::Join => Ok(body),
        CopyOp::New => {
            target.ok_or_else(|| FeatureError::internal("op new without a target passed validation"))
        }
    }
}

/// The source body of a copy feature, which must hold a solid.
pub(super) struct Source<'a> {
    /// The source shape.
    shape: &'a Shape,
    /// Its names.
    naming: &'a NamingMap,
}

impl<'a> Source<'a> {
    /// The source `body` just before this feature; an empty body is `BodyEmpty`.
    pub(super) fn new(context: &BuildContext<'a>, body: &Id) -> Result<Source<'a>, FeatureError> {
        let (state, shape) = solid(context, body)?;
        Ok(Source {
            shape,
            naming: state.naming(),
        })
    }

    /// The source moved by `motion`, named as instance `instance` of the feature being built:
    /// every source name with the suffix `@<feature>:<instance>` (naming.md section 3.5.1). The
    /// kernel transform keeps element indices, so the instance map fits the copy as it is.
    pub(super) fn copy(
        &self,
        context: &BuildContext<'_>,
        motion: &Transform,
        instance: Instance,
    ) -> Result<Tool, FeatureError> {
        let moved = transform(self.shape, motion).map_err(kernel)?;
        let naming = self
            .naming
            .with_instance(context.feature(), instance)
            .map_err(naming)?;
        Ok(Tool {
            shape: moved.shape,
            naming,
        })
    }
}

/// Puts the copies of a copy feature into its destination with one kernel boolean (features.md
/// sections 3.5 and 3.10): fused into the source with op join, fused with each other into the
/// empty target with op new. Without copies the destination is unchanged and the output warns
/// that the feature has no effect.
pub(super) fn place_copies(
    context: &BuildContext<'_>,
    op: CopyOp,
    body: &Id,
    target: Option<&Id>,
    copies: Vec<Tool>,
) -> Result<FeatureOutput, FeatureError> {
    let destination = destination(op, body, target)?;
    if copies.is_empty() {
        return Ok(no_effect(destination));
    }
    let body_op = match op {
        CopyOp::Join => BodyOp::Join,
        CopyOp::New => BodyOp::New,
    };
    let (update, warnings) = apply_op(context, destination, body_op, copies)?;
    Ok(bodies(vec![update], warnings))
}

/// The `into=<target part>` keyword of a copy feature's dedicated Python form: with op new only.
/// None when the target has no part variable.
pub(super) fn into_keyword(
    names: &dyn PythonNames,
    op: CopyOp,
    target: Option<&Id>,
) -> Option<Vec<(String, PythonValue)>> {
    match op {
        CopyOp::Join => Some(Vec::new()),
        CopyOp::New => {
            let part = names.body(target?)?;
            Some(vec![("into".to_string(), PythonValue::Name(part))])
        }
    }
}

/// Building and writing the features of this group in unit tests.
#[cfg(test)]
pub(super) mod testing {
    use std::sync::Arc;

    use partsmith_model::{EmptyContext, FeatureRecord, Id, ParameterValues, Scope, Units};
    use partsmith_sketch::RegionRef;
    use serde_json::Value;

    use crate::context::{BodyState, Values};
    use crate::error::FeatureError;
    use crate::feature::Feature;
    use crate::output::{BodyUpdate, FeatureOutput, OutputData};
    use crate::params::BodyOp;
    use crate::python::PythonNames;
    use crate::test_support::{Fixture, id};

    /// The typed params of a feature record written as JSON.
    pub(crate) fn params<F: Feature>(record: Value) -> F::Params {
        let record: FeatureRecord =
            serde_json::from_value(record).expect("records written in tests are well-formed");
        serde_json::from_value(Value::Object(record.params))
            .expect("params written in tests deserialize")
    }

    /// Builds `params` as feature `feature` in a millimeter document without parameters. Every
    /// body the params declare is empty unless `bodies` gives its state; `upstream` gives the
    /// outputs of the features they read.
    pub(crate) fn build<F: Feature>(
        feature: &str,
        params: &F::Params,
        bodies: Vec<(&str, BodyState)>,
        upstream: Vec<(&str, FeatureOutput)>,
    ) -> Result<FeatureOutput, FeatureError> {
        let fixture = fixture::<F>(feature, params, bodies, upstream);
        F::build(params, &fixture.context())
    }

    /// The fixture `build` builds in, for tests that change it first (for example cancel it).
    pub(crate) fn fixture<F: Feature>(
        feature: &str,
        params: &F::Params,
        bodies: Vec<(&str, BodyState)>,
        upstream: Vec<(&str, FeatureOutput)>,
    ) -> Fixture {
        let dependencies = F::dependencies(params);
        let mut fixture = Fixture::new(feature);
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
        for (feature, output) in upstream {
            fixture.upstream.insert(id(feature), Arc::new(output));
        }
        fixture
    }

    /// The body updates of an output.
    pub(crate) fn updates(output: &FeatureOutput) -> &[BodyUpdate] {
        match &output.data {
            OutputData::Bodies(updates) => updates,
            other => panic!("expected body updates, got {other:?}"),
        }
    }

    /// The shape of the update of `body` in an output.
    pub(crate) fn updated_shape<'a>(
        output: &'a FeatureOutput,
        body: &str,
    ) -> &'a partsmith_occt_sys::Shape {
        updates(output)
            .iter()
            .find(|update| update.body.as_str() == body)
            .unwrap_or_else(|| panic!("no update of {body}"))
            .shape
            .as_ref()
            .unwrap_or_else(|| panic!("{body} is empty after the feature"))
    }

    /// The bounding box of a shape as its minimum and maximum corners.
    pub(crate) fn bounds(shape: &partsmith_occt_sys::Shape) -> ([f64; 3], [f64; 3]) {
        let bounds = shape.bounding_box().expect("the shape has bounds");
        (bounds.min, bounds.max)
    }

    /// Asserts every coordinate within 1e-6 mm.
    pub(crate) fn assert_close(actual: [f64; 3], expected: [f64; 3]) {
        let close = actual
            .iter()
            .zip(expected)
            .all(|(component, wanted)| (component - wanted).abs() < 1e-6);
        assert!(close, "expected {expected:?}, got {actual:?}");
    }

    /// Script names in a millimeter document: body `b-<n>` is the part `part_<n>`, the datum
    /// plane `f-dp1` is `plane_1`, line `e-1` of sketch `f-sk1` is `line_1`, and the default op
    /// of every body is new.
    pub(crate) struct ScriptNames;

    impl PythonNames for ScriptNames {
        fn units(&self) -> Units {
            Units::Mm
        }

        fn body(&self, body: &Id) -> Option<String> {
            let number = body.as_str().strip_prefix("b-")?;
            Some(format!("part_{number}"))
        }

        fn feature(&self, feature: &Id) -> Option<String> {
            (feature.as_str() == "f-dp1").then(|| "plane_1".to_string())
        }

        fn entity(&self, sketch: &Id, entity: &Id) -> Option<String> {
            (sketch.as_str() == "f-sk1" && entity.as_str() == "e-1").then(|| "line_1".to_string())
        }

        fn parameter(&self, _name: &str) -> Option<String> {
            None
        }

        fn regions(&self, _sketch: &Id) -> Option<Vec<RegionRef>> {
            None
        }

        fn default_op(&self, _body: &Id) -> BodyOp {
            BodyOp::New
        }
    }

    /// The dedicated Python call of `params` rendered at indentation 0, or None.
    pub(crate) fn python<F: Feature>(params: &F::Params) -> Option<String> {
        F::python(params, &ScriptNames).map(|call| call.render(0))
    }
}

#[cfg(test)]
mod tests {
    use partsmith_naming::ElementKind;
    use serde_json::json;

    use super::*;
    use crate::output::BodyChange;
    use crate::test_support::{Fixture, box_body, face_names, id, named_box};

    fn manifest_with_target() -> FeatureManifest {
        manifest(
            "Copy",
            Category::Pattern,
            "copy",
            vec![hint("target", "Target body", true)],
        )
    }

    #[test]
    fn hints_start_without_unit_selection_or_options() {
        let field = hint("count", "Count", true);

        assert_eq!(field.path, "count");
        assert!(field.essential);
        assert_eq!(field.unit, None);
        assert_eq!(field.selection, None);
        assert!(field.options.is_empty());
    }

    #[test]
    fn options_keep_their_display_order() {
        let values: Vec<String> = options(&COPY_OP_OPTIONS)
            .into_iter()
            .map(|option| format!("{} {}", option.value, option.label))
            .collect();

        assert_eq!(values, ["join Join", "new New body"]);
    }

    #[test]
    fn op_new_requires_a_target() {
        let error = validate_target(CopyOp::New, &id("b-1"), None, &manifest_with_target())
            .expect_err("op new needs a target");

        assert_eq!(
            error.to_string(),
            "Op new puts the copies into another body. Pick an empty body for Target body."
        );
    }

    #[test]
    fn op_join_takes_no_target() {
        let error = validate_target(
            CopyOp::Join,
            &id("b-1"),
            Some(&id("b-2")),
            &manifest_with_target(),
        )
        .expect_err("op join takes no target");

        assert_eq!(error.code(), "target_not_allowed");
    }

    #[test]
    fn the_target_is_not_the_source() {
        let error = validate_target(
            CopyOp::New,
            &id("b-1"),
            Some(&id("b-1")),
            &manifest_with_target(),
        )
        .expect_err("the source cannot receive its copies as op new");

        assert_eq!(
            error,
            FeatureError::SameBody {
                field: FieldRef::new("target", &manifest_with_target()),
                body: id("b-1"),
            }
        );
    }

    #[test]
    fn valid_targets_pass() {
        let manifest = manifest_with_target();

        assert!(validate_target(CopyOp::Join, &id("b-1"), None, &manifest).is_ok());
        assert!(validate_target(CopyOp::New, &id("b-1"), Some(&id("b-2")), &manifest).is_ok());
    }

    #[test]
    fn op_join_writes_the_source() {
        let mut dependencies = Dependencies::default();

        add_source(&mut dependencies, CopyOp::Join, &id("b-1"));
        add_target(&mut dependencies, CopyOp::Join, None);

        assert_eq!(dependencies.bodies_written[0].id, id("b-1"));
        assert!(dependencies.bodies_read.is_empty());
    }

    #[test]
    fn op_new_reads_the_source_and_writes_the_target() {
        let mut dependencies = Dependencies::default();

        add_source(&mut dependencies, CopyOp::New, &id("b-1"));
        add_target(&mut dependencies, CopyOp::New, Some(&id("b-2")));

        assert_eq!(dependencies.bodies_read[0].id, id("b-1"));
        assert_eq!(dependencies.bodies_written[0].id, id("b-2"));
        assert_eq!(dependencies.bodies_written[0].field, "target");
    }

    #[test]
    fn a_copy_is_moved_and_named_as_its_instance() {
        let fixture =
            Fixture::new("f-lp1").with_body("b-1", box_body("f-box1", [0.0, 0.0, 0.0], 10.0));
        let context = fixture.context();
        let source = Source::new(&context, &id("b-1")).expect("the body holds a solid");

        let copy = source
            .copy(
                &context,
                &Transform::translation(partsmith_occt_sys::Vector::new(20.0, 0.0, 0.0)),
                Instance {
                    first: 2,
                    second: 1,
                },
            )
            .expect("the copy builds");

        let bounds = copy.shape.bounding_box().expect("the copy has bounds");
        assert!((bounds.min[0] - 20.0).abs() < 1e-9);
        assert!(face_names(&copy.naming).contains(&"f-box1/top@f-lp1:2.1".to_string()));
        assert_eq!(copy.naming.count(ElementKind::Edge), 12);
    }

    #[test]
    fn an_empty_source_has_nothing_to_copy() {
        let fixture = Fixture::new("f-lp1").with_body("b-1", BodyState::empty());

        let result = Source::new(&fixture.context(), &id("b-1"));

        assert_eq!(
            result.map(|_| ()),
            Err(FeatureError::BodyEmpty {
                body: id("b-1"),
                use_new: false
            })
        );
    }

    #[test]
    fn without_copies_the_destination_is_unchanged() {
        let fixture = Fixture::new("f-lp1").with_body("b-2", BodyState::empty());

        let output = place_copies(
            &fixture.context(),
            CopyOp::New,
            &id("b-1"),
            Some(&id("b-2")),
            Vec::new(),
        )
        .expect("no copies is not an error");

        assert!(testing::updates(&output).is_empty());
        assert_eq!(
            output.warnings,
            [FeatureWarning::NoEffect { body: id("b-2") }]
        );
    }

    #[test]
    fn copies_join_the_source() {
        let fixture =
            Fixture::new("f-lp1").with_body("b-1", box_body("f-box1", [0.0, 0.0, 0.0], 10.0));
        let copies = vec![named_box("f-box9", [20.0, 0.0, 0.0], 10.0)];

        let output = place_copies(&fixture.context(), CopyOp::Join, &id("b-1"), None, copies)
            .expect("the copies join");

        let updates = testing::updates(&output);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].change, BodyChange::Join);
        assert_eq!(updates[0].body, id("b-1"));
    }

    #[test]
    fn into_is_written_for_op_new_only() {
        let names = testing::ScriptNames;

        assert_eq!(into_keyword(&names, CopyOp::Join, None), Some(Vec::new()));
        assert_eq!(
            into_keyword(&names, CopyOp::New, Some(&id("b-2"))),
            Some(vec![(
                "into".to_string(),
                PythonValue::Name("part_2".to_string())
            )])
        );
        assert_eq!(
            into_keyword(&names, CopyOp::New, Some(&id("body-x"))),
            None
        );
    }

    #[test]
    fn no_effect_names_the_unchanged_body() {
        let output = no_effect(&id("b-1"));

        assert!(testing::updates(&output).is_empty());
        assert_eq!(output.warnings[0].code(), "no_effect");
        let _ = json!(null);
    }
}
