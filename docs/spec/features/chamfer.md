# Feature: chamfer

Status: M1 spec. Type name `chamfer`. Shared contracts: [features.md](../features.md). Naming rule: [naming.md](../naming.md) section 3.6.6. Kernel operation: [kernel.md](../kernel.md) section 3.4.7.

## 1. Purpose and scope

Chamfer bevels edges of a body with a flat face. Convex edges lose material, concave edges gain material.

### In M1

- Equal distance on both faces of each edge.
- Two distances, measured on the faces the user picked the edge between (the first face of the stored reference gets the first distance), with a `flip` to swap them.
- Tangent chains and corner faces as the kernel produces them.
- A search for the largest working distance when the requested one fails.

### Later

- Distance and angle.
- Per-edge flip and per-edge distances.
- Corner setbacks.
- Print presets such as an elephant's-foot chamfer on bed-contact edges.

## 2. Public API

```rust
/// Bevels edges of a body.
pub struct Chamfer;

impl Feature for Chamfer {
    const TYPE: &'static str = "chamfer";
    type Params = ChamferParams;
}

/// Params of a chamfer feature.
pub struct ChamferParams {
    /// The body whose edges are beveled.
    pub body: Id,
    /// Edges to bevel, at least one.
    pub edges: Vec<Reference>,
    /// Distances from each edge.
    pub distance: ChamferDistance,
}

/// How far a chamfer cuts back on each face of an edge.
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChamferDistance {
    /// `expr` on both faces. Length, greater than 0.
    Equal { expr: Expr },
    /// `expr` on the first face and `second_expr` on the second face of each edge, where the first
    /// face is the first face of the edge's stored reference (section 3.3).
    /// Lengths, greater than 0. `flip` swaps them; default false, omitted when false.
    TwoDistances { expr: Expr, second_expr: Expr, flip: bool },
}
```

JSON:

```json
{"id": "f-cha1", "type": "chamfer", "body": "b-1",
 "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}],
 "distance": {"kind": "equal", "expr": "1 mm"}}
```

Two distances: `"distance": {"kind": "two_distances", "expr": "1 mm", "second_expr": "3 mm"}`, with `"flip": true` when flipped.

### Manifest

Label `Chamfer`, category `modify`, icon `chamfer`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `edges` | Edges | yes | | `edge` | |
| `distance.kind` | Type | yes | | | `equal` Equal distance, `two_distances` Two distances |
| `distance.expr` | Distance | yes | length | | |
| `distance.second_expr` | Second distance | yes | length | | |
| `distance.flip` | Flip | no | | | |
| `body` | Body | no | | `body` | |

### Python

Dedicated form: `chamfer(edges, distance, second_distance=..., flip=...)` (python-api.md section 2.7), produced by the `Feature::python` hook with the shared rules of features.md section 3.11: `edges` as references (one alone, several as a list), `distance` from `distance.expr`; for `two_distances`, `second_distance=` with `distance.second_expr`, then `flip=True` when `flip` is true ([ADR 0048](../../adr/0048-python-keywords-gui-round-trip-and-python-budgets.md)); the `with <part>:` block gives `body`. Both kinds have a dedicated form, so the hook never returns `None`. With the edge's reference written `Ref("edge(f-box1/front|f-box1/top)", "line|n=1,0,0|c=0,-10,20|l=20")`, the JSON above is `chamfer(Ref(...), 1*mm, id="f-cha1")`, and its two distances flipped (`"expr": "1 mm", "second_expr": "3 mm", "flip": true`) are `chamfer(Ref(...), 1*mm, second_distance=3*mm, flip=True, id="f-cha1")`. Generic form of the JSON above:

```python
feature(
    "chamfer",
    id="f-cha1",
    body="b-1",
    edges=[{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}],
    distance={"kind": "equal", "expr": "1 mm"},
)
```

## 3. Behavior

### 3.1 Validation

- `edges` is not empty, else `EmptySelection { field: "edges", noun: edge }`.

### 3.2 Dependencies

- `bodies_written`: `[body]`.
- `expressions`: `("distance.expr", expr, Length)`, plus `("distance.second_expr", second_expr, Length)` for two distances.

### 3.3 Build

1. The body must hold a solid, else `BodyEmpty { field: body, body, use_new: false }` (recompute's diagnosis adds why it is empty, features.md section 4.1).
2. Each distance must be greater than 0, else `OutOfRange` (`GreaterThan(0)`) on its field, checked in field order, by the positive length rule of features.md section 3.4: a distance below `LINEAR_TOLERANCE` (1e-6 mm) is out of range like 0, before any kernel work. Then each distance is checked against the modeling limit (`AtMost(MODEL_LIMIT)`, features.md section 3.4).
3. Resolve `edges` as in [fillet.md](fillet.md) step 3 (its warnings and notices go into the output), and skip smooth edges as in fillet step 4 (`Ignored { reason: SmoothEdge }`; when none remain and `edges` holds two or more entries, `NoEffect` with `how: Skipped { field: edges }`, features.md section 3.15).
4. Face assignment for two distances: for each edge, the first face is the adjacent face that corresponds to the first face term of the stored reference the edge was resolved from (`Resolved::sources`): the face in the family of that term, through canonical names (naming.md section 3.9), so a face that kept the term's name, a face that took it as an alias when a join merged it, and a piece of it all count. So the side follows the face the user picked the edge from, whatever names a later join gives the faces (corpus R22). When no adjacent face corresponds (the edge was reached by continuation or relation), the first face is the adjacent face whose element name sorts first in byte order, and the resolution's `ReferenceFollowed` notice (features.md section 3.8) tells the user; a two-distance chamfer adds that notice itself when the resolution gave none. `expr` is measured on the first face and `second_expr` on the other face (kernel `ChamferSize::TwoDistances { distance, other_distance, face }` with `face` the first face's index). `flip: true` swaps the two for every edge. The resolved edges are passed to the kernel sorted by their element names in byte order (for every distance kind), so the result never depends on the order of `edges`: the kernel takes the side of a whole tangent chain from the first of its edges it receives, which is the chain's edge whose name sorts first. Edges the kernel adds along a tangent chain follow that side.
5. Kernel `chamfer` with every edge. On kernel failure (except `InvalidInput` and `Internal`), run the "too large" search (features.md section 4.3; both distances scale together), measure the `limit` and decide `removable` as that section says, and fail with `ChamferTooLarge { field, expr, distance, max, limit, cause }`, where `field` is `distance.expr` for an equal distance and `distance` for two distances, `expr` the equal distance's expression (None for two distances), `distance` the larger requested distance and `max` the larger working scaled distance. Only the equal-distance error carries a suggestion and a `SetField` fix (features.md section 4.4).
6. Name and return one `BodyUpdate` with change `Modify`.

### 3.4 Element names

| Name | Face |
| --- | --- |
| `<id>/chamfer/<edge name>` | The chamfer face along the input edge named `<edge name>`. |
| `<id>/corner/<vertex name>` | A corner face at an input vertex where the kernel adds one. |

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| No edges listed | `EmptySelection` | `Select at least one edge for Edges.` |
| Body empty | `BodyEmpty` | without a cause: `Body 'Body 1' has no solid at this point, so this chamfer has nothing to work on. Pick a body that has a solid.`; with one, features.md section 4.1 (`Body 'Body 1' is empty here because 'Extrude 1', which makes it, failed. Fix 'Extrude 1' first.`) |
| Distance below 1e-6 mm (0 and negative included) | `OutOfRange` | `Distance must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Second distance below 1e-6 mm | `OutOfRange` | `Second distance must be greater than 0 mm, but it is -1 mm. Use a value greater than 0 mm.` |
| Distance above the modeling limit | `OutOfRange` | `Distance must be at most 1000000 mm, but it is 10000000 mm. Use 1000000 mm or less.` |
| Expression or dimension error | `Expression` | `Distance: Expected a length, but this expression gives an angle.` |
| No edge resolves | `ReferencesNotFound` | `The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this chamfer.` (features.md section 4.1) |
| Kernel failure | `ChamferTooLarge` | `Chamfer distance 3 mm is too large for the selected edges: the largest that works is about 2.3 mm. The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across.` (features.md section 4.3) |
| Kernel failure of a kind that is not about size | `Kernel` | `Partsmith's geometry kernel could not build this chamfer. {hint}` (features.md section 4.1) |
| Warnings and notices | `ReferenceLost`, `ReferenceGuessed`, `ReferenceFollowed`, `ReferenceWidened`, `Ignored { SmoothEdge }`, `NoEffect` (`Skipped`) | features.md sections 3.15 and 4.2 |

## 5. Performance

- 1 edge of a box: 10 ms. 12 edges of a box: 100 ms.
- A failing chamfer including the search: 1.2 s.

## 6. Acceptance tests

Conventions from features.md section 6.1; fixtures `CUBE20`, `E-FT`, `E-BT`, `E-LT`, `E-RT`, `E-LOST` and the edge naming from [fillet.md](fillet.md) section 6. `CHA(edges, distance)`: `{"id": "f-cha1", "type": "chamfer", "body": "b-1", "edges": <edges>, "distance": <distance>}`. Bodies `[b-1]`.

1. **Equal distance.** `CUBE20`, `CHA([E-FT], {"kind": "equal", "expr": "2"})`. Expected: ok; volume 7960, 7 faces, bbox [-10,-10,0]-[10,10,20]; face `f-cha1/chamfer/edge(f-box1/front|f-box1/top)` planar with area 56.568542; `f-box1/top` area 360; `f-box1/front` area 360; change `Modify`.
2. **Two distances.** `CUBE20`, `CHA([E-FT], {"kind": "two_distances", "expr": "1", "second_expr": "3"})`. `f-box1/front` is the first face term of the stored reference, so 1 mm is measured on the front face. Expected: volume 7970, 7 faces; `f-box1/front` area 380; `f-box1/top` area 340; chamfer face area 63.245553.
3. **Two distances flipped.** As 2 with `"flip": true`. Expected: volume 7970; `f-box1/front` area 340; `f-box1/top` area 380.
4. **Top loop.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "10"], "op": "new"}`, `CHA([E-FT, E-BT, E-LT, E-RT], {"kind": "equal", "expr": "2"})`. Expected: volume 3850.666667, 10 faces, bbox [-10,-10,0]-[10,10,10]; `f-box1/top` area 256.
5. **Concave edge adds material.** The L-shaped body of [fillet.md](fillet.md) test 4, `CHA([{"q": "edge(f-box1/top|f-box2/left)", "fp": "line|n=0,1,0|c=0,0,10|l=20"}], {"kind": "equal", "expr": "2"})`. Expected: volume 12040, 9 faces.
6. **Too large.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "2.4"], "op": "new"}`, `CHA([E-FT, E-BT, E-LT, E-RT], {"kind": "equal", "expr": "3"})`. Expected: error `chamfer_too_large`, field `distance.expr`, `distance == 3`, `max == Some(m)` with 2.0 ≤ m < 2.4, `limit` the face `f-box1/back` across `edge(f-box1/back|f-box1/top)` with width 2.4 (the four side faces tie, and that edge's name sorts first), `removable` false (the other three edges fail without it); rendered `Chamfer distance 3 mm is too large for the selected edges: the largest that works is about {s} mm. The back top edge of 'Box 1' lies next to the back side of 'Box 1', which is only 2.4 mm across.` with `{s}` = m rounded down to 2 significant digits; fixes [Use {s} mm] only; suggestion `{s} mm`; b-1 unchanged (volume 960). Rebuilding with `"{s}"` is ok.
7. **Zero distance.** `CUBE20`, `CHA([E-FT], {"kind": "equal", "expr": "0"})`. Expected: error `out_of_range`, field `distance.expr`.
8. **Negative second distance.** `CUBE20`, `CHA([E-FT], {"kind": "two_distances", "expr": "1", "second_expr": "-1"})`. Expected: error `out_of_range`, field `distance.second_expr`, rendered `Second distance must be greater than 0 mm, but it is -1 mm. Use a value greater than 0 mm.`
9. **No edges.** `CHA([], {"kind": "equal", "expr": "1"})`. Expected: error `empty_selection`.
10. **Edge lost.** `CUBE20`, `CHA([E-FT, E-LOST], {"kind": "equal", "expr": "2"})`. Expected: warning `reference_missing` with pointer `/edges/1`; volume 7960.
11. **Performance.** Test 4 builds in under 60 ms (desktop, release).
12. **Edge order does not matter.** The body after `f-fil1` of fillet.md test 11, `CHA([edge(f-box1/front|f-box1/top), edge(f-box1/top|f-fil1/fillet/edge(f-box1/front|f-box1/left))], {"kind": "two_distances", "expr": "1", "second_expr": "2"})`, as `examples/chamfer/edge-order`, and the same chamfer with the two edges listed in the other order (the example with its `edges` reversed, made by the test) give the same volume (within 1e-9 relative) and the same face names.
13. **Chamfer beside a fillet on a pipe tee.** The features up to `f-fil1` of [fillet.md](fillet.md) test 16, then `CHA([{"q": "edge(f-cyl2/side|f-fil1/fillet/edge(f-cyl1/side|f-cyl2/side))", "fp": "bspline_curve|c=0,0,10.524948|l=40.038448"}], {"kind": "equal", "expr": "1"})`. Expected: f-cha1 status warning with exactly one warning, `ignored` with reason `smooth_edge` (one edge, so no `no_effect`, features.md section 3.15); b-1 identical to the state after f-fil1.
14. **Distance below the tolerance.** `CUBE20`, `CHA([E-FT], {"kind": "two_distances", "expr": "1", "second_expr": "0.0000001"})`. Expected: error `out_of_range`, field `distance.second_expr`, rendered `Second distance must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm.` (the value is written with the digits that tell it apart from the bound, features.md section 3.12); b-1 unchanged.
15. **The sides survive a join that renames the top (corpus R22 a).** `CUBE20`; inserted before the chamfer, `{"id": "f-base1", "type": "box", "body": "b-1", "size": ["20", "40", "20"], "position": ["20", "0", "0"], "op": "join"}` (x 10..30, y -20..20: the tops merge as `f-base1/top`, the smaller name, with the alias `f-box1/top`; the fronts are not coplanar); `CHA([{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2"}], {"kind": "two_distances", "expr": "1", "second_expr": "3"})`, as `examples/chamfer/join-renames-top`. Expected: f-cha1 status ok; the edge resolves through the alias to `edge(f-base1/top|f-box1/front)`, whose name now sorts the top first, and the chamfer keeps its sides: 1 mm down the front (the chamfer face's edge on `f-box1/front` lies at z = 19) and 3 mm into the top (its edge on the merged top lies at y = -7). The same document with the join's id `f-side1` (sorting after `f-box1`, so the merged top is `f-box1/top`; made by the test from the example) gives the same chamfer face and the same volume.
16. **The sides survive a join behind (corpus R22 b).** `CUBE20`; inserted before the chamfer, `{"id": "f-a1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "position": ["0", "20", "0"], "op": "join"}` (x -10..10, y 10..30: the tops merge as `f-a1/top` with the alias `f-box1/top`); the chamfer of test 15; as `examples/chamfer/join-behind`. Expected: f-cha1 status ok; `f-box1/front` area 380, the merged top area 740; b-1 volume 15970. The same document with the join's id `f-rear1` (sorting after `f-box1`; made by the test from the example) gives `f-box1/front` area 380, `f-box1/top` area 740 and volume 15970: the side never depends on the inserted feature's id.
17. **The limiting face is named (newcomer error corpus NC05, as a chamfer).** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "20", "10"], "op": "new"}` (x -20..20, y -10..10, z 0..10); `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["20", "30", "5"], "position": ["10", "0", "7.6"], "op": "cut"}` (a step 2.4 mm deep over x 0..20, whose riser `f-box2/left` lies at x = 0); `CHA([{"q": "edge(f-box1/top|f-box2/left)", "fp": "line|n=0,1,0|c=0,0,10|l=20|adj=2,2"}], {"kind": "equal", "expr": "3"})`. Expected: error `chamfer_too_large`, `max == Some(m)` with 2.0 ≤ m < 2.4, `limit` the face `f-box2/left` with width 2.4, rendered `Chamfer distance 3 mm is too large for the selected edges: the largest that works is about {s} mm. The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across.` with `{s}` = m rounded down to 2 significant digits; fixes [Use {s} mm]. Then: f-cha1 status ok.
18. **Remove the edge that limits it.** As 17 with edges `[{"q": "edge(f-box1/top|f-box2/left)", "fp": "line|n=0,1,0|c=0,0,10|l=20|adj=2,2"}, {"q": "edge(f-box1/bottom|f-box1/left)", "fp": "line|n=0,1,0|c=-20,0,0|l=20|adj=2,2"}]`. Expected: error `chamfer_too_large` with the message of test 17 and `removable` true (one more attempt at 3 mm without the first edge works); fixes [Use {s} mm] then `RemoveEntry { pointer: "/edges/0" }` (Remove from Edges). Then (`RemoveEntry`): f-cha1 status ok, with only the bottom left edge chamfered at 3 mm: b-1 volume 6950 (the stepped body's 7040 less 90).
19. **Every edge of a cube.** `CUBE20`, `CHA` with all 12 edges (`E-FT`, `E-BT`, `E-LT`, `E-RT` and the other eight of [fillet.md](fillet.md) section 6), `{"kind": "equal", "expr": "2"}`, as `examples/chamfer/every-edge`. Expected: ok; volume 7562.666667 (8000 less a prism of section 2 along each edge without its last 2 mm at either end, 384 in all, and 20/3 at each corner), 26 faces, bbox [-10,-10,0]-[10,10,20]; `f-box1/top` planar with area 256; 12 faces named `f-cha1/chamfer/<edge name>`, one per edge, each planar with area 32√2 = 45.254834, where each `f-cha1/chamfer/edge(a|b)` has as normal the normalized sum of the outward normals of `a` and `b`, and its centroid is the edge's midpoint moved 1 mm against each of those normals (for `edge(f-box1/front|f-box1/top)`: normal (0,-1,1)/√2, centroid (0,-9,19)); 8 faces named `f-cha1/corner/<vertex name>`, one per corner of the cube, where `<vertex name>` is `vertex(<its three faces in byte order>)` (such as `vertex(f-box1/front|f-box1/left|f-box1/top)`, the corner at (-10,-10,20)), each planar, bounded by 3 edges and 3 vertices (`Shape::topology`), with area 2√3 = 3.464102 and its centroid 4/3 mm from its corner along each axis, toward the inside of the cube (for that corner, at (-26/3, -26/3, 56/3)); no other faces.

## Open issues

1. **Resolved by the side rule of section 3.3 step 4** (reference stability and errors study section 3.5.2). The first distance is measured on the face the user picked the edge from (the stored reference's first face term), which a later join can no longer change by renaming a face (tests 15 and 16). The inspector previews which face gets which distance and offers Flip; a reference face per edge set remains a later option.
2. **Resolved:** selected edges of one tangent chain can disagree on their first face (on a box whose vertical edges are filleted, `edge(f-box1/front|f-box1/top)` has the wall first and `edge(f-box1/top|f-fil1/fillet/edge(f-box1/front|f-box1/left))` the top first), and the kernel takes a chain's side from the first edge it receives. Step 4 now sorts the edges by name before the kernel call, so the chain's edge whose name sorts first sets the side and listing order no longer changes the geometry. A reference face per edge set (open issue 1) would make the side explicit.
3. Corner face counts (test 19) assume the kernel closes the corner where three equal chamfers meet with one planar triangle, reported as generated by that corner's vertex, as OCCT 7.9.3 does (kernel.md test 142). A kernel that ran the three chamfer faces into one point would give 18 faces and no `corner` names.
