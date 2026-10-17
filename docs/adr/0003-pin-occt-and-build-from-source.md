# 0003: Pin OCCT 7.9.3, built from source as static libraries

Status: Accepted

## Context

The design calls for OCCT 7.8 or later, pinned, because kernel upgrades change output topology. Distribution packages differ by platform (Ubuntu 24.04 ships 7.6), and the web build needs OCCT compiled with Emscripten anyway.

## Decision

Pin OCCT 7.9.3 (`scripts/occt-version.sh`) and build it from source with `scripts/build-occt.sh` for every target: native static libraries with position-independent code, and Emscripten static libraries with native wasm exceptions (`-fwasm-exceptions`), optimized for size (`-Os`), and with `OCC_CONVERT_SIGNALS` undefined: that define wraps `setjmp` in `try` blocks, which Emscripten 4.0.15 compiles to invalid wasm (rejected by V8 in `ShapeUpgrade_ShapeDivide` and `ShapeCustom_BSplineRestriction`), and it only matters when OCCT signal handlers are installed, which the shim never does. Only the modeling, mesh, STEP and STL toolkits are built; CMake adds their dependencies. Release builds keep OCCT's exception checks (`BUILD_RELEASE_DISABLE_EXCEPTIONS=OFF`) so bad input fails loudly instead of corrupting memory. `partsmith-occt-sys` finds the install through the `OCCT_ROOT` environment variable. CI caches each install keyed by version and script hash.

OCCT 8.0 is released; moving to it is a separate decision gated by a full corpus run.

## Consequences

Native and web builds run the identical kernel version, so corpus baselines agree across platforms. First builds take tens of minutes; caches make later builds fast.

## Amendments (needs human review)

| Date | Specs | Change |
| --- | --- | --- |
| 2026-10-17 | kernel.md section 3.10 items 5 and 6, test 134, open issue 13 | **The pinned OCCT builds fillets and chamfers one at a time.** OCCT 7.9.3 is used unmodified, and its fillet and chamfer builder is not safe on several threads even on distinct shapes: `ChFi3d_Builder` assembles every result with `TopOpeBRepBuild`, whose file-level static `STATIC_SOLIDINDEX` (`TopOpeBRepBuild_Builder.cxx`) picks the solid an edge is classified against, so concurrent blends returned invalid shapes (about one in five with 8 threads; ThreadSanitizer reports the race). The shim builds `fillet` and `chamfer` under one mutex, as it already serializes STEP. This guards OCCT's state, not Partsmith's, and slows nothing today (single-threaded budgets, sequential recompute). The alternative, a patched OCCT that keeps the index as a member, would change what this decision pins and is left to kernel.md open issue 13. |
