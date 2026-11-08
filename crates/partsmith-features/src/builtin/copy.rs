//! What the copy features `mirror`, `linear_pattern` and `circular_pattern` share: the rules for
//! `op` and `target`, the bodies they declare, copies of the source named by instance number, and
//! how the copies are combined (features.md section 3.5, mirror.md section 3, naming.md sections
//! 3.5.1, 3.6.10 and 3.6.11).

use partsmith_model::{FeatureManifest, FieldHint, Id};
use partsmith_naming::Instance;
use partsmith_occt_sys::{KernelError, KernelErrorKind, Shape, SubShapeKind, Transform, transform};

use super::common::{
    bodies, body_empty, naming_motion, options_hint, solid_body, unchanged, without_separate_solids,
};
use crate::context::{BodyState, BuildContext};
use crate::deps::Dependencies;
use crate::error::{FeatureError, FeatureWarning, FieldRef, NoEffectHow};
use crate::output::{BodyChange, FeatureOutput, Tool, apply_op, has_volume};
use crate::params::{BodyOp, CopyOp};

/// The options of a `CopyOp` field, with their labels.
const COPY_OP_OPTIONS: [(&str, &str); 2] = [("join", "Join"), ("new", "New body")];

/// Most instances a pattern makes, the original included (linear_pattern.md and
/// circular_pattern.md section 3.3).
const INSTANCE_LIMIT: u64 = 10_000;

/// The `op` hint of a copy feature.
pub(super) fn op_hint() -> FieldHint {
    options_hint("op", "Operation", &COPY_OP_OPTIONS)
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

/// The source body of a copy feature, whose every solid must have volume (features.md section 3.5).
pub(super) struct Source<'a> {
    /// The source body.
    body: &'a Id,
    /// Its state just before the feature.
    state: &'a BodyState,
    /// Its shape.
    shape: &'a Shape,
}

impl<'a> Source<'a> {
    /// The source `body` just before this feature (features.md section 3.5): a body whose shape holds no solid is
    /// `BodyEmpty`, and one that holds a solid without volume `BodyNotValid`.
    pub(super) fn new(
        context: &BuildContext<'a>,
        body: &'a Id,
    ) -> Result<Source<'a>, FeatureError> {
        let (state, shape) = solid_body(context, "body", body)?;
        if shape.count(SubShapeKind::Solid) == 0 {
            return Err(body_empty(context, "body", body));
        }
        if holds_solid_without_volume(state)? {
            return Err(body_not_valid(context, body, None));
        }
        Ok(Source { body, state, shape })
    }

    /// The source moved by `copy_transform`, named as instance `instance` of the feature being
    /// built: every source name with the suffix `@<feature>:<instance>` (naming.md section 3.5.1),
    /// with `copy_transform` as the motion of its faces. The kernel transform keeps element
    /// indices, so the instance map fits the copy as it is. A mirror of a solid the kernel finds
    /// was already invalid is `BodyNotValid` (features.md section 3.5).
    pub(super) fn copy(
        &self,
        context: &BuildContext<'_>,
        copy_transform: &Transform,
        instance: Instance,
    ) -> Result<Tool, FeatureError> {
        let moved = transform(self.shape, copy_transform).map_err(|error| {
            if error.kind == KernelErrorKind::InvalidInputShape {
                body_not_valid(context, self.body, Some(error))
            } else {
                FeatureError::from(error)
            }
        })?;
        let naming = self.state.naming().with_instance(
            context.feature(),
            instance,
            &naming_motion(copy_transform),
        )?;
        Ok(Tool {
            shape: moved.shape,
            naming,
        })
    }

    /// Puts `copies` into the feature's destination with one kernel boolean (features.md sections
    /// 3.5 and 3.10): fused into the source with op join, fused with each other into the empty
    /// `target` with op new. With op new a target that holds a solid is `BodyNotEmpty`, with or
    /// without copies. Without copies the destination keeps its state and the feature warns
    /// `NoEffect` with `NoCopies` of `ones`, the count fields that are 1: the source gets an
    /// unchanged update with op join, and the empty target gets no update with op new. Separate
    /// copies are what a copy feature makes, so the join reports no `SeparateSolid` (section 3.5).
    pub(super) fn place(
        &self,
        context: &BuildContext<'_>,
        op: CopyOp,
        target: Option<&Id>,
        copies: Vec<Tool>,
        mut warnings: Vec<FeatureWarning>,
        ones: Vec<FieldRef>,
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
                        target: Some(context.field("target")),
                        new_body: false,
                    });
                }
                target
            }
        };
        if copies.is_empty() {
            warnings.push(FeatureWarning::NoEffect {
                body: destination.clone(),
                how: NoEffectHow::NoCopies { fields: ones },
                reverse_hits: false,
                attach: None,
            });
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
        let (update, op_warnings) = apply_op(context, destination, body_op, copies, None)?;
        warnings.extend(without_separate_solids(op_warnings));
        Ok(bodies(vec![update], warnings))
    }
}

/// True when a solid of `state` bounds no material (features.md section 3.5): `has_volume` is false for the body's
/// volume and area when it holds one solid, and for a solid's own volume and area when it holds several. The areas
/// are sums of the face sizes in the state's topology. Only a body of several solids is measured again, one volume
/// per solid.
fn holds_solid_without_volume(state: &BodyState) -> Result<bool, FeatureError> {
    let Some(shape) = state.shape() else {
        return Ok(false);
    };
    let faces = &state.topology()?.faces;
    if shape.count(SubShapeKind::Solid) == 1 {
        let area = faces.iter().filter_map(|face| face.fingerprint.size).sum();
        return Ok(!has_volume(state.volume()?, area));
    }
    for solid in shape.solids()? {
        let area = shape
            .locate(&solid, SubShapeKind::Face)?
            .into_iter()
            .flatten()
            .filter_map(|face| faces.get(face as usize - 1)?.fingerprint.size)
            .sum();
        if !has_volume(solid.volume()?, area) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// True when `state` holds a shape that `Shape::is_valid` finds not valid, the kernel's own check of a mirror's
/// input (kernel.md section 3.5).
fn holds_invalid_shape(state: &BodyState) -> Result<bool, FeatureError> {
    match state.shape() {
        Some(shape) => Ok(!shape.is_valid()?),
        None => Ok(false),
    }
}

/// `BodyNotValid` of the source `body` (features.md section 3.5), with the kernel error `cause` when there is one,
/// or the error that kept the feature that put the solid in the body from being found. That feature is found by the
/// check the source failed: the kernel's validity check with a cause, else the volume check.
fn body_not_valid(
    context: &BuildContext<'_>,
    body: &Id,
    cause: Option<KernelError>,
) -> FeatureError {
    let made_by = if cause.is_some() {
        made_by(context, body, holds_invalid_shape)
    } else {
        made_by(context, body, holds_solid_without_volume)
    };
    match made_by {
        Ok(made_by) => FeatureError::BodyNotValid {
            field: context.field("body"),
            body: body.clone(),
            made_by,
            cause,
        },
        Err(error) => error,
    }
}

/// The feature that put the solid that is not valid in `body` (features.md section 3.5): the oldest writer of the body
/// (`body_history`) whose result `not_valid` finds, else the newest, whose result is the source. The result of a
/// writer is the state just before the next one. Only a chain without a writer has none.
fn made_by(
    context: &BuildContext<'_>,
    body: &Id,
    not_valid: fn(&BodyState) -> Result<bool, FeatureError>,
) -> Result<Id, FeatureError> {
    let history = context.body_history(body)?;
    let Some((newest, _)) = history.last() else {
        return Err(FeatureError::internal(format!(
            "body {body} holds a solid that is not valid, but no writer put it there"
        )));
    };
    for ((writer, _), (_, result)) in history.iter().zip(&history[1..]) {
        if not_valid(result)? {
            return Ok(writer.clone());
        }
    }
    Ok(newest.clone())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use partsmith_model::Category;
    use partsmith_naming::ElementKind;
    use partsmith_occt_sys::{
        Interrupt, KernelError, KernelErrorKind, Operation, Plane, StepBody, Vector, compound,
        write_step,
    };

    use super::*;
    use crate::builtin::common::{essential, manifest, selection_hint};
    use crate::output::{BodyUpdate, Measurements};
    use crate::test_support::{
        Fixture, assert_volume, box_body, face_names, id, imported, named_box, self_void_step,
        step_box, update, updated_shape, updates,
    };

    fn manifest_with_target() -> FeatureManifest {
        manifest(
            "Copy",
            "Copies a body in the tests.",
            Category::Pattern,
            "copy",
            vec![essential(selection_hint(
                "target",
                "Target body",
                partsmith_model::SelectionKind::Body,
            ))],
        )
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

        // Features.md section 4.1, `target_required`.
        assert_eq!(
            error.to_string(),
            "Operation New body puts the copies into another body, so Target body needs an empty body. Pick one, or set Operation to Join."
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
            "Target body is used only when Operation is New body. Clear Target body, or set Operation to New body."
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
        let context = fixture.context();
        let body = id("b-1");

        let result = Source::new(&context, &body);

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
    fn copies_join_the_source() {
        let fixture =
            Fixture::new("f-lp1").with_body("b-1", box_body("f-box1", [0.0, 0.0, 0.0], 10.0));
        let context = fixture.context();
        let body = id("b-1");
        let source = Source::new(&context, &body).expect("the body holds a solid");
        let copies = vec![named_box("f-box9", [20.0, 0.0, 0.0], 10.0)];

        let output = source
            .place(&context, CopyOp::Join, None, copies, Vec::new(), Vec::new())
            .expect("the copies join");

        let updates = updates(&output);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].change, BodyChange::Join);
        assert_volume(updated_shape(&output, "b-1"), 2000.0);
    }

    #[test]
    fn without_copies_op_join_keeps_the_source_exactly() {
        let state = box_body("f-box1", [0.0, 0.0, 0.0], 10.0);
        let fixture = Fixture::new("f-lp1").with_body("b-1", state.clone());
        let context = fixture.context();
        let body = id("b-1");
        let source = Source::new(&context, &body).expect("the body holds a solid");

        let output = source
            .place(
                &context,
                CopyOp::Join,
                None,
                Vec::new(),
                Vec::new(),
                vec![count()],
            )
            .expect("no copies is not an error");

        let update = update(&output, "b-1");
        assert!(std::sync::Arc::ptr_eq(&update.naming, state.naming()));
        assert_eq!(update.change, BodyChange::Join);
        assert_eq!(output.warnings, [no_copies("b-1")]);
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
            .place(
                &context,
                CopyOp::New,
                Some(&id("b-2")),
                Vec::new(),
                Vec::new(),
                vec![count()],
            )
            .expect("no copies is not an error");

        assert!(updates(&output).is_empty());
        assert_eq!(output.warnings, [no_copies("b-2")]);
    }

    #[test]
    fn without_copies_op_new_still_needs_an_empty_target() {
        let fixture = Fixture::new("f-lp1")
            .with_body("b-1", box_body("f-box1", [0.0, 0.0, 0.0], 10.0))
            .with_body("b-2", box_body("f-box2", [50.0, 0.0, 0.0], 1.0));
        let context = fixture.context();
        let body = id("b-1");
        let source = Source::new(&context, &body).expect("the body holds a solid");

        let result = source.place(
            &context,
            CopyOp::New,
            Some(&id("b-2")),
            Vec::new(),
            Vec::new(),
            vec![count()],
        );

        assert_eq!(result.map(|_| ()), Err(target_not_empty(&context)));
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

        let result = source.place(
            &context,
            CopyOp::New,
            Some(&id("b-2")),
            copies,
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(result.map(|_| ()), Err(target_not_empty(&context)));
    }

    #[test]
    fn a_source_whose_shape_holds_no_solid_is_empty() {
        let nothing = compound(&[]).expect("an empty compound").shape;
        let state = BodyState::new(nothing, Arc::clone(BodyState::empty().naming()));
        let fixture = Fixture::new("f-mir1").with_body("b-1", state);
        let context = fixture.context();
        let body = id("b-1");

        let result = Source::new(&context, &body);

        // A join with a body without volume can leave such a shape (features.md section 3.5).
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
    fn a_source_without_volume_is_not_valid() {
        let fixture = self_void_fixture(vec![(id("f-imp1"), BodyState::empty())]);
        let context = fixture.context();
        let body = id("b-1");

        let result = Source::new(&context, &body);

        assert_eq!(result.map(|_| ()), Err(not_valid(&context, "f-imp1", None)));
    }

    #[test]
    fn a_source_with_one_solid_without_volume_is_not_valid() {
        let state = self_void_and_cube_state();
        // The body as a whole has the cube's volume. Only a check per solid finds the block.
        assert_volume(state.shape().expect("the import holds both solids"), 1000.0);
        let mut fixture = Fixture::new("f-lp1").with_body("b-1", state);
        fixture
            .history
            .insert(id("b-1"), vec![(id("f-imp1"), BodyState::empty())]);
        let context = fixture.context();
        let body = id("b-1");

        let result = Source::new(&context, &body);

        assert_eq!(result.map(|_| ()), Err(not_valid(&context, "f-imp1", None)));
    }

    #[test]
    fn a_source_whose_solids_all_have_volume_is_valid() {
        let cubes = imported(
            write_step(
                &[
                    step_box("A", [0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
                    step_box("B", [50.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
                ],
                &Interrupt::new(),
            )
            .expect("the kernel writes STEP"),
        );
        let fixture = Fixture::new("f-lp1").with_body("b-1", cubes);
        let context = fixture.context();
        let body = id("b-1");

        let result = Source::new(&context, &body);

        assert!(result.is_ok(), "{:?}", result.map(|_| ()));
    }

    #[test]
    fn made_by_is_the_import_whose_result_holds_the_solid() {
        // A move of the imported solid keeps it without volume. The import put it in the body.
        let fixture = self_void_fixture(vec![
            (id("f-imp1"), BodyState::empty()),
            (id("f-mv1"), self_void_state()),
        ]);
        let context = fixture.context();
        let body = id("b-1");

        let result = Source::new(&context, &body);

        assert_eq!(result.map(|_| ()), Err(not_valid(&context, "f-imp1", None)));
    }

    #[test]
    fn made_by_passes_over_results_with_volume() {
        let fixture = self_void_fixture(vec![
            (id("f-box1"), BodyState::empty()),
            (id("f-bool1"), box_body("f-box1", [0.0, 0.0, 0.0], 10.0)),
        ]);
        let context = fixture.context();
        let body = id("b-1");

        let result = Source::new(&context, &body);

        assert_eq!(
            result.map(|_| ()),
            Err(not_valid(&context, "f-bool1", None))
        );
    }

    #[test]
    fn made_by_of_a_source_without_volume_reads_volume_not_validity() {
        // The import's result is valid but has no volume, as a STEP solid whose void is a copy of
        // its outer shell can be. The import put the solid in the body, not the move after it.
        let fixture = self_void_fixture(vec![
            (id("f-imp1"), BodyState::empty()),
            (id("f-mv1"), valid_without_volume()),
        ]);
        let context = fixture.context();
        let body = id("b-1");

        let result = Source::new(&context, &body);

        assert_eq!(result.map(|_| ()), Err(not_valid(&context, "f-imp1", None)));
    }

    #[test]
    fn made_by_stops_at_the_oldest_result_that_holds_the_solid() {
        // No state after the import's result is read. The box the boolean left does not count.
        let fixture = self_void_fixture(vec![
            (id("f-imp1"), BodyState::empty()),
            (id("f-bool1"), self_void_state()),
            (id("f-mv1"), box_body("f-box1", [0.0, 0.0, 0.0], 10.0)),
        ]);
        let context = fixture.context();
        let body = id("b-1");

        let result = Source::new(&context, &body);

        assert_eq!(result.map(|_| ()), Err(not_valid(&context, "f-imp1", None)));
    }

    #[test]
    fn a_body_not_valid_without_writers_is_a_bug() {
        let fixture = self_void_fixture(Vec::new());
        let context = fixture.context();
        let body = id("b-1");

        let result = Source::new(&context, &body);

        assert!(
            matches!(result, Err(FeatureError::Internal { .. })),
            "{:?}",
            result.map(|_| ())
        );
    }

    #[test]
    fn a_mirror_of_a_solid_repair_could_not_fix_is_not_valid() {
        let fixture = self_void_fixture(vec![(id("f-imp1"), BodyState::empty())]);
        let context = fixture.context();

        let Err(FeatureError::BodyNotValid {
            field,
            body,
            made_by,
            cause: Some(cause),
        }) = mirror_unchecked(&context)
        else {
            panic!("the mirror of an invalid solid is BodyNotValid with its kernel error");
        };
        assert_eq!(field, context.field("body"));
        assert_eq!(body, id("b-1"));
        assert_eq!(made_by, id("f-imp1"));
        assert_eq!(cause.operation, Operation::Transform);
        assert_eq!(cause.kind, KernelErrorKind::InvalidInputShape);
    }

    #[test]
    fn made_by_of_a_mirror_the_kernel_refused_reads_validity() {
        // The kernel found the source not valid. The move's input is valid. The move put the solid
        // in the body however much volume its input has.
        let fixture = self_void_fixture(vec![
            (id("f-imp1"), BodyState::empty()),
            (id("f-mv1"), valid_without_volume()),
        ]);
        let context = fixture.context();

        let result = mirror_unchecked(&context);

        assert!(
            matches!(&result, Err(FeatureError::BodyNotValid { made_by, .. }) if *made_by == id("f-mv1")),
            "{result:?}"
        );
    }

    #[test]
    fn a_rigid_copy_of_a_solid_repair_could_not_fix_builds() {
        let fixture = self_void_fixture(vec![(id("f-imp1"), BodyState::empty())]);
        let context = fixture.context();
        let body = id("b-1");
        let state = self_void_state();
        let source = unchecked_source(&body, &state);

        let copy = source.copy(
            &context,
            &Transform::translation(Vector::new(30.0, 0.0, 0.0)),
            Instance {
                first: 2,
                second: 1,
            },
        );

        // The kernel does not check a rigid move (kernel.md section 3.5 item 4).
        assert!(copy.is_ok(), "{:?}", copy.map(|_| ()));
    }

    /// The block of `SELF_VOID` of features/import.md, [0,10]x[0,20]x[0,30], named `Block`.
    fn block() -> StepBody {
        step_box("Block", [0.0, 0.0, 0.0], [10.0, 20.0, 30.0])
    }

    /// The state of `b-1` after `f-imp1` imports `SELF_VOID` with op new: one solid without
    /// volume that repair could not fix.
    fn self_void_state() -> BodyState {
        imported(self_void_step(&[block()]))
    }

    /// The state of `b-1` after `f-imp1` imports the block of `SELF_VOID` and a valid 10 mm cube
    /// 50 mm away along X with op new: two solids, the first without volume.
    fn self_void_and_cube_state() -> BodyState {
        imported(self_void_step(&[
            block(),
            step_box("Cube", [50.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
        ]))
    }

    /// A valid 10 mm box whose state carries a volume of 0: a solid that `Shape::is_valid`
    /// accepts and that bounds no material.
    fn valid_without_volume() -> BodyState {
        let state = box_body("f-box1", [0.0, 0.0, 0.0], 10.0);
        BodyState::from_update(&BodyUpdate {
            body: id("b-1"),
            shape: state.shape().cloned(),
            naming: Arc::clone(state.naming()),
            change: BodyChange::New,
            measured: Measurements {
                topology: None,
                volume: Some(0.0),
            },
        })
    }

    /// A copy feature `f-mir1` whose source `b-1` holds `SELF_VOID`, with `history` as the
    /// writer chain of `b-1`.
    fn self_void_fixture(history: Vec<(Id, BodyState)>) -> Fixture {
        let mut fixture = Fixture::new("f-mir1").with_body("b-1", self_void_state());
        fixture.history.insert(id("b-1"), history);
        fixture
    }

    /// The mirror image through `YZ` of the source `b-1` of `context`, `SELF_VOID`, without the
    /// checks of `Source::new`.
    fn mirror_unchecked(context: &BuildContext<'_>) -> Result<(), FeatureError> {
        let body = id("b-1");
        let state = self_void_state();
        unchecked_source(&body, &state)
            .copy(
                context,
                &Transform::mirror(&Plane::YZ),
                Instance {
                    first: 1,
                    second: 0,
                },
            )
            .map(|_| ())
    }

    /// The source `body` in `state` without the checks of `Source::new`, for the copy's own
    /// checks.
    fn unchecked_source<'a>(body: &'a Id, state: &'a BodyState) -> Source<'a> {
        Source {
            body,
            state,
            shape: state.shape().expect("the state holds a shape"),
        }
    }

    /// `BodyNotValid` of the source `b-1` of the copy feature, with the solid from `made_by`.
    fn not_valid(
        context: &BuildContext<'_>,
        made_by: &str,
        cause: Option<KernelError>,
    ) -> FeatureError {
        FeatureError::BodyNotValid {
            field: context.field("body"),
            body: id("b-1"),
            made_by: id(made_by),
            cause,
        }
    }

    /// The count field of a pattern, which is 1 when the pattern makes no copies.
    fn count() -> FieldRef {
        FieldRef {
            path: "count".to_string(),
            label: "Count".to_string(),
        }
    }

    /// `NoEffect` of a pattern whose count is 1 on `body` (features.md section 3.15).
    fn no_copies(body: &str) -> FeatureWarning {
        FeatureWarning::NoEffect {
            body: id(body),
            how: NoEffectHow::NoCopies {
                fields: vec![count()],
            },
            reverse_hits: false,
            attach: None,
        }
    }

    /// `BodyNotEmpty` of the target `b-2`, which names the target field (features.md section 4.1).
    fn target_not_empty(context: &BuildContext<'_>) -> FeatureError {
        FeatureError::BodyNotEmpty {
            body: id("b-2"),
            target: Some(context.field("target")),
            new_body: false,
        }
    }
}
