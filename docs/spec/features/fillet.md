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

1. The body must hold a solid, else `BodyEmpty { use_new: false }`.
2. r = `radius`; r > 0 else `OutOfRange` (`GreaterThan(0)`).
3. Resolve `edges` with `ctx.resolve_all("edges", body, edges, Edge)`. Warnings go into the output; nothing resolved gives `ReferencesNotFound`.
4. Skip smooth edges. A resolved edge is smooth when the kernel's `Shape::topology()` lists fewer than two faces for it (a seam, or a degenerated edge such as a sphere pole), or when it has exactly two faces whose outward normals at the edge's middle, from kernel `Shape::edge_normals` (kernel.md section 3.6 item 5: the point at the middle of the edge's parameter range), make an angle of at most 1e-6 rad (the faces meet without a corner). Any other edge has a corner and is kept, including a knife edge (normals opposite) and an edge for which the query fails with `InvalidInput` because a face's normal is undefined at the edge's middle. Each skipped edge adds an `Ignored { reason: SmoothEdge }` warning. If no edge remains, the body is unchanged and a `NoEffect` warning is added.
5. Kernel `fillet` with every remaining edge at radius r. The kernel extends each contour along tangent chains.
6. If the kernel fails (any `KernelError` except `InvalidInput`), run the "too large" search (features.md section 4.3) and fail with `FilletTooLarge { radius: r, max, cause }`.
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
| Body empty | `BodyEmpty` | `Body 'Body 1' is empty at this point, so this feature has nothing to work on. Pick a body that has a solid.` |
| Radius ≤ 0 | `OutOfRange` | `Radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Expression or dimension error | `Expression` | `Radius: Expected a length, but this expression gives an angle.` |
| No edge resolves | `ReferencesNotFound` | `None of the 2 edges selected for Edges can be found anymore. Re-pick them in the repair panel.` |
| Kernel failure | `FilletTooLarge` | `Fillet radius 3 mm is too large for the selected edges. The largest radius that works is about 2.3 mm. Try 2.3 mm or less.` |
| Edge lost or ambiguous | warning `ReferenceMissing`, `ReferenceAmbiguous` | features.md section 4.2 |
| Smooth edge skipped | warning `Ignored { reason: SmoothEdge }` | `One of the edges selected for Edges is smooth (its faces meet without a corner), so it was skipped.` |
| Nothing left to fillet | warning `NoEffect` | `This feature does not change body 'Body 1'. Check its position, size and direction.` |

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
7. **Every edge lost.** `CUBE20`, `FIL([E-LOST], "2")`. Expected: error `references_not_found` rendered `The edge selected for Edges can no longer be found. Re-pick it in the repair panel.`; b-1 unchanged (volume 8000, 6 faces).
8. **Radius too large.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "2.4"], "op": "new"}`, `FIL([E-FT, E-BT, E-LT, E-RT], "3")`. Expected: error `fillet_too_large`, field `radius`, `radius == 3`, `max == Some(m)` with 2.0 ≤ m < 2.4 and m = 3k/1024 for an integer k (a bisection point); rendered `Fillet radius 3 mm is too large for the selected edges. The largest radius that works is about {s} mm. Try {s} mm or less.` where `{s}` is m rounded down to 2 significant digits; b-1 unchanged (volume 960). Rebuilding with radius `"{s}"` is ok.
9. **Zero radius.** `CUBE20`, `FIL([E-FT], "0")`. Expected: error `out_of_range` rendered `Radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.`
10. **No edges.** `FIL([], "2")`. Expected: error `empty_selection` rendered `Select at least one edge for Edges.`
11. **Tangent chain.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["30", "20", "10"], "op": "new"}`; `{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [<the four vertical edges: front|left, front|right, back|left, back|right>], "radius": "3"}`; `{"id": "f-fil2", "type": "fillet", "body": "b-1", "edges": [E-FT], "radius": "1"}`. Expected: both ok; after f-fil1: volume 5922.743339, 10 faces; after f-fil2: volume 5902.689635, 18 faces, bbox [-15,-10,0]-[15,10,10]; 8 faces named `f-fil2/fillet/...` (the whole top loop).
12. **Smooth edge skipped.** As 11, with f-fil2 edges `[{"q": "edge(f-box1/front|f-fil1/fillet/edge(f-box1/front|f-box1/left))", "fp": "line|n=0,0,1|c=-12,-10,5|l=10"}]`. Expected: f-fil2 status warning (`ignored` with reason `smooth_edge`, and `no_effect`); b-1 identical to the state after f-fil1.
13. **Empty body.** `FIL([E-FT], "2")` alone. Expected: error `body_empty` rendered `Body 'Body 1' is empty at this point, so this feature has nothing to work on. Pick a body that has a solid.`
14. **Edges of another body.** Bodies `[b-1, b-2]`; `CUBE20` in b-1; `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["10", "10", "10"], "position": ["50", "0", "0"], "op": "new"}`; `FIL([{"q": "edge(f-box2/front|f-box2/top)", "fp": "line|n=1,0,0|c=50,-5,10|l=10"}], "1")` on b-1. Expected: error `references_not_found`; b-1 and b-2 unchanged.
15. **Performance.** Test 3 builds in under 100 ms; test 8 including the search in under 1.2 s (desktop, release).

## Open issues

1. Test 8 bounds the suggested radius rather than fixing it, because the exact limit where the kernel stops succeeding (just under the 2.4 mm plate thickness) is a kernel property. The fixed bisection points make the value reproducible for a given kernel build.
2. Corner blend counts (test 3) assume the kernel produces one spherical face per box corner for three equal fillets, as OCCT does.
3. Resolved by kernel.md section 3.6 item 5. Step 4 (smooth edges, which chamfer.md step 3 reuses) once had to approximate the outward normals of an edge's faces from the kernel's surface description (kernel.md section 3.6 item 3), which covers planes (signed), cylinders and spheres (up to sign) at line and circle edges only: a smooth edge next to a cone, torus, surface of revolution or freeform face reached the kernel (the edge between a cylinder's side and the fillet of its rim failed with `FilletTooLarge` or `ChamferTooLarge`), and a knife edge between a flat face and a cylinder or sphere was skipped as smooth. The kernel query `Shape::edge_normals` returns the oriented outward normal of each face of an edge at the edge's middle parameter for every surface type, so step 4 uses it directly (kernel.md tests 95 to 101, test 97 being the edge next to a torus). The document test `smooth_edge_next_to_a_torus_skipped` in `crates/partsmith-recompute/tests/feature_fillet.rs` runs once the kernel and `dressup.rs` use the query.
