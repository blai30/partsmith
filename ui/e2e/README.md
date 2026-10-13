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
- The web server is `pnpm build && pnpm preview --port 4173 --strictPort`: the production build, with the core copied from `public/core` to `dist/core`, served with the cross-origin isolation headers of `public/_headers` (set again in `vite.config.ts` for the preview), so `crossOriginIsolated` is true as in production. `smoke.spec.ts` checks that the core reaches `data-core-status="ready"` and that the first frame is marked (`ps:usable`).
- Outside CI, a server already listening on port 4173 is reused instead of building. To skip the build on repeated runs, start one yourself with `pnpm -C ui build && pnpm -C ui preview --port 4173 --strictPort`, and rebuild after changing `src/` or the core: the preview serves whatever `dist/` holds. Parallel runs that each start the server rebuild `dist/` under each other, so share one server instead.
- The project is Desktop Chrome: 1280 by 720 at device scale factor 1. Headless Chromium renders WebGL2 with SwiftShader.
- `viewport.spec.ts` tests the render backend on its own page (`pages/viewport.html`), served by a Vite dev server that each worker starts.
- The page requests `/favicon.ico`, which the build does not have, so Chromium logs one 404 console error that tests collecting console errors need to ignore.

## Tags

- `@visual` (ui.md 6.6): screenshot tests in `visual.spec.ts`, with their baselines in `visual.spec.ts-snapshots/`. The configuration sets `animations: 'disabled'`, `maxDiffPixelRatio: 0.002` and `threshold: 0.05` for `toHaveScreenshot`, and `visual.css` hides the rebuild time while a screenshot is taken. They need Google Fonts to be reachable. Baselines change only with reviewer approval; regenerate them with `--update-snapshots=all`, since the default mode keeps every baseline that still passes the comparison.
- `@perf` (ui.md 6.7): left out unless `PARTSMITH_PERF=1`.

## Fixtures

`fixtures/` holds the documents of ui.md 6.2. Each `.ptsm` is canonical and valid: `partsmith fmt --check` leaves it unchanged, `partsmith validate` is clean, and the core saves it byte for byte. Check a changed fixture with the CLI:

```sh
cargo run -p partsmith-cli -- fmt --check ui/e2e/fixtures/*.ptsm
cargo run -p partsmith-cli -- validate ui/e2e/fixtures/*.ptsm
cargo run -p partsmith-cli -- rebuild --checks ui/e2e/fixtures/cube.ptsm
```

References to faces and edges (`cube-features.ptsm`, `fillet-error.ptsm`, `hole-cube.ptsm`) hold the names and fingerprints the core reports for the cube. `part-50.ptsm` is a copy of `examples/bench/fifty-features/model.ptsm`; keep the two identical. `dense.ptsm` (a 15 by 15 pattern of spheres, about 2.4 million display triangles) takes about 25 seconds to open.

## Results

A failed test keeps its trace in `ui/test-results/`; open it with `pnpm -C ui exec playwright show-trace <path to trace.zip>`. CI also writes an HTML report to `ui/playwright-report/`.
