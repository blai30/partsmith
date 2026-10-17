//! Feature records as `.ptsm` JSON for the corpus scenarios: sketch entities, regions, planes,
//! feature records, the corpus's shared setups (BLOCK, SLOT, CUBE20, CUBE10 and BOX-BELOW) with
//! their picked references, the setups more than one test file runs (PEGS of R23 to the notched
//! outline of R66), and edits of feature lists.

use std::f64::consts::{FRAC_PI_2, PI};

use serde_json::{Value, json};

// Numbers and references.

/// A number in fingerprint text form (naming.md section 3.7.3): six decimals, trailing zeros and
/// a trailing point removed, `-0` written as `0`.
pub fn fingerprint_number(value: f64) -> String {
    let mut text = format!("{value:.6}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        text = "0".to_owned();
    }
    text
}

/// A number in region fingerprint text form (sketch.md section 3.13.6): rounded to six decimals
/// and written in shortest form, a rounded zero written `0`.
pub fn region_number(value: f64) -> String {
    let rounded = (value * 1e6).round() / 1e6;
    if rounded == 0.0 {
        "0".to_owned()
    } else {
        format!("{rounded}")
    }
}

/// A stored reference `{q, fp}`.
pub fn reference(query: &str, fingerprint: &str) -> Value {
    json!({"q": query, "fp": fingerprint})
}

// Sketch entities, constraints and regions.

/// A line entity.
pub fn line(entity: &str, start: [f64; 2], end: [f64; 2]) -> Value {
    json!({"id": entity, "type": "line", "start": start, "end": end})
}

/// A circle entity.
pub fn circle(entity: &str, center: [f64; 2], radius: f64) -> Value {
    json!({"id": entity, "type": "circle", "center": center, "radius": radius})
}

/// An arc entity running counterclockwise from `start_angle` to `end_angle` (radians from +u).
pub fn arc(entity: &str, center: [f64; 2], radius: f64, start_angle: f64, end_angle: f64) -> Value {
    json!({
        "id": entity, "type": "arc", "center": center, "radius": radius,
        "start_angle": start_angle, "end_angle": end_angle
    })
}

/// A point entity.
pub fn point(entity: &str, position: [f64; 2]) -> Value {
    json!({"id": entity, "type": "point", "position": position})
}

/// Four lines `e-<first>` to `e-<first + 3>` drawn counterclockwise from (x0, y0), as features.md
/// section 6.1 `RECT` draws them.
pub fn rectangle(first: usize, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Value> {
    let corners = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    (0..4)
        .map(|i| {
            line(
                &format!("e-{}", first + i),
                corners[i],
                corners[(i + 1) % 4],
            )
        })
        .collect()
}

/// A dimensional constraint (`radius` or `diameter`) on a circle or arc.
pub fn size_constraint(constraint: &str, kind: &str, curve: &str, expr: &str) -> Value {
    json!({"id": constraint, "type": kind, "curve": curve, "expr": expr})
}

/// A region reference with this key and the fingerprint of a region with this centroid and area.
pub fn region(key: &str, center: [f64; 2], area: f64) -> Value {
    json!({
        "key": key,
        "fp": format!(
            "c={},{}|a={}",
            region_number(center[0]),
            region_number(center[1]),
            region_number(area)
        )
    })
}

/// The region reference of [`rectangle`].
pub fn rectangle_region(first: usize, x0: f64, y0: f64, x1: f64, y1: f64) -> Value {
    let key = (first..first + 4)
        .map(|number| format!("e-{number}"))
        .collect::<Vec<_>>()
        .join(",");
    region(
        &key,
        [(x0 + x1) / 2.0, (y0 + y1) / 2.0],
        (x1 - x0) * (y1 - y0),
    )
}

/// The region reference of a circle entity.
pub fn circle_region(entity: &str, center: [f64; 2], radius: f64) -> Value {
    region(entity, center, PI * radius * radius)
}

// Planes, sketches and datum planes.

/// An origin plane `{"kind": "origin", "plane": <name>}`.
pub fn origin_plane(name: &str) -> Value {
    json!({"kind": "origin", "plane": name})
}

/// A datum plane reference `{"kind": "datum", "feature": <id>}`.
pub fn datum_plane(feature: &str) -> Value {
    json!({"kind": "datum", "feature": feature})
}

/// A face plane of a sketch, with its fallback placement written after the reference.
pub fn face_plane(
    body: &str,
    face: Value,
    origin: [f64; 3],
    x_axis: [f64; 3],
    normal: [f64; 3],
) -> Value {
    json!({
        "kind": "face",
        "body": body,
        "face": face,
        "fallback": {"origin": origin, "x_axis": x_axis, "normal": normal}
    })
}

/// A face plane of a sketch whose fallback is the XY frame at height `z`.
pub fn top_face_plane(body: &str, face: Value, z: f64) -> Value {
    face_plane(body, face, [0.0, 0.0, z], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0])
}

/// A sketch record without constraints.
pub fn sketch(feature: &str, plane: Value, entities: Vec<Value>) -> Value {
    json!({"id": feature, "type": "sketch", "plane": plane, "entities": entities})
}

/// A sketch record with constraints.
pub fn constrained_sketch(
    feature: &str,
    plane: Value,
    entities: Vec<Value>,
    constraints: Vec<Value>,
) -> Value {
    with(
        sketch(feature, plane, entities),
        "constraints",
        json!(constraints),
    )
}

/// A datum plane record offset from an origin plane.
pub fn datum_offset(feature: &str, plane: &str, offset: &str) -> Value {
    json!({"id": feature, "type": "datum_plane", "base": origin_plane(plane), "offset": offset})
}

/// A datum plane record on a face of a body.
pub fn datum_on_face(feature: &str, body: &str, face: Value, offset: &str) -> Value {
    json!({
        "id": feature,
        "type": "datum_plane",
        "base": {"kind": "face", "body": body, "face": face},
        "offset": offset
    })
}

// Extents and axes.

/// A distance extent.
pub fn distance(expr: &str) -> Value {
    json!({"kind": "distance", "expr": expr})
}

/// A symmetric extent.
pub fn symmetric(expr: &str) -> Value {
    json!({"kind": "symmetric", "expr": expr})
}

/// A through-all extent.
pub fn through_all() -> Value {
    json!({"kind": "through_all"})
}

/// An angle extent of a revolve.
pub fn angle(expr: &str) -> Value {
    json!({"kind": "angle", "expr": expr})
}

/// The full-turn extent of a revolve.
pub fn full_turn() -> Value {
    json!({"kind": "full"})
}

/// An axis on an edge of a body.
pub fn edge_axis(body: &str, edge: Value) -> Value {
    json!({"kind": "edge", "body": body, "edge": edge})
}

/// A counterbore hole type.
pub fn counterbore(diameter: &str, depth: &str) -> Value {
    json!({"kind": "counterbore", "diameter": diameter, "depth": depth})
}

// Feature records.

/// The same record with `key` set to `value`.
pub fn with(mut record: Value, key: &str, value: Value) -> Value {
    record[key] = value;
    record
}

/// The same record with `"reverse": true`.
pub fn reversed(record: Value) -> Value {
    with(record, "reverse", json!(true))
}

/// The same record with `"suppressed": true`.
pub fn suppressed(record: Value) -> Value {
    with(record, "suppressed", json!(true))
}

/// The same record without its `suppressed` key (unsuppressed, as the host writes it).
pub fn unsuppressed(mut record: Value) -> Value {
    if let Some(object) = record.as_object_mut() {
        object.remove("suppressed");
    }
    record
}

/// An extrude record into `body`.
pub fn extrude(
    feature: &str,
    body: &str,
    sketch: &str,
    regions: Vec<Value>,
    extent: Value,
    op: &str,
) -> Value {
    json!({
        "id": feature, "type": "extrude", "body": body,
        "profile": {"sketch": sketch, "regions": regions},
        "extent": extent, "op": op
    })
}

/// A revolve record into `body`.
pub fn revolve(
    feature: &str,
    body: &str,
    sketch: &str,
    regions: Vec<Value>,
    axis: Value,
    extent: Value,
    op: &str,
) -> Value {
    json!({
        "id": feature, "type": "revolve", "body": body,
        "profile": {"sketch": sketch, "regions": regions},
        "axis": axis, "extent": extent, "op": op
    })
}

/// A box record; `position` is omitted when `None`.
pub fn box_record(
    feature: &str,
    body: &str,
    size: [&str; 3],
    position: Option<[&str; 3]>,
    op: &str,
) -> Value {
    let mut record = json!({"id": feature, "type": "box", "body": body, "size": size});
    if let Some(position) = position {
        record["position"] = json!(position);
    }
    record["op"] = json!(op);
    record
}

/// A fillet record.
pub fn fillet(feature: &str, body: &str, edges: Vec<Value>, radius: &str) -> Value {
    json!({"id": feature, "type": "fillet", "body": body, "edges": edges, "radius": radius})
}

/// An equal-distance chamfer record.
pub fn chamfer(feature: &str, body: &str, edges: Vec<Value>, distance: &str) -> Value {
    json!({
        "id": feature, "type": "chamfer", "body": body, "edges": edges,
        "distance": {"kind": "equal", "expr": distance}
    })
}

/// A shell record.
pub fn shell(feature: &str, body: &str, thickness: &str, remove_faces: Vec<Value>) -> Value {
    json!({
        "id": feature, "type": "shell", "body": body, "thickness": thickness,
        "remove_faces": remove_faces
    })
}

/// A simple hole record on the points of `sketch`.
pub fn hole(
    feature: &str,
    body: &str,
    sketch: &str,
    points: &[&str],
    diameter: &str,
    extent: Value,
) -> Value {
    json!({
        "id": feature, "type": "hole", "body": body, "sketch": sketch,
        "points": points, "diameter": diameter, "extent": extent
    })
}

/// A linear pattern record of `b-1` along an origin axis (op join, the default).
pub fn linear_pattern(feature: &str, axis: &str, count: &str, spacing: &str) -> Value {
    json!({
        "id": feature, "type": "linear_pattern", "body": "b-1",
        "direction": {"axis": {"kind": "origin", "axis": axis}, "count": count, "spacing": spacing}
    })
}

/// A mirror record of `b-1` across `plane` (op join, the default).
pub fn mirror(feature: &str, plane: Value) -> Value {
    json!({"id": feature, "type": "mirror", "body": "b-1", "plane": plane})
}

/// A boolean record.
pub fn boolean(feature: &str, target: &str, tools: &[&str], op: &str) -> Value {
    json!({"id": feature, "type": "boolean", "target": target, "tools": tools, "op": op})
}

/// A split body record of `body` by a datum plane into `target`.
pub fn split_by_datum(feature: &str, body: &str, datum: &str, target: &str) -> Value {
    json!({
        "id": feature, "type": "split_body", "body": body, "plane": datum_plane(datum),
        "target": target
    })
}

/// An import record of an asset.
pub fn import(feature: &str, body: &str, asset: &str, op: &str) -> Value {
    json!({"id": feature, "type": "import", "body": body, "asset": asset, "op": op})
}

// Shared setups of the corpus and their picked references.

/// CUBE20: `f-box1` box 20 x 20 x 20 in `b-1`.
pub fn cube20() -> Value {
    box_record("f-box1", "b-1", ["20", "20", "20"], None, "new")
}

/// CUBE20's front top edge.
pub fn cube20_front_top() -> Value {
    reference(
        "edge(f-box1/front|f-box1/top)",
        "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2",
    )
}

/// CUBE20's bottom front edge.
pub fn cube20_bottom_front() -> Value {
    reference(
        "edge(f-box1/bottom|f-box1/front)",
        "line|n=1,0,0|c=0,-10,0|l=20|adj=2,2",
    )
}

/// CUBE10: `f-box1` box 10 x 10 x 10 in `b-1`.
pub fn cube10() -> Value {
    box_record("f-box1", "b-1", ["10", "10", "10"], None, "new")
}

/// BOX-BELOW: `f-box1` box 40 x 40 x 10 at (0,0,-10) in `b-1` (top at z = 0).
pub fn box_below() -> Value {
    box_record(
        "f-box1",
        "b-1",
        ["40", "40", "10"],
        Some(["0", "0", "-10"]),
        "new",
    )
}

/// BLOCK's outline: `e-1` to `e-4`, the 80 x 50 rectangle.
pub fn block_outline() -> Vec<Value> {
    rectangle(1, 0.0, 0.0, 80.0, 50.0)
}

/// BLOCK's profile region, as stored when the block was made.
pub fn block_region() -> Value {
    rectangle_region(1, 0.0, 0.0, 80.0, 50.0)
}

/// BLOCK with this outline in `f-sk1` and this extrude distance: `f-sk1` on XY and `f-ext1`, an
/// extrude of the stored block region into `b-1`, op new.
pub fn block_with(outline: Vec<Value>, height: &str) -> Vec<Value> {
    vec![
        sketch("f-sk1", origin_plane("XY"), outline),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![block_region()],
            distance(height),
            "new",
        ),
    ]
}

/// BLOCK (naming.md test 81): x 0..80, y 0..50, z 0..30.
pub fn block() -> Vec<Value> {
    block_with(block_outline(), "30")
}

/// BLOCK's top.
pub fn block_top() -> Value {
    reference("f-ext1/end-cap", "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4")
}

/// BLOCK's right side.
pub fn block_right_side() -> Value {
    reference(
        "f-ext1/lateral/e-2",
        "plane|n=1,0,0|c=80,25,15|a=1500|adj=4,4",
    )
}

/// BLOCK's front top edge.
pub fn block_front_top() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-1)",
        "line|n=1,0,0|c=40,0,30|l=80|adj=2,2",
    )
}

/// BLOCK's back top edge.
pub fn block_back_top() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-3)",
        "line|n=1,0,0|c=40,50,30|l=80|adj=2,2",
    )
}

/// BLOCK's right top edge.
pub fn block_right_top() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-2)",
        "line|n=0,1,0|c=80,25,30|l=50|adj=2,2",
    )
}

/// BLOCK's front right vertical edge.
pub fn block_front_right_vertical() -> Value {
    reference(
        "edge(f-ext1/lateral/e-1|f-ext1/lateral/e-2)",
        "line|n=0,0,1|c=80,0,15|l=30|adj=2,2",
    )
}

/// A slot cut `depth` deep from the plane of datum `datum`, x from `x0` to `x1` across the whole
/// width of BLOCK: the sketch `sketch_feature` with the rectangle `e-5` to `e-8` and its reversed
/// extrude `extrude_feature`, op cut.
pub fn slot_cut(
    datum: &str,
    sketch_feature: &str,
    extrude_feature: &str,
    x0: f64,
    x1: f64,
    depth: &str,
) -> Vec<Value> {
    vec![
        sketch(
            sketch_feature,
            datum_plane(datum),
            rectangle(5, x0, -1.0, x1, 51.0),
        ),
        reversed(extrude(
            extrude_feature,
            "b-1",
            sketch_feature,
            vec![rectangle_region(5, x0, -1.0, x1, 51.0)],
            distance(depth),
            "cut",
        )),
    ]
}

/// SLOT(a, b) (naming.md test 84) on BLOCK: `f-dp1` datum XY offset 30, `f-sk2` and `f-ext2`
/// cutting 5 mm.
pub fn slot(x0: f64, x1: f64) -> Vec<Value> {
    let mut features = vec![datum_offset("f-dp1", "XY", "30")];
    features.extend(slot_cut("f-dp1", "f-sk2", "f-ext2", x0, x1, "5"));
    features
}

// Setups that more than one test file runs, R23 to R66, with their picked references.

/// R23: peg A's top rim.
pub fn peg_a_rim() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-5)",
        "circle|n=0,0,1|c=-5,0,30|l=25.132741|r=4|adj=2,1",
    )
}

/// R23 setup (PEGS): a 60 x 30 x 10 box, pegs A (`f-sk1`, `f-ext1`) and B (`f-sk2`, `f-ext2`) on
/// it, and a fillet `f-fil1` of 1 on peg A's top rim.
pub fn pegs() -> Vec<Value> {
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
        fillet("f-fil1", "b-1", vec![peg_a_rim()], "1"),
    ]
}

/// R24: the right rim of the slot `f-box2` cut into CUBE20's top.
pub fn slot_right_rim() -> Value {
    reference(
        "edge(f-box1/top#2|f-box2/right)",
        "line|n=0,1,0|c=-4,0,20|l=20|adj=2,2",
    )
}

/// R24 setup: CUBE20, the slot `f-box2` and a fillet `f-fil1` of 0.5 on the slot's right rim.
pub fn slotted_cube() -> Vec<Value> {
    vec![
        cube20(),
        box_record(
            "f-box2",
            "b-1",
            ["4", "30", "5"],
            Some(["-6", "0", "15"]),
            "cut",
        ),
        fillet("f-fil1", "b-1", vec![slot_right_rim()], "0.5"),
    ]
}

/// R25 and R37: the chamfer face of CUBE20's front top edge.
pub fn chamfer_face() -> Value {
    reference(
        "f-cha1/chamfer/edge(f-box1/front|f-box1/top)",
        "plane|n=0,-0.707107,0.707107|c=0,-9,19|a=56.568542|adj=4,4",
    )
}

/// R25 setup: CUBE20, a chamfer `f-cha1` of 2 on its front top edge, a datum `f-dp1` on the
/// chamfer face and a small boss `f-sk1`, `f-ext1` on the datum.
pub fn boss_on_chamfer_datum() -> Vec<Value> {
    vec![
        cube20(),
        chamfer("f-cha1", "b-1", vec![cube20_front_top()], "2"),
        datum_on_face("f-dp1", "b-1", chamfer_face(), "0"),
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
    ]
}

/// R28: the boss's top rim.
pub fn boss_top_rim() -> Value {
    reference(
        "edge(f-ext2/end-cap|f-ext2/lateral/e-5)",
        "circle|n=0,0,1|c=40,25,40|l=62.831853|r=10|adj=2,1",
    )
}

/// R28's variant reference: the ring where the boss meets BLOCK's top.
pub fn boss_base_ring() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext2/lateral/e-5)",
        "circle|n=0,0,1|c=40,25,30|l=62.831853|r=10|adj=2,1",
    )
}

/// R28 setup: BLOCK, a boss `f-dp1`, `f-sk2`, `f-ext2` joined on its top, and a fillet `f-fil1` of
/// 2 on `edge`.
pub fn filleted_boss(edge: Value) -> Vec<Value> {
    let mut features = block();
    features.extend([
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
        fillet("f-fil1", "b-1", vec![edge], "2"),
    ]);
    features
}

/// R30 (a): the top rim of the symmetric boss.
pub fn symmetric_boss_rim() -> Value {
    reference(
        "edge(f-ext2/end-cap|f-ext2/lateral/e-5)",
        "circle|n=0,0,1|c=40,25,50|l=62.831853|r=10|adj=2,1",
    )
}

/// R30 (a) setup: BLOCK, a boss `f-ext2` of `op` extruded symmetric 40 from z = 30, and a fillet
/// `f-fil1` of 2 on its top rim.
pub fn symmetric_boss(op: &str) -> Vec<Value> {
    let mut features = block();
    features.push(datum_offset("f-dp1", "XY", "30"));
    features.push(sketch(
        "f-sk2",
        datum_plane("f-dp1"),
        vec![circle("e-5", [40.0, 25.0], 10.0)],
    ));
    features.push(extrude(
        "f-ext2",
        "b-1",
        "f-sk2",
        vec![circle_region("e-5", [40.0, 25.0], 10.0)],
        symmetric("40"),
        op,
    ));
    features.push(fillet("f-fil1", "b-1", vec![symmetric_boss_rim()], "2"));
    features
}

/// R30 (b): the top front edge of Box 2.
pub fn box2_front_top_edge() -> Value {
    reference(
        "edge(f-box2/front|f-box2/top)",
        "line|n=1,0,0|c=0,-5,24.5|l=10|adj=2,2",
    )
}

/// R30 (b) setup: CUBE20 in `b-1`, a box in `b-2` reaching into its top, the boolean `f-bool1` of
/// `op` into `b-1`, and a fillet `f-fil1` of 1 on the box's top front edge.
pub fn boolean_boss(op: &str) -> Vec<Value> {
    vec![
        cube20(),
        box_record(
            "f-box2",
            "b-2",
            ["10", "10", "10"],
            Some(["0", "0", "14.5"]),
            "new",
        ),
        boolean("f-bool1", "b-1", &["b-2"], op),
        fillet("f-fil1", "b-1", vec![box2_front_top_edge()], "1"),
    ]
}

/// R31: the split body record `f-split1` of `b-1` by the datum `f-dp1` into `b-2`, flipped when
/// `flip`.
pub fn split(flip: bool) -> Value {
    let record = split_by_datum("f-split1", "b-1", "f-dp1", "b-2");
    if flip {
        with(record, "flip", json!(true))
    } else {
        record
    }
}

/// R31 (b): the top front edge of the upper part after the split.
pub fn upper_front_top_edge() -> Value {
    reference(
        "edge(f-box1/front#2|f-box1/top)",
        "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2",
    )
}

/// R31 (b) setup: CUBE20 split at z = 10 into `b-2` (flipped when `flip`) and a fillet `f-fil1` of
/// 2 on the top front edge of the part `b-1` keeps.
pub fn split_then_fillet(flip: bool) -> Vec<Value> {
    vec![
        cube20(),
        datum_offset("f-dp1", "XY", "10"),
        split(flip),
        fillet("f-fil1", "b-1", vec![upper_front_top_edge()], "2"),
    ]
}

/// R32 (b): BLOCK, extruded in reverse when `reverse` (its end cap is then the bottom at z = -30,
/// facing -Z), and a sketch `f-sk2` with `entities` on its end cap.
pub fn block_with_end_cap_sketch(reverse: bool, entities: Vec<Value>) -> Vec<Value> {
    let mut features = block();
    if reverse {
        features = edited(&features, "f-ext1", reversed);
    }
    features.push(sketch(
        "f-sk2",
        top_face_plane("b-1", block_top(), 30.0),
        entities,
    ));
    features
}

/// R32 (b) with these sketch entities and hole points: `block_with_end_cap_sketch` with `entities`
/// and a 5 mm hole `f-hole1` 10 deep at `points`.
pub fn block_with_holes_on_end_cap(
    reverse: bool,
    entities: Vec<Value>,
    points: &[&str],
) -> Vec<Value> {
    let mut features = block_with_end_cap_sketch(reverse, entities);
    features.push(hole("f-hole1", "b-1", "f-sk2", points, "5", distance("10")));
    features
}

/// R32 (b) setup (features/sketch.md test 24 when `reverse`): `block_with_holes_on_end_cap` with a
/// point `e-p1` at (20,10) and the hole there.
pub fn block_with_hole_on_end_cap(reverse: bool) -> Vec<Value> {
    block_with_holes_on_end_cap(reverse, vec![point("e-p1", [20.0, 10.0])], &["e-p1"])
}

/// R33: the top of the 20 x 20 x 40 box as picked when it was 10 tall.
pub fn tall_box_top() -> Value {
    reference("f-box1/top", "plane|n=0,0,1|c=0,0,10|a=400|adj=4,4")
}

/// R33 setup: a 20 x 20 x 40 box `f-box1`, these features inserted after it, then a sketch `f-sk2`
/// on its top (attached when it was 10 tall) with a circle `e-5` of radius 3 and its boss `f-ext2`
/// of 5, op join.
pub fn tall_box_with_boss(inserted: Vec<Value>) -> Vec<Value> {
    let mut features = vec![box_record("f-box1", "b-1", ["20", "20", "40"], None, "new")];
    features.extend(inserted);
    features.push(sketch(
        "f-sk2",
        top_face_plane("b-1", tall_box_top(), 10.0),
        vec![circle("e-5", [0.0, 0.0], 3.0)],
    ));
    features.push(extrude(
        "f-ext2",
        "b-1",
        "f-sk2",
        vec![circle_region("e-5", [0.0, 0.0], 3.0)],
        distance("5"),
        "join",
    ));
    features
}

/// R33's edit, inserted before the sketch: a datum `f-dp1` XY offset 35, a sketch `f-sk3` on it
/// with a square x -15..15, y -15..15 and its cut `f-ext3` of 10 ('Extrude 1'), which removes
/// z 35..40 of the box and with it the top.
pub fn top_cut() -> Vec<Value> {
    vec![
        datum_offset("f-dp1", "XY", "35"),
        sketch(
            "f-sk3",
            datum_plane("f-dp1"),
            rectangle(1, -15.0, -15.0, 15.0, 15.0),
        ),
        extrude(
            "f-ext3",
            "b-1",
            "f-sk3",
            vec![rectangle_region(1, -15.0, -15.0, 15.0, 15.0)],
            distance("10"),
            "cut",
        ),
    ]
}

/// R36 and R53 (a): the top of the 80 x 50 x 30 box.
pub fn tray_top() -> Value {
    reference("f-box1/top", "plane|n=0,0,1|c=0,0,30|a=4000|adj=4,4")
}

/// R39 (a): the front left vertical edge of the box in `b-2`, the axis of R59 (a)'s revolve.
pub fn box_corner_edge() -> Value {
    reference(
        "edge(f-box1/front|f-box1/left)",
        "line|n=0,0,1|c=-5,0,5|l=10|adj=2,2",
    )
}

/// R39 (a) setup (revolve.md test 16): a box in `b-2`, these features, and a full revolve `f-rev1`
/// into `b-1` about the box's front left edge.
pub fn revolve_about_box_edge(inserted: Vec<Value>) -> Vec<Value> {
    let mut features = vec![box_record(
        "f-box1",
        "b-2",
        ["10", "10", "10"],
        Some(["0", "5", "0"]),
        "new",
    )];
    features.extend(inserted);
    features.push(sketch(
        "f-sk1",
        origin_plane("XZ"),
        rectangle(1, 10.0, 0.0, 20.0, 30.0),
    ));
    features.push(revolve(
        "f-rev1",
        "b-1",
        "f-sk1",
        vec![rectangle_region(1, 10.0, 0.0, 20.0, 30.0)],
        edge_axis("b-2", box_corner_edge()),
        full_turn(),
        "new",
    ));
    features
}

/// R51: the outline of an 80 x 50 rectangle with radius 5 corners; `back_right_arc` false draws a
/// sharp back right corner instead of the arc `e-6`.
pub fn rounded_outline(back_right_arc: bool) -> Vec<Value> {
    let corner_end = if back_right_arc { 45.0 } else { 50.0 };
    let back_start = if back_right_arc { 75.0 } else { 80.0 };
    let mut entities = vec![
        line("e-1", [5.0, 0.0], [75.0, 0.0]),
        line("e-2", [80.0, 5.0], [80.0, corner_end]),
        line("e-3", [back_start, 50.0], [5.0, 50.0]),
        line("e-4", [0.0, 45.0], [0.0, 5.0]),
        arc("e-5", [75.0, 5.0], 5.0, 3.0 * FRAC_PI_2, 0.0),
    ];
    if back_right_arc {
        entities.push(arc("e-6", [75.0, 45.0], 5.0, 0.0, FRAC_PI_2));
    }
    entities.push(arc("e-7", [5.0, 45.0], 5.0, FRAC_PI_2, PI));
    entities.push(arc("e-8", [5.0, 5.0], 5.0, PI, 3.0 * FRAC_PI_2));
    entities
}

/// R51: the top edge of the back right corner arc.
pub fn arc_top_edge() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-6)",
        "circle|n=0,0,1|c=78.183099,48.183099,10|l=7.853982|r=5|adj=2,2",
    )
}

/// R51 setup: the rounded outline (with or without the arc `e-6`) extruded 10 and a fillet `f-fil1`
/// of 1 on the top edge of `e-6`.
pub fn rounded_plate(back_right_arc: bool) -> Vec<Value> {
    let area = 80.0 * 50.0 - (4.0 - PI) * 25.0;
    vec![
        sketch("f-sk1", origin_plane("XY"), rounded_outline(back_right_arc)),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![region(
                "e-1,e-2,e-3,e-4,e-5,e-6,e-7,e-8",
                [40.0, 25.0],
                area,
            )],
            distance("10"),
            "new",
        ),
        fillet("f-fil1", "b-1", vec![arc_top_edge()], "1"),
    ]
}

/// R57: the top edge of the revolve's end face.
pub fn revolve_end_edge() -> Value {
    reference(
        "edge(f-rev1/end-cap|f-rev1/lateral/e-3)",
        "line|n=0,1,0|c=0,-15,10|l=10|adj=2,2",
    )
}

/// R57 setup: a 10 x 10 section revolved by `extent` about Z, a chamfer `f-cha1` of 1 on the top
/// edge of its end face, a sketch `f-sk2` on its start face and a small cut `f-ext2` from it.
pub fn three_quarter_ring(extent: Value) -> Vec<Value> {
    vec![
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
            extent,
            "new",
        ),
        chamfer("f-cha1", "b-1", vec![revolve_end_edge()], "1"),
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
        reversed(extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![circle_region("e-11", [0.0, 0.0], 2.0)],
            distance("3"),
            "cut",
        )),
    ]
}

/// R66: BLOCK's outline with `e-3` replaced by the five lines `e-9` to `e-13` of a 20 x 10 notch.
pub fn notched_back_outline() -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-9", [80.0, 50.0], [50.0, 50.0]),
        line("e-10", [50.0, 50.0], [50.0, 40.0]),
        line("e-11", [50.0, 40.0], [30.0, 40.0]),
        line("e-12", [30.0, 40.0], [30.0, 50.0]),
        line("e-13", [30.0, 50.0], [0.0, 50.0]),
    ]
}

// Editing feature lists.

/// The record `feature` of `features`.
fn record_in(features: &[Value], feature: &str) -> Value {
    features
        .iter()
        .find(|record| record["id"].as_str() == Some(feature))
        .unwrap_or_else(|| panic!("no feature {feature}"))
        .clone()
}

/// The features without the records `removed`.
pub fn without(features: &[Value], removed: &[&str]) -> Vec<Value> {
    features
        .iter()
        .filter(|record| {
            !removed
                .iter()
                .any(|feature| record["id"].as_str() == Some(feature))
        })
        .cloned()
        .collect()
}

/// The features with `inserted` placed directly before the record `before`.
pub fn insert_before(features: &[Value], before: &str, inserted: Vec<Value>) -> Vec<Value> {
    let position = features
        .iter()
        .position(|record| record["id"].as_str() == Some(before))
        .unwrap_or_else(|| panic!("no feature {before}"));
    let mut result = features[..position].to_vec();
    result.extend(inserted);
    result.extend_from_slice(&features[position..]);
    result
}

/// The features with the records `moved` taken out and placed, in their order, directly before
/// the record `before` (a timeline drag).
pub fn move_before(features: &[Value], moved: &[&str], before: &str) -> Vec<Value> {
    let taken = moved
        .iter()
        .map(|feature| record_in(features, feature))
        .collect();
    insert_before(&without(features, moved), before, taken)
}

/// The features with the records `moved` taken out and appended at the end, in their order.
pub fn move_to_end(features: &[Value], moved: &[&str]) -> Vec<Value> {
    let mut result = without(features, moved);
    result.extend(moved.iter().map(|feature| record_in(features, feature)));
    result
}

/// The features with the record `feature` replaced by `change(record)`.
pub fn edited(
    features: &[Value],
    feature: &str,
    change: impl FnOnce(Value) -> Value,
) -> Vec<Value> {
    let mut change = Some(change);
    features
        .iter()
        .map(|record| match change.take() {
            Some(apply) if record["id"].as_str() == Some(feature) => apply(record.clone()),
            other => {
                change = other;
                record.clone()
            }
        })
        .collect()
}
