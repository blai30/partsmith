# Feature: sketch

Status: M1 spec. Type name `sketch`. Shared contracts: [features.md](../features.md). The params, solver, diagnosis and profiles are specified in [sketch.md](../sketch.md); this spec defines only the feature wrapper in `partsmith-features`: dependencies, plane placement, evaluation of dimensions, status mapping and output. Naming: [naming.md](../naming.md) section 3.6.1 (no naming map).

## 1. Purpose and scope

The sketch feature places a 2D sketch in model space, evaluates its driving dimensions, solves it, and outputs the solved geometry and its closed regions for profile consumers (extrude, revolve) and point consumers (hole, axes).

### In M1

- Planes: origin plane, datum plane, flat face with a fallback placement.
- Driving dimensions given as expressions, evaluated in the document scope.
- Solving with `NativeSolver`, status mapping to errors and warnings.
- Profile building from the solved, non-construction curves.

### Later

- Projected and reference geometry linked to bodies (adds body reads to the dependencies).
- Attachment options beyond plane and face (offset, angle, flip).
- Writing solved geometry back into the document (the engine commits drags as edits; recompute never writes).
- Text, images and imported DXF or SVG in sketches.

## 2. Public API

```rust
/// A 2D sketch placed on a plane.
pub struct SketchFeature;

impl Feature for SketchFeature {
    const TYPE: &'static str = "sketch";
    type Params = partsmith_sketch::SketchParams;
}
```

`SketchParams`, `SketchPlane`, `Placement`, entity and constraint JSON are defined in sketch.md section 2. sketch.md's `SketchPlane::Face` carries the face's body next to naming's `Reference` (features.md section 2.5):

```rust
/// A planar face of a body, plus the placement used when the face cannot be resolved.
Face { body: Id, face: Reference, fallback: Placement },
```

JSON:

```json
{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"},
 "entities": [
   {"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]},
   {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]},
   {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]},
   {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}
 ],
 "constraints": []}
```

On a face: `"plane": {"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=100"}, "fallback": {"origin": [0, 0, 10], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}}`.

### Manifest

Label `Sketch`, category `sketch`, icon `sketch`, version 1. The sketch editor edits entities and constraints; the inspector shows only the plane.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `plane` | Plane | yes | | `plane` | |
| `entities` | Entities | no | | | |
| `constraints` | Constraints | no | | | |
| `constraints[*].expr` | Dimension | no | | | |

### Python

Dedicated form: a `with Sketch(<plane>, fallback=..., id=...) as <var>:` block holding the entities, then the constraints (python-api.md sections 2.6 and 3.8 rule 6). The engine's script generator emits it, because a block is not a single call, so the sketch's `Feature::python` hook returns `None` (features.md section 3.11). With the plane written as in features.md section 3.11 (`Plane.XY`, a datum plane's variable, or `Ref("<q>", "<fp>", part=<part>)` with `fallback=Placement(...)` for a face), the JSON above, followed by a feature that uses the sketch, is:

```python
with Sketch(Plane.XY, id="f-sk1") as sketch_1:
    Line((0, 0), (80, 0), id="e-1")
    Line((80, 0), (80, 50), id="e-2")
    Line((80, 50), (0, 50), id="e-3")
    Line((0, 50), (0, 0), id="e-4")
```

The sketch and its entities get variables only when a later statement refers to them (python-api.md section 3.8 rule 7). Generic form of the JSON above (`Registry::python_generic`):

```python
feature(
    "sketch",
    id="f-sk1",
    plane={"kind": "origin", "plane": "XY"},
    entities=[{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}, {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]}, {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]}, {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}],
    constraints=[],
)
```

## 3. Behavior

### 3.1 Validation

`Feature::validate` runs `SketchParams::validate()`; its first error becomes `SketchInvalid { error }`, rendered with the sketch crate's message.

### 3.2 Dependencies

- `features`: the datum feature for a `datum` plane.
- `bodies_read`: the face's body for a `face` plane. `bodies_written`: none.
- `expressions`: one use per dimensional constraint, driving or reference, in constraint order: field `constraints[i].expr` (i is the constraint's index in `constraints`), the constraint's expression, unit `Length` or `Angle` from the constraint's `value_kind()` (sketch.md). The build evaluates only the driving ones, so a broken reference dimension never fails the sketch, while parameter renames and validation still see it.
- `declared_ids`: every entity id, then every constraint id, in document order.

### 3.3 Build

1. **Placement.**
   - `origin`: `OriginPlane::placement()`.
   - `datum`: the datum's placement (`ctx.datum("plane", feature)`; a non-datum gives `WrongFeatureType` with noun `datum plane` and field `plane`).
   - `face`: `ctx.resolve_all("plane.face", body, [face], Face)` semantics without failing: if the reference resolves and the first resolved face is planar, the placement is `Placement::on_plane(face centroid, outward normal, &fallback)` (sketch.md section 3.1). If the body is empty or the reference is missing (reason `missing`), ambiguous (`ambiguous`), or the first resolved face is not planar (`not_planar`), the placement is `fallback` itself and a `PlaneFallback` warning with that reason is added.
2. **Values.** For each driving dimension (`SketchParams::driving_dimensions()`), `ctx.eval_length` or `ctx.eval_angle` at `constraints[i].expr`; the results form `DimensionValues` keyed by constraint id. An evaluation error is the feature's error (`Expression`, `UndeclaredDependency`). Reference dimensions are not evaluated by the build.
3. **Solve.** `NativeSolver.solve(&params, &values)`:
   - `Err(error)` (for example a dimension value of 0 where a positive value is needed): `SketchInvalid { error }`.
   - `conflicting` not empty: `SketchConflict { constraints: conflicting }`.
   - otherwise `failed`: `SketchNotSolved`.
   - otherwise `redundant` not empty: the output gets a `RedundantConstraints` warning with the `SolveIssue::Redundant` items of `SolveResult::issues`.
4. **Profile.** `build_profile(&result.entities)`. Profile findings (open ends, overlaps, self-intersections) are not warnings of the feature; they travel in the output for the sketch editor's profile check.
5. **Output.** `OutputData::Sketch(SketchResult { placement, solved: result, profile })`.

Under-constrained sketches are normal and give no warning.

### 3.4 Element names

A sketch has no naming map. Consumers address regions by `RegionRef` and points by `PointRef` (sketch.md).

### 3.5 `PlaneFallback` reasons

`FeatureWarning::PlaneFallback` carries `reason: FallbackReason` (`missing`, `ambiguous` or `not_planar`):

| Reason | Message |
| --- | --- |
| `missing` | `The face this sketch is on can no longer be found, so the sketch stays at its last position. Re-attach it in the repair panel.` |
| `ambiguous` | `The face this sketch is on now matches several faces, so the sketch stays at its last position. Pick the face in the repair panel.` |
| `not_planar` | `The face this sketch is on is no longer flat, so the sketch stays at its last position. Attach it to a flat face.` |

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Params fail sketch validation, or the solver rejects a value | `SketchInvalid` | the sketch crate's message (sketch.md section 4.1) |
| Constraints conflict | `SketchConflict` | the message of `SolveIssue::Conflicting` (sketch.md section 4.2) |
| A component does not converge | `SketchNotSolved` | the message of `SolveIssue::NotConverged` |
| Dimension expression error | `Expression` | `Dimension: Unknown name 'wdth'. Check the spelling, or add a parameter named wdth.` |
| Datum plane is not a datum | `WrongFeatureType` | `'Sketch 1' is not a datum plane. Pick a datum plane for Plane.` |
| Redundant constraints | warning `RedundantConstraints` | the messages of `SolveIssue::Redundant` |
| Face lost, ambiguous or not flat | warning `PlaneFallback` | section 3.5 |

Dimension uses are labeled `Dimension` through the manifest hint `constraints[*].expr` (features.md section 3.2).

## 5. Performance

- Feature overhead excluding the solver and profile: 2 ms for a 500-constraint sketch. Solver and profile budgets are in sketch.md.

## 6. Acceptance tests

Conventions from features.md section 6.1. `SK-RECT` is the JSON in section 2 (id `f-sk1`). `LINE-DIM(expr)`: `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}], "constraints": [{"id": "c-1", "type": "fix", "target": "e-1.start"}, {"id": "c-2", "type": "horizontal", "line": "e-1"}, {"id": "c-3", "type": "distance", "line": "e-1", "expr": <expr>}]}`.

1. **Rectangle on XY.** `SK-RECT`. Expected: ok; output is `Sketch` with placement origin (0,0,0), x_axis (1,0,0), normal (0,0,1); `profile.regions` has one region with key `e-1,e-2,e-3,e-4`, area 4000 and centroid (40,25); no body changes.
2. **On XZ.** `SK-RECT` with plane `{"kind": "origin", "plane": "XZ"}`. Expected: placement normal (0,-1,0), x_axis (1,0,0); `SketchResult::to_world([10, 20])` is (10, 0, 20).
3. **On a datum.** `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "10"}`, `SK-RECT` with plane `{"kind": "datum", "feature": "f-dp1"}`. Expected: placement origin (0,0,10), normal (0,0,1); dependencies `features == ["f-dp1"]`.
4. **On a face.** Bodies `[b-1]`; `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`; `SK-RECT` with the face plane of section 2 but fallback origin `[3, 4, 0]`. Expected: placement origin (3,4,10) (the fallback origin projected onto the face's plane), x_axis (1,0,0), normal (0,0,1); no warning; dependencies `bodies_read == ["b-1"]`.
5. **Face lost.** As 4 with face `{"q": "f-box1/nothing", "fp": "plane|n=0,0,1|c=500,500,500|a=1"}`. Expected: status warning (`plane_fallback`, reason `missing`, pointer `/plane/face`); placement equals the fallback exactly (origin (3,4,0)).
6. **Face not flat.** Bodies `[b-1]`; `{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "5", "height": "10", "op": "new"}`; `SK-RECT` with plane `{"kind": "face", "body": "b-1", "face": {"q": "f-cyl1/side", "fp": "cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5"}, "fallback": {"origin": [0, 0, 0], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}}`. Expected: warning `plane_fallback` with reason `not_planar`; placement equals the fallback.
7. **Driving dimension from a parameter.** Parameters `[{"id": "p-w", "name": "width", "expr": "100 mm"}]`; `LINE-DIM("width")`. Expected: ok; `SketchResult::point("e-1.end")` is (100, 0) within 1e-9; dependencies `expressions == [("constraints[2].expr", "width", Length)]`.
8. **Bare dimension in inches.** Document `"units": "in"`; `LINE-DIM("2")`. Expected: `point("e-1.end")` is (50.8, 0).
9. **Conflict.** As 7 with a fourth constraint `{"id": "c-4", "type": "distance", "line": "e-1", "expr": "50 mm"}`. Expected: error `sketch_conflict` whose `constraints` include `c-4`; a following extrude whose profile names `f-sk1` reports `upstream_failed`.
10. **Redundant.** As 7 with a fourth constraint `{"id": "c-4", "type": "distance", "line": "e-1", "expr": "100 mm"}`. Expected: status warning (`redundant_constraints`); `point("e-1.end")` is (100, 0).
11. **Value rejected by the solver.** `LINE-DIM("-5")`. Expected: error `sketch_invalid` (the sketch crate's `ValueNotPositive`).
12. **Expression error.** `LINE-DIM("wdth")`. Expected: error `expression`, field `constraints[2].expr`, rendered `Dimension: Unknown name 'wdth'. Check the spelling, or add a parameter named wdth.`
13. **Datum that is a sketch.** `SK-RECT` (id f-sk1), then `{"id": "f-sk2", "type": "sketch", "plane": {"kind": "datum", "feature": "f-sk1"}}`. Expected: f-sk2 error `wrong_feature_type` rendered `'Sketch 1' is not a datum plane. Pick a datum plane for Plane.`
14. **Construction only.** A sketch with one construction line. Expected: ok, no warnings, `profile.regions` empty.
15. **Declared ids.** `dependencies` of test 9's sketch has `declared_ids == ["e-1", "c-1", "c-2", "c-3", "c-4"]`.
16. **Invalid params.** A sketch whose constraint references entity `e-9` that does not exist. Expected: `validate_and_canonicalize` returns `SketchInvalid` with the sketch crate's `UnknownEntity`; recompute reports `sketch_invalid`.
17. **Performance.** `SK-RECT` builds in under 2 ms excluding solve and profile (desktop, release).
18. **Reference dimension.** As 7 with a fourth constraint `{"id": "c-4", "type": "distance", "line": "e-1", "expr": "wdth", "driving": false}`. Expected: ok, no warnings; `point("e-1.end")` is (100, 0); dependencies `expressions == [("constraints[2].expr", "width", Length), ("constraints[3].expr", "wdth", Length)]`; `FeatureInspector::inspect` lists both expressions with pointers `/constraints/2/expr` and `/constraints/3/expr`.
19. **Empty body under a face plane.** Bodies `[b-1]`; `SK-RECT` with the face plane of test 4 and no feature writing b-1. Expected: status warning (`plane_fallback`, reason `missing`); placement equals the fallback exactly.

## Open issues

1. A conflicting sketch is an error, so every feature that uses it reports `upstream_failed`. Keeping the last good geometry with a warning would let downstream features build, but would make results depend on history; this spec keeps results a pure function of the document.
2. The `PlaneFallback` reasons and messages (section 3.5) are defined here because only the sketch feature falls back; features.md section 4.2 lists the variant with its `reason` field.
