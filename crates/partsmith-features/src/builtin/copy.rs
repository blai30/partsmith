//! What the copy features `mirror`, `linear_pattern` and `circular_pattern` share: the rules for
//! `op` and `target`, the bodies they declare, copies of the source named by instance number, and
//! how the copies are combined (features.md section 3.5, mirror.md section 3, naming.md sections
//! 3.5.1, 3.6.10 and 3.6.11).

use partsmith_model::{FeatureManifest, FieldHint, Id};
use partsmith_naming::Instance;
use partsmith_occt_sys::{Shape, Transform, transform};

use super::common::{bodies, naming_motion, options_hint, solid_body, unchanged};
use crate::context::{BodyState, BuildContext};
use crate::deps::Dependencies;
use crate::error::{FeatureError, FeatureWarning, FieldRef, NoEffectHow};
use crate::output::{BodyChange, FeatureOutput, Tool, apply_op};
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
        let (state, shape) = solid_body(context, "body", body)?;
        Ok(Source { body, state, shape })
    }

    /// The source moved by `copy_transform`, named as instance `instance` of the feature being
    /// built: every source name with the suffix `@<feature>:<instance>` (naming.md section 3.5.1),
    /// with `copy_transform` as the motion of its faces. The kernel transform keeps element
    /// indices, so the instance map fits the copy as it is.
    pub(super) fn copy(
        &self,
        context: &BuildContext<'_>,
        copy_transform: &Transform,
        instance: Instance,
    ) -> Result<Tool, FeatureError> {
        let moved = transform(self.shape, copy_transform)?;
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
        warnings.extend(
            op_warnings
                .into_iter()
                .filter(|warning| !matches!(warning, FeatureWarning::SeparateSolid { .. })),
        );
        Ok(bodies(vec![update], warnings))
    }
}

#[cfg(test)]
mod tests {
    use partsmith_model::Category;
    use partsmith_naming::ElementKind;
    use partsmith_occt_sys::Vector;

    use super::*;
    use crate::builtin::common::{essential, manifest, selection_hint};
    use crate::test_support::{
        Fixture, assert_volume, box_body, face_names, id, named_box, update, updated_shape, updates,
    };

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
