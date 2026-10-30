# Feature: linear_pattern

Status: M1 spec. Type name `linear_pattern`. Shared contracts: [features.md](../features.md), copy features in features.md section 3.5. Naming rule: [naming.md](../naming.md) section 3.6.11.

## 1. Purpose and scope

Linear pattern copies a body at equal spacing along one direction, or along two directions as a grid. The copies are fused into the source body, or put into another, empty body.

### In M1

- One or two directions, each with a count (including the original) and a spacing.
- Directions from an origin axis, a sketch line, a straight edge or a round face's axis, each with `reverse`.
- Op `join` (default) or `new` with a `target` body, as for mirror.
- A limit of 10000 instances per feature.

### Later

- Patterns of features and faces.
- Skipping instances.
- Spacing by total extent instead of step.
- Symmetric patterns around the original.
- Pattern along a path.
- Patterns of patterns as a single feature.

## 2. Public API

```rust
/// Copies a body along one or two directions.
pub struct LinearPattern;

impl Feature for LinearPattern {
    const TYPE: &'static str = "linear_pattern";
    type Params = LinearPatternParams;
}

/// Params of a linear pattern feature.
pub struct LinearPatternParams {
    /// The body to copy.
    pub body: Id,
    /// The first direction.
    pub direction: LinearPatternDirection,
    /// An optional second direction for a grid. Omitted when None.
    pub second_direction: Option<LinearPatternDirection>,
    /// Where the copies go. Default join, omitted when join.
    pub op: CopyOp,
    /// The empty body that receives the copies. Required with op new, not allowed with op join.
    pub target: Option<Id>,
}

/// One direction of a linear pattern. Named with the feature prefix so its schema definition is unique (features.md section 2.2).
pub struct LinearPatternDirection {
    /// The direction (only its direction is used).
    pub axis: AxisRef,
    /// Number of instances along this direction, including the original. Count, at least 1.
    pub count: Expr,
    /// Distance between neighboring instances. Length, greater than 0.
    pub spacing: Expr,
    /// Go against the axis direction. Default false, omitted when false.
    pub reverse: bool,
}
```

JSON:

```json
{"id": "f-lp1", "type": "linear_pattern", "body": "b-1",
 "direction": {"axis": {"kind": "origin", "axis": "X"}, "count": "3", "spacing": "20 mm"},
 "second_direction": {"axis": {"kind": "origin", "axis": "Y"}, "count": "2", "spacing": "15 mm"}}
```

### Manifest

Label `Linear pattern`, category `pattern`, icon `linear-pattern`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `body` | Body | yes | | `body` | |
| `direction.axis` | Direction | yes | | `axis` | |
| `direction.count` | Count | yes | count | | |
| `direction.spacing` | Spacing | yes | length | | |
| `direction.reverse` | Reverse | no | | | |
| `second_direction.axis` | Second direction | no | | `axis` | |
| `second_direction.count` | Second count | no | count | | |
| `second_direction.spacing` | Second spacing | no | length | | |
| `second_direction.reverse` | Second reverse | no | | | |
| `op` | Operation | no | | | `join` Join, `new` New body |
| `target` | Target body | no | | `body` | |

### Python

Dedicated form: `linear_pattern(direction, count, spacing, reverse=True, second=(axis, count, spacing), second_reverse=True, into=<target part>)` (python-api.md section 2.7), produced by the `Feature::python` hook with the shared rules of features.md section 3.11: the `with <part>:` block gives `body`; `direction` is `PythonValue::axis` of `direction.axis`, followed by its count and spacing; `reverse=True` when `direction.reverse` is true; `second=` is a tuple of the second direction's axis, count and spacing when it is set, followed by `second_reverse=True` when its `reverse` is true ([ADR 0048](../../adr/0048-python-keywords-gui-round-trip-and-python-budgets.md)); op `new` writes `into=<target part>`. The hook returns `None` only when an axis or the target has no variable. The JSON above is `linear_pattern(Axis.X, 3, 20*mm, second=(Axis.Y, 2, 15*mm), id="f-lp1")`, and with `"reverse": true` in both directions it is `linear_pattern(Axis.X, 3, 20*mm, reverse=True, second=(Axis.Y, 2, 15*mm), second_reverse=True, id="f-lp1")`. Generic form:

```python
feature(
    "linear_pattern",
    id="f-lp1",
    body="b-1",
    direction={"axis": {"kind": "origin", "axis": "X"}, "count": "3", "spacing": "20 mm"},
    second_direction={"axis": {"kind": "origin", "axis": "Y"}, "count": "2", "spacing": "15 mm"},
)
```

## 3. Behavior

### 3.1 Validation

As [mirror.md](mirror.md) section 3.1 for `op` and `target`.

### 3.2 Dependencies

- Bodies as mirror (join: write `body`; new: read `body`, write `target`); axis dependencies (sketch feature, or an edge or face body in `bodies_read` if not already listed).
- `expressions`: `("direction.count", Count)`, `("direction.spacing", Length)`, then the same for `second_direction` when set.

### 3.3 Build

1. The source body must hold a solid, else `BodyEmpty { field: body, body, use_new: false }` (recompute's diagnosis adds why it is empty, features.md section 4.1).
2. For each direction, in order: n = `ctx.eval_count("direction.count", count, 1)` (`second_direction.count` for the second direction), so a count below 1, fractional or not, is `OutOfRange` (`AtLeast(1)`) (features.md section 3.4); s = spacing > 0 (`OutOfRange`, `GreaterThan(0)`) and at most the modeling limit (`AtMost(MODEL_LIMIT)`); d = the resolved axis direction (`ctx.axis`, features.md section 3.6), negated when `reverse`. The axis's warnings and notices go into the output. An `edge` or `face` axis keeps the side the user picked: it is oriented by its stored `direction` moved with the part, or else by its fingerprint, so a small edit that tilts the edge never sends the copies the other way (corpus R38); edges that do not lie on one line give `NotCollinear`.
3. Total instances N = n1 × n2 (n2 = 1 without a second direction). N > 10000 gives `TooManyInstances { count: N, limit: 10000 }`.
4. Instances are (i, j) for i in 0..n1 and j in 0..n2; (0, 0) is the original. Each other instance is the source translated by t = i·s1·d1 + j·s2·d2 (kernel `transform`, index-preserving), named with `source_map.with_instance(<id>, Instance { first: i, second: j }, t)` (naming.md section 3.5.1): every face of a copy has the motion of its source face followed by t, so a sketch on a copy's face moves with the copy when the spacing changes ([sketch.md](sketch.md) test 27). Instances are built in order of j, then i. The build checks `ctx.is_canceled()` before each instance and returns `Canceled` when set.
5. If there are no copies (N = 1), the body (or target) is unchanged and a `NoEffect { how: NoCopies { fields } }` warning is added, with `fields` the count fields that are 1 (features.md section 3.15).
6. The copies are combined with one multi-argument kernel boolean: op join `apply_op(ctx, body, Join, copies, None)`; op new `apply_op(ctx, target, New, copies, None)`, which fuses the copies with each other into the empty `target`. Copies that overlap each other or the source fuse into one solid; separate copies stay separate solids of the same body, which is what a pattern makes, so no `SeparateSolid` warning is given (features.md section 3.5).

### 3.4 Element names

| Name | Face |
| --- | --- |
| `<face name>@<id>:<i>` | One direction: the copy of source face `<face name>` in instance i (1 ≤ i < n1), for example `f-box1/top@f-lp1:2`. |
| `<face name>@<id>:<i>.<j>` | Two directions, j ≥ 1: the copy in instance (i, j), for example `f-box1/top@f-lp1:2.1` and `f-box1/top@f-lp1:0.1`. Instances (i, 0) use the one-index form. |

Source faces keep their names. Instance numbers are pattern positions, not kernel indices, so a reference to instance 2 survives edits that do not change the count.

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Count < 1 (fractional included) | `OutOfRange` | `Count must be at least 1, but it is 0. Use 1 or more.` |
| Count not whole | `NotWholeNumber` | `Count must be a whole number, but it is 2.5.` with the fixes `SetField` `2` and `SetField` `3` (features.md section 4.1) |
| Spacing ≤ 0 | `OutOfRange` | `Spacing must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Spacing above the modeling limit | `OutOfRange` | `Spacing must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less.` |
| Too many instances | `TooManyInstances` | `This linear pattern would make 20000 copies, more than the limit of 10000. Lower Count.` |
| Axis errors | `NotStraight`, `NoAxis`, `NotCollinear`, `UnknownEntity`, `NotALine`, `ReferenceNotFound` | features.md section 4.1 |
| Op, target and body errors | as [mirror.md](mirror.md) section 4 | |
| Canceled during the build | `Canceled` | `Recompute was canceled before this linear pattern finished.` |
| No copies | warning `NoEffect` (`NoCopies`) | `This linear pattern makes no copies, because Count is 1.`; with two directions `This linear pattern makes no copies, because Count and Second count are 1.` |
| Copies inside the body | warning `NoEffect` (`Inside`) | `This linear pattern adds nothing to body 'Body 1': its copies lie entirely inside the body.` |
| The axis reference guessed or followed | warning `ReferenceGuessed`, notice `ReferenceFollowed` | features.md section 4.2 |

## 5. Performance

- 3 instances of a box with join: 30 ms. 100 instances of a 6-face box with join: 500 ms.

## 6. Acceptance tests

Conventions from features.md section 6.1. Bodies `[b-1, b-2]`. Fixtures:

- `CUBE10`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}` (spans [-5,-5,0]-[5,5,10]).
- `DIR(axis, count, spacing)`: `{"axis": <axis>, "count": count, "spacing": spacing}`; `X` is `{"kind": "origin", "axis": "X"}`, `Y` is `{"kind": "origin", "axis": "Y"}`.
- `LP(direction, extra)`: `{"id": "f-lp1", "type": "linear_pattern", "body": "b-1", "direction": <direction>, <extra>}`.

1. **Three apart.** `CUBE10`, `LP(DIR(X, "3", "20"))`. Expected: ok; b-1 volume 3000, 18 faces, 3 solids, bbox [-5,-5,0]-[45,5,10]; `f-box1/top@f-lp1:1` planar at z=10 spanning x 15..25; `f-box1/right@f-lp1:2` planar at x=45; `f-box1/right` at x=5; change `Join`.
2. **Overlapping copies fuse.** `CUBE10`, `LP(DIR(X, "3", "5"))`. Expected: volume 2000, 6 faces, 1 solid, bbox [-5,-5,0]-[15,5,10].
3. **Grid.** `CUBE10`, `LP(DIR(X, "3", "20"), "second_direction": DIR(Y, "2", "15"))`. Expected: volume 6000, 36 faces, bbox [-5,-5,0]-[45,20,10]; faces `f-box1/top@f-lp1:1`, `f-box1/top@f-lp1:0.1` and `f-box1/top@f-lp1:2.1` exist.
4. **Reverse.** `CUBE10`, `LP({"axis": X, "count": "3", "spacing": "20", "reverse": true})`. Expected: bbox [-45,-5,0]-[5,5,10].
5. **Count 1.** `CUBE10`, `LP(DIR(X, "1", "20"))`. Expected: warning `no_effect` with how `NoCopies { fields: [direction.count] }`, rendered `This linear pattern makes no copies, because Count is 1.`; b-1 unchanged. With `"second_direction": DIR(Y, "1", "15")` as well: rendered `This linear pattern makes no copies, because Count and Second count are 1.`
6. **Fractional count.** `CUBE10`, `LP(DIR(X, "2.5", "20"))`. Expected: error `not_whole_number`, field `direction.count`, rendered `Count must be a whole number, but it is 2.5.`, with the fixes `SetField` `2` then `SetField` `3`
7. **Zero count.** `CUBE10`, `LP(DIR(X, "0", "20"))`. Expected: error `out_of_range` rendered `Count must be at least 1, but it is 0. Use 1 or more.`
8. **Zero spacing.** `CUBE10`, `LP(DIR(X, "3", "0"))`. Expected: error `out_of_range`, field `direction.spacing`.
9. **New body.** `CUBE10`, `LP(DIR(X, "3", "20"), "op": "new", "target": "b-2")`. Expected: b-1 unchanged; b-2 volume 2000, 12 faces, bbox [15,-5,0]-[45,5,10], change `New`.
10. **Direction from a sketch line.** `CUBE10`; sketch `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [3, 4], "construction": true}]}`; `LP(DIR({"kind": "sketch_line", "sketch": "f-sk1", "line": "e-1"}, "2", "50"))`. Expected: volume 2000, bbox [-5,-5,0]-[35,45,10]; dependencies `features == ["f-sk1"]`.
11. **Direction from an edge.** `CUBE10`, `LP(DIR({"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-5,10|l=10"}}, "2", "20"))`. The fingerprint's direction is +X, so the edge's direction is taken as +X. Expected: bbox [-5,-5,0]-[25,5,10].
12. **Too many instances.** `CUBE10`, `LP(DIR(X, "200", "20"), "second_direction": DIR(Y, "100", "20"))`. Expected: error `too_many_instances` rendered `This linear pattern would make 20000 copies, more than the limit of 10000. Lower Count.`; b-1 unchanged.
13. **Cancellation.** Build test 3's pattern through `Registry::build` with a `BuildContext` whose `CancelToken` is already canceled. Expected: `Err(Canceled)`.
14. **Performance.** `CUBE10` with `DIR(X, "100", "20")` builds in under 500 ms (desktop, release).
15. **Fractional count below 1.** `CUBE10`, `LP(DIR(X, "0.5", "20"))`. Expected: error `out_of_range`, field `direction.count`, rendered `Count must be at least 1, but it is 0.5. Use 1 or more.`, suggestion `1`; b-1 unchanged.
16. **A sketch on a copy's face follows the spacing.** The document of [sketch.md](sketch.md) test 27 (a face sketch on `f-box1/top@f-lp1:1` with `fallback_frame: body` and a hole at its point (5, 5)). Expected: with spacing `"60"`, the motion of `f-box1/top@f-lp1:1` in b-1 after f-lp1 is the translation (60, 0, 0) and the bore's axis passes through (65, 5, 0); with spacing `"80"`, the translation (80, 0, 0) and (85, 5, 0); every feature status ok.
17. **An edge direction that leans keeps its side (corpus R38).** Bodies `[b-1]`; `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": RECT(0, 0, 40, 20)}`; `{"id": "f-ext1", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=20,10|a=800"}]}, "extent": {"kind": "distance", "expr": "10"}, "op": "new"}`; `LP(DIR({"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-ext1/lateral/e-4|f-ext1/start-cap)", "fp": "line|n=0,1,0|c=0,10,0|l=20|adj=2,2"}}, "2", "30"))` (the left wall's bottom edge; copy at y 30..50). Edit: move the shared point of `e-3`'s end and `e-4`'s start from (0, 20) to (-1, 20) (the left wall leans 1 mm). Expected: ok; the edge resolves exactly and keeps the side of its fingerprint's direction: the copy is offset by (-1.498129, 29.962570, 0) (direction (-0.049938, 0.998752, 0)), bbox [-2.498129,0,0]-[40,49.962570,10]. Leaning to (1, 20) instead: offset (1.498129, 29.962570, 0), bbox [0,0,0]-[41.498129,49.962570,10]. The same with `"direction": [0, 1, 0]` stored on the axis.
18. **A 180 degree turn of the part turns the direction.** `CUBE10`; `{"id": "f-mv1", "type": "move_body", "body": "b-1", "rotation": {"axis": {"kind": "origin", "axis": "Z"}, "angle": "180 deg"}}`; `LP(DIR({"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-5,10|l=10"}, "direction": [1, 0, 0]}, "2", "20"))` (picked before the move was inserted). Expected: ok; the stored direction turns with the part to -X: bbox [-25,-5,0]-[5,5,10]. The same document without `direction` (as written before format version 2) keeps the fingerprint's side, +X: bbox [-5,-5,0]-[25,5,10].
19. **Direction edges not on one line.** b-1 holds the L-shaped body of [fillet.md](fillet.md) test 4; `LP(DIR({"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=-10,-10,10|l=20", "merged": {"f-box1/top": ["f-box2/top"]}}}, "2", "50"))`. Expected: error `not_collinear`, field `direction.axis`, rendered `The edge selected for Direction now matches 2 edges that do not lie on one line. Pick the one you mean.`, fixes `UseReferences` with each of `edge(f-box1/front|f-box1/top)` and `edge(f-box1/front|f-box2/top)`, then `Pick`; b-1 unchanged.
20. **Direction from a round face.** `CUBE10`; `{"id": "f-cyl1", "type": "cylinder", "body": "b-2", "radius": "2", "height": "10", "plane": {"kind": "origin", "plane": "YZ"}, "position": ["0", "20", "0"], "op": "new"}` (a rod along +X from (0,0,20) to (10,0,20)); `LP(DIR({"kind": "face", "body": "b-2", "face": {"q": "f-cyl1/side", "fp": "cylinder|n=1,0,0|c=5,0,20|a=125.663706|r=2|adj=2,3"}, "direction": [1, 0, 0]}, "3", "20"))` (as a pick stores it), as `examples/linear_pattern/face-direction`. The stored direction (body coordinates; b-2 has not moved) orients the axis, +X (features.md section 3.6). Expected: ok; b-1 identical to test 1 (volume 3000, 18 faces, 3 solids, bbox [-5,-5,0]-[45,5,10], `f-box1/right@f-lp1:2` at x=45); b-2 unchanged; dependencies `bodies_read == ["b-2"]`.
21. **Second direction reversed.** `CUBE10`, `LP(DIR(X, "3", "20"), "second_direction": {"axis": Y, "count": "2", "spacing": "15", "reverse": true})`, as `examples/linear_pattern/second-reverse`. Expected: ok; volume 6000, 36 faces, 6 solids, bbox [-5,-20,0]-[45,5,10] (test 3's grid with its second row toward -Y); `f-box1/top@f-lp1:1` planar at z=10 and centered at (20,0,10); `f-box1/top@f-lp1:0.1` centered at (0,-15,10); `f-box1/top@f-lp1:2.1` centered at (40,-15,10).
22. **Second direction from a sketch line.** `CUBE10`; `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [0, 1], "construction": true}], "constraints": []}`; `LP(DIR(X, "3", "20"), "second_direction": DIR({"kind": "sketch_line", "sketch": "f-sk1", "line": "e-1"}, "2", "15"))`, as `examples/linear_pattern/second-sketch-line-direction`. The line runs from its start to its end, +Y. Expected: ok; test 3's grid: volume 6000, 36 faces, 6 solids, bbox [-5,-5,0]-[45,20,10]; `f-box1/top@f-lp1:1` planar at z=10 and centered at (20,0,10), `f-box1/top@f-lp1:0.1` centered at (0,15,10) and `f-box1/top@f-lp1:2.1` centered at (40,15,10). Tests 22 to 24 are examples of delivery.md 2.8's coverage check (runner test 7): each kind of axis for the second direction.
23. **Second direction from an edge.** `CUBE10`; `LP(DIR(X, "3", "20"), "second_direction": DIR({"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-box1/left|f-box1/top)", "fp": "line|n=0,1,0|c=-5,0,10|l=10|adj=2,2"}, "direction": [0, 1, 0]}, "2", "15"))` (the cube's top left edge, as a pick stores it), as `examples/linear_pattern/second-edge-direction`. The stored direction orients the axis, +Y. Expected: as test 22.
24. **Second direction from a round face.** `CUBE10`; `{"id": "f-cyl1", "type": "cylinder", "body": "b-2", "radius": "2", "height": "10", "plane": {"kind": "origin", "plane": "XZ"}, "position": ["20", "20", "0"], "op": "new"}` (a rod along -Y from (20,0,20) to (20,-10,20)); `LP(DIR(X, "3", "20"), "second_direction": DIR({"kind": "face", "body": "b-2", "face": {"q": "f-cyl1/side", "fp": "cylinder|n=0,1,0|c=20,-5,20|a=125.663706|r=2|adj=2,3"}, "direction": [0, 1, 0]}, "2", "15"))` (as a pick stores it), as `examples/linear_pattern/second-face-direction`. The stored direction orients the axis, +Y, against the way the rod was made. Expected: ok; b-1 as test 22; b-2 unchanged: volume 125.663706, 3 faces, bbox [18,-10,18]-[22,0,22].

## Open issues

1. **The count in `too_many_instances`.** The message says `copies` for N, which counts the original as well (the study's text, features.md section 4.1). The difference is one in ten thousand, but a later wording could say `instances` to match Count's meaning.
