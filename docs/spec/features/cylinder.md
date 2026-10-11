# Feature: cylinder

Status: M1 spec. Type name `cylinder`. Shared contracts: [features.md](../features.md), primitive placement in features.md section 3.14. Naming rule: [naming.md](../naming.md) section 3.6.4.

## 1. Purpose and scope

Cylinder makes a right circular cylinder standing on a plane, with its axis along the plane normal and its base centered on the placement origin, and combines it with a body.

### In M1

- Radius and height.
- Placement on an origin plane, a datum plane or a flat face, with an offset.
- Ops `new`, `join`, `cut`, `intersect`.

### Later

- Diameter input mode.
- Pipe (hollow cylinder) primitive.
- Centered-height placement.

## 2. Public API

```rust
/// A right circular cylinder.
pub struct Cylinder;

impl Feature for Cylinder {
    const TYPE: &'static str = "cylinder";
    type Params = CylinderParams;
}

/// Params of a cylinder feature.
pub struct CylinderParams {
    /// Target body.
    pub body: Id,
    /// Radius. Length, greater than 0.
    pub radius: Expr,
    /// Height along the placement normal. Length, greater than 0.
    pub height: Expr,
    /// Placement plane. Default origin XY, omitted when default.
    pub plane: PlaneRef,
    /// Placement offset. Default ["0", "0", "0"], omitted when default.
    pub position: [Expr; 3],
    /// How the cylinder combines with the body. Required.
    pub op: BodyOp,
}
```

JSON:

```json
{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "5", "height": "10", "op": "new"}
```

### Manifest

Label `Cylinder`, category `create`, icon `cylinder`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `radius` | Radius | yes | length | | |
| `height` | Height | yes | length | | |
| `op` | Operation | yes | | | `new` New body, `join` Join, `cut` Cut, `intersect` Intersect |
| `plane`, `position[0..2]` | features.md section 3.14 | no | | | |
| `body` | Body | no | | `body` | |

### Python

Dedicated form: `cylinder(radius, height, at=..., part=..., op=...)` (python-api.md section 2.7). Generic form: `feature("cylinder", id="f-cyl1", body="b-1", radius="5", height="10", op="new")`.

## 3. Behavior

### 3.1 Dependencies

- `bodies_written`: `[body]`; plus placement dependencies.
- `expressions`: `radius`, `height` (Length), then `position[0..2]`.

### 3.2 Build

1. radius > 0 and height > 0, else `OutOfRange` (`GreaterThan(0)`) on that field, in field order.
2. Kernel `make_cylinder` in the local frame: axis on local z, base circle of `radius` at z = 0, top at z = `height`.
3. Name the faces, then `apply_op`.

### 3.3 Element names

| Name | Face (local frame) |
| --- | --- |
| `<id>/bottom` | Disc at z = 0 |
| `<id>/top` | Disc at z = height |
| `<id>/side` | Cylindrical face |

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Radius ≤ 0 | `OutOfRange` | `Radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Height ≤ 0 | `OutOfRange` | `Height must be greater than 0 mm, but it is -2 mm. Use a value greater than 0 mm.` |
| Expression, placement, body and kernel errors | as in [box.md](box.md) section 4 | |

## 5. Performance

- Op new: 2 ms.

## 6. Acceptance tests

Conventions from features.md section 6.1. Bodies `[b-1]`. `CYL`: `{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "5", "height": "10", "op": "new"}`.

1. **Default.** `CYL`. Expected: ok; volume 785.398163, 3 faces, bbox [-5,-5,0]-[5,5,10]; `f-cyl1/side` cylindrical, radius 5, axis (0,0,1) through the origin, area 314.159265; `f-cyl1/top` area 78.539816 at z=10; `f-cyl1/bottom` area 78.539816 at z=0.
2. **On YZ.** `CYL` with `"plane": {"kind": "origin", "plane": "YZ"}`. Expected: bbox [0,-5,-5]-[10,5,5]; `f-cyl1/top` at x=10.
3. **Offset.** `CYL` with `"position": ["10", "0", "-5"]`. Expected: bbox [5,-5,-5]-[15,5,5].
4. **Zero radius.** `CYL` with `"radius": "0"`. Expected: error `out_of_range`, field `radius`.
5. **Negative height.** `CYL` with `"height": "-2"`. Expected: error `out_of_range`, field `height`, rendered `Height must be greater than 0 mm, but it is -2 mm. Use a value greater than 0 mm.`
6. **Cut a round hole.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "position": ["0", "0", "-10"], "op": "new"}`, `{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "3", "height": "10", "position": ["0", "0", "-10"], "op": "cut"}`. Expected: volume 15717.256661, 7 faces, bbox [-20,-20,-10]-[20,20,0]; `f-cyl1/side` exists in b-1.
7. **Performance.** Test 1 builds in under 2 ms (desktop, release).

## Open issues

1. Radius rather than diameter matches build123d, python-api.md and the fillet feature; makers often think in diameters, so the inspector may later offer a diameter input that writes `radius` as `d / 2`.
