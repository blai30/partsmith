# Feature: split_body

Status: M1 spec. Type name `split_body`. Shared contracts: [features.md](../features.md).

## 1. Purpose and scope

Split body cuts a body in two with a plane. The body keeps the part in front of the plane (the side its normal points to). The part behind goes into another, empty body, or is discarded. Splitting parts that do not fit the print bed is the main use.

### In M1

- Split by an origin plane, a datum plane or a flat face.
- `flip` to keep the other side.
- Optional `target` body for the back part; without it the back part is discarded (a trim).

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

```python
split_body(
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
2. frame = `ctx.plane("plane", plane)`; the front side is where (p - origin) · normal > 0, or < 0 when `flip`.
3. Split the shape by the plane into the front part and the back part. If either part has no solid with volume above 1e-9 × the body's volume (the plane misses the body or only touches it), fail with `PlaneMissesBody`.
4. If `target` is set it must be empty, else `BodyNotEmpty { body: target }`.
5. Name the new section faces (section 3.4). Output the body's update (front part, change `Modify`) and, when `target` is set, the target's update (back part, change `New`).

Either part may consist of several solids (a U-shaped body split across its arms).

### 3.4 Element names

| Name | Face |
| --- | --- |
| `<id>/split` | The new planar section face in each part. Several section faces in one part are numbered `#n` by the naming crate. |

All other faces keep their names in whichever part they end up in; faces cut by the plane keep their name in both parts.

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Target is the body | `SameBody` | `Body 'Body 1' cannot be used twice here. Pick a different body for Target body.` |
| Body empty | `BodyEmpty` | `Body 'Body 1' is empty at this point, so this feature has nothing to work on. Pick a body that has a solid.` |
| Plane does not cross the body | `PlaneMissesBody` | `The split plane does not pass through body 'Body 1'. Move the plane so it cuts through the body.` |
| Target has a solid | `BodyNotEmpty` | features.md section 4.1 |
| Plane face not flat or lost | `NotPlanar`, `ReferenceNotFound` | features.md section 4.1 |
| Kernel failure | `Kernel { operation: Split }` | `The split failed in the geometry kernel (...). Try moving the plane slightly so it does not touch an edge.` |

## 5. Performance

- A box: 15 ms. A 200-face body: 60 ms.

## 6. Acceptance tests

Conventions from features.md section 6.1. Bodies `[b-1, b-2]`. Fixtures:

- `CUBE20`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"}` (spans [-10,-10,0]-[10,10,20]).
- `DP5`: `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "5"}`.
- `SPLIT(extra)`: `{"id": "f-split1", "type": "split_body", "body": "b-1", "plane": {"kind": "datum", "feature": "f-dp1"}, <extra>}`.

1. **Split into two bodies.** `CUBE20`, `DP5`, `SPLIT("target": "b-2")`. Expected: ok; b-1 volume 6000, 6 faces, bbox [-10,-10,5]-[10,10,20], with `f-split1/split` planar at z=5 and outward normal (0,0,-1); b-2 volume 2000, 6 faces, bbox [-10,-10,0]-[10,10,5], with `f-split1/split` planar at z=5 and outward normal (0,0,1); `f-box1/front` exists in both; changes `[b-1 Modify, b-2 New]`.
2. **Trim.** `CUBE20`, `DP5`, `SPLIT()`. Expected: b-1 volume 6000; b-2 empty; one update.
3. **Flip.** `CUBE20`, `DP5`, `SPLIT("flip": true, "target": "b-2")`. Expected: b-1 volume 2000, bbox [-10,-10,0]-[10,10,5]; b-2 volume 6000, bbox [-10,-10,5]-[10,10,20].
4. **Plane misses.** `CUBE20`, `DP5` with offset `"30"`, `SPLIT("target": "b-2")`. Expected: error `plane_misses_body` rendered `The split plane does not pass through body 'Body 1'. Move the plane so it cuts through the body.`; b-1 unchanged; b-2 empty.
5. **Plane on a face.** `CUBE20`, `DP5` with offset `"0"`, `SPLIT("target": "b-2")`. Expected: error `plane_misses_body`.
6. **Target with a solid.** `CUBE20`, `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["1", "1", "1"], "position": ["50", "0", "0"], "op": "new"}`, `DP5`, `SPLIT("target": "b-2")`. Expected: error `body_not_empty`; b-1 and b-2 unchanged.
7. **Target is the body.** `CUBE20`, `DP5`, `SPLIT("target": "b-1")`. Expected: error `same_body`, field `target`.
8. **By an origin plane.** `CUBE20`, `{"id": "f-split1", "type": "split_body", "body": "b-1", "plane": {"kind": "origin", "plane": "XZ"}, "target": "b-2"}`. XZ's normal is -Y, so the front side is y < 0. Expected: b-1 volume 4000, bbox [-10,-10,0]-[10,0,20]; b-2 volume 4000, bbox [-10,0,0]-[10,10,20].
9. **Parts with several solids.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["30", "10", "10"], "op": "new"}`, `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["10", "10", "8"], "position": ["0", "0", "2"], "op": "cut"}` (a U shape, volume 2200), `DP5`, `SPLIT("target": "b-2")`. Expected: b-1 volume 1000, 12 faces, two solids, faces `f-split1/split#1` and `f-split1/split#2`; b-2 volume 1200, 10 faces, one solid, two section faces named `f-split1/split#1` and `f-split1/split#2`.
10. **Python.** `Registry::python` of the JSON in section 2 renders exactly the Python in section 2.
11. **Performance.** Test 1 builds in under 15 ms (desktop, release).

## Open issues

None beyond features.md.
