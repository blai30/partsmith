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

Dedicated form: `mirror(plane, into=<target part>)` (python-api.md section 2.7), produced by the `Feature::python` hook with the shared rules of features.md section 3.11: the `with <part>:` block gives `body`; `plane` is `PythonValue::plane`; op `new` writes `into=<target part>` and op `join` writes no `into`. With `f-dp1` as `plane_1` and `b-2` as `part_2`, the two JSON records above are `mirror(Plane.YZ, id="f-mir1")` and `mirror(plane_1, into=part_2, id="f-mir2")`. Generic form of the second JSON above:

```python
feature(
    "mirror",
    id="f-mir2",
    body="b-1",
    plane={"kind": "datum", "feature": "f-dp1"},
    op="new",
    target="b-2",
)
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

1. The source body must hold a solid, else `BodyEmpty { field: body, body, use_new: false }` (recompute's diagnosis adds why it is empty, features.md section 4.1).
2. Resolve the plane with `ctx.plane("plane", ...)` against the state before this feature, so a face of the source body may be the plane. Its warnings and notices go into the output; a face reference whose faces do not lie in one plane gives `NotCoplanar` (features.md section 3.6).
3. The image is the source shape transformed by kernel `Transform::mirror` through the plane (index-preserving, positive volume), named with `source_map.with_instance(<id>, Instance { first: 1, second: 0 }, m)` with `m` that reflection (naming.md section 3.5.1). Every face of the image has its source face's motion followed by `m`, an improper motion. A sketch on a face of the image whose fallback is in body coordinates is moved by the same reflection as the face, so the face never turns over against it, and the sketch keeps its placement and drawing ([ADR 0023](../../adr/0023-frames-live-on-the-model.md), features.md section 3.6 and open issue 9): test 13.
4. Op join: `apply_op(ctx, body, Join, vec![image], None)` (a body symmetric about the plane gives `NoEffect { how: Inside }`, `This mirror adds nothing to body 'Body 1': its mirror image lies entirely inside the body.`; an image apart from the source is what a mirror makes, so no `SeparateSolid`). Op new: `apply_op(ctx, target, New, vec![image], None)` (a target with a solid gives `BodyNotEmpty { body: target, target: Some(target) }`).

### 3.4 Element names

| Name | Face |
| --- | --- |
| `<face name>@<id>:1` | The image of the source face named `<face name>`, for example `f-box1/top@f-mir1:1`. |

Source faces keep their names. Where the image fuses with the source, merged faces follow naming.md's merge rule.

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Operation New body without target | `TargetRequired` | `Operation New body puts the copies into another body, so Target body needs an empty body. Pick one, or set Operation to Join.` |
| Target with Operation Join | `TargetNotAllowed` | `Target body is used only when Operation is New body. Clear Target body, or set Operation to New body.` |
| Target is the source | `SameBody` | `Body 'Body 1' cannot be used twice here. Pick a different body for Target body.` |
| Source empty | `BodyEmpty` | without a cause: `Body 'Body 1' has no solid at this point, so this mirror has nothing to work on. Pick a body that has a solid.`; with one, features.md section 4.1 |
| Target has a solid | `BodyNotEmpty` | `Body 'Body 2' already has a solid here, so this mirror cannot put its result there. Pick an empty body for Target body.` |
| Plane face not flat, not in one plane, or lost | `NotPlanar`, `NotCoplanar`, `ReferenceNotFound` | features.md section 4.1 |
| Kernel failure | `Kernel` | `Partsmith's geometry kernel could not build this mirror. {hint}` (features.md section 4.1) |
| Image inside the source | warning `NoEffect` (`Inside`) | `This mirror adds nothing to body 'Body 1': its mirror image lies entirely inside the body.` |
| The plane reference guessed or followed | warning `ReferenceGuessed`, notice `ReferenceFollowed` | features.md section 4.2 |

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
4. **New without target.** `BOX-R("10")`, `MIR(YZ, "op": "new")`. Expected: error `target_required` rendered `Operation New body puts the copies into another body, so Target body needs an empty body. Pick one, or set Operation to Join.`, fixes `SetField { field: op, value: "join" }` then `Pick { pointer: "/target" }`.
5. **Target with join.** `BOX-R("10")`, `MIR(YZ, "target": "b-2")`. Expected: error `target_not_allowed` rendered `Target body is used only when Operation is New body. Clear Target body, or set Operation to New body.`, fixes `SetField { field: op, value: "new" }` then `SetField { field: target, value: null }`.
6. **Target with a solid.** `BOX-R("10")`, `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["1", "1", "1"], "position": ["50", "0", "0"], "op": "new"}`, `MIR(YZ, "op": "new", "target": "b-2")`. Expected: error `body_not_empty` rendered `Body 'Body 2' already has a solid here, so this mirror cannot put its result there. Pick an empty body for Target body.`, fix `Pick { pointer: "/target" }`; b-2 volume 1.
7. **Target is the source.** `BOX-R("10")`, `MIR(YZ, "op": "new", "target": "b-1")`. Expected: error `same_body`, field `target`.
8. **Across a datum.** `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "YZ"}, "offset": "20"}`, `BOX-R("10")`, `MIR({"kind": "datum", "feature": "f-dp1"})`. Expected: volume 2000, 12 faces, bbox [5,-5,0]-[35,5,10].
9. **Across its own face.** `BOX-R("10")`, `MIR({"kind": "face", "body": "b-1", "face": {"q": "f-box1/right", "fp": "plane|n=1,0,0|c=15,0,5|a=100"}})`. Expected: volume 2000, 6 faces, bbox [5,-5,0]-[25,5,10].
10. **Symmetric body.** `BOX-R("0")`, `MIR(YZ)`. Expected: warning `no_effect` with how `Inside`, rendered `This mirror adds nothing to body 'Body 1': its mirror image lies entirely inside the body.`; b-1 unchanged.
11. **Empty source.** `MIR(YZ)` alone. Expected: error `body_empty` rendered `Body 'Body 1' has no solid at this point, so this mirror has nothing to work on. Pick a body that has a solid.`
12. **Performance.** Test 1 builds in under 15 ms (desktop, release).
13. **A sketch on the mirror image keeps its place (ADR 0023).** `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "YZ"}, "offset": "0"}`; `BOX-R("10")` (x 5..15); `MIR({"kind": "datum", "feature": "f-dp1"})` (image x -15..-5); `{"id": "f-sk2", "type": "sketch", "plane": {"kind": "face", "body": "b-1", "face": {"q": "f-box1/top@f-mir1:1", "fp": "plane|n=0,0,1|c=-10,0,10|a=100|adj=4,4"}, "fallback": {"origin": [10, 0, 10], "x_axis": [-1, 0, 0], "normal": [0, 0, 1]}, "fallback_frame": "body"}, "entities": [{"id": "e-p1", "type": "point", "position": [2, 3]}]}` (picked on the image's top with the placement origin (-10, 0, 10), x axis (1, 0, 0): stored as its reflection, in the coordinates of 'Box 1'); `{"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk2", "points": ["e-p1"], "diameter": "1", "extent": {"kind": "through_all"}}`; as `examples/mirror/sketch-on-image`. Expected: every feature status ok, with no `sketch_turned_over` (the reflection moves the hint with the face); f-sk2 placement origin (-10, 0, 10), x axis (1, 0, 0), normal (0, 0, 1), right-handed; the bore's axis passes through (-8, 3, 0); b-1 volume 1992.146018. With f-dp1's offset `"-10"` (the plane moves to x = -10 and the image to x -35..-25; made by the test from the example): status ok, no warning; placement origin (-30, 0, 10), x axis (1, 0, 0); the bore's axis passes through (-28, 3, 0); volume 1992.146018.

## Open issues

None beyond features.md.
