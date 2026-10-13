# Feature: extrude

Status: M1 spec. Type name `extrude`. Shared contracts: [features.md](../features.md). Naming rule: [naming.md](../naming.md) section 3.6.2. Kernel operation: [kernel.md](../kernel.md) section 3.4.3.

## 1. Purpose and scope

Extrude sweeps closed regions of one sketch along the sketch plane normal and combines the resulting solid with a body.

### In M1

- Profile: one or more closed regions of one sketch (regions with holes allowed); several regions are swept as their union.
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
 "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]},
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

Dedicated form: `extrude(profile, distance, regions=..., symmetric=..., reverse=..., through_all=..., op=...)` (python-api.md section 2.7), produced by the `Feature::python` hook with the shared rules of features.md section 3.11:

- `profile`: the sketch's variable, plus `regions=[Region("<key>", "<fp>"), ...]` when the stored regions are not all of the sketch's regions in solved order.
- `distance` extent: `distance` is `extent.expr`. `symmetric`: the same plus `symmetric=True`. `through_all`: no `distance`, and `through_all=True`.
- `reverse=True` when `reverse` is true; `op` when it differs from the default op of `body`.
- A `two_sides` extent has no dedicated form: the hook returns `None` and the generic form is used.

With `f-sk1` as `sketch_1` and op `new` as the default, the JSON above is `extrude(sketch_1, 30*mm, id="f-ext1")`. Generic form (features.md section 3.11):

```python
feature(
    "extrude",
    id="f-ext1",
    body="b-1",
    profile={"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]},
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

1. Resolve the profile with `ctx.profile("profile", ...)` (features.md section 3.7). Region warnings and notices go into the output, and the match's successions (`ResolvedProfile::successions`) go to naming in step 5.
2. Direction d: the sketch placement normal; its negation when `reverse` is true and the extent is not `symmetric`. A symmetric extrusion ignores `reverse` (the solid and its names are the same either way).
3. Interval [s0, s1] along d, measured from the sketch plane, checked in this order with the positive length rule of features.md section 3.4:
   - `distance`: L = `extent.expr`; L > 0 (at least `LINEAR_TOLERANCE`) else `OutOfRange` (`GreaterThan(0)`); then L at most the modeling limit, else `OutOfRange` (`AtMost(MODEL_LIMIT)`, features.md section 3.4), as for every length below; interval [0, L].
   - `symmetric`: L > 0 likewise; interval [-L/2, L/2].
   - `two_sides`: a = `extent.expr` ≥ 0 and b = `extent.opposite_expr` ≥ 0, else `OutOfRange` (`AtLeast(0)`) on that field; a + b > 0 (at least `LINEAR_TOLERANCE`) else `BothSidesZero`; interval [-b, a].
   - `through_all`: op `new` gives `ThroughAllNeedsBody { body }`. An empty body gives `BodyEmpty { field: body, body, use_new: true }`. Otherwise let D be the largest signed distance (p - placement origin) · d over the eight corners p of the body's tight bounding box. D ≤ 0 gives `ThroughAllMisses { body, reverse_hits }`, where `reverse_hits` is true when the same test along -d gives D' > 0 and the tool swept along -d over [0, D' + 1 mm] shares more than `VOLUME_TOLERANCE` times the body's volume with it (one kernel `common`, on this error path only); only then is the fix Turn on Reverse offered. Interval [0, D + 1 mm].
4. Build the union of the resolved regions as kernel faces (features.md section 3.7: shared pieces dropped, consecutive pieces of one entity that continue the same line or circle merged, then `make_profile` with curves tagged by sketch entity) and sweep it with kernel `extrude(profile, d, s0, s1)`.
5. Name the tool's faces (section 3.4), with the profile sketch's motion as the new faces' motion (`SketchResult::motion`, features.md section 3.9), and call `apply_op(ctx, body, op, vec![tool], Some(Direction { direction: d, sketch: Some(&profile.sketch) }))`. When the result changes nothing or leaves a separate piece, `apply_op` reports why (features.md section 3.15: `NoEffect` with `Inside`, `Away`, `Short`, `Beside` or `Covers`, `SeparateSolid`), and the extrude answers `reverse_hits(ctx, body, tools)` by sweeping the same profile along -d over the same interval, so Turn on Reverse is offered only when it reaches the body.
6. **Ends outside the body.** With op `cut`, an `end-cap` (or `end-cap#n`) of the tool that is not in the result records `AbsentFace { name, why: Through }` in `FeatureOutput::absent` (features.md section 2.7): the cut goes all the way through, so a reference to its floor is explained as `is no longer made by 'Extrude 2', which goes all the way through the body` and never moved to a rim (corpus R12).

### 3.4 Element names

| Name | Face |
| --- | --- |
| `<id>/start-cap` | Cap at s0 (on the sketch plane for a one-sided distance; on the side opposite d for symmetric and two sides). |
| `<id>/end-cap` | Cap at s1. |
| `<id>/lateral/<entity id>` | Side face swept from the boundary piece that sketch entity `<entity id>` produced. |

Successions of the profile match become aliases of the lateral faces (naming.md section 3.6.2): when a line of the sketch is deleted and drawn again in its place, extended by a collinear line, or replaced by several lines, the faces made from the new entities carry the old entity's face name too, so references to it follow with a notice or a warning (features.md section 3.8, tests 47 to 50).

Several separate caps (disjoint regions) are numbered `start-cap#n` and `end-cap#n`; an entity that still contributes several separate boundary pieces after the union gives `lateral/<entity id>#n`, both in naming.md's geometric order. Boundaries shared by two selected regions produce no face, and the pieces of one entity on either side of a dropped boundary give one face, so a union of regions has the faces of the single region covering the same area. Faces of the body keep their names; caps or sides that end up inside the body after a join or cut disappear.

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| No regions in the profile | `EmptySelection` | `Select at least one region for Profile.` |
| Distance or symmetric length ≤ 0 | `OutOfRange` | `Distance must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| A length above the modeling limit | `OutOfRange` | `Distance must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less.` |
| A two-sides length < 0 | `OutOfRange` | `Opposite distance must be at least 0 mm, but it is -1 mm. Use 0 mm or more.` |
| Both sides 0 (their sum below 1e-6 mm) | `BothSidesZero` | `Both sides of the extrusion are 0. Give at least one side a distance.` |
| Expression or dimension error | `Expression` | `Distance: Expected a length, but this expression gives an angle.` |
| Through all with op new | `ThroughAllNeedsBody` | `Through all goes through an existing solid, and body 'Body 1' has none here. Use a distance instead.` |
| Through all points away | `ThroughAllMisses` | reversing reaches the body: `Through all points away from body 'Body 1', so there is nothing to go through.` with the fix Turn on Reverse; otherwise the same followed by ` Point it at the body, or use a distance.` |
| Profile regions all missing | `ProfileNotFound` | `The region of 'Sketch 1' selected for Profile is gone because its outline was drawn again.` (features.md section 4.1) |
| Profile sketch is not a sketch | `WrongFeatureType` | `'Datum plane 1' is not a sketch. Pick a sketch for Profile.` |
| Body rules | `BodyNotEmpty`, `BodyEmpty`, `EmptyResult` | features.md section 4.1 |
| Kernel failure | `Kernel` | `Partsmith's geometry kernel could not build this extrude. {hint}`, with the kernel's hint (kernel.md section 4.3) and its text in the detail |
| Region split, resized, remapped, ambiguous or missing | notice `RegionSplit`, warnings `RegionResized`, `RegionRemapped`, `RegionAmbiguous`, `RegionMissing` | features.md section 4.2 |
| Join inside the body, cut beside, short of or away from it, intersection covering it | warning `NoEffect` | features.md section 3.15, for example `This cut does not touch body 'Body 1': it starts 10 mm from the body and points away from it.` |
| Join that does not touch the body | warning `SeparateSolid` | `This extrude does not touch body 'Body 1', so the body now has 2 separate pieces.` |
| Join that closes a hollow | warning `ClosedVoid` | features.md section 4.2 |

Unknown, later, failed or suppressed sketches are reported by recompute (`UnknownFeature`, `ForwardReference`, `UpstreamFailed`, `UpstreamSuppressed`).

## 5. Performance

- Op `new` with a 4-curve profile: 3 ms. With a 50-curve profile: 15 ms.
- Join or cut against a 200-face body: 60 ms.
- Profile with 20 regions: 40 ms.

## 6. Acceptance tests

Conventions from features.md section 6.1. Fixtures:

- `SK-RECT`: `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}, {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]}, {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]}, {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}]}`.
- `P-RECT`: `{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]}`.
- `SK-CIRCLE(cx, cy, r)`: `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-5", "type": "circle", "center": [cx, cy], "radius": r}]}`.
- `P-CIRCLE(fp)`: `{"sketch": "f-sk1", "regions": [{"key": "e-5", "fp": <fp>}]}`.
- `BOX-BELOW`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "position": ["0", "0", "-10"], "op": "new"}` (spans [-20,-20,-10]-[20,20,0], volume 16000).
- `EXT(profile, extent, op)`: `{"id": "f-ext1", "type": "extrude", "body": "b-1", "profile": <profile>, "extent": <extent>, "op": <op>}`; `reverse` added where stated.

Unless stated, bodies are `[b-1]` and the features are `SK-RECT` then `EXT(P-RECT, ...)`.

1. **Distance, new.** `EXT(P-RECT, {"kind": "distance", "expr": "30 mm"}, "new")`. Expected: ok; b-1 volume 120000, 6 faces, bbox [0,0,0]-[80,50,30]. Faces by name: `f-ext1/start-cap` area 4000, plane z=0, outward normal (0,0,-1); `f-ext1/end-cap` area 4000, plane z=30; `f-ext1/lateral/e-1` area 2400, plane y=0; `f-ext1/lateral/e-2` area 1500, plane x=80; `f-ext1/lateral/e-3` area 2400, plane y=50; `f-ext1/lateral/e-4` area 1500, plane x=0. `BodyUpdate.change` is `New`.
2. **Reverse.** As 1 with `"reverse": true`. Expected: volume 120000, 6 faces, bbox [0,0,-30]-[80,50,0]; `f-ext1/start-cap` at z=0, `f-ext1/end-cap` at z=-30.
3. **Symmetric.** Extent `{"kind": "symmetric", "expr": "30 mm"}`. Expected: volume 120000, 6 faces, bbox [0,0,-15]-[80,50,15]; `start-cap` at z=-15, `end-cap` at z=15.
4. **Symmetric ignores reverse.** As 3 with `"reverse": true`. Expected: identical to 3, including which face is `start-cap`; status ok.
5. **Two sides.** Extent `{"kind": "two_sides", "expr": "20 mm", "opposite_expr": "10 mm"}`. Expected: volume 120000, bbox [0,0,-10]-[80,50,20]; `start-cap` at z=-10, `end-cap` at z=20.
6. **Two sides reversed.** As 5 with `"reverse": true`. Expected: bbox [0,0,-20]-[80,50,10]; `start-cap` at z=10, `end-cap` at z=-20.
7. **Two sides, one side zero.** Extent `{"kind": "two_sides", "expr": "20", "opposite_expr": "0"}`. Expected: volume 80000, bbox [0,0,0]-[80,50,20].
8. **Two sides both zero.** Extent `{"kind": "two_sides", "expr": "0", "opposite_expr": "0"}`. Expected: error `both_sides_zero`; b-1 empty.
9. **Zero distance.** Extent distance `"0"`. Expected: error `out_of_range`, field `extent.expr`, rendered `Distance must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.`
10. **Negative distance.** Extent distance `"-5"`. Expected: error `out_of_range` rendered `Distance must be greater than 0 mm, but it is -5 mm. Use a value greater than 0 mm.`
11. **Negative opposite distance.** Extent `{"kind": "two_sides", "expr": "5", "opposite_expr": "-1"}`. Expected: error `out_of_range`, field `extent.opposite_expr`, rendered `Opposite distance must be at least 0 mm, but it is -1 mm. Use 0 mm or more.`
12. **Wrong dimension.** Extent distance `"30 deg"`. Expected: error `expression`, field `extent.expr`, rendered `Distance: Expected a length, but this expression gives an angle.`
13. **Bare number in inches.** Document `"units": "in"`, extent distance `"1"`. Expected: volume 101600, bbox [0,0,0]-[80,50,25.4] (sketch coordinates are millimeters in every document).
14. **Parameter.** Parameters `[{"id": "p-h", "name": "height", "expr": "12 mm"}]`, extent distance `"height * 2"`. Expected: volume 96000, bbox [0,0,0]-[80,50,24].
15. **Unknown name.** As 14 with extent distance `"hieght * 2"`. Expected: error `expression`, field `extent.expr`, rendered `Distance: Unknown name 'hieght'. Check the spelling, or add a parameter named hieght.`; b-1 empty.
16. **Circle.** `SK-CIRCLE(0, 0, 10)`, `EXT(P-CIRCLE("c=0,0|a=314.159265"), {"kind": "distance", "expr": "20"}, "new")`. Expected: volume 6283.185307, 3 faces, bbox [-10,-10,0]-[10,10,20]; `f-ext1/lateral/e-5` area 1256.637061; `start-cap` and `end-cap` area 314.159265 each.
17. **Region with a hole.** Sketch `SK-RECT` plus entity `{"id": "e-5", "type": "circle", "center": [40, 25], "radius": 10}`; profile `{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4;~e-5", "fp": "c=40,25|a=3685.840735"}]}`; extent distance `"10"`, op new. Expected: volume 36858.407346, 7 faces, bbox [0,0,0]-[80,50,10]; `f-ext1/lateral/e-5` area 628.318531.
18. **Union of two regions.** As 17 with a second region `{"key": "e-5", "fp": "c=40,25|a=314.159265"}`. Expected: volume 40000, 6 faces, bbox [0,0,0]-[80,50,10]; no face named `f-ext1/lateral/e-5`; faces `f-ext1/start-cap` and `f-ext1/end-cap` each have area 4000.
19. **Join.** Features `BOX-BELOW`, `SK-CIRCLE(0, 0, 5)`, `EXT(P-CIRCLE("c=0,0|a=78.539816"), {"kind": "distance", "expr": "5"}, "join")`. Expected: ok; volume 16392.699082, 8 faces, bbox [-20,-20,-10]-[20,20,5]; faces `f-box1/top`, `f-ext1/lateral/e-5` and `f-ext1/end-cap` exist; no `f-ext1/start-cap`; change `Join`.
20. **Pocket cut.** As 19 with extent distance `"4"`, `"reverse": true`, op cut. Expected: volume 15685.840735, 8 faces, bbox [-20,-20,-10]-[20,20,0]; `f-ext1/lateral/e-5` (pocket wall) and `f-ext1/end-cap` (pocket floor, plane z=-4) exist; no `f-ext1/start-cap`; change `Cut`.
21. **Through all cut.** As 19 with extent `{"kind": "through_all"}`, `"reverse": true`, op cut. Expected: volume 15214.601837, 7 faces; `f-ext1/lateral/e-5` exists; no cap names.
22. **Through all pointing away.** As 21 without `reverse`. Expected: error `through_all_misses` rendered `Through all points away from body 'Body 1', so there is nothing to go through.` with the fix `SetField { field: reverse, value: true }` (Turn on Reverse), since reversed it reaches the body; b-1 volume 16000. With the circle at center [100, 0] (fp `c=100,0|a=78.539816`), reversing misses too: rendered `Through all points away from body 'Body 1', so there is nothing to go through. Point it at the body, or use a distance.` with no fix.
23. **Through all with op new.** `EXT(P-RECT, {"kind": "through_all"}, "new")`. Expected: error `through_all_needs_body` rendered `Through all goes through an existing solid, and body 'Body 1' has none here. Use a distance instead.`
24. **Intersect.** Features `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "op": "new"}`, `SK-CIRCLE(0, 0, 10)`, `EXT(P-CIRCLE("c=0,0|a=314.159265"), {"kind": "symmetric", "expr": "100"}, "intersect")`. Expected: volume 3141.592654, 3 faces, bbox [-10,-10,0]-[10,10,10]; change `Intersect`.
25. **Intersect without overlap.** As 24 with the circle at center [100, 0], radius 5 (fp `c=100,0|a=78.539816`). Expected: error `empty_result` rendered `The shapes do not overlap body 'Body 1', so the intersection is empty. Move them so they overlap it.`; b-1 volume 16000.
26. **Cut everything.** Features `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`, a sketch `RECT(-50, -50, 50, 50)` on XY (fp `c=0,0|a=10000`), `EXT(..., {"kind": "symmetric", "expr": "100"}, "cut")`. Expected: error `empty_result` rendered `This cut removes all of body 'Body 1'. Make it smaller, or change its position or direction.`; b-1 volume 1000.
27. **Cut that misses.** As 20 with the circle at center [100, 0] (fp `c=100,0|a=78.539816`). Expected: warning `no_effect` with how `Beside` (the tool's box and the body's do not overlap across the direction), rendered `This cut does not touch body 'Body 1': it lies beside it.`, with no Reverse fix (reversed, it misses too); b-1 volume 16000, 6 faces, element names unchanged.
28. **New on a body with a solid.** `BOX-BELOW`, `SK-RECT`, `EXT(P-RECT, {"kind": "distance", "expr": "30"}, "new")`. Expected: error `body_not_empty`; b-1 volume 16000.
29. **Join on an empty body.** `EXT(P-RECT, {"kind": "distance", "expr": "30"}, "join")`. Expected: error `body_empty` rendered `Body 'Body 1' has no solid yet at this point, so there is nothing to add to. Set Operation to New body to start it here.` with the fix `SetField { field: op, value: "new" }`.
30. **Region matched by fingerprint.** As 1 with region `{"key": "e-11,e-12,e-13,e-14", "fp": "c=40,25|a=4000"}` (no entity of the stored key is left, and one region has the stored size and place). Expected: status warning (`region_remapped`, pointer `/profile/regions/0`) rendered `The outline of the region of 'Sketch 1' selected for Profile was drawn again, so the region with the same size and place is used. Check it, then keep it or pick it again.`, fixes `KeepRegions { pointer: "/profile/regions/0" }` then `Pick`; volume 120000.
31. **A hole circle drawn again.** As 17 with region `{"key": "e-1,e-2,e-3,e-4;~e-6", "fp": "c=40,25|a=3685.840735"}` (the stored hole circle `e-6` is gone and `e-5` is drawn in its place). The outer lines survive, and the disc of `e-5` fills the hole of the stored region because `e-5` is new geometry (sketch.md section 3.14 step 2), so the match is `Coverage` with both regions (features.md open issue 10). Expected: status ok with notice `region_split` (count 2) rendered `The region of 'Sketch 1' selected for Profile is now split in 2 parts by new lines, and all of them are used. Remove a part from the profile to leave it out.`; volume 40000, 6 faces (the union is the whole rectangle), no face named `f-ext1/lateral/e-5`.
32. **One region missing.** As 1 with a second region `{"key": "e-7,e-8", "fp": "c=200,200|a=100"}`. Expected: warning `region_missing` at pointer `/profile/regions/1`, rendered `The region of 'Sketch 1' selected for Profile is gone because its outline was drawn again, so it was skipped.` (none of its entities is in the sketch), fixes `Show { feature: "f-sk1" }` then `Pick`; volume 120000.
33. **All regions missing.** As 1 with the only region `{"key": "e-7,e-8", "fp": "c=200,200|a=100"}`. Expected: error `profile_not_found` rendered `The region of 'Sketch 1' selected for Profile is gone because its outline was drawn again.`
34. **Profile from a datum.** Features `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}}`, `EXT` with profile `{"sketch": "f-dp1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]}`. Expected: error `wrong_feature_type` rendered `'Datum plane 1' is not a sketch. Pick a sketch for Profile.`
35. **Sketch later in the timeline.** Test 1 with the two features in reverse order. Expected: extrude error `forward_reference` rendered `Profile uses 'Sketch 1', which comes after this extrude. Move this extrude below 'Sketch 1', or pick something earlier.` with the fixes `Move { feature: "f-ext1", place: Below("f-sk1") }` then `Pick`; sketch ok.
36. **Sketch on XZ.** As 1 with the sketch plane `{"kind": "origin", "plane": "XZ"}`. Expected: volume 120000, bbox [0,-30,0]-[80,0,50].
37. **Sketch on a datum.** Features `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "10"}`, `SK-RECT` with plane `{"kind": "datum", "feature": "f-dp1"}`, extrude as 1. Expected: bbox [0,0,10]-[80,50,40].
38. **Empty profile.** As 1 with `"regions": []`. Expected: error `empty_selection` rendered `Select at least one region for Profile.`
39. **Performance.** Test 1 builds in under 3 ms (desktop, release, build only); test 21 in under 30 ms.
40. **Union of regions split by a line.** `SK-RECT` plus `{"id": "e-5", "type": "line", "start": [40, 0], "end": [40, 50]}`; profile `{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,~e-5", "fp": "c=60,25|a=2000"}, {"key": "e-1,e-3,e-4,e-5", "fp": "c=20,25|a=2000"}]}`; extent distance `"10"`, op new (`examples/extrude/union-split-by-line.ptsm`). Expected: ok; volume 40000, 6 faces, bbox [0,0,0]-[80,50,10]; faces exactly `f-ext1/end-cap`, `f-ext1/lateral/e-1`, `f-ext1/lateral/e-2`, `f-ext1/lateral/e-3`, `f-ext1/lateral/e-4`, `f-ext1/start-cap`; `f-ext1/lateral/e-1` and `f-ext1/lateral/e-3` area 800 each; no face named `f-ext1/lateral/e-5`.
41. **Union of circle halves.** `SK-CIRCLE(0, 0, 10)` plus `{"id": "e-6", "type": "line", "start": [-10, 0], "end": [10, 0]}`; profile `{"sketch": "f-sk1", "regions": [{"key": "e-5,e-6", "fp": "c=0,4.244132|a=157.079633"}, {"key": "e-5,~e-6", "fp": "c=0,-4.244132|a=157.079633"}]}`; extent distance `"20"`, op new. Expected: as 16 (volume 6283.185307, 3 faces, `f-ext1/lateral/e-5` area 1256.637061); no face named `f-ext1/lateral/e-6`.
42. **Pieces of one line that stay separate.** `SK-RECT` plus `{"id": "e-5", "type": "circle", "center": [40, 0], "radius": 10}`; profile `{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4,~e-5", "fp": "c=40,25.848398|a=3842.920367"}]}` (the rectangle without the half disc); extent distance `"10"`, op new. The arc of e-5 interrupts e-1, so its two pieces are not consecutive and are not merged. Expected: ok; volume 38429.203673, 8 faces; `f-ext1/lateral/e-1#1` and `f-ext1/lateral/e-1#2` area 300 each; `f-ext1/lateral/e-5` area 314.159265; no face named `f-ext1/lateral/e-1`.
43. **Union touching itself on a circle.** `SK-RECT` plus `{"id": "e-5", "type": "circle", "center": [40, 40], "radius": 10}` (touching e-3 at (40, 50)) and `{"id": "e-6", "type": "line", "start": [40, 0], "end": [40, 50]}`; profile `{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,~e-5,~e-6", "fp": "c=61.342937,23.721489|a=1842.920367"}, {"key": "e-1,e-3,e-4,~e-5,e-6", "fp": "c=18.657063,23.721489|a=1842.920367"}]}` (the two halves outside the circle); extent distance `"10"`, op new. The union's boundary runs round the circle between two visits to the touch point, so its two arcs stay separate instead of merging into an arc that ends where it starts (features.md section 3.7). Expected: ok; volume 36858.407346, 9 faces, bbox [0,0,0]-[80,50,10]; faces exactly `f-ext1/end-cap`, `f-ext1/lateral/e-1`, `f-ext1/lateral/e-2`, `f-ext1/lateral/e-3#1`, `f-ext1/lateral/e-3#2`, `f-ext1/lateral/e-4`, `f-ext1/lateral/e-5#1`, `f-ext1/lateral/e-5#2`, `f-ext1/start-cap`; `f-ext1/lateral/e-1` area 800; `f-ext1/lateral/e-3#1` and `#2` area 400 each; `f-ext1/lateral/e-5#1` and `#2` area 314.159265 each.
44. **Pieces of one line in a single region.** `SK-RECT` plus `{"id": "e-5", "type": "line", "start": [20, 50], "end": [20, 70]}`, `{"id": "e-6", "type": "line", "start": [20, 70], "end": [60, 70]}` and `{"id": "e-7", "type": "line", "start": [60, 70], "end": [60, 50]}` (a second rectangle on top that uses part of e-3); profile `P-RECT` (the lower region only); extent distance `"10"`, op new. e-5 and e-7 split e-3 into three pieces that follow each other along the region's boundary, so they merge although no selected region shares them. Expected: ok; volume 40000, 6 faces, bbox [0,0,0]-[80,50,10]; faces as in test 40; `f-ext1/lateral/e-3` area 800.
45. **Distance below the tolerance.** As 1 with extent distance `"0.0000001"`. Expected: error `out_of_range`, field `extent.expr`, rendered `Distance must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm.` (the value is written with the digits that tell it apart from the bound, features.md section 3.12); b-1 empty.
46. **Two sides below the tolerance.** As 1 with extent `{"kind": "two_sides", "expr": "0.0000005", "opposite_expr": "0.0000004"}`. Expected: error `both_sides_zero`; b-1 empty.

Tests 47 to 56 use `BLOCK`: `SK-RECT` and `EXT(P-RECT-V2, {"kind": "distance", "expr": "30"}, "new")` (x 0..80, y 0..50, z 0..30, volume 120000), with `P-RECT-V2` the profile as a pick stores it since file format version 2: `{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-1,e-2,e-3,e-4"}]}`. `FIL(edge, radius)` is `{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [<edge>], "radius": <radius>}`. Sketch edits are applied to the test's document as it stands, and the run after the edit is checked.

47. **A side line deleted and drawn again (corpus R06a).** `BLOCK`, `FIL({"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-2)", "fp": "line|n=0,1,0|c=80,25,30|l=50|adj=2,2"}, "2")`. Edit: delete `e-2` from f-sk1 and add `{"id": "e-9", "type": "line", "start": [80, 0], "end": [80, 50]}`. Expected: the profile matches by `Boundary` with the succession `e-2 -> [e-9]`, so f-ext1 is ok with no region message and its right side is `f-ext1/lateral/e-9`, which carries the alias `f-ext1/lateral/e-2`; f-fil1 status ok with notice `reference_followed` (follow `Replaced`) at `/edges/0`, fixes `KeepMatch { pointer: "/edges/0" }` then `Pick`; the fillet face's axis lies at x = 78, z = 28; b-1 volume 119957.079633. Then (`KeepMatch`): f-fil1 stores `edge(f-ext1/end-cap|f-ext1/lateral/e-9)` with its anchors and the notice is gone.
48. **The follow lasts through the next edit (corpus R07a).** `BLOCK`, `FIL({"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-3)", "fp": "line|n=1,0,0|c=40,50,30|l=80|adj=2,2"}, "2")` (the back top edge). Edit (1): delete `e-3` and add `{"id": "e-9", "type": "line", "start": [80, 50], "end": [0, 50]}`; edit (2): set f-ext1's distance to `"80"`. Expected after (2): f-ext1 ok with no region message; f-fil1 keeps the back top edge, with the same notice as after (1): the fillet face's axis lies at y = 48, z = 78 and runs over x 0..80, the bottom back edge stays sharp; b-1 volume 319931.327412.
49. **A side line split by a collinear line keeps the whole opening (corpus R49).** `BLOCK`, `{"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "2", "remove_faces": [{"q": "f-ext1/lateral/e-2", "fp": "plane|n=1,0,0|c=80,25,15|a=1500|adj=4,4"}]}`. Edit: set `e-2`'s end to (80, 25) and add `{"id": "e-6", "type": "line", "start": [80, 25], "end": [80, 50]}`. Expected: the profile matches by `Boundary` with the succession `e-2 -> [e-2, e-6]` (e-6 continues e-2 on its line); the right side is `f-ext1/lateral/e-2` (y 0..25) and `f-ext1/lateral/e-6` (y 25..50); f-sh1 status ok with notice `reference_followed` (follow `Extended`, both faces in one plane) at `/remove_faces/0`; the whole right side is open: one opening in x = 80 spanning y 2..48, z 2..28; b-1 volume 26712.
50. **A side line replaced by a notch (corpus R66).** `BLOCK`, `FIL({"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-3)", "fp": "line|n=1,0,0|c=40,50,30|l=80|adj=2,2"}, "1")`. Edit: delete `e-3` and add `e-9` (80,50)-(50,50), `e-10` (50,50)-(50,40), `e-11` (50,40)-(30,40), `e-12` (30,40)-(30,50) and `e-13` (30,50)-(0,50), each `{"id": ..., "type": "line", "start": ..., "end": ...}`. Expected: the profile matches by `Boundary` with the succession `e-3 -> [e-9, e-10, e-11, e-12, e-13]`; f-ext1 ok; f-fil1 status warning (`reference_widened`) at `/edges/0`, rendered `The back top edge of 'Extrude 1' selected for Edges now covers 5 edges, because the line it was made from was replaced by 5 lines in 'Sketch 1'. Keep all of them, or use only one.`, with the fixes `KeepMatch` (Keep all 5), `UseReferences` with the top edge made from `e-9` alone (Use only the back top edge of 'Extrude 1' along the line from (80, 50) to (50, 50)), then `Pick`; the fillet is built on all five top edges (5 faces named `f-fil1/fillet/...`) until the user chooses.
51. **A cut that goes all the way through (corpus R12).** `BLOCK`; `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "30"}`; `{"id": "f-sk2", "type": "sketch", "plane": {"kind": "datum", "feature": "f-dp1"}, "entities": [{"id": "e-5", "type": "circle", "center": [40, 25], "radius": 10}]}`; `{"id": "f-ext2", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk2", "regions": [{"key": "e-5", "fp": "c=40,25|a=314.159265"}]}, "extent": {"kind": "distance", "expr": "30"}, "reverse": true, "op": "cut"}` (exactly through). Expected: f-ext2 ok; its `FeatureOutput::absent` is `[{name: "f-ext2/end-cap", why: Through}]`; b-1 volume 110575.222039, 7 faces. With distance `"40"`: the same. With distance `"5"` (a pocket): `absent` is empty and `f-ext2/end-cap` is the pocket floor at z = 25.
52. **A join buried in the body (newcomer error corpus NC02).** `BLOCK`; `{"id": "f-sk2", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-5", "type": "circle", "center": [40, 25], "radius": 5}]}`; `{"id": "f-ext2", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk2", "regions": [{"key": "e-5", "fp": "c=40,25|a=78.539816"}]}, "extent": {"kind": "distance", "expr": "10"}, "op": "join"}`. Expected: f-ext2 status warning (`no_effect` with how `Inside`) rendered `This extrude adds nothing to body 'Body 1': it lies entirely inside the body.`, its only fix `Attach { sketch: "f-sk2", body: "b-1", face }` with `face` the picked `f-ext1/end-cap` (Attach 'Sketch 2' to the top of 'Extrude 1'); no Reverse fix. Then: f-sk2 lies on the top with fallback origin (0,0,30) in body coordinates, the boss stands at z 30..40, every feature status ok, b-1 volume 120785.398163.
53. **A cut that points away or ends short (newcomer error corpus NC03).** `BLOCK`; `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "40"}`; `{"id": "f-sk2", "type": "sketch", "plane": {"kind": "datum", "feature": "f-dp1"}, "entities": [{"id": "e-5", "type": "circle", "center": [40, 25], "radius": 5}]}`; `{"id": "f-ext2", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk2", "regions": [{"key": "e-5", "fp": "c=40,25|a=78.539816"}]}, "extent": {"kind": "distance", "expr": "15"}, "op": "cut"}`. Expected: f-ext2 status warning (`no_effect` with how `Away { gap: 10 }`) rendered `This cut does not touch body 'Body 1': it starts 10 mm from the body and points away from it.` with the fix `SetField { field: reverse, value: true }` (Turn on Reverse). Then: a pocket z 25..30, status ok, b-1 volume 119607.300918. With distance `"5"`: the same message with no fix (reversed, the cut ends at z = 35 and misses too). With distance `"5"` and `"reverse": true`: how `Short { gap: 5 }`, rendered `This cut does not reach body 'Body 1': it ends 5 mm short of it.`, with no fix (the other way it misses too).
54. **A join that floats (newcomer error corpus NC04).** As 53 with op `join`. Expected: f-ext2 status warning (`separate_solid`) rendered `This extrude does not touch body 'Body 1', so the body now has 2 separate pieces.`, fixes `NewBody` (Put it in a new body) then `Attach` of f-sk2 to the picked `f-ext1/end-cap` (Attach 'Sketch 2' to the top of 'Extrude 1'); b-1 has 2 solids. Then (the `Attach` fix): b-1 is one solid with the boss at z 30..45, volume 121178.097245, status ok.
55. **Distance beyond the modeling limit.** As 1 with extent distance `"1e200"`. Expected: error `out_of_range`, field `extent.expr`, bound `AtMost(1000000)`, rendered `Distance must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less.` with the fix [Use 1000000 mm]; no kernel call is made (no kernel text about the profile appears); b-1 empty.
56. **A region whose lines survive matches by boundary.** As 1 with region `{"key": "e-1,e-2,e-3,e-9", "fp": "c=40,25|a=4000"}` (three of its four lines are in the sketch). Expected: status ok with no region message (`Boundary` with one region); volume 120000.

## Open issues

1. A symmetric extrusion silently ignores `reverse`. The UI should hide Reverse for the symmetric extent.
2. **Pieces of one entity in a union.** Resolved: the profile union dropped the boundary pieces two selected regions share but kept the remaining pieces of one entity as separate kernel curves, so both halves of `RECT(0, 0, 80, 50)` split by a line gave 8 faces (`f-ext1/lateral/e-1#1`, `#2`, `f-ext1/lateral/e-3#1`, `#2`) where the single region gives 6. Features.md section 3.7 now merges consecutive pieces of one entity that continue the same line or circle before `make_profile`, and naming.md section 3.6.2 numbers only the pieces that stay separate after the union (tests 40 to 42 and 44; revolve.md test 25 for revolve).
3. **Union touching itself on a circle.** Resolved: merging joined the arcs of a circle that the union's boundary runs round between two visits to a touch point (a circle touching e-3, split by a line, with both halves outside it selected) into one arc from that point to itself, and the kernel rejected it with `Curve 3 of the profile has zero length.` Features.md section 3.7 now never merges a run of pieces back to its own start; only a loop made of one entity all the way round becomes its circle (test 43).
4. **Lengths below the tolerance.** Resolved: a distance between 0 and `LINEAR_TOLERANCE` passed the range check and failed in the kernel with `The extrude end must be beyond its start.`, which names no field. Section 3.3 step 3 now applies the positive length rule of features.md section 3.4 (tests 45 and 46).
