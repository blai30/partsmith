# 0003: Pin OCCT 7.9.3, built from source as static libraries

Status: Accepted (owner, 2026-11-04), signed off as built with its amendment of 2026-10-17 (fillets and chamfers built one at a time)

## Context

The design calls for OCCT 7.8 or later, pinned, because kernel upgrades change output topology. Distribution packages differ by platform (Ubuntu 24.04 ships 7.6), and the web build needs OCCT compiled with Emscripten anyway.

## Decision

Pin OCCT 7.9.3 (`scripts/occt-version.sh`) and build it from source with `scripts/build-occt.sh` for every target: native static libraries with position-independent code, and Emscripten static libraries with native wasm exceptions (`-fwasm-exceptions`), optimized for size (`-Os`), and with `OCC_CONVERT_SIGNALS` undefined: that define wraps `setjmp` in `try` blocks, which Emscripten 4.0.15 compiles to invalid wasm (rejected by V8 in `ShapeUpgrade_ShapeDivide` and `ShapeCustom_BSplineRestriction`), and it only matters when OCCT signal handlers are installed, which the shim never does. Only the modeling, mesh, STEP and STL toolkits are built; CMake adds their dependencies. Release builds keep OCCT's exception checks (`BUILD_RELEASE_DISABLE_EXCEPTIONS=OFF`) so bad input fails loudly instead of corrupting memory. `partsmith-occt-sys` finds the install through the `OCCT_ROOT` environment variable. CI caches each install keyed by version and script hash.

OCCT 8.0 is released; moving to it is a separate decision gated by a full corpus run.

## Consequences

Native and web builds run the identical kernel version, so corpus baselines agree across platforms. First builds take tens of minutes; caches make later builds fast.

## Amendments

| Date | Specs | Change |
| --- | --- | --- |
| 2026-10-17 | kernel.md section 3.10 items 5 and 6, test 134, open issue 13 | **The pinned OCCT builds fillets and chamfers one at a time.** OCCT 7.9.3 is used unmodified, and its fillet and chamfer builder is not safe on several threads even on distinct shapes: `ChFi3d_Builder` assembles every result with `TopOpeBRepBuild`, whose file-level static `STATIC_SOLIDINDEX` (`TopOpeBRepBuild_Builder.cxx`) picks the solid an edge is classified against, so concurrent blends returned invalid shapes (about one in five with 8 threads; ThreadSanitizer reports the race). The shim builds `fillet` and `chamfer` under one mutex, as it already serializes STEP. This guards OCCT's state, not Partsmith's, and slows nothing today (single-threaded budgets, sequential recompute). The alternative, a patched OCCT that keeps the index as a member, would change what this decision pins and is left to kernel.md open issue 13. |
| 2026-11-15 | kernel.md section 3.8.1 read item 8, tests 176 and 188; delivery.md sections 2.1, 2.9, 2.13, 3.2 and 3.8, tests 55, 56 and 156 to 158 | **The pinned OCCT is built with a patch** ([ADR 0073](0073-a-check-of-step-bytes-and-a-patch-to-occt-7-9-3.md), Accepted (owner's rule of 2026-11-05), needs human review at the M2 gate). The pin stays the upstream tag `V7_9_3`. `scripts/build-occt.sh` applies `scripts/occt-7.9.3.patch` before the build, to its fresh clone of the tag or to an unchanged clone of the pinned commit. The patch makes two changes to OCCT's STEP reader, each for a crash that ended the process (delivery.md section 3.8 findings 5 and 6). An oriented edge whose edge element is an oriented edge gives a fail, which bounds the recursion of `StepShape_OrientedEdge::EdgeStart` and `EdgeEnd`. An edge curve whose ends are not vertex points gives a load fail, where OCCT read through a null. OCCT 7.9.3 is no longer used unmodified. |
| 2026-11-19 | delivery.md section 3.5 (Step 1 taken, and the plan's step 1) and open issue 2 | **The WebAssembly build's release flags are `-Oz`** ([ADR 0076](0076-occt-built-for-the-web-at-oz.md), Accepted (owner's rule of 2026-11-05), needs human review at the M2 gate). `scripts/build-occt.sh` compiles OCCT for Emscripten with `-Oz -DNDEBUG` in `CMAKE_CXX_FLAGS_RELEASE` and `CMAKE_C_FLAGS_RELEASE`, in place of `-Os -DNDEBUG`. The decision's "optimized for size (`-Os`)" now reads `-Oz`. Its other flags and the native build are unchanged. The core size spike of 2026-11-19 measured the change: a web core 4,266,769 bytes (21.3 percent) smaller, answers byte-identical in all 1,882 frames of the examples, and every kernel test passing under Node. |
