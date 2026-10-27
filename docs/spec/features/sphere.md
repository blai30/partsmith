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

1. radius at least `LINEAR_TOLERANCE` (1e-6 mm), else `OutOfRange` (`GreaterThan(0)`) (features.md section 3.14).
2. Kernel `make_sphere` centered on the local origin, poles on the local z-axis.
3. Name the face, then `apply_op(ctx, body, op, vec![tool], None)` (a primitive has no direction, features.md section 3.10).

### 3.3 Element names

| Name | Face |
| --- | --- |
| `<id>/surface` | The spherical face |

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Radius below 1e-6 mm (0 and negative included) | `OutOfRange` | `Radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Radius above the modeling limit | `OutOfRange` | `Radius must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less.` |
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
7. **Radius below the tolerance.** As 1 with `"radius": "0.0000001"`. Expected: error `out_of_range`, field `radius`, rendered `Radius must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm.` (the value is written with the digits that tell it apart from the bound, features.md section 3.12); b-1 empty.
8. **Intersect.** `CUBE20`, `{"id": "f-sph1", "type": "sphere", "body": "b-1", "radius": "8", "position": ["0", "0", "20"], "op": "intersect"}` (centered on the top face), as `examples/sphere/intersect`. Expected: ok; volume 1072.330292 (the lower half of the sphere), 2 faces, bbox [-8,-8,12]-[8,8,20]; change `Intersect`; `f-sph1/surface` spherical with radius 8 and area 402.123860; `f-box1/top` planar at z=20 with area 201.061930.
9. **On a datum.** `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "10"}`, test 1's sphere with `"plane": {"kind": "datum", "feature": "f-dp1"}`, as `examples/sphere/on-datum`. Expected: ok; as test 2, volume 4188.790205 and bbox [-10,-10,0]-[10,10,20] (centered on the datum's origin (0,0,10)); dependencies `features == ["f-dp1"]`.
10. **On a face of another body.** Bodies `[b-1, b-2]`; `CUBE20`; `{"id": "f-sph1", "type": "sphere", "body": "b-2", "radius": "5", "plane": {"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,20|a=400"}}, "position": ["0", "0", "5"], "op": "new"}`, as `examples/sphere/on-face`. Expected: ok; b-2 volume 523.598776, 1 face, bbox [-5,-5,20]-[5,5,30] (resting on the top of the cube); b-1 unchanged; f-sph1 dependencies `bodies_read == ["b-1"]`, `bodies_written == ["b-2"]`.

## Open issues

None beyond features.md.
