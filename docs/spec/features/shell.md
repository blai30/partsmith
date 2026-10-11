# Feature: shell

Status: M1 spec. Type name `shell`. Shared contracts: [features.md](../features.md).

## 1. Purpose and scope

Shell hollows a body to a constant wall thickness, measured inward from its outer faces, optionally opening it by removing faces.

### In M1

- Inward thickness only: the outer surface stays where it is.
- Zero or more faces to remove. With none, the result is a closed hollow (a solid with an internal void), as used for resin hollowing.
- Sharp inner corners (offset faces meet by intersection).
- A search for the largest working thickness when the requested one fails.

### Later

- Outward and both-sides thickness.
- Per-face thickness.
- Rounded (arc) inner corners.
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

```python
shell(
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

1. The body must hold a solid, else `BodyEmpty { use_new: false }`.
2. t = `thickness`; t > 0 else `OutOfRange` (`GreaterThan(0)`).
3. If `remove_faces` is not empty, resolve it with `ctx.resolve_all("remove_faces", body, ..., Face)`; nothing resolved gives `ReferencesNotFound`.
4. With faces to remove: build a thick solid by offsetting every other face inward by t, joining offset faces by intersection, and opening the removed faces. Without faces to remove: build the inward offset of the closed surface and subtract it, leaving a void. Every solid of the body is shelled; removed faces may belong to any of them.
5. On kernel failure, an invalid result after one heal, or an inner offset that turns inside out (a void of zero or negative volume), run the "too large" search (features.md section 4.3) and fail with `ShellTooThick { thickness: t, max }`.
6. Refine, name and return one `BodyUpdate` with change `Modify`.

### 3.4 Element names

| Name | Face |
| --- | --- |
| `<id>/inner/<face name>` | The inner wall offset from the outer face named `<face name>`. |
| `<id>/rim/<face name>` | The flat rim left in the plane of the removed face named `<face name>` (between the outer edge and the opening), merged into one face per removed face where it is connected. |

Outer faces keep their names; removed faces disappear.

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Body empty | `BodyEmpty` | `Body 'Body 1' is empty at this point, so this feature has nothing to work on. Pick a body that has a solid.` |
| Thickness ≤ 0 | `OutOfRange` | `Thickness must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Thickness not a length | `WrongDimension` | `Thickness must be a length, but '30 deg' is an angle. Use a value such as 10 mm.` |
| No face to remove resolves | `ReferencesNotFound` | `The face selected for Faces to remove can no longer be found. Re-pick it in the repair panel.` |
| Too thick or kernel failure | `ShellTooThick` | `Shell thickness 30 mm is too thick for body 'Body 1'. The thickest wall that works is about 24 mm. Try 24 mm or less.` |
| Warnings | `ReferenceMissing`, `ReferenceAmbiguous` | features.md section 4.2 |

## 5. Performance

- Box with one removed face: 50 ms. 20-face body: 100 ms.
- A failing shell including the search: 1.2 s.

## 6. Acceptance tests

Conventions from features.md section 6.1. Fixtures:

- `BOX805030`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["80", "50", "30"], "op": "new"}` (spans [-40,-25,0]-[40,25,30], volume 120000).
- `F-TOP`: `{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,30|a=4000"}`; `F-FRONT`: `{"q": "f-box1/front", "fp": "plane|n=0,-1,0|c=0,-25,15|a=2400"}`.
- `SHELL(thickness, faces)`: `{"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": <thickness>, "remove_faces": <faces>}` (omit `remove_faces` when empty).

Bodies `[b-1]`.

1. **Open top.** `BOX805030`, `SHELL("2", [F-TOP])`. Expected: ok; volume 22112, 11 faces, bbox [-40,-25,0]-[40,25,30]; `f-sh1/inner/f-box1/bottom` planar at z=2 with area 3496; `f-sh1/inner/f-box1/front` planar at y=-23 with area 2128; `f-sh1/rim/f-box1/top` planar at z=30 with area 504; no face named `f-box1/top`; `f-box1/bottom` area 4000; change `Modify`.
2. **Two faces removed.** `BOX805030`, `SHELL("2", [F-TOP, F-FRONT])`. Expected: volume 17856, 10 faces; `f-sh1/rim/f-box1/top` area 352; `f-sh1/rim/f-box1/front` area 272.
3. **Closed hollow.** `BOX805030`, `SHELL("2", [])`. Expected: volume 29104, 12 faces, bbox [-40,-25,0]-[40,25,30]; 6 faces named `f-sh1/inner/...`; `f-box1/top` exists; the body is one solid with two shells.
4. **Parameter thickness.** Parameters `[{"id": "p-wall", "name": "wall", "expr": "2 mm"}]`, `BOX805030`, `SHELL("wall", [F-TOP])`. Expected: identical to 1.
5. **Too thick.** `BOX805030`, `SHELL("30", [F-TOP])`. Expected: error `shell_too_thick`, field `thickness`, `thickness == 30`, `max == Some(m)` with 22.5 ≤ m < 25; rendered `Shell thickness 30 mm is too thick for body 'Body 1'. The thickest wall that works is about {s} mm. Try {s} mm or less.` with `{s}` = m rounded down to 2 significant digits; b-1 unchanged. Rebuilding with `"{s}"` is ok.
6. **Zero thickness.** `BOX805030`, `SHELL("0", [F-TOP])`. Expected: error `out_of_range` rendered `Thickness must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.`
7. **Removed face lost.** `BOX805030`, `SHELL("2", [{"q": "f-box9/top", "fp": "plane|n=0,0,1|c=500,500,500|a=1"}])`. Expected: error `references_not_found` rendered `The face selected for Faces to remove can no longer be found. Re-pick it in the repair panel.`; b-1 unchanged (it does not silently become a closed hollow).
8. **One of two removed faces lost.** `BOX805030`, `SHELL("2", [F-TOP, {"q": "f-box9/top", "fp": "plane|n=0,0,1|c=500,500,500|a=1"}])`. Expected: warning `reference_missing` at position 1; identical geometry to 1.
9. **Cylinder cup.** `{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "20", "height": "40", "op": "new"}`, `SHELL("2", [{"q": "f-cyl1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=1256.637061"}])`. Expected: volume 11586.193706, 5 faces, bbox [-20,-20,0]-[20,20,40].
10. **Empty body.** `SHELL("2", [])` alone. Expected: error `body_empty`.
11. **Python.** `Registry::python` of the JSON in section 2 renders exactly the Python in section 2. With thickness `"2"` and no removed faces it renders `shell(id="f-sh1", body="b-1", thickness=2)`.
12. **Performance.** Test 1 builds in under 50 ms (desktop, release).

## Open issues

1. Test 5 bounds the suggestion (the true limit is just under 25 mm, half the 50 mm width) instead of fixing it, because the exact point where the kernel's offset stops succeeding is a kernel property.
2. The rim naming assumes the kernel facade reports rim faces as generated from the removed face (or from its edges, merged by refinement). kernel.md should confirm which history OCCT's thick solid provides.
