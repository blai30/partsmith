# Feature: circular_pattern

Status: M1 spec. Type name `circular_pattern`. Shared contracts: [features.md](../features.md), copy features in features.md section 3.5. Naming rule: [naming.md](../naming.md) section 3.6.11.

## 1. Purpose and scope

Circular pattern copies a body around an axis, either evenly around a full turn or spread across a partial angle. The copies are fused into the source body, or put into another, empty body.

### In M1

- Count (including the original) and total angle (default 360 deg).
- Axis from an origin axis, a sketch line, a straight edge or a round face's axis, with `reverse`.
- Op `join` (default) or `new` with a `target` body.
- A limit of 10000 instances.

### Later

- Patterns of features and faces.
- Skipping instances.
- Angle between instances instead of total angle.
- Symmetric partial patterns.

## 2. Public API

```rust
/// Copies a body around an axis.
pub struct CircularPattern;

impl Feature for CircularPattern {
    const TYPE: &'static str = "circular_pattern";
    type Params = CircularPatternParams;
}

/// Params of a circular pattern feature.
pub struct CircularPatternParams {
    /// The body to copy.
    pub body: Id,
    /// The rotation axis.
    pub axis: AxisRef,
    /// Number of instances, including the original. Count, at least 1.
    pub count: Expr,
    /// Total angle. Angle, greater than 0 and at most 360 deg. Default "360 deg", omitted when default.
    pub angle: Expr,
    /// Turn in the negative sense about the axis direction. Default false, omitted when false.
    pub reverse: bool,
    /// Where the copies go. Default join, omitted when join.
    pub op: CopyOp,
    /// The empty body that receives the copies. Required with op new, not allowed with op join.
    pub target: Option<Id>,
}
```

JSON:

```json
{"id": "f-cp1", "type": "circular_pattern", "body": "b-1", "axis": {"kind": "origin", "axis": "Z"}, "count": "6"}
{"id": "f-cp2", "type": "circular_pattern", "body": "b-1", "axis": {"kind": "origin", "axis": "Z"}, "count": "3", "angle": "90 deg", "reverse": true}
```

### Manifest

Label `Circular pattern`, category `pattern`, icon `circular-pattern`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `body` | Body | yes | | `body` | |
| `axis` | Axis | yes | | `axis` | |
| `count` | Count | yes | count | | |
| `angle` | Total angle | yes | angle | | |
| `reverse` | Reverse | no | | | |
| `op` | Operation | no | | | `join` Join, `new` New body |
| `target` | Target body | no | | `body` | |

### Python

Dedicated form: `circular_pattern(axis, count, angle="360 deg", reverse=..., into=...)` (python-api.md section 2.7), produced by the `Feature::python` hook with the shared rules of features.md section 3.11: the `with <part>:` block gives `body`; `axis` is `PythonValue::axis`; `angle` is written only when the stored params set it; `reverse=True` when true; op `new` writes `into=<target part>`. The two JSON records above are `circular_pattern(Axis.Z, 6, id="f-cp1")` and `circular_pattern(Axis.Z, 3, 90*deg, reverse=True, id="f-cp2")`. Generic form of the first (longer than 88 characters, so one argument per line):

```python
feature(
    "circular_pattern",
    id="f-cp1",
    body="b-1",
    axis={"kind": "origin", "axis": "Z"},
    count="6",
)
```

## 3. Behavior

### 3.1 Validation

As [mirror.md](mirror.md) section 3.1 for `op` and `target`.

### 3.2 Dependencies

- Bodies and axis dependencies as [linear_pattern.md](linear_pattern.md).
- `expressions`: `("count", Count)`, `("angle", Angle)`.

### 3.3 Build

1. The source body must hold a solid, else `BodyEmpty { field: body, body, use_new: false }` (recompute's diagnosis adds why it is empty, features.md section 4.1).
2. n = `ctx.eval_count("count", count, 1)`, so a count below 1, fractional or not, is `OutOfRange` (`AtLeast(1)`) (features.md section 3.4); n > 10000 gives `TooManyInstances { count: n, limit: 10000 }`. θ = angle in (0, 2π] (`GreaterThan(0)`; above 2π + 1e-9 `AtMost(2π)`, rendered `360 deg`). The axis comes from `ctx.axis` (features.md section 3.6) with its warnings and notices, which go into the output; an `edge` or `face` axis keeps the side the user picked (its stored `direction` moved with the part, else its fingerprint), so a small edit that tilts the edge never turns the copies the other way (corpus R38), and edges or faces whose axes do not lie on one line give `NotCollinear`.
3. Instance k (0 ≤ k < n; 0 is the original) is the source rotated about the axis by φ_k (kernel `transform`, index-preserving), positive by the right-hand rule about the axis direction (negative when `reverse`), named with `source_map.with_instance(<id>, Instance { first: k, second: 0 }, r_k)` with `r_k` that rotation (naming.md section 3.5.1), so every face of a copy has its source face's motion followed by `r_k` and a sketch on a copy's face moves with the copy when the angle or count changes (test 12):
   - Full turn (abs(θ - 2π) ≤ 1e-9): φ_k = k·2π/n.
   - Partial: φ_k = k·θ/(n - 1) for n ≥ 2, so the first and last instances sit at the ends of the angle.
4. Cancellation is checked before each instance as in linear_pattern.
5. n = 1 gives a `NoEffect { how: NoCopies { fields: [count] } }` warning with the body unchanged (features.md section 3.15). Otherwise the copies are combined and applied as in linear_pattern (`apply_op(..., None)`, no `SeparateSolid` for separate copies).

### 3.4 Element names

| Name | Face |
| --- | --- |
| `<face name>@<id>:<k>` | The copy of source face `<face name>` in instance k (1 ≤ k < n), for example `f-cyl1/top@f-cp1:3`. |

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Count < 1 (fractional included) | `OutOfRange` | `Count must be at least 1, but it is 0. Use 1 or more.` |
| Count not whole | `NotWholeNumber` | `Count must be a whole number, but it is 2.5.` with the fixes `SetField` `2` and `SetField` `3` (features.md section 4.1) |
| Angle ≤ 0 | `OutOfRange` | `Total angle must be greater than 0 deg, but it is 0 deg. Use a value greater than 0 deg.` |
| Angle > 360 deg | `OutOfRange` | `Total angle must be at most 360 deg, but it is 400 deg. Use 360 deg or less.` |
| Too many instances | `TooManyInstances` | `This circular pattern would make 20000 copies, more than the limit of 10000. Lower Count.` |
| Axis, op, target and body errors | as [linear_pattern.md](linear_pattern.md) section 4, including `NotCollinear` | |
| No copies | warning `NoEffect` (`NoCopies`) | `This circular pattern makes no copies, because Count is 1.` |
| Copies inside the body | warning `NoEffect` (`Inside`) | `This circular pattern adds nothing to body 'Body 1': its copies lie entirely inside the body.` |
| The axis reference guessed or followed | warning `ReferenceGuessed`, notice `ReferenceFollowed` | features.md section 4.2 |

## 5. Performance

- 6 instances of a cylinder with join: 40 ms. 100 instances of a 6-face body: 500 ms.

## 6. Acceptance tests

Conventions from features.md section 6.1. Bodies `[b-1, b-2]`. Fixtures:

- `PEG`: `{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "2", "height": "5", "position": ["20", "0", "0"], "op": "new"}` (axis at x=20, y=0).
- `CP(extra)`: `{"id": "f-cp1", "type": "circular_pattern", "body": "b-1", "axis": {"kind": "origin", "axis": "Z"}, <extra>}`.

1. **Six around.** `PEG`, `CP("count": "6")`. Expected: ok; volume 376.991118, 18 faces, 6 solids, bbox [-22,-19.320508,0]-[22,19.320508,5]; `f-cyl1/top@f-cp1:3` is a disc centered at (-20,0,5); change `Join`.
2. **Partial angle.** `PEG`, `CP("count": "3", "angle": "90 deg")`. Expected: volume 188.495559, 9 faces, bbox [-2,-2,0]-[22,22,5]; `f-cyl1/top@f-cp1:1` centered at (14.142136,14.142136,5); `f-cyl1/top@f-cp1:2` centered at (0,20,5).
3. **Partial, reversed.** As 2 with `"reverse": true`. Expected: bbox [-2,-22,0]-[22,2,5].
4. **Four around a full turn.** `PEG`, `CP("count": "4")`. Expected: volume 251.327412, 12 faces, bbox [-22,-22,0]-[22,22,5] (instances at 0, 90, 180 and 270 deg, not at 360).
5. **Count 1.** `PEG`, `CP("count": "1")`. Expected: warning `no_effect` with how `NoCopies { fields: [count] }`, rendered `This circular pattern makes no copies, because Count is 1.`; b-1 unchanged.
6. **Angle above 360.** `PEG`, `CP("count": "3", "angle": "400 deg")`. Expected: error `out_of_range`, field `angle`, rendered `Total angle must be at most 360 deg, but it is 400 deg. Use 360 deg or less.`
7. **Zero angle.** `PEG`, `CP("count": "3", "angle": "0")`. Expected: error `out_of_range`, field `angle`.
8. **Axis from a hub face.** `PEG`; `{"id": "f-cyl2", "type": "cylinder", "body": "b-2", "radius": "5", "height": "5", "op": "new"}`; `{"id": "f-cp1", "type": "circular_pattern", "body": "b-1", "axis": {"kind": "face", "body": "b-2", "face": {"q": "f-cyl2/side", "fp": "cylinder|n=0,0,1|c=0,0,2.5|a=157.079633|r=5"}}, "count": "6"}`. Expected: b-1 identical to test 1; dependencies `bodies_read == ["b-2"]`; b-2 unchanged.
9. **New body.** `PEG`, `CP("count": "6", "op": "new", "target": "b-2")`. Expected: b-1 unchanged (volume 62.831853); b-2 volume 314.159265, 15 faces.
10. **Performance.** Test 1 builds in under 40 ms (desktop, release).
11. **Fractional count below 1.** `PEG`, `CP("count": "0.5")`. Expected: error `out_of_range`, field `count`, rendered `Count must be at least 1, but it is 0.5. Use 1 or more.`, suggestion `1`; b-1 unchanged.
12. **A sketch on a copy's face follows the angle.** `PEG`; `CP("count": "3", "angle": "90 deg")` (copy 2 at (0, 20)); `{"id": "f-sk2", "type": "sketch", "plane": {"kind": "face", "body": "b-1", "face": {"q": "f-cyl1/top@f-cp1:2", "fp": "plane|n=0,0,1|c=0,20,5|a=12.566371|adj=1,1"}, "fallback": {"origin": [20, 0, 5], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}, "fallback_frame": "body"}, "entities": [{"id": "e-p1", "type": "point", "position": [0, 0]}]}` (the fallback in the coordinates of 'Cylinder 1', before the copy's rotation); `{"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk2", "points": ["e-p1"], "diameter": "1", "extent": {"kind": "through_all"}}`. Expected: every feature status ok; f-sk2 placement origin (0, 20, 5) and `motion` the rotation of 90 deg about Z; the bore's axis passes through (0, 20, 0); b-1 volume 184.568568. With the angle `"120 deg"`: placement origin (-10, 17.320508, 5); the bore's axis passes through (-10, 17.320508, 0); volume 184.568568.
13. **A 180 degree turn of the part turns the axis.** `PEG`; `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["10", "10", "10"], "position": ["5", "5", "0"], "op": "new"}` (x 0..10, y 0..10, its front left edge on the Z axis); `{"id": "f-mv1", "type": "move_body", "body": "b-2", "rotation": {"axis": {"kind": "origin", "axis": "X"}, "angle": "180 deg"}}`; `{"id": "f-cp1", "type": "circular_pattern", "body": "b-1", "axis": {"kind": "edge", "body": "b-2", "edge": {"q": "edge(f-box2/front|f-box2/left)", "fp": "line|n=0,0,1|c=0,0,5|l=10"}, "direction": [0, 0, 1]}, "count": "3", "angle": "90 deg"}` (picked before the move was inserted). Expected: ok; the edge still lies on the Z axis, and its stored direction turns with the part to -Z, so the copies go the other way: b-1 bbox [-2,-22,0]-[22,2,5]. The same document without `direction` keeps the fingerprint's side, +Z: bbox [-2,-2,0]-[22,22,5], as test 2.
14. **An axis edge that leans keeps its side (corpus R38).** Bodies `[b-1, b-2]`; b-2 holds the wall `LEAN(-5)` of [revolve.md](revolve.md) test 28; `PEG`; `{"id": "f-cp1", "type": "circular_pattern", "body": "b-1", "axis": {"kind": "edge", "body": "b-2", "edge": {"q": "edge(f-ext5/lateral/e-12|f-ext5/start-cap)", "fp": "line|n=0,0,1|c=-4,0,15|l=30|adj=2,2"}}, "count": "2", "angle": "90 deg"}`. Expected: ok; the axis keeps a positive z, so the copy turns toward +Y: b-1's bounding box has maximum y above 20 and minimum y -2 within 1e-5, as with `LEAN(-4)`. The same with `LEAN(-3)`.
15. **Axis edges not on one line.** b-2 holds the L-shaped body of [fillet.md](fillet.md) test 4 (with `"body": "b-2"`); `PEG`; `{"id": "f-cp1", "type": "circular_pattern", "body": "b-1", "axis": {"kind": "edge", "body": "b-2", "edge": {"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=-10,-10,10|l=20", "merged": {"f-box1/top": ["f-box2/top"]}}}, "count": "3"}`. Expected: error `not_collinear`, field `axis`, rendered `The edge selected for Axis now matches 2 edges that do not lie on one line. Pick the one you mean.`, fixes `UseReferences` with each of the two edges, then `Pick`; b-1 unchanged.

## Open issues

None beyond features.md.
