//! Acceptance tests for feature messages, actions and candidates in `validate` and `rebuild` (cli spec sections 2.2,
//! 3.1 and 3.3, and section 6, tests 55 to 62), on documents of the reference stability corpus after their edits.

use crate::common;
use crate::corpus;

use common::{
    Workspace, assert_json_close, entry, items, keys, lines_after, number, run_of_lines_after, text,
};
use corpus::Scenario;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The message of corpus R16's fillet (recompute.md test 52).
const R16_MESSAGE: &str = "The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet.";

/// The message of corpus R03's fillet (recompute.md test 59, features.md section 6.5 test 15).
const R03_MESSAGE: &str = "The top edge of 'Extrude 1' along a line of 'Sketch 1' selected for Edges is gone because that line was deleted from 'Sketch 1', so it was skipped.";

/// The message of corpus R66's fillet (recompute.md test 64).
const R66_MESSAGE: &str = "The back top edge of 'Extrude 1' selected for Edges now covers 5 edges, because the line it was made from was replaced by 5 lines in 'Sketch 1'. Keep all of them, or use only one.";

/// A workspace holding the scenario's document after its edit as `name`.
fn workspace_with(name: &str, scenario: Scenario) -> Workspace {
    let workspace = Workspace::new();
    workspace.write(name, scenario.edited_text());
    workspace
}

/// The diagnostic of `validate --json` of the only file whose `feature` is `feature`.
fn feature_diagnostic<'a>(report: &'a Value, feature: &str) -> &'a Value {
    let found: Vec<&Value> = items(&report["files"][0]["diagnostics"])
        .iter()
        .filter(|diagnostic| diagnostic["feature"] == feature)
        .collect();
    assert_eq!(
        found.len(),
        1,
        "one diagnostic of {feature} in {}",
        report["files"][0]
    );
    found[0]
}

/// The first message of `feature` in a `rebuild --json` report.
fn first_message<'a>(report: &'a Value, feature: &str) -> &'a Value {
    &entry(&report["features"], "id", feature)["messages"][0]
}

/// The `reason` of the first message of `feature` in `rebuild --json` of the scenario written as `name`.
fn first_reason(name: &str, scenario: Scenario, feature: &str) -> Value {
    let workspace = workspace_with(name, scenario);
    let report = workspace.run(&["rebuild", "--json", name]).json();
    first_message(&report, feature)["reason"].clone()
}

#[test]
fn t055_rebuild_prints_actions_and_candidates() {
    let workspace = workspace_with("r16.ptsm", corpus::r16());

    let run = workspace.run(&["rebuild", "r16.ptsm"]);

    run.assert_code(1);
    let block = lines_after(&run, "  f-fil1 fillet error", 13);
    assert_eq!(
        block[0],
        format!("    error[references_not_found] at /features/3/edges: {R16_MESSAGE}")
    );
    assert_eq!(
        block[1],
        "      suggested: Use the front edge of the cut made by 'Extrude 1'"
    );
    assert_eq!(
        block[2],
        "      suggested: Use the top edge of the cut made by 'Extrude 1'"
    );
    for line in &block[3..6] {
        assert!(
            line.starts_with("      suggested: Use "),
            "a Use action line, got {line:?}"
        );
    }
    assert_eq!(block[6], "      suggested: Pick an edge again");
    assert_eq!(
        block[7],
        "      candidate: edge(f-box1/front|f-ext2/lateral/e-1), on the front side of 'Box 1', 5 mm from where it was"
    );
    assert_eq!(
        block[8],
        "      candidate: edge(f-box1/top|f-ext2/lateral/e-2), on the top of 'Box 1', 5 mm from where it was"
    );
    for line in &block[9..12] {
        assert!(
            line.starts_with("      candidate: "),
            "a candidate line, got {line:?}"
        );
    }
    assert!(
        !block[12].starts_with("      candidate: ") && !block[12].starts_with("      suggested: "),
        "five candidate lines end the message, then {:?}",
        block[12]
    );
    for line in run.stdout_lines() {
        for unwanted in ["This started after", "resolution=", "score", "0.913978"] {
            assert!(!line.contains(unwanted), "{line:?} contains {unwanted:?}");
        }
    }
}

#[test]
fn t056_rebuild_json_carries_the_message_members() {
    let workspace = workspace_with("r16.ptsm", corpus::r16());

    let run = workspace.run(&["rebuild", "--json", "r16.ptsm"]);

    run.assert_code(1);
    let report = run.json();
    let feature = &report["features"][3];
    assert_eq!(feature["id"], "f-fil1");
    let message = &feature["messages"][0];
    assert_eq!(
        keys(message),
        [
            "severity",
            "code",
            "message",
            "field",
            "actions",
            "candidates",
            "reason",
            "detail"
        ]
    );
    assert_eq!(message["severity"], "error");
    assert_eq!(message["code"], "references_not_found");
    assert_eq!(message["message"], R16_MESSAGE);
    assert_eq!(message["field"], "/edges");

    let actions = items(&message["actions"]);
    assert_eq!(actions.len(), 6, "{}", message["actions"]);
    for action in actions {
        assert_eq!(keys(action), ["label", "kind"]);
    }
    for action in &actions[..5] {
        assert!(text(&action["label"]).starts_with("Use "), "{action}");
        assert_eq!(action["kind"], "apply", "{action}");
    }
    assert_eq!(
        actions[5],
        json!({"label": "Pick an edge again", "kind": "pick"})
    );

    let candidates = items(&message["candidates"]);
    assert_eq!(candidates.len(), 5, "{}", message["candidates"]);
    let first = &candidates[0];
    assert_eq!(
        keys(first),
        [
            "reference",
            "description",
            "relation",
            "meaning",
            "distance",
            "score"
        ]
    );
    assert_json_close(
        &first["reference"],
        &json!({
            "q": "edge(f-box1/front|f-ext2/lateral/e-1)",
            "fp": "line|n=1,0,0|c=0,-10,15|l=20|adj=2,2"
        }),
        0.0,
    );
    assert_eq!(
        first["description"],
        "the front edge of the cut made by 'Extrude 1'"
    );
    assert_eq!(
        first["relation"],
        "on the front side of 'Box 1', 5 mm from where it was"
    );
    assert_eq!(first["meaning"], true);
    assert!((number(&first["distance"], "distance") - 5.0).abs() <= 1e-6);
    assert!((number(&first["score"], "score") - 0.913978).abs() <= 1e-6);

    assert_json_close(
        &message["reason"],
        &json!({"kind": "removed", "by": "f-ext2", "how": {"kind": "cut_away"}}),
        0.0,
    );
    let detail = text(&message["detail"]);
    assert!(
        detail.starts_with(
            "edges[0] q=edge(f-box1/front|f-box1/top) resolution=missing lost.unmet=true candidates=edge(f-box1/front|f-ext2/lateral/e-1):0.913978,"
        ),
        "{detail}"
    );
}

#[test]
fn t057_validate_fails_on_a_lost_reference() {
    let workspace = workspace_with("r03.ptsm", corpus::r03());

    let validated = workspace.run(&["validate", "--rebuild", "r03.ptsm"]);
    let rebuilt = workspace.run(&["rebuild", "r03.ptsm"]);
    let denied = workspace.run(&["rebuild", "--deny-warnings", "r03.ptsm"]);

    validated.assert_code(1);
    assert!(
        !validated.stdout_lines().contains(&"r03.ptsm: ok"),
        "{}",
        validated.stdout
    );
    let message_line =
        format!("r03.ptsm: warning[reference_missing] at /features/2/edges/0: {R03_MESSAGE}");
    let following = run_of_lines_after(&validated, &message_line, "  ");
    let action_count = following
        .iter()
        .take_while(|line| line.starts_with("  suggested: "))
        .count();
    let (action_lines, candidate_lines) = following.split_at(action_count);
    assert!(action_lines.len() >= 2, "{}", validated.stdout);
    assert_eq!(
        action_lines[action_lines.len() - 2..].to_vec(),
        [
            "  suggested: Remove from Edges",
            "  suggested: Pick an edge again"
        ]
    );
    // The same message in JSON has the actions and candidates the text prints, one line each.
    let report = workspace
        .run(&["validate", "--rebuild", "--json", "r03.ptsm"])
        .json();
    let diagnostic = feature_diagnostic(&report, "f-fil1");
    let expected_actions: Vec<String> = items(&diagnostic["actions"])
        .iter()
        .map(|action| format!("  suggested: {}", text(&action["label"])))
        .collect();
    let expected_candidates: Vec<String> = items(&diagnostic["candidates"])
        .iter()
        .map(|candidate| {
            format!(
                "  candidate: {}, {}",
                text(&candidate["reference"]["q"]),
                text(&candidate["relation"])
            )
        })
        .collect();
    assert!(!expected_candidates.is_empty(), "{diagnostic}");
    assert_eq!(action_lines.to_vec(), expected_actions);
    assert_eq!(candidate_lines.to_vec(), expected_candidates);
    rebuilt.assert_code(0);
    assert!(
        rebuilt.stdout_lines().contains(&"  f-fil1 fillet warning"),
        "{}",
        rebuilt.stdout
    );
    denied.assert_code(1);
}

#[test]
fn t058_validate_json_reports_a_lost_reference() {
    let workspace = workspace_with("r03.ptsm", corpus::r03());

    let run = workspace.run(&["validate", "--rebuild", "--json", "r03.ptsm"]);

    run.assert_code(1);
    let report = run.json();
    assert_eq!(report["files"][0]["ok"], false);
    let diagnostic = feature_diagnostic(&report, "f-fil1");
    assert_eq!(diagnostic["severity"], "warning");
    assert_eq!(diagnostic["code"], "reference_missing");
    assert_eq!(diagnostic["message"], R03_MESSAGE);
    assert_eq!(diagnostic["path"], "/features/2/edges/0");
    assert_eq!(diagnostic["field"], "/edges/0");
    let actions = items(&diagnostic["actions"]);
    assert!(actions.len() >= 2, "{diagnostic}");
    assert_eq!(
        actions[actions.len() - 2..].to_vec(),
        [
            json!({"label": "Remove from Edges", "kind": "apply"}),
            json!({"label": "Pick an edge again", "kind": "pick"})
        ]
    );
    assert_json_close(
        &diagnostic["reason"],
        &json!({"kind": "entity_deleted", "feature": "f-ext1", "sketch": "f-sk1", "entity": "e-5"}),
        0.0,
    );
    let detail = text(&diagnostic["detail"]);
    assert!(
        detail.starts_with(
            "edges[0] q=edge(f-ext1/end-cap|f-ext1/lateral/e-5) resolution=missing lost.parts=f-ext1/lateral/e-5"
        ),
        "{detail}"
    );
    for absent in ["line", "column", "root", "cause"] {
        assert!(
            diagnostic.get(absent).is_none(),
            "{diagnostic} has no {absent}"
        );
    }
}

#[test]
fn t059_validate_fails_on_a_guessed_reference() {
    let workspace = workspace_with("r08.ptsm", corpus::r08a());
    let document: Value =
        serde_json::from_str(&workspace.read_text("r08.ptsm")).expect("the document is JSON");
    let stored = &document["features"][5]["edges"][0];
    assert_eq!(
        keys(stored),
        ["q", "fp"],
        "the fillet's reference is stored without nb"
    );

    let run = workspace.run(&["validate", "--rebuild", "r08.ptsm"]);

    run.assert_code(1);
    assert!(
        run.stdout_lines().iter().any(|line| line
            .starts_with("r08.ptsm: warning[reference_guessed] at /features/5/edges/0: ")),
        "{}",
        run.stdout
    );
}

#[test]
fn t060_a_widened_reference_does_not_fail_validate() {
    let workspace = workspace_with("r66.ptsm", corpus::r66());

    let run = workspace.run(&["validate", "--rebuild", "r66.ptsm"]);
    let denied = workspace.run(&["validate", "--rebuild", "--deny-warnings", "r66.ptsm"]);

    run.assert_code(0);
    let message_line =
        format!("r66.ptsm: warning[reference_widened] at /features/2/edges/0: {R66_MESSAGE}");
    assert_eq!(
        run_of_lines_after(&run, &message_line, "  suggested: "),
        [
            "  suggested: Keep all 5",
            "  suggested: Use only the back top edge of 'Extrude 1' along the line from (80, 50) to (50, 50) in 'Sketch 1'",
            "  suggested: Pick an edge again"
        ]
    );
    denied.assert_code(1);
}

#[test]
fn t060_a_followed_reference_notice_does_not_fail_validate() {
    let workspace = workspace_with("r06.ptsm", corpus::r06a());

    let run = workspace.run(&["validate", "--rebuild", "r06.ptsm"]);
    let denied = workspace.run(&["validate", "--rebuild", "--deny-warnings", "r06.ptsm"]);
    let report = workspace
        .run(&["validate", "--rebuild", "--json", "r06.ptsm"])
        .json();

    run.assert_code(0);
    let lines = run.stdout_lines();
    let notice = lines
        .iter()
        .position(|line| {
            line.starts_with("r06.ptsm: info[reference_followed] at /features/2/edges/0: ")
                && line.ends_with(", because its line was replaced in 'Sketch 1'.")
        })
        .unwrap_or_else(|| panic!("a reference_followed notice line\n{}", run.stdout));
    assert_eq!(
        run_of_lines_after(&run, lines[notice], "  "),
        [
            "  suggested: Keep this match",
            "  suggested: Pick an edge again"
        ]
    );
    assert!(!lines.contains(&"r06.ptsm: ok"), "{}", run.stdout);
    denied.assert_code(0);
    assert_eq!(report["files"][0]["ok"], true);
}

#[test]
fn t061_an_upstream_chain_names_its_root() {
    let workspace = workspace_with("r54.ptsm", corpus::r54());

    let json_run = workspace.run(&["rebuild", "--json", "r54.ptsm"]);
    let text_run = workspace.run(&["rebuild", "r54.ptsm"]);

    json_run.assert_code(1);
    let report = json_run.json();
    let messages = &entry(&report["features"], "id", "f-ext3")["messages"];
    assert_eq!(items(messages).len(), 1, "{messages}");
    assert_json_close(
        &messages[0],
        &json!({
            "severity": "error",
            "code": "upstream_failed",
            "message": "'Sketch 2' cannot be built because 'Datum plane 1' has an error, so this extrude cannot use it either. Fix 'Datum plane 1' first.",
            "actions": [{"label": "Show 'Datum plane 1'", "kind": "show"}],
            "root": "f-dp1"
        }),
        0.0,
    );
    text_run.assert_code(1);
    assert_eq!(
        lines_after(&text_run, "  f-ext3 extrude error", 2),
        [
            "    error[upstream_failed] at /features/4: 'Sketch 2' cannot be built because 'Datum plane 1' has an error, so this extrude cannot use it either. Fix 'Datum plane 1' first.",
            "      suggested: Show 'Datum plane 1'"
        ]
    );
}

#[test]
fn t062_the_reason_of_an_edge_no_longer_made() {
    let reason = first_reason("r12.ptsm", corpus::r12(), "f-fil1");

    assert_json_close(
        &reason,
        &json!({"kind": "removed", "by": "f-ext2", "how": {"kind": "no_longer_made", "absence": "through"}}),
        0.0,
    );
}

#[test]
fn t062_the_reason_of_an_edge_in_another_body() {
    let reason = first_reason("r31.ptsm", corpus::r31a(), "f-cha1");

    assert_json_close(
        &reason,
        &json!({"kind": "other_body", "body": "b-2", "by": "f-split1", "by_suppressed": false, "present": true}),
        0.0,
    );
}

#[test]
fn t062_the_reason_of_a_copy_that_is_gone() {
    let reason = first_reason("r41.ptsm", corpus::r41(), "f-fil1");

    assert_json_close(
        &reason,
        &json!({"kind": "copy_gone", "feature": "f-lp1", "instance": {"first": 2, "second": 0}}),
        0.0,
    );
}

#[test]
fn t062_the_reason_of_a_followed_reference() {
    let reason = first_reason("r06.ptsm", corpus::r06a(), "f-fil1");

    assert_json_close(&reason, &json!({"kind": "followed", "by": "f-sk1"}), 0.0);
}

#[test]
fn t062_an_ambiguous_loss_has_no_reason() {
    let workspace = workspace_with("r63.ptsm", corpus::r63a());

    let report = workspace.run(&["rebuild", "--json", "r63.ptsm"]).json();

    let message = first_message(&report, "f-cha1");
    assert_eq!(message["code"], "references_not_found", "{message}");
    assert!(message.get("reason").is_none(), "{message}");
}
