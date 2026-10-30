# Feature: shell

Status: M1 spec. Type name `shell`. Shared contracts: [features.md](../features.md). Naming rule: [naming.md](../naming.md) section 3.6.7. Kernel operation: [kernel.md](../kernel.md) sections 3.4.8 and 3.4.9.

## 1. Purpose and scope

Shell hollows a body to a constant wall thickness, measured inward from its outer faces, optionally opening it by removing faces.

### In M1

- Inward thickness only: the outer surface stays where it is.
- Zero or more faces to remove. With none, the result is a closed hollow (a solid with an internal void), as used for resin hollowing.
- Rounded joins where inner offsets separate (the kernel's arc join): a round of radius t around a concave edge (a cylinder along a straight edge, test 24; a torus along a circular one) and a sphere of radius t at a vertex whose edges are all concave (test 25), except where the kernel cuts out the cavity instead (kernel.md section 3.4.8 items 3 and 6.1), which leaves sharp inner corners; convex corners stay sharp.
- Walls at least as thick as a convex round, or less than 0.01 mm thinner: the round's inner offset vanishes and the walls beside it meet in a sharp inner corner (kernel.md section 3.4.8 item 6), so the rounds of a fillet before the shell never limit the wall, and the rounded joins at concave edges stay (test 21).
- Openings in flat faces that meet the rest of the body only tangentially, such as the top of a box with every edge rounded: the wall keeps its thickness up to the opening and ends there in a face normal to the removed face along each of its edges (kernel.md section 3.4.8 item 7, test 14).
- A search for the largest working thickness when the requested one fails.

### Later

- Outward and both-sides thickness.
- Per-face thickness.
- Sharp (intersection) joins at concave edges.
- Resin hollowing presets with drain holes (print module).

## 2. Public API

```rust
/// Hollows a body inward to a wall thickness.
pub struct Shell;

impl Feature for Shell {
    const TYPE: &'static str = "shell";
    type Params = ShellParams;
}

/// Params of a shell feature.
pub struct ShellParams {
    /// The body to hollow.
    pub body: Id,
    /// Wall thickness, measured inward. Length, greater than 0.
    pub thickness: Expr,
    /// Faces to remove, opening the shell. Default empty (a closed hollow), omitted when empty.
    pub remove_faces: Vec<Reference>,
}
```

JSON (the technical design's example form):

```json
{"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "wall",
 "remove_faces": [{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,30|a=4000"}]}
```

### Manifest

Label `Shell`, category `modify`, icon `shell`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `remove_faces` | Faces to remove | yes | | `face` | |
| `thickness` | Thickness | yes | length | | |
| `body` | Body | no | | `body` | |

### Python

Dedicated form: `shell(faces, thickness)` (python-api.md section 2.7), produced by the `Feature::python` hook with the shared rules of features.md section 3.11: `faces` from `remove_faces` as references (one alone, several as a list), and `None` when `remove_faces` is empty (a closed hollow); `thickness` as an expression; the `with <part>:` block gives `body`. With the parameter `wall` as `wall`, the JSON above is `shell(Ref("f-box1/top", "plane|n=0,0,1|c=0,0,30|a=4000"), wall, id="f-sh1")`, and the closed hollow of test 3 is `shell(None, 2, id="f-sh1")`. Generic form of the JSON above:

```python
feature(
    "shell",
    id="f-sh1",
    body="b-1",
    thickness="wall",
    remove_faces=[{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,30|a=4000"}],
)
```

## 3. Behavior

### 3.1 Validation

None beyond serde (an empty `remove_faces` is valid).

### 3.2 Dependencies

- `bodies_written`: `[body]`.
- `expressions`: `("thickness", thickness, Length)`.

### 3.3 Build

1. The body must hold a solid, else `BodyEmpty { field: body, body, use_new: false }` (recompute's diagnosis adds why it is empty, features.md section 4.1).
2. t = `thickness`; t > 0 else `OutOfRange` (`GreaterThan(0)`), by the positive length rule of features.md section 3.4: a thickness below `LINEAR_TOLERANCE` (1e-6 mm) is out of range like 0, before any kernel work; then t is at most the modeling limit, else `OutOfRange` (`AtMost(MODEL_LIMIT)`).
3. If `remove_faces` is not empty, resolve it with `ctx.resolve_all("remove_faces", body, ..., Face)` (features.md section 3.8); its warnings and notices go into the output, and a list that resolves to nothing gives `ReferencesNotFound` with every loss (a shell meant to be open never silently becomes a closed hollow).
4. When the resolved faces include every face of a solid of the body, the shell fails with `ShellRemovesAll { body, count }`, `count` that solid's number of faces, before any kernel work: nothing would be left to hollow, and the kernel's failure would otherwise read as a wall that is too thick.
5. Shell each solid of the body with kernel `shell(solid, faces of that solid to remove, t)`: every kept face is offset inward by t, removed faces leave openings with rims, and an empty removal list makes a closed hollow. The solids' results are combined into one shape (they are disjoint). A closed hollow is what the user asked for, so it gives no `ClosedVoid` warning: shell does not use `apply_op` (features.md section 3.5).
6. On kernel failure (except `InvalidInput` and `Internal`, which are `Kernel` errors), run the "too large" search (features.md section 4.3), find the round faces of the body smaller than the wall and larger than `max` for `limit` (`Limit::RoundFaces { feature, radius, fillet }`), and fail with `ShellTooThick { field: thickness, expr: thickness, body, thickness: t, max, limit, cause }`. Its message and fixes are those of features.md section 4.3: when the round faces come from a fillet before the shell, "Set Radius of '{feature}'" (when that Radius is a plain value) and "Move this shell above '{feature}'" come before the thinner wall (features.md open issue 17).
7. Name (section 3.4) and return one `BodyUpdate` with change `Modify`.

### 3.4 Element names

| Name | Face |
| --- | --- |
| `<id>/inner/<face name>` | The inner wall offset from the outer face named `<face name>`. |
| `<id>/inner/<edge or vertex name>` | A rounded join filling the gap at a concave input edge (a cylinder along a straight edge, test 24) or vertex (a sphere, test 25), or the face ending the wall along the input edge where a removed face met a kept face tangentially (kernel.md section 3.4.8 item 7). |
| `<id>/rim/<face name>` | The rim left where the removed face named `<face name>` was (for a flat removed face, one planar face between the outer edge and the opening). Beside a round of radius r tangent to the removed face, the rim is t - r wide when the wall t is thicker than the round (the round's inner offset vanishes, kernel.md section 3.4.8 item 6.2); there is no rim where the wall is as thick as such a round, or where the removed face met the kept faces only tangentially and the wall is thinner than their rounds (item 7), so a reference to the rim is lost when the thickness changes across the radius. |

Outer faces keep their names; removed faces disappear.

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Body empty | `BodyEmpty` | without a cause: `Body 'Body 1' has no solid at this point, so this shell has nothing to work on. Pick a body that has a solid.`; with one, features.md section 4.1 |
| Thickness below 1e-6 mm (0 and negative included) | `OutOfRange` | `Thickness must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Thickness above the modeling limit | `OutOfRange` | `Thickness must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less.` |
| Expression or dimension error | `Expression` | `Thickness: Expected a length, but this expression gives an angle.` |
| No face to remove resolves | `ReferencesNotFound` | `A face of a feature that is not in this document selected for Faces to remove is gone.` (features.md section 4.1) |
| Every face of a solid removed | `ShellRemovesAll` | `This shell would remove all 6 faces of body 'Body 1', which leaves nothing to hollow. Remove at most 5 faces, or leave Faces to remove empty for a closed hollow.` |
| Too thick | `ShellTooThick` | `Shell thickness 30 mm is too thick for body 'Body 1': the largest that works is about 19 mm. Its rounded faces made by 'Cylinder 1' have radius 20 mm, smaller than the wall.` (features.md section 4.3) |
| Kernel failure of a kind that is not about size | `Kernel` | `Partsmith's geometry kernel could not build this shell. {hint}` (features.md section 4.1) |
| Warnings and notices | `ReferenceLost`, `ReferenceGuessed`, `ReferenceFollowed`, `ReferenceWidened` | features.md section 4.2 |

## 5. Performance

- Box with one removed face: 50 ms. 20-face body: 100 ms.
- A wall within 0.01 mm of the radius of the body's convex rounds (test 19 at t = 2, whose kernel shell removes the rounds from a copy of the body first, kernel.md section 3.4.8 item 6.2): 250 ms, about 30 ms more for each further separate group of rounds (features.md open issue 18).
- A failing shell including the search: 1.2 s.

## 6. Acceptance tests

Conventions from features.md section 6.1. Fixtures:

- `BOX805030`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["80", "50", "30"], "op": "new"}` (spans [-40,-25,0]-[40,25,30], volume 120000).
- `F-TOP`: `{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,30|a=4000"}`; `F-FRONT`: `{"q": "f-box1/front", "fp": "plane|n=0,-1,0|c=0,-25,15|a=2400"}`; `F-LOST`: `{"q": "f-box9/top", "fp": "plane|n=0,0,1|c=500,500,500|a=1"}`.
- `SHELL(thickness, faces)`: `{"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": <thickness>, "remove_faces": <faces>}` (omit `remove_faces` when empty).

Bodies `[b-1]`.

1. **Open top.** `BOX805030`, `SHELL("2", [F-TOP])`. Expected: ok; volume 22112, 11 faces, bbox [-40,-25,0]-[40,25,30]; `f-sh1/inner/f-box1/bottom` planar at z=2 with area 3496; `f-sh1/inner/f-box1/front` planar at y=-23 with area 2128; `f-sh1/rim/f-box1/top` planar at z=30 with area 504; no face named `f-box1/top`; `f-box1/bottom` area 4000; change `Modify`.
2. **Two faces removed.** `BOX805030`, `SHELL("2", [F-TOP, F-FRONT])`. Expected: volume 17856, 10 faces; `f-sh1/rim/f-box1/top` area 352; `f-sh1/rim/f-box1/front` area 272.
3. **Closed hollow.** `BOX805030`, `SHELL("2", [])`. Expected: volume 29104, 12 faces, bbox [-40,-25,0]-[40,25,30]; 6 faces named `f-sh1/inner/...`; `f-box1/top` exists; the body is one solid.
4. **Parameter thickness.** Parameters `[{"id": "p-wall", "name": "wall", "expr": "2 mm"}]`, `BOX805030`, `SHELL("wall", [F-TOP])`. Expected: identical to 1.
5. **Too thick.** `BOX805030`, `SHELL("30", [F-TOP])`. Expected: error `shell_too_thick`, field `thickness`, `thickness == 30`, `max == Some(m)` with 22.5 ≤ m < 25, `limit` None (the box has no round faces); rendered `Shell thickness 30 mm is too thick for body 'Body 1': the largest that works is about {s} mm.` with `{s}` = m rounded down to 2 significant digits; fixes [Use {s} mm]; suggestion `{s} mm`; b-1 unchanged. Rebuilding with `"{s}"` is ok.
6. **Zero thickness.** `BOX805030`, `SHELL("0", [F-TOP])`. Expected: error `out_of_range` rendered `Thickness must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.`
7. **Removed face lost.** `BOX805030`, `SHELL("2", [F-LOST])`. Expected: error `references_not_found` rendered `A face of a feature that is not in this document selected for Faces to remove is gone.` (`f-box9` is not in the document); fixes `UseReferences` for each candidate face, then `Pick`; b-1 unchanged.
8. **One of two removed faces lost.** `BOX805030`, `SHELL("2", [F-TOP, F-LOST])`. Expected: warning `reference_missing` with pointer `/remove_faces/1`; geometry identical to 1.
9. **Cylinder cup.** `{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "20", "height": "40", "op": "new"}`, `SHELL("2", [{"q": "f-cyl1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=1256.637061"}])`. Expected: volume 11586.193706, 5 faces, bbox [-20,-20,0]-[20,20,40].
10. **Two solids.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"}`, `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "position": ["50", "0", "0"], "op": "join"}`, `SHELL("2", [{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,20|a=400"}, {"q": "f-box2/top", "fp": "plane|n=0,0,1|c=50,0,20|a=400"}])`. Expected: volume 2 × (8000 - 16 × 16 × 18) = 6784, 22 faces, 2 solids.
11. **Empty body.** `SHELL("2", [])` alone. Expected: error `body_empty` rendered `Body 'Body 1' has no solid at this point, so this shell has nothing to work on. Pick a body that has a solid.`
12. **Performance.** Test 1 builds in under 50 ms, test 19 at t = `"2"` in under 250 ms, and the failing shell of test 20 with its search in under 1.2 s (desktop, release).
13. **Thickness below the tolerance.** `BOX805030`, `SHELL("0.0000001", [F-TOP])`. Expected: error `out_of_range`, field `thickness`, rendered `Thickness must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm.` (the value is written with the digits that tell it apart from the bound, features.md section 3.12); b-1 unchanged.
14. **Moved body.** The features of `examples/fillet/every-edge` (a 20 mm cube with every edge filleted at radius 2), then `{"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["50", "0", "0"]}`, then `SHELL("1", [{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=50,0,20|a=256"}])`, as `examples/shell/moved-body`. Expected: all ok; the volume of the same document without `f-mv1` and with the top face at x = 0 (`{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,20|a=256"}`), made by the test from the example, relative 1e-9, as many faces, bbox [40,-10,0]-[60,10,20]. (The kernel shells a copy without locations, kernel.md section 3.4.8; with the moved body's location kept, OCCT's offset failed and the shell reported `shell_too_thick`.) In both documents the top, which meets its rounds only tangentially, is open (kernel.md section 3.4.8 item 7): volume 1280 + 460π/3 = 1761.710877 (the cube, 7168 + 608π/3, less its closed hollow's cavity, an 18 mm cube with rounds of radius 1, and the 16 x 16 x 1 opening above it), 54 faces, no planar face at z = 20 facing up or down (OCCT's thick solid once kept the top and a reversed copy of it, a closed hollow of 2017.71 mm³ reported as 1932.38), and `f-sh1/inner/edge(f-box1/top|f-fil1/fillet/edge(f-box1/front|f-box1/top))`, the end of the wall along the top's front edge, planar at y = -8 with normal (0,1,0), area 16 and its centroid at z = 19.5 and x at the cube's center.
15. **Walls thicker than a fillet's rounds (newcomer error corpus NC06).** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "20"], "op": "new"}`; `{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [<the four vertical edges: front|left, front|right, back|left, back|right>], "radius": "1"}`; `SHELL("2", [{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,20|a=1600"}])`; as `examples/shell/walls-thicker-than-rounds`. Expected: every feature status ok, with no message: the rounds' inner offsets vanish and the 2 mm walls meet in sharp inner corners (kernel.md section 3.4.8 item 6); b-1 volume 8592 + 20π = 8654.831853 (the 40 mm prism with 1 mm rounds minus an 18 mm deep cavity with a sharp 36 mm square section), 15 faces, no face named `f-sh1/inner/f-fil1/...`. With `radius` `"3 mm"` (rounds thicker than the wall; made by the test from the example): ok, the inner corners rounded at radius 1, b-1 volume 8532.938010.
16. **Rounded faces not made by a fillet.** `{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "20", "height": "40", "op": "new"}`, `SHELL("30", [{"q": "f-cyl1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=1256.637061"}])`. Expected: error `shell_too_thick`, `max == Some(m)` with 15 ≤ m < 20, `limit` `RoundFaces { feature: "f-cyl1", radius: 20, fillet: false }`, rendered `Shell thickness 30 mm is too thick for body 'Body 1': the largest that works is about {s} mm. Its rounded faces made by 'Cylinder 1' have radius 20 mm, smaller than the wall.` with `{s}` = m rounded down to 2 significant digits; fixes [Use {s} mm] only (no radius or move fix: the faces are not a fillet's).
17. **Every face removed.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"}`, `SHELL("2", [<the six faces of f-box1, each as {"q": "f-box1/<face>"} with its fingerprint>])`. Expected: error `shell_removes_all` rendered `This shell would remove all 6 faces of body 'Body 1', which leaves nothing to hollow. Remove at most 5 faces, or leave Faces to remove empty for a closed hollow.`; fix `Pick { pointer: "/remove_faces" }`; no kernel call is made; b-1 unchanged.
18. **A closed hollow is intended.** Test 3. Expected: status ok with no `closed_void` warning, although b-1 has 1 solid and 2 shells (`count(Shell) == 2`).
19. **Walls at least as thick as the rounds (ui.md open issue 17).** `CUBE20` (`{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"}`, spanning [-10,-10,0]-[10,10,20]); `{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [<the four vertical edges: front|left, front|right, back|left, back|right>], "radius": "2"}` (the body of ui.md's `cube-features.ptsm`); `SHELL(t, [{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,20|a=396.566371"}])` for t `"1"`, `"2"` and `"3"`; with t `"2"`, the wall of ui.md's fixture, this is `examples/shell/rounded-cube`, and the test makes the other two from it. Expected for each t: every feature status ok, one solid, bbox [-10,-10,0]-[10,10,20], `f-sh1/inner/f-box1/front` planar at y = -10 + t, the four faces `f-fil1/fillet/...` kept, and no face named `f-sh1/face`.
    - t = `"1"`, below the radius: volume 1600 + 61π = 1791.637152, 19 faces, `f-sh1/inner/f-box1/front` area 304, `f-sh1/inner/f-fil1/fillet/edge(f-box1/front|f-box1/right)` cylindrical with radius 1.
    - t = `"2"`, equal to the radius: volume 3072 + 80π = 3323.327412, 15 faces, `f-sh1/inner/f-box1/front` area 288 (it reaches the sharp inner corners), no face named `f-sh1/inner/f-fil1/...`, `f-sh1/rim/f-box1/top` planar at z = 20 with area 128 + 4π = 140.566371.
    - t = `"3"`, above the radius: volume 4348 + 80π = 4599.327412, 15 faces, `f-sh1/inner/f-box1/front` area 238, no face named `f-sh1/inner/f-fil1/...`.
20. **Too thick past the rounds.** The features of test 19 with `SHELL("12", ...)`. Expected: error `shell_too_thick`, field `thickness`, `thickness == 12`, `max == Some(m)` with 9 ≤ m < 10 (the walls meet in the middle of the cube; while OCCT's offset limited the walls to the rounds' radius, the search stopped at about 1.99 mm), `limit` None (the rounds' radius 2 is below `max`, features.md section 4.3); rendered `Shell thickness 12 mm is too thick for body 'Body 1': the largest that works is about {s} mm.` with `{s}` = m rounded down to 2 significant digits; fixes [Use {s} mm]; suggestion `{s} mm`; b-1 unchanged. Rebuilding with `"{s}"` is ok.
21. **Arc joins beside rounds thinner than the wall.** The L-shaped body of fillet.md test 4 (`f-box1` 40 x 20 x 10, `f-box2` 20 x 20 x 10 at position (10, 0, 10) joined, spanning [-20,-10,0]-[20,10,20]); with or without `{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{"q": "edge(f-box1/front|f-box1/left)", "fp": "line|n=0,0,1|c=-20,-10,5|l=10"}], "radius": r}`; `SHELL("2", [{"q": "f-box2/top", "fp": "plane|n=0,0,1|c=10,0,20|a=400"}])`. With r = `"1"` this is `examples/shell/arc-join-beside-round`, and the test makes the other three cases from it (r = `"3"`, r = `"2"`, and without f-fil1). Expected in each case: every feature status ok, one solid, and `f-sh1/inner/edge(f-box1/top|f-box2/left)`, the arc join at the concave edge, cylindrical with radius 2 (kernel.md section 3.4.8 item 6.3): without the fillet, volume 5408 + 16π = 5458.265482 and 16 faces; r = `"3"` (thicker than the wall), 5324 + 37π = 5440.238928 and 18 faces; r = `"1"`, 5398 + 18.5π = 5456.119464 and 17 faces; r = `"2"`, 5368 + 26π = 5449.681409 and 17 faces; with r `"1"` and `"2"` no face named `f-sh1/inner/f-fil1/...`. The example's one overhang, the cavity's ceiling under the step (313.1 mm², as the first of test 24's example), includes the arc join's facets up to 45° (`generic-fdm`'s limit), so its area and bounds depend on how many steps the mesher divides the arc into (kernel.md open issue 14), which the libm perturbation check does not reach; the platforms run checks it. (A round thinner than the wall once sent the whole solid to a cavity with sharp concave corners, so editing an unrelated fillet removed the arc join and every reference to it.)
22. **Walls just under the rounds' radius (kernel.md open issue 12).** The features of test 19 with t = `"1.9995"`, where the rounds' inner offsets would be 0.0005 mm thin and vanish (kernel.md section 3.4.8 item 6), so the walls meet in sharp inner corners, as `examples/shell/walls-just-under-rounds`. Expected, with `remove_faces` as in test 19: every feature status ok, volume 7680 + 80π - 16.001² × 18.0005 = 3322.623378, 15 faces, `f-sh1/inner/f-box1/front` planar at y = -8.0005 with area 16.001 × 18.0005 = 288.0260005, `f-sh1/rim/f-box1/top` area 384 + 4π - 16.001² = 140.534370, no face named `f-sh1/inner/f-fil1/...`. With `remove_faces` omitted (made by the test from the example: a closed hollow, which failed with `shell_too_thick` and a limit of about 1.9 mm while OCCT's offset had to build those rounds): every feature status ok, one solid, volume 7680 + 80π - 16.001³ = 3834.559364, 16 faces, `f-box1/top` kept, `f-sh1/inner/f-box1/front` planar at y = -8.0005 with area 16.001² = 256.032001, no face named `f-sh1/inner/f-fil1/...`.
23. **The UI fixture with a shell after a fillet (ui.md open issue 17).** `ui/e2e/fixtures/cube-features.ptsm` as committed (ui.md section 6.2: a 20 mm cube from a sketch and an extrude, its 4 vertical edges rounded at 2 mm, shelled 2 mm with its top removed). Expected: 4 features, each status ok with no message; `b-cube` volume 3072 + 80π = 3323.327412, 15 faces, one solid, bbox [-10,-10,0]-[10,10,20]. A change to the fixture that breaks the shell fails here before the Playwright tests run.
24. **Arc join at a concave edge (naming.md section 3.6.7 rule 4).** The L-shaped body of test 21 without the fillet, then `SHELL("2", [{"q": "f-box1/bottom", "fp": "plane|n=0,0,-1|c=0,0,0|a=800"}])`, as `examples/shell/concave-edge-join`. The removed bottom meets the kept faces only at convex edges, so the kernel's first attempt opens it (kernel.md section 3.4.8 item 2) and fills the gap at the concave edge between the step and the taller box with an arc join. Expected: every feature status ok, one solid, volume 4768 + 16π = 4818.265482 (the L, 12000, less a cavity open at the bottom, 16 mm long in y, whose section in xz is 452 - π mm²: 18 x 8 under the step, 16 x 18 under the high top and 20 - π under the arc join between them), 16 faces, bbox [-20,-10,0]-[20,10,20]; `f-sh1/inner/edge(f-box1/top|f-box2/left)` cylindrical with radius 2 and area 16π = 50.265482 (a quarter turn around the concave edge over its 16 mm between the inner walls), the only face whose name starts with `f-sh1/inner/edge(`; the walls it joins, `f-sh1/inner/f-box1/top` planar at z = 8 with area 288 and `f-sh1/inner/f-box2/left` planar at x = 2 with area 128; `f-sh1/rim/f-box1/bottom` planar at z = 0 with area 224; no face named `f-box1/bottom`, and none whose name starts with `f-sh1/inner/vertex(` or `f-sh1/face` (each end of the concave edge also has two convex edges, so it gets no join of its own). The example's first overhang, the cavity's ceiling under the step, includes the arc join's facets up to 45° (`generic-fdm`'s limit), so its area and bounds depend on how many steps the mesher divides the arc into (kernel.md open issue 14).
25. **Sphere join at a concave vertex (naming.md section 3.6.7 rule 5).** `CUBE20` with a 10 mm cube cut from the bottom of its back right quarter, `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "position": ["5", "5", "0"], "op": "cut"}` (a notch over [0,10] x [0,10] x [0,10] whose ceiling `f-box2/top` and walls `f-box2/front` and `f-box2/left` meet at (0,0,10), where all three edges are concave), then `SHELL("2", [{"q": "f-box1/bottom", "fp": "plane|n=0,0,-1|c=-1.666667,-1.666667,0|a=300"}])`, as `examples/shell/concave-vertex-join`. Expected: every feature status ok, one solid, volume 3480 + 82π/3 = 3565.870199 (the notched cube, 7000, less a cavity open at the bottom: the 16 x 16 x 18 mm box inside the walls, 4608, without its part within 2 mm of the notch, 1088 + 82π/3), 21 faces, bbox [-10,-10,0]-[10,10,20]; the arc joins `f-sh1/inner/edge(f-box2/front|f-box2/top)` and `f-sh1/inner/edge(f-box2/left|f-box2/top)`, each cylindrical with radius 2 and area 8π = 25.132741, and `f-sh1/inner/edge(f-box2/front|f-box2/left)`, cylindrical with radius 2 and area 10π = 31.415927 (it runs the full 10 mm down to the opening), the only faces whose names start with `f-sh1/inner/edge(`; `f-sh1/inner/vertex(f-box2/front|f-box2/left|f-box2/top)`, spherical with radius 2 and area 2π = 6.283185 (an eighth of a sphere around the vertex, where the three arc joins meet), the only face whose name starts with `f-sh1/inner/vertex(`; `f-sh1/inner/f-box2/top` planar at z = 12 with area 64; `f-sh1/rim/f-box1/bottom` planar at z = 0 with area 140 + π = 143.141593; no face whose name starts with `f-sh1/face`. The notch is cut from the bottom so that every curved join faces up or sideways: the example's overhangs are the planar ceilings of the cavity (256 mm² at z = 18) and of the notch (100 mm² at z = 10), and none of its print results depends on where a platform puts the sphere's mesh nodes (kernel.md open issue 14).
26. **Removed face beside a concave edge (open issue 3).** The L-shaped body of test 21 without the fillet, then `SHELL("2", [{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=-10,0,10|a=400"}])` (the step face, which meets the taller box at a concave edge), as `examples/shell/step-face-removed`. The kernel's first attempt cannot open the step face, so it cuts out the cavity instead (kernel.md section 3.4.8 item 3). Expected: every feature status ok, one solid, volume 5344 (the L, 12000, less a cavity of 36 x 16 x 8 below the step's level and 16 x 16 x 8 above it), 16 faces, bbox [-20,-10,0]-[20,10,20]; no face named `f-box1/top`; `f-sh1/rim/f-box1/top` planar at z = 10 with area 112; `f-sh1/inner/f-box1/top`, the bottom of the wall over the opening, which the step face generates (naming.md section 3.6.7 rule 3), planar at z = 10 with normal (0,0,-1) and area 32; `f-sh1/inner/f-box2/left` planar at x = 2 with area 128; 8 faces named `f-sh1/inner/...`; no face whose name starts with `f-sh1/face`. With thickness `"0.5"`, as `examples/shell/step-face-removed-thin`: every feature status ok, volume 1531 (a cavity of 39 x 19 x 9.5 and 19 x 19 x 9.5), 16 faces; `f-sh1/rim/f-box1/top` area 29.5; `f-sh1/inner/f-box1/top` area 9.5; `f-sh1/inner/f-box2/left` planar at x = 0.5 with area 180.5.

## Open issues

1. Test 5 bounds the suggestion (the true limit is just under 25 mm, half the 50 mm width) instead of fixing it, because the exact point where the kernel's offset stops succeeding is a kernel property.
2. The rim naming relies on kernel.md section 3.4.9 (removed faces list their rim faces in `modified`), which naming.md open issue 7 asks to verify against OCCT 7.9.3.
3. Resolved by kernel.md section 3.4.8 item 3. OCCT's join offset (the kernel's first attempt, arc join) cannot open a removed face that borders a concave edge: the L-shaped body of fillet.md test 4 with `f-box1/top` (the step face) removed failed with `ShellTooThick` and no working thickness, at 2 mm and at 0.5 mm, while removing `f-box1/bottom` works and names the arc join `f-sh1/inner/edge(f-box1/top|f-box2/left)`. No join or offset setting fixes it, so the kernel now falls back to cutting out a cavity whose removed faces stay in place (offset 0) when the first attempt fails beside a concave edge (kernel.md test 102). For that body with the step face removed at 2 mm, the result has volume 5344 and 16 faces: `f-sh1/rim/f-box1/top` (planar at z=10, area 112), `f-sh1/inner/f-box1/top` (the bottom of the wall over the opening, planar at z=10, area 32, which the step face generates under naming.md section 3.6.7 rule 3), `f-sh1/inner/f-box2/left` (planar at x=2, area 128) and the other inner faces (test 26). In that fallback, concave edges between kept faces get sharp inner corners instead of arc joins (kernel.md section 3.4.8 item 3), and a removed face that meets a concave fillet tangentially still fails (kernel.md open issue 7).
