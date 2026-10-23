//! The scenarios of the reference stability corpus, and the other edited documents engine.md section 6 tests open:
//! each a setup with its picks and the changes of its edit ([`Scenario`], made into model edits by [`edits`]).

use partsmith_engine::Session;
use serde_json::{Value, json};

use super::edits::{after_edit, apply_changes, record};
use super::picks::picked;
use super::records::{
    angle, back_line_notched, block_back_top, block_front_right_vertical, block_outline,
    block_right_side, block_right_top, block_then, block_top, boolean, box_below, box_record,
    chamfer, circle, circle_region, counterbore, cube10, cube20, cube20_front_top, cube20_top,
    datum_offset, datum_on_face, datum_plane, distance, document, extrude, face_plane, fillet,
    fingerprint_number, full_turn, hole, line, linear_pattern, origin_plane, point, rectangle,
    rectangle_region, reference, region, revolve, right_side_redrawn, right_side_split, shell,
    sketch, symmetric, through_all, top_face_plane, with,
};
use crate::common::{Sent, element_named, number};

/// One change of a corpus edit, made into model edits against the document's current records by [`edits`].
#[derive(Debug, Clone)]
pub enum Change {
    /// The entities of a sketch replaced.
    Entities(&'static str, Vec<Value>),
    /// The value at a JSON Pointer of a feature record set, the key added when the record does not have it.
    Set(&'static str, &'static str, Value),
    /// Records inserted, in order, directly before a feature.
    InsertBefore(&'static str, Vec<Value>),
    /// Records inserted, in order, at a timeline position.
    InsertAt(usize, Vec<Value>),
    /// Features removed.
    Remove(Vec<&'static str>),
    /// A feature moved to a timeline position.
    Move(&'static str, usize),
    /// The document units set, without keeping sizes (a `set_units` edit).
    Units(&'static str),
}

/// A corpus scenario: its setup with its picks, and the changes of its edit.
#[derive(Debug, Clone)]
pub struct Scenario {
    /// The setup's text as [`picked`] saves it.
    pub setup: String,
    /// The changes of the edit.
    pub changes: Vec<Change>,
}

impl Scenario {
    /// The scenario whose setup is `setup` (picked here) and whose edit makes `changes`.
    pub fn new(setup: &str, changes: Vec<Change>) -> Scenario {
        Scenario {
            setup: picked(setup),
            changes,
        }
    }

    /// The scenario's setup with `more` changes in its edit.
    pub fn and(mut self, more: Vec<Change>) -> Scenario {
        self.changes.extend(more);
        self
    }
}

/// BLOCK followed by `more`, then BLOCK's outline changed to `outline`.
pub fn block_edited(more: Vec<Value>, outline: Vec<Value>) -> Scenario {
    Scenario::new(
        &document(&["b-1"], &block_then(more)),
        vec![Change::Entities("f-sk1", outline)],
    )
}

/// Corpus R03: a corner bevel removed from the outline. Expressions carry `mm`, so the same text in inches keeps every
/// size (test 153).
pub fn r03() -> Scenario {
    let outline = vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 40.0]),
        line("e-5", [80.0, 40.0], [70.0, 50.0]),
        line("e-3", [70.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
    ];
    let features = vec![
        sketch("f-sk1", origin_plane("XY"), outline),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![region(
                "e-1,e-2,e-3,e-4,e-5",
                [39.535865, 24.725738],
                3950.0,
            )],
            distance("30 mm"),
            "new",
        ),
        fillet(
            "f-fil1",
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
            "2 mm",
        ),
    ];
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Entities("f-sk1", block_outline())],
    )
}

/// Corpus R06 (a): BLOCK, a fillet on the right top edge, a face sketch on the right side with a pocket; `e-2` deleted
/// and `e-9` drawn in its place.
pub fn r06a() -> Scenario {
    block_edited(
        vec![
            fillet("f-fil1", "b-1", vec![block_right_top()], "2"),
            sketch(
                "f-sk2",
                face_plane(
                    "b-1",
                    block_right_side(),
                    [80.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [1.0, 0.0, 0.0],
                ),
                vec![circle("e-5", [25.0, 15.0], 4.0)],
            ),
            with(
                extrude(
                    "f-ext2",
                    "b-1",
                    "f-sk2",
                    vec![circle_region("e-5", [25.0, 15.0], 4.0)],
                    distance("5"),
                    "cut",
                ),
                "reverse",
                json!(true),
            ),
        ],
        right_side_redrawn(),
    )
}

/// Corpus R06 (a) without its face sketch and pocket (engine.md test 117): BLOCK and a fillet of 2 on the right top
/// edge, then `e-2` deleted and `e-9` drawn in its place.
pub fn r06a_fillet_only() -> Scenario {
    block_edited(
        vec![fillet("f-fil1", "b-1", vec![block_right_top()], "2")],
        right_side_redrawn(),
    )
}

/// SLOT(x0, x1) on BLOCK: the datum `f-dp1` XY offset 30, the rectangle `e-5` to `e-8` of `f-sk2` on it from (x0, -1)
/// to (x1, 51), and `f-ext2`, which cuts it 5 into the block's top (file-format.md example E10 for SLOT(30, 50)).
pub fn slot(x0: f64, x1: f64) -> Vec<Value> {
    vec![
        datum_offset("f-dp1", "XY", "30"),
        sketch(
            "f-sk2",
            datum_plane("f-dp1"),
            rectangle(5, x0, -1.0, x1, 51.0),
        ),
        with(
            extrude(
                "f-ext2",
                "b-1",
                "f-sk2",
                vec![rectangle_region(5, x0, -1.0, x1, 51.0)],
                distance("5"),
                "cut",
            ),
            "reverse",
            json!(true),
        ),
    ]
}

/// The setup of corpus R10 (a) and R48: BLOCK with SLOT(30, 50) and a fillet `f-fil1` of `2` on the front top edge of
/// the right piece of the top. Its lengths are numbers without a unit.
fn slotted_block_with_fillet() -> String {
    let mut features = block_then(slot(30.0, 50.0));
    features.push(fillet(
        "f-fil1",
        "b-1",
        vec![reference(
            "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)",
            "line|n=1,0,0|c=65,0,30|l=30|adj=2,2",
        )],
        "2",
    ));
    document(&["b-1"], &features)
}

/// Corpus R10 (a): the slot of [`slotted_block_with_fillet`] moved to x 50..81, which cuts the right piece of the top
/// away.
pub fn r10a() -> Scenario {
    Scenario::new(
        &slotted_block_with_fillet(),
        vec![Change::Entities(
            "f-sk2",
            rectangle(5, 50.0, -1.0, 81.0, 51.0),
        )],
    )
}

/// Corpus R12's features: BLOCK, the datum `f-dp1` XY offset 30, a round pocket `f-ext2` cut `depth` into the top from
/// the circle `e-5` of `f-sk2`, and a fillet `f-fil1` of 2 on the pocket's floor edge.
pub fn r12_features(depth: &str) -> Vec<Value> {
    block_then(vec![
        datum_offset("f-dp1", "XY", "30"),
        sketch(
            "f-sk2",
            datum_plane("f-dp1"),
            vec![circle("e-5", [40.0, 25.0], 10.0)],
        ),
        with(
            extrude(
                "f-ext2",
                "b-1",
                "f-sk2",
                vec![circle_region("e-5", [40.0, 25.0], 10.0)],
                distance(depth),
                "cut",
            ),
            "reverse",
            json!(true),
        ),
        fillet(
            "f-fil1",
            "b-1",
            vec![reference(
                "edge(f-ext2/end-cap|f-ext2/lateral/e-5)",
                "circle|n=0,0,1|c=40,25,25|l=62.831853|r=10|adj=2,1",
            )],
            "2",
        ),
    ])
}

/// Corpus R12: a round pocket 5 deep under a floor fillet made 30 deep, through the block.
pub fn r12() -> Scenario {
    Scenario::new(
        &document(&["b-1"], &r12_features("5")),
        vec![Change::Set("f-ext2", "/extent/expr", json!("30"))],
    )
}

/// Corpus R13: BLOCK, a lower block x 80..120 (`e-5` to `e-8` of `f-sk2`) joined 20 tall beside it by `f-ext2`, a
/// fillet `f-fil1` of 2 on the step edge (BLOCK's right top edge), a boss on a face sketch `f-sk3` on the lower block's
/// top (the circle `e-9` at (100, 25) of radius 5, joined 5 by `f-ext3`) and a fillet `f-fil2` of 1 on the lower
/// block's right top edge; then the lower block made 30 tall, flush with BLOCK.
pub fn r13() -> Scenario {
    let features = block_then(vec![
        sketch(
            "f-sk2",
            origin_plane("XY"),
            rectangle(5, 80.0, 0.0, 120.0, 50.0),
        ),
        extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![rectangle_region(5, 80.0, 0.0, 120.0, 50.0)],
            distance("20"),
            "join",
        ),
        fillet("f-fil1", "b-1", vec![block_right_top()], "2"),
        sketch(
            "f-sk3",
            top_face_plane(
                "b-1",
                reference("f-ext2/end-cap", "plane|n=0,0,1|c=100,25,20|a=2000|adj=4,4"),
                20.0,
            ),
            vec![circle("e-9", [100.0, 25.0], 5.0)],
        ),
        extrude(
            "f-ext3",
            "b-1",
            "f-sk3",
            vec![circle_region("e-9", [100.0, 25.0], 5.0)],
            distance("5"),
            "join",
        ),
        fillet(
            "f-fil2",
            "b-1",
            vec![reference(
                "edge(f-ext2/end-cap|f-ext2/lateral/e-6)",
                "line|n=0,1,0|c=120,25,20|l=50|adj=2,2",
            )],
            "1",
        ),
    ]);
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Set("f-ext2", "/extent/expr", json!("30"))],
    )
}

/// Corpus R16: CUBE20 with a fillet of 2 on its front top edge, and a 5 x 5 rabbet ('Sketch 1' and 'Extrude 1') along
/// that edge inserted before the fillet, drawn as cli.md test 55 draws it.
pub fn r16() -> Scenario {
    let features = vec![
        cube20(),
        fillet("f-fil1", "b-1", vec![cube20_front_top()], "2"),
    ];
    let rabbet = vec![
        sketch(
            "f-sk2",
            origin_plane("YZ"),
            rectangle(1, -11.0, 15.0, -5.0, 21.0),
        ),
        extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![rectangle_region(1, -11.0, 15.0, -5.0, 21.0)],
            symmetric("30"),
            "cut",
        ),
    ];
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::InsertBefore("f-fil1", rabbet)],
    )
}

/// Corpus R23's setup: a 60 x 30 x 10 box, pegs A (`f-sk1`, `f-ext1`) and B (`f-sk2`, `f-ext2`) on it, and a fillet of
/// 1 on peg A's top rim.
fn pegs() -> Vec<Value> {
    vec![
        box_record("f-box1", "b-1", ["60", "30", "10"], None, "new"),
        datum_offset("f-dp1", "XY", "10"),
        sketch(
            "f-sk1",
            datum_plane("f-dp1"),
            vec![circle("e-5", [-5.0, 0.0], 4.0)],
        ),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![circle_region("e-5", [-5.0, 0.0], 4.0)],
            distance("20"),
            "join",
        ),
        sketch(
            "f-sk2",
            datum_plane("f-dp1"),
            vec![circle("e-7", [5.0, 0.0], 4.0)],
        ),
        extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![circle_region("e-7", [5.0, 0.0], 4.0)],
            distance("20"),
            "join",
        ),
        fillet(
            "f-fil1",
            "b-1",
            vec![reference(
                "edge(f-ext1/end-cap|f-ext1/lateral/e-5)",
                "circle|n=0,0,1|c=-5,0,30|l=25.132741|r=4|adj=2,1",
            )],
            "1",
        ),
    ]
}

/// Corpus R23 (a): peg A's extrude suppressed.
pub fn r23a() -> Scenario {
    Scenario::new(
        &document(&["b-1"], &pegs()),
        vec![Change::Set("f-ext1", "/suppressed", json!(true))],
    )
}

/// Corpus R23 (b): peg A's sketch and extrude deleted, which makes peg B's extrude 'Extrude 1'.
pub fn r23b() -> Scenario {
    Scenario::new(
        &document(&["b-1"], &pegs()),
        vec![Change::Remove(vec!["f-sk1", "f-ext1"])],
    )
}

/// Corpus R25: a datum on a chamfer face, with the chamfer suppressed.
pub fn r25() -> Scenario {
    let features = vec![
        cube20(),
        chamfer("f-cha1", "b-1", vec![cube20_front_top()], "2"),
        datum_on_face(
            "f-dp1",
            "b-1",
            reference(
                "f-cha1/chamfer/edge(f-box1/front|f-box1/top)",
                "plane|n=0,-0.707107,0.707107|c=0,-9,19|a=56.568542|adj=4,4",
            ),
            "0",
        ),
        sketch(
            "f-sk1",
            datum_plane("f-dp1"),
            vec![circle("e-1", [0.0, 7.071068], 0.5)],
        ),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![circle_region("e-1", [0.0, 7.071068], 0.5)],
            distance("1"),
            "join",
        ),
    ];
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Set("f-cha1", "/suppressed", json!(true))],
    )
}

/// Corpus R26: BLOCK and a boss on its top (a face sketch `f-sk2` on the top with a circle `e-5` at (40, 25) of radius
/// 10, extruded 10 by `f-ext2` with Operation Join); edit (1) makes 'Extrude 1' 80 tall. [`r26_recreated`] makes edit
/// (2) too.
pub fn r26() -> Scenario {
    let features = block_then(vec![
        sketch_on_block_top(vec![circle("e-5", [40.0, 25.0], 10.0)]),
        extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![circle_region("e-5", [40.0, 25.0], 10.0)],
            distance("10"),
            "join",
        ),
    ]);
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Set("f-ext1", "/extent/expr", json!("80"))],
    )
}

/// Corpus R26 after edit (1) and the delete of edit (2), the first two of the three `apply_edits` engine.md test 136
/// applies: edit (1) ([`after_edit`] of [`r26`], revision 1), then `f-ext1` deleted with one `apply_edits` labeled
/// `Delete` (revision 2), which makes the boss's extrude 'Extrude 1'. Returns the handle, the reply of the delete and
/// the deleted record.
pub fn r26_deleted(session: &mut Session) -> (u64, Sent, Value) {
    let (doc, _) = after_edit(session, &r26());
    let deleted = record(session, doc, "f-ext1");
    let sent = apply_changes(
        session,
        doc,
        Some("Delete"),
        &[Change::Remove(vec!["f-ext1"])],
    );
    (doc, sent, deleted)
}

/// Corpus R26 after its edits as engine.md test 136 applies them, three `apply_edits`: [`r26_deleted`] (revisions 1
/// and 2), then `f-ext3`, the deleted extrude 80 tall under a new id, added at its place (revision 3), which makes it
/// 'Extrude 1'. Returns the handle and the reply of the add.
pub fn r26_recreated(session: &mut Session) -> (u64, Sent) {
    let (doc, _, deleted) = r26_deleted(session);
    let recreated = with(deleted, "id", json!("f-ext3"));
    let added = apply_changes(
        session,
        doc,
        Some("Add"),
        &[Change::InsertAt(1, vec![recreated])],
    );
    (doc, added)
}

/// Corpus R28: BLOCK, a boss on a datum at its top, a fillet of 2 on the boss's top rim, then the fillet dragged
/// directly after 'Extrude 1'.
pub fn r28() -> Scenario {
    let features = block_then(vec![
        datum_offset("f-dp1", "XY", "30"),
        sketch(
            "f-sk2",
            datum_plane("f-dp1"),
            vec![circle("e-5", [40.0, 25.0], 10.0)],
        ),
        extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![circle_region("e-5", [40.0, 25.0], 10.0)],
            distance("10"),
            "join",
        ),
        fillet(
            "f-fil1",
            "b-1",
            vec![reference(
                "edge(f-ext2/end-cap|f-ext2/lateral/e-5)",
                "circle|n=0,0,1|c=40,25,40|l=62.831853|r=10|adj=2,1",
            )],
            "2",
        ),
    ]);
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Move("f-fil1", 2)],
    )
}

/// Corpus R30 (a): BLOCK, a boss `f-ext2` joined symmetric 40 from the circle `e-5` of `f-sk2` on a datum at its top
/// (up to z = 50), and a fillet `f-fil1` of 2 on the boss's top rim; then the boss's Operation changed from Join to
/// Cut.
pub fn r30a() -> Scenario {
    let features = block_then(vec![
        datum_offset("f-dp1", "XY", "30"),
        sketch(
            "f-sk2",
            datum_plane("f-dp1"),
            vec![circle("e-5", [40.0, 25.0], 10.0)],
        ),
        extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![circle_region("e-5", [40.0, 25.0], 10.0)],
            symmetric("40"),
            "join",
        ),
        fillet(
            "f-fil1",
            "b-1",
            vec![reference(
                "edge(f-ext2/end-cap|f-ext2/lateral/e-5)",
                "circle|n=0,0,1|c=40,25,50|l=62.831853|r=10|adj=2,1",
            )],
            "2",
        ),
    ]);
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Set("f-ext2", "/op", json!("cut"))],
    )
}

/// Corpus R30 (b): CUBE20 in `b-1`, a 10 x 10 x 10 box `f-box2` in `b-2` at (0, 0, 14.5) reaching into its top,
/// joined into `b-1` by `f-bool1`, and a fillet `f-fil1` of 1 on the top front edge of 'Box 2'; then the boolean's
/// Operation changed from Join to Cut.
pub fn r30b() -> Scenario {
    let features = vec![
        cube20(),
        box_record(
            "f-box2",
            "b-2",
            ["10", "10", "10"],
            Some(["0", "0", "14.5"]),
            "new",
        ),
        boolean("b-1", &["b-2"], "join"),
        fillet(
            "f-fil1",
            "b-1",
            vec![reference(
                "edge(f-box2/front|f-box2/top)",
                "line|n=1,0,0|c=0,-5,24.5|l=10|adj=2,2",
            )],
            "1",
        ),
    ];
    Scenario::new(
        &document(&["b-1", "b-2"], &features),
        vec![Change::Set("f-bool1", "/op", json!("cut"))],
    )
}

/// A face sketch `f-sk2` on BLOCK's top, with its fallback at z = 30, holding `entities`.
fn sketch_on_block_top(entities: Vec<Value>) -> Value {
    sketch("f-sk2", top_face_plane("b-1", block_top(), 30.0), entities)
}

/// BLOCK followed by `features`, then 'Extrude 1' reversed, so BLOCK's top turns over to the bottom at z = -30, facing
/// -Z (corpus R32 (b)'s edit).
fn turned_over_top(features: Vec<Value>) -> Scenario {
    Scenario::new(
        &document(&["b-1"], &block_then(features)),
        vec![Change::Set("f-ext1", "/reverse", json!(true))],
    )
}

/// Corpus R32 (b): BLOCK, a face sketch `f-sk2` on its top with the point `e-p1` at (20, 10), and a hole `f-hole1` of
/// diameter 5 and depth 10 there; then 'Extrude 1' reversed.
pub fn r32b() -> Scenario {
    turned_over_top(vec![
        sketch_on_block_top(vec![point("e-p1", [20.0, 10.0])]),
        hole("f-hole1", "b-1", "f-sk2", &["e-p1"], "5", distance("10")),
    ])
}

/// Corpus R32 (b) with a chamfered hole at the end of an arc (engine.md section 3.22): a face sketch `f-sk2` on
/// BLOCK's top with its origin at the top's center (40, 25, 30) and the arc `e-5` of radius 5 about (5, 5) from 0 to
/// 90 degrees, a hole `f-hole1` of diameter 3 and depth 5 at its start (10, 5), world (50, 30), and a chamfer `f-cha1`
/// of 0.5 on the hole's rim `edge(f-ext1/end-cap|f-hole1/bore/e-5.start)`; then 'Extrude 1' reversed, which drills the
/// hole from the bottom at world (50, 20), where the mirrored drawing puts the arc's start.
pub fn r32b_chamfered_hole_at_an_arc_end() -> Scenario {
    let arc = json!({
        "id": "e-5", "type": "arc", "center": [5, 5], "radius": 5,
        "start_angle": 0, "end_angle": std::f64::consts::FRAC_PI_2
    });
    turned_over_top(vec![
        sketch(
            "f-sk2",
            face_plane(
                "b-1",
                block_top(),
                [40.0, 25.0, 30.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
            ),
            vec![arc],
        ),
        hole(
            "f-hole1",
            "b-1",
            "f-sk2",
            &["e-5.start"],
            "3",
            distance("5"),
        ),
        chamfer(
            "f-cha1",
            "b-1",
            vec![reference(
                "edge(f-ext1/end-cap|f-hole1/bore/e-5.start)",
                "circle|n=0,0,1|c=50,30,30|l=9.424778|adj=2,2",
            )],
            "0.5",
        ),
    ])
}

/// Corpus R32 (b) with BLOCK moved by (100, 0, 0) before its face sketch (features/sketch.md section 3.5): the sketch's
/// fallback at z = 30 is in body coordinates, so the sketch stands at (100, 0, 30) and the point `e-p1` at world
/// (120, 10); then 'Extrude 1' reversed.
pub fn r32b_moved() -> Scenario {
    turned_over_top(vec![
        moved_by_100(),
        sketch_on_block_top(vec![point("e-p1", [20.0, 10.0])]),
        hole("f-hole1", "b-1", "f-sk2", &["e-p1"], "5", distance("10")),
    ])
}

/// Corpus R32 (b) with a pocket in place of the hole (the region re-anchoring of `TurnDrawingBack`, engine.md section
/// 3.22): the rectangle `e-11` to `e-14` from (10, 5) to (30, 15) on BLOCK's top, held 10 tall by the line form of a
/// vertical distance on `e-12` (which mirroring rewrites, sketch.md section 3.17), cut 5 into the block by `f-ext2`;
/// then 'Extrude 1' reversed.
pub fn r32b_pocket() -> Scenario {
    let height =
        json!({"id": "c-11", "type": "vertical_distance", "line": "e-12", "expr": "10 mm"});
    turned_over_top(vec![
        with(
            sketch_on_block_top(rectangle(11, 10.0, 5.0, 30.0, 15.0)),
            "constraints",
            json!([height]),
        ),
        with(
            extrude(
                "f-ext2",
                "b-1",
                "f-sk2",
                vec![rectangle_region(11, 10.0, 5.0, 30.0, 15.0)],
                distance("5"),
                "cut",
            ),
            "reverse",
            json!(true),
        ),
    ])
}

/// The bosses on a moved box of the frame tests (engine.md tests 125 and 135, ADR 0023): a 20 x 30 x 40 box `f-box1`
/// of `b-1`, centered on the Z axis from z = 0 to 40, the Move bodies `moves`, then a sketch `f-sk1` on `plane` with
/// the square `e-1` to `e-4` from (95, -5) to (105, 5) and the free point `e-9` at (100, 0), joined 10 by `f-ext1`, then
/// a sketch `f-sk3` on the end cap of `f-ext1` with the circle `e-10` of radius 2 at (100, 0), joined 5 by `f-ext3`.
/// The fallback of `f-sk3`, origin `cap` with x axis (1, 0, 0) and normal `cap_normal`, is in body coordinates: the
/// descriptor's fallback a host stores for that cap when `f-sk1` has no motion.
pub fn bosses_on_a_moved_box(
    moves: Vec<Value>,
    plane: Value,
    cap: [f64; 3],
    cap_normal: [f64; 3],
) -> String {
    let mut first = rectangle(1, 95.0, -5.0, 105.0, 5.0);
    first.push(point("e-9", [100.0, 0.0]));
    let cap_plane = with(
        face_plane(
            "b-1",
            reference(
                "f-ext1/end-cap",
                &format!(
                    "plane|n=0,0,{}|c=100,0,{}|a=100|adj=4,4",
                    fingerprint_number(cap_normal[2]),
                    fingerprint_number(cap[2])
                ),
            ),
            cap,
            [1.0, 0.0, 0.0],
            cap_normal,
        ),
        "fallback_frame",
        json!("body"),
    );
    let mut features = vec![box_record("f-box1", "b-1", ["20", "30", "40"], None, "new")];
    features.extend(moves);
    features.extend([
        sketch("f-sk1", plane, first),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![rectangle_region(1, 95.0, -5.0, 105.0, 5.0)],
            distance("10"),
            "join",
        ),
        sketch("f-sk3", cap_plane, vec![circle("e-10", [100.0, 0.0], 2.0)]),
        extrude(
            "f-ext3",
            "b-1",
            "f-sk3",
            vec![circle_region("e-10", [100.0, 0.0], 2.0)],
            distance("5"),
            "join",
        ),
    ]);
    document(&["b-1"], &features)
}

/// The Move body `f-mv1` of `b-1` by (100, 0, 0), the move of engine.md tests 124, 125 and 135.
pub fn moved_by_100() -> Value {
    json!({"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["100", "0", "0"]})
}

/// The round wall of the boss `f-ext3` of [`bosses_on_a_moved_box`].
pub const UPPER_BOSS_WALL: &str = "f-ext3/lateral/e-10";

/// The x and y of the axis origin of the element of `b-1` named `element`.
pub fn axis_xy(session: &mut Session, doc: u64, element: &str) -> [f64; 2] {
    let origin = &element_named(session, doc, "b-1", element)["axis"]["origin"];
    [number(&origin[0]), number(&origin[1])]
}

/// Corpus R33: a 20 x 20 x 40 box with a boss on its top (a face sketch `f-sk2` attached when the box was 10 tall, so
/// its fallback is at z = 10, with a circle `e-5` at (0, 0) of radius 3, extruded 5 by `f-ext2` with Operation Join);
/// then a cut inserted before the sketch (`f-dp1` at z = 35, `f-sk3` with a 30 x 30 square on it, `f-ext3` cutting
/// z 35..40), which removes the top and makes its own start cap the new top at z = 35. The cut is then 'Extrude 1'.
pub fn r33() -> Scenario {
    let features = vec![
        box_record("f-box1", "b-1", ["20", "20", "40"], None, "new"),
        sketch(
            "f-sk2",
            top_face_plane(
                "b-1",
                reference("f-box1/top", "plane|n=0,0,1|c=0,0,10|a=400|adj=4,4"),
                10.0,
            ),
            vec![circle("e-5", [0.0, 0.0], 3.0)],
        ),
        extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![circle_region("e-5", [0.0, 0.0], 3.0)],
            distance("5"),
            "join",
        ),
    ];
    let cut = vec![
        datum_offset("f-dp1", "XY", "35"),
        sketch(
            "f-sk3",
            datum_plane("f-dp1"),
            rectangle(11, -15.0, -15.0, 15.0, 15.0),
        ),
        extrude(
            "f-ext3",
            "b-1",
            "f-sk3",
            vec![rectangle_region(11, -15.0, -15.0, 15.0, 15.0)],
            distance("10"),
            "cut",
        ),
    ];
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::InsertBefore("f-sk2", cut)],
    )
}

/// Corpus R41: CUBE10 patterned 3 times along X, a fillet of 1 on the top right edges of the second and third cube,
/// then the count lowered from 3 to 2.
pub fn r41() -> Scenario {
    let copy_right_top_edge = |copy: u32| {
        reference(
            &format!("edge(f-box1/right@f-lp1:{copy}|f-box1/top@f-lp1:{copy})"),
            &format!("line|n=0,1,0|c={},0,10|l=10|adj=2,2", 5 + 20 * copy),
        )
    };
    let features = vec![
        cube10(),
        linear_pattern("3", "20"),
        fillet(
            "f-fil1",
            "b-1",
            vec![copy_right_top_edge(1), copy_right_top_edge(2)],
            "1",
        ),
    ];
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Set("f-lp1", "/direction/count", json!("2"))],
    )
}

/// Corpus R48: the setup of R10 (a), whose lengths are numbers without a unit, switched from millimeters to inches.
pub fn r48() -> Scenario {
    Scenario::new(&slotted_block_with_fillet(), vec![Change::Units("in")])
}

/// Corpus R50: a circle drawn inside BLOCK's region.
pub fn r50() -> Scenario {
    let mut outline = block_outline();
    outline.push(circle("e-5", [40.0, 25.0], 10.0));
    block_edited(Vec::new(), outline)
}

/// Corpus R54: a boss under a datum, with the boss's sketch and extrude deleted.
pub fn r54() -> Scenario {
    let features = block_then(vec![
        sketch(
            "f-sk2",
            top_face_plane("b-1", block_top(), 30.0),
            rectangle(11, 20.0, 10.0, 60.0, 30.0),
        ),
        extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![rectangle_region(11, 20.0, 10.0, 60.0, 30.0)],
            distance("10"),
            "join",
        ),
        datum_on_face(
            "f-dp1",
            "b-1",
            reference("f-ext2/end-cap", "plane|n=0,0,1|c=40,20,40|a=800|adj=4,4"),
            "0",
        ),
        sketch(
            "f-sk3",
            datum_plane("f-dp1"),
            vec![circle("e-21", [40.0, 20.0], 5.0)],
        ),
        extrude(
            "f-ext3",
            "b-1",
            "f-sk3",
            vec![circle_region("e-21", [40.0, 20.0], 5.0)],
            distance("5"),
            "join",
        ),
    ]);
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Remove(vec!["f-sk2", "f-ext2"])],
    )
}

/// Corpus R56 (b): a small box joined onto CUBE20 by a boolean, a fillet on the small box's top front edge, then the
/// boolean's target and tool swapped.
pub fn r56b() -> Scenario {
    let features = vec![
        cube20(),
        box_record(
            "f-box2",
            "b-2",
            ["10", "10", "10"],
            Some(["0", "0", "20"]),
            "new",
        ),
        boolean("b-1", &["b-2"], "join"),
        fillet(
            "f-fil1",
            "b-1",
            vec![reference(
                "edge(f-box2/front|f-box2/top)",
                "line|n=1,0,0|c=0,-5,30|l=10|adj=2,2",
            )],
            "1",
        ),
    ];
    Scenario::new(
        &document(&["b-1", "b-2"], &features),
        vec![
            Change::Set("f-bool1", "/target", json!("b-2")),
            Change::Set("f-bool1", "/tools", json!(["b-1"])),
        ],
    )
}

/// Corpus R57: the 10 x 10 section `e-1` to `e-4` of `f-sk1` on XZ from (10, 0) to (20, 10) revolved 270 degrees about
/// the Z axis by `f-rev1`, a chamfer `f-cha1` of 1 on the top edge of its end face, and a cut `f-ext2` 3 deep from the
/// circle `e-11` of radius 2 of a face sketch `f-sk2` on its start face; then the revolve made a full turn.
pub fn r57() -> Scenario {
    let features = vec![
        sketch(
            "f-sk1",
            origin_plane("XZ"),
            rectangle(1, 10.0, 0.0, 20.0, 10.0),
        ),
        revolve(
            "f-rev1",
            "b-1",
            "f-sk1",
            vec![rectangle_region(1, 10.0, 0.0, 20.0, 10.0)],
            json!({"kind": "origin", "axis": "Z"}),
            angle("270"),
            "new",
        ),
        chamfer(
            "f-cha1",
            "b-1",
            vec![reference(
                "edge(f-rev1/end-cap|f-rev1/lateral/e-3)",
                "line|n=0,1,0|c=0,-15,10|l=10|adj=2,2",
            )],
            "1",
        ),
        sketch(
            "f-sk2",
            face_plane(
                "b-1",
                reference("f-rev1/start-cap", "plane|n=0,-1,0|c=15,0,5|a=100|adj=4,4"),
                [15.0, 0.0, 5.0],
                [1.0, 0.0, 0.0],
                [0.0, -1.0, 0.0],
            ),
            vec![circle("e-11", [0.0, 0.0], 2.0)],
        ),
        with(
            extrude(
                "f-ext2",
                "b-1",
                "f-sk2",
                vec![circle_region("e-11", [0.0, 0.0], 2.0)],
                distance("3"),
                "cut",
            ),
            "reverse",
            json!(true),
        ),
    ];
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Set("f-rev1", "/extent", full_turn())],
    )
}

/// Corpus R63 (a): holes at (0, 0) and (10, 10) in BOX-BELOW, a chamfer of 0.5 on the top rim of the second, then the
/// hole type set to a counterbore of 11 by 4.
pub fn r63a() -> Scenario {
    let features = vec![
        box_below(),
        sketch(
            "f-sk1",
            origin_plane("XY"),
            vec![point("e-p1", [0.0, 0.0]), point("e-p2", [10.0, 10.0])],
        ),
        hole(
            "f-hole1",
            "b-1",
            "f-sk1",
            &["e-p1", "e-p2"],
            "6",
            through_all(),
        ),
        chamfer(
            "f-cha1",
            "b-1",
            vec![reference(
                "edge(f-box1/top|f-hole1/bore/e-p2)",
                "circle|n=0,0,1|c=10,10,0|l=18.849556|r=3|adj=2,1",
            )],
            "0.5",
        ),
    ];
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Set("f-hole1", "/hole_type", counterbore("11", "4"))],
    )
}

/// Corpus R66: BLOCK, a fillet of 1 on the back top edge, then `e-3` replaced by the five lines of a notch.
pub fn r66() -> Scenario {
    block_edited(
        vec![fillet("f-fil1", "b-1", vec![block_back_top()], "1")],
        back_line_notched(),
    )
}

/// A datum plane `f-dp1` offset 5 from BLOCK's right side (engine.md section 3.22, `KeepMatch` of a field that holds
/// one reference), then corpus R49's edit, so the face it is on follows to both halves of the side.
pub fn datum_on_a_split_side() -> Scenario {
    block_edited(
        vec![datum_on_face("f-dp1", "b-1", block_right_side(), "5")],
        right_side_split(),
    )
}

/// CUBE20 with a shell `f-sh1` 2 thick whose params store the top under the misspelled key `remove_face` instead of
/// `remove_faces` (engine.md test 131).
pub fn shell_with_a_misspelled_key() -> String {
    picked(&document(
        &["b-1"],
        &[cube20(), shell("2", "remove_face", vec![cube20_top()])],
    ))
}

/// Engine.md test 119: BLOCK with `f-sk1` changed to the lines of sketch.md test 231, so its region matches two
/// separate areas.
pub fn two_areas() -> Scenario {
    let lines = vec![
        line("e-1", [0.0, 0.0], [200.0, 0.0]),
        line("e-9", [80.0, 0.0], [80.0, 50.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-5", [200.0, 0.0], [200.0, 20.0]),
        line("e-6", [200.0, 20.0], [150.0, 20.0]),
        line("e-7", [150.0, 20.0], [150.0, 0.0]),
    ];
    block_edited(Vec::new(), lines)
}

/// Corpus R06 (a) on a profile of two regions (the re-anchoring of `KeepRegions`, engine.md section 3.22): BLOCK and a
/// separate 20 x 20 tile `e-5` to `e-8` at x 100..120 extruded together, and a fillet of 2 on BLOCK's front right
/// vertical edge. The edit deletes `e-2` and draws `e-9` in its place (BLOCK's region follows its outline with the
/// succession `e-2 -> [e-9]`, and the fillet follows it with a notice) and draws a guide circle `e-10` inside the tile
/// (the tile's region is split, with the `region_split` notice and `Keep these regions`).
pub fn redrawn_beside_a_split() -> Scenario {
    redrawn_beside_a_split_then(Vec::new())
}

/// [`redrawn_beside_a_split`] with `more` features after the fillet.
fn redrawn_beside_a_split_then(more: Vec<Value>) -> Scenario {
    let tile = rectangle(5, 100.0, 0.0, 120.0, 20.0);
    let mut outline = block_outline();
    outline.extend(tile.clone());
    let mut features = vec![
        sketch("f-sk1", origin_plane("XY"), outline),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![
                rectangle_region(1, 0.0, 0.0, 80.0, 50.0),
                rectangle_region(5, 100.0, 0.0, 120.0, 20.0),
            ],
            distance("30"),
            "new",
        ),
        fillet("f-fil1", "b-1", vec![block_front_right_vertical()], "2"),
    ];
    features.extend(more);
    let mut edited = right_side_redrawn();
    edited.extend(tile);
    edited.push(circle("e-10", [110.0, 10.0], 5.0));
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Entities("f-sk1", edited)],
    )
}

/// [`redrawn_beside_a_split`] with a face sketch `f-sk2` on BLOCK's right side after the fillet, a circle `e-1` at
/// (25, 15) of radius 4 on it. After the edit the sketch follows the replaced line to `f-ext1/lateral/e-9` with a
/// `reference_followed` notice at `/plane/face` (features.md section 3.6, features/sketch.md section 4).
pub fn redrawn_beside_a_split_with_a_face_sketch() -> Scenario {
    redrawn_beside_a_split_then(vec![sketch(
        "f-sk2",
        face_plane(
            "b-1",
            block_right_side(),
            [80.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
        ),
        vec![circle("e-1", [25.0, 15.0], 4.0)],
    )])
}
