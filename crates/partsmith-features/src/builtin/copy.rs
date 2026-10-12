//! What the copy features `mirror`, `linear_pattern` and `circular_pattern` share: the rules for
//! `op` and `target`, the bodies they declare, copies of the source named by instance number, and
//! how the copies are combined (features.md section 3.5, mirror.md section 3, naming.md sections
//! 3.5.1, 3.6.10 and 3.6.11).

use partsmith_model::{Expr, FeatureManifest, FieldUnit, Id};
use partsmith_naming::Instance;
use partsmith_occt_sys::{Shape, Transform, transform};

use super::common::{bodies, solid_body, unchanged};
use crate::context::{BodyState, BuildContext};
use crate::deps::Dependencies;
use crate::error::{Bound, FeatureError, FeatureWarning, FieldRef};
use crate::naming::{kernel, naming};
use crate::output::{BodyChange, FeatureOutput, Tool, apply_op};
use crate::params::{BodyOp, CopyOp};

/// Most instances a pattern makes, the original included (linear_pattern.md and
/// circular_pattern.md section 3.3).
const INSTANCE_LIMIT: u64 = 10_000;

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

/// The declared instance count of a pattern at `field`, the original included, which must be at
/// least 1 (linear_pattern.md and circular_pattern.md section 3.3). Evaluation already rejects a
/// negative count as below 0; it is reported against the bound of 1 instead, so the suggested fix
/// is a count that works.
pub(super) fn instance_count(
    context: &BuildContext<'_>,
    field: &str,
    expr: &Expr,
) -> Result<u32, FeatureError> {
    let below_one = |value: f64| FeatureError::OutOfRange {
        field: context.field(field),
        kind: FieldUnit::Count,
        value,
        bound: Bound::AtLeast(1.0),
        bound_name: None,
    };
    match context.eval_count(field, expr) {
        Ok(0) => Err(below_one(0.0)),
        Ok(count) => Ok(count),
        Err(FeatureError::OutOfRange {
            value,
            bound: Bound::AtLeast(_),
            ..
        }) => Err(below_one(value)),
        Err(error) => Err(error),
    }
}

/// `TooManyInstances` when a pattern would make more than 10000 instances, the original included
/// (linear_pattern.md and circular_pattern.md section 3.3).
pub(super) fn check_instance_limit(total: u64) -> Result<(), FeatureError> {
    if total > INSTANCE_LIMIT {
        Err(FeatureError::TooManyInstances {
            count: total,
            limit: INSTANCE_LIMIT,
        })
    } else {
        Ok(())
    }
}

/// `Canceled` once the run has been canceled. Patterns check it before each instance.
pub(super) fn check_canceled(context: &BuildContext<'_>) -> Result<(), FeatureError> {
    if context.is_canceled() {
        Err(FeatureError::Canceled)
    } else {
        Ok(())
    }
}

/// The source body of a copy feature, which must hold a solid.
pub(super) struct Source<'a> {
    /// The source body.
    body: &'a Id,
    /// Its state just before the feature.
    state: &'a BodyState,
    /// Its shape.
    shape: &'a Shape,
}

impl<'a> Source<'a> {
    /// The source `body` just before this feature; an empty body is `BodyEmpty`.
    pub(super) fn new(
        context: &BuildContext<'a>,
        body: &'a Id,
    ) -> Result<Source<'a>, FeatureError> {
        let (state, shape) = solid_body(context, body)?;
        Ok(Source { body, state, shape })
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
            .state
            .naming()
            .with_instance(context.feature(), instance)
            .map_err(naming)?;
        Ok(Tool {
            shape: moved.shape,
            naming,
        })
    }

    /// Puts `copies` into the feature's destination with one kernel boolean (features.md sections
    /// 3.5 and 3.10): fused into the source with op join, fused with each other into the empty
    /// `target` with op new. With op new a target that holds a solid is `BodyNotEmpty`, with or
    /// without copies. Without copies the destination keeps its state and the feature warns
    /// `NoEffect`: the source gets an unchanged update with op join, and the empty target gets no
    /// update with op new.
    pub(super) fn place(
        &self,
        context: &BuildContext<'_>,
        op: CopyOp,
        target: Option<&Id>,
        copies: Vec<Tool>,
    ) -> Result<FeatureOutput, FeatureError> {
        let destination = match op {
            CopyOp::Join => self.body,
            CopyOp::New => {
                let target = target.ok_or_else(|| {
                    FeatureError::internal(
                        "a copy feature with op new and no target passed validation",
                    )
                })?;
                if !context.body(target)?.is_empty() {
                    return Err(FeatureError::BodyNotEmpty {
                        body: target.clone(),
                    });
                }
                target
            }
        };
        if copies.is_empty() {
            let warnings = vec![FeatureWarning::NoEffect {
                body: destination.clone(),
            }];
            let updates = match op {
                CopyOp::Join => vec![unchanged(self.body, self.state, BodyChange::Join)],
                CopyOp::New => Vec::new(),
            };
            return Ok(bodies(updates, warnings));
        }
        let body_op = match op {
            CopyOp::Join => BodyOp::Join,
            CopyOp::New => BodyOp::New,
        };
        let (update, warnings) = apply_op(context, destination, body_op, copies)?;
        Ok(bodies(vec![update], warnings))
    }
}

/// Building the body features of this group (boolean, the copy features, move body, split body
/// and import) in unit tests, without a timeline.
#[cfg(test)]
pub(super) mod testing {
    use std::sync::Arc;

    use partsmith_model::{EmptyContext, FeatureRecord, ParameterValues, Scope, Units};
    use partsmith_occt_sys::{Point, Shape};
    use serde_json::Value;

    use crate::context::{BodyState, Values};
    use crate::error::FeatureError;
    use crate::feature::Feature;
    use crate::output::{BodyUpdate, FeatureOutput, OutputData};
    use crate::test_support::{Fixture, ScriptNames, assert_point, assert_relative, id, volume};

    /// The typed params of a feature record written as JSON.
    pub(crate) fn params<F: Feature>(record: Value) -> F::Params {
        let record: FeatureRecord =
            serde_json::from_value(record).expect("records written in tests are well-formed");
        serde_json::from_value(Value::Object(record.params))
            .expect("params written in tests deserialize")
    }

    /// The typed params of a feature record written as JSON, with the members of the JSON object
    /// `extra` added to it or replacing its own.
    pub(crate) fn params_with<F: Feature>(mut record: Value, extra: Value) -> F::Params {
        let Value::Object(members) = extra else {
            panic!("extra members written in tests are a JSON object, not {extra}");
        };
        for (key, value) in members {
            record[key] = value;
        }
        params::<F>(record)
    }

    /// The fixture of feature `feature` with `params` in a millimeter document without
    /// parameters: its declared expressions evaluated, every declared body empty unless `bodies`
    /// gives its state, and `upstream` as the outputs of the features it reads.
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

    /// Builds `params` as feature `feature` over the fixture that `fixture` makes for them.
    pub(crate) fn build<F: Feature>(
        feature: &str,
        params: &F::Params,
        bodies: Vec<(&str, BodyState)>,
        upstream: Vec<(&str, FeatureOutput)>,
    ) -> Result<FeatureOutput, FeatureError> {
        let fixture = fixture::<F>(feature, params, bodies, upstream);
        F::build(params, &fixture.context())
    }

    /// The body updates of an output.
    pub(crate) fn updates(output: &FeatureOutput) -> &[BodyUpdate] {
        match &output.data {
            OutputData::Bodies(updates) => updates,
            other => panic!("expected body updates, got {other:?}"),
        }
    }

    /// The update of `body` in an output.
    pub(crate) fn update<'a>(output: &'a FeatureOutput, body: &str) -> &'a BodyUpdate {
        updates(output)
            .iter()
            .find(|update| update.body.as_str() == body)
            .unwrap_or_else(|| panic!("no update of {body}"))
    }

    /// The shape the update of `body` gives it; fails the test when it empties the body.
    pub(crate) fn updated_shape<'a>(output: &'a FeatureOutput, body: &str) -> &'a Shape {
        update(output, body)
            .shape
            .as_ref()
            .unwrap_or_else(|| panic!("{body} is empty after the feature"))
    }

    /// Asserts a shape's volume within the relative tolerance of features.md section 6.1.
    pub(crate) fn assert_volume(shape: &Shape, expected: f64) {
        assert_relative("volume", volume(shape), expected);
    }

    /// Asserts a shape's tight bounding box within the 1e-5 mm per coordinate of features.md
    /// section 6.1.
    pub(crate) fn assert_bounds(shape: &Shape, min: [f64; 3], max: [f64; 3]) {
        let bounds = shape.bounding_box().expect("the shape has bounds");
        let corner = |[x, y, z]: [f64; 3]| Point::new(x, y, z);
        assert_point("bounding box minimum", corner(bounds.min), min);
        assert_point("bounding box maximum", corner(bounds.max), max);
    }

    /// The dedicated Python call of `params` with the default `ScriptNames`, rendered at
    /// indentation 0, or None.
    pub(crate) fn python<F: Feature>(params: &F::Params) -> Option<String> {
        python_with::<F>(params, &ScriptNames::default())
    }

    /// The dedicated Python call of `params` with `names`, rendered at indentation 0, or None.
    pub(crate) fn python_with<F: Feature>(
        params: &F::Params,
        names: &ScriptNames,
    ) -> Option<String> {
        F::python(params, names).map(|call| call.render(0))
    }
}

#[cfg(test)]
mod tests {
    use partsmith_model::{Category, EmptyContext, ParameterValues, Scope, Units};
    use partsmith_naming::ElementKind;
    use partsmith_occt_sys::Vector;

    use super::*;
    use crate::builtin::common::{essential, manifest, selection_hint, unit_hint};
    use crate::context::Values;
    use crate::deps::ExprUse;
    use crate::test_support::{Fixture, box_body, face_names, id, named_box};

    fn manifest_with_target() -> FeatureManifest {
        manifest(
            "Copy",
            Category::Pattern,
            "copy",
            vec![essential(selection_hint(
                "target",
                "Target body",
                partsmith_model::SelectionKind::Body,
            ))],
        )
    }

    /// A pattern fixture whose count, labeled Count, is declared at `count` as `expr`.
    fn count_fixture(expr: &str) -> Fixture {
        let mut fixture = Fixture::new("f-lp1");
        fixture.manifest = manifest(
            "Pattern",
            Category::Pattern,
            "pattern",
            vec![unit_hint("count", "Count", FieldUnit::Count)],
        );
        let uses = [ExprUse {
            field: "count".to_string(),
            expr: Expr::new(expr),
            kind: FieldUnit::Count,
        }];
        let parameters = ParameterValues::default();
        let scope = Scope {
            units: Units::Mm,
            parameters: &parameters,
            context: &EmptyContext,
        };
        fixture.values = Values::evaluate(&uses, &scope, &fixture.manifest);
        fixture
    }

    /// The instance count of `count_fixture(expr)`.
    fn instance_count_of(expr: &str) -> Result<u32, FeatureError> {
        let fixture = count_fixture(expr);
        instance_count(&fixture.context(), "count", &Expr::new(expr))
    }

    #[test]
    fn a_whole_count_of_at_least_one_is_the_instance_count() {
        assert_eq!(instance_count_of("3"), Ok(3));
        assert_eq!(instance_count_of("1"), Ok(1));
    }

    #[test]
    fn a_zero_count_is_below_one() {
        let error = instance_count_of("0").expect_err("count 0 makes no instance");

        assert_eq!(
            error.to_string(),
            "Count must be at least 1, but it is 0. Use 1 or more."
        );
        assert_eq!(error.suggestion(Units::Mm).as_deref(), Some("1"));
    }

    #[test]
    fn a_negative_count_is_below_one_so_the_suggestion_works() {
        let error = instance_count_of("-1").expect_err("count -1 makes no instance");

        assert_eq!(error.field().expect("names a field").path, "count");
        assert_eq!(
            error.to_string(),
            "Count must be at least 1, but it is -1. Use 1 or more."
        );
        assert_eq!(error.suggestion(Units::Mm).as_deref(), Some("1"));
    }

    #[test]
    fn a_fractional_count_is_not_a_whole_number() {
        let error = instance_count_of("2.5").expect_err("count 2.5 is not whole");

        assert_eq!(error.code(), "not_whole_number");
    }

    #[test]
    fn ten_thousand_instances_are_the_limit() {
        assert_eq!(check_instance_limit(10_000), Ok(()));
        assert_eq!(
            check_instance_limit(10_001),
            Err(FeatureError::TooManyInstances {
                count: 10_001,
                limit: 10_000
            })
        );
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

        assert_eq!(
            error.to_string(),
            "Target body is only used with op new. Remove it, or set the op to new."
        );
    }

    #[test]
    fn the_target_is_not_the_source() {
        let error = validate_target(
            CopyOp::New,
            &id("b-1"),
            Some(&id("b-1")),
            &manifest_with_target(),
        )
        .expect_err("the source cannot receive its own copies with op new");

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
        let body = id("b-1");
        let source = Source::new(&context, &body).expect("the body holds a solid");

        let copy = source
            .copy(
                &context,
                &Transform::translation(Vector::new(20.0, 0.0, 0.0)),
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
        let body = id("b-1");

        let result = Source::new(&fixture.context(), &body);

        assert_eq!(
            result.map(|_| ()),
            Err(FeatureError::BodyEmpty {
                body: id("b-1"),
                use_new: false
            })
        );
    }

    #[test]
    fn copies_join_the_source() {
        let fixture =
            Fixture::new("f-lp1").with_body("b-1", box_body("f-box1", [0.0, 0.0, 0.0], 10.0));
        let context = fixture.context();
        let body = id("b-1");
        let source = Source::new(&context, &body).expect("the body holds a solid");
        let copies = vec![named_box("f-box9", [20.0, 0.0, 0.0], 10.0)];

        let output = source
            .place(&context, CopyOp::Join, None, copies)
            .expect("the copies join");

        let updates = testing::updates(&output);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].change, BodyChange::Join);
        testing::assert_volume(testing::updated_shape(&output, "b-1"), 2000.0);
    }

    #[test]
    fn without_copies_op_join_keeps_the_source_exactly() {
        let state = box_body("f-box1", [0.0, 0.0, 0.0], 10.0);
        let fixture = Fixture::new("f-lp1").with_body("b-1", state.clone());
        let context = fixture.context();
        let body = id("b-1");
        let source = Source::new(&context, &body).expect("the body holds a solid");

        let output = source
            .place(&context, CopyOp::Join, None, Vec::new())
            .expect("no copies is not an error");

        let update = testing::update(&output, "b-1");
        assert!(std::sync::Arc::ptr_eq(&update.naming, state.naming()));
        assert_eq!(update.change, BodyChange::Join);
        assert_eq!(
            output.warnings,
            [FeatureWarning::NoEffect { body: id("b-1") }]
        );
    }

    #[test]
    fn without_copies_op_new_leaves_the_target_empty() {
        let fixture = Fixture::new("f-lp1")
            .with_body("b-1", box_body("f-box1", [0.0, 0.0, 0.0], 10.0))
            .with_body("b-2", BodyState::empty());
        let context = fixture.context();
        let body = id("b-1");
        let source = Source::new(&context, &body).expect("the body holds a solid");

        let output = source
            .place(&context, CopyOp::New, Some(&id("b-2")), Vec::new())
            .expect("no copies is not an error");

        assert!(testing::updates(&output).is_empty());
        assert_eq!(
            output.warnings,
            [FeatureWarning::NoEffect { body: id("b-2") }]
        );
    }

    #[test]
    fn without_copies_op_new_still_needs_an_empty_target() {
        let fixture = Fixture::new("f-lp1")
            .with_body("b-1", box_body("f-box1", [0.0, 0.0, 0.0], 10.0))
            .with_body("b-2", box_body("f-box2", [50.0, 0.0, 0.0], 1.0));
        let context = fixture.context();
        let body = id("b-1");
        let source = Source::new(&context, &body).expect("the body holds a solid");

        let result = source.place(&context, CopyOp::New, Some(&id("b-2")), Vec::new());

        assert_eq!(
            result.map(|_| ()),
            Err(FeatureError::BodyNotEmpty { body: id("b-2") })
        );
    }

    #[test]
    fn copies_into_a_target_with_a_solid_are_body_not_empty() {
        let fixture = Fixture::new("f-lp1")
            .with_body("b-1", box_body("f-box1", [0.0, 0.0, 0.0], 10.0))
            .with_body("b-2", box_body("f-box2", [50.0, 0.0, 0.0], 1.0));
        let context = fixture.context();
        let body = id("b-1");
        let source = Source::new(&context, &body).expect("the body holds a solid");
        let copies = vec![named_box("f-box9", [20.0, 0.0, 0.0], 10.0)];

        let result = source.place(&context, CopyOp::New, Some(&id("b-2")), copies);

        assert_eq!(
            result.map(|_| ()),
            Err(FeatureError::BodyNotEmpty { body: id("b-2") })
        );
    }
}
