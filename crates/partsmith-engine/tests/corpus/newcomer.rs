//! The newcomer error corpus (`docs/design/newcomer-error-corpus.md`). A scenario without an edit is its setup's text
//! with its picks; one with an edit is a [`Scenario`].

use serde_json::{Value, json};

use super::picks::picked;
use super::records::{
    block, block_back_top, block_outline, block_then, block_top, box_record, circle, circle_region,
    coincident, constrained_sketch, datum_offset, datum_plane, distance, document, document_with,
    extrude, fillet, hole, line, line_distance, origin_plane, parameter, point, rectangle,
    rectangle_region, reference, shell, sketch, through_all, top_face_plane,
};
use super::scenarios::{Change, Scenario, block_edited};

/// Newcomer error NC01: BLOCK's outline with a 0.3 mm gap at the origin (`e-4` ends at (0, 0.3)); no feature uses the
/// sketch yet.
pub fn nc01() -> String {
    let mut outline = block_outline();
    outline[3] = line("e-4", [0.0, 50.0], [0.0, 0.3]);
    picked(&document(
        &["b-1"],
        &[sketch("f-sk1", origin_plane("XY"), outline)],
    ))
}

/// The sketch `f-sk2` of NC02 to NC04 on `plane`: the circle `e-5` at (40, 25) of radius 5.
fn small_circle_sketch(plane: Value) -> Value {
    sketch("f-sk2", plane, vec![circle("e-5", [40.0, 25.0], 5.0)])
}

/// `f-ext2`, the circle of [`small_circle_sketch`] extruded `depth` with `op`.
fn small_circle_extrude(depth: &str, op: &str) -> Value {
    extrude(
        "f-ext2",
        "b-1",
        "f-sk2",
        vec![circle_region("e-5", [40.0, 25.0], 5.0)],
        distance(depth),
        op,
    )
}

/// Newcomer error NC02: BLOCK and the circle `e-5` of `f-sk2` on XY joined 10 by `f-ext2`, inside the block.
pub fn nc02() -> String {
    picked(&document(
        &["b-1"],
        &block_then(vec![
            small_circle_sketch(origin_plane("XY")),
            small_circle_extrude("10", "join"),
        ]),
    ))
}

/// NC03 (op cut) and NC04 (op join): BLOCK, the datum `f-dp1` XY offset 40, and the circle `e-5` of `f-sk2` on it
/// extruded `depth` by `f-ext2` with `op`, above the block.
pub fn above_the_block(depth: &str, op: &str) -> String {
    picked(&document(
        &["b-1"],
        &block_then(vec![
            datum_offset("f-dp1", "XY", "40"),
            small_circle_sketch(datum_plane("f-dp1")),
            small_circle_extrude(depth, op),
        ]),
    ))
}

/// Newcomer error NC03: a cut 15 deep that starts 10 above the block and points away from it.
pub fn nc03() -> String {
    above_the_block("15", "cut")
}

/// Newcomer error NC04: a join 15 deep that floats 10 above the block.
pub fn nc04() -> String {
    above_the_block("15", "join")
}

/// Newcomer error NC05: `f-box1` 40 x 20 x 10, `f-box2` 20 x 30 x 5 at (10, 0, 7.6) cutting a step 2.4 mm deep whose
/// riser is `f-box2/left` at x = 0, and `f-fil1` of radius 3 on the edge between the top of 'Box 1' and the riser.
pub fn nc05() -> String {
    picked(&document(
        &["b-1"],
        &[
            box_record("f-box1", "b-1", ["40", "20", "10"], None, "new"),
            box_record(
                "f-box2",
                "b-1",
                ["20", "30", "5"],
                Some(["10", "0", "7.6"]),
                "cut",
            ),
            fillet(
                "f-fil1",
                "b-1",
                vec![reference(
                    "edge(f-box1/top|f-box2/left)",
                    "line|n=0,1,0|c=0,0,10|l=20|adj=2,2",
                )],
                "3",
            ),
        ],
    ))
}

/// Newcomer error NC06: `f-box1` 40 x 40 x 20, `f-fil1` of radius 1 on its four vertical edges, and `f-sh1` 2 thick
/// with its top removed.
pub fn nc06() -> String {
    let vertical = |sides: &str, x: f64, y: f64| {
        reference(
            &format!("edge({sides})"),
            &format!("line|n=0,0,1|c={x},{y},10|l=20|adj=2,2"),
        )
    };
    picked(&document(
        &["b-1"],
        &[
            box_record("f-box1", "b-1", ["40", "40", "20"], None, "new"),
            fillet(
                "f-fil1",
                "b-1",
                vec![
                    vertical("f-box1/front|f-box1/left", -20.0, -20.0),
                    vertical("f-box1/front|f-box1/right", 20.0, -20.0),
                    vertical("f-box1/back|f-box1/left", -20.0, 20.0),
                    vertical("f-box1/back|f-box1/right", 20.0, 20.0),
                ],
                "1",
            ),
            shell(
                "2",
                "remove_faces",
                vec![reference(
                    "f-box1/top",
                    "plane|n=0,0,1|c=0,0,20|a=1599.141593|adj=8,8",
                )],
            ),
        ],
    ))
}

/// Newcomer error NC07: the 40 x 20 rectangle `e-1` to `e-4` of `f-sk1`, loose lines with Distance `c-1` 40 mm on
/// `e-1`, extruded 10 by `f-ext1`; the edit adds Distance `c-2` 30 mm on `e-1`.
pub fn nc07() -> Scenario {
    let features = vec![
        constrained_sketch(
            "f-sk1",
            origin_plane("XY"),
            rectangle(1, 0.0, 0.0, 40.0, 20.0),
            vec![line_distance("c-1", "e-1", "40 mm")],
        ),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![rectangle_region(1, 0.0, 0.0, 40.0, 20.0)],
            distance("10"),
            "new",
        ),
    ];
    Scenario::new(
        &document(&["b-1"], &features),
        vec![Change::Set(
            "f-sk1",
            "/constraints/1",
            line_distance("c-2", "e-1", "30 mm"),
        )],
    )
}

/// Newcomer error NC08: the triangle `e-1` (0, 0)-(30, 0), `e-2` (30, 0)-(30, 40), `e-3` (30, 40)-(0, 0) of `f-sk1`
/// with Distance `c-1` 30 mm, `c-2` 40 mm and `c-3` 50 mm on its sides and coincident corners `c-4` to `c-6`, as
/// sketch.md test 251 draws it; the edit sets `c-3` to 80 mm, which no triangle with sides 30 and 40 can have.
pub fn nc08() -> Scenario {
    let triangle = vec![
        line("e-1", [0.0, 0.0], [30.0, 0.0]),
        line("e-2", [30.0, 0.0], [30.0, 40.0]),
        line("e-3", [30.0, 40.0], [0.0, 0.0]),
    ];
    let constraints = vec![
        line_distance("c-1", "e-1", "30 mm"),
        line_distance("c-2", "e-2", "40 mm"),
        line_distance("c-3", "e-3", "50 mm"),
        coincident("c-4", "e-1.end", "e-2.start"),
        coincident("c-5", "e-2.end", "e-3.start"),
        coincident("c-6", "e-3.end", "e-1.start"),
    ];
    Scenario::new(
        &document(
            &["b-1"],
            &[constrained_sketch(
                "f-sk1",
                origin_plane("XY"),
                triangle,
                constraints,
            )],
        ),
        vec![Change::Set("f-sk1", "/constraints/2/expr", json!("80 mm"))],
    )
}

/// Newcomer error NC09: a millimeter document with `f-box1` of size `40`, `20` and `10` and `f-fil1` of radius `2` on
/// its front top edge, four numbers without a unit.
pub fn nc09() -> String {
    picked(&document(
        &["b-1"],
        &[
            box_record("f-box1", "b-1", ["40", "20", "10"], None, "new"),
            fillet(
                "f-fil1",
                "b-1",
                vec![reference(
                    "edge(f-box1/front|f-box1/top)",
                    "line|n=1,0,0|c=0,-10,10|l=40|adj=2,2",
                )],
                "2",
            ),
        ],
    ))
}

/// Newcomer error NC10: BLOCK with the parameter `height` 30 mm and `f-ext1`'s distance misspelled `hieght`.
pub fn nc10() -> String {
    let mut features = block();
    features[1]["extent"] = distance("hieght");
    picked(&document_with(
        "mm",
        &[parameter("p-height", "height", "30 mm")],
        &["b-1"],
        &features,
    ))
}

/// The cause line of every message newcomer error NC11 gives.
pub const NC11_CAUSE: &str =
    "This started after you deleted the line from (80, 50) to (0, 50) in 'Sketch 1'.";

/// Newcomer error NC11: BLOCK, a fillet of 2 on the back top edge, then the back line `e-3` deleted.
pub fn nc11() -> Scenario {
    let without_back_line = vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
    ];
    block_edited(
        vec![fillet("f-fil1", "b-1", vec![block_back_top()], "2")],
        without_back_line,
    )
}

/// Newcomer error NC12: BLOCK, a face sketch `f-sk2` on its top with the point `e-p1` at (95, 25), past the right
/// side, and `f-hole1`, a hole of 5 through all at that point.
pub fn nc12() -> String {
    picked(&document(
        &["b-1"],
        &block_then(vec![
            sketch(
                "f-sk2",
                top_face_plane("b-1", block_top(), 30.0),
                vec![point("e-p1", [95.0, 25.0])],
            ),
            hole("f-hole1", "b-1", "f-sk2", &["e-p1"], "5", through_all()),
        ]),
    ))
}
