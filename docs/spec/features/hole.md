# Feature: hole

Status: M1 spec. Type name `hole`. Shared contracts: [features.md](../features.md). Naming rule: [naming.md](../naming.md) section 3.6.8.

## 1. Purpose and scope

Hole cuts round holes into a body at points of a sketch, drilled along the sketch normal into the material.

### In M1

- Placement at one or more sketch points: point entities, line and arc ends, arc and circle centers.
- Types: simple, counterbore, countersink.
- Extent: a depth (to the shoulder of the hole) or through all.
- Flat bottom or a drill-point tip angle.
- `reverse` to drill along the sketch normal instead of against it.

### Later

- Tapped and threaded holes, ISO and UNC size tables.
- Print presets: teardrop and flat-top horizontal holes, sacrificial bridging layers, heat-set insert pockets.
- Placement on a face by click without a sketch. The UI already places holes by clicking a flat face: it writes a helper sketch on that face with one point entity per click and sets `sketch` to the helper and `points` to those points ([ui.md](../ui.md) section 3.7, "Placing holes on a face"), so the feature itself needs no change.
- Holes on curved faces (drilled along the face normal).
- Per-hole depths and types in one feature.
- Clearance, close and loose fit size presets.

## 2. Public API

```rust
/// Cuts round holes at sketch points.
pub struct Hole;

impl Feature for Hole {
    const TYPE: &'static str = "hole";
    type Params = HoleParams;
}

/// Params of a hole feature.
pub struct HoleParams {
    /// The body to cut.
    pub body: Id,
    /// The sketch whose points place the holes; its plane sets the drilling direction.
    pub sketch: Id,
    /// Sketch points, at least one, as point refs (`e-p1`, `e-1.start`, `e-5.center`).
    pub points: Vec<PointRef>,
    /// Hole diameter. Length, greater than 0.
    pub diameter: Expr,
    /// Simple, counterbore or countersink. Default simple, omitted when simple.
    pub hole_type: HoleType,
    /// Depth or through all.
    pub extent: HoleExtent,
    /// Included angle of a drill-point tip. Angle, greater than 0 and less than 180 deg. None (omitted) is a flat bottom.
    pub tip_angle: Option<Expr>,
    /// Drill along the sketch normal instead of against it. Default false, omitted when false.
    pub reverse: bool,
}

/// The head of a hole.
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HoleType {
    /// A plain cylinder.
    Simple,
    /// A wider cylinder of `diameter` and `depth` at the top. Lengths.
    Counterbore { diameter: Expr, depth: Expr },
    /// A cone of top `diameter` and included `angle` at the top. Length and angle.
    Countersink { diameter: Expr, angle: Expr },
}

/// How deep a hole goes, from the sketch plane.
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HoleExtent {
    /// To the shoulder at depth `expr` (the tip, if any, goes beyond). Length, greater than 0.
    Distance { expr: Expr },
    /// Through the whole body.
    ThroughAll,
}
```

`PointRef` is sketch.md's point reference string.

JSON:

```json
{"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk1", "points": ["e-p1", "e-p2"],
 "diameter": "3.4 mm", "hole_type": {"kind": "counterbore", "diameter": "6.5 mm", "depth": "3.5 mm"},
 "extent": {"kind": "through_all"}}
```

Other forms: `"hole_type": {"kind": "countersink", "diameter": "6.5 mm", "angle": "90 deg"}`; `"extent": {"kind": "distance", "expr": "8 mm"}, "tip_angle": "118 deg"`.

### Manifest

Label `Hole`, category `create`, icon `hole`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `points` | Points | yes | | `sketch_point` | |
| `diameter` | Diameter | yes | length | | |
| `hole_type.kind` | Type | yes | | | `simple` Simple, `counterbore` Counterbore, `countersink` Countersink |
| `hole_type.diameter` | Head diameter | yes | length | | |
| `hole_type.depth` | Counterbore depth | yes | length | | |
| `hole_type.angle` | Countersink angle | yes | angle | | |
| `extent.kind` | Extent | yes | | | `distance` Depth, `through_all` Through all |
| `extent.expr` | Depth | yes | length | | |
| `tip_angle` | Tip angle | no | angle | | |
| `reverse` | Reverse | no | | | |
| `sketch` | Sketch | no | | `sketch` | |
| `body` | Body | no | | `body` | |

### Python

Dedicated form: `hole(points, diameter, depth=None, counterbore=..., countersink=..., tip_angle=..., reverse=...)` (python-api.md section 2.7), produced by the `Feature::python` hook with the shared rules of features.md section 3.11. The `with <part>:` block gives `body`, and Python derives `sketch` from the points:

- `points`: each point ref as a sketch point (`point_1`, `line_1.start`, `circle_1.center`), one point alone, several as a list. The hook never uses the `Sketch` shorthand.
- `diameter`: `diameter`. `depth`: `extent.expr` for a `distance` extent; omitted for `through_all`.
- `counterbore=(diameter, depth)` or `countersink=(diameter, angle)` from `hole_type`; nothing for `simple`.
- `tip_angle=<expr>` when set; `reverse=True` when true.

With `e-p1` and `e-p2` as `point_1` and `point_2`, the JSON above is `hole([point_1, point_2], 3.4*mm, counterbore=(6.5*mm, 3.5*mm), id="f-hole1")`. Generic form:

```python
feature(
    "hole",
    id="f-hole1",
    body="b-1",
    sketch="f-sk1",
    points=["e-p1", "e-p2"],
    diameter="3.4 mm",
    hole_type={"kind": "counterbore", "diameter": "6.5 mm", "depth": "3.5 mm"},
    extent={"kind": "through_all"},
)
```

## 3. Behavior

### 3.1 Validation

- `points` is not empty, else `EmptySelection { field: "points", noun: point }`.

### 3.2 Dependencies

- `features`: `[sketch]`. `bodies_written`: `[body]`.
- `expressions`, in this order: `("diameter", Length)`; counterbore `("hole_type.diameter", Length)`, `("hole_type.depth", Length)`; countersink `("hole_type.diameter", Length)`, `("hole_type.angle", Angle)`; distance `("extent.expr", Length)`; `("tip_angle", Angle)` when set.

### 3.3 Build

1. The body must hold a solid, else `BodyEmpty { field: body, body, use_new: false }` (recompute's diagnosis adds why it is empty, features.md section 4.1).
2. Values, checked in this order (D hole diameter, L depth), each length by the positive length rule of features.md section 3.4:
   - D > 0 (`GreaterThan(0)`): the bore radius D / 2 at least `LINEAR_TOLERANCE`, since the entry and bottom segments of the cross-section are that long.
   - Distance extent: L > 0 (`GreaterThan(0)` on `extent.expr`), at least `LINEAR_TOLERANCE`. It comes before the head checks, so a head check never compares with a depth that is not positive and its message always names a value that can work.
   - Counterbore: head diameter > D (`GreaterThan(D)`, bound name `the hole diameter`), at least D + 2 `LINEAR_TOLERANCE` so the floor between the head and the bore radius is at least `LINEAR_TOLERANCE` wide; depth > 0 (`GreaterThan(0)`), at least `LINEAR_TOLERANCE`; with a distance extent, depth ≤ L - `LINEAR_TOLERANCE`, else `OutOfRange` on `hole_type.depth` with `LessThan(L)` and bound name `the hole depth`.
   - Countersink: head diameter > D, at least D + 2 `LINEAR_TOLERANCE` (as above); 0 < angle < 180 deg (`GreaterThan(0)`, `LessThan(π)`); with a distance extent, the countersink depth (head - D) / 2 / tan(angle / 2) must be at most L - `LINEAR_TOLERANCE`, else `OutOfRange` on `hole_type.diameter` with `LessThan(D + 2 L tan(angle / 2))` and bound name `the largest that fits the hole depth`.
   - Tip angle, when set: 0 < tip < 180 deg. With a through-all extent the tip is ignored with an `Ignored { field: "tip_angle", reason: TipAngleThroughAll }` warning.

   Every length (D, L, the head diameter and the counterbore depth) is then checked against the modeling limit (`AtMost(MODEL_LIMIT)`, features.md section 3.4), after its own bounds.

   A head must end at least `LINEAR_TOLERANCE` (1e-6 mm) above the shoulder, because a shorter bore between them is below the kernel's modeling tolerance and the kernel would fail on it (kernel.md section 3.4.2). The messages keep the bounds L and D + 2 L tan(angle / 2), which come from the depth the user typed.
3. Direction d: the negated sketch normal (into the material when the sketch sits on an outward face); the sketch normal when `reverse` is true.
4. For each point, in `points` order: look it up with `SketchResult::point` and map it to world p. A point that the sketch no longer has is skipped with a `PointMissing { field: points[i], position: i, sketch, point }` warning, whose fixes are `RemoveEntry { pointer: "/points/i" }` and `Pick { pointer: "/points/i" }`, so one deleted point never fails the other holes (corpus R43b); when no point is left, the feature fails with `UnknownEntity { field: "points[0]", sketch, entity, noun: point }` for the first point. Build its tool by revolving a half cross-section about the line through p along d (kernel `revolve`, naming.md section 3.6.8), with curves tagged by role. The half cross-section lies in the plane through p spanned by d and the sketch placement's x_axis, on the side of +x_axis, so the seam placement is deterministic. Its curves:
   - `entry`: the segment closing the tool on the sketch plane, at the radius of the widest part.
   - Simple: `bore` at radius D/2 from 0 to L.
   - Counterbore: `counterbore` at the head radius from 0 to the counterbore depth, `counterbore-floor` stepping in to D/2, then `bore` to L.
   - Countersink: `countersink` from the head radius at 0 narrowing at the included angle to D/2, then `bore` to L.
   - Bottom: `bottom` (flat) at L; or `tip`, a cone of included angle `tip_angle` with its apex at L + (D / 2) / tan(tip / 2).
   - Through all: with F that point's largest signed distance (c - p) · d over the corners c of the body's tight bounding box, and h the head depth (the counterbore depth, the countersink depth of step 2, or 0 for a simple hole), L = max(F, h) + 1 mm. If F ≤ 0 the point gets no tool. Taking the larger of F and h keeps the bore at least 1 mm long below the head, so a head deeper than the body (a counterbore 4 mm deep in a 2 mm plate) still gives a valid cross-section: without it, the bore would run back up from the counterbore floor. The tool always reaches at least 1 mm past the body and past its head.
5. Decide which tools overlap the body. The body is cut with every tool whose tight bounding box meets the body's, in point order, with one kernel `cut` call (a tool whose box misses the body's does not overlap it). With V the body's volume:
   - If the cut changes the volume by at most `VOLUME_TOLERANCE` (1e-9) × V (the body rules give `NoEffect`, features.md section 3.5), no tool overlaps.
   - Otherwise a tool overlaps when a face of the result descends from one of the tool's faces and from no face of the body (kernel history: the face is in the `modified` list of a tool face and of no body face). Such a face is part of the tool's boundary inside the body, so the tool removes material there.
   - A tool without such a face misses the body, only touches its surface, or swallows whole solids of it. It overlaps when it shares more than `VOLUME_TOLERANCE` × V with the body, measured with the kernel `common` of the body and the tool; several such tools whose boxes are more than `LINEAR_TOLERANCE` apart share one `common` with their compound, each solid of the result counting for the tool whose box is nearest to its center.

   A point whose tool does not overlap the body, or that got no tool, adds a `HoleMissesBody { sketch, point, position, body, reverse_hits }` warning and is left out, with `position` the point's sketch coordinates (`SketchResult::point`), so the message names the hole by where it is (`The hole at (95, 25) in 'Sketch 2'`). On this warning path only, the point's tool is built the other way round (direction `-d`) and `reverse_hits` is true when it shares more than `VOLUME_TOLERANCE` × V with the body (one kernel `common`, cached with the result); only then is the fix `SetField` of `reverse` to the other value offered, and otherwise the message adds ` Move the point onto the body.` (features.md section 4.2). Points with identical refs are used once.
   - **Ends outside the body.** A point whose tool overlaps the body but whose `bottom` or `tip` face is not in the result (the hole's end lies outside the body, as for every through-all hole) records `AbsentFace { name: <id>/bottom/<point> or <id>/tip/<point>, why: Through }` in `FeatureOutput::absent`, so a reference to that face is explained as `is no longer made by 'Hole 1', which goes all the way through the body` (features.md section 2.7).
6. If every cut tool overlaps, that cut is the result, with the body rules applied as `apply_op` does (`apply_op_result`, with `direction` `Direction { direction: d, sketch: Some(sketch) }`). Otherwise the body is cut again with only the overlapping tools (`apply_op(ctx, body, Cut, tools, Some(direction))`, one kernel `cut` call), so a tool that does not overlap never shapes the result. If no tool remains, the body is unchanged with a `NoEffect { how: Missed }` warning (`This hole cuts nothing from body 'Body 1': every hole misses it.`, features.md section 3.15). Change `Cut`.
7. Naming: the hole's new faces take the motion of the point sketch (`SketchResult::motion`, features.md section 3.9), so a face sketch on a hole's face moves with the part like the hole does.

### 3.4 Element names

`<point>` is the point ref string as written in `points`.

| Name | Face |
| --- | --- |
| `<id>/bore/<point>` | The cylindrical wall of diameter D. |
| `<id>/bottom/<point>` | The flat bottom of a blind hole without a tip. |
| `<id>/tip/<point>` | The conical bottom of a blind hole with a tip. |
| `<id>/counterbore/<point>` | The counterbore's cylindrical wall. |
| `<id>/counterbore-floor/<point>` | The flat annular shoulder of a counterbore. |
| `<id>/countersink/<point>` | The countersink cone. |
| `<id>/entry/<point>` | The disc closing the tool on the sketch plane; deleted by the cut when the sketch plane lies on or outside the body surface. |

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| No points | `EmptySelection` | `Select at least one point for Points.` |
| A point the sketch no longer has | warning `PointMissing` | `This hole no longer makes the hole at a point that was deleted from 'Sketch 1', so it was skipped.` |
| No point left | `UnknownEntity` | `'Sketch 1' no longer has the point picked for Points. Pick a point of 'Sketch 1' again.` |
| Diameter below 2e-6 mm (bore radius below 1e-6 mm, 0 and negative included) | `OutOfRange` | `Diameter must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Depth or counterbore depth below 1e-6 mm (0 and negative included) | `OutOfRange` | `Depth must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Head diameter not at least 2e-6 mm larger than the hole diameter | `OutOfRange` | `Head diameter must be greater than the hole diameter (6 mm), but it is 5 mm. Use a value greater than 6 mm.` |
| Counterbore not at least 1e-6 mm shallower than the hole | `OutOfRange` | `Counterbore depth must be less than the hole depth (5 mm), but it is 6 mm. Use a value less than 5 mm.` |
| Countersink angle out of range | `OutOfRange` | `Countersink angle must be less than 180 deg, but it is 180 deg. Use a value less than 180 deg.` |
| Countersink not at least 1e-6 mm shallower than the hole | `OutOfRange` | `Head diameter must be less than the largest that fits the hole depth (16 mm), but it is 20 mm. Use a value less than 16 mm.` |
| Expression or dimension error | `Expression` | `Diameter: Expected a length, but this expression gives an angle.` |
| A length above the modeling limit | `OutOfRange` | `Diameter must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less.` |
| Body empty | `BodyEmpty` | without a cause: `Body 'Body 1' has no solid at this point, so this hole has nothing to work on. Pick a body that has a solid.`; with one, features.md section 4.1 |
| Kernel failure | `Kernel` | `Partsmith's geometry kernel could not build this hole. {hint}` (features.md section 4.1) |
| Point misses the body | warning `HoleMissesBody` | reversing reaches the body: `The hole at (0, 0) in 'Sketch 1' does not touch body 'Body 1'.` with the fix Turn on Reverse; otherwise `The hole at (100, 0) in 'Sketch 1' does not touch body 'Body 1'. Move the point onto the body.` |
| Tip with through all | warning `Ignored { TipAngleThroughAll }` | `Tip angle is ignored because the hole goes through all.` |
| Every point misses | warning `NoEffect` (`Missed`) | `This hole cuts nothing from body 'Body 1': every hole misses it.` |
| Removes everything | `EmptyResult` | features.md section 4.1 |

## 5. Performance

- One hole in a box: 15 ms. Ten holes in a 200-face body: 80 ms (test 33; over budget in the kernel's cut, features.md open issue 5).

## 6. Acceptance tests

Conventions from features.md section 6.1. Fixtures:

- `BOX-BELOW`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "position": ["0", "0", "-10"], "op": "new"}` (spans [-20,-20,-10]-[20,20,0], volume 16000, top face at z=0).
- `SK-PTS`: `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-p1", "type": "point", "position": [0, 0]}, {"id": "e-p2", "type": "point", "position": [10, 10]}, {"id": "e-p3", "type": "point", "position": [100, 0]}]}`.
- `HOLE(points, extra)`: `{"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk1", "points": <points>, "diameter": "6", <extra>}`.

Bodies `[b-1]`; features `BOX-BELOW`, `SK-PTS`, then the hole, unless stated. The sketch normal is +Z, so holes go down into the box.

1. **Simple through.** `HOLE(["e-p1"], "extent": {"kind": "through_all"})`. Expected: ok; volume 15717.256661, 7 faces, bbox [-20,-20,-10]-[20,20,0]; `f-hole1/bore/e-p1` cylindrical with radius 3 and area 188.495559; no `f-hole1/entry/e-p1`; change `Cut`.
2. **Blind, flat.** `"extent": {"kind": "distance", "expr": "5"}`. Expected: volume 15858.628331, 8 faces; `f-hole1/bore/e-p1` area 94.247780; `f-hole1/bottom/e-p1` planar at z=-5 with area 28.274334.
3. **Blind with a tip.** As 2 plus `"tip_angle": "118 deg"`. Expected: volume 15841.639397, 8 faces; `f-hole1/tip/e-p1` conical; no `bottom`; the vertex `vertex(f-hole1/tip/e-p1)` (the tip apex) is at (0, 0, -6.802582).
4. **Counterbore.** `"hole_type": {"kind": "counterbore", "diameter": "11", "depth": "4"}, "extent": {"kind": "through_all"}`. Expected: volume 15450.221286, 9 faces; `f-hole1/counterbore/e-p1` area 138.230077; `f-hole1/counterbore-floor/e-p1` planar at z=-4 with area 66.758844; `f-hole1/bore/e-p1` area 113.097336.
5. **Countersink.** `"hole_type": {"kind": "countersink", "diameter": "12", "angle": "90 deg"}, "extent": {"kind": "through_all"}`. Expected: volume 15604.159326, 8 faces; `f-hole1/countersink/e-p1` conical; `f-hole1/bore/e-p1` area 131.946891.
6. **Two points.** `HOLE(["e-p1", "e-p2"], "extent": {"kind": "through_all"})`. Expected: volume 15434.513322, 8 faces; walls `f-hole1/bore/e-p1` and `f-hole1/bore/e-p2`.
7. **One point off the body.** `HOLE(["e-p1", "e-p3"], "extent": {"kind": "through_all"})`. Expected: warning `hole_misses_body` rendered `The hole at (100, 0) in 'Sketch 1' does not touch body 'Body 1'. Move the point onto the body.` with no Reverse fix (reversed, it misses too); volume 15717.256661. Reversed variant, with the body on the other side of the sketch plane: features `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "op": "new"}` (z 0..10, above the sketch plane), `SK-PTS`, `HOLE(["e-p1", "e-p3"], "extent": {"kind": "through_all"})`. Expected: status warning (`hole_misses_body` twice, `no_effect`); the warning of e-p1 rendered `The hole at (0, 0) in 'Sketch 1' does not touch body 'Body 1'.` with the fix `SetField { field: reverse, value: true }` (Turn on Reverse), the one of e-p3 rendered `The hole at (100, 0) in 'Sketch 1' does not touch body 'Body 1'. Move the point onto the body.` with no Reverse fix; b-1 unchanged. Then (Turn on Reverse): status warning (`hole_misses_body` of e-p3 only); volume 15717.256661.
8. **Every point off the body.** `HOLE(["e-p3"], "extent": {"kind": "through_all"})`. Expected: status warning (`hole_misses_body`, `no_effect`), the `no_effect` rendered `This hole cuts nothing from body 'Body 1': every hole misses it.`; b-1 unchanged.
9. **Reverse drills away.** `HOLE(["e-p1"], "extent": {"kind": "distance", "expr": "5"}, "reverse": true)`. Expected: status warning (`hole_misses_body`, `no_effect`); the `hole_misses_body` rendered `The hole at (0, 0) in 'Sketch 1' does not touch body 'Body 1'.` with the fix `SetField { field: reverse, value: false }` (Turn off Reverse), verified: drilled the other way, the hole reaches the body; b-1 unchanged.
10. **Head diameter too small.** As 4 with head diameter `"5"`. Expected: error `out_of_range`, field `hole_type.diameter`, rendered `Head diameter must be greater than the hole diameter (6 mm), but it is 5 mm. Use a value greater than 6 mm.`
11. **Counterbore deeper than the hole.** `"hole_type": {"kind": "counterbore", "diameter": "11", "depth": "6"}, "extent": {"kind": "distance", "expr": "5"}`. Expected: error `out_of_range`, field `hole_type.depth`, rendered `Counterbore depth must be less than the hole depth (5 mm), but it is 6 mm. Use a value less than 5 mm.`
12. **Countersink angle 180.** As 5 with angle `"180 deg"`. Expected: error `out_of_range`, field `hole_type.angle`, rendered `Countersink angle must be less than 180 deg, but it is 180 deg. Use a value less than 180 deg.`
13. **Countersink deeper than the hole.** `"hole_type": {"kind": "countersink", "diameter": "20", "angle": "90 deg"}, "extent": {"kind": "distance", "expr": "5"}`. Expected: error `out_of_range`, field `hole_type.diameter`, rendered `Head diameter must be less than the largest that fits the hole depth (16 mm), but it is 20 mm. Use a value less than 16 mm.`
14. **Tip with through all.** As 1 plus `"tip_angle": "118 deg"`. Expected: warning `ignored` rendered `Tip angle is ignored because the hole goes through all.`; geometry as 1.
15. **Unknown point.** `HOLE(["e-p9"], "extent": {"kind": "through_all"})`. Expected: error `unknown_entity`, field `points[0]`, rendered `'Sketch 1' no longer has the point picked for Points. Pick a point of 'Sketch 1' again.` (no point is left, so the hole fails); fix `Pick`.
16. **Line end as a point.** Sketch f-sk1 with `{"id": "e-1", "type": "line", "start": [0, 0], "end": [10, 0]}`; `HOLE(["e-1.start"], "extent": {"kind": "through_all"})`. Expected: geometry as 1; wall named `f-hole1/bore/e-1.start`.
17. **Circle center as a point.** Sketch f-sk1 with `{"id": "e-5", "type": "circle", "center": [0, 0], "radius": 3}`; `HOLE(["e-5.center"], "extent": {"kind": "through_all"})`. Expected: geometry as 1; wall named `f-hole1/bore/e-5.center`.
18. **Zero diameter.** As 1 with `"diameter": "0"`. Expected: error `out_of_range`, field `diameter`.
19. **No points.** `HOLE([], "extent": {"kind": "through_all"})`. Expected: error `empty_selection` rendered `Select at least one point for Points.`
20. **Sketch on the body's face.** `SK-PTS` with plane `{"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,0|a=1600"}, "fallback": {"origin": [0, 0, 0], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}}`; hole as 1. Expected: geometry as 1 (the face placement has normal +Z and origin (0,0,0)).
21. **Performance.** Test 6 builds in under 25 ms (desktop, release).
22. **Zero depth with a counterbore.** `"hole_type": {"kind": "counterbore", "diameter": "11", "depth": "4"}, "extent": {"kind": "distance", "expr": "0"}`. Expected: error `out_of_range`, field `extent.expr`, rendered `Depth must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.`; b-1 unchanged.
23. **Negative depth with a countersink.** `"hole_type": {"kind": "countersink", "diameter": "12", "angle": "90 deg"}, "extent": {"kind": "distance", "expr": "-2"}`. Expected: error `out_of_range`, field `extent.expr`, rendered `Depth must be greater than 0 mm, but it is -2 mm. Use a value greater than 0 mm.`
24. **Through all with a head deeper than the body.** Features `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "2"], "position": ["0", "0", "-2"], "op": "new"}` (a plate [-20,-20,-2]-[20,20,0]), `SK-PTS`, then as 4 (counterbore 11 x 4, through all). Expected: ok; volume 3009.933644, 7 faces, bbox [-20,-20,-2]-[20,20,0]; `f-hole1/counterbore/e-p1` cylindrical with radius 5.5 and area 69.115038; no `f-hole1/bore/e-p1` and no `f-hole1/counterbore-floor/e-p1`; change `Cut`.
25. **Counterbore ending at the shoulder within the tolerance.** `"hole_type": {"kind": "counterbore", "diameter": "11", "depth": "4.9999999"}, "extent": {"kind": "distance", "expr": "5"}`. Expected: error `out_of_range`, field `hole_type.depth`, rendered `Counterbore depth must be less than the hole depth (5 mm), but it is 4.9999999 mm. Use a value less than 5 mm.` (the value is written with the digits that tell it apart from the bound, features.md section 3.12).
26. **Countersink ending at the shoulder within the tolerance.** `"hole_type": {"kind": "countersink", "diameter": "15.9999999", "angle": "90 deg"}, "extent": {"kind": "distance", "expr": "5"}` (countersink depth 4.99999995). Expected: error `out_of_range`, field `hole_type.diameter`, rendered `Head diameter must be less than the largest that fits the hole depth (16 mm), but it is 15.9999999 mm. Use a value less than 16 mm.`
27. **Diameter below the tolerance.** As 1 with `"diameter": "0.0000001"`. Expected: error `out_of_range`, field `diameter`, rendered `Diameter must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm.`; b-1 unchanged.
28. **Bore radius below the tolerance.** As 1 with `"diameter": "0.0000015"` (radius 7.5e-7 mm). Expected: error `out_of_range`, field `diameter`, bound `GreaterThan(0)`; b-1 unchanged.
29. **Depth below the tolerance.** `"extent": {"kind": "distance", "expr": "0.0000001"}`. Expected: error `out_of_range`, field `extent.expr`, rendered `Depth must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm.`; b-1 unchanged.
30. **Counterbore depth below the tolerance.** `"hole_type": {"kind": "counterbore", "diameter": "11", "depth": "0.0000001"}`, through all. Expected: error `out_of_range`, field `hole_type.depth`, rendered `Counterbore depth must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm.`; b-1 unchanged.
31. **Head within the tolerance of the hole diameter.** As 4 with head diameter `"6.0000001"`. Expected: error `out_of_range`, field `hole_type.diameter`, rendered `Head diameter must be greater than the hole diameter (6 mm), but it is 6.0000001 mm. Use a value greater than 6 mm.`; b-1 unchanged.
32. **Tools decided by the cut and by their shared volume.** Features `BOX-BELOW`; `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["2", "2", "2"], "position": ["30", "0", "-6"], "op": "join"}` (a separate cube [29,-1,-6]-[31,1,-4], so b-1 holds 2 solids and spans [-20,-20,-10]-[31,20,0], volume 16008); `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-p1", "type": "point", "position": [0, 0]}, {"id": "e-p4", "type": "point", "position": [30, 0]}, {"id": "e-p5", "type": "point", "position": [25, 15]}]}`; `HOLE(["e-p1", "e-p4", "e-p5"], "extent": {"kind": "through_all"})`. The box of every tool meets the body's: e-p1 drills the box, e-p4 swallows the small cube without leaving a face, and e-p5 misses both solids. Expected: warning `hole_misses_body` rendered `The hole at (25, 15) in 'Sketch 1' does not touch body 'Body 1'. Move the point onto the body.` (reversed, it misses too); volume 15717.256661, 7 faces, 1 solid, bbox [-20,-20,-10]-[20,20,0]; `f-hole1/bore/e-p1` exists; no face named `f-hole1/bore/e-p4` or `f-hole1/bore/e-p5`, and none whose name starts with `f-box2/`.
33. **Ten holes in a 200-face body.** Features `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["200", "200", "10"], "position": ["0", "0", "-10"], "op": "new"}` (top face at z=0); a sketch `f-sk1` on origin XY with 194 points `e-p<i + 1>` at (-90 + 13.5 (i mod 14), -90 + 13.5 floor(i / 14)) for i from 0 to 193; `{"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk1", "points": [<the 194 points>], "diameter": "8", "extent": {"kind": "through_all"}}` (b-1 then has 200 faces); a sketch `f-sk2` on origin XY with 10 points `e-p<j + 1>` at (-83.25 + 13.5 j, -83.25) for j from 0 to 9; `{"id": "f-hole2", "type": "hole", "body": "b-1", "sketch": "f-sk2", "points": [<the 10 points>], "diameter": "6", "extent": {"kind": "through_all"}}`. Expected: every feature ok; b-1 has 210 faces; f-hole2 builds in under 80 ms (desktop, release).
34. **A deleted point is skipped (corpus R43b).** `HOLE(["e-p1", "e-p9"], "extent": {"kind": "through_all"})` (`SK-PTS` has no `e-p9`). Expected: status warning (`point_missing`) at `/points/1`, rendered `This hole no longer makes the hole at a point that was deleted from 'Sketch 1', so it was skipped.`, fixes `RemoveEntry { pointer: "/points/1" }` (Remove from Points) then `Pick`; one hole at e-p1: volume 15717.256661, `f-hole1/bore/e-p1` exists. Then (`RemoveEntry`): points `["e-p1"]`, status ok, the same volume.
35. **Hole beside the body (newcomer error corpus NC12).** Features `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": RECT(0, 0, 80, 50)}`, `{"id": "f-ext1", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]}, "extent": {"kind": "distance", "expr": "30"}, "op": "new"}` (x 0..80, y 0..50, z 0..30), `{"id": "f-sk2", "type": "sketch", "plane": {"kind": "face", "body": "b-1", "face": {"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}, "fallback": {"origin": [0, 0, 30], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}, "fallback_frame": "body"}, "entities": [{"id": "e-p1", "type": "point", "position": [95, 25]}]}`, `{"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk2", "points": ["e-p1"], "diameter": "5", "extent": {"kind": "through_all"}}`. Expected: status warning (`hole_misses_body`, `no_effect`), the `hole_misses_body` rendered `The hole at (95, 25) in 'Sketch 2' does not touch body 'Body 1'. Move the point onto the body.` with no Reverse fix (reversed, it misses too); b-1 volume 120000.
36. **The end that lies outside the body.** Test 1. Expected: `FeatureOutput::absent` is `[{name: "f-hole1/bottom/e-p1", why: Through}]`. Test 2 (a blind hole whose bottom is in the result): `absent` is empty.
37. **The hole's faces move with the part.** The document of [features/sketch.md](sketch.md) test 20 (a hole on a face sketch with `fallback_frame: body` after a Move body of (100, 0, 0)). Expected: in the body state after f-hole1, `BodyState::motion` of `f-hole1/bore/e-p1` is the translation (100, 0, 0), the motion of f-sk2.

## Open issues

1. Depth is measured to the shoulder (the end of the full-diameter cylinder), with any tip beyond it, as Fusion does by default.
2. **Depth checked before the head.** Resolved: section 3.3 step 2 checked the head against L before L > 0, so a head on a hole whose depth is not positive got a fix that cannot be followed (a counterbore 11 x 4 with depth `0` rendered `Use a value less than 0 mm.`). Step 2 now checks L > 0 right after D > 0 and reports the depth (tests 22 and 23).
3. **Through all past the head.** Resolved: a through-all length of the farthest corner distance plus 1 mm can be less than the head depth (a counterbore 4 mm deep in a 2 mm plate gives 3 mm), which makes the bore run back up from the counterbore floor and the cross-section invalid. Section 3.3 step 4 now takes L = max(farthest corner distance, head depth) + 1 mm (test 24).
4. **Head ending at the shoulder.** Resolved: the head checks accepted a head that ends less than the modeling tolerance above the shoulder, which left a bore too short for the kernel (a counterbore 11 x 4.9999999 in a hole 5 deep failed with the kernel's `Curve 5 of the profile has zero length.`). Section 3.3 step 2 now requires the head to end at least `LINEAR_TOLERANCE` above L and reports `OutOfRange` on the head's field (tests 25 and 26).
5. **Sizes below the tolerance.** Resolved: a diameter, depth or counterbore depth between 0 and `LINEAR_TOLERANCE`, or a head diameter within 2 `LINEAR_TOLERANCE` of D, passed the checks and failed in the kernel with `Curve <n> of the profile has zero length.`, which names no field. Section 3.3 step 2 now applies the positive length rule of features.md section 3.4 to each of them (tests 27 to 31).
6. **Overlap from the cut.** Resolved: section 3.3 step 5 measured every tool's shared volume with a kernel `common` before the cut, a second boolean on the body as costly as a third of the cut (150 to 166 ms on the 200-face plate of test 33). The cut's history now decides every tool that leaves a face of its own in the result, and the `common` remains for the rare tool that leaves none (one that misses the body, only touches it, or swallows whole solids of it, test 32). A cut that includes a tool which does not overlap is made again without it, so results are as before, except for a tool that leaves faces of its own while sharing at most `VOLUME_TOLERANCE` of the body's volume (a sliver), which now overlaps. The cut is named (through `apply_op_result`) only once every tool is decided: when every tool leaves a face of its own, the body rules name it and their `NoEffect` is step 5's unchanged volume; otherwise the volume and the shared volumes decide first, so a cut that is made again is never named.
