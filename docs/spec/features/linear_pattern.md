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
    pub direction: PatternDirection,
    /// An optional second direction for a grid. Omitted when None.
    pub second_direction: Option<PatternDirection>,
    /// Where the copies go. Default join, omitted when join.
    pub op: CopyOp,
    /// The empty body that receives the copies. Required with op new, not allowed with op join.
    pub target: Option<Id>,
}

/// One direction of a linear pattern.
pub struct PatternDirection {
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

Dedicated form: `linear_pattern(direction, count, spacing, part=<body>)` (python-api.md section 2.7) for one direction with op join; other cases use the generic form:

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

1. The source body must hold a solid, else `BodyEmpty { use_new: false }`.
2. For each direction, in order: n = count ≥ 1 (`OutOfRange`, `AtLeast(1)`); s = spacing > 0 (`OutOfRange`, `GreaterThan(0)`); d = the resolved axis direction, negated when `reverse`.
3. Total instances N = n1 × n2 (n2 = 1 without a second direction). N > 10000 gives `TooManyInstances { count: N, limit: 10000 }`.
4. Instances are (i, j) for i in 0..n1 and j in 0..n2; (0, 0) is the original. Each other instance is the source translated by i·s1·d1 + j·s2·d2 (kernel `transform`, index-preserving), named with `source_map.with_instance(<id>, Instance { first: i, second: j })`. Instances are built in order of j, then i. The build checks `ctx.is_cancelled()` before each instance and returns `Cancelled` when set.
5. If there are no copies (N = 1), the body (or target) is unchanged and a `NoEffect` warning is added.
6. The copies are combined with one multi-argument kernel boolean: op join `apply_op(body, Join, copies)`; op new: the copies fused with each other into the empty `target` (`apply_op(target, New, copies)`). Copies that overlap each other or the source fuse into one solid; separate copies stay separate solids of the same body.

### 3.4 Element names

| Name | Face |
| --- | --- |
| `<face name>@<id>:<i>` | One direction: the copy of source face `<face name>` in instance i (1 ≤ i < n1), for example `f-box1/top@f-lp1:2`. |
| `<face name>@<id>:<i>.<j>` | Two directions, j ≥ 1: the copy in instance (i, j), for example `f-box1/top@f-lp1:2.1` and `f-box1/top@f-lp1:0.1`. Instances (i, 0) use the one-index form. |

Source faces keep their names. Instance numbers are pattern positions, not kernel indices, so a reference to instance 2 survives edits that do not change the count.

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Count < 1 | `OutOfRange` | `Count must be at least 1, but it is 0. Use 1 or more.` |
| Count not whole | `NotWholeNumber` | `Count must be a whole number, but it is 2.5. Use 2 or 3.` |
| Spacing ≤ 0 | `OutOfRange` | `Spacing must be greater than 0 mm, but it is 0 mm. Use a value greater than 0 mm.` |
| Too many instances | `TooManyInstances` | `This pattern would make 20000 instances; the limit is 10000. Use fewer instances.` |
| Axis errors | `NotStraight`, `NoAxis`, `UnknownEntity`, `NotALine`, `ReferenceNotFound` | features.md section 4.1 |
| Op, target and body errors | as [mirror.md](mirror.md) section 4 | |
| Cancelled during the build | `Cancelled` | `Recompute was cancelled before this feature finished.` |
| No copies, or copies inside the source | warning `NoEffect` | features.md section 4.2 |

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
5. **Count 1.** `CUBE10`, `LP(DIR(X, "1", "20"))`. Expected: warning `no_effect`; b-1 unchanged.
6. **Fractional count.** `CUBE10`, `LP(DIR(X, "2.5", "20"))`. Expected: error `not_whole_number`, field `direction.count`, rendered `Count must be a whole number, but it is 2.5. Use 2 or 3.`
7. **Zero count.** `CUBE10`, `LP(DIR(X, "0", "20"))`. Expected: error `out_of_range` rendered `Count must be at least 1, but it is 0. Use 1 or more.`
8. **Zero spacing.** `CUBE10`, `LP(DIR(X, "3", "0"))`. Expected: error `out_of_range`, field `direction.spacing`.
9. **New body.** `CUBE10`, `LP(DIR(X, "3", "20"), "op": "new", "target": "b-2")`. Expected: b-1 unchanged; b-2 volume 2000, 12 faces, bbox [15,-5,0]-[45,5,10], change `New`.
10. **Direction from a sketch line.** `CUBE10`; sketch `{"id": "f-sk1", "type": "sketch", "plane": {"type": "origin", "name": "XY"}, "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [3, 4], "construction": true}]}`; `LP(DIR({"kind": "sketch_line", "sketch": "f-sk1", "line": "e-1"}, "2", "50"))`. Expected: volume 2000, bbox [-5,-5,0]-[35,45,10]; dependencies `features == ["f-sk1"]`.
11. **Direction from an edge.** `CUBE10`, `LP(DIR({"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-5,10|l=10"}}, "2", "20"))`. The canonical direction is +X. Expected: bbox [-5,-5,0]-[25,5,10].
12. **Too many instances.** `CUBE10`, `LP(DIR(X, "200", "20"), "second_direction": DIR(Y, "100", "20"))`. Expected: error `too_many_instances` rendered `This pattern would make 20000 instances; the limit is 10000. Use fewer instances.`; b-1 unchanged.
13. **Cancellation.** Build test 3's pattern through `Registry::build` with a `BuildContext` whose `CancelToken` is already cancelled. Expected: `Err(Cancelled)`.
14. **Performance.** `CUBE10` with `DIR(X, "100", "20")` builds in under 500 ms (desktop, release).

## Open issues

None beyond features.md.
