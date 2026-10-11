# Feature: mirror

Status: M1 spec. Type name `mirror`. Shared contracts: [features.md](../features.md), copy features in features.md section 3.5. Naming rule: [naming.md](../naming.md) section 3.6.10.

## 1. Purpose and scope

Mirror reflects a body across a plane. The mirror image is fused into the source body, or put into another, empty body.

### In M1

- Mirror one body across an origin plane, a datum plane or a flat face.
- Op `join` (default): the source body becomes source ∪ mirror image.
- Op `new`: the mirror image goes into an empty `target` body; the source is unchanged.

### Later

- Mirroring features and faces (feature-level mirror).
- Mirroring several bodies at once.
- Mirror that replaces the source with its image (a mirror move).

## 2. Public API

```rust
/// Reflects a body across a plane.
pub struct Mirror;

impl Feature for Mirror {
    const TYPE: &'static str = "mirror";
    type Params = MirrorParams;
}

/// Params of a mirror feature.
pub struct MirrorParams {
    /// The body to mirror.
    pub body: Id,
    /// The mirror plane.
    pub plane: PlaneRef,
    /// Where the image goes. Default join, omitted when join.
    pub op: CopyOp,
    /// The empty body that receives the image. Required with op new, not allowed with op join. Omitted when None.
    pub target: Option<Id>,
}
```

JSON:

```json
{"id": "f-mir1", "type": "mirror", "body": "b-1", "plane": {"kind": "origin", "plane": "YZ"}}
{"id": "f-mir2", "type": "mirror", "body": "b-1", "plane": {"kind": "datum", "feature": "f-dp1"}, "op": "new", "target": "b-2"}
```

### Manifest

Label `Mirror`, category `pattern`, icon `mirror`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `body` | Body | yes | | `body` | |
| `plane` | Mirror plane | yes | | `plane` | |
| `op` | Operation | yes | | | `join` Join, `new` New body |
| `target` | Target body | yes | | `body` | |

### Python

Dedicated form: `mirror(plane, part=<body>)` (python-api.md section 2.7) for op join; op new uses the generic form until python-api.md adds it. Generic form of the second JSON above:

```python
feature("mirror", id="f-mir2", body="b-1", plane={"kind": "datum", "feature": "f-dp1"}, op="new", target="b-2")
```

## 3. Behavior

### 3.1 Validation

- Op `new` without `target`: `TargetRequired { field: "target" }`.
- Op `join` with `target`: `TargetNotAllowed { field: "target" }`.
- `target` equal to `body`: `SameBody { field: "target" }`.

### 3.2 Dependencies

- Op join: `bodies_written` `[body]`. Op new: `bodies_read` `[body]`, `bodies_written` `[target]`.
- Plane dependencies: a datum adds its feature; a face of a body not already listed adds it to `bodies_read`.
- `expressions`: none.

### 3.3 Build

1. The source body must hold a solid, else `BodyEmpty { use_new: false }`.
2. Resolve the plane with `ctx.plane("plane", ...)` against the state before this feature, so a face of the source body may be the plane.
3. The image is the source shape transformed by kernel `Transform::mirror` through the plane (index-preserving, positive volume), named with `source_map.with_instance(<id>, Instance { first: 1, second: 0 })`.
4. Op join: `apply_op(ctx, body, Join, image)` (a body symmetric about the plane gives `NoEffect`). Op new: `apply_op(ctx, target, New, image)` (a target with a solid gives `BodyNotEmpty`).

### 3.4 Element names

| Name | Face |
| --- | --- |
| `<face name>@<id>:1` | The image of the source face named `<face name>`, for example `f-box1/top@f-mir1:1`. |

Source faces keep their names. Where the image fuses with the source, merged faces follow naming.md's merge rule.

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Op new without target | `TargetRequired` | `Op new puts the copies into another body. Pick an empty body for Target body.` |
| Target with op join | `TargetNotAllowed` | `Target body is only used with op new. Remove it, or set the op to new.` |
| Target is the source | `SameBody` | `Body 'Body 1' cannot be used twice here. Pick a different body for Target body.` |
| Source empty | `BodyEmpty` | `Body 'Body 1' is empty at this point, so this feature has nothing to work on. Pick a body that has a solid.` |
| Target has a solid | `BodyNotEmpty` | `Body 'Body 2' already has a solid at this point, so op new cannot start it. Use join, cut or intersect, or pick an empty body.` |
| Plane face not flat or lost | `NotPlanar`, `ReferenceNotFound` | features.md section 4.1 |
| Kernel failure | `Kernel` | the kernel's message |
| Image inside the source | warning `NoEffect` | features.md section 4.2 |

## 5. Performance

- 6-face box with join: 15 ms. 200-face body with join: 80 ms.

## 6. Acceptance tests

Conventions from features.md section 6.1. Bodies `[b-1, b-2]`. Fixtures:

- `BOX-R(x)`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "position": [x, "0", "0"], "op": "new"}` (spans [x-5,-5,0]-[x+5,5,10]).
- `YZ`: `{"kind": "origin", "plane": "YZ"}`.
- `MIR(plane, extra)`: `{"id": "f-mir1", "type": "mirror", "body": "b-1", "plane": <plane>, <extra>}`.

1. **Join, apart.** `BOX-R("10")`, `MIR(YZ)`. Expected: ok; b-1 volume 2000, 12 faces, 2 solids, bbox [-15,-5,0]-[15,5,10]; `f-box1/top@f-mir1:1` planar at z=10 spanning x -15..-5; `f-box1/right@f-mir1:1` planar at x=-15 with outward normal (-1,0,0); `f-box1/right` at x=15; change `Join`.
2. **Join, touching.** `BOX-R("5")`, `MIR(YZ)`. Expected: volume 2000, 6 faces, bbox [-10,-5,0]-[10,5,10].
3. **New body.** `BOX-R("10")`, `MIR(YZ, "op": "new", "target": "b-2")`. Expected: b-1 unchanged (volume 1000, bbox [5,-5,0]-[15,5,10]); b-2 volume 1000, 6 faces, bbox [-15,-5,0]-[-5,5,10], face `f-box1/top@f-mir1:1`, change `New`; dependencies `bodies_read == ["b-1"]`, `bodies_written == ["b-2"]`.
4. **New without target.** `BOX-R("10")`, `MIR(YZ, "op": "new")`. Expected: error `target_required` rendered `Op new puts the copies into another body. Pick an empty body for Target body.`
5. **Target with join.** `BOX-R("10")`, `MIR(YZ, "target": "b-2")`. Expected: error `target_not_allowed` rendered `Target body is only used with op new. Remove it, or set the op to new.`
6. **Target with a solid.** `BOX-R("10")`, `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["1", "1", "1"], "position": ["50", "0", "0"], "op": "new"}`, `MIR(YZ, "op": "new", "target": "b-2")`. Expected: error `body_not_empty`; b-2 volume 1.
7. **Target is the source.** `BOX-R("10")`, `MIR(YZ, "op": "new", "target": "b-1")`. Expected: error `same_body`, field `target`.
8. **Across a datum.** `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "YZ"}, "offset": "20"}`, `BOX-R("10")`, `MIR({"kind": "datum", "feature": "f-dp1"})`. Expected: volume 2000, 12 faces, bbox [5,-5,0]-[35,5,10].
9. **Across its own face.** `BOX-R("10")`, `MIR({"kind": "face", "body": "b-1", "face": {"q": "f-box1/right", "fp": "plane|n=1,0,0|c=15,0,5|a=100"}})`. Expected: volume 2000, 6 faces, bbox [5,-5,0]-[25,5,10].
10. **Symmetric body.** `BOX-R("0")`, `MIR(YZ)`. Expected: warning `no_effect`; b-1 unchanged.
11. **Empty source.** `MIR(YZ)` alone. Expected: error `body_empty`.
12. **Performance.** Test 1 builds in under 15 ms (desktop, release).

## Open issues

None beyond features.md.
