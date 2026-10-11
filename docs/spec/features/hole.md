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
- Placement on a face by click without a sketch.
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

Dedicated form: `hole(points, diameter, depth, counterbore=..., countersink=...)` (python-api.md section 2.7). Generic form of the JSON above:

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

1. The body must hold a solid, else `BodyEmpty { use_new: false }`.
2. Values, checked in this order (D hole diameter, L depth):
   - D > 0 (`GreaterThan(0)`).
   - Counterbore: head diameter > D (`GreaterThan(D)`, bound name `the hole diameter`); depth > 0; with a distance extent, depth < L (`LessThan(L)` on `hole_type.depth`, bound name `the hole depth`).
   - Countersink: head diameter > D (as above); 0 < angle < 180 deg (`GreaterThan(0)`, `LessThan(π)`); with a distance extent, the countersink depth (head - D) / 2 / tan(angle / 2) must be less than L, else `OutOfRange` on `hole_type.diameter` with `LessThan(D + 2 L tan(angle / 2))` and bound name `the largest that fits the hole depth`.
   - Distance extent: L > 0.
   - Tip angle, when set: 0 < tip < 180 deg. With a through-all extent the tip is ignored with an `Ignored { field: "tip_angle", reason: TipAngleThroughAll }` warning.
3. Direction d: the negated sketch normal (into the material when the sketch sits on an outward face); the sketch normal when `reverse` is true.
4. For each point, in `points` order: look it up with `SketchResult::point` (unknown: `UnknownEntity { field: "points[i]", noun: point }`) and map it to world p. Build its tool by revolving a half cross-section about the line through p along d (kernel `revolve`, naming.md open issue 10), with curves tagged by role:
   - `entry`: the segment closing the tool on the sketch plane, at the radius of the widest part.
   - Simple: `bore` at radius D/2 from 0 to L.
   - Counterbore: `counterbore` at the head radius from 0 to the counterbore depth, `counterbore-floor` stepping in to D/2, then `bore` to L.
   - Countersink: `countersink` from the head radius at 0 narrowing at the included angle to D/2, then `bore` to L.
   - Bottom: `bottom` (flat) at L; or `tip`, a cone of included angle `tip_angle` with its apex at L + (D / 2) / tan(tip / 2).
   - Through all: L is that point's largest signed distance (c - p) · d over the corners c of the body's tight bounding box, plus 1 mm. If that distance is ≤ 0 the point gets no tool.
5. A point whose tool does not overlap the body (common volume ≤ 1e-9 × body volume), or that got no tool, adds a `HoleMissesBody` warning and is left out. Points with identical refs are used once.
6. Combine the remaining tools in one multi-argument kernel cut of the body: `apply_op(ctx, body, Cut, tool)` with the tools fused. If no tool remains, the body is unchanged with a `NoEffect` warning. Change `Cut`.

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
| Unknown point | `UnknownEntity` | `'Sketch 1' has no point 'e-p9'. Re-pick it for Points.` |
| Diameter ≤ 0 | `OutOfRange` | `Diameter must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Head diameter too small | `OutOfRange` | `Head diameter must be greater than the hole diameter (6 mm), but it is 5 mm. Use a value greater than 6 mm.` |
| Counterbore deeper than the hole | `OutOfRange` | `Counterbore depth must be less than the hole depth (5 mm), but it is 6 mm. Use a value less than 5 mm.` |
| Countersink angle out of range | `OutOfRange` | `Countersink angle must be less than 180 deg, but it is 180 deg. Use a value less than 180 deg.` |
| Countersink deeper than the hole | `OutOfRange` | `Head diameter must be less than the largest that fits the hole depth (16 mm), but it is 20 mm. Use a value less than 16 mm.` |
| Expression or dimension error | `Expression` | `Diameter: Expected a length, but this expression gives an angle.` |
| Body empty | `BodyEmpty` | `Body 'Body 1' is empty at this point, so this feature has nothing to work on. Pick a body that has a solid.` |
| Kernel failure | `Kernel` | the kernel's message |
| Point misses the body | warning `HoleMissesBody` | `The hole at point 'e-p3' does not touch body 'Body 1'. Move the point, or turn on Reverse.` |
| Tip with through all | warning `Ignored { TipAngleThroughAll }` | `Tip angle is ignored because the hole goes through all.` |
| Nothing cut | warning `NoEffect` | features.md section 4.2 |
| Removes everything | `EmptyResult` | features.md section 4.1 |

## 5. Performance

- One hole in a box: 15 ms. Ten holes in a 200-face body: 80 ms.

## 6. Acceptance tests

Conventions from features.md section 6.1. Fixtures:

- `BOX-BELOW`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "position": ["0", "0", "-10"], "op": "new"}` (spans [-20,-20,-10]-[20,20,0], volume 16000, top face at z=0).
- `SK-PTS`: `{"id": "f-sk1", "type": "sketch", "plane": {"type": "origin", "name": "XY"}, "entities": [{"id": "e-p1", "type": "point", "position": [0, 0]}, {"id": "e-p2", "type": "point", "position": [10, 10]}, {"id": "e-p3", "type": "point", "position": [100, 0]}]}`.
- `HOLE(points, extra)`: `{"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk1", "points": <points>, "diameter": "6", <extra>}`.

Bodies `[b-1]`; features `BOX-BELOW`, `SK-PTS`, then the hole, unless stated. The sketch normal is +Z, so holes go down into the box.

1. **Simple through.** `HOLE(["e-p1"], "extent": {"kind": "through_all"})`. Expected: ok; volume 15717.256661, 7 faces, bbox [-20,-20,-10]-[20,20,0]; `f-hole1/bore/e-p1` cylindrical with radius 3 and area 188.495559; no `f-hole1/entry/e-p1`; change `Cut`.
2. **Blind, flat.** `"extent": {"kind": "distance", "expr": "5"}`. Expected: volume 15858.628331, 8 faces; `f-hole1/bore/e-p1` area 94.247780; `f-hole1/bottom/e-p1` planar at z=-5 with area 28.274334.
3. **Blind with a tip.** As 2 plus `"tip_angle": "118 deg"`. Expected: volume 15841.639397, 8 faces; `f-hole1/tip/e-p1` conical; no `bottom`; the tip apex at z=-6.802582.
4. **Counterbore.** `"hole_type": {"kind": "counterbore", "diameter": "11", "depth": "4"}, "extent": {"kind": "through_all"}`. Expected: volume 15450.221286, 9 faces; `f-hole1/counterbore/e-p1` area 138.230077; `f-hole1/counterbore-floor/e-p1` planar at z=-4 with area 66.758844; `f-hole1/bore/e-p1` area 113.097336.
5. **Countersink.** `"hole_type": {"kind": "countersink", "diameter": "12", "angle": "90 deg"}, "extent": {"kind": "through_all"}`. Expected: volume 15604.159326, 8 faces; `f-hole1/countersink/e-p1` conical; `f-hole1/bore/e-p1` area 131.946891.
6. **Two points.** `HOLE(["e-p1", "e-p2"], "extent": {"kind": "through_all"})`. Expected: volume 15434.513322, 8 faces; walls `f-hole1/bore/e-p1` and `f-hole1/bore/e-p2`.
7. **One point off the body.** `HOLE(["e-p1", "e-p3"], "extent": {"kind": "through_all"})`. Expected: warning `hole_misses_body` rendered `The hole at point 'e-p3' does not touch body 'Body 1'. Move the point, or turn on Reverse.`; volume 15717.256661.
8. **Every point off the body.** `HOLE(["e-p3"], "extent": {"kind": "through_all"})`. Expected: warnings `hole_misses_body` and `no_effect`; b-1 unchanged.
9. **Reverse drills away.** `HOLE(["e-p1"], "extent": {"kind": "distance", "expr": "5"}, "reverse": true)`. Expected: warnings `hole_misses_body` and `no_effect`; b-1 unchanged.
10. **Head diameter too small.** As 4 with head diameter `"5"`. Expected: error `out_of_range`, field `hole_type.diameter`, rendered `Head diameter must be greater than the hole diameter (6 mm), but it is 5 mm. Use a value greater than 6 mm.`
11. **Counterbore deeper than the hole.** `"hole_type": {"kind": "counterbore", "diameter": "11", "depth": "6"}, "extent": {"kind": "distance", "expr": "5"}`. Expected: error `out_of_range`, field `hole_type.depth`, rendered `Counterbore depth must be less than the hole depth (5 mm), but it is 6 mm. Use a value less than 5 mm.`
12. **Countersink angle 180.** As 5 with angle `"180 deg"`. Expected: error `out_of_range`, field `hole_type.angle`, rendered `Countersink angle must be less than 180 deg, but it is 180 deg. Use a value less than 180 deg.`
13. **Countersink deeper than the hole.** `"hole_type": {"kind": "countersink", "diameter": "20", "angle": "90 deg"}, "extent": {"kind": "distance", "expr": "5"}`. Expected: error `out_of_range`, field `hole_type.diameter`, rendered `Head diameter must be less than the largest that fits the hole depth (16 mm), but it is 20 mm. Use a value less than 16 mm.`
14. **Tip with through all.** As 1 plus `"tip_angle": "118 deg"`. Expected: warning `ignored` rendered `Tip angle is ignored because the hole goes through all.`; geometry as 1.
15. **Unknown point.** `HOLE(["e-p9"], "extent": {"kind": "through_all"})`. Expected: error `unknown_entity`, field `points[0]`, rendered `'Sketch 1' has no point 'e-p9'. Re-pick it for Points.`
16. **Line end as a point.** Sketch f-sk1 with `{"id": "e-1", "type": "line", "start": [0, 0], "end": [10, 0]}`; `HOLE(["e-1.start"], "extent": {"kind": "through_all"})`. Expected: geometry as 1; wall named `f-hole1/bore/e-1.start`.
17. **Circle center as a point.** Sketch f-sk1 with `{"id": "e-5", "type": "circle", "center": [0, 0], "radius": 3}`; `HOLE(["e-5.center"], "extent": {"kind": "through_all"})`. Expected: geometry as 1; wall named `f-hole1/bore/e-5.center`.
18. **Zero diameter.** As 1 with `"diameter": "0"`. Expected: error `out_of_range`, field `diameter`.
19. **No points.** `HOLE([], "extent": {"kind": "through_all"})`. Expected: error `empty_selection` rendered `Select at least one point for Points.`
20. **Sketch on the body's face.** `SK-PTS` with plane `{"type": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,0|a=1600"}, "fallback": {"origin": [0, 0, 0], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}}`; hole as 1. Expected: geometry as 1 (the face placement has normal +Z and origin (0,0,0)).
21. **Performance.** Test 6 builds in under 25 ms (desktop, release).

## Open issues

1. An unknown sketch point is an error rather than a skipped hole with a warning, because sketch ids are under the user's direct control (unlike topological references). The reviewer may prefer skip-and-warn for consistency with reference lists.
2. Depth is measured to the shoulder (the end of the full-diameter cylinder), with any tip beyond it, as Fusion does by default.
3. Test 20 uses a sketch plane with `body`, which sketch.md's `SketchPlane::Face` does not have yet (features.md open issue 6).
