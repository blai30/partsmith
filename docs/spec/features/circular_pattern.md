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

1. The source body must hold a solid, else `BodyEmpty { use_new: false }`.
2. n = count ≥ 1 (`AtLeast(1)`); n > 10000 gives `TooManyInstances`. θ = angle in (0, 2π] (`GreaterThan(0)`; above 2π + 1e-9 `AtMost(2π)`, rendered `360 deg`).
3. Instance k (0 ≤ k < n; 0 is the original) is the source rotated about the axis by φ_k (kernel `transform`, index-preserving), positive by the right-hand rule about the axis direction (negative when `reverse`), named with `source_map.with_instance(<id>, Instance { first: k, second: 0 })`:
   - Full turn (abs(θ - 2π) ≤ 1e-9): φ_k = k·2π/n.
   - Partial: φ_k = k·θ/(n - 1) for n ≥ 2, so the first and last instances sit at the ends of the angle.
4. Cancellation is checked before each instance as in linear_pattern.
5. n = 1 gives a `NoEffect` warning with the body unchanged. Otherwise the copies are combined and applied as in linear_pattern.

### 3.4 Element names

| Name | Face |
| --- | --- |
| `<face name>@<id>:<k>` | The copy of source face `<face name>` in instance k (1 ≤ k < n), for example `f-cyl1/top@f-cp1:3`. |

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Count < 1 | `OutOfRange` | `Count must be at least 1, but it is 0. Use 1 or more.` |
| Count not whole | `NotWholeNumber` | `Count must be a whole number, but it is 2.5. Use 2 or 3.` |
| Angle ≤ 0 | `OutOfRange` | `Total angle must be greater than 0 deg, but it is 0 deg. Use a value greater than 0 deg.` |
| Angle > 360 deg | `OutOfRange` | `Total angle must be at most 360 deg, but it is 400 deg. Use 360 deg or less.` |
| Too many instances | `TooManyInstances` | `This pattern would make 20000 instances; the limit is 10000. Use fewer instances.` |
| Axis, op, target and body errors | as [linear_pattern.md](linear_pattern.md) section 4 | |
| No copies, or copies inside the source | warning `NoEffect` | features.md section 4.2 |

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
5. **Count 1.** `PEG`, `CP("count": "1")`. Expected: warning `no_effect`; b-1 unchanged.
6. **Angle above 360.** `PEG`, `CP("count": "3", "angle": "400 deg")`. Expected: error `out_of_range`, field `angle`, rendered `Total angle must be at most 360 deg, but it is 400 deg. Use 360 deg or less.`
7. **Zero angle.** `PEG`, `CP("count": "3", "angle": "0")`. Expected: error `out_of_range`, field `angle`.
8. **Axis from a hub face.** `PEG`; `{"id": "f-cyl2", "type": "cylinder", "body": "b-2", "radius": "5", "height": "5", "op": "new"}`; `{"id": "f-cp1", "type": "circular_pattern", "body": "b-1", "axis": {"kind": "face", "body": "b-2", "face": {"q": "f-cyl2/side", "fp": "cylinder|n=0,0,1|c=0,0,2.5|a=157.079633|r=5"}}, "count": "6"}`. Expected: b-1 identical to test 1; dependencies `bodies_read == ["b-2"]`; b-2 unchanged.
9. **New body.** `PEG`, `CP("count": "6", "op": "new", "target": "b-2")`. Expected: b-1 unchanged (volume 62.831853); b-2 volume 314.159265, 15 faces.
10. **Performance.** Test 1 builds in under 40 ms (desktop, release).

## Open issues

None beyond features.md.
