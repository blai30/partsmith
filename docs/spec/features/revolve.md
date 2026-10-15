# Feature: revolve

Status: M1 spec. Type name `revolve`. Shared contracts: [features.md](../features.md). Naming rule: [naming.md](../naming.md) section 3.6.3. Kernel operation: [kernel.md](../kernel.md) section 3.4.4.

## 1. Purpose and scope

Revolve sweeps closed regions of one sketch around an axis that lies in the sketch plane and combines the resulting solid with a body.

### In M1

- Profile: one or more closed regions of one sketch, swept as their union (features.md section 3.7, the same union as extrude).
- Axis: an origin axis, a sketch line (construction lines allowed), a straight edge, or the axis of a round face, lying in the sketch plane.
- Extents: `angle` (0 < angle ≤ 360 deg) and `full`.
- `reverse` to revolve in the negative sense.
- Ops `new`, `join`, `cut`, `intersect`.

### Later

- Two-sided and symmetric angles.
- Revolve to a face.
- Axes that do not lie in the sketch plane.
- Thin-wall revolve.
- Revolving body faces instead of sketch regions.

## 2. Public API

```rust
/// Revolves closed sketch regions around an axis and combines the result with a body.
pub struct Revolve;

impl Feature for Revolve {
    const TYPE: &'static str = "revolve";
    type Params = RevolveParams;
}

/// Params of a revolve feature.
pub struct RevolveParams {
    /// Target body.
    pub body: Id,
    /// Closed regions of one sketch.
    pub profile: ProfileRef,
    /// The axis of revolution. Must lie in the sketch plane.
    pub axis: AxisRef,
    /// How far the profile turns.
    pub extent: RevolveExtent,
    /// Turn in the negative sense about the axis direction. Default false, omitted when false.
    pub reverse: bool,
    /// How the solid combines with the body. Required.
    pub op: BodyOp,
}

/// How far a revolve turns.
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RevolveExtent {
    /// Turn by `expr`. Angle, greater than 0 and at most 360 deg.
    Angle { expr: Expr },
    /// A full turn.
    Full,
}
```

JSON:

```json
{"id": "f-rev1", "type": "revolve", "body": "b-1",
 "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=15,15|a=300"}]},
 "axis": {"kind": "origin", "axis": "Z"}, "extent": {"kind": "angle", "expr": "90 deg"}, "op": "new"}
```

Full turn: `"extent": {"kind": "full"}`.

### Manifest

Label `Revolve`, category `create`, icon `revolve`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `profile` | Profile | yes | | `sketch_region` | |
| `axis` | Axis | yes | | `axis` | |
| `extent.kind` | Extent | yes | | | `angle` Angle, `full` Full |
| `extent.expr` | Angle | yes | angle | | |
| `op` | Operation | yes | | | `new` New body, `join` Join, `cut` Cut, `intersect` Intersect |
| `reverse` | Reverse | no | | | |
| `body` | Body | no | | `body` | |

### Python

Dedicated form: `revolve(profile, axis, angle=None, regions=..., reverse=..., op=...)` (python-api.md section 2.7), produced by the `Feature::python` hook with the shared rules of features.md section 3.11. An omitted angle maps to `{"kind": "full"}` and a given angle to `{"kind": "angle", "expr"}`:

- `profile`: as for extrude (the sketch's variable, plus `regions=` when needed).
- `axis`: `PythonValue::axis` (`Axis.Z`, a sketch line's variable, or `Ref(..., part=...)`).
- `angle`: `extent.expr` for an `angle` extent; omitted for `full`.
- `reverse=True` when `reverse` is true; `op` when it differs from the default op of `body`.

With `f-sk1` as `sketch_1` and op `new` as the default, the JSON above is `revolve(sketch_1, Axis.Z, 90*deg, id="f-rev1")`. Generic form:

```python
feature(
    "revolve",
    id="f-rev1",
    body="b-1",
    profile={"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=15,15|a=300"}]},
    axis={"kind": "origin", "axis": "Z"},
    extent={"kind": "angle", "expr": "90 deg"},
    op="new",
)
```

## 3. Behavior

### 3.1 Validation

- `profile.regions` is not empty, else `EmptySelection { field: "profile", noun: region }`.

### 3.2 Dependencies

- `features`: `[profile.sketch]`, plus `axis.sketch` for a `sketch_line` axis in another sketch (no duplicates).
- `bodies_written`: `[body]`. `bodies_read`: the axis body for an `edge` or `face` axis, unless it is `body`.
- `expressions`: `angle`: `("extent.expr", expr, Angle)`; `full`: none.

### 3.3 Build

1. Resolve the profile (`ctx.profile("profile", ...)`) and the axis (`ctx.axis("axis", ...)`, features.md section 3.6). Their warnings and notices go into the output, and the profile match's successions go to naming in step 6. An `edge` or `face` axis keeps the side the user picked: it is oriented by its stored `direction` moved with the part, or else by its fingerprint, never by the sign of its components, so a small edit that tilts the edge never reverses the revolve (corpus R38); every resolved edge must lie on one line, else `NotCollinear`.
2. The axis must lie in the sketch plane: abs(normal · direction) ≤ 1e-9 and the distance from the axis origin to the plane ≤ 1e-6 mm, else `AxisNotInPlane`.
3. The union of the resolved regions must lie in one closed half-plane of the sketch plane bounded by the axis line: its boundary points' signed distances to the axis (measured along normal × direction) are all ≥ -1e-6 mm or all ≤ 1e-6 mm, else `ProfileCrossesAxis`.
4. Angle θ: for `angle`, θ = `extent.expr` in radians; θ > 1e-9 (the angular tolerance, within which the kernel sweeps nothing, kernel.md section 3.4.4) else `OutOfRange` (`GreaterThan(0)`); θ ≤ 2π + 1e-9 else `OutOfRange` (`AtMost(2π)`, rendered `360 deg`). If abs(θ - 2π) ≤ 1e-9 the revolve is treated as `full`.
5. Sense: positive rotation about the axis direction by the right-hand rule; negative when `reverse` is true. A full revolve ignores `reverse`.
6. Kernel `revolve(profile, axis, ±θ)` (2π for full), name (section 3.4) with the profile sketch's motion as the new faces' motion (`SketchResult::motion`, features.md section 3.9), and `apply_op(ctx, body, op, vec![tool], None)`: a revolve has no straight direction, so a cut that changes nothing is `Beside` (features.md section 3.15). For a partial revolve, `reverse_hits` revolves the profile by the opposite angle and checks that it reaches the body, so Turn on Reverse is offered only when it works; a full revolve has no Reverse to offer.
7. **No caps at a full turn.** A full revolve (including an angle within 1e-9 of 360 deg) records `AbsentFace { name: <id>/start-cap, why: FullTurn }` and the same for `<id>/end-cap` in `FeatureOutput::absent` (features.md section 2.7), so a reference to a cap is explained as `is no longer made by 'Revolve 1', which turns all the way round` (corpus R57).

Boundary pieces that lie on the axis generate no face (the solid closes on the axis). As for extrude, the union merges consecutive pieces of one entity that continue the same line or circle before `make_profile`, so a union of regions has the faces of the single region covering the same area.

### 3.4 Element names

| Name | Face |
| --- | --- |
| `<id>/lateral/<entity id>` | Surface of revolution swept by the boundary piece that sketch entity `<entity id>` produced. |
| `<id>/start-cap` | The profile face at angle 0. Partial revolves only. |
| `<id>/end-cap` | The profile face at angle θ. Partial revolves only. |

Successions of the profile match are passed to `NamingMap::build` as `added_successions` of the lateral faces, kept apart from merge aliases (naming.md section 3.6.2), as for extrude ([extrude.md](extrude.md) section 3.4).

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| No regions | `EmptySelection` | `Select at least one region for Profile.` |
| Angle ≤ 0 | `OutOfRange` | `Angle must be greater than 0 deg, but it is 0 deg. Use a value greater than 0 deg.` |
| Angle > 360 deg | `OutOfRange` | `Angle must be at most 360 deg, but it is 400 deg. Use 360 deg or less.` |
| Expression or dimension error | `Expression` | `Angle: Expected an angle, but this expression gives a length.` |
| Axis outside the sketch plane | `AxisNotInPlane` | `The revolve axis does not lie in the sketch plane. Pick a line in the sketch, or an axis that lies in the sketch plane.` |
| A region crosses the axis | `ProfileCrossesAxis` | `The profile crosses the revolve axis. Move the profile to one side of the axis, or pick another axis.` |
| Axis is a curved edge | `NotStraight` | `The top rim of 'Cylinder 1' selected for Axis is not straight. Pick a straight edge, a sketch line or an origin axis.` |
| Axis face has no axis | `NoAxis` | `The top of 'Box 1' selected for Axis has no axis. Pick a round face, a straight edge, a sketch line or an origin axis.` |
| Axis edges or faces not on one line | `NotCollinear` | `The edge selected for Axis now matches 2 edges that do not lie on one line. Pick the one you mean.` |
| Axis sketch line unknown | `UnknownEntity` | `'Sketch 1' no longer has the line picked for Axis. Pick a line of 'Sketch 1' again.` |
| Axis entity is not a line | `NotALine` | `The circle at (50, 50) with radius 1 in 'Sketch 1', picked for Axis, is not a line with length. Pick a line.` |
| Axis edge or face lost | `ReferenceNotFound` | `{Element} {reason}.`, for example `The front left edge of 'Box 1' selected for Axis belongs to 'Box 1', which is suppressed.` (features.md section 4.1) |
| Profile, body, kernel and result errors | as in [extrude.md](extrude.md) section 4 | |
| Warnings and notices | `RegionSplit`, `RegionResized`, `RegionRemapped`, `RegionAmbiguous`, `RegionMissing`, `ReferenceGuessed`, `ReferenceFollowed`, `NoEffect`, `SeparateSolid`, `ClosedVoid` | features.md sections 3.15 and 4.2 |

## 5. Performance

- Op `new` of a 4-curve profile: 5 ms. 50-curve profile: 20 ms.
- With a boolean against a 200-face body: 60 ms.

## 6. Acceptance tests

Conventions from features.md section 6.1. Fixtures:

- `SK-RING`: `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XZ"}, "entities": [{"id": "e-1", "type": "line", "start": [10, 0], "end": [20, 0]}, {"id": "e-2", "type": "line", "start": [20, 0], "end": [20, 30]}, {"id": "e-3", "type": "line", "start": [20, 30], "end": [10, 30]}, {"id": "e-4", "type": "line", "start": [10, 30], "end": [10, 0]}]}`. On XZ, sketch (u, v) is world (u, 0, v), so the rectangle spans x 10..20, z 0..30 at y = 0.
- `P-RING`: `{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=15,15|a=300"}]}`.
- `REV(axis, extent, op)`: `{"id": "f-rev1", "type": "revolve", "body": "b-1", "profile": <P-RING>, "axis": <axis>, "extent": <extent>, "op": <op>}`.
- `AXIS-Z`: `{"kind": "origin", "axis": "Z"}`.

Bodies `[b-1]` unless stated.

1. **Full ring.** `SK-RING`, `REV(AXIS-Z, {"kind": "full"}, "new")`. Expected: ok; volume 28274.333882, 4 faces, bbox [-20,-20,0]-[20,20,30]. Faces: `f-rev1/lateral/e-1` area 942.477796 (plane z=0); `f-rev1/lateral/e-2` area 3769.911184 (cylinder radius 20); `f-rev1/lateral/e-3` area 942.477796 (plane z=30); `f-rev1/lateral/e-4` area 1884.955592 (cylinder radius 10). No `start-cap` or `end-cap`.
2. **Quarter turn.** As 1 with extent `{"kind": "angle", "expr": "90 deg"}`. Expected: volume 7068.583471, 6 faces, bbox [0,0,0]-[20,20,30]; `f-rev1/start-cap` area 300 in plane y=0; `f-rev1/end-cap` area 300 in plane x=0 with y between 10 and 20.
3. **Quarter turn reversed.** As 2 with `"reverse": true`. Expected: volume 7068.583471, bbox [0,-20,0]-[20,0,30]; `end-cap` in plane x=0 with y between -20 and -10.
4. **Bare angle is degrees.** As 2 with expr `"90"`. Expected: identical to 2.
5. **360 deg is full.** As 2 with expr `"360 deg"`. Expected: identical to 1 (4 faces, no caps).
6. **Too large.** As 2 with expr `"400 deg"`. Expected: error `out_of_range` rendered `Angle must be at most 360 deg, but it is 400 deg. Use 360 deg or less.`; b-1 empty.
7. **Zero.** As 2 with expr `"0"`. Expected: error `out_of_range` rendered `Angle must be greater than 0 deg, but it is 0 deg. Use a value greater than 0 deg.`
8. **Wrong dimension.** As 2 with expr `"10 mm"`. Expected: error `expression` rendered `Angle: Expected an angle, but this expression gives a length.`
9. **Profile crosses the axis.** Sketch `RECT(-5, 0, 5, 30)` on XZ, profile its region (fp `c=0,15|a=300`), `REV(AXIS-Z, {"kind": "full"}, "new")`. Expected: error `profile_crosses_axis`.
10. **Profile touches the axis.** Sketch `RECT(0, 0, 10, 30)` on XZ (fp `c=5,15|a=300`), full about Z. Expected: volume 9424.777961, 3 faces, bbox [-10,-10,0]-[10,10,30]; faces `f-rev1/lateral/e-1`, `f-rev1/lateral/e-2`, `f-rev1/lateral/e-3`; no `f-rev1/lateral/e-4` (it lies on the axis).
11. **Sphere from a half disc.** Sketch on XZ with `{"id": "e-1", "type": "arc", "center": [0, 0], "radius": 10, "start_angle": -1.5707963267948966, "end_angle": 1.5707963267948966}` and `{"id": "e-2", "type": "line", "start": [0, 10], "end": [0, -10]}`; profile `{"key": "e-1,e-2", "fp": "c=4.244132,0|a=157.079633"}`; full about Z, op new. Expected: volume 4188.790205, 1 face, bbox [-10,-10,-10]-[10,10,10]; face `f-rev1/lateral/e-1` area 1256.637061.
12. **Axis from a construction line.** `SK-RING` plus `{"id": "e-9", "type": "line", "start": [0, 0], "end": [0, 10], "construction": true}`; axis `{"kind": "sketch_line", "sketch": "f-sk1", "line": "e-9"}`, full. Expected: identical to 1.
13. **Sketch line direction sets the sense.** As 12 with e-9 from [0, 10] to [0, 0] and extent angle `"90 deg"`. Expected: bbox [0,-20,0]-[20,0,30] (the axis points down, so the turn goes toward -Y).
14. **Axis not in the sketch plane.** Sketch `RECT(10, 0, 20, 30)` on XY, axis `AXIS-Z`, full. Expected: error `axis_not_in_plane`.
15. **Revolve about X on XY.** Sketch `RECT(0, 10, 30, 20)` on XY (fp `c=15,15|a=300`), axis `{"kind": "origin", "axis": "X"}`, full, op new. Expected: volume 28274.333882, 4 faces, bbox [0,-20,-20]-[30,20,20].
16. **Axis from a body edge.** Bodies `[b-1, b-2]`; features `{"id": "f-box1", "type": "box", "body": "b-2", "size": ["10", "10", "10"], "position": ["0", "5", "0"], "op": "new"}`, `SK-RING`, `REV` with axis `{"kind": "edge", "body": "b-2", "edge": {"q": "edge(f-box1/front|f-box1/left)", "fp": "line|n=0,0,1|c=-5,0,5|l=10"}}`, full, op new. The edge is x = -5, y = 0, along Z. Expected: b-1 volume 37699.111843, 4 faces, bbox [-30,-25,0]-[20,25,30]; b-2 unchanged; dependencies `bodies_read == ["b-2"]`.
17. **Groove cut.** Features `{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "30", "height": "30", "op": "new"}`, sketch on XZ `RECT(10, 20, 20, 30)` (fp `c=15,25|a=100`), `REV(AXIS-Z, {"kind": "full"}, "cut")` with that profile. Expected: volume 75398.223686, 7 faces, bbox [-30,-30,0]-[30,30,30]; faces `f-rev1/lateral/e-1` (groove floor at z=20), `f-rev1/lateral/e-2` (cylinder radius 20) and `f-rev1/lateral/e-4` (cylinder radius 10) exist.
18. **Intersect.** As 17 with `SK-RING`, `P-RING` and op intersect. Expected: volume 28274.333882, 4 faces, bbox [-20,-20,0]-[20,20,30].
19. **New on a body with a solid.** As 17 with op new. Expected: error `body_not_empty`; b-1 volume 84823.001647 (the cylinder).
20. **Performance.** Test 1 builds in under 5 ms (desktop, release, build only).
21. **Unknown axis line.** `SK-RING`, `REV({"kind": "sketch_line", "sketch": "f-sk1", "line": "e-9"}, {"kind": "full"}, "new")`. Expected: error `unknown_entity`, field `axis`, rendered `'Sketch 1' no longer has the line picked for Axis. Pick a line of 'Sketch 1' again.`; b-1 empty.
22. **Axis entity is not a line.** `SK-RING` plus `{"id": "e-5", "type": "circle", "center": [50, 50], "radius": 1, "construction": true}`, `REV({"kind": "sketch_line", "sketch": "f-sk1", "line": "e-5"}, {"kind": "full"}, "new")`. Expected: error `not_a_line`, rendered `The circle at (50, 50) with radius 1 in 'Sketch 1', picked for Axis, is not a line with length. Pick a line.`
23. **Axis face without an axis.** Bodies `[b-1, b-2]`; `{"id": "f-box1", "type": "box", "body": "b-2", "size": ["10", "10", "10"], "op": "new"}`, `SK-RING`, `REV({"kind": "face", "body": "b-2", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=100"}}, {"kind": "full"}, "new")`. Expected: error `no_axis`, rendered `The top of 'Box 1' selected for Axis has no axis. Pick a round face, a straight edge, a sketch line or an origin axis.`; b-2 unchanged.
24. **Curved axis edge.** Bodies `[b-1, b-2]`; `{"id": "f-cyl1", "type": "cylinder", "body": "b-2", "radius": "5", "height": "10", "op": "new"}`, `SK-RING`, `REV({"kind": "edge", "body": "b-2", "edge": {"q": "edge(f-cyl1/side|f-cyl1/top)", "fp": "circle|n=0,0,1|c=0,0,10|l=31.415927|r=5"}}, {"kind": "full"}, "new")`. Expected: error `not_straight`, rendered `The top rim of 'Cylinder 1' selected for Axis is not straight. Pick a straight edge, a sketch line or an origin axis.`
25. **Union of regions split by a line.** `SK-RING` plus `{"id": "e-5", "type": "line", "start": [15, 0], "end": [15, 30]}`; profile `{"sketch": "f-sk1", "regions": [{"key": "e-1,e-3,e-4,e-5", "fp": "c=12.5,15|a=150"}, {"key": "e-1,e-2,e-3,~e-5", "fp": "c=17.5,15|a=150"}]}`; `REV(AXIS-Z, {"kind": "full"}, "new")` with this profile in place of `P-RING`. Expected: identical to 1 (volume 28274.333882, 4 faces; `f-rev1/lateral/e-1` and `f-rev1/lateral/e-3` area 942.477796 each); no face named `f-rev1/lateral/e-5`.
26. **Angle within the tolerance of 0.** As 2 with expr `"0.00000001 deg"` (1.7e-10 rad). Expected: error `out_of_range`, field `extent.expr`, rendered `Angle must be greater than 0 deg, but it is 0.00000001 deg. Use a value greater than 0 deg.` (the value is written with the digits that tell it apart from the bound, features.md section 3.12); b-1 empty.
27. **No caps at a full turn (corpus R57).** Test 1. Expected: `FeatureOutput::absent` is `[{name: "f-rev1/end-cap", why: FullTurn}, {name: "f-rev1/start-cap", why: FullTurn}]` (in name order); the same for test 5 (360 deg). Test 2: `absent` is empty.
28. **An axis edge that leans keeps its side (corpus R38).** Bodies `[b-1, b-2]`; `LEAN(x)` is `{"id": "f-sk5", "type": "sketch", "plane": {"kind": "origin", "plane": "XZ"}, "entities": [{"id": "e-11", "type": "line", "start": [-6, 0], "end": [-4, 0]}, {"id": "e-12", "type": "line", "start": [-4, 0], "end": [x, 30]}, {"id": "e-13", "type": "line", "start": [x, 30], "end": [-6, 30]}, {"id": "e-14", "type": "line", "start": [-6, 30], "end": [-6, 0]}]}` and `{"id": "f-ext5", "type": "extrude", "body": "b-2", "profile": {"sketch": "f-sk5", "regions": [{"key": "e-11,e-12,e-13,e-14", "fp": "c=-5,15|a=60"}]}, "extent": {"kind": "distance", "expr": "10"}, "op": "new"}` (a wall in y -10..0 whose face at y = 0 lies in the XZ plane); then `SK-RING` and `REV({"kind": "edge", "body": "b-2", "edge": {"q": "edge(f-ext5/lateral/e-12|f-ext5/start-cap)", "fp": "line|n=0,0,1|c=-4,0,15|l=30|adj=2,2"}, "direction": [0, 0, 1]}, {"kind": "angle", "expr": "90 deg"}, "new")`. Expected with `LEAN(-4)`: ok; b-1 bbox [-4,0,0]-[20,24,30] (the axis is +Z through (-4, 0, 0), so the turn goes toward +Y). With `LEAN(-5)` and with `LEAN(-3)` (the wall leans 1 mm either way): ok, the axis direction keeps a positive z (for `LEAN(-5)`, (-0.033315, 0, 0.999445)), so the turn still goes toward +Y: b-1's bounding box has minimum y 0 within 1e-5. The same holds for the axis without `direction` (oriented by its fingerprint's n=0,0,1).
29. **A 180 degree turn of the part turns the axis.** Test 16's document with `{"kind": "angle", "expr": "90 deg"}` in place of `full`, the axis with `"direction": [0, 0, 1]`, and `{"id": "f-mv1", "type": "move_body", "body": "b-2", "rotation": {"axis": {"kind": "origin", "axis": "X"}, "angle": "180 deg"}}` inserted before the revolve (the box now spans y -10..0, z -10..0; the edge still lies at x = -5, y = 0, and its stored direction moves with the part to -Z). Expected: ok; b-1 bbox [-5,-25,0]-[20,0,30] (the turn goes toward -Y). The same document without `direction` (as written before format version 2) keeps the fingerprint's side, +Z: b-1 bbox [-5,0,0]-[20,25,30], as before this change.
30. **Axis edges not on one line.** Bodies `[b-1, b-2]`; b-2 holds the L-shaped body of [fillet.md](fillet.md) test 4 (`f-box1` and `f-box2` with `"body": "b-2"`); `SK-RING`; `REV({"kind": "edge", "body": "b-2", "edge": {"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=-10,-10,10|l=20", "merged": {"f-box1/top": ["f-box2/top"]}}}, {"kind": "full"}, "new")`. Expected: error `not_collinear`, field `axis`, rendered `The edge selected for Axis now matches 2 edges that do not lie on one line. Pick the one you mean.`, fixes `UseReferences` with `edge(f-box1/front|f-box1/top)`, `UseReferences` with `edge(f-box1/front|f-box2/top)`, then `Pick`; b-1 empty. Neither edge is chosen by name order.
31. **A side line deleted and drawn again (corpus R06a for revolve).** `SK-RING` and `REV(AXIS-Z, {"kind": "full"}, "new")` with `P-RING-V2` in place of `P-RING` (the region as a pick stores it since file format version 2, with `"outline": "e-1,e-2,e-3,e-4"`), and `{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{"q": "edge(f-rev1/lateral/e-2|f-rev1/lateral/e-3)", "fp": "circle|n=0,0,1|c=0,0,30|l=125.663706|r=20|adj=2,1"}], "radius": "2"}` (the outer top rim). Edit: delete `e-2` from f-sk1 and add `{"id": "e-9", "type": "line", "start": [20, 0], "end": [20, 30]}`. Expected: the profile matches by `Boundary` with the succession `e-2 -> [e-9]`, so f-rev1 is ok with no region message, its outer side is `f-rev1/lateral/e-9`, and b-1's naming map records the succession `f-rev1/lateral/e-2 -> [f-rev1/lateral/e-9]` (as for extrude, [extrude.md](extrude.md) test 47); f-fil1 status ok with notice `reference_followed` (follow `Replaced`) at `/edges/0`, fixes `KeepMatch { pointer: "/edges/0" }` then `Pick`; the fillet face `f-fil1/fillet/edge(f-rev1/lateral/e-3|f-rev1/lateral/e-9)` exists; b-1 volume 28168.872718.

## Open issues

1. Requiring the axis to lie in the sketch plane follows Fusion and the kernel facade (kernel.md section 3.4.4). Revolving around an axis parallel to the plane but outside it is valid geometry and can be added later.
