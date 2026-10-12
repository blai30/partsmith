# Feature: chamfer

Status: M1 spec. Type name `chamfer`. Shared contracts: [features.md](../features.md). Naming rule: [naming.md](../naming.md) section 3.6.6. Kernel operation: [kernel.md](../kernel.md) section 3.4.7.

## 1. Purpose and scope

Chamfer bevels edges of a body with a flat face. Convex edges lose material, concave edges gain material.

### In M1

- Equal distance on both faces of each edge.
- Two distances, with a deterministic rule for which face gets which distance and a `flip` to swap them.
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
    /// `expr` on the first face and `second_expr` on the second face of each edge (section 3.3).
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

Dedicated form: `chamfer(edges, distance)` (python-api.md section 2.7), produced by the `Feature::python` hook with the shared rules of features.md section 3.11: `edges` as references (one alone, several as a list), `distance` from `distance.expr` of an `equal` distance; the `with <part>:` block gives `body`. A `two_distances` chamfer has no dedicated form: the hook returns `None` and the generic form is used. Generic form of the JSON above:

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

1. The body must hold a solid, else `BodyEmpty { use_new: false }`.
2. Each distance must be greater than 0, else `OutOfRange` (`GreaterThan(0)`) on its field, checked in field order, by the positive length rule of features.md section 3.4: a distance below `LINEAR_TOLERANCE` (1e-6 mm) is out of range like 0, before any kernel work.
3. Resolve `edges` as in [fillet.md](fillet.md) step 3, and skip smooth edges as in fillet step 4 (`Ignored { reason: SmoothEdge }`, `NoEffect` if none remain).
4. Face assignment for two distances: for each edge, the first face is the adjacent face whose element name sorts first in byte order; `expr` is measured on it and `second_expr` on the other face (kernel `ChamferSize::TwoDistances { distance, other_distance, face }` with `face` the first face's index). `flip: true` swaps the two for every edge. The resolved edges are passed to the kernel sorted by their element names in byte order (for every distance kind), so the result never depends on the order of `edges`: the kernel takes the side of a whole tangent chain from the first of its edges it receives, which is the chain's edge whose name sorts first. Edges the kernel adds along a tangent chain follow that side.
5. Kernel `chamfer` with every edge. On kernel failure (except `InvalidInput`), run the "too large" search (features.md section 4.3; both distances scale together) and fail with `ChamferTooLarge { field, distance, max, cause }`, where `field` is `distance.expr` for an equal distance and `distance` for two distances, `distance` is the larger requested distance and `max` the larger working scaled distance. Only the equal-distance error carries a suggestion (features.md section 4.4).
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
| Body empty | `BodyEmpty` | `Body 'Body 1' is empty at this point, so this feature has nothing to work on. Pick a body that has a solid.` |
| Distance below 1e-6 mm (0 and negative included) | `OutOfRange` | `Distance must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Second distance below 1e-6 mm | `OutOfRange` | `Second distance must be greater than 0 mm, but it is -1 mm. Use a value greater than 0 mm.` |
| Expression or dimension error | `Expression` | `Distance: Expected a length, but this expression gives an angle.` |
| No edge resolves | `ReferencesNotFound` | `The edge selected for Edges can no longer be found. Re-pick it in the repair panel.` |
| Kernel failure | `ChamferTooLarge` | `Chamfer distance 3 mm is too large for the selected edges. The largest distance that works is about 2.3 mm. Try 2.3 mm or less.` |
| Warnings | `ReferenceMissing`, `ReferenceAmbiguous`, `Ignored { SmoothEdge }`, `NoEffect` | features.md section 4.2 |

## 5. Performance

- 1 edge of a box: 10 ms. 12 edges of a box: 100 ms.
- A failing chamfer including the search: 1.2 s.

## 6. Acceptance tests

Conventions from features.md section 6.1; fixtures `CUBE20`, `E-FT`, `E-BT`, `E-LT`, `E-RT`, `E-LOST` and the edge naming from [fillet.md](fillet.md) section 6. `CHA(edges, distance)`: `{"id": "f-cha1", "type": "chamfer", "body": "b-1", "edges": <edges>, "distance": <distance>}`. Bodies `[b-1]`.

1. **Equal distance.** `CUBE20`, `CHA([E-FT], {"kind": "equal", "expr": "2"})`. Expected: ok; volume 7960, 7 faces, bbox [-10,-10,0]-[10,10,20]; face `f-cha1/chamfer/edge(f-box1/front|f-box1/top)` planar with area 56.568542; `f-box1/top` area 360; `f-box1/front` area 360; change `Modify`.
2. **Two distances.** `CUBE20`, `CHA([E-FT], {"kind": "two_distances", "expr": "1", "second_expr": "3"})`. `f-box1/front` sorts before `f-box1/top`, so 1 mm is measured on the front face. Expected: volume 7970, 7 faces; `f-box1/front` area 380; `f-box1/top` area 340; chamfer face area 63.245553.
3. **Two distances flipped.** As 2 with `"flip": true`. Expected: volume 7970; `f-box1/front` area 340; `f-box1/top` area 380.
4. **Top loop.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "10"], "op": "new"}`, `CHA([E-FT, E-BT, E-LT, E-RT], {"kind": "equal", "expr": "2"})`. Expected: volume 3850.666667, 10 faces, bbox [-10,-10,0]-[10,10,10]; `f-box1/top` area 256.
5. **Concave edge adds material.** The L-shaped body of [fillet.md](fillet.md) test 4, `CHA([{"q": "edge(f-box1/top|f-box2/left)", "fp": "line|n=0,1,0|c=0,0,10|l=20"}], {"kind": "equal", "expr": "2"})`. Expected: volume 12040, 9 faces.
6. **Too large.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "2.4"], "op": "new"}`, `CHA([E-FT, E-BT, E-LT, E-RT], {"kind": "equal", "expr": "3"})`. Expected: error `chamfer_too_large`, field `distance.expr`, `distance == 3`, `max == Some(m)` with 2.0 ≤ m < 2.4; rendered `Chamfer distance 3 mm is too large for the selected edges. The largest distance that works is about {s} mm. Try {s} mm or less.` with `{s}` = m rounded down to 2 significant digits; b-1 unchanged (volume 960). Rebuilding with `"{s}"` is ok.
7. **Zero distance.** `CUBE20`, `CHA([E-FT], {"kind": "equal", "expr": "0"})`. Expected: error `out_of_range`, field `distance.expr`.
8. **Negative second distance.** `CUBE20`, `CHA([E-FT], {"kind": "two_distances", "expr": "1", "second_expr": "-1"})`. Expected: error `out_of_range`, field `distance.second_expr`, rendered `Second distance must be greater than 0 mm, but it is -1 mm. Use a value greater than 0 mm.`
9. **No edges.** `CHA([], {"kind": "equal", "expr": "1"})`. Expected: error `empty_selection`.
10. **Edge lost.** `CUBE20`, `CHA([E-FT, E-LOST], {"kind": "equal", "expr": "2"})`. Expected: warning `reference_missing` with pointer `/edges/1`; volume 7960.
11. **Performance.** Test 4 builds in under 60 ms (desktop, release).
12. **Edge order does not matter.** The body after `f-fil1` of fillet.md test 11, `CHA([edge(f-box1/front|f-box1/top), edge(f-box1/top|f-fil1/fillet/edge(f-box1/front|f-box1/left))], {"kind": "two_distances", "expr": "1", "second_expr": "2"})` and the same chamfer with the two edges listed in the other order give the same volume (within 1e-9 relative) and the same face names.
13. **Chamfer beside a fillet on a pipe tee.** The features up to `f-fil1` of [fillet.md](fillet.md) test 16, then `CHA([{"q": "edge(f-cyl2/side|f-fil1/fillet/edge(f-cyl1/side|f-cyl2/side))", "fp": "bspline_curve|c=0,0,10.524948|l=40.038448"}], {"kind": "equal", "expr": "1"})`. Expected: f-cha1 status warning (`ignored` with reason `smooth_edge`, and `no_effect`); b-1 identical to the state after f-fil1.
14. **Distance below the tolerance.** `CUBE20`, `CHA([E-FT], {"kind": "two_distances", "expr": "1", "second_expr": "0.0000001"})`. Expected: error `out_of_range`, field `distance.second_expr`, rendered `Second distance must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.`; b-1 unchanged.

## Open issues

1. Assigning the first distance to the face whose name sorts first is deterministic but not visual. The inspector should preview which face gets which distance and offer Flip; a later version may add a reference face per edge set.
2. **Resolved:** selected edges of one tangent chain can disagree on their first face (on a box whose vertical edges are filleted, `edge(f-box1/front|f-box1/top)` has the wall first and `edge(f-box1/top|f-fil1/fillet/edge(f-box1/front|f-box1/left))` the top first), and the kernel takes a chain's side from the first edge it receives. Step 4 now sorts the edges by name before the kernel call, so the chain's edge whose name sorts first sets the side and listing order no longer changes the geometry. A reference face per edge set (open issue 1) would make the side explicit.
