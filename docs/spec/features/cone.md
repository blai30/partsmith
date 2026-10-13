# Feature: cone

Status: M1 spec. Type name `cone`. Shared contracts: [features.md](../features.md), primitive placement in features.md section 3.14. Naming rule: [naming.md](../naming.md) section 3.6.4.

## 1. Purpose and scope

Cone makes a right circular cone or frustum standing on a plane, with its axis along the plane normal and its base centered on the placement origin, and combines it with a body.

### In M1

- Bottom radius, top radius (either may be 0, not both) and height.
- Equal radii give a cylinder.
- Placement on an origin plane, a datum plane or a flat face, with an offset.
- Ops `new`, `join`, `cut`, `intersect`.

### Later

- Cone by half angle instead of a top radius.
- Diameter input mode.

## 2. Public API

```rust
/// A right circular cone or frustum.
pub struct Cone;

impl Feature for Cone {
    const TYPE: &'static str = "cone";
    type Params = ConeParams;
}

/// Params of a cone feature.
pub struct ConeParams {
    /// Target body.
    pub body: Id,
    /// Radius at z = 0. Length, at least 0.
    pub bottom_radius: Expr,
    /// Radius at z = height. Length, at least 0. Not both radii 0.
    pub top_radius: Expr,
    /// Height along the placement normal. Length, greater than 0.
    pub height: Expr,
    /// Placement plane. Default origin XY, omitted when default.
    pub plane: PlaneRef,
    /// Placement offset. Default ["0", "0", "0"], omitted when default.
    pub position: [Expr; 3],
    /// How the cone combines with the body. Required.
    pub op: BodyOp,
}
```

JSON:

```json
{"id": "f-cone1", "type": "cone", "body": "b-1", "bottom_radius": "10", "top_radius": "5", "height": "10", "op": "new"}
```

### Manifest

Label `Cone`, category `create`, icon `cone`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `bottom_radius` | Bottom radius | yes | length | | |
| `top_radius` | Top radius | yes | length | | |
| `height` | Height | yes | length | | |
| `op` | Operation | yes | | | `new` New body, `join` Join, `cut` Cut, `intersect` Intersect |
| `plane`, `position[0..2]` | features.md section 3.14 | no | | | |
| `body` | Body | no | | `body` | |

### Python

Dedicated form: `cone(bottom_radius, top_radius, height, plane=..., position=..., op=...)` (python-api.md section 2.7), produced by the `Feature::python` hook with the primitive rules of features.md section 3.14. The JSON above is `cone(10, 5, 10, id="f-cone1")`. Generic form (longer than 88 characters, so one argument per line):

```python
feature(
    "cone",
    id="f-cone1",
    body="b-1",
    bottom_radius="10",
    top_radius="5",
    height="10",
    op="new",
)
```

## 3. Behavior

### 3.1 Dependencies

- `bodies_written`: `[body]`; plus placement dependencies.
- `expressions`: `bottom_radius`, `top_radius`, `height` (Length), then `position[0..2]`.

### 3.2 Build

1. Checks in order: bottom_radius ≥ 0 and top_radius ≥ 0 (`OutOfRange`, `AtLeast(0)`); not both 0 (`ConeRadiiZero`); height at least `LINEAR_TOLERANCE` (`OutOfRange`, `GreaterThan(0)`, features.md section 3.14). From the second check on, a radius below `LINEAR_TOLERANCE` (1e-6 mm) counts as 0: it is an apex, because the kernel builds no circle smaller than its modeling tolerance (kernel.md section 3.4.1), and such a value displays as 0 mm anyway.
2. In the local frame the axis is local z; the circle of `bottom_radius` is at z = 0 and the circle of `top_radius` at z = height. A zero radius is a point (apex). With radii that differ by at least `LINEAR_TOLERANCE` the tool is kernel `make_cone`; with radii closer than that it is kernel `make_cylinder` of `bottom_radius`, because the kernel's cone needs radii at least `LINEAR_TOLERANCE` apart. With these rules every pair of radii that passes step 1 builds in the kernel.
3. Name the faces, then `apply_op(ctx, body, op, vec![tool], None)` (a primitive has no direction, features.md section 3.10).

### 3.3 Element names

| Name | Face (local frame) |
| --- | --- |
| `<id>/bottom` | Disc at z = 0 (absent when bottom_radius counts as 0) |
| `<id>/top` | Disc at z = height (absent when top_radius counts as 0) |
| `<id>/side` | Conical face (cylindrical when the radii are closer than `LINEAR_TOLERANCE`) |

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| A radius < 0 | `OutOfRange` | `Top radius must be at least 0 mm, but it is -1 mm. Use 0 mm or more.` |
| Both radii 0 (below 1e-6 mm) | `ConeRadiiZero` | `Cone bottom and top radius cannot both be 0. Give one of them a size.` |
| Height below 1e-6 mm (0 and negative included) | `OutOfRange` | `Height must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| A size above the modeling limit | `OutOfRange` | `Height must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less.` |
| Expression, placement, body and kernel errors | as in [box.md](box.md) section 4 | |

## 5. Performance

- Op new: 2 ms.

## 6. Acceptance tests

Conventions from features.md section 6.1. Bodies `[b-1]`. `CONE(r1, r2, h)`: `{"id": "f-cone1", "type": "cone", "body": "b-1", "bottom_radius": r1, "top_radius": r2, "height": h, "op": "new"}`.

1. **Frustum.** `CONE("10", "5", "10")`. Expected: ok; volume 1832.595715, 3 faces, bbox [-10,-10,0]-[10,10,10]; `f-cone1/side` conical with area 526.861105; `f-cone1/bottom` area 314.159265; `f-cone1/top` area 78.539816.
2. **Pointed.** `CONE("10", "0", "10")`. Expected: volume 1047.197551, 2 faces (`bottom`, `side`), bbox [-10,-10,0]-[10,10,10]; no face named `f-cone1/top`.
3. **Inverted.** `CONE("0", "5", "10")`. Expected: volume 261.799388, 2 faces (`top`, `side`), bbox [-5,-5,0]-[5,5,10].
4. **Equal radii.** `CONE("5", "5", "10")`. Expected: volume 785.398163, 3 faces; `f-cone1/side` cylindrical.
5. **Both zero.** `CONE("0", "0", "10")`. Expected: error `cone_radii_zero` rendered `Cone bottom and top radius cannot both be 0. Give one of them a size.`
6. **Negative radius.** `CONE("10", "-1", "10")`. Expected: error `out_of_range`, field `top_radius`, rendered `Top radius must be at least 0 mm, but it is -1 mm. Use 0 mm or more.`
7. **Zero height.** `CONE("10", "5", "0")`. Expected: error `out_of_range`, field `height`.
8. **Performance.** Test 1 builds in under 2 ms (desktop, release).
9. **Nearly equal radii.** `CONE("5", "5.0000005", "10")`. Expected: ok; volume 785.398163, 3 faces; `f-cone1/side` cylindrical.
10. **Apex below the tolerance.** `CONE("10", "0.0000005", "10")`. Expected: ok; volume 1047.197551, 2 faces (`bottom`, `side`).
11. **Both radii below the tolerance.** `CONE("0.0000000001", "0", "10")`. Expected: error `cone_radii_zero`.
12. **Height below the tolerance.** `CONE("10", "5", "0.0000001")`. Expected: error `out_of_range`, field `height`, rendered `Height must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm.` (the value is written with the digits that tell it apart from the bound, features.md section 3.12); b-1 empty.

## Open issues

None beyond features.md.
