//! What features produce, and the shared body semantics of solid features (features.md sections 2.7 and 3.5).

use std::sync::Arc;

use partsmith_model::Id;
use partsmith_naming::{ElementName, Motion, NamingMap, TopologyInfo};
use partsmith_occt_sys::{
    InputHistory, KernelError, OpResult, Shape, SubShapeKind, common, cut, fuse,
};
use partsmith_sketch::{ORIGIN, Placement, PointRef, PointRole, Profile, SolveResult};

use crate::context::BuildContext;
use crate::error::FeatureError;
use crate::error::FeatureWarning;
use crate::naming::{has_element_counts, name_by_history};
use crate::params::BodyOp;

/// Relative volume tolerance of the body rules (section 3.5): a volume change, or a remaining
/// volume, of at most `VOLUME_TOLERANCE` times the body's volume counts as none.
pub const VOLUME_TOLERANCE: f64 = 1e-9;

/// A kernel boolean of a target with every tool (`fuse`, `cut` or `common`).
type KernelBoolean = fn(&Shape, &[Shape]) -> Result<OpResult, KernelError>;

/// What a feature produced, plus warnings.
#[derive(Clone, Debug)]
pub struct FeatureOutput {
    /// The produced data.
    pub data: OutputData,
    /// Warnings and info notices in the order they were raised.
    pub warnings: Vec<FeatureWarning>,
    /// Faces its naming rule would make that this build did not make, with why. Recompute's loss diagnosis reads
    /// them (recompute.md section 3.10). Empty for most features.
    pub absent: Vec<AbsentFace>,
}

/// A face a feature's naming rule names that this build did not make.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbsentFace {
    /// The face name it would have, for example `f-ext2/end-cap`.
    pub name: ElementName,
    /// Why it is absent.
    pub why: Absence,
}

/// Why a face is absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Absence {
    /// A cut or hole that goes all the way through the body: its end face and its start face (an extrude's start
    /// cap, a hole's entry) are both not in the result (extrude, hole).
    Through,
    /// A cut or hole whose end face is not in the result while its start face is: its end reaches the outside of
    /// the body, but it does not go all the way through (an extrude that starts inside the body, corpus R30 (a)).
    EndOutside,
    /// A revolve of a full turn has no caps.
    FullTurn,
}

/// The four kinds of feature output.
// A sketch result is larger than a list of updates, but an output is built once and shared through an `Arc`, and
// boxing would change the shape features.md section 2.7 gives.
#[expect(
    clippy::large_enum_variant,
    reason = "OutputData keeps the variants the spec gives; outputs are built once and shared"
)]
#[derive(Clone, Debug)]
pub enum OutputData {
    /// New states of some written bodies, in `bodies_written` order, at most one per body.
    Bodies(Vec<BodyUpdate>),
    /// A solved sketch.
    Sketch(SketchResult),
    /// Construction geometry.
    Datum(DatumResult),
    /// Nothing (a feature that only reports).
    Nothing,
}

/// The new state of one body.
#[derive(Clone, Debug)]
pub struct BodyUpdate {
    /// The body.
    pub body: Id,
    /// The new shape, or None when the body becomes empty.
    pub shape: Option<Shape>,
    /// Names of every face, edge and vertex of `shape`.
    pub naming: Arc<NamingMap>,
    /// What happened to the body.
    pub change: BodyChange,
    /// What the feature already measured on `shape`, which the body's next state reuses
    /// (`BodyState::from_update`).
    pub measured: Measurements,
}

/// Measurements of a `BodyUpdate`'s shape that the feature made anyway (naming a shape reads its
/// topology, the body rules of section 3.5 read its volume), carried to the body's next state so
/// that later features and hosts do not ask the kernel again. Each value must be exactly what
/// `BodyState::topology` or `BodyState::volume` would compute for the shape; a value the feature
/// did not measure is `None` and is computed when first needed.
#[derive(Clone, Debug, Default)]
pub struct Measurements {
    /// Topology and fingerprints of every element of the shape (`topology_info`).
    pub topology: Option<Arc<TopologyInfo>>,
    /// Volume of the shape in cubic millimeters.
    pub volume: Option<f64>,
}

/// What a feature did to a body (the architecture's "boolean op applied", extended).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BodyChange {
    /// The body was started (op new).
    New,
    /// The tool was added.
    Join,
    /// The tool was removed.
    Cut,
    /// Only the overlap was kept.
    Intersect,
    /// The body was changed in place (fillet, chamfer, shell, move, split).
    Modify,
    /// The body was emptied (a consumed boolean tool).
    Clear,
}

/// A solved sketch placed in space.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchResult {
    /// Placement of the sketch plane in world space.
    pub placement: Placement,
    /// The motion of the geometry the sketch is placed on (naming.md section 3.6, `new_motion`), which features
    /// that build faces from the sketch name them with (features/sketch.md section 3.3).
    pub motion: Motion,
    /// Solved entity geometry and solver status (sketch.md `SolveResult`).
    pub solved: SolveResult,
    /// Closed regions and profile findings of the solved entities (sketch.md `Profile`).
    pub profile: Profile,
}

impl SketchResult {
    /// The solved 2D position of a sketch point, or None if the point does not exist. The reserved
    /// `origin` is the point (0, 0).
    pub fn point(&self, point: &PointRef) -> Option<[f64; 2]> {
        if point.entity.as_str() == ORIGIN {
            return (point.role == PointRole::Point).then_some([0.0, 0.0]);
        }
        self.solved
            .entities
            .iter()
            .find(|entity| entity.id == point.entity)?
            .point(point.role)
    }

    /// The world position of sketch coordinates.
    pub fn to_world(&self, local: [f64; 2]) -> [f64; 3] {
        self.placement.to_world(local)
    }
}

/// Construction geometry produced by a datum feature.
#[derive(Clone, Debug, PartialEq)]
pub enum DatumResult {
    /// A plane.
    Plane(Placement),
}

/// A shape a feature made, with names for its elements, before it is combined with a body.
#[derive(Clone, Debug)]
pub struct Tool {
    /// The tool solid or compound of solids. Never empty.
    pub shape: Shape,
    /// Names of the tool's elements.
    pub naming: NamingMap,
}

/// Applies `op` with `tools` to `body` with the shared body semantics (section 3.5). At least one tool.
/// New with several tools fuses them (kernel `fuse(tools[0], rest)`). Join, Cut and Intersect pass every tool to one
/// kernel `fuse`, `cut` or `common` call; Intersect keeps what lies inside the body and every tool. No tool is a bug
/// in the feature and returns `Internal`. Returns the update and any NoEffect warning.
pub fn apply_op(
    context: &BuildContext<'_>,
    body: &Id,
    op: BodyOp,
    tools: Vec<Tool>,
) -> Result<(BodyUpdate, Vec<FeatureWarning>), FeatureError> {
    if tools.is_empty() {
        return Err(FeatureError::internal("apply_op needs at least one tool"));
    }
    let state = context.body(body)?;
    let boolean: KernelBoolean = match op {
        BodyOp::New => {
            if !state.is_empty() {
                return Err(FeatureError::BodyNotEmpty { body: body.clone() });
            }
            let (shape, naming, topology) = combine_tools(context, tools)?;
            let update = BodyUpdate {
                body: body.clone(),
                shape: Some(shape),
                naming: Arc::new(naming),
                change: BodyChange::New,
                measured: Measurements {
                    topology,
                    volume: None,
                },
            };
            return Ok((update, Vec::new()));
        }
        BodyOp::Join => fuse,
        BodyOp::Cut => cut,
        BodyOp::Intersect => common,
    };
    let Some(target) = state.shape() else {
        return Err(FeatureError::BodyEmpty {
            field: context.field("body"),
            body: body.clone(),
            use_new: true,
        });
    };
    context.check_canceled()?;
    let tool_shapes: Vec<Shape> = tools.iter().map(|tool| tool.shape.clone()).collect();
    let result = boolean(target, &tool_shapes)?;
    apply_op_result(context, body, op, &tools, result)
}

/// Applies the body semantics of `op` (section 3.5) to `result`, the kernel `fuse`, `cut` or
/// `common` of the body's shape with the shapes of `tools` in order, exactly as `apply_op` does
/// after its kernel call. For a feature that reads the kernel result before deciding (hole reads
/// from the cut's history which tools overlap the body). Op new, an empty body and a result whose
/// history does not have one input per shape, the body's and then each tool's, with that shape's
/// face, edge and vertex counts return `Internal`.
pub fn apply_op_result(
    context: &BuildContext<'_>,
    body: &Id,
    op: BodyOp,
    tools: &[Tool],
    result: OpResult,
) -> Result<(BodyUpdate, Vec<FeatureWarning>), FeatureError> {
    let change = match op {
        BodyOp::New => {
            return Err(FeatureError::internal(
                "apply_op_result applies join, cut or intersect, not op new",
            ));
        }
        BodyOp::Join => BodyChange::Join,
        BodyOp::Cut => BodyChange::Cut,
        BodyOp::Intersect => BodyChange::Intersect,
    };
    let state = context.body(body)?;
    let Some(target) = state.shape() else {
        return Err(FeatureError::internal(
            "apply_op_result needs a body that holds a solid",
        ));
    };
    if result.history.inputs.len() != tools.len() + 1 {
        return Err(FeatureError::internal(format!(
            "the kernel result has {} history inputs, not the body and {} tools",
            result.history.inputs.len(),
            tools.len()
        )));
    }
    let shapes = std::iter::once(target).chain(tools.iter().map(|tool| &tool.shape));
    for (position, (input, shape)) in result.history.inputs.iter().zip(shapes).enumerate() {
        if !history_fits(input, shape) {
            return Err(FeatureError::internal(format!(
                "history input {position} of the kernel result is not the history of {}",
                if position == 0 {
                    "the body".to_owned()
                } else {
                    format!("tool {position}")
                }
            )));
        }
    }
    let before = state.volume()?;
    let after = result.shape.volume()?;
    let nothing_left =
        result.shape.count(SubShapeKind::Solid) == 0 || after <= VOLUME_TOLERANCE * before;
    if op != BodyOp::Join && nothing_left {
        return Err(FeatureError::EmptyResult {
            body: body.clone(),
            op,
        });
    }
    if (after - before).abs() <= VOLUME_TOLERANCE * before {
        let update = BodyUpdate {
            body: body.clone(),
            shape: Some(target.clone()),
            naming: Arc::clone(state.naming()),
            change,
            measured: state.measurements(),
        };
        return Ok((
            update,
            vec![FeatureWarning::NoEffect { body: body.clone() }],
        ));
    }
    let mut inputs: Vec<&NamingMap> = vec![state.naming()];
    inputs.extend(tools.iter().map(|tool| &tool.naming));
    let named = name_by_history(context.feature(), &result, &inputs, &Motion::IDENTITY)?;
    let update = BodyUpdate {
        body: body.clone(),
        shape: Some(result.shape),
        naming: Arc::new(named.naming),
        change,
        measured: Measurements {
            topology: Some(named.topology),
            volume: Some(after),
        },
    };
    Ok((update, Vec::new()))
}

/// True when `input` has one entry per face, edge and vertex of `shape`.
fn history_fits(input: &InputHistory, shape: &Shape) -> bool {
    has_element_counts(
        shape,
        input.faces.len(),
        input.edges.len(),
        input.vertices.len(),
    )
}

/// The shape and names of the tools fused into one: the only tool as it is (its topology unknown
/// here), or several tools fused with one kernel call and named by the boolean rule, with the
/// topology that naming read.
fn combine_tools(
    context: &BuildContext<'_>,
    mut tools: Vec<Tool>,
) -> Result<(Shape, NamingMap, Option<Arc<TopologyInfo>>), FeatureError> {
    if tools.len() == 1 {
        let tool = tools.remove(0);
        return Ok((tool.shape, tool.naming, None));
    }
    context.check_canceled()?;
    let shapes: Vec<Shape> = tools.iter().map(|tool| tool.shape.clone()).collect();
    let result = fuse(&shapes[0], &shapes[1..])?;
    let inputs: Vec<&NamingMap> = tools.iter().map(|tool| &tool.naming).collect();
    let named = name_by_history(context.feature(), &result, &inputs, &Motion::IDENTITY)?;
    Ok((result.shape, named.naming, Some(named.topology)))
}

#[cfg(test)]
mod tests {
    use partsmith_sketch::{OriginPlane, build_profile};
    use serde_json::json;

    use super::*;
    use crate::context::BodyState;
    use crate::error::FieldRef;
    use crate::naming::topology_info;
    use crate::test_support::{Fixture, box_body, face_names, id, named_box, volume};

    /// Applies `op` with `tools` to `b-1` in a fixture holding `state` as `b-1`.
    fn apply(
        state: BodyState,
        op: BodyOp,
        tools: Vec<Tool>,
    ) -> Result<(BodyUpdate, Vec<FeatureWarning>), FeatureError> {
        let fixture = Fixture::new("f-op1").with_body("b-1", state);
        apply_op(&fixture.context(), &id("b-1"), op, tools)
    }

    fn shape(update: &BodyUpdate) -> &Shape {
        update.shape.as_ref().expect("the body holds a shape")
    }

    #[test]
    fn new_on_an_empty_body_is_the_tool() {
        let tool = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);

        let (update, warnings) =
            apply(BodyState::empty(), BodyOp::New, vec![tool]).expect("op new builds");

        assert_eq!(update.change, BodyChange::New);
        assert_eq!(update.body, id("b-1"));
        assert!((volume(shape(&update)) - 1000.0).abs() < 1e-6);
        assert!(face_names(&update.naming).contains(&"f-box1/top".to_string()));
        assert!(warnings.is_empty());
    }

    #[test]
    fn new_with_several_tools_fuses_them() {
        let tools = vec![
            named_box("f-a", [0.0, 0.0, 0.0], 10.0),
            named_box("f-b", [20.0, 0.0, 0.0], 10.0),
        ];

        let (update, _) = apply(BodyState::empty(), BodyOp::New, tools).expect("op new builds");

        assert!((volume(shape(&update)) - 2000.0).abs() < 1e-6);
        assert_eq!(
            shape(&update).count(SubShapeKind::Solid),
            2,
            "two disjoint solids"
        );
        assert_eq!(face_names(&update.naming).len(), 12);
    }

    #[test]
    fn new_on_a_body_with_a_solid_is_body_not_empty() {
        let tool = named_box("f-box2", [0.0, 0.0, 0.0], 10.0);

        let result = apply(
            box_body("f-box1", [0.0, 0.0, 0.0], 10.0),
            BodyOp::New,
            vec![tool],
        );

        assert_eq!(
            result.map(|_| ()),
            Err(FeatureError::BodyNotEmpty { body: id("b-1") })
        );
    }

    #[test]
    fn join_on_an_empty_body_suggests_op_new() {
        let tool = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);

        let result = apply(BodyState::empty(), BodyOp::Join, vec![tool]);

        assert_eq!(
            result.map(|_| ()),
            Err(FeatureError::BodyEmpty {
                field: FieldRef {
                    path: "body".to_string(),
                    label: "body".to_string(),
                },
                body: id("b-1"),
                use_new: true
            })
        );
    }

    #[test]
    fn join_of_a_disjoint_tool_adds_a_lump() {
        let tool = named_box("f-box2", [20.0, 0.0, 0.0], 10.0);

        let (update, warnings) = apply(
            box_body("f-box1", [0.0, 0.0, 0.0], 10.0),
            BodyOp::Join,
            vec![tool],
        )
        .expect("join builds");

        assert_eq!(update.change, BodyChange::Join);
        assert!((volume(shape(&update)) - 2000.0).abs() < 1e-6);
        assert_eq!(shape(&update).count(SubShapeKind::Face), 12);
        assert!(warnings.is_empty());
    }

    #[test]
    fn cut_removes_the_overlap_and_names_new_faces_from_the_tool() {
        let tool = named_box("f-box2", [5.0, 5.0, 5.0], 10.0);

        let (update, _) = apply(
            box_body("f-box1", [0.0, 0.0, 0.0], 10.0),
            BodyOp::Cut,
            vec![tool],
        )
        .expect("cut builds");

        assert_eq!(update.change, BodyChange::Cut);
        assert!((volume(shape(&update)) - 875.0).abs() < 1e-6);
        let names = face_names(&update.naming);
        assert!(names.contains(&"f-box2/bottom".to_string()));
        assert!(names.contains(&"f-box1/bottom".to_string()));
    }

    #[test]
    fn cut_of_everything_is_an_empty_result() {
        let tool = named_box("f-box2", [-5.0, -5.0, -5.0], 20.0);

        let result = apply(
            box_body("f-box1", [0.0, 0.0, 0.0], 10.0),
            BodyOp::Cut,
            vec![tool],
        );

        assert_eq!(
            result.map(|_| ()),
            Err(FeatureError::EmptyResult {
                body: id("b-1"),
                op: BodyOp::Cut
            })
        );
    }

    #[test]
    fn intersect_without_overlap_is_an_empty_result() {
        let tool = named_box("f-box2", [100.0, 0.0, 0.0], 10.0);

        let result = apply(
            box_body("f-box1", [0.0, 0.0, 0.0], 10.0),
            BodyOp::Intersect,
            vec![tool],
        );

        assert_eq!(
            result.map(|_| ()),
            Err(FeatureError::EmptyResult {
                body: id("b-1"),
                op: BodyOp::Intersect
            })
        );
    }

    #[test]
    fn intersect_keeps_the_overlap() {
        let tool = named_box("f-box2", [5.0, 0.0, 0.0], 10.0);

        let (update, _) = apply(
            box_body("f-box1", [0.0, 0.0, 0.0], 10.0),
            BodyOp::Intersect,
            vec![tool],
        )
        .expect("intersect builds");

        assert_eq!(update.change, BodyChange::Intersect);
        assert!((volume(shape(&update)) - 500.0).abs() < 1e-6);
    }

    #[test]
    fn a_cut_that_misses_keeps_the_body_exactly() {
        let body = box_body("f-box1", [0.0, 0.0, 0.0], 10.0);
        let tool = named_box("f-box2", [100.0, 0.0, 0.0], 2.0);

        let (update, warnings) =
            apply(body.clone(), BodyOp::Cut, vec![tool]).expect("the cut builds");

        assert_eq!(update.change, BodyChange::Cut);
        assert!(Arc::ptr_eq(&update.naming, body.naming()));
        assert_eq!(shape(&update).count(SubShapeKind::Face), 6);
        assert_eq!(warnings, [FeatureWarning::NoEffect { body: id("b-1") }]);
    }

    #[test]
    fn a_join_inside_the_body_has_no_effect() {
        let tool = named_box("f-box2", [4.0, 4.0, 4.0], 2.0);

        let (_, warnings) = apply(
            box_body("f-box1", [0.0, 0.0, 0.0], 10.0),
            BodyOp::Join,
            vec![tool],
        )
        .expect("the join builds");

        assert_eq!(warnings, [FeatureWarning::NoEffect { body: id("b-1") }]);
    }

    #[test]
    fn no_tool_is_internal_for_every_op() {
        for op in [BodyOp::New, BodyOp::Join, BodyOp::Cut, BodyOp::Intersect] {
            let result = apply(box_body("f-box1", [0.0, 0.0, 0.0], 10.0), op, Vec::new());

            assert_eq!(
                result.map(|_| ()).map_err(|error| error.code()),
                Err("internal"),
                "{op:?}"
            );
        }
    }

    #[test]
    fn join_with_several_tools_adds_them_all() {
        let tools = vec![
            named_box("f-a", [20.0, 0.0, 0.0], 10.0),
            named_box("f-b", [40.0, 0.0, 0.0], 10.0),
        ];

        let (update, warnings) = apply(
            box_body("f-box1", [0.0, 0.0, 0.0], 10.0),
            BodyOp::Join,
            tools,
        )
        .expect("join builds");

        assert_eq!(update.change, BodyChange::Join);
        assert!((volume(shape(&update)) - 3000.0).abs() < 1e-6);
        assert_eq!(shape(&update).count(SubShapeKind::Solid), 3);
        assert!(warnings.is_empty());
    }

    #[test]
    fn cut_with_several_tools_removes_them_all() {
        let tools = vec![
            named_box("f-a", [5.0, 5.0, 5.0], 10.0),
            named_box("f-b", [-5.0, -5.0, -5.0], 10.0),
        ];

        let (update, _) = apply(
            box_body("f-box1", [0.0, 0.0, 0.0], 10.0),
            BodyOp::Cut,
            tools,
        )
        .expect("cut builds");

        assert!((volume(shape(&update)) - 750.0).abs() < 1e-6);
        let names = face_names(&update.naming);
        assert!(names.contains(&"f-a/bottom".to_string()));
        assert!(names.contains(&"f-b/top".to_string()));
    }

    #[test]
    fn intersect_with_several_tools_keeps_what_is_inside_every_tool() {
        let tools = vec![
            named_box("f-a", [5.0, 0.0, 0.0], 10.0),
            named_box("f-b", [0.0, 0.0, 8.0], 10.0),
        ];

        let (update, warnings) = apply(
            box_body("f-box1", [0.0, 0.0, 0.0], 10.0),
            BodyOp::Intersect,
            tools,
        )
        .expect("intersect builds");

        assert_eq!(update.change, BodyChange::Intersect);
        assert!((volume(shape(&update)) - 100.0).abs() < 1e-6);
        assert_eq!(shape(&update).count(SubShapeKind::Face), 6);
        assert!(warnings.is_empty());
    }

    #[test]
    fn intersect_with_a_tool_that_misses_is_an_empty_result() {
        let tools = vec![
            named_box("f-a", [5.0, 0.0, 0.0], 10.0),
            named_box("f-b", [100.0, 0.0, 0.0], 10.0),
        ];

        let result = apply(
            box_body("f-box1", [0.0, 0.0, 0.0], 10.0),
            BodyOp::Intersect,
            tools,
        );

        assert_eq!(
            result.map(|_| ()),
            Err(FeatureError::EmptyResult {
                body: id("b-1"),
                op: BodyOp::Intersect
            })
        );
    }

    #[test]
    fn an_undeclared_body_is_undeclared() {
        let fixture = Fixture::new("f-op1");
        let tool = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);

        let result = apply_op(&fixture.context(), &id("b-9"), BodyOp::New, vec![tool]);

        assert_eq!(
            result.map(|_| ()).map_err(|error| error.code()),
            Err("undeclared_dependency")
        );
    }

    #[test]
    fn a_canceled_run_does_not_combine() {
        let fixture = Fixture::new("f-op1").with_body("b-1", box_body("f-box1", [0.0; 3], 10.0));
        fixture.cancel.cancel();
        let tool = named_box("f-box2", [5.0, 0.0, 0.0], 10.0);

        let result = apply_op(&fixture.context(), &id("b-1"), BodyOp::Join, vec![tool]);

        assert_eq!(result.map(|_| ()), Err(FeatureError::Canceled));
    }

    #[test]
    fn a_boolean_carries_the_volume_and_topology_of_its_result() {
        let tool = named_box("f-box2", [5.0, 5.0, 5.0], 10.0);

        let (update, _) = apply(
            box_body("f-box1", [0.0, 0.0, 0.0], 10.0),
            BodyOp::Cut,
            vec![tool],
        )
        .expect("cut builds");

        let result = shape(&update);
        assert_eq!(update.measured.volume, Some(volume(result)));
        assert_eq!(
            update.measured.topology.as_deref(),
            Some(&topology_info(result).expect("the kernel topology"))
        );
    }

    #[test]
    fn op_new_with_several_tools_carries_the_topology_of_their_fusion() {
        let tools = vec![
            named_box("f-a", [0.0, 0.0, 0.0], 10.0),
            named_box("f-b", [20.0, 0.0, 0.0], 10.0),
        ];

        let (update, _) = apply(BodyState::empty(), BodyOp::New, tools).expect("op new builds");

        assert_eq!(
            update
                .measured
                .topology
                .map(|topology| topology.faces.len()),
            Some(12)
        );
        assert_eq!(update.measured.volume, None);
    }

    #[test]
    fn an_update_without_effect_carries_what_the_body_measured() {
        let body = box_body("f-box1", [0.0, 0.0, 0.0], 10.0);
        body.elements().expect("element infos");
        let tool = named_box("f-box2", [100.0, 0.0, 0.0], 2.0);

        let (update, _) = apply(body.clone(), BodyOp::Cut, vec![tool]).expect("the cut builds");

        let topology = update.measured.topology.expect("the body's topology");
        assert!(Arc::ptr_eq(
            &topology,
            &body.topology().expect("the body's topology")
        ));
        assert_eq!(
            update.measured.volume,
            Some(body.volume().expect("the body's volume"))
        );
    }

    #[test]
    fn a_kernel_result_gets_the_body_rules_of_apply_op() {
        let state = box_body("f-box1", [0.0, 0.0, 0.0], 10.0);
        let fixture = Fixture::new("f-op1").with_body("b-1", state.clone());
        let tools = vec![named_box("f-box2", [5.0, 5.0, 5.0], 10.0)];
        let result = cut(
            state.shape().expect("a solid"),
            std::slice::from_ref(&tools[0].shape),
        )
        .expect("the kernel cuts");

        let (update, warnings) =
            apply_op_result(&fixture.context(), &id("b-1"), BodyOp::Cut, &tools, result)
                .expect("the rules apply");
        let (expected, _) = apply(state, BodyOp::Cut, tools).expect("cut builds");

        assert_eq!(update.change, BodyChange::Cut);
        assert!(warnings.is_empty());
        assert_eq!(volume(shape(&update)), volume(shape(&expected)));
        assert_eq!(face_names(&update.naming), face_names(&expected.naming));
    }

    #[test]
    fn a_kernel_result_for_op_new_or_without_every_tool_is_internal() {
        let state = box_body("f-box1", [0.0, 0.0, 0.0], 10.0);
        let fixture = Fixture::new("f-op1").with_body("b-1", state.clone());
        let tools = vec![
            named_box("f-a", [5.0, 5.0, 5.0], 10.0),
            named_box("f-b", [-5.0, -5.0, -5.0], 10.0),
        ];
        let target = state.shape().expect("a solid");
        let with_one = || cut(target, std::slice::from_ref(&tools[0].shape)).expect("cuts");
        let apply_result = |op, result| {
            apply_op_result(&fixture.context(), &id("b-1"), op, &tools, result)
                .map(|_| ())
                .map_err(|error| error.code())
        };

        assert_eq!(apply_result(BodyOp::Cut, with_one()), Err("internal"));
        assert_eq!(apply_result(BodyOp::New, with_one()), Err("internal"));
    }

    #[test]
    fn sketch_points_and_world_positions() {
        let entities = serde_json::from_value(json!([
            {"id": "e-1", "type": "line", "start": [1, 2], "end": [3, 4]},
            {"id": "e-p1", "type": "point", "position": [5, 6]}
        ]))
        .expect("the entities deserialize");
        let solved: SolveResult = serde_json::from_value(json!({
            "status": "under_constrained",
            "failed": false,
            "entities": [],
            "dof": 6,
            "redundant": [],
            "conflicting": [],
            "entity_states": {},
            "measured": {}
        }))
        .expect("the solve result deserializes");
        let sketch = SketchResult {
            placement: OriginPlane::Xz.placement(),
            motion: Motion::IDENTITY,
            profile: build_profile(&[]),
            solved: SolveResult { entities, ..solved },
        };
        let point = |text: &str| text.parse::<PointRef>().expect("valid point reference");

        assert_eq!(sketch.point(&point("e-1.end")), Some([3.0, 4.0]));
        assert_eq!(sketch.point(&point("e-p1")), Some([5.0, 6.0]));
        assert_eq!(sketch.point(&point("origin")), Some([0.0, 0.0]));
        assert_eq!(sketch.point(&point("e-1.center")), None);
        assert_eq!(sketch.point(&point("e-9.start")), None);
        assert_eq!(sketch.to_world([10.0, 20.0]), [10.0, 0.0, 20.0]);
    }
}
