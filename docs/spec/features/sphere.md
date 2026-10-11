# Feature: sphere

Status: M1 spec. Type name `sphere`. Shared contracts: [features.md](../features.md), primitive placement in features.md section 3.14. Naming rule: [naming.md](../naming.md) section 3.6.4.

## 1. Purpose and scope

Sphere makes a sphere centered on the placement origin and combines it with a body.

### In M1

- Radius.
- Placement on an origin plane, a datum plane or a flat face, with an offset.
- Ops `new`, `join`, `cut`, `intersect`.

### Later

- Partial spheres (angle limits) and hemispheres as one feature.
- Diameter input mode.

## 2. Public API

```rust
/// A sphere.
pub struct Sphere;

impl Feature for Sphere {
    const TYPE: &'static str = "sphere";
    type Params = SphereParams;
}

/// Params of a sphere feature.
pub struct SphereParams {
    /// Target body.
    pub body: Id,
    /// Radius. Length, greater than 0.
    pub radius: Expr,
    /// Placement plane. Default origin XY, omitted when default.
    pub plane: PlaneRef,
    /// Placement offset (the center). Default ["0", "0", "0"], omitted when default.
    pub position: [Expr; 3],
    /// How the sphere combines with the body. Required.
    pub op: BodyOp,
}
```

JSON:

```json
{"id": "f-sph1", "type": "sphere", "body": "b-1", "radius": "10", "position": ["0", "0", "10"], "op": "new"}
```

### Manifest

Label `Sphere`, category `create`, icon `sphere`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `radius` | Radius | yes | length | | |
| `op` | Operation | yes | | | `new` New body, `join` Join, `cut` Cut, `intersect` Intersect |
| `plane`, `position[0..2]` | features.md section 3.14 | no | | | |
| `body` | Body | no | | `body` | |

### Python

Dedicated form: `sphere(radius, plane=..., position=..., op=...)` (python-api.md section 2.7), produced by the `Feature::python` hook with the primitive rules of features.md section 3.14. The JSON above is `sphere(10, position=(0, 0, 10), id="f-sph1")`. Generic form (longer than 88 characters, so one argument per line):

```python
feature(
    "sphere",
    id="f-sph1",
    body="b-1",
    radius="10",
    position=["0", "0", "10"],
    op="new",
)
```

## 3. Behavior

### 3.1 Dependencies

- `bodies_written`: `[body]`; plus placement dependencies.
- `expressions`: `radius` (Length), then `position[0..2]`.

### 3.2 Build

1. radius > 0, else `OutOfRange` (`GreaterThan(0)`).
2. Kernel `make_sphere` centered on the local origin, poles on the local z-axis.
3. Name the face, then `apply_op(ctx, body, op, vec![tool])`.

### 3.3 Element names

| Name | Face |
| --- | --- |
| `<id>/surface` | The spherical face |

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Radius ≤ 0 | `OutOfRange` | `Radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Expression, placement, body and kernel errors | as in [box.md](box.md) section 4 | |

## 5. Performance

- Op new: 2 ms.

## 6. Acceptance tests

Conventions from features.md section 6.1. Bodies `[b-1]`. `CUBE20`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"}`.

1. **Default.** `{"id": "f-sph1", "type": "sphere", "body": "b-1", "radius": "10", "op": "new"}`. Expected: ok; volume 4188.790205, 1 face, bbox [-10,-10,-10]-[10,10,10]; `f-sph1/surface` spherical with area 1256.637061.
2. **Resting on XY.** As 1 with `"position": ["0", "0", "10"]`. Expected: bbox [-10,-10,0]-[10,10,20].
3. **Dome on a box.** `CUBE20`, `{"id": "f-sph1", "type": "sphere", "body": "b-1", "radius": "5", "position": ["0", "0", "20"], "op": "join"}`. Expected: volume 8261.799388, 7 faces, bbox [-10,-10,0]-[10,10,25].
4. **Dimple in a box.** As 3 with op cut. Expected: volume 7738.200612, 7 faces, bbox [-10,-10,0]-[10,10,20].
5. **Zero radius.** As 1 with `"radius": "0"`. Expected: error `out_of_range`, field `radius`.
6. **Performance.** Test 1 builds in under 2 ms (desktop, release).

## Open issues

None beyond features.md.
