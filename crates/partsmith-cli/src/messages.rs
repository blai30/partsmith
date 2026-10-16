//! Feature messages with their actions and candidates (cli.md section 2.2), shared by `validate`
//! and `rebuild`: the members they add to a diagnostic in JSON reports (sections 3.1 and 3.3), and
//! the action and candidate lines under the message in text reports.

use std::io;

use partsmith_engine::FeatureMessage;
use partsmith_model::Id;
use partsmith_protocol::{Diagnostic, FixAction, FixKind, LossReason, RepairCandidate};
use serde::Serialize;

use crate::output::Output;

/// What a feature message adds to its severity, code and text in JSON reports, in the order of
/// cli.md section 3.3, each member omitted when absent or empty. Text reports print its actions and
/// candidates under the message (`write_lines`); its `detail` appears only in JSON. The CLI opens
/// every document fresh, so a message has no cause line (engine.md section 3.21).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MessageMembers {
    /// JSON Pointer of the field the message is about, relative to the feature record.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// Replacement expression for `field` that fixes the problem.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// The one-click fixes, in the order offered.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<Action>,
    /// The possible replacements of the reference the message is about, in repair order, then
    /// candidate order, without their mesh element, so they serialize with exactly the members of
    /// section 3.3.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<RepairCandidate>,
    /// Why the reference was lost, or which feature a followed or widened reference followed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<LossReason>,
    /// The first failing feature of a chain, or the sketch that lost its face.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<Id>,
    /// The technical text for reports, never printed in text reports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// One action of a feature message in JSON reports: its label and what it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Action {
    /// The button label, for example "Pick an edge again".
    pub label: String,
    /// The `kind` of the action's run: `apply`, `pick` or `show` (engine.md section 3.22).
    pub kind: &'static str,
}

impl MessageMembers {
    /// The members of `diagnostic`, a feature message as engine.md section 3.4 projects it, with
    /// its `candidates`. Its cause, which a fresh document never has, is left out.
    pub fn new(diagnostic: Diagnostic, candidates: Vec<RepairCandidate>) -> MessageMembers {
        MessageMembers {
            field: diagnostic.field,
            suggestion: diagnostic.suggestion,
            actions: diagnostic.actions.into_iter().map(Action::from).collect(),
            candidates: candidates
                .into_iter()
                .map(|candidate| RepairCandidate {
                    element: None,
                    ..candidate
                })
                .collect(),
            reason: diagnostic.reason,
            root: diagnostic.root,
            detail: diagnostic.detail,
        }
    }

    /// Writes the lines under the message, each indented `indent` spaces: one `suggested: <label>`
    /// per action in the order offered, then one `candidate: <q>, <relation>` per candidate.
    pub fn write_lines(&self, output: &mut Output<'_>, indent: usize) -> io::Result<()> {
        for action in &self.actions {
            output.line(format_args!("{:indent$}suggested: {}", "", action.label))?;
        }
        for candidate in &self.candidates {
            output.line(format_args!(
                "{:indent$}candidate: {}, {}",
                "", candidate.reference.query, candidate.relation
            ))?;
        }
        Ok(())
    }
}

impl From<FeatureMessage> for MessageMembers {
    fn from(message: FeatureMessage) -> MessageMembers {
        MessageMembers::new(message.diagnostic, message.candidates)
    }
}

impl From<FixAction> for Action {
    fn from(action: FixAction) -> Action {
        Action {
            label: action.label,
            kind: kind_name(&action.run),
        }
    }
}

/// The wire name of what an action does, its `kind` tag in the protocol.
fn kind_name(run: &FixKind) -> &'static str {
    match run {
        FixKind::Apply { .. } => "apply",
        FixKind::Pick { .. } => "pick",
        FixKind::Show { .. } => "show",
        FixKind::Undo => "undo",
        FixKind::FinishTransaction => "finish_transaction",
    }
}

#[cfg(test)]
mod tests {
    use partsmith_protocol::{FixRef, Severity};
    use serde_json::{Value, json};

    use super::*;
    use crate::output::capture;

    fn id(text: &str) -> Id {
        Id::new(text).expect("a valid id")
    }

    fn candidate(query: &str, relation: &str) -> RepairCandidate {
        serde_json::from_value(json!({
            "reference": {"q": query, "fp": "line|n=1,0,0|c=0,-10,15|l=20|adj=2,2"},
            "description": "the front edge of the cut made by 'Extrude 1'",
            "relation": relation,
            "meaning": true,
            "distance": 5.0,
            "score": 0.9,
            "element": {"body": "b-1", "mesh_id": 7, "kind": "edge", "index": 3}
        }))
        .expect("a valid candidate")
    }

    fn apply(label: &str) -> FixAction {
        FixAction {
            label: label.to_owned(),
            run: FixKind::Apply {
                fix: FixRef {
                    feature: id("f-fil1"),
                    code: "references_not_found".to_owned(),
                    field: Some("/edges".to_owned()),
                    cause: false,
                    index: 0,
                    revision: 0,
                },
            },
        }
    }

    fn pick(label: &str) -> FixAction {
        FixAction {
            label: label.to_owned(),
            run: FixKind::Pick {
                feature: id("f-fil1"),
                path: "/edges/0".to_owned(),
            },
        }
    }

    fn lost_edge() -> Diagnostic {
        Diagnostic {
            field: Some("/edges".to_owned()),
            actions: vec![
                apply("Use the front edge of the cut made by 'Extrude 1'"),
                pick("Pick an edge again"),
            ],
            detail: Some("edges[0] q=edge(f-box1/front|f-box1/top) resolution=missing".to_owned()),
            ..Diagnostic::new(Severity::Error, "references_not_found", "The edge is gone.")
        }
    }

    fn lines(members: &MessageMembers, indent: usize) -> String {
        let ((), stdout, stderr) = capture(|output| members.write_lines(output, indent));
        assert!(stderr.is_empty());
        stdout
    }

    #[test]
    fn actions_print_their_labels_then_candidates_their_query_and_relation() {
        let members = MessageMembers::new(
            lost_edge(),
            vec![candidate(
                "edge(f-box1/front|f-ext2/lateral/e-1)",
                "on the front side of 'Box 1', 5 mm from where it was",
            )],
        );

        assert_eq!(
            lines(&members, 6),
            "      suggested: Use the front edge of the cut made by 'Extrude 1'\n      suggested: Pick an edge again\n      candidate: edge(f-box1/front|f-ext2/lateral/e-1), on the front side of 'Box 1', 5 mm from where it was\n"
        );
    }

    #[test]
    fn a_message_without_actions_or_candidates_prints_no_lines() {
        let members = MessageMembers::new(
            Diagnostic::new(Severity::Warning, "no_effect", "Nothing changed."),
            Vec::new(),
        );

        assert_eq!(lines(&members, 2), "");
    }

    #[test]
    fn the_detail_is_never_printed() {
        let members = MessageMembers::new(lost_edge(), Vec::new());

        assert!(!lines(&members, 2).contains("resolution="));
    }

    #[test]
    fn actions_serialize_as_their_label_and_kind() {
        let members = MessageMembers::new(lost_edge(), Vec::new());

        assert_eq!(
            serde_json::to_value(&members.actions).expect("actions serialize"),
            json!([
                {"label": "Use the front edge of the cut made by 'Extrude 1'", "kind": "apply"},
                {"label": "Pick an edge again", "kind": "pick"}
            ])
        );
    }

    #[test]
    fn every_kind_of_action_has_its_wire_name() {
        let kinds = [
            FixKind::Show {
                feature: id("f-dp1"),
                entity: None,
            },
            FixKind::Undo,
            FixKind::FinishTransaction,
            apply("Use 2 mm").run,
            pick("Pick an edge again").run,
        ];

        for run in kinds {
            let wire = serde_json::to_value(&run).expect("a fix kind serializes");
            assert_eq!(Value::from(kind_name(&run)), wire["kind"], "{wire}");
        }
    }

    #[test]
    fn candidates_serialize_without_their_mesh_element() {
        let members = MessageMembers::new(
            lost_edge(),
            vec![candidate("edge(f-box1/back|f-box1/top)", "near")],
        );

        let json = serde_json::to_value(&members.candidates[0]).expect("a candidate serializes");

        assert_eq!(
            json.as_object()
                .expect("an object")
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            [
                "reference",
                "description",
                "relation",
                "meaning",
                "distance",
                "score"
            ]
        );
    }

    #[test]
    fn members_serialize_in_order_and_omit_what_is_absent() {
        let members = MessageMembers::new(
            Diagnostic {
                root: Some(id("f-dp1")),
                actions: vec![FixAction {
                    label: "Show 'Datum plane 1'".to_owned(),
                    run: FixKind::Show {
                        feature: id("f-dp1"),
                        entity: None,
                    },
                }],
                ..Diagnostic::new(
                    Severity::Error,
                    "upstream_failed",
                    "Fix 'Datum plane 1' first.",
                )
            },
            Vec::new(),
        );

        assert_eq!(
            serde_json::to_string(&members).expect("members serialize"),
            r#"{"actions":[{"label":"Show 'Datum plane 1'","kind":"show"}],"root":"f-dp1"}"#
        );
    }

    #[test]
    fn every_member_keeps_the_spec_order() {
        let members = MessageMembers::new(
            Diagnostic {
                suggestion: Some("2.3 mm".to_owned()),
                reason: Some(LossReason::Unknown),
                root: Some(id("f-ext1")),
                ..lost_edge()
            },
            vec![candidate("edge(f-box1/back|f-box1/top)", "near")],
        );

        let json = serde_json::to_value(&members).expect("members serialize");

        assert_eq!(
            json.as_object()
                .expect("an object")
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            [
                "field",
                "suggestion",
                "actions",
                "candidates",
                "reason",
                "root",
                "detail"
            ]
        );
    }
}
