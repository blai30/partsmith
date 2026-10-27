# Feature: torus

Status: M1 spec. Type name `torus`. Shared contracts: [features.md](../features.md), primitive placement in features.md section 3.14. Naming rule: [naming.md](../naming.md) section 3.6.4.

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

Dedicated form: `torus(major_radius, minor_radius, plane=..., position=..., op=...)` (python-api.md section 2.7), produced by the `Feature::python` hook with the primitive rules of features.md section 3.14. The JSON above is `torus(20, 5, id="f-tor1")`. Generic form (exactly 88 characters, so one line): `feature("torus", id="f-tor1", body="b-1", major_radius="20", minor_radius="5", op="new")`.

## 3. Behavior

### 3.1 Dependencies

- `bodies_written`: `[body]`; plus placement dependencies.
- `expressions`: `major_radius`, `minor_radius` (Length), then `position[0..2]`.

### 3.2 Build

1. Checks in order: major_radius at least `LINEAR_TOLERANCE` (`GreaterThan(0)`); minor_radius at least `LINEAR_TOLERANCE` (`GreaterThan(0)`), both as features.md section 3.14 says for sizes; minor_radius at least `LINEAR_TOLERANCE` less than major_radius (major_radius - minor_radius ≥ `LINEAR_TOLERANCE`, the positive length rule of features.md section 3.4, because the kernel needs that much of a hole in the middle), else `OutOfRange` on `minor_radius`, `LessThan(major)`, bound name `the ring radius`.
2. Kernel `make_torus` centered on the local origin with its axis on local z; it spans z in [-minor, minor].
3. Name the face, then `apply_op(ctx, body, op, vec![tool], None)` (a primitive has no direction, features.md section 3.10).

### 3.3 Element names

| Name | Face |
| --- | --- |
| `<id>/surface` | The toroidal face |

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Ring radius below 1e-6 mm (0 and negative included) | `OutOfRange` | `Ring radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Tube radius below 1e-6 mm (0 and negative included) | `OutOfRange` | `Tube radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Tube not at least 1e-6 mm smaller than ring | `OutOfRange` | `Tube radius must be less than the ring radius (20 mm), but it is 20 mm. Use a value less than 20 mm.` |
| A radius above the modeling limit | `OutOfRange` | `Ring radius must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less.` |
| Expression, placement, body and kernel errors | as in [box.md](box.md) section 4 | |

## 5. Performance

- Op new: 2 ms.

## 6. Acceptance tests

Conventions from features.md section 6.1. Bodies `[b-1]`. `TOR`: `{"id": "f-tor1", "type": "torus", "body": "b-1", "major_radius": "20", "minor_radius": "5", "op": "new"}`.

1. **Default.** `TOR`. Expected: ok; volume 9869.604401, 1 face, bbox [-25,-25,-5]-[25,25,5]; `f-tor1/surface` toroidal with area 3947.841760.
2. **Resting on XY.** `TOR` with `"position": ["0", "0", "5"]`. Expected: bbox [-25,-25,0]-[25,25,10].
3. **On XZ.** `TOR` with `"plane": {"kind": "origin", "plane": "XZ"}`. Expected: bbox [-25,-5,-25]-[25,5,25] (axis along Y).
4. **Tube as large as the ring.** `TOR` with `"minor_radius": "20"`. Expected: error `out_of_range`, field `minor_radius`, rendered `Tube radius must be less than the ring radius (20 mm), but it is 20 mm. Use a value less than 20 mm.`
5. **Zero tube.** `TOR` with `"minor_radius": "0"`. Expected: error `out_of_range`, field `minor_radius`, bound `GreaterThan(0)`.
6. **Performance.** Test 1 builds in under 2 ms (desktop, release).
7. **Tube below the tolerance.** `TOR` with `"minor_radius": "0.0000001"`. Expected: error `out_of_range`, field `minor_radius`, rendered `Tube radius must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm.` (the value is written with the digits that tell it apart from the bound, features.md section 3.12); b-1 empty.
8. **Tube within the tolerance of the ring.** `TOR` with `"major_radius": "10", "minor_radius": "9.9999999"`. Expected: error `out_of_range`, field `minor_radius`, rendered `Tube radius must be less than the ring radius (10 mm), but it is 9.9999999 mm. Use a value less than 10 mm.`; b-1 empty.
9. **Intersect.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["60", "60", "10"], "op": "new"}` (z 0..10), `TOR` with `"op": "intersect"`, as `examples/torus/intersect`. Expected: ok; volume 4934.802201 (the upper half of the torus), 2 faces, bbox [-25,-25,0]-[25,25,5]; change `Intersect`; `f-tor1/surface` toroidal with area 1973.920880; `f-box1/bottom` planar at z=0, the ring between radius 15 and 25, with area 1256.637061.
10. **Join.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "20", "20"], "op": "new"}` (spans [-20,-10,0]-[20,10,20]), `TOR` with `"major_radius": "10", "minor_radius": "2", "plane": {"kind": "origin", "plane": "XZ"}, "position": ["0", "20", "0"], "op": "join"`, as `examples/torus/join`: a ring standing on the top face, centered at (0,0,20) with its axis along Y, so the plane of the top face contains the axis and halves the ring. Expected: ok; volume 16394.784176 (the upper half joins as a handle: 16000 + 40π²), 7 faces, 1 solid, bbox [-20,-10,0]-[20,10,32]; change `Join`; `f-tor1/surface` toroidal with area 394.784176; `f-box1/top` planar at z=20 with area 774.867259 (800 less the two discs of radius 2 where the handle stands).
11. **Cut.** As 10 with op cut, as `examples/torus/cut`. Expected: ok; volume 15605.215824 (the lower half is cut away, a channel under the top face), 7 faces, 1 solid, bbox [-20,-10,0]-[20,10,20]; change `Cut`; `f-tor1/surface` toroidal with area 394.784176; `f-box1/top` planar at z=20 with area 774.867259.
12. **On a datum.** `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XZ"}, "offset": "10"}` (origin (0,-10,0), normal (0,-1,0)), `TOR` with `"plane": {"kind": "datum", "feature": "f-dp1"}`, as `examples/torus/on-datum`. Expected: ok; volume 9869.604401, 1 face, bbox [-25,-15,-25]-[25,-5,25] (test 3's torus moved 10 along the datum's normal); dependencies `features == ["f-dp1"]`.
13. **On a face of another body.** Bodies `[b-1, b-2]`; `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "10"], "op": "new"}`; `{"id": "f-tor1", "type": "torus", "body": "b-2", "major_radius": "8", "minor_radius": "2", "plane": {"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=400"}}, "position": ["0", "0", "2"], "op": "new"}`, as `examples/torus/on-face`. Expected: ok; b-2 volume 631.654682, 1 face, bbox [-10,-10,10]-[10,10,14] (resting on the top of the box); b-1 unchanged; f-tor1 dependencies `bodies_read == ["b-1"]`, `bodies_written == ["b-2"]`.

## Open issues

None beyond features.md.
