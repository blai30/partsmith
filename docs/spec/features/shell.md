# Feature: shell

Status: M1 spec. Type name `shell`. Shared contracts: [features.md](../features.md). Naming rule: [naming.md](../naming.md) section 3.6.7. Kernel operation: [kernel.md](../kernel.md) sections 3.4.8 and 3.4.9.

## 1. Purpose and scope

Shell hollows a body to a constant wall thickness, measured inward from its outer faces, optionally opening it by removing faces.

### In M1

- Inward thickness only: the outer surface stays where it is.
- Zero or more faces to remove. With none, the result is a closed hollow (a solid with an internal void), as used for resin hollowing.
- Rounded joins where inner offsets separate at concave edges (the kernel's arc join); convex corners stay sharp.
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
6. On kernel failure (except `InvalidInput` and `Internal`, which are `Kernel` errors), run the "too large" search (features.md section 4.3), find the round faces of the body smaller than the wall for `limit` (`Limit::RoundFaces { feature, radius, fillet }`), and fail with `ShellTooThick { field: thickness, expr: thickness, body, thickness: t, max, limit, cause }`. Its message and fixes are those of features.md section 4.3: when the round faces come from a fillet before the shell, "Set Radius of '{feature}'" (when that Radius is a plain value) and "Move this shell above '{feature}'" come before the thinner wall (newcomer error corpus NC06).
7. Name (section 3.4) and return one `BodyUpdate` with change `Modify`.

### 3.4 Element names

| Name | Face |
| --- | --- |
| `<id>/inner/<face name>` | The inner wall offset from the outer face named `<face name>`. |
| `<id>/inner/<edge or vertex name>` | A rounded join filling the gap at a concave input edge or vertex. |
| `<id>/rim/<face name>` | The rim left where the removed face named `<face name>` was (for a flat removed face, one planar face between the outer edge and the opening). |

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
| Too thick | `ShellTooThick` | `Shell thickness 2 mm is too thick for body 'Body 1': the largest that works is about 0.99 mm. Its rounded faces made by 'Fillet 1' have radius 1 mm, smaller than the wall.` (features.md section 4.3) |
| Kernel failure of a kind that is not about size | `Kernel` | `Partsmith's geometry kernel could not build this shell. {hint}` (features.md section 4.1) |
| Warnings and notices | `ReferenceLost`, `ReferenceGuessed`, `ReferenceFollowed`, `ReferenceWidened` | features.md section 4.2 |

## 5. Performance

- Box with one removed face: 50 ms. 20-face body: 100 ms.
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
12. **Performance.** Test 1 builds in under 50 ms (desktop, release).
13. **Thickness below the tolerance.** `BOX805030`, `SHELL("0.0000001", [F-TOP])`. Expected: error `out_of_range`, field `thickness`, rendered `Thickness must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm.` (the value is written with the digits that tell it apart from the bound, features.md section 3.12); b-1 unchanged.
14. **Moved body.** The features of `examples/fillet/every-edge` (a 20 mm cube with every edge filleted at radius 2), then `{"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["50", "0", "0"]}`, then `SHELL("1", [{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=50,0,20|a=256"}])`. Expected: all ok; the volume of the same document without `f-mv1` and with the top face at x = 0 (relative 1e-9), as many faces, bbox [40,-10,0]-[60,10,20]. (The kernel shells a copy without locations, kernel.md section 3.4.8; with the moved body's location kept, OCCT's offset failed and the shell reported `shell_too_thick`.)
15. **Rounded faces thinner than the wall (newcomer error corpus NC06).** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "20"], "op": "new"}`; `{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [<the four vertical edges: front|left, front|right, back|left, back|right>], "radius": "1"}`; `SHELL("2", [{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,20|a=1600"}])`. Expected: error `shell_too_thick` with `limit` `RoundFaces { feature: "f-fil1", radius: 1, fillet: true }`, rendered `Shell thickness 2 mm is too thick for body 'Body 1': the largest that works is about 0.99 mm. Its rounded faces made by 'Fillet 1' have radius 1 mm, smaller than the wall.`; fixes [Set Radius of 'Fillet 1' to 3 mm] (1.5 times the wall), [Move 'Shell 1' above 'Fillet 1'], [Use 0.99 mm], in that order. Then (the first fix): f-fil1 `radius` is `"3 mm"`; every feature status ok, the wall 2 mm thick, b-1 volume 8532.938010.
16. **Rounded faces not made by a fillet.** `{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "20", "height": "40", "op": "new"}`, `SHELL("30", [{"q": "f-cyl1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=1256.637061"}])`. Expected: error `shell_too_thick`, `max == Some(m)` with 15 ≤ m < 20, `limit` `RoundFaces { feature: "f-cyl1", radius: 20, fillet: false }`, rendered `Shell thickness 30 mm is too thick for body 'Body 1': the largest that works is about {s} mm. Its rounded faces made by 'Cylinder 1' have radius 20 mm, smaller than the wall.` with `{s}` = m rounded down to 2 significant digits; fixes [Use {s} mm] only (no radius or move fix: the faces are not a fillet's).
17. **Every face removed.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"}`, `SHELL("2", [<the six faces of f-box1, each as {"q": "f-box1/<face>"} with its fingerprint>])`. Expected: error `shell_removes_all` rendered `This shell would remove all 6 faces of body 'Body 1', which leaves nothing to hollow. Remove at most 5 faces, or leave Faces to remove empty for a closed hollow.`; fix `Pick { pointer: "/remove_faces" }`; no kernel call is made; b-1 unchanged.
18. **A closed hollow is intended.** Test 3. Expected: status ok with no `closed_void` warning, although b-1 has 1 solid and 2 shells (`count(Shell) == 2`).

## Open issues

1. Test 5 bounds the suggestion (the true limit is just under 25 mm, half the 50 mm width) instead of fixing it, because the exact point where the kernel's offset stops succeeding is a kernel property.
2. The rim naming relies on kernel.md section 3.4.9 (removed faces list their rim faces in `modified`), which naming.md open issue 7 asks to verify against OCCT 7.9.3.
3. Resolved by kernel.md section 3.4.8 item 3. OCCT's join offset (the kernel's first attempt, arc join) cannot open a removed face that borders a concave edge: the L-shaped body of fillet.md test 4 with `f-box1/top` (the step face) removed failed with `ShellTooThick` and no working thickness, at 2 mm and at 0.5 mm, while removing `f-box1/bottom` works and names the arc join `f-sh1/inner/edge(f-box1/top|f-box2/left)`. No join or offset setting fixes it, so the kernel now falls back to cutting out a cavity whose removed faces stay in place (offset 0) when the first attempt fails beside a concave edge (kernel.md test 102). For that body with the step face removed at 2 mm, the result has volume 5344 and 16 faces: `f-sh1/rim/f-box1/top` (planar at z=10, area 112), `f-sh1/inner/f-box1/top` (the bottom of the wall over the opening, planar at z=10, area 32, which the step face generates under naming.md section 3.6.7 rule 3), `f-sh1/inner/f-box2/left` (planar at x=2, area 128) and the other inner faces. In that fallback, concave edges between kept faces get sharp inner corners instead of arc joins (kernel.md section 3.4.8 item 3), and a removed face that meets a concave fillet tangentially still fails (kernel.md open issue 7).
