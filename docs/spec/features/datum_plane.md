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

1. placement = `ctx.plane("base", base)` (features.md section 3.6). Its warnings and notices go into the output: a guess (`ReferenceGuessed`), or a follow such as the continuation of a removed face by the rim a shell left in the same plane (`ReferenceFollowed`, corpus R58). A face reference whose faces do not all lie in one plane gives `NotCoplanar`, with the faces as candidates; a face split into coplanar pieces keeps its plane.
2. placement = `placement.offset(offset)` (any sign; 0 is allowed and makes a datum on the base itself), with the offset at most the modeling limit in absolute value (`AtMost(MODEL_LIMIT)`, `AtLeast(-MODEL_LIMIT)`, features.md section 3.4).
3. If `flip`, placement = `placement.flipped()` (same origin and x-axis, normal and y-axis reversed).
4. **Motion.** For an origin base, the identity. For a datum base, that datum's motion. For a face base, the rigid motion that takes the frame `P0` to the frame `P`, with `P` the base placement of step 1 and `P0` the placement the same rule gives with the face's centroid and normal mapped by `m.inverse()`, `m` the motion of the base face's stored query in the body's state just before the datum (`BodyState::motion`): the part of the face's motion that the datum's frame follows. The face placement rule takes the datum's x axis from world +X, so a Move body that slides the part along its face leaves the datum's frame, and the motion, as they were, and one that turns the face over turns both (test 19). Sketches on the datum take this motion (features/sketch.md section 3.3), and the features built on them pass it to naming as the motion of their new faces (features.md section 3.9).
5. Output `OutputData::Datum(DatumResult::Plane { placement, motion })`, with the warnings and notices of step 1.

A datum on a face follows that face through later edits by name; if the face is lost, the datum fails with `ReferenceNotFound` (with the loss and its candidates, features.md section 4.1), and features that use the datum report `UpstreamFailed` with the datum as the root.

### 3.3 Element names

None. Features address the datum by its feature id.

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Expression or dimension error | `Expression` | `Offset: Expected a length, but this expression gives an angle.` |
| Offset beyond the modeling limit | `OutOfRange` | `Offset must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less.` |
| Base face not flat | `NotPlanar` | `The side of 'Cylinder 1' selected for Base plane is not flat. Pick a flat face, a datum plane or an origin plane.` |
| Base faces not in one plane | `NotCoplanar` | `The face selected for Base plane now matches 2 faces that do not lie in one plane. Pick the one you mean.` |
| Base face lost | `ReferenceNotFound` | `{Element} {reason}.`, for example `The top of 'Box 1' selected for Base plane was cut away by 'Extrude 1', which comes before this datum plane.` (features.md section 4.1) |
| Base feature not a datum | `WrongFeatureType` | `'Sketch 1' is not a datum plane. Pick a datum plane for Base plane.` |
| Base body empty | `BodyEmpty` | features.md section 4.1 |
| Base face guessed or followed | warning `ReferenceGuessed`, notice `ReferenceFollowed` | features.md section 4.2 |

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
11. **Curved face.** Bodies `[b-1]`; `{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "5", "height": "10", "op": "new"}`; `DP({"kind": "face", "body": "b-1", "face": {"q": "f-cyl1/side", "fp": "cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5"}})`. Expected: error `not_planar` rendered `The side of 'Cylinder 1' selected for Base plane is not flat. Pick a flat face, a datum plane or an origin plane.`, fix `Pick { pointer: "/base/face" }`.
12. **Base is a sketch.** A sketch `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}}`, then `DP({"kind": "datum", "feature": "f-sk1"})`. Expected: error `wrong_feature_type` rendered `'Sketch 1' is not a datum plane. Pick a datum plane for Base plane.`
13. **Lost face.** As 6 with face `{"q": "f-box1/nothing", "fp": "plane|n=0,0,1|c=500,500,500|a=1"}`. Expected: error `reference_not_found`; `repair()` is `Element` with pointer `/base/face`; the first fix is `UseReferences` with `f-box1/top` (the only face of 'Box 1' that faces the way the stored face did), the last `Pick`.
14. **Expression offset.** Parameters `[{"id": "p-wall", "name": "wall", "expr": "2 mm"}]`, `DP({"kind": "origin", "plane": "XY"}, "offset": "2 * wall")`. Expected: origin (0,0,4).
15. **Performance.** Test 1 builds in under 0.1 ms (desktop, release).
16. **Faces in two planes.** Bodies `[b-1]`; the L-shaped body of [fillet.md](fillet.md) test 4; `DP({"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=-10,0,10|a=400", "merged": {"f-box1/top": ["f-box2/top"]}}})`. Expected: error `not_coplanar`, field `base`, rendered `The face selected for Base plane now matches 2 faces that do not lie in one plane. Pick the one you mean.`, fixes `UseReferences` with `f-box1/top`, `UseReferences` with `f-box2/top`, then `Pick`. Neither face is chosen by name order.
17. **A face split into coplanar pieces keeps its plane.** Bodies `[b-1]`; `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": RECT(0, 0, 80, 50)}`; `{"id": "f-ext1", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]}, "extent": {"kind": "distance", "expr": "30"}, "op": "new"}`; the slot of naming.md test 84 (`{"id": "f-dp9", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "30"}`, `{"id": "f-sk9", "type": "sketch", "plane": {"kind": "datum", "feature": "f-dp9"}, "entities": [{"id": "e-11", "type": "line", "start": [30, -1], "end": [50, -1]}, {"id": "e-12", "type": "line", "start": [50, -1], "end": [50, 51]}, {"id": "e-13", "type": "line", "start": [50, 51], "end": [30, 51]}, {"id": "e-14", "type": "line", "start": [30, 51], "end": [30, -1]}]}`, `{"id": "f-ext9", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk9", "regions": [{"key": "e-11,e-12,e-13,e-14", "fp": "c=40,25|a=1040"}]}, "extent": {"kind": "distance", "expr": "5"}, "reverse": true, "op": "cut"}`), which splits the end cap into `f-ext1/end-cap#1` and `#2`; `DP({"kind": "face", "body": "b-1", "face": {"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}}, "offset": "5")`. Expected: f-dp1 status ok with no warning; origin (0,0,35), x_axis (1,0,0), normal (0,0,1).
18. **A shell inserted under the datum's face (corpus R58).** Bodies `[b-1]`; `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["80", "50", "30"], "op": "new"}`; `{"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "2", "remove_faces": [{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,30|a=4000|adj=4,4"}]}`; `DP({"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,30|a=4000|adj=4,4"}}, "offset": "-2")`. Expected: f-dp1 status ok with notice `reference_followed` (follow `Continued`) at `/base/face`, rendered `The top of 'Box 1' selected for Base plane was removed by 'Shell 1', which left its rim in the same place, so the rim is used.`, fixes `KeepMatch` then `Pick`; origin (0,0,28), normal (0,0,1).
19. **The motion a datum passes on.** Bodies `[b-1]`; `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`; `{"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["100", "0", "0"]}`; `DP({"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=100"}})`. Expected: origin (0,0,10), x_axis (1,0,0), normal (0,0,1), as before this change, and `motion` the identity (sliding along the face does not move the datum's frame). With f-mv1 a rotation of 180 deg about the X axis instead (`"rotation": {"axis": {"kind": "origin", "axis": "X"}, "angle": "180 deg"}`, the top now at z = -10 facing -Z): origin (0,0,-10), x_axis (1,0,0), normal (0,0,-1), and `motion` that rotation. With an origin base (test 1), `motion` is the identity; for f-dp2 of test 10, f-dp1's motion.

## Open issues

1. **The motion of a datum on a face.** Resolved. naming.md section 3.6 once said that a sketch on a datum plane passes on "the motion of the face that plane is placed on", which would move a body-frame sketch on a face built from the datum by a slide along the face although that face did not move (the datum's frame takes its x axis from world +X and stays where it was). naming.md section 3.6 now takes the motion the datum passes on, `DatumResult::Plane::motion` of section 3.2 step 4: the face's motion whenever the frame follows it (a face turned over, test 19) and the identity when it does not. Moving the datum's frame with the face instead would change the placement of every version 1 document with a Move body before a datum on a face, which [ADR 0025](../../adr/0025-file-format-version-2.md) rules out.
