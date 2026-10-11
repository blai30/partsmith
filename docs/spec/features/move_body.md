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

1. The body must hold a solid, else `BodyEmpty { use_new: false }`.
2. Resolve the rotation axis (if any) against the state before the move, so an edge of the moved body itself may be the axis.
3. The transform is kernel `Transform::rotation(axis, angle).then(&Transform::translation(t))`: rotate about the axis line, then translate.
4. If the translation is zero (each component abs ≤ 1e-12 mm) and there is no rotation or the angle is a multiple of 2π within 1e-12 rad, the body is unchanged with a `NoEffect` warning.
5. Apply kernel `transform` (index-preserving). The naming map is carried over unchanged: every element keeps its name, and the map is not rebuilt (naming.md section 3.6.12). Change `Modify`.

### 3.3 Element names

No new names; all names are kept.

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Body empty | `BodyEmpty` | `Body 'Body 1' is empty at this point, so this feature has nothing to work on. Pick a body that has a solid.` |
| Expression or dimension error | `Expression` | `Rotation angle: Expected an angle, but this expression gives a length.` |
| Axis errors | `NotStraight`, `NoAxis`, `UnknownEntity`, `NotALine`, `ReferenceNotFound` | features.md section 4.1 |
| Kernel failure | `Kernel` | `Moving the body failed.` |
| Nothing moves | warning `NoEffect` | features.md section 4.2 |

## 5. Performance

- Any body: 5 ms (a rotation or translation shares the geometry).

## 6. Acceptance tests

Conventions from features.md section 6.1. Bodies `[b-1]`. Fixtures: `BOX123`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "20", "30"], "op": "new"}` (spans [-5,-10,0]-[5,10,30]); `MV(extra)`: `{"id": "f-mv1", "type": "move_body", "body": "b-1", <extra>}`; `ROT-Z(angle)`: `{"axis": {"kind": "origin", "axis": "Z"}, "angle": angle}`.

1. **Translate.** `BOX123`, `MV("translation": ["5", "0", "0"])`. Expected: ok; volume 6000, 6 faces, bbox [0,-10,0]-[10,10,30]; `f-box1/top` planar at z=30 spanning x 0..10; change `Modify`.
2. **Rotate.** `BOX123`, `MV("rotation": ROT-Z("90 deg"))`. Expected: bbox [-10,-5,0]-[10,5,30]; `f-box1/front` (previously y=-10) is the plane x=10 with outward normal (1,0,0).
3. **Rotate, then translate.** `BOX123`, `MV("translation": ["5", "0", "0"], "rotation": ROT-Z("90 deg"))`. Expected: bbox [-5,-5,0]-[15,5,30].
4. **Rotate about its own edge.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`, `MV("rotation": {"axis": {"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-box1/right|f-box1/top)", "fp": "line|n=0,1,0|c=5,0,10|l=10"}}, "angle": "180 deg"})`. Expected: volume 1000, bbox [5,-5,10]-[15,5,20].
5. **References survive a move.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`, `MV("translation": ["50", "0", "0"])`, `{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-5,10|l=10"}], "radius": "2"}`. Expected: fillet ok with no warnings (exact name match); volume 991.415927, 7 faces, bbox [45,-5,0]-[55,5,10].
6. **Nothing moves.** `BOX123`, `MV("translation": ["0", "0", "0"], "rotation": ROT-Z("360 deg"))`. Expected: warning `no_effect`; b-1 unchanged.
7. **Empty body.** `MV("translation": ["5", "0", "0"])` alone. Expected: error `body_empty`.
8. **Wrong angle dimension.** `BOX123`, `MV("rotation": ROT-Z("5 mm"))`. Expected: error `expression`, field `rotation.angle`, rendered `Rotation angle: Expected an angle, but this expression gives a length.`
9. **Performance.** Test 3 builds in under 5 ms (desktop, release).

## Open issues

None beyond features.md.
