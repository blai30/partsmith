# Feature: fillet

Status: M1 spec. Type name `fillet`. Shared contracts: [features.md](../features.md). Naming rule: [naming.md](../naming.md) section 3.6.5. Kernel operation: [kernel.md](../kernel.md) section 3.4.6.

## 1. Purpose and scope

Fillet rounds edges of a body with a constant radius. Convex edges lose material, concave edges gain material.

### In M1

- Constant radius on one or more edges of one body.
- Tangent chains: edges that continue a selected edge without a corner are filleted with it.
- Corner blends where three or more filleted edges meet.
- A search for the largest working radius when the requested radius fails.

### Later

- Variable radius along an edge.
- Chord-length fillets.
- Setback corners.
- G2 (curvature-continuous) fillets.
- Rule fillets by face groups, face-to-face fillets, full round fillets.
- Quick selections such as "all vertical edges" (a UI and selector feature).

## 2. Public API

```rust
/// Rounds edges of a body with a constant radius.
pub struct Fillet;

impl Feature for Fillet {
    const TYPE: &'static str = "fillet";
    type Params = FilletParams;
}

/// Params of a fillet feature.
pub struct FilletParams {
    /// The body whose edges are rounded.
    pub body: Id,
    /// Edges to round, at least one.
    pub edges: Vec<Reference>,
    /// Fillet radius. Length, greater than 0.
    pub radius: Expr,
}
```

JSON:

```json
{"id": "f-fil1", "type": "fillet", "body": "b-1",
 "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}], "radius": "2 mm"}
```

### Manifest

Label `Fillet`, category `modify`, icon `fillet`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `edges` | Edges | yes | | `edge` | |
| `radius` | Radius | yes | length | | |
| `body` | Body | no | | `body` | |

### Python

Dedicated form: `fillet(edges, radius)` (python-api.md section 2.7), produced by the `Feature::python` hook with the shared rules of features.md section 3.11: `edges` as references (one alone, several as a list), `radius` as an expression; the `with <part>:` block gives `body`. The JSON above is `fillet(Ref("edge(f-box1/front|f-box1/top)", "line|n=1,0,0|c=0,-10,20|l=20"), 2*mm, id="f-fil1")`, which is longer than 88 characters and so is written one argument per line. Generic form:

```python
feature(
    "fillet",
    id="f-fil1",
    body="b-1",
    edges=[{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}],
    radius="2 mm",
)
```

## 3. Behavior

### 3.1 Validation

- `edges` is not empty, else `EmptySelection { field: "edges", noun: edge }`.

### 3.2 Dependencies

- `bodies_written`: `[body]`.
- `expressions`: `("radius", radius, Length)`.

### 3.3 Build

1. The body must hold a solid, else `BodyEmpty { field: body, body, use_new: false }` (recompute's diagnosis adds why it is empty, features.md section 4.1).
2. r = `radius`; r > 0 else `OutOfRange` (`GreaterThan(0)`), by the positive length rule of features.md section 3.4: a radius below `LINEAR_TOLERANCE` (1e-6 mm) is out of range like 0, before any kernel work; then r is at most the modeling limit, else `OutOfRange` (`AtMost(MODEL_LIMIT)`, features.md section 3.4).
3. Resolve `edges` with `ctx.resolve_all("edges", body, edges, Edge)` (features.md section 3.8). Its warnings and notices (`ReferenceLost`, `ReferenceGuessed`, `ReferenceFollowed`, `ReferenceWidened`) go into the output; a list that resolves to nothing gives `ReferencesNotFound` with every loss, in reference order.
4. Skip smooth edges. A resolved edge is smooth when the body's topology (`BodyState::topology`, features.md section 2.6) lists fewer than two faces for it (a seam, or a degenerated edge such as a sphere pole), or when it has exactly two faces that meet without a corner. Kernel `Shape::edge_normals` describes the edge at the middle of its parameter range (kernel.md section 3.6 item 5), and its two faces meet without a corner when the continuity the shape records between them is `G1OrBetter` (item 5.6), or when their outward normals there make an angle of at most 1e-3 rad. Any other edge has a corner and is kept, including a knife edge (normals opposite) and an edge for which the query fails with `InvalidInput` because a face's normal is undefined at the edge's middle. Each skipped edge adds an `Ignored { reason: SmoothEdge }` warning, whose fix removes that entry when its reference resolved to that edge alone and no other entry selects the edge. If no edge remains, the body is unchanged, and when `edges` holds two or more entries a `NoEffect` warning with `how: Skipped { field: edges }` is added (features.md section 3.15); a single skipped edge is reported by its `Ignored` warning alone. The recorded continuity covers the tangent edges OCCT's blends make, whose approximated surfaces leave the normals of a fillet and the faces it blends into slightly apart (test 16). The angle covers tangent edges without a record, such as edges a boolean rebuilt; 1e-3 rad (about 0.06 deg) lies far above those deviations and far below any corner a printed part is designed with.
5. Kernel `fillet` with every remaining edge at radius r, the edges sorted by their element names in byte order (the order chamfer.md section 3.3 step 4 uses), so the order of `edges` never changes the result: the kernel's blend surfaces, and with them the last bits of the volume and the BRep bytes, depend on the order it receives the edges in. The kernel extends each contour along tangent chains.
6. If the kernel fails (any `KernelError` except `InvalidInput` and `Internal`, which are `Kernel` errors), run the "too large" search (features.md section 4.3): bisect for `max`, measure the narrowest face next to a selected edge for `limit` (kernel `width_across`, on this failure path only), decide `removable` with one more attempt without the limiting edge's reference when another reference resolved, and fail with `FilletTooLarge { field: radius, expr: radius, radius: r, max, limit, cause }`. Its message and fixes are those of features.md section 4.3, including the bare-number sentence in a document whose units are not millimeters (corpus R48).
7. Name (section 3.4) and return one `BodyUpdate` with change `Modify`.

### 3.4 Element names

| Name | Face |
| --- | --- |
| `<id>/fillet/<edge name>` | The fillet face that rolls along the input edge named `<edge name>`, including edges added by tangent chains. |
| `<id>/corner/<vertex name>` | The blend face at an input vertex named `<vertex name>` where three or more fillets meet. |

All other faces keep their names (faces trimmed by the fillet are modified, not renamed).

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| No edges listed | `EmptySelection` | `Select at least one edge for Edges.` |
| Body empty | `BodyEmpty` | without a cause: `Body 'Body 1' has no solid at this point, so this fillet has nothing to work on. Pick a body that has a solid.`; with one, features.md section 4.1 |
| Radius below 1e-6 mm (0 and negative included) | `OutOfRange` | `Radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Radius above the modeling limit | `OutOfRange` | `Radius must be at most 1000000 mm, but it is 10000000 mm. Use 1000000 mm or less.` |
| Expression or dimension error | `Expression` | `Radius: Expected a length, but this expression gives an angle.` |
| No edge resolves | `ReferencesNotFound` | one: `The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet.`; several: `None of the 2 edges selected for Edges are left: the front top edge of 'Box 1' was cut away by 'Extrude 1', which comes before this fillet, and 1 more.` (features.md section 4.1) |
| Kernel failure | `FilletTooLarge` | `Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm. The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across.` (features.md section 4.3) |
| Kernel failure of a kind that is not about size | `Kernel` | `Partsmith's geometry kernel could not build this fillet. {hint}` (features.md section 4.1) |
| Edge lost, ambiguous, guessed, followed or widened | warning `ReferenceLost`, `ReferenceGuessed`, `ReferenceWidened`, notice `ReferenceFollowed` | features.md section 4.2 |
| Smooth edge skipped | warning `Ignored { reason: SmoothEdge }` | `One of the edges selected for Edges is smooth (its faces meet without a corner), so it was skipped.` |
| Nothing left to fillet, two or more edges selected | warning `NoEffect` (`Skipped`) | `This fillet changes nothing in body 'Body 1', because every edge selected for Edges was skipped.` |

## 5. Performance

- 1 edge of a box: 15 ms. 12 edges of a box: 100 ms.
- A failing fillet including the search: 1.2 s.

## 6. Acceptance tests

Conventions from features.md section 6.1. Fixtures:

- `CUBE20`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"}` (spans [-10,-10,0]-[10,10,20]).
- Edge references on a box `f-box1` (fingerprints shown for `CUBE20`; tests on other boxes use the same names):
  - `E-FT`: `{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}`
  - `E-BT`: `{"q": "edge(f-box1/back|f-box1/top)", "fp": "line|n=1,0,0|c=0,10,20|l=20"}`
  - `E-LT`: `{"q": "edge(f-box1/left|f-box1/top)", "fp": "line|n=0,1,0|c=-10,0,20|l=20"}`
  - `E-RT`: `{"q": "edge(f-box1/right|f-box1/top)", "fp": "line|n=0,1,0|c=10,0,20|l=20"}`
  - The other eight edges of a box are named the same way from their face pairs in byte order: `edge(f-box1/bottom|f-box1/front)`, `edge(f-box1/back|f-box1/bottom)`, `edge(f-box1/bottom|f-box1/left)`, `edge(f-box1/bottom|f-box1/right)`, `edge(f-box1/front|f-box1/left)`, `edge(f-box1/front|f-box1/right)`, `edge(f-box1/back|f-box1/left)`, `edge(f-box1/back|f-box1/right)`; their fingerprints are `line|n=<axis>|c=<midpoint>|l=<length>`.
- `E-LOST`: `{"q": "edge(f-box1/nothing|f-box1/top)", "fp": "line|n=0,0,1|c=500,500,500|l=1"}`.
- `FIL(edges, radius)`: `{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": <edges>, "radius": <radius>}`.

Bodies `[b-1]` unless stated.

1. **One convex edge.** `CUBE20`, `FIL([E-FT], "2")`. Expected: ok; volume 7982.831853, 7 faces, bbox [-10,-10,0]-[10,10,20]; face `f-fil1/fillet/edge(f-box1/front|f-box1/top)` is a cylinder of radius 2 with area 62.831853; `f-box1/top` area 360; `f-box1/front` area 360; change `Modify`.
2. **Top loop of a box.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["30", "20", "10"], "op": "new"}`, `FIL([E-FT, E-BT, E-LT, E-RT], "2")`. Expected: volume 5917.227116, 10 faces, bbox [-15,-10,0]-[15,10,10]; `f-box1/top` area 416; 4 faces named `f-fil1/fillet/...`, one per edge; no `f-fil1/corner/...` faces.
3. **Every edge of a cube.** `CUBE20`, `FIL` with all 12 edges, radius `"2"`. Expected: volume 7804.696111, 26 faces, bbox [-10,-10,0]-[10,10,20]; 12 faces named `f-fil1/fillet/...`; 8 spherical faces named `f-fil1/corner/...`.
4. **Concave edge adds material.** Features `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "20", "10"], "op": "new"}`, `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["20", "20", "10"], "position": ["10", "0", "10"], "op": "join"}`, `FIL([{"q": "edge(f-box1/top|f-box2/left)", "fp": "line|n=0,1,0|c=0,0,10|l=20"}], "2")`. Expected: ok; volume 12017.168147, 9 faces, bbox [-20,-10,0]-[20,10,20].
5. **Inch document.** Document `"units": "in"`, `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20 mm", "20 mm", "20 mm"], "op": "new"}` (the size of `CUBE20`, which in an inch document would be 20 in), `FIL([E-FT], "0.1")`. Expected: volume 8000 - (1 - π/4) × 2.54² × 20 = 7972.309496 (radius 2.54 mm).
6. **One edge lost.** `CUBE20`, `FIL([E-FT, E-LOST], "2")`. Expected: status warning (`reference_missing`, field `edges`); volume 7982.831853. The warning's `repair()` is `Element` with pointer `/edges/1` and body `b-1`.
7. **Every edge lost.** `CUBE20`, `FIL([{"q": "edge(f-box9/front|f-box9/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}], "2")` (an edge of a feature that is not in the document). Expected: error `references_not_found` rendered `An edge of a feature that is not in this document selected for Edges is gone.`; its first fix is `UseReferences` with `edge(f-box1/front|f-box1/top)` (the nearest candidate, at distance 0), its last `Pick`; b-1 unchanged (volume 8000, 6 faces).
8. **Radius too large.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "2.4"], "op": "new"}`, `FIL([E-FT, E-BT, E-LT, E-RT], "3")`. Expected: error `fillet_too_large`, field `radius`, `radius == 3`, `max == Some(m)` with 2.0 ≤ m < 2.4 and m = 3k/1024 for an integer k (a bisection point); `limit` the face `f-box1/back` across `edge(f-box1/back|f-box1/top)`, width 2.4 (the four side faces tie, and that edge's name sorts first), `removable` false; rendered `Fillet radius 3 mm is too large for the selected edges: the largest that works is about {s} mm. The back top edge of 'Box 1' lies next to the back side of 'Box 1', which is only 2.4 mm across.` where `{s}` is m rounded down to 2 significant digits; fixes [Use {s} mm]; suggestion `{s} mm`; b-1 unchanged (volume 960). Rebuilding with radius `"{s}"` is ok.
9. **Zero radius.** `CUBE20`, `FIL([E-FT], "0")`. Expected: error `out_of_range` rendered `Radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.`
10. **No edges.** `FIL([], "2")`. Expected: error `empty_selection` rendered `Select at least one edge for Edges.`
11. **Tangent chain.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["30", "20", "10"], "op": "new"}`; `{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [<the four vertical edges: front|left, front|right, back|left, back|right>], "radius": "3"}`; `{"id": "f-fil2", "type": "fillet", "body": "b-1", "edges": [E-FT], "radius": "1"}`. Expected: both ok; after f-fil1: volume 5922.743339, 10 faces; after f-fil2: volume 5902.689635, 18 faces, bbox [-15,-10,0]-[15,10,10]; 8 faces named `f-fil2/fillet/...` (the whole top loop).
12. **Smooth edge skipped.** As 11, with f-fil2 edges `[{"q": "edge(f-box1/front|f-fil1/fillet/edge(f-box1/front|f-box1/left))", "fp": "line|n=0,0,1|c=-12,-10,5|l=10"}]`. Expected: f-fil2 status warning with exactly one warning, `ignored` with reason `smooth_edge` (one edge, so no `no_effect`, features.md section 3.15), whose fix is `RemoveEntry { pointer: "/edges/0" }`; b-1 identical to the state after f-fil1. With both tangent edges of that fillet face, f-fil2 edges `[{"q": "edge(f-box1/front|f-fil1/fillet/edge(f-box1/front|f-box1/left))", "fp": "line|n=0,0,1|c=-12,-10,5|l=10"}, {"q": "edge(f-box1/left|f-fil1/fillet/edge(f-box1/front|f-box1/left))", "fp": "line|n=0,0,1|c=-15,-7,5|l=10"}]`: status warning (`ignored` twice, then `no_effect` with how `Skipped { field: edges }`, rendered `This fillet changes nothing in body 'Body 1', because every edge selected for Edges was skipped.`); b-1 identical to the state after f-fil1. With that edge in f-fil2's edges twice: warnings `ignored` with field `edges` and no fix (removing one entry would leave the other selecting the edge, features.md section 4.2), then `no_effect` (`Skipped`, two entries); b-1 unchanged.
13. **Empty body.** `FIL([E-FT], "2")` alone. Expected: error `body_empty` rendered `Body 'Body 1' has no solid at this point, so this fillet has nothing to work on. Pick a body that has a solid.` (no feature makes the body, so the diagnosis gives no cause).
14. **Edges of another body.** Bodies `[b-1, b-2]`; `CUBE20` in b-1; `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["10", "10", "10"], "position": ["50", "0", "0"], "op": "new"}`; `FIL([{"q": "edge(f-box2/front|f-box2/top)", "fp": "line|n=1,0,0|c=50,-5,10|l=10"}], "1")` on b-1. Expected: error `references_not_found`; b-1 and b-2 unchanged.
15. **Performance.** Test 3 builds in under 100 ms; test 8 including the search in under 1.2 s (desktop, release).
16. **Fillet beside a fillet on a pipe tee.** Features `{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "10", "height": "60", "plane": {"kind": "origin", "plane": "YZ"}, "position": ["0", "0", "-30"], "op": "new"}` (the run, along X from x=-30 to 30), `{"id": "f-cyl2", "type": "cylinder", "body": "b-1", "radius": "6", "height": "25", "op": "join"}` (the branch, up to z=25), `{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{"q": "edge(f-cyl1/side|f-cyl2/side)", "fp": "bspline_curve|c=0,0,9.026701|l=38.725445"}], "radius": "2"}` (the joint), then `{"id": "f-fil2", "type": "fillet", "body": "b-1", "edges": [{"q": "edge(f-cyl2/side|f-fil1/fillet/edge(f-cyl1/side|f-cyl2/side))", "fp": "bspline_curve|c=0,0,10.524948|l=40.038448"}], "radius": "1"}`, the edge where the joint's fillet meets the branch. Expected: f-cyl1, f-cyl2 and f-fil1 ok; after f-cyl2 volume 20599.513715, 5 faces; after f-fil1 volume 20616.732472, 6 faces, with a face `f-fil1/fillet/edge(f-cyl1/side|f-cyl2/side)`; f-fil2 status warning with exactly one warning, `ignored` with reason `smooth_edge` (one edge, so no `no_effect`, features.md section 3.15); b-1 identical to the state after f-fil1. The shape records the edge as `G1OrBetter`, while the outward normals of its faces at its middle are about 1.8e-6 rad apart, so normals alone within 1e-6 rad would keep it, and the kernel cannot fillet it.
17. **Radius below the tolerance.** `CUBE20`, `FIL([E-FT], "0.0000001")`. Expected: error `out_of_range`, field `radius`, rendered `Radius must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm.` (the value is written with the digits that tell it apart from the bound, features.md section 3.12); b-1 unchanged.
18. **Edge order does not matter.** `CUBE20`, `FIL([E-FT, E-BT, E-LT], "1")` and, in a second document, the same with `edges` listed as `[E-LT, E-BT, E-FT]`. Expected: both ok with equal BRep bytes (`Shape::to_brep`), the same volume to the last bit and the same element names.

Tests 19 to 22 use `STEP`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "20", "10"], "op": "new"}` (x -20..20, y -10..10, z 0..10) and `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["20", "30", "5"], "position": ["10", "0", "7.6"], "op": "cut"}` (a step 2.4 mm deep over x 0..20, whose riser `f-box2/left` lies at x = 0; volume 7040), and `E-RISER`: `{"q": "edge(f-box1/top|f-box2/left)", "fp": "line|n=0,1,0|c=0,0,10|l=20|adj=2,2"}`.

19. **The limiting face is named (newcomer error corpus NC05).** `STEP`, `FIL([E-RISER], "3")`. Expected: error `fillet_too_large`, `limit` the face `f-box2/left` across that edge with width 2.4, `removable` false (there is no other reference), rendered `Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm. The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across.`; fixes [Use 2.3 mm]. Then: f-fil1 status ok.
20. **Remove the edge that limits it, when the others work.** `STEP`, `FIL([E-RISER, {"q": "edge(f-box1/bottom|f-box1/left)", "fp": "line|n=0,1,0|c=-20,0,0|l=20|adj=2,2"}], "3")`. Expected: error `fillet_too_large` with the message of test 19 and `removable` true (one more attempt at 3 mm without `edges[0]` works); fixes [Use 2.3 mm] then `RemoveEntry { pointer: "/edges/0" }` (Remove from Edges). Then (`RemoveEntry`): f-fil1 status ok with only the bottom left edge rounded at 3 mm, b-1 volume 7001.371669. Test 8 is the counterpart: there every other edge fails as well, so no removal is offered, because a removal is offered only when it was verified to work. With `[E-RISER, E-RISER, {"q": "edge(f-box1/bottom|f-box1/left)", ...}]`: `removable` false and the only fix [Use 2.3 mm], since removing `edges[0]` would leave `edges[1]` selecting the riser's edge (features.md section 4.3).
21. **A limit far below the request counts as none.** `STEP`, `FIL([E-RISER], "30")`. Expected: error `fillet_too_large` with `max` None, although the search finds about 2.3 mm: that is below 10 percent of 30 mm, so it is not a useful fix (features.md section 4.3); `limit` the face `f-box2/left` with width 2.4; no suggestion; fixes `Suppress { feature: "f-fil1" }` only; the rendered message ends with `The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across.`
22. **A bare number in an inch document (corpus R48).** Document `"units": "in"`; `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20 mm", "20 mm", "20 mm"], "op": "new"}`; `FIL([E-FT], "2")` (2 in, 50.8 mm, on a 20 mm cube). Expected: error `fillet_too_large`, `max == Some(m)` with 12.7 ≤ m < 20 (millimeters), rendered `Fillet radius 2 in is too large for the selected edges: the largest that works is about {s} in. '2' has no unit, so it means 2 in in this document.` with `{s}` = m in inches rounded down to 2 significant digits (the bare-number sentence takes the place of the limit sentence); fixes `SetField` radius `"2 mm"` [Use 2 mm] first, then [Use {s} in]; suggestion `2 mm`. Then (the first fix): f-fil1 status ok, `radius` is `"2 mm"`, b-1 volume 7982.831853.

## Open issues

1. Test 8 bounds the suggested radius rather than fixing it, because the exact limit where the kernel stops succeeding (just under the 2.4 mm plate thickness) is a kernel property. The fixed bisection points make the value reproducible for a given kernel build.
2. Corner blend counts (test 3) assume the kernel produces one spherical face per box corner for three equal fillets, as OCCT does.
3. Resolved by kernel.md section 3.6 item 5. Step 4 (smooth edges, which chamfer.md step 3 reuses) once had to approximate the outward normals of an edge's faces from the kernel's surface description (kernel.md section 3.6 item 3), which covers planes (signed), cylinders and spheres (up to sign) at line and circle edges only: a smooth edge next to a cone, torus, surface of revolution or freeform face reached the kernel (the edge between a cylinder's side and the fillet of its rim failed with `FilletTooLarge` or `ChamferTooLarge`), and a knife edge between a flat face and a cylinder or sphere was skipped as smooth. The kernel query `Shape::edge_normals` returns the oriented outward normal of each face of an edge at the edge's middle parameter for every surface type, so step 4 uses it directly (kernel.md tests 95 to 101, test 97 being the edge next to a torus). The document test `smooth_edge_next_to_a_torus_skipped` in `crates/partsmith-recompute/tests/feature_fillet.rs` runs once the kernel and `dressup.rs` use the query.
4. **Smooth edges by recorded continuity (decided).** Step 4 first called two faces smooth only when their normals at the edge's middle were within 1e-6 rad. The surfaces OCCT's blends build are approximations, and where a fillet meets a curved face its normals can differ from that face's by more: on the pipe tee of test 16 the joint's fillet meets the branch 1.8e-6 rad apart, so a second fillet or a chamfer of that edge reached the kernel and failed (`FilletTooLarge`, `ChamferTooLarge`). The kernel now reports the continuity the shape records for an edge (kernel.md section 3.6 item 5.6): fillets record `G1` between a fillet face and each face it blends into, which step 4 trusts, and the normal angle widens to 1e-3 rad for smooth edges without a record, such as edges a boolean rebuilt, since such approximations stay far below it and designed corners far above it.
5. **The first sentence when a limit counts as none (needs human review).** Test 21 pins the limit sentence only. The first sentence of features.md section 4.3 for a `max` that counts as none, `Fillet radius 30 mm does not work on the selected edges, even when smaller.`, is not true when the search found a much smaller radius that works (about 2.3 mm there), which the error guidelines forbid; features.md open issue 14 proposes a wording.
