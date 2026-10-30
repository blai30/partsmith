//! Acceptance tests for the suggestion lines of `partsmith rebuild --checks` (cli spec section 3.3,
//! test 63, and print.md section 3.12).

use crate::common;

use common::{
    BOX, Run, Workspace, edited_fixture, entry, feature_mut, items, lines_after, number, record_mut,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The trapped volume line of the hollow box of test 63 (b) on `generic-resin`.
const HOLLOW_LINE: &str = "    warning[trapped_volumes] b-1: An enclosed hollow of 14976 mm³ will trap liquid resin. Add a drain hole near its lowest point.";

/// What starts a suggestion line under a finding.
const SUGGESTED: &str = "      suggested: ";

/// `box.ptsm` with `f-box1`'s `size` set to these expressions and parameter `height` to `height`.
fn box_document(size: [&str; 3], height: &str) -> String {
    edited_fixture(BOX, |document| {
        feature_mut(document, "f-box1")["size"] = json!(size);
        record_mut(document, "parameters", "p-h")["expr"] = Value::from(height);
    })
}

/// `box.ptsm` with the closed 2 mm shell `f-sh1` after `f-box1`: a hollow x in [-8, 8], y in
/// [-13, 13], z in [2, 38].
fn hollow_box_document() -> String {
    edited_fixture(BOX, |document| {
        document["features"]
            .as_array_mut()
            .expect("the document has a features array")
            .push(json!({"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "2 mm"}));
    })
}

/// Runs the binary on `text` written as `box.ptsm`, with `arguments` before the file name.
fn rebuild(text: &str, arguments: &[&str]) -> Run {
    let workspace = Workspace::new();
    workspace.write(BOX, text);
    let mut all = vec!["rebuild"];
    all.extend_from_slice(arguments);
    all.push(BOX);
    workspace.run(&all)
}

/// The line right after the only line that starts with `prefix`.
fn line_after_the_one_starting_with<'a>(run: &'a Run, prefix: &str) -> &'a str {
    let lines = run.stdout_lines();
    let matching: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with(prefix))
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "one line starts with {prefix:?}\n--- standard output ---\n{}",
        run.stdout
    );
    lines
        .get(matching[0] + 1)
        .unwrap_or_else(|| panic!("a line follows {prefix:?}\n{}", run.stdout))
}

/// The x and y of a drain hole line `      suggested: Add a drain hole near (X, Y, 2 mm)`, each
/// written `-8` or `8` and `-13` or `13`.
fn drain_hole_corner(line: &str) -> (f64, f64) {
    let inside = line
        .strip_prefix(SUGGESTED)
        .and_then(|rest| rest.strip_prefix("Add a drain hole near ("))
        .and_then(|rest| rest.strip_suffix(", 2 mm)"))
        .unwrap_or_else(|| panic!("a drain hole line at z = 2, got {line:?}"));
    let (x, y) = inside
        .split_once(", ")
        .unwrap_or_else(|| panic!("two coordinates before z, got {line:?}"));
    assert!(["-8", "8"].contains(&x), "x is -8 or 8 in {line:?}");
    assert!(["-13", "13"].contains(&y), "y is -13 or 13 in {line:?}");
    (
        x.parse().expect("x is a number"),
        y.parse().expect("y is a number"),
    )
}

#[test]
fn t063_a_bed_fit_findings_are_followed_by_their_split_lines() {
    let run = rebuild(
        &box_document(["300 mm", "30 mm", "height"], "300 mm"),
        &["--checks"],
    );

    run.assert_code(0);
    let lines = run.stdout_lines();
    assert!(lines.len() >= 5, "{}", run.stdout);
    assert_eq!(
        lines[lines.len() - 5..],
        [
            "  print health: 2 errors (generic-fdm)",
            "    error[bed_fit] b-1: The part footprint is 300 x 30 mm, larger than the 256 x 256 mm bed. Split the part or scale it down.",
            "      suggested: Split at x = 0 mm",
            "    error[bed_fit] b-1: The part is 300 mm tall, taller than the 256 mm build height. Split the part or lay it on its side.",
            "      suggested: Split at z = 150 mm",
        ]
    );
}

#[test]
fn t063_b_a_trapped_volume_is_followed_by_its_drain_hole_line() {
    let run = rebuild(
        &hollow_box_document(),
        &["--checks", "--printer", "generic-resin"],
    );

    run.assert_code(0);
    let [line] = lines_after(&run, HOLLOW_LINE, 1)[..] else {
        unreachable!("lines_after returns exactly one line")
    };
    drain_hole_corner(line);
}

#[test]
fn t063_b_the_overhang_of_the_hollow_has_no_suggestion_line() {
    let run = rebuild(
        &hollow_box_document(),
        &["--checks", "--printer", "generic-resin"],
    );

    run.assert_code(0);
    let lines = run.stdout_lines();
    let overhangs: Vec<usize> = (0..lines.len())
        .filter(|&index| lines[index].starts_with("    warning[overhangs] b-1: "))
        .collect();
    assert!(!overhangs.is_empty(), "{}", run.stdout);
    for index in overhangs {
        let next = lines.get(index + 1).copied().unwrap_or_default();
        assert!(
            !next.starts_with(SUGGESTED),
            "no suggestion follows {:?}\n{}",
            lines[index],
            run.stdout
        );
    }
}

#[test]
fn t063_b_json_carries_the_drain_hole_point_without_words() {
    let document = hollow_box_document();
    let human_run = rebuild(&document, &["--checks", "--printer", "generic-resin"]);
    human_run.assert_code(0);
    let [line] = lines_after(&human_run, HOLLOW_LINE, 1)[..] else {
        unreachable!("lines_after returns exactly one line")
    };
    let (x, y) = drain_hole_corner(line);

    let json_run = rebuild(
        &document,
        &["--checks", "--printer", "generic-resin", "--json"],
    );

    json_run.assert_code(0);
    let report = json_run.json();
    let body = entry(&report["checks"]["bodies"], "body", "b-1");
    let trapped = entry(&body["report"]["checks"], "check", "trapped_volumes");
    let findings = items(&trapped["findings"]);
    assert_eq!(findings.len(), 1, "{trapped}");
    let suggestion = &findings[0]["suggestion"];
    assert_eq!(suggestion["kind"], "drain_hole");
    let mut members: Vec<&str> = suggestion
        .as_object()
        .unwrap_or_else(|| panic!("a suggestion object, got {suggestion}"))
        .keys()
        .map(String::as_str)
        .collect();
    members.sort_unstable();
    assert_eq!(members, ["at", "kind"]);
    let at = items(&suggestion["at"]);
    assert_eq!(at.len(), 3, "{suggestion}");
    for (coordinate, expected) in at.iter().zip([x, y, 2.0]) {
        assert!(
            (number(coordinate, "a drain hole coordinate") - expected).abs() <= 1e-9,
            "{suggestion} is within 1e-9 of [{x}, {y}, 2]"
        );
    }
    assert!(
        !json_run.stdout.contains("Add a drain hole near ("),
        "the JSON output carries no suggestion words"
    );
}

#[test]
fn t063_c_a_turned_footprint_is_followed_by_a_tilted_plane_line() {
    let run = rebuild(
        &box_document(["300 mm", "100 mm", "height"], "10 mm"),
        &["--checks", "--rotate", "z:30"],
    );

    run.assert_code(0);
    assert_eq!(
        line_after_the_one_starting_with(&run, "    error[bed_fit] b-1: "),
        "      suggested: Split at the plane through (0, 0, 5 mm) with normal (0.866, -0.5, 0)"
    );
}

#[test]
fn t063_d_several_planes_are_joined_in_one_line() {
    let run = rebuild(
        &box_document(["600 mm", "300 mm", "height"], "10 mm"),
        &["--checks"],
    );

    run.assert_code(0);
    assert_eq!(
        lines_after(
            &run,
            "    error[bed_fit] b-1: The part footprint is 600 x 300 mm, larger than the 256 x 256 mm bed. Split the part or scale it down.",
            1,
        ),
        ["      suggested: Split at x = -100 mm, x = 100 mm and y = 0 mm"]
    );
}
