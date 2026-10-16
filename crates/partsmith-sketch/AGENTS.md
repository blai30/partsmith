# partsmith-sketch

Sketch params (plane, entities, constraints), the native solver (ADR 0004), DOF and redundancy diagnosis, drag solving, and profile regions with `RegionKey`.

Spec: `docs/spec/sketch.md`. Pure Rust, no kernel dependency. Depends on `partsmith-model` (`Id`, `Expr`, `LINEAR_TOLERANCE`) and `partsmith-naming` (`Reference`).

## Public interface

- Params: `SketchParams { plane, entities, constraints }` with `validate`, `entity`, `constraint`, `point` and `driving_dimensions` (each a `DimensionInput { constraint, expr, kind: ValueKind }`). Callers evaluate the expressions and pass `DimensionValues` (`BTreeMap<Id, f64>`, mm or rad).
- Planes: `SketchPlane::{Origin, Datum, Face}`, `OriginPlane::{Xy, Xz, Yz}` with `placement()`, and `Placement { origin, x_axis, normal }` with `y_axis`, `to_world`, `validate` and `on_plane`.
- Entities: `SketchEntity { id, geometry, construction }`, `Geometry::{Point, Line, Circle, Arc}` (with `arc_sweep`), `EntityKind`, and `PointRef { entity, role: PointRole }` parsed from `"e-1.start"` or `"e-7"`.
- Constraints: `SketchConstraint { id, kind: ConstraintKind }` with the 19 M1 types, `Alignment`, `DistanceTarget`, `DimensionExpr { expr, driving }`, plus `type_name`, `dimension`, `value_kind` and `referenced_entities`. `Serialize` is derived (the `type` tag and fields flattened into one record); the hand-written `Deserialize` keeps the accepted record shapes and rejects unknown fields.
- Reserved references `ORIGIN`, `X_AXIS`, `Y_AXIS` are constants in rows, never entities.
- Solver: the `Solver` trait with `NativeSolver::solve(sketch, values) -> SolveResult` and `begin_drag(sketch, values, target) -> NativeDragSession`; `DragSession::{initial, update}`; `DragTarget::{Point, Entity}`; `SolveResult { status, failed, entities, dof, redundant, conflicting, entity_states, measured }` with `issues() -> Vec<SolveIssue>`; `SketchStatus`, `EntityState`.
- Profiles: `build_profile(&[SketchEntity]) -> Profile { regions, findings }`; `Region` (`fingerprint`, `reference`), `ProfileLoop` (`signed_area`), `ProfileEdge`, `ProfileCurve`, `ProfileFinding`; `RegionKey`, `KeyEdge`, `RegionFingerprint`, `RegionRef`; `match_region(regions, reference) -> RegionMatch` with `MatchMethod`.
- Errors: `SketchError` (messages of spec section 4.1) and `PlacementProblem`.

## Layout

- `src/params.rs`, `plane.rs`, `entity.rs`, `point_ref.rs`, `constraint.rs`, `validation.rs`, `reserved.rs`, `error.rs`: the data model, its JSON and validation.
- `src/vector.rs` (vector helpers for points, placements and the solver's vectors) and `src/union_find.rs` (the one union-find, used by components, identification, substitution and profile chains).
- `src/solver/`: `system.rs` builds the rows of every constraint with their warm-start choices and splits components; `rows.rs` evaluates residuals with analytic gradients; `layout.rs` maps entities to the flat parameter vector and back; `substitution.rs` merges trivial equalities; `numeric.rs` runs dog-leg, the Levenberg-Marquardt fallback and a final Gauss-Newton refinement that keeps returned arcs within tolerance; `sparse_qr.rs` is the sequential rank-revealing Householder QR used by the steps and the diagnosis, with per-column plans worked out from sparsity patterns and reused across the factorizations of a solve and the frames of a drag; `diagnosis.rs`, `drag.rs`, `measure.rs`.
- `src/profile/`: `arrangement.rs` (the steps of `build_profile`: vertices, findings, holes and sorted regions), `intersection.rs` (curve shapes, their bounds and where two curves meet), `graph.rs` (pieces between vertices, overlap and bridge removal, face tracing; a circle with one vertex and an arc whose ends meet are closed pieces that take part in face tracing, then each closed edge is split off into a loop of its own), `area.rs` (exact moments and winding numbers), `key.rs` (region keys and fingerprints), `matching.rs`.
- `src/test_support.rs`: fixtures shared by the unit tests (ids, point references, entities, origin-plane sketches).
- `tests/common/`: builders and assertions (`mod.rs`), the plate, chain and grid sketches (`benchmark.rs`), the known-solution generator (`generator.rs`), the independent residual oracle (`residuals.rs`) and the plane vector helpers both oracles use (`plane.rs`).
- `benches/solver.rs`: the budgets of spec section 5, using `tests/common/benchmark.rs`.

## Commands

```sh
cargo test -p partsmith-sketch
cargo clippy -p partsmith-sketch --all-targets -- -D warnings
cargo fmt -p partsmith-sketch
cargo bench -p partsmith-sketch --bench solver
# nightly property run
PROPTEST_CASES=4096 cargo test -p partsmith-sketch --release --test properties
```

## Rules

- Pure Rust, no kernel dependency.
- Solver must meet the 4 ms drag budget on the 500-constraint chain; keep the benchmark green.
- Results are deterministic: iterate in document or index order only, never over hash maps, and do not parallelize inside a component.
