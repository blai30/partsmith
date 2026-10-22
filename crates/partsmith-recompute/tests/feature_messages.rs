//! Acceptance tests for element descriptions and messages that need documents (features.md section 6.5, tests 1, 2,
//! 15, 16 and 17), run through recompute (section 6.1). Tests named `tNNN` are section 6.5 test NNN. Messages are
//! rendered by the features crate with the diagnosis each scenario's cause implies (recompute.md section 3.10), so
//! they check the rendering rules on their own; recompute's own diagnosis is checked in its tests. Test 15 covers
//! the corpus messages of the reference stability corpus and of the newcomer error corpus that it lists.

use crate::feature_support;

use std::collections::BTreeMap;
use std::f64::consts::{FRAC_PI_2, PI};

use feature_support::{
    Run, assert_messages_pass_lint, body_before, box_step_asset, document_text, id, parse_document,
    pick, remove_entry, set_field, set_reverse, step_asset_name, use_reference,
};
use partsmith_features::{
    Because, Diagnosis, Explanation, FeatureError, FeatureWarning, Fix, Repair, describe_elements,
};
use partsmith_naming::{ElementName, Fingerprint};
use serde_json::{Value, json};

// Records of the shared setups (reference stability corpus, "Shared setups"; features.md section 6.1).

/// A line entity.
fn line(entity: &str, start: [f64; 2], end: [f64; 2]) -> Value {
    json!({"id": entity, "type": "line", "start": start, "end": end})
}

/// An arc entity.
fn arc(entity: &str, center: [f64; 2], radius: f64, start_angle: f64, end_angle: f64) -> Value {
    json!({"id": entity, "type": "arc", "center": center, "radius": radius, "start_angle": start_angle, "end_angle": end_angle})
}

/// `RECT(x0, y0, x1, y1)` with the lines `e-<first>` to `e-<first + 3>`, drawn counterclockwise.
fn rectangle(first: usize, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Value> {
    vec![
        line(&format!("e-{first}"), [x0, y0], [x1, y0]),
        line(&format!("e-{}", first + 1), [x1, y0], [x1, y1]),
        line(&format!("e-{}", first + 2), [x1, y1], [x0, y1]),
        line(&format!("e-{}", first + 3), [x0, y1], [x0, y0]),
    ]
}

/// A sketch on an origin plane.
fn sketch(feature: &str, plane: &str, entities: Vec<Value>) -> Value {
    json!({"id": feature, "type": "sketch", "plane": {"kind": "origin", "plane": plane}, "entities": entities})
}

/// A sketch on a datum plane.
fn sketch_on_datum(feature: &str, datum: &str, entities: Vec<Value>) -> Value {
    json!({"id": feature, "type": "sketch", "plane": {"kind": "datum", "feature": datum}, "entities": entities})
}

/// A datum plane parallel to XY.
fn datum_xy(feature: &str, offset: &str) -> Value {
    json!({"id": feature, "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": offset})
}

/// An extrude of `regions` of `sketch` into `b-1`; `extent` is the extent object, plus `"reverse": true` when set.
fn extrude(
    feature: &str,
    sketch: &str,
    regions: Value,
    extent: Value,
    reverse: bool,
    op: &str,
) -> Value {
    let mut record = json!({
        "id": feature,
        "type": "extrude",
        "body": "b-1",
        "profile": {"sketch": sketch, "regions": regions},
        "extent": extent,
        "op": op
    });
    if reverse {
        record["reverse"] = json!(true);
    }
    record
}

/// A distance extent.
fn distance(expr: &str) -> Value {
    json!({"kind": "distance", "expr": expr})
}

/// A box in `body`.
fn box_in(feature: &str, body: &str, size: [&str; 3], position: [&str; 3], op: &str) -> Value {
    json!({"id": feature, "type": "box", "body": body, "size": size, "position": position, "op": op})
}

/// A stored reference.
fn reference(query: &str, fingerprint: &str) -> Value {
    json!({"q": query, "fp": fingerprint})
}

/// A fillet `f-fil1` of `edges` in `body`.
fn fillet(body: &str, edges: Vec<Value>, radius: &str) -> Value {
    json!({"id": "f-fil1", "type": "fillet", "body": body, "edges": edges, "radius": radius})
}

/// `CUBE20`.
fn cube20() -> Value {
    box_in("f-box1", "b-1", ["20", "20", "20"], ["0", "0", "0"], "new")
}

/// `CUBE10`.
fn cube10() -> Value {
    box_in("f-box1", "b-1", ["10", "10", "10"], ["0", "0", "0"], "new")
}

/// `BOX-BELOW`.
fn box_below() -> Value {
    box_in(
        "f-box1",
        "b-1",
        ["40", "40", "10"],
        ["0", "0", "-10"],
        "new",
    )
}

/// The sketch and extrude of `BLOCK`, with the sketch's entities `entities` (the rectangle, or an edit of it).
fn block_with(entities: Vec<Value>) -> Vec<Value> {
    vec![
        sketch("f-sk1", "XY", entities),
        extrude(
            "f-ext1",
            "f-sk1",
            json!([{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-1,e-2,e-3,e-4"}]),
            distance("30"),
            false,
            "new",
        ),
    ]
}

/// `BLOCK`.
fn block() -> Vec<Value> {
    block_with(rectangle(1, 0.0, 0.0, 80.0, 50.0))
}

/// `entities` without those whose id is in `removed`, followed by `added`.
fn edited(entities: Vec<Value>, removed: &[&str], added: Vec<Value>) -> Vec<Value> {
    entities
        .into_iter()
        .filter(|entity| !removed.contains(&entity["id"].as_str().unwrap_or_default()))
        .chain(added)
        .collect()
}

/// `SLOT(a, b)` on `BLOCK`.
fn slot(from: f64, to: f64) -> Vec<Value> {
    let center = (from + to) / 2.0;
    let area = (to - from) * 52.0;
    vec![
        datum_xy("f-dp1", "30"),
        sketch_on_datum("f-sk2", "f-dp1", rectangle(21, from, -1.0, to, 51.0)),
        extrude(
            "f-ext2",
            "f-sk2",
            json!([{"key": "e-21,e-22,e-23,e-24", "fp": format!("c={center},25|a={area}")}]),
            distance("5"),
            true,
            "cut",
        ),
    ]
}

/// Runs `features` with these bodies.
fn run_features(bodies: &[&str], features: &[Value]) -> Run {
    let texts: Vec<String> = features.iter().map(Value::to_string).collect();
    let records: Vec<&str> = texts.iter().map(String::as_str).collect();
    Run::records(bodies, &records)
}

/// An element name written in a test.
fn name(text: &str) -> ElementName {
    ElementName::parse(text).expect("element names written in tests are valid")
}

/// A fingerprint written in a test.
fn fingerprint(text: &str) -> Fingerprint {
    Fingerprint::parse(text).expect("fingerprints written in tests are valid")
}

/// The descriptions of elements of `body` at the end of `run`, given by their names, in one call (section 3.12).
fn describe_present(run: &Run, body: &str, names: &[&str]) -> Vec<String> {
    let state = run.body(body);
    let infos = state.elements().expect("element infos of a built body");
    let pairs: Vec<(ElementName, Fingerprint)> = names
        .iter()
        .map(|text| {
            let wanted = name(text);
            let info = infos
                .iter()
                .find(|info| info.name == wanted)
                .unwrap_or_else(|| panic!("{body} has no element {text}"));
            (info.name.clone(), info.fingerprint.clone())
        })
        .collect();
    describe(run, body, &pairs)
}

/// The description of one element of `body` at the end of `run`.
fn describe_one(run: &Run, body: &str, element: &str) -> String {
    describe_present(run, body, &[element]).remove(0)
}

/// The description of a stored element, by its name and stored fingerprint, in `body` at the end of `run` (the
/// element need not be there).
fn describe_stored(run: &Run, body: &str, element: &str, stored: &str) -> String {
    describe(run, body, &[(name(element), fingerprint(stored))]).remove(0)
}

/// `describe_elements` of `pairs` in the end state of `body`, with the document's names and units.
fn describe(run: &Run, body: &str, pairs: &[(ElementName, Fingerprint)]) -> Vec<String> {
    let state = run.body(body);
    let borrowed: Vec<(&ElementName, &Fingerprint)> =
        pairs.iter().map(|(name, print)| (name, print)).collect();
    run.with_names(|names| {
        describe_elements(
            &borrowed,
            &id(body),
            Some(state),
            None,
            names,
            run.document.units,
        )
    })
}

/// A diagnosis with one explanation at the loss position `position`.
fn explained(position: usize, text: &str) -> Diagnosis {
    Diagnosis {
        losses: BTreeMap::from([(
            position,
            Explanation {
                text: text.to_string(),
                after: None,
                fixes: Vec::new(),
                upstream: false,
            },
        )]),
        ..Diagnosis::default()
    }
}

/// `UseReferences` at `pointer` of each element, as a pick of it just before `feature` stores it.
fn use_each(run: &Run, feature: &str, body: &str, pointer: &str, elements: &[&str]) -> Vec<Fix> {
    elements
        .iter()
        .map(|element| use_reference(pointer, run.picked_before(feature, body, element)))
        .collect()
}

// Test 1: element descriptions (section 3.12).

#[test]
fn t001_faces_edges_and_corners_of_primitives() {
    let cube = run_features(&["b-1"], &[cube20()]);
    assert_eq!(
        describe_one(&cube, "b-1", "f-box1/top"),
        "the top of 'Box 1'"
    );
    assert_eq!(
        describe_one(&cube, "b-1", "edge(f-box1/front|f-box1/top)"),
        "the front top edge of 'Box 1'"
    );
    assert_eq!(
        describe_one(&cube, "b-1", "vertex(f-box1/front|f-box1/left|f-box1/top)"),
        "the front left top corner of 'Box 1'"
    );

    let cylinder = run_features(
        &["b-1"],
        &[
            json!({"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "5", "height": "10", "op": "new"}),
        ],
    );
    assert_eq!(
        describe_one(&cylinder, "b-1", "f-cyl1/side"),
        "the side of 'Cylinder 1'"
    );
}

#[test]
fn t001_faces_and_edges_of_an_extrude() {
    let run = run_features(&["b-1"], &block());

    assert_eq!(
        describe_one(&run, "b-1", "f-ext1/lateral/e-2"),
        "the right side of 'Extrude 1'"
    );
    assert_eq!(
        describe_one(&run, "b-1", "f-ext1/end-cap"),
        "the top of 'Extrude 1'"
    );
    assert_eq!(
        describe_one(&run, "b-1", "edge(f-ext1/end-cap|f-ext1/lateral/e-3)"),
        "the back top edge of 'Extrude 1'"
    );
}

/// Corpus R03's beveled outline: `e-5` runs from (80, 40) to (70, 50).
fn beveled_outline() -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 40.0]),
        line("e-5", [80.0, 40.0], [70.0, 50.0]),
        line("e-3", [70.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
    ]
}

/// Corpus R03's outline after the edit: the bevel deleted, a square corner.
fn square_outline() -> Vec<Value> {
    rectangle(1, 0.0, 0.0, 80.0, 50.0)
}

/// Corpus R03: the outline, its extrude of the beveled region as stored, and the fillet of the bevel's top edge and
/// the front top edge.
fn r03(outline: Vec<Value>) -> Run {
    run_features(
        &["b-1"],
        &[
            sketch("f-sk1", "XY", outline),
            extrude(
                "f-ext1",
                "f-sk1",
                json!([{"key": "e-1,e-2,e-3,e-4,e-5", "fp": "c=39.535865,24.725738|a=3950"}]),
                distance("30"),
                false,
                "new",
            ),
            fillet(
                "b-1",
                vec![
                    reference(
                        "edge(f-ext1/end-cap|f-ext1/lateral/e-5)",
                        "line|n=0.707107,-0.707107,0|c=75,45,30|l=14.142136|adj=2,2",
                    ),
                    reference(
                        "edge(f-ext1/end-cap|f-ext1/lateral/e-1)",
                        "line|n=1,0,0|c=40,0,30|l=80|adj=2,2",
                    ),
                ],
                "2",
            ),
        ],
    )
}

#[test]
fn t001_faces_and_edges_named_by_their_sketch_entity() {
    let beveled = r03(beveled_outline());
    let state_after_extrude = feature_support::output_body(&beveled, "f-ext1", "b-1");
    let describe_in_extrude = |element: &str| {
        let infos = state_after_extrude.elements().expect("element infos");
        let info = infos
            .iter()
            .find(|info| info.name == name(element))
            .expect("the element exists");
        beveled.with_names(|names| {
            describe_elements(
                &[(&info.name, &info.fingerprint)],
                &id("b-1"),
                Some(&state_after_extrude),
                Some(&id("f-fil1")),
                names,
                beveled.document.units,
            )
            .remove(0)
        })
    };

    assert_eq!(
        describe_in_extrude("f-ext1/lateral/e-5"),
        "the side of 'Extrude 1' made from the line from (80, 40) to (70, 50) in 'Sketch 1'"
    );
    assert_eq!(
        describe_in_extrude("edge(f-ext1/end-cap|f-ext1/lateral/e-5)"),
        "the top edge of 'Extrude 1' along the line from (80, 40) to (70, 50) in 'Sketch 1'"
    );

    let square = r03(square_outline());
    assert_eq!(
        describe_stored(
            &square,
            "b-1",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-5)",
            "line|n=0.707107,-0.707107,0|c=75,45,30|l=14.142136|adj=2,2"
        ),
        "the top edge of 'Extrude 1' along a line of 'Sketch 1'"
    );
}

#[test]
fn t001_faces_and_edges_a_cut_leaves() {
    // Newcomer error corpus NC05: a step 2.4 mm deep cut by 'Box 2'.
    let run = run_features(
        &["b-1"],
        &[
            box_in("f-box1", "b-1", ["40", "20", "10"], ["0", "0", "0"], "new"),
            box_in(
                "f-box2",
                "b-1",
                ["20", "30", "5"],
                ["10", "0", "7.6"],
                "cut",
            ),
        ],
    );

    assert_eq!(
        describe_one(&run, "b-1", "edge(f-box1/top|f-box2/left)"),
        "the top edge of the cut made by 'Box 2'"
    );
    assert_eq!(
        describe_one(&run, "b-1", "f-box2/left"),
        "a wall of the cut made by 'Box 2'"
    );
    assert_eq!(
        describe_one(&run, "b-1", "f-box2/bottom"),
        "the floor of the cut made by 'Box 2'"
    );
}

/// Corpus R12: `BLOCK` and the round cut `f-ext2` of the circle `e-5` from the top, `depth` deep, with a fillet of
/// radius 2 on the edge where the pocket's floor meets its wall.
fn r12(depth: &str) -> Run {
    let mut features = block();
    features.extend([
        datum_xy("f-dp1", "30"),
        sketch_on_datum(
            "f-sk2",
            "f-dp1",
            vec![json!({"id": "e-5", "type": "circle", "center": [40, 25], "radius": 10})],
        ),
        extrude(
            "f-ext2",
            "f-sk2",
            json!([{"key": "e-5", "fp": "c=40,25|a=314.159265"}]),
            distance(depth),
            true,
            "cut",
        ),
        fillet(
            "b-1",
            vec![reference(
                "edge(f-ext2/end-cap|f-ext2/lateral/e-5)",
                "circle|n=0,0,1|c=40,25,25|l=62.831853|r=10|adj=2,1",
            )],
            "2",
        ),
    ]);
    run_features(&["b-1"], &features)
}

#[test]
fn t001_edges_and_floor_of_a_pocket() {
    let run = r12("5");
    let before_fillet = body_before(&run, "f-fil1", "b-1");
    let describe_before = |element: &str| {
        let infos = before_fillet.elements().expect("element infos");
        let info = infos
            .iter()
            .find(|info| info.name == name(element))
            .expect("the element exists");
        run.with_names(|names| {
            describe_elements(
                &[(&info.name, &info.fingerprint)],
                &id("b-1"),
                Some(before_fillet),
                Some(&id("f-fil1")),
                names,
                run.document.units,
            )
            .remove(0)
        })
    };

    assert_eq!(
        describe_before("edge(f-ext2/end-cap|f-ext2/lateral/e-5)"),
        "the bottom edge of the cut made by 'Extrude 2'"
    );
    assert_eq!(
        describe_before("edge(f-ext1/end-cap|f-ext2/lateral/e-5)"),
        "the top rim of the cut made by 'Extrude 2'"
    );
    assert_eq!(
        describe_before("f-ext2/end-cap"),
        "the floor of the cut made by 'Extrude 2'"
    );
}

/// Corpus R30 (a) after its edit: `BLOCK` and the boss `f-ext2` of the circle `e-5`, extruded symmetric 40 from
/// z = 30 with Operation Cut, so it ends outside the body.
fn r30a_cut() -> Run {
    let mut features = block();
    features.extend([
        datum_xy("f-dp1", "30"),
        sketch_on_datum(
            "f-sk2",
            "f-dp1",
            vec![json!({"id": "e-5", "type": "circle", "center": [40, 25], "radius": 10})],
        ),
        extrude(
            "f-ext2",
            "f-sk2",
            json!([{"key": "e-5", "fp": "c=40,25|a=314.159265"}]),
            json!({"kind": "symmetric", "expr": "40"}),
            false,
            "cut",
        ),
    ]);
    run_features(&["b-1"], &features)
}

#[test]
fn t001_a_cap_a_cut_records_ending_outside_reads_as_the_tools_own() {
    let run = r30a_cut();

    // The boss's top rim: its end cap ends outside the body, so it never was a face the cut leaves.
    assert_eq!(
        describe_stored(
            &run,
            "b-1",
            "edge(f-ext2/end-cap|f-ext2/lateral/e-5)",
            "circle|n=0,0,1|c=40,25,50|l=62.831853|r=10|adj=2,1"
        ),
        "the top rim of 'Extrude 2'"
    );
    // The cut's opening in the top of 'Extrude 1' is still a rim of the cut.
    assert_eq!(
        describe_one(&run, "b-1", "edge(f-ext1/end-cap|f-ext2/lateral/e-5)"),
        "the top rim of the cut made by 'Extrude 2'"
    );
}

/// `BOX-BELOW`, the points `points` of `f-sk1` on XY and the through hole `f-hole1` of diameter 6 at `used`, with
/// `extra` params.
fn holes(points: Vec<Value>, used: &[&str], extra: Value) -> Run {
    let mut hole = json!({
        "id": "f-hole1",
        "type": "hole",
        "body": "b-1",
        "sketch": "f-sk1",
        "points": used,
        "diameter": "6",
        "extent": {"kind": "through_all"}
    });
    if let (Some(hole), Some(extra)) = (hole.as_object_mut(), extra.as_object()) {
        hole.extend(extra.clone());
    }
    run_features(
        &["b-1"],
        &[box_below(), sketch("f-sk1", "XY", points), hole],
    )
}

/// A point entity.
fn point(entity: &str, position: [f64; 2]) -> Value {
    json!({"id": entity, "type": "point", "position": position})
}

#[test]
fn t001_hole_faces_by_their_point() {
    // Corpus R43: holes at (0, 0) and (7, 0).
    let run = holes(
        vec![point("e-p1", [0.0, 0.0]), point("e-p2", [7.0, 0.0])],
        &["e-p1", "e-p2"],
        json!({}),
    );

    assert_eq!(
        describe_one(&run, "b-1", "edge(f-box1/top|f-hole1/bore/e-p2)"),
        "the top rim of the hole at (7, 0)"
    );
    assert_eq!(
        describe_one(&run, "b-1", "f-hole1/bore/e-p2"),
        "the wall of the hole at (7, 0)"
    );
}

#[test]
fn t001_copies_of_patterns_and_mirrors() {
    let pattern = |extra: Value| {
        let mut record = json!({
            "id": "f-lp1",
            "type": "linear_pattern",
            "body": "b-1",
            "direction": {"axis": {"kind": "origin", "axis": "X"}, "count": "3", "spacing": "20"}
        });
        if let (Some(record), Some(extra)) = (record.as_object_mut(), extra.as_object()) {
            record.extend(extra.clone());
        }
        record
    };
    let row = run_features(&["b-1"], &[cube10(), pattern(json!({}))]);
    assert_eq!(
        describe_one(&row, "b-1", "edge(f-box1/right@f-lp1:2|f-box1/top@f-lp1:2)"),
        "the right top edge of 'Box 1' of the third copy of 'Linear pattern 1'"
    );

    let grid = run_features(
        &["b-1"],
        &[
            cube10(),
            pattern(
                json!({"second_direction": {"axis": {"kind": "origin", "axis": "Y"}, "count": "2", "spacing": "15"}}),
            ),
        ],
    );
    assert_eq!(
        describe_one(&grid, "b-1", "f-box1/top@f-lp1:2.1"),
        "the top of 'Box 1' of the copy in row 2, column 3 of 'Linear pattern 1'"
    );

    let mirrored = run_features(
        &["b-1"],
        &[
            box_in("f-box1", "b-1", ["10", "10", "10"], ["10", "0", "0"], "new"),
            json!({"id": "f-mir1", "type": "mirror", "body": "b-1", "plane": {"kind": "origin", "plane": "YZ"}}),
        ],
    );
    assert_eq!(
        describe_one(&mirrored, "b-1", "f-box1/top@f-mir1:1"),
        "the top of 'Box 1' of the mirror image made by 'Mirror 1'"
    );
}

#[test]
fn t001_pieces_of_a_split_face() {
    let mut features = block();
    features.extend(slot(30.0, 50.0));
    let run = run_features(&["b-1"], &features);

    assert_eq!(
        describe_present(&run, "b-1", &["f-ext1/end-cap#1", "f-ext1/end-cap#2"]),
        [
            "the left part of the top of 'Extrude 1'",
            "the right part of the top of 'Extrude 1'"
        ]
    );
}

#[test]
fn t001_a_stored_piece_reads_by_the_side_its_stored_shape_lies_on() {
    // A piece stored without anchors under its sibling's shape (a guess, section 3.8) reads by where that shape lies
    // along the axis on which the two pieces differ, never by an axis they share.
    let mut features = block();
    features.extend(slot(30.0, 50.0));
    let run = run_features(&["b-1"], &features);

    assert_eq!(
        describe_stored(
            &run,
            "b-1",
            "f-ext1/end-cap#1",
            "plane|n=0,0,1|c=65,25,30|a=1500|adj=4,4"
        ),
        "the right part of the top of 'Extrude 1'"
    );
    assert_eq!(
        describe_stored(
            &run,
            "b-1",
            "f-ext1/end-cap#2",
            "plane|n=0,0,1|c=15,25,30|a=1500|adj=4,4"
        ),
        "the left part of the top of 'Extrude 1'"
    );
}

#[test]
fn t001_a_role_a_primitive_never_makes_reads_as_a_face_of_it() {
    let cube = run_features(&["b-1"], &[cube20()]);

    assert_eq!(
        describe_stored(
            &cube,
            "b-1",
            "f-box1/nothing",
            "plane|n=0,0,1|c=500,500,500|a=1"
        ),
        "a face of 'Box 1'"
    );
}

#[test]
fn t001_faces_of_dress_ups_and_splits() {
    // features/fillet.md test 1, features/shell.md test 1 and features/split_body.md test 1.
    let filleted = run_features(
        &["b-1"],
        &[
            cube20(),
            fillet(
                "b-1",
                vec![reference(
                    "edge(f-box1/front|f-box1/top)",
                    "line|n=1,0,0|c=0,-10,20|l=20",
                )],
                "2",
            ),
        ],
    );
    assert_eq!(
        describe_one(
            &filleted,
            "b-1",
            "f-fil1/fillet/edge(f-box1/front|f-box1/top)"
        ),
        "the rounded face of 'Fillet 1'"
    );

    let shelled = run_features(
        &["b-1"],
        &[
            box_in("f-box1", "b-1", ["80", "50", "30"], ["0", "0", "0"], "new"),
            json!({"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "2", "remove_faces": [reference("f-box1/top", "plane|n=0,0,1|c=0,0,30|a=4000")]}),
        ],
    );
    assert_eq!(
        describe_one(&shelled, "b-1", "f-sh1/inner/f-box1/bottom"),
        "the inside floor of 'Shell 1'"
    );
    assert_eq!(
        describe_one(&shelled, "b-1", "f-sh1/rim/f-box1/top"),
        "the rim of 'Shell 1'"
    );

    let split = run_features(
        &["b-1", "b-2"],
        &[
            cube20(),
            datum_xy("f-dp1", "5"),
            json!({"id": "f-split1", "type": "split_body", "body": "b-1", "plane": {"kind": "datum", "feature": "f-dp1"}, "target": "b-2"}),
        ],
    );
    assert_eq!(
        describe_one(&split, "b-1", "f-split1/split-positive"),
        "the cut face of 'Split body 1'"
    );
}

/// features/import.md test 1: a STEP file with one block [0,10]x[0,20]x[0,30], imported with op new; its faces have
/// fallback names `f-imp1/face#1` to `#6`.
fn imported_block() -> Run {
    let record = json!({
        "id": "f-imp1", "type": "import", "body": "b-1", "asset": step_asset_name(), "op": "new"
    });
    let text = record.to_string();
    let assets = box_step_asset("Block", [10.0, 20.0, 30.0]);
    Run::with_assets(feature_support::document(&["b-1"], &[&text]), &assets)
}

#[test]
fn t001_a_face_of_an_imported_file() {
    let run = imported_block();

    assert_eq!(
        describe_one(&run, "b-1", "f-imp1/face#4"),
        "a face of 'Import 1'"
    );
}

#[test]
fn t001_an_edge_of_fallback_faces_reads_by_its_position() {
    // Corpus R46 (a) after its edit: the stored edge between two fallback faces was a hole's top rim.
    let run = imported_block();

    assert_eq!(
        describe_stored(
            &run,
            "b-1",
            "edge(f-imp1/face#3|f-imp1/face#6)",
            "circle|n=0,0,1|c=30,10,5|l=18.849556|r=3|adj=2,1"
        ),
        "a round edge of 'Import 1' near (30, 10, 5)"
    );
}

#[test]
fn t001_a_face_of_a_feature_not_in_the_document() {
    let run = run_features(&["b-1"], &[cube20()]);

    assert_eq!(
        describe_stored(&run, "b-1", "f-gone/top", "plane|n=0,0,1|c=0,0,20|a=400"),
        "a face of a feature that is not in this document"
    );
}

#[test]
fn t001_the_rim_of_a_suppressed_peg() {
    // Corpus R23 (a) after its edit: peg A's extrude is suppressed, so neither face of its top rim is in the state.
    let mut peg_a = extrude(
        "f-ext1",
        "f-sk1",
        json!([{"key": "e-5", "fp": "c=-5,0|a=50.265482"}]),
        distance("20"),
        false,
        "join",
    );
    peg_a["suppressed"] = json!(true);
    let run = run_features(
        &["b-1"],
        &[
            box_in("f-box1", "b-1", ["60", "30", "10"], ["0", "0", "0"], "new"),
            datum_xy("f-dp1", "10"),
            sketch_on_datum(
                "f-sk1",
                "f-dp1",
                vec![json!({"id": "e-5", "type": "circle", "center": [-5, 0], "radius": 4})],
            ),
            peg_a,
            sketch_on_datum(
                "f-sk2",
                "f-dp1",
                vec![json!({"id": "e-7", "type": "circle", "center": [5, 0], "radius": 4})],
            ),
            extrude(
                "f-ext2",
                "f-sk2",
                json!([{"key": "e-7", "fp": "c=5,0|a=50.265482"}]),
                distance("20"),
                false,
                "join",
            ),
        ],
    );

    assert_eq!(
        describe_stored(
            &run,
            "b-1",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-5)",
            "circle|n=0,0,1|c=-5,0,30|l=25.132741|r=4|adj=2,1"
        ),
        "the top rim of 'Extrude 1'"
    );
}

#[test]
fn t001_the_rim_of_a_hole_whose_point_was_deleted() {
    // Corpus R43 (b) after its edit: e-p2 is no longer in 'Sketch 1'.
    let run = holes(
        vec![point("e-p1", [0.0, 0.0])],
        &["e-p1", "e-p2"],
        json!({}),
    );

    assert_eq!(
        describe_stored(
            &run,
            "b-1",
            "edge(f-box1/top|f-hole1/bore/e-p2)",
            "circle|n=0,0,1|c=7,0,0|l=18.849556|r=3|adj=2,1"
        ),
        "the top rim of a hole of 'Hole 1' near (7, 0, 0)"
    );
}

#[test]
fn t001_the_top_edge_of_a_deleted_arc() {
    // Corpus R51 after its edit: the back right corner arc e-6 is gone and the corner is sharp.
    let outline = vec![
        line("e-1", [5.0, 0.0], [75.0, 0.0]),
        line("e-2", [80.0, 5.0], [80.0, 50.0]),
        line("e-3", [80.0, 50.0], [5.0, 50.0]),
        line("e-4", [0.0, 45.0], [0.0, 5.0]),
        arc("e-5", [75.0, 5.0], 5.0, 3.0 * FRAC_PI_2, 0.0),
        arc("e-7", [5.0, 45.0], 5.0, FRAC_PI_2, PI),
        arc("e-8", [5.0, 5.0], 5.0, PI, 3.0 * FRAC_PI_2),
    ];
    let area = 80.0 * 50.0 - (4.0 - PI) * 25.0;
    let run = run_features(
        &["b-1"],
        &[
            sketch("f-sk1", "XY", outline),
            extrude(
                "f-ext1",
                "f-sk1",
                json!([{"key": "e-1,e-2,e-3,e-4,e-5,e-6,e-7,e-8", "fp": format!("c=40,25|a={area:.6}")}]),
                distance("10"),
                false,
                "new",
            ),
        ],
    );

    assert_eq!(
        describe_stored(
            &run,
            "b-1",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-6)",
            "circle|n=0,0,1|c=78.183099,48.183099,10|l=7.853982|r=5|adj=2,2"
        ),
        "the top edge of 'Extrude 1' along a curve of 'Sketch 1'"
    );
}

#[test]
fn t001_the_rims_of_a_counterbore_and_a_bore() {
    // Corpus R63 (b): before the edit the hole has a counterbore, after it the hole is simple.
    let counterbored = holes(
        vec![point("e-p1", [0.0, 0.0])],
        &["e-p1"],
        json!({"hole_type": {"kind": "counterbore", "diameter": "11", "depth": "4"}}),
    );
    assert_eq!(
        describe_one(
            &counterbored,
            "b-1",
            "edge(f-box1/top|f-hole1/counterbore/e-p1)"
        ),
        "the top rim of the counterbore of the hole at (0, 0)"
    );

    let simple = holes(vec![point("e-p1", [0.0, 0.0])], &["e-p1"], json!({}));
    assert_eq!(
        describe_one(&simple, "b-1", "edge(f-box1/top|f-hole1/bore/e-p1)"),
        "the top rim of the hole at (0, 0)"
    );
}

#[test]
fn t001_an_edge_between_two_boxes() {
    // Corpus R21 (b) after its edit: a taller box joined against the right side.
    let run = run_features(
        &["b-1"],
        &[
            box_in("f-box1", "b-1", ["20", "20", "10"], ["0", "0", "0"], "new"),
            box_in(
                "f-box2",
                "b-1",
                ["20", "20", "15"],
                ["20", "0", "0"],
                "join",
            ),
        ],
    );

    assert_eq!(
        describe_one(&run, "b-1", "edge(f-box1/top|f-box2/left)"),
        "the edge between the top of 'Box 1' and the left side of 'Box 2'"
    );
    assert_eq!(
        describe_one(&run, "b-1", "f-box1/front"),
        "the front side of 'Box 1'"
    );
}

#[test]
fn t001_rule_6_names_both_faces() {
    let run = run_features(
        &["b-1"],
        &[
            cube20(),
            fillet(
                "b-1",
                vec![reference(
                    "edge(f-box1/front|f-box1/top)",
                    "line|n=1,0,0|c=0,-10,20|l=20",
                )],
                "2",
            ),
        ],
    );

    assert_eq!(
        describe_one(
            &run,
            "b-1",
            "edge(f-box1/top|f-fil1/fillet/edge(f-box1/front|f-box1/top))"
        ),
        "the edge between the top of 'Box 1' and the rounded face of 'Fillet 1'"
    );
}

// Test 2: the clash rule (corpus R66).

/// The lines of the notch of corpus R66, drawn in place of `e-3`.
fn notch() -> Vec<Value> {
    vec![
        line("e-9", [80.0, 50.0], [50.0, 50.0]),
        line("e-10", [50.0, 50.0], [50.0, 40.0]),
        line("e-11", [50.0, 40.0], [30.0, 40.0]),
        line("e-12", [30.0, 40.0], [30.0, 50.0]),
        line("e-13", [30.0, 50.0], [0.0, 50.0]),
    ]
}

/// The back top edge of `BLOCK`, as stored before the edit.
fn back_top_edge() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-3)",
        "line|n=1,0,0|c=40,50,30|l=80|adj=2,2",
    )
}

/// Corpus R66 after its edit: `BLOCK` with the notch in place of `e-3`, and the fillet of radius 1 on the back top
/// edge as stored before the edit.
fn r66() -> Run {
    let mut features = block_with(edited(
        rectangle(1, 0.0, 0.0, 80.0, 50.0),
        &["e-3"],
        notch(),
    ));
    features.push(fillet("b-1", vec![back_top_edge()], "1"));
    run_features(&["b-1"], &features)
}

/// The five top edges along the notch, from `e-9` to `e-13`.
const NOTCH_TOP_EDGES: [&str; 5] = [
    "edge(f-ext1/end-cap|f-ext1/lateral/e-9)",
    "edge(f-ext1/end-cap|f-ext1/lateral/e-10)",
    "edge(f-ext1/end-cap|f-ext1/lateral/e-11)",
    "edge(f-ext1/end-cap|f-ext1/lateral/e-12)",
    "edge(f-ext1/end-cap|f-ext1/lateral/e-13)",
];

#[test]
fn t002_elements_that_would_read_the_same_get_their_entity_clause() {
    let run = r66();
    let state = feature_support::output_body(&run, "f-ext1", "b-1");
    let infos = state.elements().expect("element infos");
    let pairs: Vec<(&ElementName, &Fingerprint)> = NOTCH_TOP_EDGES
        .iter()
        .map(|text| {
            let info = infos
                .iter()
                .find(|info| info.name == name(text))
                .unwrap_or_else(|| panic!("no edge {text}"));
            (&info.name, &info.fingerprint)
        })
        .collect();

    let texts = run.with_names(|names| {
        describe_elements(
            &pairs,
            &id("b-1"),
            Some(&state),
            None,
            names,
            run.document.units,
        )
    });

    assert_eq!(
        texts[0],
        "the back top edge of 'Extrude 1' along the line from (80, 50) to (50, 50) in 'Sketch 1'"
    );
    let mut distinct = texts.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), 5, "five different texts: {texts:?}");
}

#[test]
fn t002_a_stored_edge_whose_face_is_gone_takes_its_word_from_the_edge() {
    let run = r66();

    assert_eq!(
        describe_stored(
            &run,
            "b-1",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-3)",
            "line|n=1,0,0|c=40,50,30|l=80|adj=2,2"
        ),
        "the back top edge of 'Extrude 1'"
    );
}

// Test 15: the corpus messages of phase 1's rows, with the diagnosis each cause implies.

#[test]
fn t015_r56b_a_body_used_as_a_tool() {
    let run = run_features(
        &["b-1", "b-2"],
        &[
            cube20(),
            box_in("f-box2", "b-2", ["10", "10", "10"], ["0", "0", "20"], "new"),
            json!({"id": "f-bool1", "type": "boolean", "target": "b-2", "tools": ["b-1"], "op": "join"}),
            fillet(
                "b-1",
                vec![reference(
                    "edge(f-box2/front|f-box2/top)",
                    "line|n=1,0,0|c=0,-5,30|l=10|adj=2,2",
                )],
                "1",
            ),
        ],
    );
    let diagnosis = Diagnosis {
        because: Some(Because::UsedAsTool {
            feature: id("f-bool1"),
            target: id("b-2"),
        }),
        ..Diagnosis::default()
    };

    run.assert_error("f-fil1", "body_empty");
    assert_eq!(
        run.render_error("f-fil1", &diagnosis),
        "Body 'Body 1' is empty here because 'Boolean 1' uses it as a tool, which empties it."
    );
    let fixes = run.error_fixes("f-fil1", &diagnosis);
    assert!(
        matches!(
            fixes.as_slice(),
            [Fix::UseBody { field, body }] if field.path == "body" && *body == id("b-2")
        ),
        "[Use body 'Body 2']: {fixes:?}"
    );
}

#[test]
fn t015_r66_a_widened_edge() {
    let run = r66();
    let diagnosis = Diagnosis {
        by: Some(id("f-sk1")),
        ..Diagnosis::default()
    };

    run.assert_warnings("f-fil1", &["reference_widened"]);
    assert_eq!(
        run.render_warning("f-fil1", "reference_widened", &diagnosis),
        "The back top edge of 'Extrude 1' selected for Edges now covers 5 edges, because the line it was made from was replaced by 5 lines in 'Sketch 1'. Keep all of them, or use only one."
    );
    let mut expected = vec![Fix::KeepMatch {
        pointer: "/edges/0".to_string(),
    }];
    expected.extend(use_each(
        &run,
        "f-fil1",
        "b-1",
        "/edges/0",
        &[NOTCH_TOP_EDGES[0]],
    ));
    expected.push(pick("/edges/0"));
    assert_eq!(
        run.warning_fixes("f-fil1", "reference_widened", &diagnosis),
        expected
    );
}

#[test]
fn t015_nc07_an_extrude_on_a_sketch_with_a_conflict() {
    let mut conflicted = sketch("f-sk1", "XY", rectangle(1, 0.0, 0.0, 40.0, 20.0));
    conflicted["constraints"] = json!([
        {"id": "c-1", "type": "distance", "line": "e-1", "expr": "40 mm"},
        {"id": "c-2", "type": "distance", "line": "e-1", "expr": "30 mm"}
    ]);
    let run = run_features(
        &["b-1"],
        &[
            conflicted,
            extrude(
                "f-ext1",
                "f-sk1",
                json!([{"key": "e-1,e-2,e-3,e-4", "fp": "c=20,10|a=800"}]),
                distance("10"),
                false,
                "new",
            ),
        ],
    );

    run.assert_error("f-sk1", "sketch_conflict");
    run.assert_error("f-ext1", "upstream_failed");
    assert_eq!(
        run.render_error("f-ext1", &Diagnosis::default()),
        "'Sketch 1' has an error, so this extrude cannot use it. Fix 'Sketch 1' first."
    );
    assert_eq!(
        run.error_fixes("f-ext1", &Diagnosis::default()),
        [Fix::Show {
            feature: id("f-sk1"),
            entity: None,
        }]
    );
}

/// Newcomer error corpus NC11: `BLOCK` with its back line deleted, and a fillet of radius 2 on the back top edge.
fn nc11() -> Run {
    let mut features = block_with(edited(
        rectangle(1, 0.0, 0.0, 80.0, 50.0),
        &["e-3"],
        Vec::new(),
    ));
    features.push(fillet("b-1", vec![back_top_edge()], "2"));
    run_features(&["b-1"], &features)
}

#[test]
fn t015_nc11_a_profile_whose_line_was_deleted() {
    let run = nc11();

    run.assert_error("f-ext1", "profile_not_found");
    assert_eq!(
        run.render_error("f-ext1", &Diagnosis::default()),
        "The region of 'Sketch 1' selected for Profile is gone because part of its outline was deleted."
    );
    assert_eq!(
        run.error_fixes("f-ext1", &Diagnosis::default()),
        [
            Fix::Show {
                feature: id("f-sk1"),
                entity: None,
            },
            pick("/profile/regions/0"),
        ]
    );
}

#[test]
fn t015_nc11_a_body_whose_maker_failed() {
    let run = nc11();
    let diagnosis = Diagnosis {
        because: Some(Because::Failed(id("f-ext1"))),
        ..Diagnosis::default()
    };

    run.assert_error("f-fil1", "body_empty");
    assert_eq!(
        run.render_error("f-fil1", &diagnosis),
        "Body 'Body 1' is empty here because 'Extrude 1', which makes it, failed. Fix 'Extrude 1' first."
    );
    assert_eq!(
        run.error_fixes("f-fil1", &diagnosis),
        [Fix::Show {
            feature: id("f-ext1"),
            entity: None,
        }]
    );
}

#[test]
fn t015_r03_a_deleted_bevel() {
    let run = r03(square_outline());
    let diagnosis = explained(0, "is gone because that line was deleted from 'Sketch 1'");

    run.assert_warnings("f-fil1", &["reference_missing"]);
    assert_eq!(
        run.render_warning("f-fil1", "reference_missing", &diagnosis),
        "The top edge of 'Extrude 1' along a line of 'Sketch 1' selected for Edges is gone because that line was deleted from 'Sketch 1', so it was skipped."
    );
    let mut expected = use_each(
        &run,
        "f-fil1",
        "b-1",
        "/edges/0",
        &[
            "edge(f-ext1/end-cap|f-ext1/lateral/e-2)",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-3)",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-4)",
            "edge(f-ext1/lateral/e-2|f-ext1/lateral/e-3)",
        ],
    );
    expected.push(remove_entry("/edges/0"));
    expected.push(pick("/edges/0"));
    assert_eq!(
        run.warning_fixes("f-fil1", "reference_missing", &diagnosis),
        expected
    );
    assert_messages_pass_lint(&run, "f-fil1");
}

#[test]
fn t015_r12_a_pocket_made_through() {
    let run = r12("30");
    let diagnosis = explained(
        0,
        "is no longer made by 'Extrude 2', which goes all the way through the body",
    );

    run.assert_error("f-fil1", "references_not_found");
    assert_eq!(
        run.render_error("f-fil1", &diagnosis),
        "The bottom edge of the cut made by 'Extrude 2' selected for Edges is no longer made by 'Extrude 2', which goes all the way through the body."
    );
    let mut expected = use_each(
        &run,
        "f-fil1",
        "b-1",
        "/edges/0",
        &[
            "edge(f-ext1/end-cap|f-ext2/lateral/e-5)",
            "edge(f-ext1/start-cap|f-ext2/lateral/e-5)",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-1)",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-3)",
        ],
    );
    expected.push(pick("/edges/0"));
    assert_eq!(run.error_fixes("f-fil1", &diagnosis), expected);
    assert_messages_pass_lint(&run, "f-fil1");
}

#[test]
fn t015_r16_a_cut_that_removes_the_filleted_edge() {
    let run = run_features(
        &["b-1"],
        &[
            cube20(),
            sketch("f-sk2", "YZ", rectangle(1, -11.0, 15.0, -5.0, 21.0)),
            extrude(
                "f-ext2",
                "f-sk2",
                json!([{"key": "e-1,e-2,e-3,e-4", "fp": "c=-8,18|a=36"}]),
                json!({"kind": "symmetric", "expr": "30"}),
                false,
                "cut",
            ),
            fillet(
                "b-1",
                vec![reference(
                    "edge(f-box1/front|f-box1/top)",
                    "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2",
                )],
                "2",
            ),
        ],
    );
    let diagnosis = explained(
        0,
        "was cut away by 'Extrude 1', which comes before this fillet",
    );

    run.assert_error("f-fil1", "references_not_found");
    assert_eq!(
        run.render_error("f-fil1", &diagnosis),
        "The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet."
    );
    let mut expected = use_each(
        &run,
        "f-fil1",
        "b-1",
        "/edges/0",
        &[
            "edge(f-box1/front|f-ext2/lateral/e-1)",
            "edge(f-box1/top|f-ext2/lateral/e-2)",
            "edge(f-box1/front|f-box1/left)",
            "edge(f-box1/front|f-box1/right)",
            "edge(f-box1/left|f-box1/top)",
        ],
    );
    expected.push(pick("/edges/0"));
    assert_eq!(run.error_fixes("f-fil1", &diagnosis), expected);
    assert_messages_pass_lint(&run, "f-fil1");
}

#[test]
fn t015_r41_a_copy_that_is_gone() {
    let run = run_features(
        &["b-1"],
        &[
            cube10(),
            json!({
                "id": "f-lp1",
                "type": "linear_pattern",
                "body": "b-1",
                "direction": {"axis": {"kind": "origin", "axis": "X"}, "count": "2", "spacing": "20"}
            }),
            fillet(
                "b-1",
                vec![
                    reference(
                        "edge(f-box1/right@f-lp1:1|f-box1/top@f-lp1:1)",
                        "line|n=0,1,0|c=25,0,10|l=10|adj=2,2",
                    ),
                    reference(
                        "edge(f-box1/right@f-lp1:2|f-box1/top@f-lp1:2)",
                        "line|n=0,1,0|c=45,0,10|l=10|adj=2,2",
                    ),
                ],
                "1",
            ),
        ],
    );
    let diagnosis = explained(
        1,
        "is gone because 'Linear pattern 1' no longer makes its third copy",
    );

    run.assert_warnings("f-fil1", &["reference_missing"]);
    assert_eq!(
        run.render_warning("f-fil1", "reference_missing", &diagnosis),
        "The right top edge of 'Box 1' of the third copy of 'Linear pattern 1' selected for Edges is gone because 'Linear pattern 1' no longer makes its third copy, so it was skipped."
    );
    let mut expected = use_each(
        &run,
        "f-fil1",
        "b-1",
        "/edges/1",
        &[
            "edge(f-box1/back@f-lp1:1|f-box1/right@f-lp1:1)",
            "edge(f-box1/front@f-lp1:1|f-box1/right@f-lp1:1)",
            "edge(f-box1/bottom@f-lp1:1|f-box1/right@f-lp1:1)",
            "edge(f-box1/back@f-lp1:1|f-box1/top@f-lp1:1)",
        ],
    );
    expected.push(remove_entry("/edges/1"));
    expected.push(pick("/edges/1"));
    assert_eq!(
        run.warning_fixes("f-fil1", "reference_missing", &diagnosis),
        expected
    );
    assert_messages_pass_lint(&run, "f-fil1");
}

// Test 16: repairs of references and regions that are still used.

// Test 8: unknown settings (sections 3.2 and 4.2), as recompute reports them for a document.

/// `CUBE20` and a 2 mm shell of it whose record has the extra keys `extra` (JSON members).
fn shell_with(extra: Value) -> Run {
    let mut shell = json!({"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "2"});
    for (key, value) in extra.as_object().expect("the extra keys are a JSON object") {
        shell[key] = value.clone();
    }
    run_features(&["b-1"], &[cube20(), shell])
}

/// The top of `CUBE20` as a stored reference.
fn cube20_top() -> Value {
    reference("f-box1/top", "plane|n=0,0,1|c=0,0,20|a=400|adj=4,4")
}

#[test]
fn t008_a_misspelled_setting_offers_its_rename() {
    let run = shell_with(json!({"remove_face": [cube20_top()]}));

    // The misspelled key is ignored, so the shell builds a closed hollow and warns.
    run.assert_warnings("f-sh1", &["unknown_params"]);
    assert_eq!(
        run.messages("f-sh1"),
        ["'remove_face' is not a Shell setting, so it is ignored. Did you mean Faces to remove?"]
    );
    assert_eq!(
        run.feature("f-sh1").messages[0].fixes,
        [Fix::RenameKey {
            from: "remove_face".to_string(),
            to: "remove_faces".to_string(),
        }]
    );
    assert_messages_pass_lint(&run, "f-sh1");
}

#[test]
fn t008_an_unknown_setting_without_a_near_name_is_kept_and_ignored() {
    let run = shell_with(json!({"zzz": 1}));

    run.assert_warnings("f-sh1", &["unknown_params"]);
    assert_eq!(
        run.messages("f-sh1"),
        [
            "This shell has settings Partsmith does not know ('zzz'). They are kept in the file and ignored."
        ]
    );
    assert_eq!(run.feature("f-sh1").messages[0].fixes, []);
}

#[test]
fn t008_both_settings_list_together_with_the_one_rename() {
    let run = shell_with(json!({"remove_face": [cube20_top()], "zzz": 1}));

    run.assert_warnings("f-sh1", &["unknown_params"]);
    assert_eq!(
        run.messages("f-sh1"),
        [
            "This shell has settings Partsmith does not know ('remove_face', 'zzz'). They are kept in the file and ignored."
        ]
    );
    assert_eq!(
        run.feature("f-sh1").messages[0].fixes,
        [Fix::RenameKey {
            from: "remove_face".to_string(),
            to: "remove_faces".to_string(),
        }]
    );
}

// Test 15, the rows of the feature rules (sections 3.15 and 4.3): newcomer error corpus NC02 to NC05 and NC12, and
// corpus R26 and R48.

/// `f-sk2` with the circle `e-5` at (40, 25), radius 5, on `plane` (NC02 to NC04).
fn small_circle_sketch(plane: Value) -> Value {
    json!({"id": "f-sk2", "type": "sketch", "plane": plane, "entities": [{"id": "e-5", "type": "circle", "center": [40, 25], "radius": 5}]})
}

/// `f-ext2`, the circle of `small_circle_sketch` extruded `depth` with `op`.
fn small_circle_extrude(depth: &str, op: &str) -> Value {
    extrude(
        "f-ext2",
        "f-sk2",
        json!([{"key": "e-5", "fp": "c=40,25|a=78.539816"}]),
        distance(depth),
        false,
        op,
    )
}

/// `BLOCK` with `others` after it.
fn on_block(others: Vec<Value>) -> Run {
    let mut features = block();
    features.extend(others);
    run_features(&["b-1"], &features)
}

/// NC03 and NC04: `BLOCK`, the datum `f-dp1` at z = 40 and the circle extruded `depth` up from it with `op`.
fn above_the_block(depth: &str, op: &str) -> Run {
    on_block(vec![
        datum_xy("f-dp1", "40"),
        small_circle_sketch(json!({"kind": "datum", "feature": "f-dp1"})),
        small_circle_extrude(depth, op),
    ])
}

/// [Attach 'Sketch 2' to the top of 'Extrude 1'], with the top as a pick just before `f-ext2`.
fn attach_sketch_2_to_the_top(run: &Run) -> Fix {
    Fix::Attach {
        sketch: id("f-sk2"),
        body: id("b-1"),
        face: run.picked_before("f-ext2", "b-1", "f-ext1/end-cap"),
    }
}

#[test]
fn t015_nc02_a_join_buried_in_the_body() {
    let run = on_block(vec![
        small_circle_sketch(json!({"kind": "origin", "plane": "XY"})),
        small_circle_extrude("10", "join"),
    ]);

    run.assert_warnings("f-ext2", &["no_effect"]);
    let diagnosis = Diagnosis::default();
    assert_eq!(
        run.render_warning("f-ext2", "no_effect", &diagnosis),
        "This extrude adds nothing to body 'Body 1': it lies entirely inside the body."
    );
    assert_eq!(
        run.warning_fixes("f-ext2", "no_effect", &diagnosis),
        [attach_sketch_2_to_the_top(&run)]
    );
    assert_messages_pass_lint(&run, "f-ext2");
}

#[test]
fn t015_nc03_a_cut_that_points_away() {
    let run = above_the_block("15", "cut");

    run.assert_warnings("f-ext2", &["no_effect"]);
    let diagnosis = Diagnosis::default();
    assert_eq!(
        run.render_warning("f-ext2", "no_effect", &diagnosis),
        "This cut does not touch body 'Body 1': it starts 10 mm from the body and points away from it."
    );
    assert_eq!(
        run.warning_fixes("f-ext2", "no_effect", &diagnosis),
        [set_reverse(true)]
    );
    assert_messages_pass_lint(&run, "f-ext2");
}

#[test]
fn t015_nc03_a_cut_that_misses_both_ways_offers_no_reverse() {
    // Reversed, the cut ends at z = 35 and still misses, so the sentence stays and Reverse is not offered.
    let run = above_the_block("5", "cut");

    run.assert_warnings("f-ext2", &["no_effect"]);
    let diagnosis = Diagnosis::default();
    assert_eq!(
        run.render_warning("f-ext2", "no_effect", &diagnosis),
        "This cut does not touch body 'Body 1': it starts 10 mm from the body and points away from it."
    );
    assert_eq!(run.warning_fixes("f-ext2", "no_effect", &diagnosis), []);
}

#[test]
fn t015_nc04_a_join_that_floats() {
    let run = above_the_block("15", "join");

    run.assert_warnings("f-ext2", &["separate_solid"]);
    let diagnosis = Diagnosis::default();
    assert_eq!(
        run.render_warning("f-ext2", "separate_solid", &diagnosis),
        "This extrude does not touch body 'Body 1', so the body now has 2 separate pieces."
    );
    assert_eq!(
        run.warning_fixes("f-ext2", "separate_solid", &diagnosis),
        [Fix::NewBody, attach_sketch_2_to_the_top(&run)]
    );
    assert_messages_pass_lint(&run, "f-ext2");
}

#[test]
fn t015_nc05_a_fillet_too_large_for_a_step() {
    // A step 2.4 mm deep cut by 'Box 2'; the fillet rounds the edge between the top and the step's riser at x = 0.
    let run = run_features(
        &["b-1"],
        &[
            box_in("f-box1", "b-1", ["40", "20", "10"], ["0", "0", "0"], "new"),
            box_in(
                "f-box2",
                "b-1",
                ["20", "30", "5"],
                ["10", "0", "7.6"],
                "cut",
            ),
            fillet(
                "b-1",
                vec![reference(
                    "edge(f-box1/top|f-box2/left)",
                    "line|n=0,1,0|c=0,0,10|l=20|adj=2,2",
                )],
                "3",
            ),
        ],
    );

    run.assert_error("f-fil1", "fillet_too_large");
    let diagnosis = Diagnosis::default();
    assert_eq!(
        run.render_error("f-fil1", &diagnosis),
        "Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm. The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across."
    );
    assert_eq!(
        run.error_fixes("f-fil1", &diagnosis),
        [set_field("radius", "Radius", json!("2.3 mm"))]
    );
    assert_messages_pass_lint(&run, "f-fil1");
}

#[test]
fn t015_nc12_a_hole_beside_the_body() {
    let run = on_block(vec![
        json!({
            "id": "f-sk2",
            "type": "sketch",
            "plane": {
                "kind": "face",
                "body": "b-1",
                "face": reference("f-ext1/end-cap", "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"),
                "fallback": {"origin": [0, 0, 30], "x_axis": [1, 0, 0], "normal": [0, 0, 1]},
                "fallback_frame": "body"
            },
            "entities": [point("e-p1", [95.0, 25.0])]
        }),
        json!({"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk2", "points": ["e-p1"], "diameter": "5", "extent": {"kind": "through_all"}}),
    ]);

    run.assert_warnings("f-hole1", &["hole_misses_body"]);
    let diagnosis = Diagnosis::default();
    assert_eq!(
        run.render_warning("f-hole1", "hole_misses_body", &diagnosis),
        "The hole at (95, 25) in 'Sketch 2' does not touch body 'Body 1'. Move the point onto the body."
    );
    // Reversed, it misses too, so there is no Reverse action.
    assert_eq!(
        run.warning_fixes("f-hole1", "hole_misses_body", &diagnosis),
        []
    );
    assert_messages_pass_lint(&run, "f-hole1");
}

/// R26 after its edit: `BLOCK`'s extrude deleted and an extrude `f-ext3` 80 tall drawn in its place ('Extrude 1'),
/// with 'Sketch 2' still stored on the deleted extrude's top and a boss `f-ext2` on it.
fn r26() -> Run {
    run_features(
        &["b-1"],
        &[
            sketch("f-sk1", "XY", rectangle(1, 0.0, 0.0, 80.0, 50.0)),
            extrude(
                "f-ext3",
                "f-sk1",
                json!([{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]),
                distance("80"),
                false,
                "new",
            ),
            json!({
                "id": "f-sk2",
                "type": "sketch",
                "plane": {
                    "kind": "face",
                    "body": "b-1",
                    "face": reference("f-ext1/end-cap", "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"),
                    "fallback": {"origin": [0, 0, 30], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}
                },
                "entities": [{"id": "e-5", "type": "circle", "center": [40, 25], "radius": 10}]
            }),
            extrude(
                "f-ext2",
                "f-sk2",
                json!([{"key": "e-5", "fp": "c=40,25|a=314.159265"}]),
                distance("10"),
                false,
                "join",
            ),
        ],
    )
}

#[test]
fn t015_r26_a_sketch_whose_face_feature_was_deleted() {
    let run = r26();
    // The cause `NotInDocument` explains a face already described as a feature's not in this document as gone
    // (recompute.md section 3.10).
    let diagnosis = explained(0, "is gone");

    run.assert_warnings("f-sk2", &["plane_fallback"]);
    assert_eq!(
        run.render_warning("f-sk2", "plane_fallback", &diagnosis),
        "The face this sketch is on, a face of a feature that is not in this document, is gone, so the sketch uses the position saved with it."
    );
    // The top of 'Extrude 1' first, then the nearest planar faces (naming.md test 133), then Pick.
    let mut expected: Vec<Fix> = [
        "f-ext3/end-cap",
        "f-ext3/lateral/e-1",
        "f-ext3/lateral/e-3",
        "f-ext3/start-cap",
        "f-ext3/lateral/e-2",
    ]
    .iter()
    .map(|face| Fix::Attach {
        sketch: id("f-sk2"),
        body: id("b-1"),
        face: run.picked_before("f-sk2", "b-1", face),
    })
    .collect();
    expected.push(pick("/plane/face"));
    assert_eq!(
        run.warning_fixes("f-sk2", "plane_fallback", &diagnosis),
        expected
    );
}

#[test]
fn t015_r26_the_boss_on_it_names_the_sketch_as_the_cause() {
    let run = r26();
    let diagnosis = Diagnosis {
        lost_face: Some(id("f-sk2")),
        ..Diagnosis::default()
    };

    run.assert_warnings("f-ext2", &["no_effect"]);
    assert_eq!(
        run.render_warning("f-ext2", "no_effect", &diagnosis),
        "This extrude adds nothing to body 'Body 1' because 'Sketch 2' lost the face it was on. Fix 'Sketch 2' first."
    );
    assert_eq!(
        run.warning_fixes("f-ext2", "no_effect", &diagnosis),
        [Fix::Show {
            feature: id("f-sk2"),
            entity: None,
        }]
    );
}

#[test]
fn t015_r48_bare_numbers_after_switching_to_inches() {
    // R48 after its edit: `BLOCK` and SLOT(30, 50) with bare numbers, now in inches, and a fillet of radius '2' on the
    // front top edge of the right piece.
    let mut features = block();
    features.extend(slot(30.0, 50.0));
    features.push(fillet(
        "b-1",
        vec![reference(
            "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)",
            "line|n=1,0,0|c=65,0,30|l=30|adj=2,2",
        )],
        "2",
    ));
    let texts: Vec<String> = features.iter().map(Value::to_string).collect();
    let records: Vec<&str> = texts.iter().map(String::as_str).collect();
    let run = Run::new(parse_document(&document_text(
        "in",
        &[],
        &["b-1"],
        &records,
    )));

    run.assert_error("f-fil1", "fillet_too_large");
    let diagnosis = Diagnosis::default();
    assert_eq!(
        run.render_error("f-fil1", &diagnosis),
        "Fillet radius 2 in is too large for the selected edges: the largest that works is about 1.9 in. '2' has no unit, so it means 2 in in this document."
    );
    assert_eq!(
        run.error_fixes("f-fil1", &diagnosis),
        [
            set_field("radius", "Radius", json!("2 mm")),
            set_field("radius", "Radius", json!("1.9 in")),
        ]
    );
    assert_messages_pass_lint(&run, "f-fil1");
}

#[test]
fn t016_a_widened_edge_repairs_as_ambiguous_with_nothing_lost() {
    let run = r66();

    let Some(Repair::Element { pointer, loss, .. }) =
        run.warning("f-fil1", "reference_widened").repair()
    else {
        panic!("expected an element repair");
    };
    assert_eq!(pointer, "/edges/0");
    assert!(loss.ambiguous);
    assert!(loss.lost.is_empty());
    let mut candidates: Vec<String> = loss
        .candidates
        .iter()
        .map(|candidate| candidate.reference.query.to_string())
        .collect();
    candidates.sort();
    let mut expected: Vec<String> = NOTCH_TOP_EDGES.iter().map(ToString::to_string).collect();
    expected.sort();
    assert_eq!(candidates, expected);
}

#[test]
fn t016_a_split_region_repairs_as_ambiguous() {
    // Corpus R50: a circle drawn inside the selected region.
    let mut entities = rectangle(1, 0.0, 0.0, 80.0, 50.0);
    entities.push(json!({"id": "e-5", "type": "circle", "center": [40, 25], "radius": 10}));
    let run = run_features(&["b-1"], &block_with(entities));

    let Some(Repair::Region {
        pointer, ambiguous, ..
    }) = run.warning("f-ext1", "region_split").repair()
    else {
        panic!("expected a region repair");
    };
    assert_eq!(pointer, "/profile/regions/0");
    assert!(ambiguous);
}

#[test]
fn t016_a_profile_whose_line_was_deleted_repairs_as_missing() {
    let run = nc11();

    let repairs = run.error("f-ext1").repairs();
    let [
        Repair::Region {
            pointer, ambiguous, ..
        },
    ] = repairs.as_slice()
    else {
        panic!("expected one region repair, got {repairs:?}");
    };
    assert_eq!(pointer, "/profile/regions/0");
    assert!(!*ambiguous);
}

// Test 17: candidates a fix could not use are not offered (section 3.8).

/// The candidates of the first loss of `feature`'s error or warning, by name.
fn candidate_names(run: &Run, feature: &str) -> Vec<String> {
    let loss = match run.feature(feature).error.as_ref() {
        Some(FeatureError::ReferencesNotFound { losses, .. }) => losses[0].clone(),
        Some(FeatureError::ReferenceNotFound { loss, .. }) => loss.clone(),
        Some(other) => panic!("{feature} failed with {other:?}"),
        None => match run.feature(feature).warnings.first() {
            Some(FeatureWarning::ReferenceLost { loss, .. }) => loss.clone(),
            other => panic!("{feature} has no loss: {other:?}"),
        },
    };
    loss.candidates
        .iter()
        .map(|candidate| candidate.reference.query.to_string())
        .collect()
}

#[test]
fn t017_a_chamfer_is_not_offered_the_smooth_edges_of_a_fillet() {
    // Corpus R18 (a): a fillet inserted before a chamfer on the same vertical edge.
    let vertical = reference(
        "edge(f-ext1/lateral/e-1|f-ext1/lateral/e-2)",
        "line|n=0,0,1|c=80,0,15|l=30|adj=2,2",
    );
    let mut features = block();
    features.push(fillet("b-1", vec![vertical.clone()], "5"));
    features.push(json!({"id": "f-cha1", "type": "chamfer", "body": "b-1", "edges": [vertical], "distance": {"kind": "equal", "expr": "2"}}));
    let run = run_features(&["b-1"], &features);

    let candidates = candidate_names(&run, "f-cha1");

    assert!(
        candidates.iter().all(|candidate| !candidate
            .contains("f-fil1/fillet/edge(f-ext1/lateral/e-1|f-ext1/lateral/e-2)")),
        "{candidates:?}"
    );
    assert_eq!(
        candidates.first().map(String::as_str),
        Some("edge(f-ext1/end-cap|f-ext1/lateral/e-2)")
    );
}

#[test]
fn t017_a_list_is_not_offered_what_another_entry_resolves_to() {
    // Corpus R03: the front top edge, which `edges[1]` resolves to, is no candidate of `edges[0]`.
    let run = r03(square_outline());

    let candidates = candidate_names(&run, "f-fil1");

    assert!(
        !candidates.contains(&"edge(f-ext1/end-cap|f-ext1/lateral/e-1)".to_string()),
        "{candidates:?}"
    );
}

#[test]
fn t017_an_axis_keeps_the_straight_tangent_edges_of_a_fillet() {
    // Corpus R59 (a): the edge used as a revolve axis is rounded upstream; an axis can use a straight edge.
    let axis_edge = reference(
        "edge(f-box1/front|f-box1/left)",
        "line|n=0,0,1|c=-5,0,5|l=10|adj=2,2",
    );
    let run = run_features(
        &["b-1", "b-2"],
        &[
            box_in("f-box1", "b-2", ["10", "10", "10"], ["0", "5", "0"], "new"),
            fillet("b-2", vec![axis_edge.clone()], "1"),
            sketch("f-sk1", "XZ", rectangle(1, 10.0, 0.0, 20.0, 30.0)),
            json!({
                "id": "f-rev1",
                "type": "revolve",
                "body": "b-1",
                "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=15,15|a=300"}]},
                "axis": {"kind": "edge", "body": "b-2", "edge": axis_edge},
                "extent": {"kind": "full"},
                "op": "new"
            }),
        ],
    );

    run.assert_error("f-rev1", "reference_not_found");
    let candidates = candidate_names(&run, "f-rev1");
    for tangent in [
        "edge(f-box1/front|f-fil1/fillet/edge(f-box1/front|f-box1/left))",
        "edge(f-box1/left|f-fil1/fillet/edge(f-box1/front|f-box1/left))",
    ] {
        assert!(
            candidates.contains(&tangent.to_string()),
            "{tangent} is offered: {candidates:?}"
        );
    }
}
