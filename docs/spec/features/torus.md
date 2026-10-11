# Feature: torus

Status: M1 spec. Type name `torus`. Shared contracts: [features.md](../features.md), primitive placement in features.md section 3.14.

## 1. Purpose and scope

Torus makes a ring torus centered on the placement origin, with its axis along the plane normal, and combines it with a body.

### In M1

- Ring radius (axis to tube center) and tube radius, with the tube smaller than the ring (no self-intersecting spindle or horn tori).
- Placement on an origin plane, a datum plane or a flat face, with an offset.
- Ops `new`, `join`, `cut`, `intersect`.

### Later

- Partial tori (sweep angle).
- Inner and outer diameter input mode (Fusion style).

## 2. Public API

```rust
/// A ring torus.
pub struct Torus;

impl Feature for Torus {
    const TYPE: &'static str = "torus";
    type Params = TorusParams;
}

/// Params of a torus feature.
pub struct TorusParams {
    /// Target body.
    pub body: Id,
    /// Distance from the axis to the tube center. Length, greater than `minor_radius`.
    pub major_radius: Expr,
    /// Tube radius. Length, greater than 0.
    pub minor_radius: Expr,
    /// Placement plane. Default origin XY, omitted when default.
    pub plane: PlaneRef,
    /// Placement offset (the center). Default ["0", "0", "0"], omitted when default.
    pub position: [Expr; 3],
    /// How the torus combines with the body. Required.
    pub op: BodyOp,
}
```

JSON:

```json
{"id": "f-tor1", "type": "torus", "body": "b-1", "major_radius": "20", "minor_radius": "5", "op": "new"}
```

### Manifest

Label `Torus`, category `create`, icon `torus`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `major_radius` | Ring radius | yes | length | | |
| `minor_radius` | Tube radius | yes | length | | |
| `op` | Operation | yes | | | `new` New body, `join` Join, `cut` Cut, `intersect` Intersect |
| `plane`, `position[0..2]` | features.md section 3.14 | no | | | |
| `body` | Body | no | | `body` | |

### Python

`torus(id="f-tor1", body="b-1", major_radius=20, minor_radius=5, op="new")`.

## 3. Behavior

### 3.1 Dependencies

- `bodies_written`: `[body]`; plus placement dependencies.
- `expressions`: `major_radius`, `minor_radius` (Length), then `position[0..2]`.

### 3.2 Build

1. Checks in order: major_radius > 0 (`GreaterThan(0)`); minor_radius > 0 (`GreaterThan(0)`); minor_radius < major_radius (`OutOfRange` on `minor_radius`, `LessThan(major)`, bound name `the ring radius`).
2. The torus is centered on the local origin with its axis on local z; it spans z in [-minor, minor].
3. Name the face, then `apply_op`.

### 3.3 Element names

| Name | Face |
| --- | --- |
| `<id>/surface` | The toroidal face |

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Ring radius ≤ 0 | `OutOfRange` | `Ring radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Tube radius ≤ 0 | `OutOfRange` | `Tube radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Tube not smaller than ring | `OutOfRange` | `Tube radius must be less than the ring radius (20 mm), but it is 20 mm. Use a value less than 20 mm.` |
| Placement, body and kernel errors | as in [box.md](box.md) section 4 | |

## 5. Performance

- Op new: 2 ms.

## 6. Acceptance tests

Conventions from features.md section 6.1. Bodies `[b-1]`. `TOR`: `{"id": "f-tor1", "type": "torus", "body": "b-1", "major_radius": "20", "minor_radius": "5", "op": "new"}`.

1. **Default.** `TOR`. Expected: ok; volume 9869.604401, 1 face, bbox [-25,-25,-5]-[25,25,5]; `f-tor1/surface` toroidal with area 3947.841760.
2. **Resting on XY.** `TOR` with `"position": ["0", "0", "5"]`. Expected: bbox [-25,-25,0]-[25,25,10].
3. **On XZ.** `TOR` with `"plane": {"kind": "origin", "plane": "XZ"}`. Expected: bbox [-25,-5,-25]-[25,5,25] (axis along Y).
4. **Tube as large as the ring.** `TOR` with `"minor_radius": "20"`. Expected: error `out_of_range`, field `minor_radius`, rendered `Tube radius must be less than the ring radius (20 mm), but it is 20 mm. Use a value less than 20 mm.`
5. **Zero tube.** `TOR` with `"minor_radius": "0"`. Expected: error `out_of_range`, field `minor_radius`, bound `GreaterThan(0)`.
6. **Python.** `Registry::python` of `TOR` renders `torus(id="f-tor1", body="b-1", major_radius=20, minor_radius=5, op="new")`.
7. **Performance.** Test 1 builds in under 2 ms (desktop, release).

## Open issues

None beyond features.md.
