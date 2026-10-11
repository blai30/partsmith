# 0003: Pin OCCT 7.9.3, built from source as static libraries

Status: Accepted

## Context

The design calls for OCCT 7.8 or later, pinned, because kernel upgrades change output topology. Distribution packages differ by platform (Ubuntu 24.04 ships 7.6), and the web build needs OCCT compiled with Emscripten anyway.

## Decision

Pin OCCT 7.9.3 (`scripts/occt-version.sh`) and build it from source with `scripts/build-occt.sh` for every target: native static libraries with position-independent code, and Emscripten static libraries with native wasm exceptions (`-fwasm-exceptions`). Only the modeling, mesh, STEP and STL toolkits are built; CMake adds their dependencies. Release builds keep OCCT's exception checks (`BUILD_RELEASE_DISABLE_EXCEPTIONS=OFF`) so bad input fails loudly instead of corrupting memory. `partsmith-occt-sys` finds the install through the `OCCT_ROOT` environment variable. CI caches each install keyed by version and script hash.

OCCT 8.0 is released; moving to it is a separate decision gated by a full corpus run.

## Consequences

Native and web builds run the identical kernel version, so corpus baselines agree across platforms. First builds take tens of minutes; caches make later builds fast.
