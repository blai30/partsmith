# Feature: split_body

Status: M1 spec. Type name `split_body`. Shared contracts: [features.md](../features.md). Naming rule: [naming.md](../naming.md) section 3.6.13. Kernel operation: [kernel.md](../kernel.md) section 3.4.11.

## 1. Purpose and scope

Split body cuts a body in two with a plane. The body keeps the part in front of the plane (the side its normal points to). The part behind goes into another, empty body, or is discarded. Splitting parts that do not fit the print bed is the main use.

### In M1

- Split by an origin plane, a datum plane or a flat face.
- `flip` to keep the other side.
- Optional `target` body for the other part; without it the other part is discarded (a trim).

### Later

- Split by a face of another body, by a sketch, or by a surface.
- Split with connectors (dowel pins, dovetails, alignment keys).
- Split face.
- Keeping both parts in the same body as separate solids.

## 2. Public API

```rust
/// Cuts a body in two with a plane.
pub struct SplitBody;

impl Feature for SplitBody {
    const TYPE: &'static str = "split_body";
    type Params = SplitBodyParams;
}

/// Params of a split body feature.
pub struct SplitBodyParams {
    /// The body to split. It keeps the front part.
    pub body: Id,
    /// The split plane.
    pub plane: PlaneRef,
    /// Keep the back part instead of the front part. Default false, omitted when false.
    pub flip: bool,
    /// The empty body that receives the other part. Omitted (None) discards it.
    pub target: Option<Id>,
}
```

JSON:

```json
{"id": "f-split1", "type": "split_body", "body": "b-1", "plane": {"kind": "datum", "feature": "f-dp1"}, "target": "b-2"}
```

### Manifest

Label `Split body`, category `modify`, icon `split-body`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `body` | Body | yes | | `body` | |
| `plane` | Split plane | yes | | `plane` | |
| `target` | Target body | yes | | `body` | |
| `flip` | Flip | yes | | | |

### Python

Dedicated form: `split(plane, into=<target>, part=<body>)` (python-api.md section 2.7). Generic form of the JSON above (longer than 100 characters, so one argument per line):

```python
feature(
    "split_body",
    id="f-split1",
    body="b-1",
    plane={"kind": "datum", "feature": "f-dp1"},
    target="b-2",
)
```

## 3. Behavior

### 3.1 Validation

- `target` equal to `body`: `SameBody { field: "target" }`.

### 3.2 Dependencies

- `bodies_written`: `[body]`, then `target` when set. Plane dependencies as for mirror.
- `expressions`: none.

### 3.3 Build

1. The body must hold a solid, else `BodyEmpty { use_new: false }`.
2. placement = `ctx.plane("plane", plane)`. The positive side is where (p - origin) · normal > 0. The body keeps the positive side, or the negative side when `flip` is true; the other side goes to `target`.
3. Kernel `split(shape, plane)`. If either side has no solid with volume above 1e-9 × the body's volume (the plane misses the body or only touches it), fail with `PlaneMissesBody`.
4. If `target` is set it must be empty, else `BodyNotEmpty { body: target }`.
5. Build one naming map over the whole split result (section 3.4), then give each body the projection of that map onto its solids (naming.md section 3.5.2). Output the body's update (change `Modify`) and, when `target` is set, the target's update (change `New`).

Either part may consist of several solids (a U-shaped body split across its arms).

### 3.4 Element names

| Name | Face |
| --- | --- |
| `<id>/split-positive` | A section face bounding a piece on the positive side of the plane (its outward normal is opposite the plane normal). |
| `<id>/split-negative` | A section face bounding a piece on the negative side. |

The names depend on the plane's side, not on `flip`. Several section faces of one side are numbered `#n`, and faces cut by the plane get `#n` pieces numbered together across both parts, both in naming.md's geometric order (centroid x, then y, then z). Other faces keep their names.

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Target is the body | `SameBody` | `Body 'Body 1' cannot be used twice here. Pick a different body for Target body.` |
| Body empty | `BodyEmpty` | `Body 'Body 1' is empty at this point, so this feature has nothing to work on. Pick a body that has a solid.` |
| Plane does not cross the body | `PlaneMissesBody` | `The split plane does not pass through body 'Body 1'. Move the plane so it cuts through the body.` |
| Target has a solid | `BodyNotEmpty` | features.md section 4.1 |
| Plane face not flat or lost | `NotPlanar`, `ReferenceNotFound` | features.md section 4.1 |
| Kernel failure | `Kernel` | `Splitting the body failed. Try moving the split plane slightly.` |

## 5. Performance

- A box: 15 ms. A 200-face body: 60 ms.

## 6. Acceptance tests

Conventions from features.md section 6.1. Bodies `[b-1, b-2]`. Fixtures:

- `CUBE20`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"}` (spans [-10,-10,0]-[10,10,20]).
- `DP5`: `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "5"}`.
- `SPLIT(extra)`: `{"id": "f-split1", "type": "split_body", "body": "b-1", "plane": {"kind": "datum", "feature": "f-dp1"}, <extra>}`.

1. **Split into two bodies.** `CUBE20`, `DP5`, `SPLIT("target": "b-2")`. Expected: ok; b-1 volume 6000, 6 faces, bbox [-10,-10,5]-[10,10,20], with `f-split1/split-positive` planar at z=5 and outward normal (0,0,-1), and the upper front piece named `f-box1/front#2`; b-2 volume 2000, 6 faces, bbox [-10,-10,0]-[10,10,5], with `f-split1/split-negative` planar at z=5 and outward normal (0,0,1), and the lower front piece named `f-box1/front#1`; `f-box1/top` in b-1 and `f-box1/bottom` in b-2 keep their names; changes `[b-1 Modify, b-2 New]`.
2. **Trim.** `CUBE20`, `DP5`, `SPLIT()`. Expected: b-1 volume 6000; b-2 empty; one update.
3. **Flip.** `CUBE20`, `DP5`, `SPLIT("flip": true, "target": "b-2")`. Expected: b-1 volume 2000, bbox [-10,-10,0]-[10,10,5], with `f-split1/split-negative`; b-2 volume 6000, bbox [-10,-10,5]-[10,10,20], with `f-split1/split-positive`.
4. **Plane misses.** `CUBE20`, `DP5` with offset `"30"`, `SPLIT("target": "b-2")`. Expected: error `plane_misses_body` rendered `The split plane does not pass through body 'Body 1'. Move the plane so it cuts through the body.`; b-1 unchanged; b-2 empty.
5. **Plane on a face.** `CUBE20`, `DP5` with offset `"0"`, `SPLIT("target": "b-2")`. Expected: error `plane_misses_body`.
6. **Target with a solid.** `CUBE20`, `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["1", "1", "1"], "position": ["50", "0", "0"], "op": "new"}`, `DP5`, `SPLIT("target": "b-2")`. Expected: error `body_not_empty`; b-1 and b-2 unchanged.
7. **Target is the body.** `CUBE20`, `DP5`, `SPLIT("target": "b-1")`. Expected: error `same_body`, field `target`.
8. **By an origin plane.** `CUBE20`, `{"id": "f-split1", "type": "split_body", "body": "b-1", "plane": {"kind": "origin", "plane": "XZ"}, "target": "b-2"}`. XZ's normal is -Y, so the positive side is y < 0. Expected: b-1 volume 4000, bbox [-10,-10,0]-[10,0,20]; b-2 volume 4000, bbox [-10,0,0]-[10,10,20].
9. **Parts with several solids.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["30", "10", "10"], "op": "new"}`, `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["10", "10", "8"], "position": ["0", "0", "2"], "op": "cut"}` (a U shape, volume 2200), `DP5`, `SPLIT("target": "b-2")`. Expected: b-1 volume 1000, 12 faces, 2 solids, section faces `f-split1/split-positive#1` (centroid x=-10) and `f-split1/split-positive#2` (centroid x=10); b-2 volume 1200, 10 faces, 1 solid, section faces `f-split1/split-negative#1` and `f-split1/split-negative#2`.
10. **Performance.** Test 1 builds in under 15 ms (desktop, release).

## Open issues

None beyond features.md.
