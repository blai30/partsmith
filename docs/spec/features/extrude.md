# Feature: extrude

Status: M1 spec. Type name `extrude`. Shared contracts: [features.md](../features.md).

## 1. Purpose and scope

Extrude sweeps closed regions of one sketch along the sketch plane normal and combines the resulting solid with a body.

### In M1

- Profile: one or more closed regions of one sketch (regions with holes allowed).
- Extents: `distance` (one side), `symmetric` (centered on the sketch plane), `two_sides` (independent distances on each side), `through_all` (through the whole target body in one direction).
- `reverse` to extrude against the sketch normal.
- Ops `new`, `join`, `cut`, `intersect` with the shared body semantics.

### Later

- Extent to a face, body or vertex.
- Start from an offset or from a face instead of the sketch plane.
- Taper (draft) angle.
- Thin-wall extrusion of open or closed profiles.
- Through all in both directions.
- Extrusion along a direction other than the sketch normal.
- Extruding body faces instead of sketch regions.
- Profiles that mix regions of several sketches.
- Surface extrusion of open profiles.

## 2. Public API

```rust
/// Extrudes closed sketch regions along the sketch normal and combines the result with a body.
pub struct Extrude;

impl Feature for Extrude {
    const TYPE: &'static str = "extrude";
    type Params = ExtrudeParams;
    // manifest, dependencies, build, validate, python_arguments as specified below
}

/// Params of an extrude feature.
pub struct ExtrudeParams {
    /// Target body.
    pub body: Id,
    /// Closed regions of one sketch.
    pub profile: ProfileRef,
    /// How far the extrusion goes.
    pub extent: ExtrudeExtent,
    /// Extrude against the sketch normal. Default false, omitted when false.
    pub reverse: bool,
    /// How the extrusion combines with the body. Required.
    pub op: BodyOp,
}

/// How far an extrusion goes, measured along its direction from the sketch plane.
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExtrudeExtent {
    /// From the sketch plane to `expr` along the direction. Length, greater than 0.
    Distance { expr: Expr },
    /// Total length `expr`, centered on the sketch plane. Length, greater than 0.
    Symmetric { expr: Expr },
    /// `expr` along the direction and `opposite_expr` against it. Lengths, at least 0, not both 0.
    TwoSides { expr: Expr, opposite_expr: Expr },
    /// From the sketch plane through the whole target body along the direction.
    ThroughAll,
}
```

JSON (canonical key order):

```json
{"id": "f-ext1", "type": "extrude", "body": "b-1",
 "profile": {"sketch": "f-sk1", "regions": [{"key": [["e-1", "e-2", "e-3", "e-4"]], "fp": "region|a=4000|c=40,25"}]},
 "extent": {"kind": "distance", "expr": "30 mm"}, "op": "new"}
```

Other extents: `{"kind": "symmetric", "expr": "30 mm"}`, `{"kind": "two_sides", "expr": "20 mm", "opposite_expr": "10 mm"}`, `{"kind": "through_all"}`. With reverse: `"reverse": true` between `extent` and `op`.

### Manifest

Label `Extrude`, category `create`, icon `extrude`, version 1. Fields in display order:

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `profile` | Profile | yes | | `sketch_region` | |
| `extent.kind` | Extent | yes | | | `distance` Distance, `symmetric` Symmetric, `two_sides` Two sides, `through_all` Through all |
| `extent.expr` | Distance | yes | length | | |
| `extent.opposite_expr` | Opposite distance | yes | length | | |
| `op` | Operation | yes | | | `new` New body, `join` Join, `cut` Cut, `intersect` Intersect |
| `reverse` | Reverse | no | | | |
| `body` | Body | no | | `body` | |

### Python

`extrude(id=..., [name=...], [suppressed=True], body=..., profile=..., extent=..., [reverse=True], op=...)`. Example for the JSON above:

```python
extrude(
    id="f-ext1",
    body="b-1",
    profile={"sketch": "f-sk1", "regions": [{"key": [["e-1", "e-2", "e-3", "e-4"]], "fp": "region|a=4000|c=40,25"}]},
    extent={"kind": "distance", "expr": "30 mm"},
    op="new",
)
```

## 3. Behavior

### 3.1 Validation

- `profile.regions` is not empty, else `EmptySelection { field: "profile", noun: region }`.

### 3.2 Dependencies

- `features`: `[profile.sketch]`.
- `bodies_written`: `[body]`. `bodies_read`: empty.
- `expressions`: `distance` and `symmetric`: `("extent.expr", expr, Length)`; `two_sides`: `("extent.expr", expr, Length)`, `("extent.opposite_expr", opposite_expr, Length)`; `through_all`: none.

### 3.3 Build

1. Resolve the profile with `ctx.profile("profile", ...)`. Region warnings go into the output.
2. Direction d: the sketch frame normal; its negation when `reverse` is true and the extent is not `symmetric`. A symmetric extrusion ignores `reverse` (the solid and its names are the same either way).
3. Interval [s0, s1] along d, measured from the sketch plane, checked in this order:
   - `distance`: L = `extent.expr`; L > 0 else `OutOfRange` (`GreaterThan(0)`); interval [0, L].
   - `symmetric`: L > 0 likewise; interval [-L/2, L/2].
   - `two_sides`: a = `extent.expr` ≥ 0 and b = `extent.opposite_expr` ≥ 0, else `OutOfRange` (`AtLeast(0)`) on that field; a + b > 0 else `BothSidesZero`; interval [-b, a].
   - `through_all`: op `new` gives `ThroughAllNeedsBody`. An empty body gives `BodyEmpty { use_new: true }`. Otherwise let D be the largest signed distance (p - frame origin) · d over the eight corners p of the body's tight bounding box. D ≤ 0 gives `ThroughAllMisses`. Interval [0, D + 1 mm].
4. Build one face per resolved region in the sketch frame (holes as inner loops), translate it by s0·d and extrude it by (s1 - s0)·d. Several regions are fused into one tool with one multi-argument boolean and refined.
5. Name the tool's faces (section 3.4) and call `apply_op(ctx, body, op, tool)`.

### 3.4 Element names

| Name | Face |
| --- | --- |
| `<id>/start-cap` | Cap at s0 (the end of the interval with the smaller coordinate along d). |
| `<id>/end-cap` | Cap at s1. |
| `<id>/lateral/<entity id>` | Side face swept from the boundary edge that sketch entity `<entity id>` produced. |

With several regions, each region contributes caps named `start-cap` and `end-cap`; caps that refine merges into one face follow the naming crate's merge rule, and separate caps are numbered with `#n`. An entity that contributes several separate boundary pieces gets `#n` parts. Faces of the body keep their names; caps or sides that end up inside the body after a join or cut disappear.

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| No regions in the profile | `EmptySelection` | `Select at least one region for Profile.` |
| Distance or symmetric length ≤ 0 | `OutOfRange` | `Distance must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| A two-sides length < 0 | `OutOfRange` | `Opposite distance must be at least 0 mm, but it is -1 mm. Use 0 mm or more.` |
| Both sides 0 | `BothSidesZero` | `Both sides of the extrusion are 0. Give at least one side a distance.` |
| Length is not a length | `WrongDimension` | `Distance must be a length, but '30 deg' is an angle. Use a value such as 10 mm.` |
| Expression error | `Expression` | `Distance: {model message}` |
| Through all with op new | `ThroughAllNeedsBody` | `Through all needs a body that already has a solid. Use a distance when the op is new.` |
| Through all points away | `ThroughAllMisses` | `Through all points away from body 'Body 1', so there is nothing to go through. Turn on Reverse, or use a distance.` |
| Profile regions all missing | `ProfileNotFound` | `None of the selected regions of 'Sketch 1' can be found anymore. Re-pick the profile.` |
| Profile sketch is not a sketch | `WrongFeatureType` | `'Datum plane 1' is not a sketch. Pick a sketch for Profile.` |
| Body rules | `BodyNotEmpty`, `BodyEmpty`, `EmptyResult` | features.md section 4.1 |
| Kernel failure building faces or the prism | `Kernel { operation: Profile or Extrude }` | features.md section 4.1 |
| Kernel failure in the boolean | `Kernel { operation: Boolean }` | features.md section 4.1 |
| Invalid result | `InvalidGeometry` | features.md section 4.1 |
| Region remapped or missing | warning `RegionRemapped`, `RegionMissing` | features.md section 4.2 |
| Volume unchanged | warning `NoEffect` | `This feature does not change body 'Body 1'. Check its position, size and direction.` |

Unknown, later, failed or suppressed sketches are reported by recompute (`UnknownFeature`, `ForwardReference`, `UpstreamFailed`, `UpstreamSuppressed`).

## 5. Performance

- Op `new` with a 4-curve profile: 3 ms. With a 50-curve profile: 15 ms.
- Join or cut against a 200-face body: 60 ms.
- Profile with 20 regions: 40 ms.

## 6. Acceptance tests

Conventions from features.md section 6.1. Fixtures:

- `SK-RECT`: `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}, {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]}, {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]}, {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}], "constraints": []}`.
- `P-RECT`: `{"sketch": "f-sk1", "regions": [{"key": [["e-1", "e-2", "e-3", "e-4"]], "fp": "region|a=4000|c=40,25"}]}`.
- `SK-CIRCLE(cx, cy, r)`: `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-5", "type": "circle", "center": [cx, cy], "radius": r}], "constraints": []}`.
- `P-CIRCLE`: `{"sketch": "f-sk1", "regions": [{"key": [["e-5"]], "fp": "region|a=<πr²>|c=<cx>,<cy>"}]}`.
- `BOX-BELOW`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "position": ["0", "0", "-10"], "op": "new"}` (spans [-20,-20,-10]-[20,20,0], volume 16000).
- `EXT(extent, op)`: `{"id": "f-ext1", "type": "extrude", "body": "b-1", "profile": <profile>, "extent": <extent>, "op": <op>}`; `reverse` added where stated.

Unless stated, bodies are `[b-1]` and the features are a sketch then the extrude.

1. **Distance, new.** `SK-RECT`, `EXT({"kind": "distance", "expr": "30 mm"}, "new")` with `P-RECT`. Expected: ok; b-1 volume 120000, 6 faces, bbox [0,0,0]-[80,50,30]. Faces by name: `f-ext1/start-cap` area 4000, plane z=0, outward normal (0,0,-1); `f-ext1/end-cap` area 4000, plane z=30; `f-ext1/lateral/e-1` area 2400, plane y=0; `f-ext1/lateral/e-2` area 1500, plane x=80; `f-ext1/lateral/e-3` area 2400, plane y=50; `f-ext1/lateral/e-4` area 1500, plane x=0. `BodyUpdate.change` is `New`.
2. **Reverse.** As 1 with `"reverse": true`. Expected: volume 120000, 6 faces, bbox [0,0,-30]-[80,50,0]; `f-ext1/start-cap` at z=0, `f-ext1/end-cap` at z=-30.
3. **Symmetric.** As 1 with extent `{"kind": "symmetric", "expr": "30 mm"}`. Expected: volume 120000, 6 faces, bbox [0,0,-15]-[80,50,15]; `start-cap` at z=-15, `end-cap` at z=15.
4. **Symmetric ignores reverse.** As 3 with `"reverse": true`. Expected: identical to 3, including which face is `start-cap`; status ok (no warning).
5. **Two sides.** As 1 with extent `{"kind": "two_sides", "expr": "20 mm", "opposite_expr": "10 mm"}`. Expected: volume 120000, bbox [0,0,-10]-[80,50,20]; `start-cap` at z=-10, `end-cap` at z=20.
6. **Two sides reversed.** As 5 with `"reverse": true`. Expected: bbox [0,0,-20]-[80,50,10]; `start-cap` at z=10, `end-cap` at z=-20.
7. **Two sides, one side zero.** Extent `{"kind": "two_sides", "expr": "20", "opposite_expr": "0"}`. Expected: volume 80000, bbox [0,0,0]-[80,50,20].
8. **Two sides both zero.** Extent `{"kind": "two_sides", "expr": "0", "opposite_expr": "0"}`. Expected: error `both_sides_zero`; b-1 empty.
9. **Zero distance.** Extent distance `"0"`. Expected: error `out_of_range`, field `extent.expr`, rendered `Distance must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.`
10. **Negative distance.** Extent distance `"-5"`. Expected: error `out_of_range` rendered `Distance must be greater than 0 mm, but it is -5 mm. Use a value greater than 0 mm.`
11. **Negative opposite distance.** Extent `{"kind": "two_sides", "expr": "5", "opposite_expr": "-1"}`. Expected: error `out_of_range`, field `extent.opposite_expr`, rendered `Opposite distance must be at least 0 mm, but it is -1 mm. Use 0 mm or more.`
12. **Wrong dimension.** Extent distance `"30 deg"`. Expected: error `wrong_dimension` rendered `Distance must be a length, but '30 deg' is an angle. Use a value such as 10 mm.`
13. **Bare number in inches.** Document `"units": "in"`, extent distance `"1"`. Expected: volume 101600, bbox [0,0,0]-[80,50,25.4].
14. **Parameter.** Parameters `[{"id": "p-h", "name": "height", "expr": "12 mm"}]`, extent distance `"height * 2"`. Expected: volume 96000, bbox [0,0,0]-[80,50,24].
15. **Unknown name.** Extent distance `"hieght * 2"` with the parameter of 14. Expected: error `expression`, field `extent.expr`; b-1 empty.
16. **Circle.** `SK-CIRCLE(0, 0, 10)`, `P-CIRCLE` with fp `region|a=314.159265|c=0,0`, extent distance `"20"`, op new. Expected: volume 6283.185307, 3 faces, bbox [-10,-10,0]-[10,10,20]; `f-ext1/lateral/e-5` area 1256.637061; `start-cap` and `end-cap` area 314.159265 each.
17. **Region with a hole.** Sketch `SK-RECT` plus entity `{"id": "e-5", "type": "circle", "center": [40, 25], "radius": 10}`; profile region key `[["e-1", "e-2", "e-3", "e-4"], ["e-5"]]`, fp `region|a=3685.840735|c=40,25`; extent distance `"10"`, op new. Expected: volume 36858.407346, 7 faces, bbox [0,0,0]-[80,50,10]; `f-ext1/lateral/e-5` area 628.318531.
18. **Two regions fused.** As 17 with a second region `{"key": [["e-5"]], "fp": "region|a=314.159265|c=40,25"}`. Expected: volume 40000, 6 faces, bbox [0,0,0]-[80,50,10]; no face named `f-ext1/lateral/e-5`.
19. **Join.** Features `BOX-BELOW`, `SK-CIRCLE(0, 0, 5)`, `EXT({"kind": "distance", "expr": "5"}, "join")` with `P-CIRCLE` (fp `region|a=78.539816|c=0,0`). Expected: ok; volume 16392.699082, 8 faces, bbox [-20,-20,-10]-[20,20,5]; faces `f-box1/top`, `f-ext1/lateral/e-5` and `f-ext1/end-cap` exist; no `f-ext1/start-cap`; change `Join`.
20. **Pocket cut.** As 19 with extent distance `"4"`, `"reverse": true`, op cut. Expected: volume 15685.840735, 8 faces, bbox [-20,-20,-10]-[20,20,0]; `f-ext1/lateral/e-5` (pocket wall) and `f-ext1/end-cap` (pocket floor, plane z=-4) exist; no `f-ext1/start-cap`; change `Cut`.
21. **Through all cut.** As 19 with extent `{"kind": "through_all"}`, `"reverse": true`, op cut. Expected: volume 15214.601837, 7 faces; `f-ext1/lateral/e-5` exists; no cap names.
22. **Through all pointing away.** As 21 without `reverse`. Expected: error `through_all_misses` rendered `Through all points away from body 'Body 1', so there is nothing to go through. Turn on Reverse, or use a distance.`; b-1 volume 16000.
23. **Through all with op new.** `SK-RECT`, `EXT({"kind": "through_all"}, "new")`. Expected: error `through_all_needs_body`.
24. **Intersect.** Features `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "op": "new"}`, `SK-CIRCLE(0, 0, 10)`, `EXT({"kind": "symmetric", "expr": "100"}, "intersect")` with `P-CIRCLE`. Expected: volume 3141.592654, 3 faces, bbox [-10,-10,0]-[10,10,10]; change `Intersect`.
25. **Intersect without overlap.** As 24 with the circle at center [100, 0], radius 5. Expected: error `empty_result` rendered `The shapes do not overlap body 'Body 1', so the intersection is empty. Move them so they overlap the body.`; b-1 volume 16000.
26. **Cut everything.** Features `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`, a sketch `RECT(-50, -50, 50, 50)` on XY with ids e-1..e-4, `EXT({"kind": "symmetric", "expr": "100"}, "cut")`. Expected: error `empty_result` rendered `This cut removes all of body 'Body 1'. Make the cut smaller, or change its position or direction.`; b-1 volume 1000.
27. **Cut that misses.** As 20 with the circle at center [100, 0]. Expected: warning `no_effect`; b-1 volume 16000, 6 faces, element names unchanged.
28. **New on a body with a solid.** `BOX-BELOW`, `SK-RECT`, `EXT({"kind": "distance", "expr": "30"}, "new")`. Expected: error `body_not_empty`; b-1 volume 16000.
29. **Join on an empty body.** `SK-RECT`, `EXT({"kind": "distance", "expr": "30"}, "join")`. Expected: error `body_empty` rendered `Body 'Body 1' is empty at this point. Use op new to start it.`
30. **Remapped region.** As 1 with region key `[["e-1", "e-2", "e-3", "e-9"]]`. Expected: warning `region_remapped` at position 0; volume 120000.
31. **One region missing.** As 1 with a second region `{"key": [["e-7", "e-8"]], "fp": "region|a=100|c=200,200"}`. Expected: warning `region_missing` at position 1; volume 120000.
32. **All regions missing.** As 1 with the only region key `[["e-7", "e-8"]]`. Expected: error `profile_not_found` rendered `None of the selected regions of 'Sketch 1' can be found anymore. Re-pick the profile.`
33. **Profile from a datum.** Features `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}}`, `EXT(...)` with profile sketch `f-dp1` and the region of `P-RECT`. Expected: error `wrong_feature_type` rendered `'Datum plane 1' is not a sketch. Pick a sketch for Profile.`
34. **Sketch later in the timeline.** Test 1 with the two features in reverse order. Expected: extrude error `forward_reference` rendered `'Sketch 1' comes later in the timeline than this feature. Move it above this feature, or pick something earlier for Profile.`; sketch ok.
35. **Sketch on XZ.** As 1 with the sketch plane `{"kind": "origin", "plane": "XZ"}`. Expected: volume 120000, bbox [0,-30,0]-[80,0,50].
36. **Sketch on a datum.** Features `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "10"}`, `SK-RECT` with plane `{"kind": "datum", "feature": "f-dp1"}`, extrude as 1. Expected: bbox [0,0,10]-[80,50,40].
37. **Empty profile.** As 1 with `"regions": []`. Expected: error `empty_selection` rendered `Select at least one region for Profile.`
38. **Python, two sides.** `Registry::python` of `{"id": "f-ext2", "type": "extrude", "body": "b-1", "profile": <P-RECT>, "extent": {"kind": "two_sides", "expr": "20", "opposite_expr": "10"}, "reverse": true, "op": "cut"}` renders:

    ```python
    extrude(
        id="f-ext2",
        body="b-1",
        profile={"sketch": "f-sk1", "regions": [{"key": [["e-1", "e-2", "e-3", "e-4"]], "fp": "region|a=4000|c=40,25"}]},
        extent={"kind": "two_sides", "expr": 20, "opposite_expr": 10},
        reverse=True,
        op="cut",
    )
    ```

39. **Performance.** Test 1 builds in under 3 ms (desktop, release, build only); test 21 in under 30 ms.

## Open issues

1. Through all extends 1 mm past the body's bounding box. The margin only has to exceed the modeling tolerance; 1 mm keeps the tool's cap clear of the body even for curved faces near the box corners.
2. A symmetric extrusion silently ignores `reverse`. The UI should hide Reverse for the symmetric extent.
