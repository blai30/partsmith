# Feature: sketch

Status: M1 spec. Type name `sketch`. Shared contracts: [features.md](../features.md). The params, solver, diagnosis and profiles are specified in [sketch.md](../sketch.md); this spec defines only the feature wrapper in `partsmith-features`: dependencies, plane placement, evaluation of dimensions, status mapping and output. Naming: [naming.md](../naming.md) section 3.6.1 (no naming map). Where a sketch on a face goes when the face moves, turns over or is lost follows [ADR 0023](../../adr/0023-frames-live-on-the-model.md) (frames live on the model).

## 1. Purpose and scope

The sketch feature places a 2D sketch in model space, evaluates its driving dimensions, solves it, and outputs the solved geometry and its closed regions for profile consumers (extrude, revolve) and point consumers (hole, axes).

### In M1

- Planes: origin plane, datum plane, flat face with a fallback placement in body or world coordinates (`fallback_frame`).
- A sketch on a face keeps its place on the face when the part moves, stays where its face was when a later feature removed it, and says so truthfully when it uses its saved position; a face that turned over keeps a right-handed placement with a warning and a one-click "Turn the drawing back".
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

`SketchParams`, `SketchPlane`, `HintFrame`, `Placement`, entity and constraint JSON are defined in sketch.md section 2. sketch.md's `SketchPlane::Face` carries the face's body next to naming's `Reference` (features.md section 2.5), and says which coordinates `fallback` is in:

```rust
/// A planar face of a body, plus the placement used when the face cannot be resolved and the coordinates it is in.
Face { body: Id, face: Reference, fallback: Placement, fallback_frame: HintFrame },
```

`fallback_frame` (file format version 2, [ADR 0025](../../adr/0025-file-format-version-2.md)) is omitted from JSON for `world`, the default. `world` means `fallback` is in world coordinates, as in every document written before format version 2; `body` means it is in the coordinates of the face's own feature, before any later Move body or copy transform (features.md section 3.6). Hosts write `body` with every new attach and every committed sketch drag (engine.md section 3.9, from the element descriptor's `fallback`), and the fixes `Attach`, `TurnDrawingBack` and the engine's "Keep the sketch where it was" write it too; nothing rewrites a fallback on its own, so a document opened, rebuilt and saved without edits is unchanged.

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

On a face: `"plane": {"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=100"}, "fallback": {"origin": [0, 0, 10], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}}`, and as attached since format version 2, with `"fallback_frame": "body"` after `fallback`.

### Manifest

Label `Sketch`, category `sketch`, icon `sketch`, version 1. The sketch editor edits entities and constraints; the inspector shows only the plane.

| Path | Label | Essential | Unit | Selection | Options |
| --- | --- | --- | --- | --- | --- |
| `plane` | Plane | yes | | `plane` | |
| `entities` | Entities | no | | | |
| `constraints` | Constraints | no | | | |
| `constraints[*].expr` | Dimension | no | | | |

### Python

Dedicated form: a `with Sketch(<plane>, fallback=..., id=...) as <var>:` block holding the entities, then the constraints (python-api.md sections 2.6 and 3.8 rule 6). The engine's script generator emits it, because a block is not a single call, so the sketch's `Feature::python` hook returns `None` (features.md section 3.11). With the plane written as in features.md section 3.11 (`Plane.XY`, a datum plane's variable, or `Ref("<q>", "<fp>", part=<part>)` with `fallback=Placement(...)` for a face, and `fallback_frame="body"` when stored, python-api.md section 2.8), the JSON above, followed by a feature that uses the sketch, is:

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

`Feature::validate` runs `SketchParams::validate()`; its first error becomes `SketchInvalid { error, values }` with empty `values`, rendered with the sketch crate's message.

### 3.2 Dependencies

- `features`: the datum feature for a `datum` plane.
- `bodies_read`: the face's body for a `face` plane. `bodies_written`: none.
- `expressions`: one use per dimensional constraint, driving or reference, in constraint order: field `constraints[i].expr` (i is the constraint's index in `constraints`), the constraint's expression, unit `Length` or `Angle` from the constraint's `value_kind()` (sketch.md). The build evaluates only the driving ones, so a broken reference dimension never fails the sketch, while parameter renames and validation still see it.
- `declared_ids`: every entity id, then every constraint id, in document order.

### 3.3 Build

1. **Placement and motion.**
   - `origin`: `OriginPlane::placement()`; motion the identity.
   - `datum`: the datum's `DatumResult::Plane { placement, motion }` (`ctx.datum("plane", feature)`; a non-datum gives `WrongFeatureType` with noun `datum plane` and field `plane`); the sketch takes both.
   - `face`: the first of these that applies ([ADR 0023](../../adr/0023-frames-live-on-the-model.md)), each a public helper a plugin would use (features.md section 3.6):
     1. **On the face.** The reference resolves to coplanar planar faces: `ctx.face_placement(body, face, &fallback, fallback_frame)` gives `Ok(Ok(FacePlacement { placement, turned_over, warnings }))`, and the sketch uses `placement`. Its `warnings` (a guess, a follow such as the continuation of corpus R34) go into the output. When `turned_over`, the face now faces the other way (a reversed extrude, corpus R32b): the placement stays right-handed, the drawing lies mirrored on the face (decision D3, recommended rule), and the output gets the warning `SketchTurnedOver` with the fix `TurnDrawingBack`.
     2. **Where the face was.** The reference is `Missing`: `ctx.face_placement_before_removal(body, face, &fallback, fallback_frame)`. When an earlier state of the body has the face, the sketch is placed on it as it was just before the writer `W` that removed it, moved with the body since, and a `PlaneFallback { cause: Missing, placed: BeforeRemoval { by: W }, loss }` warning carries the loss. This is the sketch's last resolved placement, read from the run itself, so nothing is written to the document and a document opened fresh gives the same placement: the sketch of corpus R33 stays at z = 40 and that of corpus R34 at z = 30.
     3. **The saved position.** Otherwise (the body is empty, the reference is ambiguous, the faces are not flat or not in one plane, or no earlier state has the face because its feature is not in the document, is suppressed or comes later): `fallback`, moved by `m` when `fallback_frame` is `body` (`m` the motion of the stored query's face, `BodyState::motion` in the state just before the sketch, applied as features.md section 3.6 moves a hint), with `PlaneFallback { cause, placed: Saved, loss }` and the `FaceFallback`'s cause and loss.
   - The motion of a face sketch, which the features that build faces from it pass to naming (features.md section 3.9), is `BodyState::motion` of the stored query in the body's state just before the sketch when `fallback_frame` is `body` (the identity for an empty body), and the identity for `world`, whose placement does not follow the face's motion (naming.md section 3.6).
2. **Values.** For each driving dimension (`SketchParams::driving_dimensions()`), `ctx.eval_length` or `ctx.eval_angle` at `constraints[i].expr`; the results form `DimensionValues` keyed by constraint id. An evaluation error is the feature's error (`Expression`, `UndeclaredDependency`). A length above the modeling limit is `OutOfRange` with `AtMost(MODEL_LIMIT)` on `constraints[i].expr`, and a value below `-MODEL_LIMIT` of a dimension that may be negative (`horizontal_distance`, `vertical_distance`) is `OutOfRange` with `AtLeast(-MODEL_LIMIT)` (features.md section 3.4); a value that is not positive where the solver needs a positive one is left to the solver's own bound (step 3), which comes first. Reference dimensions are not evaluated by the build.
3. **Solve.** `NativeSolver.solve(&params, &values)`:
   - `Err(error)` (for example a dimension value of 0 where a positive value is needed): `SketchInvalid { error, values }`.
   - `conflicting` not empty: `SketchConflict { constraints: conflicting, values }`.
   - otherwise `failed`: `SketchNotSolved`.
   - otherwise `redundant` not empty: the output gets a `RedundantConstraints` warning with the `SolveIssue::Redundant` items of `SolveResult::issues` and the values.
   Errors and warnings carry the dimension values so that their messages name dimensions by title and value ("Distance 30 mm", sketch.md section 4) when they are rendered from the cache (features.md section 4.2).
4. **Profile.** `build_profile(&result.entities)`. Profile findings (open ends, overlaps, self-intersections) are not warnings of the feature; they travel in the output for the sketch editor's profile check, which offers their fixes (`ConnectEnds` for two open ends a coincident constraint joins, sketch.md section 4.3).
5. **Output.** `OutputData::Sketch(SketchResult { placement, motion, solved: result, profile })`.

Under-constrained sketches are normal and give no warning.

### 3.4 Element names

A sketch has no naming map. Consumers address regions by `RegionRef` and points by `PointRef` (sketch.md).

### 3.5 A face that was lost or turned over

`FeatureWarning::PlaneFallback { field, body, reference, loss, cause, placed }` (features.md section 4.2) has the field `plane.face` (pointer `/plane/face`), the face's body, the stored reference, the loss of the `FaceFallback` (none for an empty body or a face that is not flat), its `cause` and where the sketch was `placed`. Messages, with `{element}` the description of the stored face (features.md section 3.12), `{reason}` the explanation of recompute's diagnosis for the loss (features.md section 4.2), `{after}` as in features.md section 4.2, and `{n}` the number of faces the reference now resolves to:

| `cause` | `placed` | Message |
| --- | --- | --- |
| `Missing` | `BeforeRemoval { by }` | `The face this sketch is on, {element}, {reason}, so the sketch stays where that face was.` |
| `Missing`, with a loss | `Saved` | `The face this sketch is on, {element}, {reason}, so the sketch uses the position saved with it.` |
| `Missing`, without a loss (the body is empty) | `Saved` | `The face this sketch is on, {element}, is gone because body '{body}' is empty here, so the sketch uses the position saved with it.` |
| `NotPlanar` | `Saved` | `The face this sketch is on, {element}, is no longer flat, so the sketch uses the position saved with it.` |
| `Ambiguous` | `Saved` | `The face this sketch is on, {element}, now matches {n} faces{after}, so the sketch uses the position saved with it. Pick the one you mean.` |
| `NotCoplanar` | `Saved` | `The face this sketch is on, {element}, now matches {n} faces that do not lie in one plane, so the sketch uses the position saved with it. Pick the one you mean.` |

So the message says where the sketch is, and never claims a position it does not have: "stays where that face was" only when the run placed it there, "the position saved with it" otherwise. When a feature removed the face, `{reason}` names it (`was cut away by 'Extrude 1', which comes before this sketch`); when the face's feature is not in the document, the description says so and `{reason}` is `is gone` (corpus R26).

Fixes, in the order of features.md section 4.4: `Attach { sketch: this sketch, body, face }` for each planar candidate of the loss (`face` its `Reference::from_element`), those whose candidate has `meaning` first, then the fixes of the loss's reason (`Unsuppress`, `Move`), then the other `Attach` fixes, then `Pick { pointer: "/plane/face" }` (no `Pick` when the explanation is `upstream`). Candidates come in naming's candidate order, so a face that faces the way the stored one did comes before nearer faces that do not (naming.md section 3.1, corpus R26). Applying `Attach` stores the sketch's current placement projected onto the face as the fallback, with `fallback_frame: body` (engine.md section 3.22). The engine adds "Keep the sketch where it was" to the cause line when the session saw the sketch placed elsewhere (engine.md section 3.21); recompute never moves a sketch to a face the user did not pick.

`FeatureWarning::SketchTurnedOver { field, body, reference }` (code `sketch_turned_over`, warning), field `plane.face`: `The face this sketch is on, {element}, now faces the other way, so the drawing is mirrored on it.` Fixes: `TurnDrawingBack { sketch: this sketch }`, then `Pick { pointer: "/plane/face" }`.

`TurnDrawingBack` ("Turn the drawing back") is one edit and one undo entry (engine.md section 3.22): the sketch's entities and constraints become `SketchParams::mirrored()` (sketch.md section 3.17), its `fallback` becomes the current placement in body coordinates (`m.inverse()` applied to it, `m` the sketch's motion) with `fallback_frame: body`, and every feature that uses the sketch stores its regions as `mirrored_region` gives them and its points as `mirrored_point` gives them. The drawing then lies on the face as it did before the face turned over, and the sketch has no warning (test 24).

## 4. Errors and warnings

| Condition | Variant | Example message |
| --- | --- | --- |
| Params fail sketch validation, or the solver rejects a value | `SketchInvalid` | the sketch crate's message (sketch.md section 4.1) |
| Constraints conflict | `SketchConflict` | the message of `SolveIssue::Conflicting` (sketch.md section 4.2) |
| A component does not converge | `SketchNotSolved` | the message of `SolveIssue::NotConverged` |
| Dimension expression error | `Expression` | `Dimension: Unknown name 'wdth'. Check the spelling, or add a parameter named wdth.` |
| Dimension beyond the modeling limit | `OutOfRange` | `Dimension must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less.` |
| Datum plane is not a datum | `WrongFeatureType` | `'Sketch 1' is not a datum plane. Pick a datum plane for Plane.` |
| Redundant constraints | warning `RedundantConstraints` | the messages of `SolveIssue::Redundant` |
| Face lost, ambiguous, not flat or not in one plane | warning `PlaneFallback` | section 3.5 |
| Face turned over | warning `SketchTurnedOver` | `The face this sketch is on, the top of 'Extrude 1', now faces the other way, so the drawing is mirrored on it.` |
| A face plane's reference guessed, followed or continued | warning `ReferenceGuessed`, notice `ReferenceFollowed` | features.md section 4.2 |

Dimension uses are labeled `Dimension` through the manifest hint `constraints[*].expr` (features.md section 3.2).

## 5. Performance

- Feature overhead excluding the solver and profile: 2 ms for a 500-constraint sketch. Solver and profile budgets are in sketch.md.

## 6. Acceptance tests

Conventions from features.md section 6.1. `SK-RECT` is the JSON in section 2 (id `f-sk1`). `LINE-DIM(expr)`: `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}], "constraints": [{"id": "c-1", "type": "fix", "target": "e-1.start"}, {"id": "c-2", "type": "horizontal", "line": "e-1"}, {"id": "c-3", "type": "distance", "line": "e-1", "expr": <expr>}]}`.

1. **Rectangle on XY.** `SK-RECT`. Expected: ok; output is `Sketch` with placement origin (0,0,0), x_axis (1,0,0), normal (0,0,1); `profile.regions` has one region with key `e-1,e-2,e-3,e-4`, area 4000 and centroid (40,25); no body changes.
2. **On XZ.** `SK-RECT` with plane `{"kind": "origin", "plane": "XZ"}`. Expected: placement normal (0,-1,0), x_axis (1,0,0); `SketchResult::to_world([10, 20])` is (10, 0, 20).
3. **On a datum.** `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "10"}`, `SK-RECT` with plane `{"kind": "datum", "feature": "f-dp1"}`. Expected: placement origin (0,0,10), normal (0,0,1); dependencies `features == ["f-dp1"]`.
4. **On a face.** Bodies `[b-1]`; `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`; `SK-RECT` with the face plane of section 2 but fallback origin `[3, 4, 0]`. Expected: placement origin (3,4,10) (the fallback origin projected onto the face's plane), x_axis (1,0,0), normal (0,0,1); no warning; dependencies `bodies_read == ["b-1"]`.
5. **Face lost.** As 4 with face `{"q": "f-box1/nothing", "fp": "plane|n=0,0,1|c=500,500,500|a=1"}`. Expected: status warning (`plane_fallback` with cause `missing`, placed `saved`, pointer `/plane/face`); placement equals the fallback exactly (origin (3,4,0)); the first fix is `Attach` to `f-box1/top`, the only candidate that faces the way the stored face did, followed by `Attach` to the other four candidates and `Pick`.
6. **Face not flat.** Bodies `[b-1]`; `{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "5", "height": "10", "op": "new"}`; `SK-RECT` with plane `{"kind": "face", "body": "b-1", "face": {"q": "f-cyl1/side", "fp": "cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5"}, "fallback": {"origin": [0, 0, 0], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}}`. Expected: warning `plane_fallback` with cause `not_planar`, placed `saved` and no loss, rendered `The face this sketch is on, the side of 'Cylinder 1', is no longer flat, so the sketch uses the position saved with it.`; its only fix is `Pick { pointer: "/plane/face" }`; placement equals the fallback.
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
19. **Empty body under a face plane.** Bodies `[b-1]`; `SK-RECT` with the face plane of test 4 and no feature writing b-1. Expected: status warning (`plane_fallback` with cause `missing`, placed `saved` and no loss) rendered `The face this sketch is on, a face of a feature that is not in this document, is gone because body 'Body 1' is empty here, so the sketch uses the position saved with it.`; its only fix is `Pick`; placement equals the fallback exactly; `motion` is the identity.

Tests 20 to 31 use these records. `FACE-SK2(face, fallback, frame, entities)` is `{"id": "f-sk2", "type": "sketch", "plane": {"kind": "face", "body": "b-1", "face": <face>, "fallback": <fallback>, "fallback_frame": <frame>}, "entities": <entities>}`, with `"fallback_frame"` left out where a test says `world`. `PL(ox, oy, oz)` is the fallback `{"origin": [ox, oy, oz], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}`. `RECT-11(x0, y0, x1, y1)` is four lines `e-11` (x0,y0)-(x1,y0), `e-12` (x1,y0)-(x1,y1), `e-13` (x1,y1)-(x0,y1), `e-14` (x0,y1)-(x0,y0), region key `e-11,e-12,e-13,e-14` (entity ids are unique in a document). `BLOCK` is `SK-RECT` and `{"id": "f-ext1", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]}, "extent": {"kind": "distance", "expr": "30"}, "op": "new"}` (x 0..80, y 0..50, z 0..30). Fixes are given as `Fix` values; the engine labels them (engine.md section 3.22). Bodies are `[b-1]`, and labels follow timeline order (features.md section 3.12).

20. **The part moves, the sketch moves with its face (corpus R32a).** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "op": "new"}` (x -20..20, y -20..20, z 0..10); `{"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["100", "0", "0"]}`; `FACE-SK2({"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=1600|adj=4,4"}, PL(0, 0, 10), "body", [{"id": "e-p1", "type": "point", "position": [15, 15]}])`; `{"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk2", "points": ["e-p1"], "diameter": "4", "extent": {"kind": "through_all"}}`. Expected: status ok for every feature; f-sk2 placement origin (100,0,10), x_axis (1,0,0), normal (0,0,1), `motion` the translation (100,0,0); `to_world([15, 15])` is (115,15,10); the axis of `f-hole1/bore/e-p1` passes through (115,15,0); b-1 volume 15874.336294. Rotation variant, f-mv1 `{"id": "f-mv1", "type": "move_body", "body": "b-1", "rotation": {"axis": {"kind": "origin", "axis": "Z"}, "angle": "90 deg"}}`: status ok; placement origin (0,0,10), x_axis (0,1,0), normal (0,0,1); `to_world([15, 15])` is (-15,15,10); the bore's axis passes through (-15,15,0); volume 15874.336294.
21. **A fallback in world coordinates rebuilds as before (format version 1).** Test 20's translation document with frame `world` (as every document written before format version 2). Expected: f-sk2 status ok, placement origin (0,0,10), x_axis (1,0,0), `motion` the identity: the point stays at world (15,15,10), off the moved box; f-hole1 status warning (`hole_misses_body`, `no_effect`), the first rendered `The hole at (15, 15) in 'Sketch 1' does not touch body 'Body 1'. Move the point onto the body.`; b-1 volume 16000; the document's canonical text is the same after the run as before it (nothing is written back, and no `fallback_frame` is added).
22. **The face was cut away by a feature inserted before the sketch (corpus R33).** In timeline order: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "40"], "op": "new"}`; `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "35"}`; `{"id": "f-sk3", "type": "sketch", "plane": {"kind": "datum", "feature": "f-dp1"}, "entities": RECT-11(-15, -15, 15, 15)}`; `{"id": "f-ext3", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk3", "regions": [{"key": "e-11,e-12,e-13,e-14", "fp": "c=0,0|a=900"}]}, "extent": {"kind": "distance", "expr": "10"}, "op": "cut"}` (removes z 35..40, so the box top is gone and `f-ext3/start-cap` is the new top at z = 35); `FACE-SK2({"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=400|adj=4,4"}, PL(0, 0, 10), world, [{"id": "e-5", "type": "circle", "center": [0, 0], "radius": 3}])` (attached when the box was 10 tall); `{"id": "f-ext2", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk2", "regions": [{"key": "e-5", "fp": "c=0,0|a=28.274334"}]}, "extent": {"kind": "distance", "expr": "5"}, "op": "join"}`. Labels: f-sk3 'Sketch 1', f-ext3 'Extrude 1', f-sk2 'Sketch 2', f-ext2 'Extrude 2'. Expected: f-sk2 placement origin (0,0,40), normal (0,0,1) (where the top was just before 'Extrude 1' removed it, never the fallback's z = 10); warning `plane_fallback` with cause `missing` and placed `before_removal` by `f-ext3`, rendered `The face this sketch is on, the top of 'Box 1', was cut away by 'Extrude 1', which comes before this sketch, so the sketch stays where that face was.`; its first fix is `Attach { sketch: "f-sk2", body: "b-1", face }` with `face` the picked `f-ext3/start-cap` (the floor of the cut made by 'Extrude 1', the only candidate that faces the way the stored face did), then `Attach` to the other planar candidates, then `Pick`; b-1 bounding box max z 45, so the boss is never inside the box; f-ext2 builds its boss at z 40..45, above the new top, and warns `separate_solid` with root `f-sk2`. The run writes nothing to the document, and running the serialized document again gives the same placement.
23. **The face's feature was deleted (corpus R26, amended).** `SK-RECT`; `{"id": "f-ext3", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]}, "extent": {"kind": "distance", "expr": "80"}, "op": "new"}`; `FACE-SK2({"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}, PL(0, 0, 30), world, [{"id": "e-5", "type": "circle", "center": [40, 25], "radius": 10}])`; `{"id": "f-ext2", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk2", "regions": [{"key": "e-5", "fp": "c=40,25|a=314.159265"}]}, "extent": {"kind": "distance", "expr": "10"}, "op": "join"}` (the BLOCK document after `f-ext1` was deleted and `f-ext3` added in its place; f-ext3 is 'Extrude 1', f-sk2 'Sketch 2'). Expected: f-sk2 placement equals the fallback exactly (origin (0,0,30)); warning `plane_fallback` with cause `missing` and placed `saved`, rendered `The face this sketch is on, a face of a feature that is not in this document, is gone, so the sketch uses the position saved with it.`; fixes `Attach` to `f-ext3/end-cap` (the top of 'Extrude 1'), then `Attach` to `f-ext3/lateral/e-1`, `f-ext3/lateral/e-3`, `f-ext3/start-cap` and `f-ext3/lateral/e-2` (the candidate order of naming.md test 133), then `Pick`; f-ext2 warning `no_effect` with root `f-sk2`, rendered `This extrude adds nothing to body 'Body 1' because 'Sketch 2' lost the face it was on. Fix 'Sketch 2' first.`, fix `Show { feature: "f-sk2" }`; b-1 volume 320000. Then (the first fix): f-sk2's plane is `f-ext3/end-cap` with fallback origin (0,0,80) and frame `body`; every feature status ok; the boss stands at z 80..90; b-1 is one solid, volume 323141.592654, bounding box max z 90.
24. **The face turned over (corpus R32b, decision D3).** `BLOCK` with `"reverse": true` on f-ext1 (the end cap is now the bottom at z = -30, facing -Z); `FACE-SK2({"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}, PL(0, 0, 30), world, [{"id": "e-p1", "type": "point", "position": [20, 10]}])`; `{"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk2", "points": ["e-p1"], "diameter": "5", "extent": {"kind": "distance", "expr": "10"}}`. Expected: f-sk2 placement origin (0,0,-30), x_axis (1,0,0), normal (0,0,-1), right-handed (y axis (0,-1,0)); warning `sketch_turned_over` rendered `The face this sketch is on, the top of 'Extrude 1', now faces the other way, so the drawing is mirrored on it.`, fixes `TurnDrawingBack { sketch: "f-sk2" }` then `Pick`; `to_world([20, 10])` is (20,-10,-30), off the face, so f-hole1 has status warning (`hole_misses_body`, `no_effect`) and b-1 volume stays 120000. Then (`TurnDrawingBack`, one edit): f-sk2's `e-p1` is at (20,-10) and its fallback is origin (0,0,-30), x_axis (1,0,0), normal (0,0,-1) with frame `body`; f-hole1's points are unchanged; every feature status ok; the bore's axis passes through world (20,10), drilled from z = -30 up to z = -20; b-1 volume 119803.650459. The same document with frame `body` and fallback `PL(0, 0, 30)` gives the same results before and after the fix.
25. **A face split into coplanar pieces keeps the placement.** `BLOCK`; `{"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "30"}`; `{"id": "f-sk3", "type": "sketch", "plane": {"kind": "datum", "feature": "f-dp1"}, "entities": RECT-11(30, -1, 50, 51)}`; `{"id": "f-ext3", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk3", "regions": [{"key": "e-11,e-12,e-13,e-14", "fp": "c=40,25|a=1040"}]}, "extent": {"kind": "distance", "expr": "5"}, "reverse": true, "op": "cut"}` (the slot of naming.md test 84, which splits the end cap into `f-ext1/end-cap#1` and `f-ext1/end-cap#2`); `FACE-SK2({"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}, PL(0, 0, 30), "body", [{"id": "e-p1", "type": "point", "position": [10, 10]}])`. Expected: f-sk2 status ok with no warning or notice; placement origin (0,0,30), x_axis (1,0,0), normal (0,0,1), as without the slot.
26. **Faces in two planes.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "20", "10"], "op": "new"}`; `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["20", "20", "10"], "position": ["10", "0", "10"], "op": "join"}` (the L-shaped body of [features/fillet.md](fillet.md) test 4, tops at z = 10 and z = 20); `SK-RECT` with plane `{"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=-10,0,10|a=400", "merged": {"f-box1/top": ["f-box2/top"]}}, "fallback": PL(0, 0, 10)}`. Expected: warning `plane_fallback` with cause `not_coplanar` and placed `saved`, rendered `The face this sketch is on, the top of 'Box 1', now matches 2 faces that do not lie in one plane, so the sketch uses the position saved with it. Pick the one you mean.`; fixes `Attach` to `f-box1/top`, `Attach` to `f-box2/top`, then `Pick`; placement equals the fallback exactly. Neither face is chosen by name order.
27. **A face of a pattern copy follows the spacing.** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "10"], "op": "new"}`; `{"id": "f-lp1", "type": "linear_pattern", "body": "b-1", "direction": {"axis": {"kind": "origin", "axis": "X"}, "count": "2", "spacing": "60"}}` (the copy spans x 50..70); `FACE-SK2({"q": "f-box1/top@f-lp1:1", "fp": "plane|n=0,0,1|c=60,0,10|a=400|adj=4,4"}, PL(0, 0, 10), "body", [{"id": "e-p1", "type": "point", "position": [5, 5]}])` (as a pick on the copy's top stores it: in the coordinates of 'Box 1', before the copy's transform); `{"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk2", "points": ["e-p1"], "diameter": "4", "extent": {"kind": "through_all"}}`. Expected: status ok; f-sk2 placement origin (60,0,10), `motion` the translation (60,0,0); the bore's axis passes through (65,5,0); b-1 volume 7874.336294. With f-lp1's spacing `"80"`: status ok; placement origin (80,0,10); the bore's axis passes through (85,5,0); volume 7874.336294.
28. **A shell inserted under the sketch keeps it in the plane (corpus R34).** `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["200", "200", "30"], "op": "new"}`; `{"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "2", "remove_faces": [{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,30|a=40000|adj=4,4"}]}` (inserted before the sketch: an open tray); `FACE-SK2({"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,30|a=40000|adj=4,4"}, PL(0, 0, 30), world, [{"id": "e-1", "type": "circle", "center": [0, 0], "radius": 10}])`; `{"id": "f-ext2", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk2", "regions": [{"key": "e-1", "fp": "c=0,0|a=314.159265"}]}, "extent": {"kind": "distance", "expr": "10"}, "op": "join"}`. Expected: f-sk2 resolves its face by continuation to the rim `f-sh1/rim/f-box1/top` in the plane z = 30, with no `plane_fallback`: placement origin (0,0,30), normal (0,0,1), notice `reference_followed` rendered `The top of 'Box 1' selected for Plane was removed by 'Shell 1', which left its rim in the same place, so the rim is used.`; the boss spans z 30..40 (b-1 bounding box max z 40), never on the tray floor at z = 2; it stands over the tray's opening, so f-ext2 warns `separate_solid`.
29. **A revolve that turns all the way round (corpus R57).** `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XZ"}, "entities": [{"id": "e-1", "type": "line", "start": [10, 0], "end": [20, 0]}, {"id": "e-2", "type": "line", "start": [20, 0], "end": [20, 10]}, {"id": "e-3", "type": "line", "start": [20, 10], "end": [10, 10]}, {"id": "e-4", "type": "line", "start": [10, 10], "end": [10, 0]}]}`; `{"id": "f-rev1", "type": "revolve", "body": "b-1", "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=15,5|a=100"}]}, "axis": {"kind": "origin", "axis": "Z"}, "extent": {"kind": "full"}, "op": "new"}` (the corpus edit from an angle of 270 deg); `FACE-SK2({"q": "f-rev1/start-cap", "fp": "plane|n=0,-1,0|c=15,0,5|a=100|adj=4,4"}, {"origin": [15, 0, 5], "x_axis": [1, 0, 0], "normal": [0, -1, 0]}, world, [{"id": "e-11", "type": "circle", "center": [0, 0], "radius": 2}])`; `{"id": "f-ext2", "type": "extrude", "body": "b-1", "profile": {"sketch": "f-sk2", "regions": [{"key": "e-11", "fp": "c=0,0|a=12.566371"}]}, "extent": {"kind": "distance", "expr": "3"}, "reverse": true, "op": "cut"}`. Expected: no state of b-1 has the start face ('Revolve 1' itself no longer makes it), so f-sk2 uses the saved position: placement origin (15,0,5), normal (0,-1,0), warning `plane_fallback` with cause `missing` and placed `saved`, rendered `The face this sketch is on, the front side of 'Revolve 1', is no longer made by 'Revolve 1', which turns all the way round, so the sketch uses the position saved with it.`; fixes `Attach` to `f-rev1/lateral/e-1` (the bottom of 'Revolve 1'), then `Attach` to `f-rev1/lateral/e-3` (the top), then `Pick`: the round faces come first among the candidates but cannot hold a sketch, so they give no `Attach`; f-ext2 status ok, its cut at (15,0,5). With the extent `{"kind": "angle", "expr": "270 deg"}` again: every feature status ok, with the same f-sk2 placement.
30. **Dimension beyond the modeling limit.** `LINE-DIM("1e200")`. Expected: error `out_of_range`, field `constraints[2].expr`, bound `AtMost(1000000)`, rendered `Dimension must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less.`; the solver is not run. `LINE-DIM("-1e200")`: error `sketch_invalid` with the sketch crate's `ValueNotPositive` (the dimension's own bound comes first).
31. **Values travel with sketch errors.** As test 9. Expected: the `sketch_conflict` error's `values` hold `c-3` = 100 and `c-4` = 50 (millimeters); its message, rendered as sketch.md section 4.2 gives it (the failed sketch has no result to read values from), starts with `In 'Sketch 1', Distance 50 mm cannot hold together with` and names `Distance 100 mm (width)`.

## Open issues

1. A conflicting sketch is an error, so every feature that uses it reports `upstream_failed`. Keeping the last good geometry with a warning would let downstream features build, but would make results depend on history; this spec keeps results a pure function of the document.
2. The `PlaneFallback` and `SketchTurnedOver` messages (section 3.5) are defined here because only the sketch feature falls back or turns over; features.md section 4.2 lists the variants with their `cause` and `placed` fields.
3. **A face that turned over (decision D3).** This spec follows the recommended rule of [ADR 0023](../../adr/0023-frames-live-on-the-model.md): the placement stays right-handed, so the drawing lies mirrored and the sketch warns until the user turns it back (test 24). The alternative, a mirrored placement that keeps the drawing in place, would need a handedness rule in sketch.md, features.md (profiles, extrude and revolve directions, hole drilling) and kernel.md.
4. **No automatic re-attach.** When the face's feature was deleted and another feature makes a face in the same place (test 23), recompute cannot tell that one replaces the other, so the sketch uses its saved position and offers the face as the first `Attach`. The engine's "Keep the sketch where it was" (engine.md section 3.21) covers the case where the session saw the sketch elsewhere.
