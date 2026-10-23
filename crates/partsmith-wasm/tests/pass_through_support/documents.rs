//! The documents the pass-through tests open, the records they are built from, and the edits the tests apply.

use partsmith_model::{load, save};
use serde_json::{Value, json};

// Records, recipes and model edits.

/// A millimeter `.ptsm` text at format version 2 of the one body `b-1`, 'Body 1', with `parameters` and `features`.
fn body_1_text(parameters: Vec<Value>, features: Vec<Value>) -> String {
    let document = json!({
        "format": "partsmith",
        "format_version": 2,
        "units": "mm",
        "parameters": parameters,
        "bodies": [{"id": "b-1", "name": "Body 1"}],
        "features": features
    });
    serde_json::to_string_pretty(&document).expect("documents serialize")
}

/// The box `feature` of `b-1` of `size`, at `position` when there is one, with Operation `op`.
fn box_feature(feature: &str, size: [&str; 3], position: Option<[&str; 3]>, op: &str) -> Value {
    let mut record = json!({"id": feature, "type": "box", "body": "b-1", "size": size});
    if let Some(position) = position {
        record["position"] = json!(position);
    }
    record["op"] = json!(op);
    record
}

/// A line entity from `start` to `end`.
pub fn line(entity: &str, start: [f64; 2], end: [f64; 2]) -> Value {
    json!({"id": entity, "type": "line", "start": start, "end": end})
}

/// The lines `e-1` to `e-4` of the rectangle from the origin to (`width`, `depth`), counterclockwise from the origin, as features.md section 6.1 `RECT` draws them.
fn rectangle(width: f64, depth: f64) -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [width, 0.0]),
        line("e-2", [width, 0.0], [width, depth]),
        line("e-3", [width, depth], [0.0, depth]),
        line("e-4", [0.0, depth], [0.0, 0.0]),
    ]
}

/// A circle entity at `center` with `radius`.
pub fn circle(entity: &str, center: [f64; 2], radius: f64) -> Value {
    json!({"id": entity, "type": "circle", "center": center, "radius": radius})
}

/// The origin plane XY, as a sketch or a datum plane stores it.
fn xy_plane() -> Value {
    json!({"kind": "origin", "plane": "XY"})
}

/// The world X direction.
pub const X_AXIS: [f64; 3] = [1.0, 0.0, 0.0];

/// The world Z direction.
pub const Z_AXIS: [f64; 3] = [0.0, 0.0, 1.0];

/// The plane of a sketch on the face `face` of `b-1` with its fallback at `origin`, x axis (1, 0, 0) and normal (0, 0, 1) in `frame` (`None` writes no `fallback_frame`: world coordinates).
fn face_plane(face: Value, origin: [f64; 3], frame: Option<&str>) -> Value {
    let mut plane = json!({
        "kind": "face", "body": "b-1", "face": face,
        "fallback": {"origin": origin, "x_axis": X_AXIS, "normal": Z_AXIS}
    });
    if let Some(frame) = frame {
        plane["fallback_frame"] = json!(frame);
    }
    plane
}

/// The sketch `feature` on `plane` with `entities` and `constraints`.
pub fn sketch(feature: &str, plane: Value, entities: Vec<Value>, constraints: Vec<Value>) -> Value {
    json!({
        "id": feature, "type": "sketch", "plane": plane,
        "entities": entities, "constraints": constraints
    })
}

/// The extrude `feature` into `b-1` of the profile regions `regions` of `sketch`, by `distance`, with Operation `op`.
pub fn extrude(feature: &str, sketch: &str, regions: Value, distance: &str, op: &str) -> Value {
    json!({
        "id": feature, "type": "extrude", "body": "b-1",
        "profile": {"sketch": sketch, "regions": regions},
        "extent": {"kind": "distance", "expr": distance}, "op": op
    })
}

/// The fillet `f-fil1` of `b-1` of radius `radius` on the one edge `edge`.
pub fn fillet(edge: Value, radius: &str) -> Value {
    json!({"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [edge], "radius": radius})
}

/// The feature records of the recipe `document`, in timeline order.
fn feature_records(document: &Value) -> &[Value] {
    document["features"]
        .as_array()
        .expect("documents list their features")
}

/// The record of `feature` in the recipe `document`.
pub fn feature_record(document: &Value, feature: &str) -> Value {
    feature_records(document)
        .iter()
        .find(|record| record["id"] == feature)
        .unwrap_or_else(|| panic!("the document has {feature}"))
        .clone()
}

/// The feature records of the document `text`, in timeline order.
fn features_of(text: &str) -> Vec<Value> {
    let document: Value = serde_json::from_str(text).expect("documents are JSON");
    feature_records(&document).to_vec()
}

/// The `update_feature` edit that stores `record` in place of the record with its id.
fn update_feature(record: Value) -> Value {
    json!({"type": "update_feature", "feature": record})
}

/// The `add_feature` edit that inserts `record` at timeline position `index`.
fn add_feature(index: usize, record: Value) -> Value {
    json!({"type": "add_feature", "index": index, "feature": record})
}

/// The `remove_feature` edit of `feature`.
pub fn remove_feature(feature: &str) -> Value {
    json!({"type": "remove_feature", "id": feature})
}

// Diagnostics, fixes, errors and format version 2 (the reference stability plan, section 10.3 item 9).

/// File-format.md example E10: every field format version 2 adds (`nb`, `merged`, `outline`, `fallback_frame` and `direction`), exactly as `save` writes it.
pub const E10_TEXT: &str =
    include_str!("../../../partsmith-model/tests/fixtures/e10_version_2.ptsm");

/// The line of canonical text at format version 2, E10 among them, that holds its format version.
const VERSION_2_LINE: &str = "  \"format_version\": 2,";

/// The canonical text `text`, at format version 2, with its format version line set to `version`.
fn at_version(text: &str, version: u32) -> String {
    assert_eq!(
        text.matches(VERSION_2_LINE).count(),
        1,
        "the text states its format version once"
    );
    text.replace(VERSION_2_LINE, &format!("  \"format_version\": {version},"))
}

/// The canonical text `save` writes for the document `text`.
pub fn canonical(text: &str) -> String {
    let loaded = load(text).unwrap_or_else(|error| panic!("the document loads: {error}"));
    save(&loaded.document).unwrap_or_else(|error| panic!("the document saves: {error}"))
}

/// The canonical text of the document `text` as a file written at format version 1 holds it: only its `format_version` line differs.
pub fn at_version_1(text: &str) -> String {
    at_version(&canonical(text), 1)
}

/// E10 with its format version line set to `version`.
pub fn e10_at_version(version: u32) -> String {
    at_version(E10_TEXT, version)
}

/// CUBE10 with a linear pattern of 2.5 copies along X (features/linear_pattern.md test 6): `f-lp1` fails with `not_whole_number` and offers `Use 2` and `Use 3` (engine.md test 126).
pub const FRACTIONAL_PATTERN_TEXT: &str = r#"{
  "format": "partsmith",
  "format_version": 2,
  "units": "mm",
  "parameters": [],
  "bodies": [
    {"id": "b-1", "name": "Body 1"}
  ],
  "features": [
    {"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"},
    {
      "id": "f-lp1",
      "type": "linear_pattern",
      "body": "b-1",
      "direction": {"axis": {"kind": "origin", "axis": "X"}, "count": "2.5", "spacing": "20"}
    }
  ]
}
"#;

/// The `fix` reference of `Use 3`, the second action of the fractional pattern's message, offered at revision 0.
pub fn use_three() -> Value {
    json!({
        "feature": "f-lp1", "code": "not_whole_number", "field": "/direction/count",
        "cause": false, "index": 1, "revision": 0
    })
}

/// Corpus R16 after its edit (engine.md test 115): CUBE20 with a 5 x 5 rabbet ('Sketch 1' and 'Extrude 1') inserted before the fillet of its front top edge, so `f-fil1` fails with `references_not_found` and an element repair whose candidates are the rabbet's edges.
pub const CUT_AWAY_EDGE_TEXT: &str = r#"{
  "format": "partsmith",
  "format_version": 2,
  "units": "mm",
  "parameters": [],
  "bodies": [
    {"id": "b-1", "name": "Body 1"}
  ],
  "features": [
    {"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"},
    {
      "id": "f-sk2",
      "type": "sketch",
      "plane": {"kind": "origin", "plane": "YZ"},
      "entities": [
        {"id": "e-1", "type": "line", "start": [-11, 15], "end": [-5, 15]},
        {"id": "e-2", "type": "line", "start": [-5, 15], "end": [-5, 21]},
        {"id": "e-3", "type": "line", "start": [-5, 21], "end": [-11, 21]},
        {"id": "e-4", "type": "line", "start": [-11, 21], "end": [-11, 15]}
      ],
      "constraints": []
    },
    {
      "id": "f-ext2",
      "type": "extrude",
      "body": "b-1",
      "profile": {"sketch": "f-sk2", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=-8,18|a=36"}]},
      "extent": {"kind": "symmetric", "expr": "30"},
      "op": "cut"
    },
    {
      "id": "f-fil1",
      "type": "fillet",
      "body": "b-1",
      "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2"}],
      "radius": "2"
    }
  ]
}
"#;

// Frames on the model (ADR 0023; the reference stability plan, section 10.4).

/// `box.ptsm` of the engine tests: the 20 x 30 x 40 box `f-box1` of body `b-1`, centered on the Z axis from z = 0 to 40.
const BOX_TEXT: &str = include_str!("../../../partsmith-engine/tests/fixtures/box.ptsm");

/// The top of the box of `box.ptsm`.
pub const BOX_TOP: &str = "f-box1/top";

/// The front top edge of the box of `box.ptsm`, which runs along X before anything moves the box.
pub const BOX_FRONT_TOP_EDGE: &str = "edge(f-box1/front|f-box1/top)";

/// `box.ptsm` with `more` features after `f-box1`.
pub fn box_then(more: Vec<Value>) -> String {
    let mut document: Value = serde_json::from_str(BOX_TEXT).expect("box.ptsm is JSON");
    document["features"]
        .as_array_mut()
        .expect("documents list their features")
        .extend(more);
    serde_json::to_string_pretty(&document).expect("documents serialize")
}

/// The Move body `f-mv1` of `b-1` by (100, 0, 0), the move of engine.md tests 124, 125 and 135.
fn moved_by_100() -> Value {
    json!({"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["100", "0", "0"]})
}

/// `box.ptsm` moved by (100, 0, 0), the document of engine.md test 124.
pub fn moved_box() -> String {
    box_then(vec![moved_by_100()])
}

/// The reference of the top of `box.ptsm` moved by (100, 0, 0), as a pick stores it (engine.md test 125).
fn moved_top() -> Value {
    json!({"q": BOX_TOP, "fp": "plane|n=0,0,1|c=100,0,40|a=600|adj=4,4"})
}

/// The document of engine.md test 125: `box.ptsm` moved by (100, 0, 0), then a face sketch `f-sk1` on its top with one line `e-1` from (0, 0) to (10, 0) and its fallback at (0, 0, 40) in `frame` (see [`face_plane`]).
pub fn sketch_on_moved_top(frame: Option<&str>) -> String {
    box_then(vec![
        moved_by_100(),
        json!({
            "id": "f-sk1", "type": "sketch",
            "plane": face_plane(moved_top(), [0.0, 0.0, 40.0], frame),
            "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [10, 0]}],
            "constraints": []
        }),
    ])
}

/// features/sketch.md test 5 after a Move body (engine.md test 135): CUBE10 moved by (100, 0, 0), then a sketch `f-sk1` with the point `e-p1` on a face the cube does not have, its fallback at (3, 4, 0) in body coordinates, so it stands at its saved position, world (103, 4, 0), and offers to attach to the cube's faces.
pub const LOST_FACE_ON_A_MOVED_CUBE_TEXT: &str = r#"{
  "format": "partsmith",
  "format_version": 2,
  "units": "mm",
  "parameters": [],
  "bodies": [
    {"id": "b-1", "name": "Body 1"}
  ],
  "features": [
    {"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"},
    {"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["100", "0", "0"]},
    {
      "id": "f-sk1",
      "type": "sketch",
      "plane": {
        "kind": "face",
        "body": "b-1",
        "face": {"q": "f-box1/nothing", "fp": "plane|n=0,0,1|c=500,500,500|a=1"},
        "fallback": {"origin": [3, 4, 0], "x_axis": [1, 0, 0], "normal": [0, 0, 1]},
        "fallback_frame": "body"
      },
      "entities": [
        {"id": "e-p1", "type": "point", "position": [0, 0]}
      ],
      "constraints": []
    }
  ]
}
"#;

/// Corpus R32 (b) after its edit (features/sketch.md test 24, engine.md tests 126 and 135): BLOCK with 'Extrude 1' reversed, so its end cap is the bottom at z = -30, facing -Z, and a face sketch `f-sk2` on that cap with its world fallback at (0, 0, 30) and the point `e-p1` at (20, 10), where the hole `f-hole1` of diameter 5 and depth 10 misses the body because the drawing lies mirrored on the turned-over face.
pub const TURNED_OVER_TOP_TEXT: &str = r#"{
  "format": "partsmith",
  "format_version": 2,
  "units": "mm",
  "parameters": [],
  "bodies": [
    {"id": "b-1", "name": "Body 1"}
  ],
  "features": [
    {
      "id": "f-sk1",
      "type": "sketch",
      "plane": {"kind": "origin", "plane": "XY"},
      "entities": [
        {"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]},
        {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]},
        {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]},
        {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}
      ],
      "constraints": []
    },
    {
      "id": "f-ext1",
      "type": "extrude",
      "body": "b-1",
      "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]},
      "extent": {"kind": "distance", "expr": "30"},
      "reverse": true,
      "op": "new"
    },
    {
      "id": "f-sk2",
      "type": "sketch",
      "plane": {
        "kind": "face",
        "body": "b-1",
        "face": {"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"},
        "fallback": {"origin": [0, 0, 30], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}
      },
      "entities": [
        {"id": "e-p1", "type": "point", "position": [20, 10]}
      ],
      "constraints": []
    },
    {
      "id": "f-hole1",
      "type": "hole",
      "body": "b-1",
      "sketch": "f-sk2",
      "points": ["e-p1"],
      "diameter": "5",
      "extent": {"kind": "distance", "expr": "10"}
    }
  ]
}
"#;

/// The timeline of file-format.md test 57, corpus R32 (a) after its edit, at format version 2: a box 40 x 40 x 10, the Move body of (100, 0, 0), a face sketch `f-sk2` on the box's top with its fallback at (0, 0, 10) in `frame` (see [`face_plane`]) and its point `e-p1` at (15, 15), and a hole `f-hole1` of diameter 4 through all at the point.
pub fn moved_box_with_a_face_sketch(frame: Option<&str>) -> String {
    let top = json!({"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=1600|adj=4,4"});
    let plane = face_plane(top, [0.0, 0.0, 10.0], frame);
    body_1_text(
        Vec::new(),
        vec![
            box_feature("f-box1", ["40", "40", "10"], None, "new"),
            moved_by_100(),
            json!({
                "id": "f-sk2", "type": "sketch", "plane": plane,
                "entities": [{"id": "e-p1", "type": "point", "position": [15, 15]}],
                "constraints": []
            }),
            json!({
                "id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk2",
                "points": ["e-p1"], "diameter": "4", "extent": {"kind": "through_all"}
            }),
        ],
    )
}

// Newcomer errors and document units (the reference stability plan, section 10.5).

/// BLOCK of the newcomer error corpus with 'Extrude 1' extruded `distance`: the 80 x 50 rectangle `e-1` to `e-4` of `f-sk1` on XY, extruded by `f-ext1` into `b-1` (x 0..80, y 0..50, z 0..30 at distance 30).
pub fn block(distance: &str) -> Vec<Value> {
    vec![
        sketch("f-sk1", xy_plane(), rectangle(80.0, 50.0), Vec::new()),
        extrude(
            "f-ext1",
            "f-sk1",
            json!([{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]),
            distance,
            "new",
        ),
    ]
}

/// Newcomer error NC01: BLOCK's outline in `f-sk1` with a 0.3 mm gap at the origin (`e-4` ends at (0, 0.3)), which no feature uses yet.
pub fn open_outline() -> String {
    let mut outline = rectangle(80.0, 50.0);
    outline[3] = line("e-4", [0.0, 50.0], [0.0, 0.3]);
    body_1_text(
        Vec::new(),
        vec![sketch("f-sk1", xy_plane(), outline, Vec::new())],
    )
}

/// NC03 (`op` cut) and NC04 (`op` join): BLOCK, the datum plane `f-dp1` 40 above XY, and the circle `e-5` of `f-sk2` on it at (40, 25) with radius 5, extruded 15 by `f-ext2` with Operation `op`: it starts 10 above the block and points away from it.
pub fn circle_above_the_block(op: &str) -> String {
    let mut features = block("30");
    features.extend([
        json!({"id": "f-dp1", "type": "datum_plane", "base": xy_plane(), "offset": "40"}),
        sketch(
            "f-sk2",
            json!({"kind": "datum", "feature": "f-dp1"}),
            vec![json!({"id": "e-5", "type": "circle", "center": [40, 25], "radius": 5})],
            Vec::new(),
        ),
        extrude(
            "f-ext2",
            "f-sk2",
            json!([{"key": "e-5", "fp": "c=40,25|a=78.539816"}]),
            "15",
            op,
        ),
    ]);
    body_1_text(Vec::new(), features)
}

/// Newcomer error NC05: 'Box 1' 40 x 20 x 10 (x -20..20), 'Box 2' 20 x 30 x 5 at (10, 0, 7.6) cutting a step 2.4 mm deep whose riser is `f-box2/left` at x = 0, and a fillet of radius 3 on the edge between the top of 'Box 1' and the riser.
pub fn fillet_on_a_low_step() -> String {
    body_1_text(
        Vec::new(),
        vec![
            box_feature("f-box1", ["40", "20", "10"], None, "new"),
            box_feature("f-box2", ["20", "30", "5"], Some(["10", "0", "7.6"]), "cut"),
            fillet(
                json!({"q": "edge(f-box1/top|f-box2/left)", "fp": "line|n=0,1,0|c=0,0,10|l=20|adj=2,2"}),
                "3",
            ),
        ],
    )
}

/// Newcomer error NC07 after its edit: the 40 x 20 rectangle of `f-sk1` with Distance `c-1` 40 mm and the added Distance `c-2` 30 mm on `e-1`, extruded 10 by `f-ext1`. Its messages and actions are those of the setup with the edit applied (the newcomer error corpus, rule 5).
pub fn conflicting_dimensions() -> String {
    let distance = |constraint: &str, expr: &str| json!({"id": constraint, "type": "distance", "line": "e-1", "expr": expr});
    body_1_text(
        Vec::new(),
        vec![
            sketch(
                "f-sk1",
                xy_plane(),
                rectangle(40.0, 20.0),
                vec![distance("c-1", "40 mm"), distance("c-2", "30 mm")],
            ),
            extrude(
                "f-ext1",
                "f-sk1",
                json!([{"key": "e-1,e-2,e-3,e-4", "fp": "c=20,10|a=800"}]),
                "10",
                "new",
            ),
        ],
    )
}

/// Newcomer error NC09: 'Box 1' of size `40`, `20` and `10` and a fillet of radius `2` on its front top edge, four numbers without a unit in a millimeter document.
pub fn numbers_without_units() -> String {
    body_1_text(
        Vec::new(),
        vec![
            box_feature("f-box1", ["40", "20", "10"], None, "new"),
            fillet(
                json!({"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,10|l=40|adj=2,2"}),
                "2",
            ),
        ],
    )
}

/// CUBE20 with its sizes in millimeters and a fillet of radius `2`, without a unit, on its front top edge: in inches the radius is 50.8 mm, too large (NC09, with the sentence of corpus R48).
pub fn cube_with_a_bare_radius() -> String {
    body_1_text(
        Vec::new(),
        vec![
            box_feature("f-box1", ["20 mm", "20 mm", "20 mm"], None, "new"),
            fillet(
                json!({"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2"}),
                "2",
            ),
        ],
    )
}

/// Newcomer error NC10: BLOCK with the parameter `height` 30 mm and the distance of 'Extrude 1' misspelled `hieght`.
pub fn misspelled_parameter() -> String {
    body_1_text(
        vec![json!({"id": "p-height", "name": "height", "expr": "30 mm"})],
        block("hieght"),
    )
}

/// CUBE20 with a shell `f-sh1` 2 thick whose params store the top under the misspelled key `remove_face` instead of `remove_faces` (engine.md test 131).
pub fn misspelled_shell_setting() -> String {
    body_1_text(
        Vec::new(),
        vec![
            box_feature("f-box1", ["20", "20", "20"], None, "new"),
            json!({
                "id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "2",
                "remove_face": [{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,20|a=400|adj=4,4"}]
            }),
        ],
    )
}

// Causes, ghosts and uses (the reference stability plan, section 10.6).

/// A corpus scenario as engine.md test 136 sends it: a setup and the edit applied to it.
pub struct Scenario {
    /// The setup's text, opened as written, without picks.
    pub setup: String,
    /// The model edits of the scenario's edit, applied as one `apply_edits` labeled `Edit`.
    pub edits: Value,
}

/// BLOCK's top, as a face sketch stores it.
fn block_top() -> Value {
    json!({"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"})
}

/// BLOCK's back top edge, along the line `e-3` from (80, 50) to (0, 50).
fn block_back_top() -> Value {
    json!({"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-3)", "fp": "line|n=1,0,0|c=40,50,30|l=80|adj=2,2"})
}

/// The profile regions of the one region inside the circle `e-5` at (40, 25) with radius 10, as an extrude stores them.
fn round_region() -> Value {
    json!([{"key": "e-5", "fp": "c=40,25|a=314.159265"}])
}

/// Corpus R12's round pocket `f-ext2`: the circle `e-5` of `f-sk2` at (40, 25) with radius 10, cut `depth` down from BLOCK's top.
fn round_pocket(depth: &str) -> Value {
    let mut pocket = extrude("f-ext2", "f-sk2", round_region(), depth, "cut");
    pocket["reverse"] = json!(true);
    pocket
}

/// Corpus R12: BLOCK, the datum plane `f-dp1` at its top, the circle `e-5` of `f-sk2` on it, the pocket `f-ext2` 5 deep, and a fillet `f-fil1` of 2 on the pocket's floor edge; the edit makes the pocket 30 deep, through the block, so its floor and the fillet's edge are gone.
pub fn r12() -> Scenario {
    let mut features = block("30");
    features.extend([
        json!({"id": "f-dp1", "type": "datum_plane", "base": xy_plane(), "offset": "30"}),
        sketch(
            "f-sk2",
            json!({"kind": "datum", "feature": "f-dp1"}),
            vec![circle("e-5", [40.0, 25.0], 10.0)],
            Vec::new(),
        ),
        round_pocket("5"),
        fillet(
            json!({"q": "edge(f-ext2/end-cap|f-ext2/lateral/e-5)", "fp": "circle|n=0,0,1|c=40,25,25|l=62.831853|r=10|adj=2,1"}),
            "2",
        ),
    ]);
    Scenario {
        setup: body_1_text(Vec::new(), features),
        edits: json!([update_feature(round_pocket("30"))]),
    }
}

/// The ids of the rabbet ('Sketch 1' and 'Extrude 1') that corpus R16's edit inserts before the fillet.
const RABBET: [&str; 2] = ["f-sk2", "f-ext2"];

/// Whether `record` is one of the [`RABBET`] records.
fn is_rabbet(record: &Value) -> bool {
    RABBET.iter().any(|feature| record["id"] == *feature)
}

/// Corpus R16: CUBE20 with a fillet of 2 on its front top edge, [`CUT_AWAY_EDGE_TEXT`] without the rabbet; the edit inserts the rabbet before the fillet, at timeline positions 1 and 2.
pub fn r16() -> Scenario {
    let (rabbet, cube): (Vec<Value>, Vec<Value>) = features_of(CUT_AWAY_EDGE_TEXT)
        .into_iter()
        .partition(is_rabbet);
    let inserted = rabbet
        .into_iter()
        .enumerate()
        .map(|(offset, record)| add_feature(1 + offset, record))
        .collect();
    Scenario {
        setup: body_1_text(Vec::new(), cube),
        edits: Value::Array(inserted),
    }
}

/// Corpus R26: BLOCK, a face sketch `f-sk2` on its top with its fallback at (0, 0, 30) in body coordinates and the circle `e-5` at (40, 25) with radius 10, and the boss `f-ext2` joined 10 tall from it; edit (1) makes 'Extrude 1' 80 tall.
pub fn r26() -> Scenario {
    let mut features = block("30");
    features.extend([
        sketch(
            "f-sk2",
            face_plane(block_top(), [0.0, 0.0, 30.0], Some("body")),
            vec![circle("e-5", [40.0, 25.0], 10.0)],
            Vec::new(),
        ),
        extrude("f-ext2", "f-sk2", round_region(), "10", "join"),
    ]);
    Scenario {
        setup: body_1_text(Vec::new(), features),
        edits: json!([update_feature(block("80").remove(1))]),
    }
}

/// BLOCK with a fillet `f-fil1` of 2 on its back top edge, then `more` features (newcomer error NC11 before its edit, and engine.md test 143).
pub fn block_with_a_back_top_fillet(more: Vec<Value>) -> String {
    let mut features = block("30");
    features.push(fillet(block_back_top(), "2"));
    features.extend(more);
    body_1_text(Vec::new(), features)
}

/// Newcomer error NC11: BLOCK, a fillet of 2 on the back top edge, then the back line `e-3` deleted, which leaves the outline open.
pub fn nc11() -> Scenario {
    let mut outline = rectangle(80.0, 50.0);
    outline.remove(2);
    Scenario {
        setup: block_with_a_back_top_fillet(Vec::new()),
        edits: json!([update_feature(sketch(
            "f-sk1",
            xy_plane(),
            outline,
            Vec::new()
        ))]),
    }
}
