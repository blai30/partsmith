# 0017: Keep the web build; the desktop app leads where the platforms differ

Status: Accepted, needs human review

## Context

The owner allowed dropping the web build if supporting it proves too much hassle, too hacky or not worth the effort, in favor of a better desktop app. The decision was taken on measurements from the current code on 2026-10-12:

- **Kernel on WebAssembly works.** With OCCT 7.9.3 built by `scripts/build-occt.sh wasm` (`-fwasm-exceptions`, `-UOCC_CONVERT_SIGNALS`, `-Os`, ADR 0003), all kernel tests pass under Node: 352 passed, and the 2 ignored are the threaded tests, which the web build does not run.
- **Size fits the 25 MiB per-file hosting limit.** Built with the `wasm-release` profile, the largest kernel test module (the crate's unit tests, which reach the whole facade) is 14.08 MiB, 13.5 MiB after `wasm-opt -Oz`, 4.9 MiB gzip and 3.6 MiB brotli. Other kernel test modules range from 3.4 to 13.8 MiB. The engine, features, recompute and protocol crates are pure Rust and add a few MiB, so the linked core is expected near 17 to 19 MiB. Splitting the core into lazily loaded modules (technical design section 4) is not needed.
- **The extra machinery is small and already built:** a C ABI bridge with three exports (ADR 0009, 0010), a worker transport and file service in the UI, and a cached OCCT wasm build in CI.
- **The web build pays for itself beyond reach:** zero-install trials, a preview URL for every pull request, and Playwright end-to-end tests against the real core in headless Chromium. Desktop end-to-end testing through `tauri-driver` is not available on macOS, so the web build is also the project's most portable UI test harness.

The real costs are the platform limits: one worker and no OCCT threads (web budgets are three times the desktop ones), the 4 GB wasm32 memory limit, the File System Access API only in Chromium (others download), and Python scripting, which needs Pyodide.

## Decision

Keep the web build, and let the desktop app lead where the platforms differ:

- The core, the protocol and the UI stay shared, and every M1 modeling feature works on both.
- Capabilities that need the native platform ship on desktop first without waiting for a web equivalent: embedded CPython and script features, sending to installed slicers, background analysis events, multithreaded recompute, native file dialogs and recent files by path.
- Python on the web (Pyodide) is not planned until there is demand; the web app can still show and export the generated Python code.
- No core design decision bends for the web. If one ever would, or if the linked core exceeds 25 MiB after `wasm-opt`, this decision is revisited rather than working around the limit.

## Consequences

The project keeps PR previews, a browser demo and portable end-to-end tests at the cost of maintaining the Emscripten toolchain pin, the OCCT wasm build in CI and the size gate (delivery.md). The `partsmith-wasm` pipeline reports the linked core's size, and delivery.md's size gate fails CI if any file exceeds 25 MiB.
