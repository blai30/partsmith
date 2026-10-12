//! The `boolean` feature: joins, cuts or intersects a target body with tool bodies, and empties
//! the tools unless they are kept (docs/spec/features/boolean.md).

use partsmith_model::{Category, FeatureManifest, Id, SelectionKind};
use partsmith_naming::NamingMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    bodies, essential, flag, hint, is_false, manifest, one_or_list, options_hint, part,
    selection_hint, solid_body, unchanged,
};
use crate::context::{BodyState, BuildContext};
use crate::deps::Dependencies;
use crate::error::{FeatureError, FieldRef, Noun};
use crate::feature::Feature;
use crate::output::{BodyChange, FeatureOutput, Tool, apply_op};
use crate::params::BodyOp;
use crate::python::{PythonCall, PythonNames, PythonValue};

/// Joins, cuts or intersects a target body with tool bodies.
#[derive(Clone, Copy, Debug, Default)]
pub struct Boolean;

/// Params of a boolean feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BooleanParams {
    /// The body that receives the result.
    pub target: Id,
    /// Tool bodies, at least one, distinct, not the target.
    pub tools: Vec<Id>,
    /// The operation. Required.
    pub op: BooleanOp,
    /// Leave the tools unchanged instead of consuming them. Default false, omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub keep_tools: bool,
}

/// The operation of a boolean feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BooleanOp {
    /// The target joined with every tool (kernel `fuse`).
    Join,
    /// The target minus the union of the tools (kernel `cut`).
    Cut,
    /// The part of the target inside every tool (kernel `common`).
    Intersect,
}

impl BooleanOp {
    /// The stored name, which Python also writes.
    fn name(self) -> &'static str {
        match self {
            BooleanOp::Join => "join",
            BooleanOp::Cut => "cut",
            BooleanOp::Intersect => "intersect",
        }
    }

    /// The body op of the same name, which `apply_op` applies to the target.
    fn body_op(self) -> BodyOp {
        match self {
            BooleanOp::Join => BodyOp::Join,
            BooleanOp::Cut => BodyOp::Cut,
            BooleanOp::Intersect => BodyOp::Intersect,
        }
    }
}

impl Feature for Boolean {
    const TYPE: &'static str = "boolean";
    type Params = BooleanParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Boolean",
            Category::Modify,
            "boolean",
            vec![
                essential(selection_hint("target", "Target body", SelectionKind::Body)),
                essential(selection_hint("tools", "Tool bodies", SelectionKind::Body)),
                essential(options_hint(
                    "op",
                    "Operation",
                    &[("join", "Join"), ("cut", "Cut"), ("intersect", "Intersect")],
                )),
                essential(hint("keep_tools", "Keep tools")),
            ],
        )
    }

    /// At least one tool, and no tool that is the target or repeats (boolean.md section 3.1).
    fn validate(params: &BooleanParams) -> Result<(), FeatureError> {
        let manifest = Boolean::manifest();
        if params.tools.is_empty() {
            return Err(FeatureError::EmptySelection {
                field: FieldRef::new("tools", &manifest),
                noun: Noun::Body,
            });
        }
        for (position, tool) in params.tools.iter().enumerate() {
            if *tool == params.target || params.tools[..position].contains(tool) {
                return Err(FeatureError::SameBody {
                    field: FieldRef::new(&format!("tools[{position}]"), &manifest),
                    body: tool.clone(),
                });
            }
        }
        Ok(())
    }

    /// Writes the target, then the tools it consumes; kept tools are only read (boolean.md
    /// section 3.2).
    fn dependencies(params: &BooleanParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_body_written(&params.target, "target");
        for (position, tool) in params.tools.iter().enumerate() {
            let field = format!("tools[{position}]");
            if params.keep_tools {
                dependencies.add_body_read(tool, &field);
            } else {
                dependencies.add_body_written(tool, &field);
            }
        }
        dependencies
    }

    /// The target with every tool body as a tool through `apply_op`, which makes one kernel call
    /// (boolean.md section 3.3). The target's update comes first, then a `Clear` update for each
    /// consumed tool, even when the operation had no effect.
    fn build(
        params: &BooleanParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        solid_body(context, &params.target)?;
        let mut tools = Vec::with_capacity(params.tools.len());
        for tool in &params.tools {
            let state = context.body(tool)?;
            let shape = state
                .shape()
                .ok_or_else(|| FeatureError::ToolEmpty { body: tool.clone() })?;
            tools.push(Tool {
                shape: shape.clone(),
                naming: NamingMap::clone(state.naming()),
            });
        }
        let (update, warnings) = apply_op(context, &params.target, params.op.body_op(), tools)?;
        let mut updates = vec![update];
        if !params.keep_tools {
            let empty = BodyState::empty();
            updates.extend(
                params
                    .tools
                    .iter()
                    .map(|tool| unchanged(tool, &empty, BodyChange::Clear)),
            );
        }
        Ok(bodies(updates, warnings))
    }

    /// `boolean(tools, op=..., keep_tools=True)` (boolean.md section 2): the tools' parts, one
    /// alone or several as a list, `op` when it is not `cut` (the function's own default), and
    /// `keep_tools=True` when set. None when a tool has no part variable.
    fn python(params: &BooleanParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let tools = params
            .tools
            .iter()
            .map(|tool| part(names, tool))
            .collect::<Option<Vec<_>>>()?;
        let mut call = PythonCall::new("boolean");
        call.positional.push(one_or_list(tools));
        if params.op != BooleanOp::Cut {
            call.keywords.push((
                "op".to_string(),
                PythonValue::Str(params.op.name().to_string()),
            ));
        }
        if params.keep_tools {
            call.keywords.push(flag("keep_tools"));
        }
        Some(call)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use partsmith_model::FeatureRecord;
    use partsmith_naming::ElementKind;
    use serde_json::{Value, json};

    use super::*;
    use crate::builtin::copy::testing::{
        assert_bounds, assert_volume, build, params, python, python_with, update, updated_shape,
        updates,
    };
    use crate::error::FeatureWarning;
    use crate::registry::Registry;
    use crate::test_support::{ScriptNames, box_body, face_names, id};

    fn boolean(tools: Value, op: &str) -> BooleanParams {
        params::<Boolean>(json!({
            "id": "f-bool1", "type": "boolean", "target": "b-1", "tools": tools, "op": op
        }))
    }

    /// `b-1` spanning [0,0,0]-[20,20,20], `b-2` spanning [10,0,0]-[30,20,20] and `b-3` spanning
    /// [5,5,15]-[15,15,25].
    fn three_boxes() -> Vec<(&'static str, BodyState)> {
        vec![
            ("b-1", box_body("f-box1", [0.0, 0.0, 0.0], 20.0)),
            ("b-2", box_body("f-box2", [10.0, 0.0, 0.0], 20.0)),
            ("b-3", box_body("f-box3", [5.0, 5.0, 15.0], 10.0)),
        ]
    }

    fn run(
        params: &BooleanParams,
        bodies: Vec<(&str, BodyState)>,
    ) -> Result<FeatureOutput, FeatureError> {
        build::<Boolean>("f-bool1", params, bodies, Vec::new())
    }

    #[test]
    fn registers_with_its_manifest() {
        let mut registry = Registry::new();

        registry
            .register::<Boolean>()
            .expect("the boolean passes the registration checks");

        assert_eq!(registry.types().collect::<Vec<_>>(), ["boolean"]);
    }

    #[test]
    fn canonical_params_omit_keep_tools_when_false() {
        let mut registry = Registry::new();
        registry.register::<Boolean>().expect("registers");
        let record: FeatureRecord = serde_json::from_value(json!({
            "id": "f-bool1", "type": "boolean", "keep_tools": false, "op": "cut",
            "tools": ["b-2", "b-3"], "target": "b-1"
        }))
        .expect("the record is well-formed");

        let validated = registry
            .validate_and_canonicalize(&record)
            .expect("the record is valid");

        assert_eq!(
            serde_json::to_string(&validated.record.params).expect("serializes"),
            r#"{"target":"b-1","tools":["b-2","b-3"],"op":"cut"}"#
        );
    }

    #[test]
    fn no_tools_is_an_empty_selection() {
        let error = Boolean::validate(&boolean(json!([]), "join")).expect_err("no tools");

        assert_eq!(
            error.to_string(),
            "Select at least one body for Tool bodies."
        );
    }

    #[test]
    fn the_target_as_a_tool_is_the_same_body() {
        let error = Boolean::validate(&boolean(json!(["b-2", "b-1"]), "join"))
            .expect_err("the target is a tool");

        assert_eq!(error.field().expect("names a field").path, "tools[1]");
        assert_eq!(
            error.to_string(),
            "Body 'b-1' cannot be used twice here. Pick a different body for Tool bodies."
        );
    }

    #[test]
    fn a_repeated_tool_is_the_same_body_at_its_second_entry() {
        let error = Boolean::validate(&boolean(json!(["b-2", "b-3", "b-2"]), "join"))
            .expect_err("b-2 repeats");

        assert_eq!(
            error,
            FeatureError::SameBody {
                field: FieldRef::new("tools[2]", &Boolean::manifest()),
                body: id("b-2"),
            }
        );
    }

    #[test]
    fn consumed_tools_are_written_after_the_target() {
        let dependencies = Boolean::dependencies(&boolean(json!(["b-2", "b-3"]), "cut"));

        let written: Vec<(&str, &str)> = dependencies
            .bodies_written
            .iter()
            .map(|link| (link.id.as_str(), link.field.as_str()))
            .collect();
        assert_eq!(
            written,
            [("b-1", "target"), ("b-2", "tools[0]"), ("b-3", "tools[1]")]
        );
        assert!(dependencies.bodies_read.is_empty());
        assert!(dependencies.expressions.is_empty());
    }

    #[test]
    fn kept_tools_are_only_read() {
        let mut params = boolean(json!(["b-2"]), "join");
        params.keep_tools = true;

        let dependencies = Boolean::dependencies(&params);

        assert_eq!(dependencies.bodies_written.len(), 1);
        assert_eq!(dependencies.bodies_read[0].id, id("b-2"));
        assert_eq!(dependencies.bodies_read[0].field, "tools[0]");
    }

    #[test]
    fn join_adds_the_tool_and_clears_it() {
        let output = run(&boolean(json!(["b-2"]), "join"), three_boxes()).expect("the join builds");

        let changes: Vec<(String, BodyChange, bool)> = updates(&output)
            .iter()
            .map(|update| {
                (
                    update.body.to_string(),
                    update.change,
                    update.shape.is_some(),
                )
            })
            .collect();
        assert_eq!(
            changes,
            [
                ("b-1".to_string(), BodyChange::Join, true),
                ("b-2".to_string(), BodyChange::Clear, false)
            ]
        );
        assert_volume(updated_shape(&output, "b-1"), 12000.0);
        let names = face_names(&update(&output, "b-1").naming);
        assert!(names.contains(&"f-box1/left".to_string()));
        assert!(names.contains(&"f-box2/right".to_string()));
        assert_eq!(update(&output, "b-2").naming.count(ElementKind::Face), 0);
    }

    #[test]
    fn kept_tools_get_no_update() {
        let mut params = boolean(json!(["b-2"]), "join");
        params.keep_tools = true;

        let output = run(&params, three_boxes()).expect("the join builds");

        assert_eq!(updates(&output).len(), 1);
    }

    #[test]
    fn cut_names_the_new_wall_after_the_tool_face() {
        let output = run(&boolean(json!(["b-2"]), "cut"), three_boxes()).expect("the cut builds");

        assert_volume(updated_shape(&output, "b-1"), 4000.0);
        assert!(face_names(&update(&output, "b-1").naming).contains(&"f-box2/left".to_string()));
    }

    #[test]
    fn intersect_with_two_tools_keeps_what_is_inside_both() {
        let output = run(&boolean(json!(["b-2", "b-3"]), "intersect"), three_boxes())
            .expect("the intersection builds");

        let shape = updated_shape(&output, "b-1");
        assert_volume(shape, 250.0);
        assert_bounds(shape, [10.0, 5.0, 15.0], [15.0, 15.0, 20.0]);
        assert_eq!(update(&output, "b-1").change, BodyChange::Intersect);
    }

    #[test]
    fn intersect_with_a_tool_that_misses_is_an_empty_result() {
        let mut bodies = three_boxes();
        bodies[2] = ("b-3", box_body("f-box3", [100.0, 5.0, 15.0], 10.0));

        let result = run(&boolean(json!(["b-2", "b-3"]), "intersect"), bodies);

        assert_eq!(
            result.map(|_| ()),
            Err(FeatureError::EmptyResult {
                body: id("b-1"),
                op: BodyOp::Intersect
            })
        );
    }

    #[test]
    fn a_canceled_run_does_not_combine() {
        let params = boolean(json!(["b-2"]), "join");
        let fixture = crate::builtin::copy::testing::fixture::<Boolean>(
            "f-bool1",
            &params,
            three_boxes(),
            Vec::new(),
        );
        fixture.cancel.cancel();

        let result = Boolean::build(&params, &fixture.context());

        assert_eq!(result.map(|_| ()), Err(FeatureError::Canceled));
    }

    #[test]
    fn an_empty_target_has_nothing_to_work_on() {
        let bodies = vec![("b-2", box_body("f-box2", [0.0; 3], 10.0))];

        let result = run(&boolean(json!(["b-2"]), "join"), bodies);

        assert_eq!(
            result.map(|_| ()),
            Err(FeatureError::BodyEmpty {
                body: id("b-1"),
                use_new: false
            })
        );
    }

    #[test]
    fn an_empty_tool_cannot_be_used() {
        let bodies = vec![("b-1", box_body("f-box1", [0.0; 3], 10.0))];

        let result = run(&boolean(json!(["b-2"]), "join"), bodies);

        assert_eq!(
            result.map(|_| ()),
            Err(FeatureError::ToolEmpty { body: id("b-2") })
        );
    }

    #[test]
    fn a_cut_that_misses_keeps_the_target_and_still_clears_the_tool() {
        let target = box_body("f-box1", [0.0; 3], 10.0);
        let bodies = vec![
            ("b-1", target.clone()),
            ("b-2", box_body("f-box2", [50.0, 0.0, 0.0], 10.0)),
        ];

        let output = run(&boolean(json!(["b-2"]), "cut"), bodies).expect("the cut builds");

        assert_eq!(
            output.warnings,
            [FeatureWarning::NoEffect { body: id("b-1") }]
        );
        let kept = update(&output, "b-1");
        assert_eq!(kept.change, BodyChange::Cut);
        assert!(Arc::ptr_eq(&kept.naming, target.naming()));
        assert_eq!(update(&output, "b-2").change, BodyChange::Clear);
    }

    #[test]
    fn a_cut_of_everything_is_an_empty_result() {
        let bodies = vec![
            ("b-1", box_body("f-box1", [0.0; 3], 10.0)),
            ("b-2", box_body("f-box2", [-5.0, -5.0, -5.0], 20.0)),
        ];

        let result = run(&boolean(json!(["b-2"]), "cut"), bodies);

        assert_eq!(
            result.map(|_| ()),
            Err(FeatureError::EmptyResult {
                body: id("b-1"),
                op: BodyOp::Cut
            })
        );
    }

    #[test]
    fn python_writes_one_tool_alone_and_omits_cut() {
        assert_eq!(
            python::<Boolean>(&boolean(json!(["b-2"]), "cut")).as_deref(),
            Some("boolean(part_2)")
        );
    }

    #[test]
    fn python_writes_several_tools_as_a_list_with_op_and_keep_tools() {
        let mut params = boolean(json!(["b-2", "b-3"]), "join");
        params.keep_tools = true;

        assert_eq!(
            python::<Boolean>(&params).as_deref(),
            Some(r#"boolean([part_2, part_3], op="join", keep_tools=True)"#)
        );
    }

    #[test]
    fn python_needs_every_tool_part() {
        let mut names = ScriptNames::default();
        names.bodies.remove(&id("b-3"));

        assert_eq!(
            python_with::<Boolean>(&boolean(json!(["b-2", "b-3"]), "cut"), &names),
            None
        );
    }
}
