# 0004: Native Rust sketch solver instead of PlaneGCS

Status: Accepted, needs human review

## Context

The design proposed a fork of FreeCAD's PlaneGCS behind our own interface, noting that a native Rust port would be possible later. PlaneGCS is C++ (LGPL), depends on Eigen and on Boost graph headers, and would add a second C++ boundary to build for every target, including Emscripten.

## Decision

Write the sketch solver in Rust inside `partsmith-sketch`, behind a `Solver` interface:

- Decompose the constraint graph into connected components and solve each independently.
- Eliminate trivial equalities (coincident points, horizontal and vertical pairs) by parameter substitution before solving.
- Solve with a dog-leg trust region method, falling back to Levenberg-Marquardt, on the component Jacobian.
- Diagnose with a rank-revealing QR of the Jacobian: degrees of freedom, redundant constraints (dependent rows with zero residual) and conflicting constraints (dependent rows with nonzero residual).
- Drag by weakly pinning the dragged point to the cursor.

Correctness is checked with property-based tests (random constraint sets built from known solutions) and performance with a 500-constraint benchmark against the 4 ms budget.

Addendum (superseded by ADR 0015, which replaces `faer` with the crate's own sparse QR): linear algebra uses `faer` (MIT) for the sparse and dense factorizations, instead of `nalgebra`, because the drag and diagnosis budgets on a 500-constraint component need a sparse QR that `nalgebra` does not offer (sketch.md). The dependency is logged in ADR 0012. Benchmarks are bench targets with `harness = false` that time runs with `std::time::Instant`, so no benchmark framework such as `criterion` is added.

## Consequences

One C++ boundary (OCCT) instead of two, identical behavior on all targets, and no Boost or Eigen in the build. The risk is solver robustness on hard sketches; if the corpus shows gaps, PlaneGCS can still be wrapped behind the same `Solver` interface.
