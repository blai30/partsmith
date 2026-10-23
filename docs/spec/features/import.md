# Feature: import

Status: M1 spec. Type name `import`. Shared contracts: [features.md](../features.md). Naming rule: [naming.md](../naming.md) section 3.6.14 (fallback names). Kernel operation: [kernel.md](../kernel.md) section 3.8.1 (`read_step`). Assets: [file-format.md](../file-format.md) (the `assets/` folder) and [engine.md](../engine.md) section 3.14 (the `import_step` command that adds this feature and its body).

## 1. Purpose and scope

Import puts the solids of a STEP file into a body. The file is stored next to the document as an asset named by the hash of its bytes, so the feature always rebuilds from exactly the same bytes.

### In M1

- STEP assets, read by the kernel facade in millimeters.
- Every solid of the file goes into one body; disjoint solids become lumps of that body.
- Ops `new`, `join`, `cut`, `intersect` with the shared body semantics.
- Faces named by the fallback rule and confirmed by fingerprint.

### Later

- Mesh import (STL, 3MF, OBJ).
- One body per STEP part.
- Relinking a changed file: replacing the asset while keeping references to its faces.
- Semantic face names for imported geometry.
- A placement transform at import (use `move_body` in M1).

## 2. Public API

```rust
/// Puts the solids of a STEP asset into a body.
pub struct Import;

impl Feature for Import {
    const TYPE: &'static str = "import";
    type Params = ImportParams;
}

/// Params of an import feature.
pub struct ImportParams {
    /// Target body.
    pub body: Id,
    /// The STEP asset: the lowercase hex BLAKE3 hash of the file's bytes followed by ".step" (engine.md section 3.14).
    pub asset: String,
    /// How the imported solids combine with the body. Required.
    pub op: BodyOp,
}
```

JSON (canonical key order):

```json
{"id": "f-imp1", "type": "import", "body": "b-1", "asset": "7c3e9a1f0b2d4c6e8a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f.step", "op": "new"}
```

### Manifest

Label `Import`, category `create`, icon `import`, version 1.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `asset` | File | yes | | | |
| `op` | Operation | yes | | | `new` New body, `join` Join, `cut` Cut, `intersect` Intersect |
| `body` | Body | no | | `body` | |

### Python

No dedicated form: the hook returns `None`, because a script cannot carry the asset's bytes (python-api.md's `Document.import_step` adds the feature from a file instead). Generic form of the JSON above (features.md section 3.11; longer than 88 characters, so one argument per line):

```python
feature(
    "import",
    id="f-imp1",
    body="b-1",
    asset="7c3e9a1f0b2d4c6e8a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f.step",
    op="new",
)
```

## 3. Behavior

### 3.1 Validation

- `asset` matches `^[0-9a-f]{64}\.step$`, else `InvalidParams` with detail `'asset' must name a file in the assets folder, such as <64 hex digits>.step`.

### 3.2 Dependencies

- `bodies_written`: `[body]`. `bodies_read`, `features`: none.
- `assets`: `[AssetLink { name: asset, field: "asset" }]`.
- `expressions`: none.

### 3.3 Build

1. bytes = `ctx.asset("asset", &asset)`. Recompute has already checked that the host supplied the asset; a missing one fails before the build with `AssetMissing` (recompute.md section 3.3).
2. Kernel `read_step(bytes)`. A `KernelError` becomes `Kernel { error }`, rendered `Partsmith's geometry kernel could not build this import. {hint}` with the kernel's hint (`The file is not a valid STEP file.` or `The STEP file contains no solid bodies.`, kernel.md section 4.3) and its technical text in the detail. Each string of `StepImport::warnings`, in order, becomes an `ImportWarning { message }` warning.
3. The tool shape is the single solid when the file has one, else kernel `compound` of all solids (`StepImport::bodies[i].shape`) in read order. Product names and colors are not used by the feature; the engine uses them for the new body's record (engine.md section 3.14).
4. Every face of the tool is `New(<id>/face)` (naming.md section 3.6.14). Several faces are numbered `#1`, `#2`, ... in naming.md's geometric order (section 3.4: centroid x, then y, then z, then area); a single face keeps the name `<id>/face`.
5. `apply_op(ctx, body, op, vec![tool], None)` (an import has no direction; a join that does not touch the body gives `SeparateSolid` with the fix `NewBody`, features.md section 3.5).

### 3.4 Element names

| Name | Face |
| --- | --- |
| `<id>/face#<n>` | Face n of the imported solids in naming.md's geometric order (`<id>/face` when there is only one face). |

Faces of the body keep their names through the boolean of a join, cut or intersect. Imported names depend on the geometric order, so references to them are confirmed by fingerprint.

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| `asset` is not a STEP asset name | `InvalidParams` | `File of this import cannot be read, so it was not built. Set File again.` (the parser text, `'asset' must name a file in the assets folder, such as <64 hex digits>.step`, in the detail) |
| The host did not supply the asset | `AssetMissing` (raised by recompute) | `The STEP file of this import is not in this document's assets folder. Copy the assets folder that came with the document next to it, or import the file again.` (the asset name in the detail) |
| The bytes are not STEP | `Kernel` | `Partsmith's geometry kernel could not build this import. The file is not a valid STEP file.` |
| The file has no solid | `Kernel` | `Partsmith's geometry kernel could not build this import. The STEP file contains no solid bodies.` |
| Body rules | `BodyNotEmpty`, `BodyEmpty`, `EmptyResult`, warnings `SeparateSolid`, `ClosedVoid` | features.md sections 3.5 and 4 |
| Skipped or repaired content | warning `ImportWarning` | the kernel's text, for example `Skipped 2 shapes that are not closed solids.` (test 10) |
| Volume unchanged | warning `NoEffect` | features.md section 3.15, for example `This import adds nothing to body 'Body 1': it lies entirely inside the body.` |

## 5. Performance

- Op new of a 6-face body, excluding kernel `read_step` (kernel.md section 5): 10 ms.
- With a boolean against a 200-face body: 60 ms plus `read_step`.

## 6. Acceptance tests

Conventions from features.md section 6.1, with the asset map given per test. Bodies `[b-1]`. Fixtures:

- `A`: the asset name made of 64 zeros followed by `.step`. The tests do not check that it is the hash of the bytes; the engine checks hashes when it accepts assets (engine.md section 3.14).
- `BYTES`: `write_step(&[StepBody { shape: make_box(&Plane::XY, 10.0, 20.0, 30.0)?.shape, name: "Block".into(), color: None }])` (kernel.md section 2.12), a box spanning [0,10]x[0,20]x[0,30].
- `TWO`: `write_step` of two bodies named `A` and `B`, each `make_box` of size 10, 10, 10: the first on `Plane::XY`, the second on `Plane::XY` with its origin moved to (50, 0, 0).
- `IMP(op)`: `{"id": "f-imp1", "type": "import", "body": "b-1", "asset": A, "op": op}`.
- `FACES`: the shape of `make_profile(&Plane::XY, ..)` of two squares with 10 mm sides, [50,60]x[0,10] and [70,80]x[0,10] (kernel.md section 2.5): a compound of two faces that bound no solid.
- `SKIPPED`: `write_step(&[StepBody { shape: compound(&[make_box(&Plane::XY, 10.0, 20.0, 30.0)?.shape, FACES])?.shape, name: "Block".into(), color: None }])`: the block of `BYTES` with the two faces beside it in the same part (kernel.md test 135).
- `S`: `b8d947e1aae9d107df35bea403414dc9ae3df630b84426c70e92770948bdfdb4.step`, the file of that name in `examples/import/assets/`: the 15,303 bytes `write_step` wrote for `BYTES` with OCCT 7.9.3, committed once and never rewritten (delivery.md section 2.8). The asset map `{S: <that file>}` pairs this name with the file's bytes.
- `IMP_S(op)`: `IMP(op)` with `"asset": S`.

The asset map is `{A: BYTES}` unless stated.

1. **Op new.** `IMP("new")`. Expected: ok; volume 6000, 6 faces named `f-imp1/face#1` to `f-imp1/face#6`, bbox [0,0,0]-[10,20,30]; `f-imp1/face#4` is the plane z=30 (by centroid the order is left, front, bottom, top, back, right); change `New`; dependencies `assets [(A, "asset")]`, `bodies_written [("b-1", "body")]`, `expressions []`.
2. **Join.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`, then `IMP("join")`. Expected: ok; volume 6750 (1000 + 6000 - 250 overlap), bbox [-5,-5,0]-[10,20,30]; change `Join`.
3. **Missing asset.** `IMP("new")` with an empty asset map. Expected: error `asset_missing`, pointer `/asset`, rendered `The STEP file of this import is not in this document's assets folder. Copy the assets folder that came with the document next to it, or import the file again.`, with `assets/0000000000000000000000000000000000000000000000000000000000000000.step` in its detail; b-1 empty. A second run on the same `Recompute` with the map `{A: BYTES}` builds the feature (`from_cache` false) and gives the result of test 1.
4. **Not a STEP file.** Asset map `{A: b"not a step file"}`. Expected: error `kernel` rendered `Partsmith's geometry kernel could not build this import. The file is not a valid STEP file.`, with the kernel's `detail()` (starting `read_step read_failed`) as its detail; b-1 empty.
5. **Invalid asset name.** `IMP("new")` with `"asset": "box.step"`. Expected: `validate_and_canonicalize` returns `InvalidParams` with field `asset` and detail `'asset' must name a file in the assets folder, such as <64 hex digits>.step`; recompute reports `invalid_params` rendered `File of this import cannot be read, so it was not built. Set File again.`
6. **Two solids.** Asset map `{A: TWO}`. Expected: ok; volume 2000, 12 faces, 2 solids, bbox [0,0,0]-[60,10,10].
7. **Cut.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "position": ["0", "5", "0"], "op": "new"}` (spanning [-20,-15,0]-[20,25,10], so the block's y range 0 to 20 lies inside it), then `IMP("cut")`. The imported block passes through the plate. Expected: ok; volume 14000, 10 faces, bbox [-20,-15,0]-[20,25,10]; the walls of the rectangular hole are `f-imp1/face#1` (x=0), `f-imp1/face#2` (y=0), `f-imp1/face#5` (y=20) and `f-imp1/face#6` (x=10); change `Cut`.
8. **Performance.** Test 1 builds in under 10 ms plus the kernel's `read_step` time (desktop, release).
9. **Intersect.** The example `import/intersect-step-block` with the asset map `{S: <that file>}`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}` (spanning [-5,-5,0]-[5,5,10]), then `IMP_S("intersect")`. Expected: ok; volume 250, the corner [0,5]x[0,5]x[0,10] that the box and the block share; 6 faces, bbox [0,0,0]-[5,5,10]; the faces at x=0 and y=0 are `f-imp1/face#1` and `f-imp1/face#2`, those at x=5, y=5 and z=10 are `f-box1/right`, `f-box1/back` and `f-box1/top`, and the bottom at z=0, where both bottoms lie, is `f-box1/bottom`, and the body's naming map aliases the block's bottom `f-imp1/face#3` to `[f-box1/bottom]` (a merge keeps the smallest name and records the other as an alias, naming.md section 3.3 step 2); change `Intersect`.
10. **Skipped content.** `IMP("new")` with the asset map `{A: SKIPPED}`. Expected: status warning (`import_warning`), rendered `Skipped 2 shapes that are not closed solids.` (the kernel's one warning for both faces, kernel.md section 3.8.1 read item 3), with no pointer, no detail and no fixes; b-1 holds the block alone, as in test 1: volume 6000, 6 faces named `f-imp1/face#1` to `f-imp1/face#6`, bbox [0,0,0]-[10,20,30].
11. **Examples of the committed block.** Tests 1, 2 and 7 also hold, with the values they state (no second run with `BYTES`), for the examples `import/step-block` (`IMP_S("new")`), `import/join-step-block` (the box of test 2, then `IMP_S("join")`) and `import/cut-step-block` (the plate of test 7, then `IMP_S("cut")`), each with the asset map `{S: <that file>}`; test 1's dependencies name `S` in place of `A`. The file name of `S` is the lowercase hex BLAKE3 hash of its bytes followed by `.step` (engine.md section 3.14; `blake3` is already a dependency of `partsmith-recompute`), so the engine accepts the asset and `partsmith rebuild` builds these examples from the bytes tests 9 and 11 use.

## Open issues

1. One import makes one body. A STEP assembly with several parts becomes one body with several lumps; one body per part is later.
2. Imported face names depend on the geometric order, so references to them rely on fingerprints, and importing a changed file (a new asset name) renumbers them. Relinking is later.
3. **Import warnings are the kernel's sentences.** `ImportWarning` shows each string of `StepImport::warnings` as it is (features.md section 4.2), so its words are those of kernel.md section 3.8.1 read item 3: they have no subject, count with a plural only (one skipped face reads `Skipped 1 shapes that are not closed solids.`), and name a solid by its position in the file (`Solid 2 is not valid after repair; booleans with it may fail.`), which docs/guidelines/errors.md keeps out of messages. They are also kernel text, which features.md test 14 keeps out of every message a per-feature spec renders in its tests, while features.md section 4.2 has `ImportWarning` render it, so test 10 checks its text against the lint half of test 14 only. Test 10 skips two faces, whose text is at least grammatical; no test pins the one-shape text or the repair warning. Two fixes are possible. (a) Reword only the kernel's two strings, for example `Skipped 1 shape that is not a closed solid.` and a repair warning without the solid's number: this changes kernel.md section 3.8.1 read item 3 and no public API, so it needs no human gate, but users still read text the kernel wrote. (b) `StepImport` returns counts (shapes skipped, solids still invalid after repair) and the feature words them, for example `This import left out 1 shape of the STEP file that is not a closed solid.`, with the kernel's text in the detail: this changes kernel.md sections 2.12 and 3.8.1 and features.md section 4.2 together, and as a public API change it waits for the review of the next gate (the M1 gap list's human gates). Recommended: (b), the only one that keeps kernel text out of messages. Either fix changes the kernel's `cpp/exchange.cpp` and `src/exchange.rs`, and (b) also the feature's rendering, all outside the import track, so the fix is left to a later track: item IMP-1 of the M1 gap list.
