//! Element descriptions (features.md section 3.12): one test per rule that section 6.5 test 1 does not already
//! check in `crates/partsmith-recompute/tests/feature_messages.rs`. Each test builds built-in records in timeline
//! order (`common::timeline`) and describes elements of the end state with `describe_elements`, the document's names
//! and the run's outputs, as messages do.

mod common;

use std::f64::consts::FRAC_PI_2;

use common::records::{
    arc, block, box_below, box_record, circle, cube20, distance, extrude, fillet, line, point,
    reference, shell, sketch, slot, xy,
};
use common::id;
use common::timeline::Timeline;
use partsmith_features::describe_elements;
use partsmith_model::Units;
use partsmith_naming::{ElementName, Fingerprint};
use serde_json::{Value, json};

/// An element name written in a test.
fn name(text: &str) -> ElementName {
    ElementName::parse(text).expect("element names written in tests are valid")
}

/// A fingerprint written in a test.
fn fingerprint(text: &str) -> Fingerprint {
    Fingerprint::parse(text).expect("fingerprints written in tests are valid")
}

/// `describe_elements` of `pairs` in one call, in the state of `body` at the end of `timeline`.
fn describe(timeline: &Timeline, body: &str, pairs: &[(ElementName, Fingerprint)]) -> Vec<String> {
    let state = timeline.state(body);
    let borrowed: Vec<(&ElementName, &Fingerprint)> =
        pairs.iter().map(|(name, print)| (name, print)).collect();
    timeline.with_names(|names| {
        describe_elements(&borrowed, &id(body), Some(&state), names, Units::Mm)
    })
}

/// The descriptions, in one call, of elements of `body` present at the end of `timeline`, each with its own
/// fingerprint.
fn describe_present(timeline: &Timeline, body: &str, names: &[&str]) -> Vec<String> {
    let state = timeline.state(body);
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
    describe(timeline, body, &pairs)
}

/// The description of one element of `body` present at the end of `timeline`.
fn describe_one(timeline: &Timeline, body: &str, element: &str) -> String {
    describe_present(timeline, body, &[element]).remove(0)
}

/// The description of a stored element, by its name and stored fingerprint, in the state of `body` at the end of
/// `timeline` (the element need not be there).
fn describe_stored(timeline: &Timeline, body: &str, element: &str, stored: &str) -> String {
    describe(timeline, body, &[(name(element), fingerprint(stored))]).remove(0)
}

/// The description of the element of `body` named `element`, present in the state of `body` just before `feature`,
/// in that state.
fn describe_before(timeline: &Timeline, feature: &str, body: &str, element: &str) -> String {
    let state = &timeline.before(feature)[&id(body)];
    let infos = state.elements().expect("element infos of a built body");
    let info = infos
        .iter()
        .find(|info| info.name == name(element))
        .unwrap_or_else(|| panic!("{body} has no element {element} before {feature}"));
    timeline.with_names(|names| {
        describe_elements(
            &[(&info.name, &info.fingerprint)],
            &id(body),
            Some(state),
            names,
            Units::Mm,
        )
        .remove(0)
    })
}

/// `SK-RING` of features/revolve.md (x 10..20, z 0..30 on XZ) and `f-rev1`, its revolve about the Z axis with
/// `extent`, op new, the region stored as a pick stores it.
fn ring_revolve(extent: Value) -> Timeline {
    Timeline::run(&["b-1"], &ring_revolve_records(extent))
}

/// The records of `ring_revolve`: `f-sk1` with `SK-RING` and `f-rev1`.
fn ring_revolve_records(extent: Value) -> Vec<Value> {
    let ring = vec![
        line("e-1", [10.0, 0.0], [20.0, 0.0]),
        line("e-2", [20.0, 0.0], [20.0, 30.0]),
        line("e-3", [20.0, 30.0], [10.0, 30.0]),
        line("e-4", [10.0, 30.0], [10.0, 0.0]),
    ];
    vec![
        sketch("f-sk1", json!({"kind": "origin", "plane": "XZ"}), ring),
        json!({
            "id": "f-rev1",
            "type": "revolve",
            "body": "b-1",
            "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=15,15|a=300", "outline": "e-1,e-2,e-3,e-4"}]},
            "axis": {"kind": "origin", "axis": "Z"},
            "extent": extent,
            "op": "new"
        }),
    ]
}

/// `ring_revolve_records` with `f-rev1` changed by `change`.
fn changed_revolve(extent: Value, change: impl FnOnce(&mut Value)) -> Vec<Value> {
    let mut records = ring_revolve_records(extent);
    change(&mut records[1]);
    records
}

/// A revolve of `angle`.
fn angle(expr: &str) -> Value {
    json!({"kind": "angle", "expr": expr})
}

/// A one-feature timeline in `b-1`.
fn single(record: Value) -> Timeline {
    Timeline::run(&["b-1"], &[record])
}

/// `f-cyl1`, a cylinder of radius 5 and height 10 (features/cylinder.md test 1).
fn cylinder() -> Value {
    json!({"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "5", "height": "10", "op": "new"})
}

/// `f-sph1`, a sphere of radius 5 centered at the origin.
fn sphere() -> Value {
    json!({"id": "f-sph1", "type": "sphere", "body": "b-1", "radius": "5", "op": "new"})
}

// Primitive faces (section 3.12, "Primitive faces").

#[test]
fn a_box_reads_its_vertical_faces_as_sides_and_its_caps_by_their_role() {
    let timeline = single(cube20());

    assert_eq!(
        describe_present(
            &timeline,
            "b-1",
            &[
                "f-box1/back",
                "f-box1/left",
                "f-box1/right",
                "f-box1/bottom"
            ]
        ),
        [
            "the back side of 'Box 1'",
            "the left side of 'Box 1'",
            "the right side of 'Box 1'",
            "the bottom of 'Box 1'",
        ]
    );
}

#[test]
fn a_sphere_reads_as_its_surface() {
    let timeline = single(sphere());

    assert_eq!(
        describe_one(&timeline, "b-1", "f-sph1/surface"),
        "the surface of 'Sphere 1'"
    );
}

// Extrude and revolve faces (section 3.12, "Extrude and revolve faces" and "Direction words").

#[test]
fn flat_faces_of_a_revolve_read_by_their_direction() {
    // A quarter turn: the start cap lies in the XZ plane facing -Y, the end cap in the YZ plane facing -X.
    let timeline = ring_revolve(angle("90 deg"));

    assert_eq!(
        describe_present(
            &timeline,
            "b-1",
            &[
                "f-rev1/lateral/e-1",
                "f-rev1/lateral/e-3",
                "f-rev1/start-cap",
                "f-rev1/end-cap",
            ]
        ),
        [
            "the bottom of 'Revolve 1'",
            "the top of 'Revolve 1'",
            "the front side of 'Revolve 1'",
            "the left side of 'Revolve 1'",
        ]
    );
}

#[test]
fn a_round_side_of_a_revolve_reads_by_the_line_it_was_made_from() {
    let timeline = ring_revolve(angle("90 deg"));

    assert_eq!(
        describe_one(&timeline, "b-1", "f-rev1/lateral/e-2"),
        "the side of 'Revolve 1' made from the line from (20, 0) to (20, 30) in 'Sketch 1'"
    );
}

#[test]
fn a_cap_that_faces_along_no_world_axis_reads_as_the_end_face() {
    // An eighth of a turn: the end cap faces (-0.707107, 0.707107, 0).
    let timeline = ring_revolve(angle("45 deg"));

    assert_eq!(
        describe_one(&timeline, "b-1", "f-rev1/end-cap"),
        "the end face of 'Revolve 1'"
    );
}

#[test]
fn a_cap_of_a_full_turn_takes_the_word_of_the_turning_direction() {
    // Corpus R57: the revolve now turns all the way round, so it makes no caps. The stored start cap reads by its own
    // fingerprint. As a face term of an edge it faces against the turning direction at the sketch plane, which is +Y
    // at x > 0 for a turn about +Z, so it faces front: the top ring's centroid lies on the axis, so the edge gives no
    // estimate of its normal.
    let timeline = ring_revolve(json!({"kind": "full"}));

    assert_eq!(
        describe_stored(
            &timeline,
            "b-1",
            "f-rev1/start-cap",
            "plane|n=0,-1,0|c=15,0,15|a=300|adj=4,4"
        ),
        "the front side of 'Revolve 1'"
    );
    assert_eq!(
        describe_stored(
            &timeline,
            "b-1",
            "edge(f-rev1/lateral/e-3|f-rev1/start-cap)",
            "line|n=1,0,0|c=15,0,30|l=10|adj=2,2"
        ),
        "the front top edge of 'Revolve 1'"
    );
}

/// The stored edge between the top of the ring's revolve and `cap`, which runs along a radius of the ring at the cap.
fn top_edge_of(cap: &str, fingerprint: &str) -> (ElementName, Fingerprint) {
    let mut faces = ["f-rev1/lateral/e-3".to_string(), format!("f-rev1/{cap}")];
    faces.sort();
    (
        name(&format!("edge({})", faces.join("|"))),
        self::fingerprint(fingerprint),
    )
}

#[test]
fn the_end_cap_of_a_suppressed_partial_turn_takes_the_word_of_the_turn() {
    // A quarter turn about +Z ends in the YZ plane at y > 0, where the profile turns toward -X. With the revolve
    // suppressed its top is not in the state either, so the edge reads along the top's line.
    let records = changed_revolve(angle("90 deg"), |revolve| {
        revolve["suppressed"] = json!(true);
    });
    let timeline = Timeline::run(&["b-1"], &records);

    assert_eq!(
        describe(
            &timeline,
            "b-1",
            &[top_edge_of("end-cap", "line|n=0,1,0|c=0,15,30|l=10|adj=2,2")]
        ),
        ["the left edge of 'Revolve 1' along the line from (20, 30) to (10, 30) in 'Sketch 1'"]
    );
}

#[test]
fn the_caps_of_a_reversed_partial_turn_take_the_words_of_the_reversed_turn() {
    // Reversed, the quarter turn goes from +X toward -Y: it starts turning toward -Y, so the start cap faces +Y, and
    // ends in the YZ plane at y < 0 turning toward -X.
    let records = changed_revolve(angle("90 deg"), |revolve| {
        revolve["suppressed"] = json!(true);
        revolve["reverse"] = json!(true);
    });
    let timeline = Timeline::run(&["b-1"], &records);

    assert_eq!(
        describe(
            &timeline,
            "b-1",
            &[
                top_edge_of("start-cap", "line|n=1,0,0|c=15,0,30|l=10|adj=2,2"),
                top_edge_of("end-cap", "line|n=0,1,0|c=0,-15,30|l=10|adj=2,2"),
            ]
        ),
        [
            "the back edge of 'Revolve 1' along the line from (20, 30) to (10, 30) in 'Sketch 1'",
            "the left edge of 'Revolve 1' along the line from (20, 30) to (10, 30) in 'Sketch 1'",
        ]
    );
}

#[test]
fn a_full_turn_about_a_sketch_line_turns_the_way_the_line_points() {
    // The construction line e-9 runs down at x = 5, so the turn is about -Z: the profile turns toward -Y at the
    // sketch plane and the start cap faces +Y.
    let records = changed_revolve(json!({"kind": "full"}), |revolve| {
        revolve["axis"] = json!({"kind": "sketch_line", "sketch": "f-sk1", "line": "e-9"});
    });
    let mut records = records;
    records[0]["entities"]
        .as_array_mut()
        .expect("sketch entities")
        .push(json!({"id": "e-9", "type": "line", "start": [5.0, 10.0], "end": [5.0, 0.0], "construction": true}));
    let timeline = Timeline::run(&["b-1"], &records);

    assert_eq!(
        describe(
            &timeline,
            "b-1",
            &[top_edge_of("start-cap", "line|n=1,0,0|c=15,0,30|l=10|adj=2,2")]
        ),
        ["the back top edge of 'Revolve 1'"]
    );
}

#[test]
fn a_full_turn_about_a_straight_edge_turns_about_the_stored_edge() {
    // The front left edge of a box in b-2 lies on the Z axis, so the turn is the one about origin Z.
    let mut records = vec![json!({"id": "f-box2", "type": "box", "body": "b-2", "size": ["10", "10", "30"], "position": ["5", "5", "0"], "op": "new"})];
    records.extend(changed_revolve(json!({"kind": "full"}), |revolve| {
        revolve["axis"] = json!({
            "kind": "edge",
            "body": "b-2",
            "edge": reference("edge(f-box2/front|f-box2/left)", "line|n=0,0,1|c=0,0,15|l=30|adj=2,2")
        });
    }));
    let timeline = Timeline::run(&["b-1", "b-2"], &records);

    assert_eq!(
        describe(
            &timeline,
            "b-1",
            &[top_edge_of("start-cap", "line|n=1,0,0|c=15,0,30|l=10|adj=2,2")]
        ),
        ["the front top edge of 'Revolve 1'"]
    );
}

/// `RECT(0, 0, 80, 50)` with its back right corner rounded by the arc `e-5` from (80, 40) to (70, 50), center
/// (70, 40), radius 10.
fn rounded_corner() -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 40.0]),
        arc("e-5", [70.0, 40.0], 10.0, 0.0, FRAC_PI_2),
        line("e-3", [70.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
    ]
}

/// The region of `rounded_corner` as a pick stores it.
fn rounded_corner_region() -> Value {
    json!({"key": "e-1,e-2,e-3,e-4,e-5", "fp": "c=39.796289,24.877199|a=3978.539816", "outline": "e-1,e-2,e-5,e-3,e-4"})
}

/// `f-sk1` with `entities` on XY and `f-ext1`, its extrude of `region`, distance 30, op new.
fn extruded(entities: Vec<Value>, region: Value) -> Timeline {
    Timeline::run(
        &["b-1"],
        &[
            sketch("f-sk1", xy(), entities),
            extrude("f-ext1", "f-sk1", vec![region], distance("30"), "new"),
        ],
    )
}

#[test]
fn a_side_made_from_an_arc_reads_by_the_arc() {
    let timeline = extruded(rounded_corner(), rounded_corner_region());

    assert_eq!(
        describe_one(&timeline, "b-1", "f-ext1/lateral/e-5"),
        "the side of 'Extrude 1' made from the arc from (80, 40) to (70, 50) in 'Sketch 1'"
    );
}

#[test]
fn a_side_made_from_a_circle_reads_by_the_circle() {
    let timeline = extruded(
        vec![circle("e-5", [40.0, 25.0], 5.0)],
        json!({"key": "e-5", "fp": "c=40,25|a=78.539816", "outline": "e-5"}),
    );

    assert_eq!(
        describe_one(&timeline, "b-1", "f-ext1/lateral/e-5"),
        "the side of 'Extrude 1' made from the circle at (40, 25) in 'Sketch 1'"
    );
}

#[test]
fn a_side_whose_line_the_sketch_no_longer_has_reads_as_made_from_a_line() {
    let timeline = Timeline::run(&["b-1"], &block());

    assert_eq!(
        describe_stored(
            &timeline,
            "b-1",
            "f-ext1/lateral/e-5",
            "plane|n=0.707107,0.707107,0|c=75,45,15|a=424.264069|adj=4,4"
        ),
        "the side of 'Extrude 1' made from a line of 'Sketch 1'"
    );
}

#[test]
fn a_side_whose_curve_the_sketch_no_longer_has_reads_as_made_from_a_curve() {
    let timeline = Timeline::run(&["b-1"], &block());

    assert_eq!(
        describe_stored(
            &timeline,
            "b-1",
            "f-ext1/lateral/e-5",
            "cylinder|n=0,0,1|c=76.366198,46.366198,15|a=471.238898|r=10|adj=4,4"
        ),
        "the side of 'Extrude 1' made from a curve of 'Sketch 1'"
    );
}

// Faces a cut leaves (section 3.12, "Faces a cut leaves").

#[test]
fn a_wall_of_a_cut_made_from_a_line_reads_along_that_line() {
    let mut features = block();
    features.extend(slot(30.0, 50.0));
    let timeline = Timeline::run(&["b-1"], &features);

    assert_eq!(
        describe_one(&timeline, "b-1", "f-ext2/lateral/e-22"),
        "the wall of the cut made by 'Extrude 2' along the line from (50, -1) to (50, 51)"
    );
}

/// `CUBE20` in b-1, `f-box2` a 10 mm cube at (0, 0, 15) in b-2, and `f-bool1` cutting b-1 with b-2: a pocket
/// x -5..5, y -5..5, z 15..20 whose faces are the tool's.
fn boolean_pocket() -> Timeline {
    Timeline::run(&["b-1", "b-2"], &boolean_pocket_records())
}

/// The records of `boolean_pocket`.
fn boolean_pocket_records() -> Vec<Value> {
    vec![
        cube20(),
        json!({"id": "f-box2", "type": "box", "body": "b-2", "size": ["10", "10", "10"], "position": ["0", "0", "15"], "op": "new"}),
        json!({"id": "f-bool1", "type": "boolean", "target": "b-1", "tools": ["b-2"], "op": "cut"}),
    ]
}

#[test]
fn the_tool_faces_of_a_cut_boolean_read_as_the_floor_and_walls_of_its_cut() {
    let timeline = boolean_pocket();

    assert_eq!(
        describe_present(&timeline, "b-1", &["f-box2/bottom", "f-box2/left"]),
        [
            "the floor of the cut made by 'Boolean 1'",
            "a wall of the cut made by 'Boolean 1'",
        ]
    );
}

#[test]
fn an_edge_between_a_cut_boolean_wall_and_the_body_reads_as_an_edge_of_its_cut() {
    let timeline = boolean_pocket();

    assert_eq!(
        describe_one(&timeline, "b-1", "edge(f-box1/top|f-box2/left)"),
        "the top edge of the cut made by 'Boolean 1'"
    );
}

#[test]
fn the_tool_faces_read_as_their_own_in_the_tool_body_before_the_cut() {
    let timeline = boolean_pocket();

    assert_eq!(
        describe_before(&timeline, "f-bool1", "b-2", "f-box2/bottom"),
        "the bottom of 'Box 2'"
    );
}

#[test]
fn a_kept_tool_reads_as_its_own_while_each_target_reads_as_its_cut() {
    // 'Boolean 1' keeps Box 2, so 'Boolean 2' cuts with it again, into a second CUBE20 in b-3.
    let mut records = boolean_pocket_records();
    records[2]["keep_tools"] = json!(true);
    records.push(json!({"id": "f-box3", "type": "box", "body": "b-3", "size": ["20", "20", "20"], "op": "new"}));
    records.push(json!({"id": "f-bool2", "type": "boolean", "target": "b-3", "tools": ["b-2"], "op": "cut"}));
    let timeline = Timeline::run(&["b-1", "b-2", "b-3"], &records);

    assert_eq!(
        describe_before(&timeline, "f-bool2", "b-2", "f-box2/bottom"),
        "the bottom of 'Box 2'"
    );
    assert_eq!(
        describe_one(&timeline, "b-1", "f-box2/bottom"),
        "the floor of the cut made by 'Boolean 1'"
    );
    assert_eq!(
        describe_one(&timeline, "b-3", "f-box2/bottom"),
        "the floor of the cut made by 'Boolean 2'"
    );
}

#[test]
fn a_cut_joined_into_another_body_still_reads_as_the_cut() {
    // 'Boolean 1' cuts a pocket into a box in b-3 at x 10..30, which 'Boolean 2' joins to CUBE20 in b-1.
    let timeline = Timeline::run(
        &["b-1", "b-2", "b-3"],
        &[
            cube20(),
            json!({"id": "f-box3", "type": "box", "body": "b-3", "size": ["20", "20", "20"], "position": ["20", "0", "0"], "op": "new"}),
            json!({"id": "f-box2", "type": "box", "body": "b-2", "size": ["10", "10", "10"], "position": ["20", "0", "15"], "op": "new"}),
            json!({"id": "f-bool1", "type": "boolean", "target": "b-3", "tools": ["b-2"], "op": "cut"}),
            json!({"id": "f-bool2", "type": "boolean", "target": "b-1", "tools": ["b-3"], "op": "join"}),
        ],
    );

    assert_eq!(
        describe_present(&timeline, "b-1", &["f-box2/bottom", "f-box2/right"]),
        [
            "the floor of the cut made by 'Boolean 1'",
            "a wall of the cut made by 'Boolean 1'",
        ]
    );
}

#[test]
fn a_mirror_image_of_a_cut_reads_as_the_cut_in_the_body_it_goes_to() {
    let mut records = boolean_pocket_records();
    records.push(json!({"id": "f-mir1", "type": "mirror", "body": "b-1", "plane": {"kind": "origin", "plane": "YZ"}, "op": "new", "target": "b-3"}));
    let timeline = Timeline::run(&["b-1", "b-2", "b-3"], &records);

    assert_eq!(
        describe_one(&timeline, "b-3", "f-box2/bottom@f-mir1:1"),
        "the floor of the cut made by 'Boolean 1' of the mirror image made by 'Mirror 1'"
    );
}

#[test]
fn a_suppressed_boolean_leaves_its_tools_to_the_next_boolean() {
    // 'Boolean 1' is suppressed, so 'Boolean 2' is the one that cuts with Box 2, into a second CUBE20 in b-3.
    let mut records = boolean_pocket_records();
    records[2]["suppressed"] = json!(true);
    records.push(json!({"id": "f-box3", "type": "box", "body": "b-3", "size": ["20", "20", "20"], "op": "new"}));
    records.push(json!({"id": "f-bool2", "type": "boolean", "target": "b-3", "tools": ["b-2"], "op": "cut"}));
    let timeline = Timeline::run(&["b-1", "b-2", "b-3"], &records);

    assert_eq!(
        describe_one(&timeline, "b-3", "f-box2/bottom"),
        "the floor of the cut made by 'Boolean 2'"
    );
}

// Fillet, chamfer, shell and hole faces (section 3.12, "Fillet, chamfer, shell, hole and split faces").

#[test]
fn a_face_a_fillet_makes_at_a_corner_reads_as_a_corner_face() {
    // The three edges that meet at the front left top corner of BLOCK, rounded together.
    let mut features = block();
    features.push(fillet(
        vec![
            reference(
                "edge(f-ext1/end-cap|f-ext1/lateral/e-1)",
                "line|n=1,0,0|c=40,0,30|l=80|adj=2,2",
            ),
            reference(
                "edge(f-ext1/end-cap|f-ext1/lateral/e-4)",
                "line|n=0,1,0|c=0,25,30|l=50|adj=2,2",
            ),
            reference(
                "edge(f-ext1/lateral/e-1|f-ext1/lateral/e-4)",
                "line|n=0,0,1|c=0,0,15|l=30|adj=2,2",
            ),
        ],
        "3",
    ));
    let timeline = Timeline::run(&["b-1"], &features);

    assert_eq!(
        describe_one(
            &timeline,
            "b-1",
            "f-fil1/corner/vertex(f-ext1/end-cap|f-ext1/lateral/e-1|f-ext1/lateral/e-4)"
        ),
        "a corner face of 'Fillet 1'"
    );
}

#[test]
fn a_chamfer_face_reads_as_the_beveled_face() {
    let mut features = block();
    features.push(json!({
        "id": "f-cha1",
        "type": "chamfer",
        "body": "b-1",
        "edges": [reference("edge(f-ext1/end-cap|f-ext1/lateral/e-1)", "line|n=1,0,0|c=40,0,30|l=80|adj=2,2")],
        "distance": {"kind": "equal", "expr": "2"}
    }));
    let timeline = Timeline::run(&["b-1"], &features);

    assert_eq!(
        describe_one(
            &timeline,
            "b-1",
            "f-cha1/chamfer/edge(f-ext1/end-cap|f-ext1/lateral/e-1)"
        ),
        "the beveled face of 'Chamfer 1'"
    );
}

#[test]
fn inner_faces_of_a_shell_read_by_the_face_they_line() {
    // BLOCK hollowed with its front side open.
    let mut features = block();
    features.push(shell(
        "2",
        vec![reference(
            "f-ext1/lateral/e-1",
            "plane|n=0,-1,0|c=40,0,15|a=2400|adj=4,4",
        )],
    ));
    let timeline = Timeline::run(&["b-1"], &features);

    assert_eq!(
        describe_present(
            &timeline,
            "b-1",
            &[
                "f-sh1/inner/f-ext1/end-cap",
                "f-sh1/inner/f-ext1/lateral/e-3",
                "f-sh1/inner/f-ext1/lateral/e-4",
                "f-sh1/inner/f-ext1/lateral/e-2",
            ]
        ),
        [
            "the inside ceiling of 'Shell 1'",
            "the inside back wall of 'Shell 1'",
            "the inside left wall of 'Shell 1'",
            "the inside right wall of 'Shell 1'",
        ]
    );
}

#[test]
fn an_inner_face_of_a_round_face_reads_as_an_inside_face() {
    let timeline = Timeline::run(
        &["b-1"],
        &[
            json!({"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "10", "height": "20", "op": "new"}),
            shell(
                "2",
                vec![reference(
                    "f-cyl1/top",
                    "plane|n=0,0,1|c=0,0,20|a=314.159265|adj=1,1",
                )],
            ),
        ],
    );

    assert_eq!(
        describe_one(&timeline, "b-1", "f-sh1/inner/f-cyl1/side"),
        "an inside face of 'Shell 1'"
    );
}

/// `BOX-BELOW` (top at z = 0), `f-sk1` with the point `e-p1` at (0, 0), and `f-hole1`, a hole of diameter 6 through
/// all at `e-p1` with the extra params `extra`.
fn drilled(extra: Value) -> Timeline {
    let mut hole = json!({
        "id": "f-hole1",
        "type": "hole",
        "body": "b-1",
        "sketch": "f-sk1",
        "points": ["e-p1"],
        "diameter": "6",
        "extent": {"kind": "through_all"}
    });
    if let (Some(hole), Some(extra)) = (hole.as_object_mut(), extra.as_object()) {
        hole.extend(extra.clone());
    }
    Timeline::run(
        &["b-1"],
        &[
            box_below(),
            sketch("f-sk1", xy(), vec![point("e-p1", [0.0, 0.0])]),
            hole,
        ],
    )
}

/// A countersink 11 mm across at 90 degrees.
fn countersink() -> Value {
    json!({"hole_type": {"kind": "countersink", "diameter": "11", "angle": "90 deg"}})
}

/// A blind hole 5 mm deep.
fn blind() -> Value {
    json!({"extent": {"kind": "distance", "expr": "5"}})
}

#[test]
fn a_counterbore_reads_as_the_counterbore_of_its_hole() {
    let timeline =
        drilled(json!({"hole_type": {"kind": "counterbore", "diameter": "11", "depth": "4"}}));

    assert_eq!(
        describe_one(&timeline, "b-1", "f-hole1/counterbore/e-p1"),
        "the counterbore of the hole at (0, 0)"
    );
}

#[test]
fn a_countersink_reads_as_the_countersink_of_its_hole() {
    let timeline = drilled(countersink());

    assert_eq!(
        describe_one(&timeline, "b-1", "f-hole1/countersink/e-p1"),
        "the countersink of the hole at (0, 0)"
    );
}

#[test]
fn the_rim_of_a_countersink_is_named_by_the_countersink() {
    let timeline = drilled(countersink());

    assert_eq!(
        describe_one(
            &timeline,
            "b-1",
            "edge(f-box1/top|f-hole1/countersink/e-p1)"
        ),
        "the top rim of the countersink of the hole at (0, 0)"
    );
}

#[test]
fn the_flat_end_of_a_blind_hole_reads_as_its_bottom() {
    let timeline = drilled(blind());

    assert_eq!(
        describe_one(&timeline, "b-1", "f-hole1/bottom/e-p1"),
        "the bottom of the hole at (0, 0)"
    );
}

#[test]
fn the_drilled_end_of_a_blind_hole_reads_as_its_tip() {
    let mut extra = blind();
    extra["tip_angle"] = json!("118 deg");
    let timeline = drilled(extra);

    assert_eq!(
        describe_one(&timeline, "b-1", "f-hole1/tip/e-p1"),
        "the tip of the hole at (0, 0)"
    );
}

#[test]
fn a_hole_face_whose_point_was_deleted_reads_by_its_stored_position() {
    // The sketch has no e-p2, so the hole's words take the stored centroid in world coordinates.
    let timeline = drilled(json!({}));

    assert_eq!(
        describe_stored(
            &timeline,
            "b-1",
            "f-hole1/bore/e-p2",
            "cylinder|n=0,0,1|c=7,0,-5|a=188.495559|r=3|adj=2,3"
        ),
        "the wall of a hole of 'Hole 1' near (7, 0, -5)"
    );
}

// Pieces of a split face (section 3.12, "Pieces").

#[test]
fn two_pieces_apart_along_y_read_as_the_front_and_back_parts() {
    // BLOCK with a groove across its top along X (y 20..30, 5 deep), which splits the top into a front and a back
    // piece.
    let mut features = block();
    features.push(json!({"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "30"}));
    features.push(sketch(
        "f-sk2",
        json!({"kind": "datum", "feature": "f-dp1"}),
        vec![
            line("e-21", [-1.0, 20.0], [81.0, 20.0]),
            line("e-22", [81.0, 20.0], [81.0, 30.0]),
            line("e-23", [81.0, 30.0], [-1.0, 30.0]),
            line("e-24", [-1.0, 30.0], [-1.0, 20.0]),
        ],
    ));
    let mut groove = extrude(
        "f-ext2",
        "f-sk2",
        vec![json!({"key": "e-21,e-22,e-23,e-24", "fp": "c=40,25|a=820"})],
        distance("5"),
        "cut",
    );
    groove["reverse"] = json!(true);
    features.push(groove);
    let timeline = Timeline::run(&["b-1"], &features);

    assert_eq!(
        describe_present(&timeline, "b-1", &["f-ext1/end-cap#1", "f-ext1/end-cap#2"]),
        [
            "the front part of the top of 'Extrude 1'",
            "the back part of the top of 'Extrude 1'",
        ]
    );
}

#[test]
fn two_pieces_apart_along_z_read_as_the_lower_and_upper_parts() {
    // CUBE20 with a groove 2 mm high across its right side (z 9..11), which splits that side in two.
    let timeline = Timeline::run(
        &["b-1"],
        &[
            cube20(),
            box_record("f-box2", ["4", "40", "2"], ["10", "0", "9"], "cut"),
        ],
    );

    assert_eq!(
        describe_present(&timeline, "b-1", &["f-box1/right#1", "f-box1/right#2"]),
        [
            "the lower part of the right side of 'Box 1'",
            "the upper part of the right side of 'Box 1'",
        ]
    );
}

/// BLOCK with two slots, x 20..30 and x 50..60, which split its top into three pieces: x 0..20, 30..50 and 60..80.
fn top_in_three_pieces() -> Timeline {
    let mut features = block();
    features.extend(slot(20.0, 30.0));
    let mut second = slot(50.0, 60.0);
    second[0]["id"] = json!("f-dp2");
    second[1]["id"] = json!("f-sk3");
    second[1]["plane"]["feature"] = json!("f-dp2");
    second[2]["id"] = json!("f-ext3");
    second[2]["profile"]["sketch"] = json!("f-sk3");
    features.extend(second);
    Timeline::run(&["b-1"], &features)
}

#[test]
fn a_piece_of_a_face_in_three_pieces_reads_as_a_part_of_it() {
    let timeline = top_in_three_pieces();

    assert_eq!(
        describe_one(&timeline, "b-1", "f-ext1/end-cap#1"),
        "a part of the top of 'Extrude 1'"
    );
}

#[test]
fn elements_that_read_alike_without_an_entity_clause_get_their_position() {
    // Section 3.12, "Fallback and clashes": the cap has no sketch entity, so each piece gets its position clause.
    let timeline = top_in_three_pieces();

    assert_eq!(
        describe_present(
            &timeline,
            "b-1",
            &[
                "f-ext1/end-cap#1",
                "f-ext1/end-cap#2#1",
                "f-ext1/end-cap#2#2"
            ]
        ),
        [
            "a part of the top of 'Extrude 1' near (10, 25, 30)",
            "a part of the top of 'Extrude 1' near (40, 25, 30)",
            "a part of the top of 'Extrude 1' near (70, 25, 30)",
        ]
    );
}

// Copies (section 3.12, "Copies").

#[test]
fn a_copy_of_a_circular_pattern_reads_by_its_ordinal() {
    let timeline = Timeline::run(
        &["b-1"],
        &[
            box_record("f-box1", ["4", "4", "4"], ["20", "0", "0"], "new"),
            json!({"id": "f-cp1", "type": "circular_pattern", "body": "b-1", "axis": {"kind": "origin", "axis": "Z"}, "count": "4"}),
        ],
    );

    assert_eq!(
        describe_one(&timeline, "b-1", "f-box1/top@f-cp1:1"),
        "the top of 'Box 1' of the second copy of 'Circular pattern 1'"
    );
}

// Edges (section 3.12, "Edges").

#[test]
fn the_rim_of_a_boss_belongs_to_the_boss() {
    // Rule (2): a circle edge between the block's top and the round side a join made.
    let mut features = block();
    features.push(json!({"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "30"}));
    features.push(sketch(
        "f-sk2",
        json!({"kind": "datum", "feature": "f-dp1"}),
        vec![circle("e-5", [40.0, 25.0], 5.0)],
    ));
    features.push(extrude(
        "f-ext2",
        "f-sk2",
        vec![json!({"key": "e-5", "fp": "c=40,25|a=78.539816"})],
        distance("10"),
        "join",
    ));
    let timeline = Timeline::run(&["b-1"], &features);

    assert_eq!(
        describe_one(&timeline, "b-1", "edge(f-ext1/end-cap|f-ext2/lateral/e-5)"),
        "the top rim of 'Extrude 2'"
    );
}

#[test]
fn an_edge_between_a_worded_face_and_a_face_made_from_an_arc_runs_along_the_arc() {
    // Rule (5) with an arc: the right side ends where the rounded corner starts.
    let timeline = extruded(rounded_corner(), rounded_corner_region());

    assert_eq!(
        describe_one(
            &timeline,
            "b-1",
            "edge(f-ext1/lateral/e-2|f-ext1/lateral/e-5)"
        ),
        "the right edge of 'Extrude 1' along the arc from (80, 40) to (70, 50) in 'Sketch 1'"
    );
}

#[test]
fn an_edge_between_faces_of_one_feature_without_two_direction_words_names_both_faces() {
    // Rule (6): the end cap of an eighth of a turn has no direction word.
    let timeline = ring_revolve(angle("45 deg"));

    assert_eq!(
        describe_one(&timeline, "b-1", "edge(f-rev1/end-cap|f-rev1/lateral/e-3)"),
        "the edge between the end face of 'Revolve 1' and the top of 'Revolve 1'"
    );
}

#[test]
fn a_seam_reads_by_its_position() {
    // A seam has one face, so no edge rule applies (section 3.12, "Fallback and clashes").
    let timeline = single(cylinder());

    assert_eq!(
        describe_one(&timeline, "b-1", "edge(f-cyl1/side)"),
        "an edge of 'Cylinder 1' near (5, 0, 5)"
    );
}

// Vertices (section 3.12, "Vertices").

#[test]
fn a_corner_of_faces_of_two_features_names_the_faces_that_meet() {
    // CUBE20 joined with a 10 x 10 x 30 box at x 10..20, y -5..5: the bottoms merge into f-box1/bottom.
    let timeline = Timeline::run(
        &["b-1"],
        &[
            cube20(),
            box_record("f-box2", ["10", "10", "30"], ["15", "0", "0"], "join"),
        ],
    );

    assert_eq!(
        describe_one(
            &timeline,
            "b-1",
            "vertex(f-box1/bottom|f-box2/front|f-box2/right)"
        ),
        "the corner where the bottom of 'Box 1', the front side of 'Box 2' and the right side of 'Box 2' meet"
    );
}

#[test]
fn a_corner_where_two_faces_meet_names_both() {
    let timeline = single(cylinder());

    assert_eq!(
        describe_one(&timeline, "b-1", "vertex(f-cyl1/side|f-cyl1/top)"),
        "the corner where the side of 'Cylinder 1' and the top of 'Cylinder 1' meet"
    );
}

#[test]
fn a_corner_on_one_face_reads_by_its_position() {
    // The north pole of a sphere lies on its surface alone.
    let timeline = single(sphere());

    assert_eq!(
        describe_one(&timeline, "b-1", "vertex(f-sph1/surface)#2"),
        "a corner of 'Sphere 1' near (0, 0, 5)"
    );
}

// Missing features (section 3.12, "Missing features").

#[test]
fn an_edge_with_a_face_of_a_feature_not_in_the_document_says_so() {
    let timeline = single(cube20());

    assert_eq!(
        describe_stored(
            &timeline,
            "b-1",
            "edge(f-box1/top|f-gone/x)",
            "line|n=1,0,0|c=0,-10,20|l=20"
        ),
        "an edge of a feature that is not in this document"
    );
}

#[test]
fn a_corner_with_a_face_of_a_feature_not_in_the_document_says_so() {
    let timeline = single(cube20());

    assert_eq!(
        describe_stored(
            &timeline,
            "b-1",
            "vertex(f-box1/front|f-box1/top|f-gone/x)",
            "point|c=0,-10,20"
        ),
        "a corner of a feature that is not in this document"
    );
}
