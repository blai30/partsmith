# Feature: boolean

Status: M1 spec. Type name `boolean`. Shared contracts: [features.md](../features.md). Naming rule: [naming.md](../naming.md) section 3.6.9. Kernel operations: [kernel.md](../kernel.md) section 3.4.5.

## 1. Purpose and scope

Boolean combines a target body with one or more tool bodies: join (union), cut (difference) or intersect (common volume). Tools are consumed unless kept.

### In M1

- One target, one or more tools, op `join`, `cut` or `intersect`.
- Join adds every tool; cut removes the union of the tools; intersect keeps what lies inside the target and every tool.
- `keep_tools` to leave the tool bodies unchanged.

### Later

- Per-tool ops in one feature.
- Booleans with mesh bodies.
- Booleans that put the result into a new body (use a copy feature with op `new` plus a boolean in M1).

## 2. Public API

```rust
/// Joins, cuts or intersects a target body with tool bodies.
pub struct Boolean;

impl Feature for Boolean {
    const TYPE: &'static str = "boolean";
    type Params = BooleanParams;
}

/// Params of a boolean feature.
pub struct BooleanParams {
    /// The body that receives the result.
    pub target: Id,
    /// Tool bodies, at least one, distinct, not the target.
    pub tools: Vec<Id>,
    /// The operation. Required.
    pub op: BooleanOp,
    /// Leave the tools unchanged instead of consuming them. Default false, omitted when false.
    pub keep_tools: bool,
}

/// The operation of a boolean feature.
pub enum BooleanOp {
    /// Target ∪ every tool (kernel `fuse`).
    Join,
    /// Target minus the union of the tools (kernel `cut`).
    Cut,
    /// Target ∩ every tool (kernel `common`).
    Intersect,
}
```

JSON:

```json
{"id": "f-bool1", "type": "boolean", "target": "b-1", "tools": ["b-2", "b-3"], "op": "cut", "keep_tools": true}
```

### Manifest

Label `Boolean`, category `modify`, icon `boolean`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `target` | Target body | yes | | `body` | |
| `tools` | Tool bodies | yes | | `body` | |
| `op` | Operation | yes | | | `join` Join, `cut` Cut, `intersect` Intersect |
| `keep_tools` | Keep tools | yes | | | |

### Python

Dedicated form: `boolean(tools, op="cut", keep_tools=False)` (python-api.md section 2.7), produced by the `Feature::python` hook with the shared rules of features.md section 3.11: the `with <part>:` block gives `target`; `tools` are the tool bodies' part variables (one alone, several as a list); `op="<op>"` when it is not `cut` (the function's own default, not the static default op of solid features); `keep_tools=True` when true. With `b-2` and `b-3` as `part_2` and `part_3`, the JSON above is `boolean([part_2, part_3], keep_tools=True, id="f-bool1")`. Generic form (longer than 88 characters, so one argument per line):

```python
feature(
    "boolean",
    id="f-bool1",
    target="b-1",
    tools=["b-2", "b-3"],
    op="cut",
    keep_tools=True,
)
```

## 3. Behavior

### 3.1 Validation

- `tools` is not empty, else `EmptySelection { field: "tools", noun: body }`.
- No tool equals `target` and no tool repeats, else `SameBody { field: "tools[i]", body }` for the first offending entry.

### 3.2 Dependencies

- `bodies_written`: `[target]`, then the tools in order when `keep_tools` is false.
- `bodies_read`: the tools in order when `keep_tools` is true.
- `expressions`: none.

### 3.3 Build

1. The target must hold a solid, else `BodyEmpty { field: target, body: target, use_new: false }` (recompute's diagnosis adds why it is empty, features.md section 4.1).
2. Each tool, in order, must hold a solid, else `ToolEmpty { field: tools[i], body }`, with the same diagnosis.
3. `apply_op(ctx, target, op, tools, None)` (features.md section 2.7) with one `Tool` per tool body in `tools` order (its shape and naming map), which makes one kernel call with the target and the tool shapes: `fuse`, `cut` or `common` (kernel.md section 3.4.5). The result naming follows the boolean rule over the target map and every tool map. The shared semantics of features.md section 3.5 apply: `EmptyResult` when nothing is left, `NoEffect` when the volume does not change (the target keeps its previous shape and naming), with `how` `Inside`, `Beside` (a boolean has no direction) or `Covers` (features.md section 3.15), and `ClosedVoid` when a join or cut closes a hollow. A join of separate bodies is what the user asked for, so it gives no `SeparateSolid`.
4. When `keep_tools` is false, each tool gets a `BodyUpdate` with `shape: None`, an empty naming map and change `Clear`, even when the op had no effect.
5. Updates are returned in `bodies_written` order.

### 3.4 Element names

A boolean creates no names. Faces from the target and from the tools keep their names through the kernel history (modified, split with `#n`, merged, or deleted). With `keep_tools`, a tool body and the target may hold faces with the same name; names are unique per body.

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| No tools | `EmptySelection` | `Select at least one body for Tool bodies.` |
| Tool is the target or repeats | `SameBody` | `Body 'Body 1' cannot be used twice here. Pick a different body for Tool bodies.` |
| Target empty | `BodyEmpty` | without a cause: `Body 'Body 1' has no solid at this point, so this boolean has nothing to work on. Pick a body that has a solid.`; with one, features.md section 4.1 (`Body 'Body 1' is empty here because 'Box 1', which makes it, is suppressed.`) |
| A tool empty | `ToolEmpty` | without a cause: `Body 'Body 2' is empty here, so it cannot be used as a tool.`; with one: `Body 'Body 2' is empty here because 'Box 2', which makes it, failed, so it cannot be used as a tool.` |
| Nothing left | `EmptyResult` | `This cut removes all of body 'Body 1'. Make it smaller, or change its position or direction.` (features.md section 4.1) |
| Unknown body | `UnknownBody` (reported by recompute) | `Tool bodies names a body that is not in this document. Pick a body for Tool bodies.` |
| Kernel failure | `Kernel` | `Partsmith's geometry kernel could not build this boolean. {hint}`, with the kernel's hint (kernel.md section 4.3) and its text in the detail |
| Volume unchanged | warning `NoEffect` | features.md section 3.15, for example `This cut does not touch body 'Body 1': it lies beside it.` |
| A join or cut closes a hollow | warning `ClosedVoid` | features.md section 4.2 |

## 5. Performance

- Two 6-face boxes: 15 ms. Two 200-face bodies: 100 ms.

## 6. Acceptance tests

Conventions from features.md section 6.1. Bodies `[b-1, b-2, b-3]`. Fixtures:

- `BOX1`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"}` (spans [-10,-10,0]-[10,10,20]).
- `BOX2`: `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["20", "20", "20"], "position": ["10", "0", "0"], "op": "new"}` (spans [0,-10,0]-[20,10,20]).
- `BOX3`: `{"id": "f-box3", "type": "box", "body": "b-3", "size": ["10", "10", "10"], "position": ["0", "0", "15"], "op": "new"}` (spans [-5,-5,15]-[5,5,25]).
- `BOOL(tools, op)`: `{"id": "f-bool1", "type": "boolean", "target": "b-1", "tools": <tools>, "op": <op>}`.

1. **Join.** `BOX1`, `BOX2`, `BOOL(["b-2"], "join")`. Expected: ok; b-1 volume 12000, 6 faces, bbox [-10,-10,0]-[20,10,20], faces `f-box1/left` and `f-box2/right` exist; b-2 empty; output updates are `[b-1 Join, b-2 Clear]`.
2. **Join keeping tools.** As 1 with `"keep_tools": true`. Expected: b-1 volume 12000; b-2 volume 8000 unchanged; dependencies `bodies_read == ["b-2"]`, `bodies_written == ["b-1"]`; one update.
3. **Cut.** `BOX1`, `BOX2`, `BOOL(["b-2"], "cut")`. Expected: b-1 volume 4000, 6 faces, bbox [-10,-10,0]-[0,10,20]; the face at x=0 is named `f-box2/left`; b-2 empty.
4. **Intersect.** `BOX1`, `BOX2`, `BOOL(["b-2"], "intersect")`. Expected: b-1 volume 4000, 6 faces, bbox [0,-10,0]-[10,10,20].
5. **Cut with two tools.** `BOX1`, `BOX2`, `BOX3`, `BOOL(["b-2", "b-3"], "cut")`. Expected: b-1 volume 3750, 10 faces, bbox [-10,-10,0]-[0,10,20]; b-2 and b-3 empty.
6. **Intersect with two tools keeps what is inside every tool.** `BOX1`, `BOX2`, `BOX3`, `BOOL(["b-2", "b-3"], "intersect")`. Expected: b-1 volume 250, 6 faces, bbox [0,-5,15]-[5,5,20].
7. **Tool is the target.** `BOX1`, `BOOL(["b-1"], "join")`. Expected: error `same_body`, field `tools[0]`, rendered `Body 'Body 1' cannot be used twice here. Pick a different body for Tool bodies.`
8. **Repeated tool.** `BOX1`, `BOX2`, `BOOL(["b-2", "b-2"], "join")`. Expected: error `same_body`, field `tools[1]`.
9. **Empty tool.** `BOX1`, `BOOL(["b-2"], "join")`. Expected: error `tool_empty`, field `tools[0]`, rendered `Body 'Body 2' is empty here, so it cannot be used as a tool.` (no feature makes b-2, so the diagnosis gives no cause), fix `Pick { pointer: "/tools/0" }`; b-1 unchanged.
10. **Empty target.** `BOX2`, `BOOL(["b-2"], "join")`. Expected: error `body_empty` rendered `Body 'Body 1' has no solid at this point, so this boolean has nothing to work on. Pick a body that has a solid.`; b-2 unchanged.
11. **Disjoint join.** `BOX1`, `BOX2` with `"position": ["50", "0", "0"]`, `BOOL(["b-2"], "join")`. Expected: status ok (no `separate_solid`: joining separate bodies is what a boolean is asked to do); b-1 volume 16000, 12 faces, 2 solids, bbox [-10,-10,0]-[60,10,20].
12. **Cut that misses.** As 11 with op cut. Expected: warning `no_effect` with how `Beside`, rendered `This cut does not touch body 'Body 1': it lies beside it.`, with no fix; b-1 unchanged (volume 8000); b-2 empty.
13. **Cut everything.** `BOX1`, `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["30", "30", "30"], "position": ["0", "0", "-5"], "op": "new"}`, `BOOL(["b-2"], "cut")`. Expected: error `empty_result` rendered `This cut removes all of body 'Body 1'. Make it smaller, or change its position or direction.`; b-1 and b-2 unchanged.
14. **No tools.** `BOX1`, `BOOL([], "join")`. Expected: error `empty_selection` rendered `Select at least one body for Tool bodies.`
15. **Tool names survive.** As 3, then `{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{"q": "edge(f-box1/top|f-box2/left)", "fp": "line|n=0,1,0|c=0,0,20|l=20"}], "radius": "2"}`. Expected: fillet ok; b-1 volume 3982.831853, 7 faces.
16. **Performance.** Test 1 builds in under 15 ms (desktop, release).
17. **A tool whose feature is suppressed.** `BOX1`, `BOX2` with `"suppressed": true`, `BOOL(["b-2"], "join")`. Expected: error `tool_empty` rendered `Body 'Body 2' is empty here because 'Box 2', which makes it, is suppressed, so it cannot be used as a tool.`, fixes `Unsuppress { feature: "f-box2" }` then `Pick`; b-1 unchanged. Then (`Unsuppress`): as test 1.

## Open issues

1. Intersect with several tools keeps what lies inside every tool, as kernel.md section 3.4.5 defines `common`, while join and cut use the union of the tools. The inspector should say so for intersect.
