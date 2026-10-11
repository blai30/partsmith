# Feature: datum_plane

Status: M1 spec. Type name `datum_plane`. Shared contracts: [features.md](../features.md), planes in features.md section 3.6. Naming rule: [naming.md](../naming.md) section 3.6.1 (no naming map).

## 1. Purpose and scope

A datum plane is construction geometry: a plane offset from an origin plane, another datum plane or a flat face. Sketches, primitives, mirrors and splits use it. It changes no body.

### In M1

- Offset (default 0) along the base plane's normal from an origin plane, a datum plane or a flat face.
- `flip` to reverse the normal.

### Later

- Angle to a plane about an edge, three points, midplane between two faces, tangent to a round face, plane through an edge and a point, plane at a point on a path.
- Datum axes, datum points and coordinate systems.

## 2. Public API

```rust
/// A construction plane offset from another plane.
pub struct DatumPlane;

impl Feature for DatumPlane {
    const TYPE: &'static str = "datum_plane";
    type Params = DatumPlaneParams;
}

/// Params of a datum plane feature.
pub struct DatumPlaneParams {
    /// The plane to offset from.
    pub base: PlaneRef,
    /// Distance along the base normal. Length, any sign. Default "0", omitted when "0".
    pub offset: Expr,
    /// Reverse the normal (the x-axis stays). Default false, omitted when false.
    pub flip: bool,
}
```

JSON:

```json
{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "10 mm"}
{"id": "f-dp2", "type": "datum_plane", "base": {"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=3,4,10|a=100"}}, "offset": "5", "flip": true}
```

The output placement uses sketch.md's `Placement` JSON, for example `{"origin": [0, 0, 10], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}` (the protocol's `datum_plane` event carries it).

### Manifest

Label `Datum plane`, category `construct`, icon `datum-plane`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `base` | Base plane | yes | | `plane` | |
| `offset` | Offset | yes | length | | |
| `flip` | Flip | no | | | |

### Python

Dedicated form: `datum_plane(base, offset=0, flip=...)` (python-api.md section 2.7), produced by the `Feature::python` hook with the shared rules of features.md section 3.11: `base` is `PythonValue::plane`; `offset` is written when the stored params set it; `flip=True` when true. With `b-1` as `block`, the two JSON records above are `datum_plane(Plane.XY, 10*mm, id="f-dp1")` and `datum_plane(Ref("f-box1/top", "plane|n=0,0,1|c=3,4,10|a=100", part=block), 5, flip=True, id="f-dp2")` (written one argument per line, being longer than 88 characters). Generic form of the first (longer than 88 characters, so one argument per line):

```python
feature(
    "datum_plane",
    id="f-dp1",
    base={"kind": "origin", "plane": "XY"},
    offset="10 mm",
)
```

## 3. Behavior

### 3.1 Dependencies

- `features`: the base datum, if any. `bodies_read`: the base face's body, if any. `bodies_written`: none.
- `expressions`: `("offset", offset, Length)`.

### 3.2 Build

1. placement = `ctx.plane("base", base)`.
2. placement = `placement.offset(offset)` (any sign; 0 is allowed and makes a datum on the base itself).
3. If `flip`, placement = `placement.flipped()` (same origin and x-axis, normal and y-axis reversed).
4. Output `OutputData::Datum(DatumResult::Plane(placement))`, no warnings.

A datum on a face follows that face through later edits by name; if the face is lost, the datum fails with `ReferenceNotFound`, and features that use the datum report `UpstreamFailed`.

### 3.3 Element names

None. Features address the datum by its feature id.

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Expression or dimension error | `Expression` | `Offset: Expected a length, but this expression gives an angle.` |
| Base face not flat | `NotPlanar` | `The face selected for Base plane is not flat. Pick a flat face, a datum plane or an origin plane.` |
| Base face lost | `ReferenceNotFound` | `The face selected for Base plane can no longer be found. Re-pick it in the repair panel.` |
| Base feature not a datum | `WrongFeatureType` | `'Sketch 1' is not a datum plane. Pick a datum plane for Base plane.` |
| Base body empty | `BodyEmpty` | features.md section 4.1 |

## 5. Performance

- Origin or datum base: 0.1 ms. Face base on a 200-face body: 1 ms plus the body's element info (computed once per state).

## 6. Acceptance tests

Conventions from features.md section 6.1. The expected placement is the `DatumResult::Plane` of the feature's output (tolerance 1e-9). `DP(base, extra)`: `{"id": "f-dp1", "type": "datum_plane", "base": <base>, <extra>}`.

1. **XY offset.** `DP({"kind": "origin", "plane": "XY"}, "offset": "10")`. Expected: ok; origin (0,0,10), x_axis (1,0,0), normal (0,0,1), y_axis (0,1,0); no body changes.
2. **XZ offset.** `DP({"kind": "origin", "plane": "XZ"}, "offset": "10")`. Expected: origin (0,-10,0), x_axis (1,0,0), normal (0,-1,0), y_axis (0,0,1).
3. **YZ negative offset.** `DP({"kind": "origin", "plane": "YZ"}, "offset": "-5")`. Expected: origin (-5,0,0), x_axis (0,1,0), normal (1,0,0), y_axis (0,0,1).
4. **Flip.** `DP({"kind": "origin", "plane": "XY"}, "offset": "10", "flip": true)`. Expected: origin (0,0,10), x_axis (1,0,0), normal (0,0,-1), y_axis (0,-1,0).
5. **Default offset.** `DP({"kind": "origin", "plane": "XY"})`. Expected: origin (0,0,0), normal (0,0,1).
6. **On a face.** Bodies `[b-1]`; `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "position": ["3", "4", "0"], "op": "new"}`; `DP({"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=3,4,10|a=100"}}, "offset": "5")`. Expected: origin (0,0,15), x_axis (1,0,0), normal (0,0,1); dependencies `bodies_read == ["b-1"]`.
7. **On a side face (x-axis fallback).** As 6 with face `{"q": "f-box1/right", "fp": "plane|n=1,0,0|c=8,4,5|a=100"}` (plane x=8) and no offset. Expected: origin (8,0,0), normal (1,0,0), x_axis (0,1,0), y_axis (0,0,1).
8. **On a front face.** As 6 with face `{"q": "f-box1/front", "fp": "plane|n=0,-1,0|c=3,-1,5|a=100"}` (plane y=-1) and no offset. Expected: origin (0,-1,0), normal (0,-1,0), x_axis (1,0,0), y_axis (0,0,1).
9. **On a tilted face.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"}`, `{"id": "f-cha1", "type": "chamfer", "body": "b-1", "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}], "distance": {"kind": "equal", "expr": "2"}}`, `DP({"kind": "face", "body": "b-1", "face": {"q": "f-cha1/chamfer/edge(f-box1/front|f-box1/top)", "fp": "plane|n=0,-0.707107,0.707107|c=0,-9,19|a=56.568542"}})`. Expected: origin (0,-14,14), normal (0,-0.70710678,0.70710678), x_axis (1,0,0).
10. **Chained datums.** `DP({"kind": "origin", "plane": "XY"}, "offset": "10")`, then `{"id": "f-dp2", "type": "datum_plane", "base": {"kind": "datum", "feature": "f-dp1"}, "offset": "5"}`. Expected: f-dp2 origin (0,0,15), normal (0,0,1); f-dp2 dependencies `features == ["f-dp1"]`.
11. **Curved face.** Bodies `[b-1]`; `{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "5", "height": "10", "op": "new"}`; `DP({"kind": "face", "body": "b-1", "face": {"q": "f-cyl1/side", "fp": "cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5"}})`. Expected: error `not_planar` rendered `The face selected for Base plane is not flat. Pick a flat face, a datum plane or an origin plane.`
12. **Base is a sketch.** A sketch `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}}`, then `DP({"kind": "datum", "feature": "f-sk1"})`. Expected: error `wrong_feature_type` rendered `'Sketch 1' is not a datum plane. Pick a datum plane for Base plane.`
13. **Lost face.** As 6 with face `{"q": "f-box1/nothing", "fp": "plane|n=0,0,1|c=500,500,500|a=1"}`. Expected: error `reference_not_found`; `repair()` is `Element` with pointer `/base/face`.
14. **Expression offset.** Parameters `[{"id": "p-wall", "name": "wall", "expr": "2 mm"}]`, `DP({"kind": "origin", "plane": "XY"}, "offset": "2 * wall")`. Expected: origin (0,0,4).
15. **Performance.** Test 1 builds in under 0.1 ms (desktop, release).

## Open issues

None beyond features.md (the face placement rule is features.md section 3.6).
