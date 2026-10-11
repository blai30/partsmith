# Feature: box

Status: M1 spec. Type name `box`. Shared contracts: [features.md](../features.md), primitive placement in features.md section 3.14. Why primitives are separate types: features.md section 3.13. Naming rule: [naming.md](../naming.md) section 3.6.4.

## 1. Purpose and scope

Box makes a rectangular block standing on a plane, centered on the placement origin in x and y, and combines it with a body.

### In M1

- Size in x, y and z of the placement frame.
- Placement on an origin plane, a datum plane or a flat face, with an offset.
- Ops `new`, `join`, `cut`, `intersect`.

### Later

- Corner-anchored and fully centered placement modes.
- Rotation of the placement within the plane (use `move_body` in M1).
- Wedge, prism and pipe primitives.

## 2. Public API

```rust
/// A rectangular block.
pub struct BoxFeature;

impl Feature for BoxFeature {
    const TYPE: &'static str = "box";
    type Params = BoxParams;
}

/// Params of a box feature.
pub struct BoxParams {
    /// Target body.
    pub body: Id,
    /// Size along the placement x, y and z axes. Lengths, greater than 0.
    pub size: [Expr; 3],
    /// Placement plane. Default origin XY, omitted when default.
    pub plane: PlaneRef,
    /// Placement offset. Default ["0", "0", "0"], omitted when default.
    pub position: [Expr; 3],
    /// How the box combines with the body. Required.
    pub op: BodyOp,
}
```

The Rust type is `BoxFeature` because `Box` is the standard library's pointer type; the type name in files is `box`.

JSON:

```json
{"id": "f-box1", "type": "box", "body": "b-1", "size": ["80", "50", "30"], "op": "new"}
{"id": "f-box2", "type": "box", "body": "b-1", "size": ["10", "10", "5 mm"], "plane": {"kind": "datum", "feature": "f-dp1"}, "position": ["20", "0", "0"], "op": "cut"}
```

### Manifest

Label `Box`, category `create`, icon `box`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `size[0]` | Length | yes | length | | |
| `size[1]` | Width | yes | length | | |
| `size[2]` | Height | yes | length | | |
| `op` | Operation | yes | | | `new` New body, `join` Join, `cut` Cut, `intersect` Intersect |
| `plane` | Plane | no | | `plane` | |
| `position[0]` | Position X | no | length | | |
| `position[1]` | Position Y | no | length | | |
| `position[2]` | Position Z | no | length | | |
| `body` | Body | no | | `body` | |

### Python

Dedicated form: `box(length, width, height, at=..., part=..., op=...)` (python-api.md section 2.7). Generic form of the first JSON above: `feature("box", id="f-box1", body="b-1", size=["80", "50", "30"], op="new")`.

## 3. Behavior

### 3.1 Dependencies

- `bodies_written`: `[body]`; plus the placement dependencies (features.md section 3.14).
- `expressions`: `size[0]`, `size[1]`, `size[2]` (Length), then `position[0..2]` (Length).

### 3.2 Build

1. Each size must be greater than 0, else `OutOfRange` (`GreaterThan(0)`) on `size[i]`.
2. In the local frame the box spans x in [-sx/2, sx/2], y in [-sy/2, sy/2], z in [0, sz]: kernel `make_box` (corner-anchored) with the local frame moved by (-sx/2, -sy/2, 0).
3. Name the faces from the kernel's `PrimitiveFace` roles, then `apply_op`.

### 3.3 Element names

| Name | Face (local frame) |
| --- | --- |
| `<id>/bottom` | z = 0, outward normal -z |
| `<id>/top` | z = sz, outward normal +z |
| `<id>/front` | y = -sy/2, outward normal -y |
| `<id>/back` | y = +sy/2, outward normal +y |
| `<id>/left` | x = -sx/2, outward normal -x |
| `<id>/right` | x = +sx/2, outward normal +x |

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| A size ≤ 0 | `OutOfRange` | `Length must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Expression or dimension error | `Expression` | `Height: Expected a length, but this expression gives an angle.` |
| Placement face not flat | `NotPlanar` | `The face selected for Plane is not flat. Pick a flat face, a datum plane or an origin plane.` |
| Placement face lost | `ReferenceNotFound` | `The face selected for Plane can no longer be found. Re-pick it in the repair panel.` |
| Placement datum is not a datum | `WrongFeatureType` | `'Sketch 1' is not a datum plane. Pick a datum plane for Plane.` |
| Body rules | `BodyNotEmpty`, `BodyEmpty`, `EmptyResult`, warning `NoEffect` | features.md sections 3.5 and 4 |
| Kernel failure | `Kernel` | the kernel's message |

## 5. Performance

- Op new: 2 ms. With a boolean against a 200-face body: 60 ms.

## 6. Acceptance tests

Conventions from features.md section 6.1. Bodies `[b-1]` unless stated.

1. **Default placement.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "20", "30"], "op": "new"}`. Expected: ok; volume 6000, 6 faces, bbox [-5,-10,0]-[5,10,30]. Faces: `f-box1/bottom` area 200, plane z=0, normal (0,0,-1); `f-box1/top` area 200, plane z=30, normal (0,0,1); `f-box1/front` area 300, plane y=-10; `f-box1/back` area 300, plane y=10; `f-box1/left` area 600, plane x=-5; `f-box1/right` area 600, plane x=5.
2. **Offset.** As 1 with `"position": ["1", "2", "3"]`. Expected: bbox [-4,-8,3]-[6,12,33].
3. **On XZ.** As 1 with `"plane": {"kind": "origin", "plane": "XZ"}`. The local frame has x +X, y +Z and normal -Y. Expected: volume 6000, bbox [-5,-30,-10]-[5,0,10]; `f-box1/top` is the face at y=-30 with outward normal (0,-1,0); `f-box1/front` is the face at z=-10.
4. **On a datum.** Features `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "5"}` and test 1's box with `"plane": {"kind": "datum", "feature": "f-dp1"}`. Expected: bbox [-5,-10,5]-[5,10,35]; dependencies `features == ["f-dp1"]`.
5. **On a face of another body.** Bodies `[b-1, b-2]`; test 1's box; `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["4", "4", "4"], "plane": {"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,30|a=200"}}, "op": "new"}`. Expected: b-2 bbox [-2,-2,30]-[2,2,34]; f-box2 dependencies `bodies_read == ["b-1"]`, `bodies_written == ["b-2"]`.
6. **Zero size.** `"size": ["0", "20", "30"]`. Expected: error `out_of_range`, field `size[0]`, rendered `Length must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.`
7. **Join overlapping.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`, `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "position": ["5", "0", "0"], "op": "join"}`. Expected: volume 1500, 6 faces (coplanar faces merged by the kernel boolean), bbox [-5,-5,0]-[10,5,10].
8. **Cut a square hole.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "op": "new"}`, `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["10", "10", "20"], "position": ["0", "0", "-5"], "op": "cut"}`. Expected: volume 15000, 10 faces, bbox [-20,-20,0]-[20,20,10].
9. **Expressions.** Parameters `[{"id": "p-w", "name": "w", "expr": "8 mm"}]`; `"size": ["w", "2 * w", "w / 2"]`. Expected: volume 512, bbox [-4,-8,0]-[4,8,4].
10. **Performance.** Test 1 builds in under 2 ms (desktop, release).

## Open issues

1. naming.md section 3.6.4 describes the box as spanning `[0, length]` locally (the kernel's `make_box`); the box feature centers it in x and y, which changes positions, not face roles. The reviewer should align the description.
