# End-to-end tests

The Playwright tests in this folder run against the production build of the app with the real WebAssembly core in `ui/public/core` ([ui.md](../../docs/spec/ui.md) section 6, the `e2e` job of [delivery.md](../../docs/spec/delivery.md)).

## Run locally

1. Build the web core into `ui/public/core`, once and again after the Rust core changes. It needs an activated Emscripten SDK of the version in `scripts/emsdk-version.sh` and OCCT built with `scripts/build-occt.sh wasm <prefix>`. The script checks `OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN` in the shell, so export it even when `~/.cargo/config.toml` sets it for cargo.

   ```sh
   source /opt/emsdk/emsdk_env.sh
   export OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN=/opt/occt/7.9.3/wasm
   scripts/build-wasm.sh
   ```

2. Run the tests from the repository root, all of them or the files you name. `PLAYWRIGHT_CHROMIUM_EXECUTABLE` points Playwright at a preinstalled Chromium; without it, install Playwright's own first with `pnpm -C ui exec playwright install chromium`.

   ```sh
   pnpm -C ui install
   PLAYWRIGHT_CHROMIUM_EXECUTABLE=/opt/pw-browsers/chromium pnpm -C ui e2e
   PLAYWRIGHT_CHROMIUM_EXECUTABLE=/opt/pw-browsers/chromium pnpm -C ui e2e smoke.spec.ts viewport.spec.ts
   ```

## What the configuration does

- `playwright.config.ts` stops with a message when `ui/public/core` lacks `partsmith_core.mjs` or `partsmith_core.wasm`.
- The web server is `pnpm build && pnpm preview --port 4173 --strictPort`: the production build, with the core copied from `public/core` to `dist/core`, served with the cross-origin isolation headers of `public/_headers` (set again in `vite.config.ts` for the preview), so `crossOriginIsolated` is true as in production. `smoke.spec.ts` checks that the core reaches `data-core-status="ready"` and that the first frame is marked (`ps:usable`), and checks the helpers that only the performance tests use (ui.md 6.3).
- Outside CI, a server already listening on port 4173 is reused instead of building. To skip the build on repeated runs, start one yourself with `pnpm -C ui build && pnpm -C ui preview --port 4173 --strictPort`, and rebuild after changing `src/` or the core: the preview serves whatever `dist/` holds. Parallel runs that each start the server rebuild `dist/` under each other, so share one server instead.
- The default project, `chromium`, is Desktop Chrome: 1280 by 720 at device scale factor 1. The `firefox` and `webkit` projects of the engine subset (`@engines`, below) are Desktop Firefox and Desktop Safari at the same size and scale. Chromium and WebKit run headless, and headless Chromium renders WebGL2 with SwiftShader. Firefox runs headed, since headless it gave the canvas no WebGL2 context on the runner (ui.md 6.8), so it needs a display.
- `viewport.spec.ts` tests the render backend on its own page (`pages/viewport.html`), served by a Vite dev server that each worker starts (`serveTestPage` in `support/test-pages.ts`). `perf.spec.ts` serves the worker transfer page of P-12 (`pages/transfer.html`) the same way.
- The page requests `/favicon.ico`, which the build does not have, so Chromium logs one 404 console error that tests collecting console errors need to ignore.

## Tags

- `@visual` (ui.md 6.6): screenshot tests in `visual.spec.ts`, with their baselines in `visual.spec.ts-snapshots/`, left out unless `PARTSMITH_VISUAL=1`. A baseline matches only screenshots from the machine and Chromium build that took it, so they run on the fixed hardware runner (delivery.md 1.2), not in CI's `e2e` job. The baselines here were taken with `/opt/pw-browsers/chromium` (Chromium 141) in the development container and are not approved yet; run them there with `PARTSMITH_VISUAL=1 PLAYWRIGHT_CHROMIUM_EXECUTABLE=/opt/pw-browsers/chromium pnpm -C ui e2e visual.spec.ts`. The configuration sets `animations: 'disabled'`, `maxDiffPixelRatio: 0.002` and `threshold: 0.05` for `toHaveScreenshot`, and `visual.css` hides the rebuild time while a screenshot is taken. They need no network: the fonts are files of the app (ui.md 2.16.7), and each screenshot waits until Geist and JetBrains Mono have loaded. Baselines change only with reviewer approval; regenerate them with `--update-snapshots=all`, since the default mode keeps every baseline that still passes the comparison.
- `@engines` (ui.md 6.8): the 24 tests of the engine subset, which run as written in every engine. The `firefox` and `webkit` projects run only them, and only when `--project` names them (`pnpm -C ui e2e --project webkit`); `pnpm -C ui e2e` runs `chromium`, which runs every test. The `firefox` project runs headed (ui.md 6.8), so on a machine without a display it runs on a virtual one, as the platforms workflow runs it: `xvfb-run --auto-servernum pnpm -C ui e2e --project firefox`. Only Chromium is installed in the development container, so a local check is `PLAYWRIGHT_CHROMIUM_EXECUTABLE=/opt/pw-browsers/chromium pnpm -C ui e2e --project chromium --grep @engines`, and E-157 lists what Firefox and WebKit would run without launching either. A test joins or leaves the subset only with a change to ui.md 6.8, which E-157 checks.
- `@perf` (ui.md 6.7): the tests in `perf.spec.ts`, left out unless `PARTSMITH_PERF=1`. Until the fixed hardware runner exists they are run by hand on the reference machine before a release, with `PARTSMITH_PERF=1 pnpm -C ui e2e perf.spec.ts`. They run one after another in one worker, and each adds a `perf` annotation per measured value (`P-4 ps:rebuild: 1840.25 ms (budget 3000 ms)`) before it checks the budget, so the HTML or JSON report shows the numbers of a run that passes or fails.

## Helpers

`helpers.ts` holds the shared helpers of ui.md 6.3 (opening the app and fixtures, the canvas geometry, the mapping from millimeters to page pixels, hover polling, downloads, performance measures, percentiles and long tasks, and `useFolders` with its companions, which put the File System Access pickers over the page's origin private file system for the tests of files and assets) and the locators every area uses. `support/` holds the helpers of one area each: documents, modeling, print and export, and sketch, and `test-pages.ts` serves the test pages of `pages/`.

## Fixtures

`fixtures/` holds the documents of ui.md 6.2, and `fixtures.spec.ts` (E-108) checks that each opens with its stated result. Each `.ptsm` is canonical and valid: `partsmith fmt --check` leaves it unchanged, `partsmith validate` is clean, and the core saves it byte for byte. Check a changed fixture with the CLI:

```sh
cargo run -p partsmith-cli -- fmt --check ui/e2e/fixtures/*.ptsm
cargo run -p partsmith-cli -- validate ui/e2e/fixtures/*.ptsm
cargo run -p partsmith-cli -- rebuild --checks ui/e2e/fixtures/cube.ptsm
```

References to faces and edges (`cube-features.ptsm`, `fillet-error.ptsm`, `hole-cube.ptsm`) hold the names and fingerprints the core reports for the cube. The fixtures of the reference stability corpus (`bevel-corner.ptsm`, `rabbet.ptsm`, `pattern-fillet.ptsm`, `boss-fillet.ptsm`, `side-line.ptsm`) store every reference as a pick stores it: the full reference `pick_reference` gives with each feature focused, and each profile region as `solve_sketch` gives it, with its outline (ui.md 6.2). Their scenarios are in `repairs.spec.ts` (E-109, E-122 to E-126). The fixtures of [the newcomer error corpus](../../docs/design/newcomer-error-corpus.md), `nc01.ptsm` to `nc12.ptsm` and `nc08-broken.ptsm`, store their references the same way; their scenarios are in `newcomer-errors.spec.ts` (E-110 to E-121). `nc07.ptsm`, `nc08-broken.ptsm` and `nc10.ptsm` hold the problem their scenario states, so `partsmith validate` reports it, and `nc09.ptsm` keeps its four numbers without a unit. `part-50.ptsm` is a copy of `examples/bench/fifty-features/model.ptsm`; keep the two identical. `step-block.ptsm` is a copy of `examples/import/step-block.ptsm`, kept identical too; its STEP asset stays in `examples/import/assets/`, so `partsmith validate` reports `asset_missing` for it, and `assets.spec.ts` (E-150 to E-154) writes the asset next to it in the origin private file system. `part-300.ptsm`, the input of P-9, is a stepped tower of 60 groups (a sketch on XY, an extrude of it 2 mm higher than the one before, a 0.5 mm fillet on two of its vertical edges, a face sketch and a datum plane on its top), every reference picked from the core as the UI picks it; it rebuilds with every feature ok. `dense.ptsm` (a 15 by 15 pattern of spheres, about 2.4 million display triangles) takes about 25 seconds to open.

## Results

A failed test keeps its trace in `ui/test-results/`; open it with `pnpm -C ui exec playwright show-trace <path to trace.zip>`. CI also writes an HTML report to `ui/playwright-report/`.
