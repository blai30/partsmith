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

Description (since M2, [ADR 0065](../../adr/0065-feature-help-start-values-and-new-body-fields-in-the-manifests.md)): `Adds a cylinder of the given radius and height.`

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `radius` | Radius | yes | length | | |
| `height` | Height | yes | length | | |
| `op` | Operation | yes | | | `new` New body, `join` Join, `cut` Cut, `intersect` Intersect |
| `plane`, `position[0..2]` | features.md section 3.14 | no | | | |
| `body` | Body | no | | `body` | |

No field has help, a start value or a new body (since M2, features.md section 2.3).

### Python

Dedicated form: `cylinder(radius, height, plane=..., position=..., op=...)` (python-api.md section 2.7), produced by the `Feature::python` hook with the primitive rules of features.md section 3.14. The JSON above is `cylinder(5, 10, id="f-cyl1")`. Generic form: `feature("cylinder", id="f-cyl1", body="b-1", radius="5", height="10", op="new")`.

## 3. Behavior

### 3.1 Dependencies

- `bodies_written`: `[body]`; plus placement dependencies.
- `expressions`: `radius`, `height` (Length), then `position[0..2]`.

### 3.2 Build

1. radius and height at least `LINEAR_TOLERANCE` (1e-6 mm), else `OutOfRange` on that field (`GreaterThan(0)`, or `AtLeast(LINEAR_TOLERANCE)` for a value above 0, features.md section 3.14), in field order.
2. Kernel `make_cylinder` in the local frame: axis on local z, base circle of `radius` at z = 0, top at z = `height`.
3. Name the faces, then `apply_op(ctx, body, op, vec![tool], None)` (a primitive has no direction, features.md section 3.10).

### 3.3 Element names

| Name | Face (local frame) |
| --- | --- |
| `<id>/bottom` | Disc at z = 0 |
| `<id>/top` | Disc at z = height |
| `<id>/side` | Cylindrical face |

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Radius below 1e-6 mm (0 and negative included) | `OutOfRange` | `Radius must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Height below 1e-6 mm (0 and negative included) | `OutOfRange` | `Height must be greater than 0 mm, but it is -2 mm. Use a value greater than 0 mm.` |
| A size above the modeling limit | `OutOfRange` | `Radius must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less.` |
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
8. **Radius below the tolerance.** `CYL` with `"radius": "0.0000001"`. Expected: error `out_of_range`, field `radius`, rendered `Radius must be at least 0.000001 mm, but it is 0.0000001 mm. Use 0.000001 mm or more.` (the bound the check applies, features.md section 3.4); b-1 empty.
9. **Height below the tolerance.** `CYL` with `"height": "0.0000001"`. Expected: error `out_of_range`, field `height`, rendered `Height must be at least 0.000001 mm, but it is 0.0000001 mm. Use 0.000001 mm or more.`
10. **Intersect.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "10"], "op": "new"}`, `CYL` with `"height": "20", "position": ["0", "0", "-5"], "op": "intersect"` (z -5..15, through the box), as `examples/cylinder/intersect`. Expected: ok; volume 785.398163 (the part of the cylinder inside the box, as test 1), 3 faces, bbox [-5,-5,0]-[5,5,10]; change `Intersect`; `f-cyl1/side` cylindrical with radius 5 and area 314.159265; `f-box1/bottom` planar at z=0 and `f-box1/top` planar at z=10, each with area 78.539816; no face named `f-cyl1/top` or `f-cyl1/bottom`.
11. **On a datum.** `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "5", "flip": true}` (origin (0,0,5), normal (0,0,-1)), `CYL` with `"plane": {"kind": "datum", "feature": "f-dp1"}`, as `examples/cylinder/on-datum`. Expected: ok; volume 785.398163, 3 faces, bbox [-5,-5,-5]-[5,5,5]: the cylinder stands on the datum and grows along its normal, down; `f-cyl1/bottom` planar at z=5 with outward normal (0,0,1); `f-cyl1/top` planar at z=-5 with outward normal (0,0,-1); dependencies `features == ["f-dp1"]`.
12. **On a face of the body.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"}` (spans [-10,-10,0]-[10,10,20]), `CYL` with `"plane": {"kind": "face", "body": "b-1", "face": {"q": "f-box1/right", "fp": "plane|n=1,0,0|c=10,0,10|a=400"}}, "position": ["0", "10", "0"], "op": "join"`, as `examples/cylinder/on-face`. The face placement rule (features.md section 3.6) gives the frame origin (10,0,0), x-axis (0,1,0) (world +X is perpendicular to the face), normal (1,0,0) and y-axis (0,0,1), so the position is world (10,0,10), the center of the face. Expected: ok; volume 8785.398163, 8 faces, bbox [-10,-10,0]-[20,10,20]; change `Join`; `f-cyl1/side` cylindrical with radius 5, its axis along (1,0,0) on the line y=0, z=10; `f-cyl1/top` planar at x=20 with outward normal (1,0,0) and area 78.539816; no face named `f-cyl1/bottom` (it lies on the face, which keeps its name `f-box1/right` with area 321.460184); dependencies `bodies_read == []`, `bodies_written == ["b-1"]` (the face's body is the target, features.md section 3.14).
13. **The manifest's description, help, start values and new bodies** (since M2, features.md section 2.3, [ADR 0065](../../adr/0065-feature-help-start-values-and-new-body-fields-in-the-manifests.md)). Not a document test: it lives in `crates/partsmith-features/tests/registry_registration.rs`, as the `cylinder` case of features.md tests 46, 48, 50 and 51. The manifest of `Registry::with_builtins().get("cylinder")` has the description of the Manifest section, and none of its hints has help, a start value or a new body.

## Open issues

1. Radius rather than diameter matches build123d, python-api.md and the fillet feature; makers often think in diameters, so the inspector may later offer a diameter input that writes `radius` as `d / 2`.
