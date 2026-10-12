# partsmith-sketch

Sketch params (plane, entities, constraints), the native solver (ADR 0004), DOF and redundancy diagnosis, drag solving, and profile regions with `RegionKey`.

Spec: `docs/spec/sketch.md`. Pure Rust, no kernel dependency. Depends on `partsmith-model` (`Id`, `Expr`, `LINEAR_TOLERANCE`) and `partsmith-naming` (`Reference`).

## Public interface

- Params: `SketchParams { plane, entities, constraints }` with `validate`, `entity`, `constraint`, `point` and `driving_dimensions` (each a `DimensionInput { constraint, expr, kind: ValueKind }`). Callers evaluate the expressions and pass `DimensionValues` (`BTreeMap<Id, f64>`, mm or rad).
- Planes: `SketchPlane::{Origin, Datum, Face}`, `OriginPlane::{Xy, Xz, Yz}` with `placement()`, and `Placement { origin, x_axis, normal }` with `y_axis`, `to_world`, `validate` and `on_plane`.
- Entities: `SketchEntity { id, geometry, construction }`, `Geometry::{Point, Line, Circle, Arc}` (with `arc_sweep`), `EntityKind`, and `PointRef { entity, role: PointRole }` parsed from `"e-1.start"` or `"e-7"`.
- Constraints: `SketchConstraint { id, kind: ConstraintKind }` with the 19 M1 types, `Alignment`, `DistanceTarget`, `DimensionExpr { expr, driving }`, plus `type_name`, `dimension`, `value_kind` and `referenced_entities`. Hand-written serde keeps the flat JSON records and rejects unknown fields.
- Reserved references `ORIGIN`, `X_AXIS`, `Y_AXIS` are constants in rows, never entities.
- Solver: the `Solver` trait with `NativeSolver::solve(sketch, values) -> SolveResult` and `begin_drag(sketch, values, target) -> NativeDragSession`; `DragSession::{initial, update}`; `DragTarget::{Point, Entity}`; `SolveResult { status, failed, entities, dof, redundant, conflicting, entity_states, measured }` with `issues() -> Vec<SolveIssue>`; `SketchStatus`, `EntityState`.
- Profiles: `build_profile(&[SketchEntity]) -> Profile { regions, findings }`; `Region` (`fingerprint`, `reference`), `ProfileLoop` (`signed_area`), `ProfileEdge`, `ProfileCurve`, `ProfileFinding`; `RegionKey`, `KeyEdge`, `RegionFingerprint`, `RegionRef`; `match_region(regions, reference) -> RegionMatch` with `MatchMethod`.
- Errors: `SketchError` (messages of spec section 4.1) and `PlacementProblem`.

## Layout

- `src/params.rs`, `plane.rs`, `entity.rs`, `point_ref.rs`, `constraint.rs`, `validation.rs`, `reserved.rs`, `error.rs`: the data model, its JSON and validation.
- `src/solver/`: `system.rs` builds the rows of every constraint with their warm-start choices and splits components; `rows.rs` evaluates residuals with analytic gradients; `layout.rs` maps entities to the flat parameter vector and back; `substitution.rs` merges trivial equalities; `numeric.rs` runs dog-leg, the Levenberg-Marquardt fallback and a final Gauss-Newton refinement; `sparse_qr.rs` is the sequential rank-revealing Householder QR used by the steps and the diagnosis; `diagnosis.rs`, `drag.rs`, `measure.rs`.
- `src/profile/`: `arrangement.rs` (intersections, vertices, edges, bridges, faces, findings), `area.rs` (exact moments and winding numbers), `key.rs` (region keys and fingerprints), `matching.rs`.
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
