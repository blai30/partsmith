//! Acceptance tests 41 to 54 and 109 of `docs/spec/engine.md` section 6: sketch solutions, candidate solves, drags and datum planes.

use crate::common;

use common::{
    GEOMETRY_TOLERANCE, apply, assert_bounding_box, assert_near, assert_point2, assert_point3,
    drag_sketch, end_sketch_drag, get_document, number, open, open_doc, point2, point3, send,
    session,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// Tolerance for solved sketch coordinates in tests 41 to 45 and 52.
const SKETCH_TOLERANCE: f64 = 1e-9;

/// The solved position of a point address in a `SketchSolution`.
fn solved_point(solution: &Value, point: &str) -> [f64; 2] {
    let entry = solution["points"]
        .as_array()
        .expect("solutions list points")
        .iter()
        .find(|entry| entry["point"] == point)
        .unwrap_or_else(|| panic!("the solution has point {point}: {solution}"));
    point2(&entry["at"])
}

#[test]
fn t041_opening_drag_reports_the_under_constrained_solution() {
    let mut session = session();

    let opened = open(&mut session, "drag.ptsm");

    let solution = opened.sketch_solution("f-sk1");
    assert_eq!(solution["dof"], 1);
    assert_eq!(solution["status"], "under_constrained");
    assert_eq!(solution["failed"], false);
    let entities = solution["entities"]
        .as_array()
        .expect("solutions list entities");
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0]["id"], "e-1");
    assert_eq!(entities[0]["constrained"], "under_constrained");
    let points = solution["points"]
        .as_array()
        .expect("solutions list points");
    assert_eq!(points.len(), 2);
    assert_eq!(points[0]["point"], "e-1.start");
    assert_point2(
        point2(&points[0]["at"]),
        [0.0, 0.0],
        SKETCH_TOLERANCE,
        "e-1.start",
    );
    assert_eq!(points[1]["point"], "e-1.end");
    assert_point2(
        point2(&points[1]["at"]),
        [10.0, 0.0],
        SKETCH_TOLERANCE,
        "e-1.end",
    );
    assert_eq!(solution["regions"], json!([]));
    let findings = solution["profile_findings"]
        .as_array()
        .expect("solutions list profile findings");
    assert_eq!(findings.len(), 2);
    for (finding, at) in findings.iter().zip([[0.0, 0.0], [10.0, 0.0]]) {
        assert_eq!(finding["type"], "open_endpoint");
        assert_eq!(finding["entity"], "e-1");
        assert_point2(
            point2(&finding["point"]),
            at,
            SKETCH_TOLERANCE,
            "open endpoint",
        );
    }
}

#[test]
fn t042_solving_a_candidate_records_nothing() {
    let mut session = session();
    let doc = open_doc(&mut session, "drag.ptsm");
    let candidate = json!({
        "plane": {"kind": "origin", "plane": "XY"},
        "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [10, 0]}],
        "constraints": [
            {"id": "c-1", "type": "fix", "target": "e-1.start"},
            {"id": "c-2", "type": "horizontal", "line": "e-1"},
            {"id": "c-3", "type": "distance", "points": ["e-1.start", "e-1.end"], "expr": "10 mm"}
        ]
    });

    let solved = send(
        &mut session,
        json!({"type": "solve_sketch", "doc": doc, "feature": "f-sk1", "candidate": candidate}),
    );

    let solution = &solved.ok()["solution"];
    assert_eq!(solution["dof"], 0);
    assert_eq!(solution["status"], "fully_constrained");
    assert!(
        solved.events.is_empty(),
        "no events: {:?}",
        solved.event_types()
    );
    let constraints = &get_document(&mut session, doc)["document"]["features"][0]["constraints"];
    assert_eq!(constraints.as_array().map(Vec::len), Some(2));
}

#[test]
fn t043_a_drag_frame_follows_the_constraints_and_records_nothing() {
    let mut session = session();
    let doc = open_doc(&mut session, "drag.ptsm");
    let before = get_document(&mut session, doc);

    let frame = drag_sketch(&mut session, doc, "e-1.end", [20.0, 5.0]);

    let end = solved_point(&frame.ok()["solution"], "e-1.end");
    assert_point2(end, [20.0, 0.0], SKETCH_TOLERANCE, "e-1.end");
    assert!(
        frame.events.is_empty(),
        "no events: {:?}",
        frame.event_types()
    );
    assert_eq!(get_document(&mut session, doc), before);
}

#[test]
fn t044_a_second_frame_continues_the_drag() {
    let mut session = session();
    let doc = open_doc(&mut session, "drag.ptsm");
    drag_sketch(&mut session, doc, "e-1.end", [20.0, 5.0]).ok();

    let frame = drag_sketch(&mut session, doc, "e-1.end", [30.0, -5.0]);

    let end = solved_point(&frame.ok()["solution"], "e-1.end");
    assert_point2(end, [30.0, 0.0], SKETCH_TOLERANCE, "e-1.end");
}

#[test]
fn t045_committing_a_drag_is_one_undoable_edit_labeled_drag() {
    let mut session = session();
    let doc = open_doc(&mut session, "drag.ptsm");
    drag_sketch(&mut session, doc, "e-1.end", [20.0, 5.0]).ok();
    drag_sketch(&mut session, doc, "e-1.end", [30.0, -5.0]).ok();

    let ended = end_sketch_drag(&mut session, doc, true);

    assert_eq!(ended.ok()["revision"], 1);
    let after = get_document(&mut session, doc);
    assert_eq!(after["revision"], 1);
    let end = point2(&after["document"]["features"][0]["entities"][0]["end"]);
    assert_point2(end, [30.0, 0.0], SKETCH_TOLERANCE, "committed e-1 end");

    let undone = send(&mut session, json!({"type": "undo", "doc": doc}));

    assert_eq!(undone.ok()["label"], "Drag");
    let restored = &get_document(&mut session, doc)["document"]["features"][0]["entities"][0];
    assert_eq!(point2(&restored["end"]), [10.0, 0.0]);
}

#[test]
fn t046_ending_a_drag_without_commit_changes_nothing() {
    let mut session = session();
    let doc = open_doc(&mut session, "drag.ptsm");
    let before = get_document(&mut session, doc);
    drag_sketch(&mut session, doc, "e-1.end", [20.0, 5.0]).ok();

    let ended = end_sketch_drag(&mut session, doc, false);

    assert_eq!(ended.ok()["revision"], 0);
    assert_eq!(get_document(&mut session, doc), before);
}

#[test]
fn t047_ending_without_a_drag_fails() {
    let mut session = session();
    let doc = open_doc(&mut session, "drag.ptsm");

    let ended = end_sketch_drag(&mut session, doc, true);

    assert_eq!(ended.error_code(), "no_drag");
}

#[test]
fn t048_dragging_a_non_sketch_feature_fails() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let sent = send(
        &mut session,
        json!({"type": "drag_sketch", "doc": doc, "feature": "f-box1", "point": "e-1.end", "to": [1.0, 1.0]}),
    );

    assert_eq!(sent.error_code(), "not_a_sketch");
    assert!(
        sent.error_message().contains("box"),
        "the message names the type: {}",
        sent.error_message()
    );
}

#[test]
fn t049_dragging_an_unknown_point_fails() {
    let mut session = session();
    let doc = open_doc(&mut session, "drag.ptsm");

    let sent = drag_sketch(&mut session, doc, "e-9.end", [1.0, 1.0]);

    assert_eq!(sent.error_code(), "unknown_point");
}

#[test]
fn t050_a_recipe_change_discards_the_drag() {
    let mut session = session();
    let doc = open_doc(&mut session, "drag.ptsm");
    drag_sketch(&mut session, doc, "e-1.end", [20.0, 5.0]).ok();
    apply(
        &mut session,
        doc,
        Some("Add parameter"),
        json!([{"type": "add_parameter", "index": 0, "parameter": {"id": "p-x", "name": "x", "expr": "1 mm"}}]),
    )
    .ok();

    let ended = end_sketch_drag(&mut session, doc, true);

    assert_eq!(ended.error_code(), "no_drag");
}

#[test]
fn t051_conflicting_constraints_are_reported() {
    let mut session = session();

    let opened = open(&mut session, "conflict.ptsm");

    let solution = opened.sketch_solution("f-sk1");
    assert_eq!(solution["status"], "conflicting");
    let conflicting: Vec<&str> = solution["conflicting"]
        .as_array()
        .expect("solutions list conflicting constraints")
        .iter()
        .map(|id| id.as_str().expect("constraint ids are strings"))
        .collect();
    assert!(!conflicting.is_empty(), "some constraints conflict");
    for id in &conflicting {
        assert!(
            ["c-1", "c-2", "c-4"].contains(id),
            "{id} is one of c-1, c-2, c-4: {conflicting:?}"
        );
    }
}

#[test]
fn t052_the_plate_has_one_region_and_an_extruded_mesh() {
    let mut session = session();

    let opened = open(&mut session, "plate.ptsm");

    let solution = opened.sketch_solution("f-sk1");
    assert_eq!(solution["dof"], 4);
    let regions = solution["regions"]
        .as_array()
        .expect("solutions list regions");
    assert_eq!(regions.len(), 1);
    assert_near(
        number(&regions[0]["area"]),
        4000.0,
        SKETCH_TOLERANCE,
        "region area",
    );
    assert_point2(
        point2(&regions[0]["centroid"]),
        [40.0, 25.0],
        SKETCH_TOLERANCE,
        "region centroid",
    );
    assert_bounding_box(
        &opened.body_mesh("b-1", "model")["mesh"]["bounding_box"],
        [0.0, 0.0, 0.0],
        [80.0, 50.0, 10.0],
    );
}

#[test]
fn t053_region_fill_buffers_cover_the_disc() {
    let mut session = session();

    let opened = open(&mut session, "circle.ptsm");

    let display = &opened.sketch_solution("f-sk1")["display"];
    let positions = opened.f32_buffer(&display["region_positions"]);
    let indices = opened.u32_buffer(&display["region_indices"]);
    let region_ids = opened.u32_buffer(&display["region_ids"]);
    assert_eq!(positions.len() % 2, 0);
    assert_eq!(region_ids.len(), positions.len() / 2);
    assert!(region_ids.iter().all(|&region| region == 0), "one region");
    assert_eq!(indices.len() % 3, 0);
    let vertex = |index: u32| {
        let index = index as usize;
        [
            f64::from(positions[2 * index]),
            f64::from(positions[2 * index + 1]),
        ]
    };
    let area: f64 = indices
        .chunks(3)
        .map(|triangle| {
            let [first, second, third] = [
                vertex(triangle[0]),
                vertex(triangle[1]),
                vertex(triangle[2]),
            ];
            ((second[0] - first[0]) * (third[1] - first[1])
                - (third[0] - first[0]) * (second[1] - first[1]))
                .abs()
                / 2.0
        })
        .sum();
    let disc = 25.0 * std::f64::consts::PI;
    assert_near(area, disc, 0.002 * disc, "triangulated area");
}

#[test]
fn t054_datum_planes_are_reported_with_their_placement() {
    let mut session = session();

    let opened = open(&mut session, "datum.ptsm");

    let event = opened.event("datum_plane");
    assert_eq!(event["feature"], "f-dp1");
    let placement = &event["placement"];
    assert_point3(
        point3(&placement["origin"]),
        [0.0, 0.0, 10.0],
        GEOMETRY_TOLERANCE,
        "origin",
    );
    assert_point3(
        point3(&placement["x_axis"]),
        [1.0, 0.0, 0.0],
        GEOMETRY_TOLERANCE,
        "x axis",
    );
    assert_point3(
        point3(&placement["normal"]),
        [0.0, 0.0, 1.0],
        GEOMETRY_TOLERANCE,
        "normal",
    );
}

#[test]
fn t109_a_failed_sketch_still_reports_its_solution() {
    let mut session = session();

    let opened = open(&mut session, "conflict.ptsm");

    assert_eq!(opened.feature_status("f-sk1")["status"], "error");
    assert_eq!(opened.sketch_solution("f-sk1")["feature"], "f-sk1");
}
