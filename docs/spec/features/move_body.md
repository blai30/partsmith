# Feature: move_body

Status: M1 spec. Type name `move_body`. Shared contracts: [features.md](../features.md). Naming rule: [naming.md](../naming.md) section 3.6.12.

## 1. Purpose and scope

Move body rotates a body about an axis and then translates it. All element names are kept, so references to the body survive the move.

### In M1

- Translation by x, y and z in world coordinates.
- Optional rotation by an angle about an origin axis, a sketch line, a straight edge or a round face's axis.
- Rotation first, then translation.

### Later

- Copy instead of move.
- Point-to-point, align, along-axis-by-reference and free-drag modes.
- Moving several bodies at once.
- Scale (uniform and non-uniform) as its own feature.

## 2. Public API

```rust
/// Rotates and then translates a body.
pub struct MoveBody;

impl Feature for MoveBody {
    const TYPE: &'static str = "move_body";
    type Params = MoveBodyParams;
}

/// Params of a move body feature.
pub struct MoveBodyParams {
    /// The body to move.
    pub body: Id,
    /// Translation along world X, Y and Z, applied after the rotation. Lengths. Default ["0", "0", "0"], omitted when default.
    pub translation: [Expr; 3],
    /// Rotation applied first. Omitted when None.
    pub rotation: Option<MoveBodyRotation>,
}

/// A rotation about an axis. Named with the feature prefix so its schema definition is unique (features.md section 2.2).
pub struct MoveBodyRotation {
    /// The axis, resolved before the move.
    pub axis: AxisRef,
    /// Angle, positive by the right-hand rule about the axis direction. Any value.
    pub angle: Expr,
}
```

JSON:

```json
{"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["5", "0", "0"],
 "rotation": {"axis": {"kind": "origin", "axis": "Z"}, "angle": "90 deg"}}
```

### Manifest

Label `Move body`, category `modify`, icon `move-body`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `body` | Body | yes | | `body` | |
| `translation[0]` | Move X | yes | length | | |
| `translation[1]` | Move Y | yes | length | | |
| `translation[2]` | Move Z | yes | length | | |
| `rotation.axis` | Rotation axis | no | | `axis` | |
| `rotation.angle` | Rotation angle | no | angle | | |

### Python

Dedicated form: `move(translate=(x, y, z), rotate=(axis, angle))` (python-api.md section 2.7), produced by the `Feature::python` hook with the shared rules of features.md section 3.11: the `with <part>:` block gives `body`, so the `part` argument is not written; `translate=` is a tuple of the three `translation` expressions when it is not the default; `rotate=` is a tuple of `PythonValue::axis` and the angle when `rotation` is set. The JSON above is `move(translate=(5, 0, 0), rotate=(Axis.Z, 90*deg), id="f-mv1")`. Generic form of the JSON above:

```python
feature(
    "move_body",
    id="f-mv1",
    body="b-1",
    translation=["5", "0", "0"],
    rotation={"axis": {"kind": "origin", "axis": "Z"}, "angle": "90 deg"},
)
```

## 3. Behavior

### 3.1 Dependencies

- `bodies_written`: `[body]`; axis dependencies (sketch feature, or another body in `bodies_read`).
- `expressions`: `translation[0..2]` (Length), then `("rotation.angle", Angle)` when set.

### 3.2 Build

1. The body must hold a solid, else `BodyEmpty { field: body, body, use_new: false }` (recompute's diagnosis adds why it is empty, features.md section 4.1).
2. Each translation component is at most the modeling limit in absolute value (`AtMost(MODEL_LIMIT)`, `AtLeast(-MODEL_LIMIT)` on `translation[i]`, features.md section 3.4).
3. Resolve the rotation axis (if any) with `ctx.axis` against the state before the move, so an edge of the moved body itself may be the axis. Its warnings and notices go into the output. An `edge` or `face` axis keeps the side the user picked: it is oriented by its stored `direction` moved with the part, or else by its fingerprint, so a small edit that tilts the edge never turns the part the other way (corpus R38); edges or faces whose axes do not lie on one line give `NotCollinear` (features.md section 3.6).
4. The transform is kernel `Transform::rotation(axis, angle).then(&Transform::translation(t))`: rotate about the axis line, then translate.
5. If the translation is zero (each component abs ≤ 1e-12 mm) and there is no rotation or the angle is a multiple of 2π within 1e-12 rad, the body is unchanged with a `NoEffect { how: NotMoved }` warning (features.md section 3.15).
6. Apply kernel `transform` (index-preserving). The output map is `NamingMap::moved(transform)` (naming.md section 3.6.12): every element keeps its name, and the transform is composed after the motion of every face (and the map's base), so a sketch on a face of the moved body whose fallback is in body coordinates, a stored axis `direction` and the faces later features build on such a sketch all move with the part. Change `Modify`.

### 3.3 Element names

No new names; all names are kept, with their motions moved by the transform.

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Body empty | `BodyEmpty` | without a cause: `Body 'Body 1' has no solid at this point, so this move body has nothing to work on. Pick a body that has a solid.`; with one, features.md section 4.1 |
| Translation beyond the modeling limit | `OutOfRange` | `Move X must be at least -1000000 mm, but it is -10000000 mm. Use -1000000 mm or more.` |
| Expression or dimension error | `Expression` | `Rotation angle: Expected an angle, but this expression gives a length.` |
| Axis errors | `NotStraight`, `NoAxis`, `NotCollinear`, `UnknownEntity`, `NotALine`, `ReferenceNotFound` | features.md section 4.1 |
| Kernel failure | `Kernel` | `Partsmith's geometry kernel could not build this move body. {hint}` (features.md section 4.1) |
| Nothing moves | warning `NoEffect` (`NotMoved`) | `This move body leaves body 'Body 1' where it is, because its move is 0 and it turns by whole turns or not at all.` |
| The axis reference guessed or followed | warning `ReferenceGuessed`, notice `ReferenceFollowed` | features.md section 4.2 |

## 5. Performance

- Any body: 5 ms (a rotation or translation shares the geometry).

## 6. Acceptance tests

Conventions from features.md section 6.1. Bodies `[b-1]`. Fixtures: `BOX123`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "20", "30"], "op": "new"}` (spans [-5,-10,0]-[5,10,30]); `MV(extra)`: `{"id": "f-mv1", "type": "move_body", "body": "b-1", <extra>}`; `ROT-Z(angle)`: `{"axis": {"kind": "origin", "axis": "Z"}, "angle": angle}`.

1. **Translate.** `BOX123`, `MV("translation": ["5", "0", "0"])`. Expected: ok; volume 6000, 6 faces, bbox [0,-10,0]-[10,10,30]; `f-box1/top` planar at z=30 spanning x 0..10; change `Modify`.
2. **Rotate.** `BOX123`, `MV("rotation": ROT-Z("90 deg"))`. Expected: bbox [-10,-5,0]-[10,5,30]; `f-box1/front` (previously y=-10) is the plane x=10 with outward normal (1,0,0).
3. **Rotate, then translate.** `BOX123`, `MV("translation": ["5", "0", "0"], "rotation": ROT-Z("90 deg"))`. Expected: bbox [-5,-5,0]-[15,5,30].
4. **Rotate about its own edge.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`, `MV("rotation": {"axis": {"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-box1/right|f-box1/top)", "fp": "line|n=0,1,0|c=5,0,10|l=10"}}, "angle": "180 deg"})`. Expected: volume 1000, bbox [5,-5,10]-[15,5,20].
5. **References survive a move.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`, `MV("translation": ["50", "0", "0"])`, `{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-5,10|l=10"}], "radius": "2"}`. Expected: fillet ok with no warnings (exact name match); volume 991.415927, 7 faces, bbox [45,-5,0]-[55,5,10].
6. **Nothing moves.** `BOX123`, `MV("translation": ["0", "0", "0"], "rotation": ROT-Z("360 deg"))`. Expected: warning `no_effect` with how `NotMoved`, rendered `This move body leaves body 'Body 1' where it is, because its move is 0 and it turns by whole turns or not at all.`; b-1 unchanged.
7. **Empty body.** `MV("translation": ["5", "0", "0"])` alone. Expected: error `body_empty` rendered `Body 'Body 1' has no solid at this point, so this move body has nothing to work on. Pick a body that has a solid.`
8. **Wrong angle dimension.** `BOX123`, `MV("rotation": ROT-Z("5 mm"))`. Expected: error `expression`, field `rotation.angle`, rendered `Rotation angle: Expected an angle, but this expression gives a length.`
9. **Performance.** Test 3 builds in under 5 ms (desktop, release).
10. **A sketch after the move keeps its point on the moved face.** The document of [sketch.md](sketch.md) test 20 (corpus R32a: a face sketch on the top of a 40 x 40 x 10 box with `fallback_frame: body`, a hole at its point (15, 15), and this move body of (100, 0, 0) inserted before the sketch), `examples/references/moved-face-sketch`. Expected: every feature status ok; in b-1's state after f-mv1, `BodyState::motion` of `f-box1/top` is the translation (100, 0, 0); the bore's axis passes through (115, 15, 0); with the rotation of 90 deg about Z instead (made by the test from the example), through (-15, 15, 0).
11. **An axis edge that leans keeps its side (corpus R38).** `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": RECT(0, 0, 40, 20)}`; `{"id": "f-ext1", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=20,10|a=800"}]}, "extent": {"kind": "distance", "expr": "10"}, "op": "new"}`; `MV("rotation": {"axis": {"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-ext1/lateral/e-4|f-ext1/start-cap)", "fp": "line|n=0,1,0|c=0,10,0|l=20|adj=2,2"}}, "angle": "90 deg"})` (about the left wall's bottom edge, which passes through the origin). Expected: ok; bbox [0,0,-40]-[10,20,0] (the part turns down). Edit: move the shared point of `e-3`'s end and `e-4`'s start from (0, 20) to (-1, 20). The document after the edit is `examples/move_body/leaning-edge`, and the test makes the document before the edit and the one leaning to (1, 20) from it. Expected: ok; the axis keeps the fingerprint's side, (-0.049938, 0.998752, 0), so the part still turns down: bbox [-1,-1.995012,-40.948846]-[10.087274,20.499376,0]. Leaning to (1, 20) instead: bbox [0,-0.499376,-39.950094]-[11.08478,21.945137,0].
12. **A 180 degree turn before the move turns its axis.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`; `MV("rotation": ROT-Z("180 deg"))` (inserted later, so the edge `edge(f-box1/right|f-box1/top)` now lies at x = -5, z = 10); `{"id": "f-mv2", "type": "move_body", "body": "b-1", "rotation": {"axis": {"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-box1/right|f-box1/top)", "fp": "line|n=0,1,0|c=5,0,10|l=10"}, "direction": [0, 1, 0]}, "angle": "90 deg"}}` (picked before f-mv1 was inserted), as `examples/move_body/turned-edge-axis`. Expected: ok; the stored direction turns with the part to -Y, so the box turns over that edge to where it went before the insert: bbox [-5,-5,10]-[5,5,20]. The same document without `direction` (made by the test from the example) keeps the fingerprint's side, +Y: bbox [-15,-5,0]-[-5,5,10].
13. **Axis edges not on one line.** b-1 holds the L-shaped body of [fillet.md](fillet.md) test 4; `MV("rotation": {"axis": {"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=-10,-10,10|l=20", "merged": {"f-box1/top": ["f-box2/top"]}}}, "angle": "90 deg"})`. Expected: error `not_collinear`, field `rotation.axis`, rendered `The edge selected for Rotation axis now matches 2 edges that do not lie on one line. Pick the one you mean.`, fixes `UseReferences` with each of the two edges, then `Pick`; b-1 unchanged.
14. **Rotate about a sketch line.** `BOX123`; `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-1", "type": "line", "start": [5, -10], "end": [5, 10], "construction": true}]}` (along the box's bottom right edge); `MV("rotation": {"axis": {"kind": "sketch_line", "sketch": "f-sk1", "line": "e-1"}, "angle": "90 deg"})`, as `examples/move_body/rotate-about-sketch-line`. The axis runs from the line's start to its end, +Y through (5,-10,0) (features.md section 3.6), so the turn takes +Z toward +X. Expected: ok; volume 6000, 6 faces, bbox [5,-10,0]-[35,10,10] (the box tips over onto its right side); `f-box1/top` (previously z=30) is the plane x=35 with outward normal (1,0,0); `f-box1/right` is the plane z=0 with outward normal (0,0,-1); dependencies `features == ["f-sk1"]`.
15. **Rotate about a round face.** Bodies `[b-1, b-2]`; `BOX123`; `{"id": "f-cyl1", "type": "cylinder", "body": "b-2", "radius": "2", "height": "5", "position": ["20", "0", "0"], "op": "new"}` (its axis along Z through (20,0,0)); `MV("rotation": {"axis": {"kind": "face", "body": "b-2", "face": {"q": "f-cyl1/side", "fp": "cylinder|n=0,0,1|c=20,0,2.5|a=62.831853|r=2|adj=2,3"}, "direction": [0, 0, 1]}, "angle": "90 deg"})` (as a pick stores it), as `examples/move_body/rotate-about-face`. The stored direction (body coordinates; b-2 has not moved) orients the axis, +Z (features.md section 3.6). Expected: ok; volume 6000, 6 faces, bbox [10,-25,0]-[30,-15,30]; `f-box1/front` (previously y=-10) is the plane x=30 with outward normal (1,0,0); b-2 unchanged; dependencies `bodies_read == ["b-2"]`.

## Open issues

None beyond features.md.
