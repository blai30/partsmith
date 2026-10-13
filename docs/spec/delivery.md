# Delivery specification

Scope: continuous integration (`.github/workflows/ci.yml`), the Cloudflare Pages deploy of the web app, the desktop release workflow (`.github/workflows/release.yml`), the distribution check `ui/scripts/check-dist.mjs`, the cross-origin isolation headers `ui/public/_headers`, and the examples corpus (its layout, expected results and runners). Read with [technical design](../design/technical-design.md) sections 4 (build and deployment), 16 (testing) and 17 (guardrails in CI, where humans decide), [architecture.md](../design/architecture.md) section "Testing", ADRs [0003](../adr/0003-pin-occt-and-build-from-source.md), [0006](../adr/0006-protocol-framing-and-schema.md), [0009](../adr/0009-emscripten-c-abi-bridge.md), [0012](../adr/0012-dependency-log-m1.md), [0013](../adr/0013-oxc-tooling-and-theme-stylesheet.md) and [0014](../adr/0014-desktop-host-and-dependencies.md), and the specs whose checks CI runs: [cli.md](cli.md) (`schema`, `rebuild --json`), [protocol.md](protocol.md) 2.9 and 2.10, [ui.md](ui.md) 2.2 and 6, [python-api.md](python-api.md) 3.9, [features.md](features.md) (examples) and [desktop.md](desktop.md).

This spec is the definition of done for delivery. The checks of section 6 are its acceptance tests; implementers do not weaken them.

## 1. Purpose and scope

Every change is built and tested on every platform the project ships before it merges, and every merge to `main` deploys the web app, so reviewers always try the current state (technical design section 4). Nothing reaches users without a green build, and desktop releases wait for a human (technical design section 17).

### 1.1 In this milestone

- CI jobs: OCCT native and WebAssembly builds with caches; Rust format, lint and tests (including the desktop crate and the examples runner); `cargo-deny`; kernel tests on WebAssembly; the WebAssembly core built with `scripts/build-wasm.sh`; UI lint, typecheck, unit tests and script tests; the web build with the distribution check; Playwright end-to-end tests against the real WebAssembly core; schema and generated-type drift; Python package tests including the examples' Python twins; a Linux desktop build.
- Cloudflare Pages: production on every push to `main`, a preview per pull request, through the Wrangler action, skipped cleanly when the secrets are absent (forks, unconfigured repositories).
- The 25 MiB per-file gate on `ui/dist`, with the measured sizes and the plan when the core outgrows it (3.5).
- The initial JavaScript limit of ui.md 5, 250 KB gzipped, checked on `ui/dist` by the same script (2.6).
- `ui/public/_headers` with the cross-origin isolation headers.
- The desktop release workflow: on a version tag, bundles on Linux, Windows and macOS with the Tauri bundler and a draft GitHub Release that a human publishes.
- The examples corpus: one rule for every file under `examples/`, the expected-results format, the Rust runner with a documented way to regenerate expected files, the feature coverage check and the Python twin checks (section 2.8).
- Every action pinned to an exact version.

### 1.2 Later (not in this milestone)

| Item | What it needs |
| --- | --- |
| Code signing (Windows, macOS) and notarization | Certificates and accounts, secrets (desktop.md 1.2), human decision on ownership. They plug into the `bundle` job's environment (3.6). |
| Auto-update | `tauri-plugin-updater`, the signing key secrets, `bundle.createUpdaterArtifacts` and a `latest.json` asset on the release (desktop.md 1.2). |
| Line and branch coverage with `cargo-llvm-cov` (technical design section 16, 90 percent target for core crates) | A per-crate threshold decision and a job; it doubles test time, so it waits until the core crates are complete. |
| The model corpus (500+ models), held-out corpus, naming metric and differential build123d tests (technical design sections 16 and 17) | The corpus itself, baseline storage and a build123d environment. |
| Performance regression gates and visual tests on fixed hardware | A self-hosted runner on the reference machine (ui.md 6.6 and 6.7). |
| `cargo-semver-checks`, fuzzing (`cargo fuzz`) | Public API baselines after the first release; a nightly toolchain job. |
| Serving oversized core files from Cloudflare R2 | See 3.5 step 3. |
| Desktop end-to-end tests | A `tauri-driver` harness (desktop.md 1.2). |
| Pinning actions to commit SHAs | Tags are mutable; SHA pins with a comment naming the version are stronger. Needs a decision and an update tool (Dependabot or Renovate). |

## 2. Public interface

### 2.1 Files

```text
.github/workflows/ci.yml          CI and the Pages deploy (2.4)
.github/workflows/release.yml     desktop release (2.5)
ui/scripts/check-dist.mjs         distribution check (2.6)
ui/scripts/check-dist.test.mjs    its unit tests (node:test)
ui/public/_headers                cross-origin isolation headers (2.7)
ui/package.json                   scripts "check:dist" and "test:scripts" (2.6)
crates/partsmith-cli/tests/examples.rs   examples runner, coverage and layout checks (2.8)
python/tests/test_examples.py     Python twin checks (2.8)
examples/**                       the corpus (2.8)
```

### 2.2 Pinned actions and tools

Every `uses:` names an exact release tag. Actions already in `ci.yml` keep their version; the three that name only a major (`actions/checkout@v7`, `actions/cache@v6`, `actions/cache/restore@v6`) are pinned to the current release of that major. `mymindstorm/setup-emsdk` publishes major tags only, so `v16` is its exact release.

| Action | Version | Used by |
| --- | --- | --- |
| `actions/checkout` | `v7.0.1` | every job |
| `actions/cache`, `actions/cache/restore` | `v6.1.0` | OCCT caches, `tauri-cli` cache |
| `actions/setup-node` | `v7.1.0` | jobs that run pnpm |
| `pnpm/action-setup` | `v6.1.0` | jobs that run pnpm |
| `Swatinem/rust-cache` | `v2.9.2` | jobs that run cargo |
| `EmbarkStudios/cargo-deny-action` | `v2.1.1` | `deny` |
| `mymindstorm/setup-emsdk` | `v16` | `occt-wasm`, `kernel-wasm`, `core-wasm` |
| `actions/upload-artifact` | `v7.0.2` | `core-wasm`, `web`, `e2e`, `bundle` |
| `actions/download-artifact` | `v8.0.2` | `web`, `e2e`, `deploy`, `draft` |
| `actions/setup-python` | `v7.0.0` | `python` |
| `cloudflare/wrangler-action` | `v4.1.3` | `deploy` |
| `ilammy/msvc-dev-cmd` | `v1.13.0` | `bundle` on Windows |

| Tool | Version | Where it is pinned |
| --- | --- | --- |
| Rust | 1.97.0 | `rust-toolchain.toml` |
| Emscripten | 4.0.15 | `scripts/emsdk-version.sh` (and the `setup-emsdk` inputs) |
| OCCT | 7.9.3 | `scripts/occt-version.sh` |
| Node | 22 | `node-version` inputs (as `ci.yml`) |
| pnpm | 10.28.0 | `ui/package.json` `packageManager` |
| Playwright Chromium | from `@playwright/test` 1.64.0 | `ui/pnpm-lock.yaml` |
| Python | 3.12 | `python-version` input |
| maturin | 1.15.0 | `pip install maturin==1.15.0` |
| pytest | 9.1.1 | `pip install pytest==9.1.1` |
| `tauri-cli` | 2.12.2 | `cargo install tauri-cli --version 2.12.2 --locked` |
| Wrangler | 4.149.0 | `wranglerVersion` input |

Runners: `ubuntu-24.04` for every CI job; `ubuntu-24.04`, `windows-2025` and `macos-15` for release bundles.

### 2.3 Secrets and variables

| Name | Kind | Used by | Purpose |
| --- | --- | --- | --- |
| `CLOUDFLARE_API_TOKEN` | repository secret | `deploy` | API token with only the permission "Account, Cloudflare Pages, Edit". |
| `CLOUDFLARE_ACCOUNT_ID` | repository secret | `deploy` | The Cloudflare account that owns the Pages project. |
| `CLOUDFLARE_PAGES_PROJECT` | repository variable, optional | `deploy` | Pages project name; `partsmith` when unset. |
| `GITHUB_TOKEN` | automatic | `verify`, `draft` | Reads CI runs (`actions: read`) and creates the draft release (`contents: write`). |

No other secret exists in this milestone. Signing and updater secrets are later (1.2).

### 2.4 CI workflow (`ci.yml`)

Triggers `push` to `main` and `pull_request`; `concurrency` `ci-${{ github.ref }}` with `cancel-in-progress: true`; workflow `permissions: contents: read`; `env` `CARGO_TERM_COLOR: always`, `OCCT_ROOT: ${{ github.workspace }}/.occt/native` and `CARGO_PROFILE_DEV_DEBUG: "0"` (debug builds and tests without debug info, 3.2). Jobs:

| Job | Needs | What it runs |
| --- | --- | --- |
| `occt-native` | | Restores or builds OCCT native into `.occt/native` (unchanged). |
| `occt-wasm` | | Restores or builds OCCT for WebAssembly into `.occt/wasm` (unchanged). |
| `kernel-wasm` | `occt-wasm` | `cargo test -p partsmith-occt-sys --target wasm32-unknown-emscripten` (unchanged). |
| `rust` | `occt-native` | Frees disk space and installs the Tauri Linux packages (3.2), then `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p partsmith-recompute --features stability-study --all-targets -- -D warnings` (the reference stability corpus, which fails by design until its study's plan is implemented, is compiled and linted but not run), `cargo test --workspace`, and prints the free disk space after the tests, also when they fail. The workspace includes `partsmith-desktop` and the examples runner. |
| `deny` | | `cargo-deny` with `deny.toml` (unchanged action; the policy now covers the Tauri tree, 3.2). |
| `ui` | | In `ui/`: `pnpm install --frozen-lockfile`, `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm test:scripts`. |
| `schema` | `occt-native` | `cargo run -p partsmith-cli -- schema --write`, `pnpm -C ui install --frozen-lockfile`, `pnpm -C ui generate:protocol`, then the drift check of 3.4. |
| `core-wasm` | `occt-native`, `occt-wasm` | Restores both OCCT caches, sets up Emscripten 4.0.15, runs `scripts/build-wasm.sh` with `OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN=$GITHUB_WORKSPACE/.occt/wasm`, then `cargo test -p partsmith-wasm --test web_module` (the Node round trip against the module just built) with `PARTSMITH_REQUIRE_WEB_MODULE=1`, so the test fails instead of skipping when Node or the module is missing, and uploads `ui/public/core/` as the artifact `core-wasm` (retention 7 days, `if-no-files-found: error`). |
| `web` | `core-wasm` | Downloads `core-wasm` into `ui/public/core/`, then in `ui/`: `pnpm install --frozen-lockfile`, `pnpm build`, `pnpm check:dist`, then checks that `dist/core/partsmith_core.wasm` and `dist/core/partsmith_core.mjs` exist (`check:dist` does not look for the core); uploads `ui/dist/` as the artifact `web-dist` (retention 7 days, `if-no-files-found: error`). |
| `e2e` | `core-wasm` | Downloads `core-wasm` into `ui/public/core/`, then in `ui/`: `pnpm install --frozen-lockfile`, `pnpm exec playwright install --with-deps chromium`, `pnpm e2e`. On failure uploads `ui/playwright-report/` and `ui/test-results/` as the artifact `playwright-report`. |
| `python` | `occt-native` | Python 3.12, a virtual environment, `pip install maturin==1.15.0 pytest==9.1.1`, `maturin develop --release` in `python/`, `pytest python/tests`. |
| `desktop` | `occt-native` | The Tauri Linux packages, `pnpm -C ui install --frozen-lockfile`, `pnpm -C ui build` (no WebAssembly core: the desktop app does not load it), `tauri-cli` from the cache or `cargo install`, `cargo tauri build --bundles deb` in `desktop/src-tauri`, then the package check of section 6, test 12. |
| `deploy` | all jobs above | The Pages deploy of 3.3, on `pull_request` and on pushes to `main`. |

Every job that needs OCCT restores its cache with `actions/cache/restore@v6.1.0` and `fail-on-cache-miss: true`, as `rust` and `kernel-wasm` do today. Every job that runs cargo uses `Swatinem/rust-cache@v2.9.2` after `rustup show active-toolchain || rustup toolchain install`. Every job that runs pnpm sets up pnpm with `package_json_file: ui/package.json` and Node 22 with the pnpm cache keyed on `ui/pnpm-lock.yaml`, as `ui` does today.

The `deploy` job, exactly in structure:

```yaml
deploy:
  name: Deploy to Cloudflare Pages
  needs: [kernel-wasm, rust, deny, ui, schema, web, e2e, python, desktop]
  if: github.event_name == 'pull_request' || github.ref == 'refs/heads/main'
  runs-on: ubuntu-24.04
  environment:
    name: ${{ github.event_name == 'pull_request' && 'preview' || 'production' }}
    url: ${{ steps.pages.outputs.pages-deployment-alias-url || steps.pages.outputs.deployment-url }}
  env:
    CLOUDFLARE_API_TOKEN: ${{ secrets.CLOUDFLARE_API_TOKEN }}
    CLOUDFLARE_ACCOUNT_ID: ${{ secrets.CLOUDFLARE_ACCOUNT_ID }}
  steps:
    - id: secrets
      run: |
        if [ -n "$CLOUDFLARE_API_TOKEN" ] && [ -n "$CLOUDFLARE_ACCOUNT_ID" ]; then
          echo "available=true" >> "$GITHUB_OUTPUT"
        else
          echo "available=false" >> "$GITHUB_OUTPUT"
          echo "::notice title=Deploy skipped::Cloudflare secrets are not available to this run (a fork, or the repository is not configured), so nothing was deployed."
        fi
    - if: steps.secrets.outputs.available == 'true'
      uses: actions/checkout@v7.0.1
    - if: steps.secrets.outputs.available == 'true'
      uses: actions/download-artifact@v8.0.2
      with:
        name: web-dist
        path: ui/dist
    - if: steps.secrets.outputs.available == 'true'
      id: pages
      uses: cloudflare/wrangler-action@v4.1.3
      with:
        apiToken: ${{ secrets.CLOUDFLARE_API_TOKEN }}
        accountId: ${{ secrets.CLOUDFLARE_ACCOUNT_ID }}
        wranglerVersion: "4.149.0"
        command: >-
          pages deploy ui/dist
          --project-name=${{ vars.CLOUDFLARE_PAGES_PROJECT || 'partsmith' }}
          --branch=${{ github.event_name == 'pull_request' && github.head_ref || 'main' }}
          --commit-hash=${{ github.event.pull_request.head.sha || github.sha }}
    - if: steps.secrets.outputs.available == 'true'
      run: echo "Deployed to ${{ steps.pages.outputs.pages-deployment-alias-url || steps.pages.outputs.deployment-url }}" >> "$GITHUB_STEP_SUMMARY"
```

### 2.5 Release workflow (`release.yml`)

Triggers: `push` of tags matching `v[0-9]+.[0-9]+.[0-9]+`, and `workflow_dispatch` (a dry run that builds the bundles without a release). Workflow `permissions: contents: read`; `concurrency` `release-${{ github.ref }}` with `cancel-in-progress: false`.

| Job | Needs | Runs on | What it runs |
| --- | --- | --- | --- |
| `verify` | | `ubuntu-24.04` | For a tag only (`permissions: contents: read, actions: read`), with `shell: bash`, whose `pipefail` makes a failing `cargo metadata` fail the step instead of reading as an empty version: the tag without `v` is compared separately with the version of `partsmith-desktop` (`cargo metadata --no-deps --format-version 1`) and with the version of `ui/package.json`, and each wrong version is reported once with the message of section 4 (a wrong version both files share gives one message); and `gh run list --workflow ci.yml --commit "$GITHUB_SHA" --status success --limit 1 --json databaseId --jq length` prints `1`. |
| `bundle` | `verify` | matrix of 3.6 | OCCT native from its cache or built, the UI build, `tauri-cli`, `cargo tauri build --bundles <matrix bundles>`, copies the bundle files into one flat `bundles/` folder, and uploads it as the artifact `bundle-<runner.os>` (3.6, step 6). `fail-fast: false`. |
| `draft` | `bundle` | `ubuntu-24.04` | For a tag only (`permissions: contents: write`): downloads every `bundle-*` artifact into `release/` (merged, so `release/` holds the flat bundle files), writes `release/SHA256SUMS` (`sha256sum` of each file, sorted by name), and runs `gh release create "$GITHUB_REF_NAME" --draft --verify-tag --title "Partsmith $VERSION" --generate-notes release/*`. |

### 2.6 Distribution check (`ui/scripts/check-dist.mjs`)

An ES module with no dependencies beyond Node's standard library, usable as a library and as a command:

```js
/** Cloudflare Pages' per-file limit: 25 MiB. */
export const PAGES_FILE_LIMIT = 26_214_400

/** The initial JavaScript limit of ui.md section 5: 250 KB gzipped, with 1 KB = 1,000 bytes. */
export const INITIAL_JAVASCRIPT_LIMIT = 250_000

/** The exact text of `_headers` (2.7). */
export const EXPECTED_HEADERS = '/*\n  Cross-Origin-Opener-Policy: same-origin\n  Cross-Origin-Embedder-Policy: require-corp\n'

/**
 * Every regular file under `directory` (recursively, not following symbolic links) larger than `limit` bytes,
 * as `{ path, size }` with `path` relative to `directory` using `/`, sorted by path.
 */
export async function oversizedFiles(directory, limit = PAGES_FILE_LIMIT)

/**
 * The initial JavaScript of `directory` (below) as `{ files, unresolved }`. `files` lists each initial script once as
 * `{ path, size, gzippedSize }`, sorted by path, with `path` as `oversizedFiles` writes it, `size` its length in bytes
 * and `gzippedSize` the length of `gzipSync(contents, { level: 9 })` from `node:zlib`. `unresolved` lists each distinct
 * reference of `index.html` that is not a file of the build, in order of first appearance.
 */
export async function initialJavaScript(directory)

/**
 * The initial JavaScript problems of `directory` as messages (section 4): one per unresolved reference, in order,
 * then the limit message when the `gzippedSize` values of its files sum to more than `limit` bytes.
 */
export async function initialJavaScriptProblems(directory, limit = INITIAL_JAVASCRIPT_LIMIT)

/**
 * Every problem that keeps `directory` from deploying, as messages (section 4): oversized files by path, then
 * `_headers`, then `initialJavaScriptProblems(directory)`; only the missing-directory message when `directory` does not
 * exist; empty when it is ready.
 */
export async function checkDist(directory)
```

Command: `node scripts/check-dist.mjs [directory]` (default `dist`, relative to the working directory). `ui/package.json` gains `"check:dist": "node scripts/check-dist.mjs dist"` and `"test:scripts": "node --test \"scripts/**/*.test.mjs\""`.

- Exit 0 and two lines on standard output, `{directory} is ready for Cloudflare Pages: {n} files, the largest is {path} ({size} bytes).` and `Initial JavaScript: {total} bytes gzipped, {percent} percent of the limit of 250000 bytes.`, when `checkDist` returns no problems. `{total}` is the sum of the `gzippedSize` values of `initialJavaScript(directory).files` (0 when there are none) and `{percent}` is `(total * 100 / INITIAL_JAVASCRIPT_LIMIT).toFixed(1)`.
- Exit 1 with every problem on standard error, one per line, when it returns any.
- Exit 2 with `usage: node scripts/check-dist.mjs [directory]` when given more than one argument.

`build-wasm.sh` keeps its informational size line and warning; the gate is this script.

**Initial JavaScript** is the JavaScript a browser downloads and runs before the app is usable (ui.md 3.1 and 5). In `directory` it is:

1. Every file that the regular file `index.html` names in the `src` of a `<script>` tag or in the `href` of a `<link>` tag whose `rel` is `modulepreload`. Vite writes a `modulepreload` link for every chunk the entry script imports statically, directly or through other chunks, so these tags cover the whole static import graph; `vite.config.ts` keeps Vite's default `build.modulePreload` for that reason (test 32). Tags are read as Vite writes them: lowercase tag and attribute names, attribute values in double quotes, attributes in any order. An inline script (a `<script>` without `src`) is part of `index.html` and does not count, and neither its text nor the text of an HTML comment is read for tags. Without a regular file `index.html`, nothing is read.
2. The core worker script: every regular file whose path starts with `assets/core.worker-`, ends with `.js` and contains no further `/`. That is the name Vite gives `src/core/core.worker.ts` (its default `assets/[name]-[hash].js`). The app starts the worker during startup (ui.md 3.1), before it is usable, so its script counts; a change that renames the worker entry or its output name changes this pattern and test 32 too.

A reference of `index.html` is a file of the build when, with one leading `/` or `./` removed, it equals the `path` of a regular file of `directory` as `oversizedFiles` writes paths. Any other reference is unresolved: a URL with a scheme or starting with `//`, a missing file, a folder, a symbolic link, or a path with a query or a fragment. Each file counts once, even when it is named twice or is both referenced and the worker script.

Not initial: the core module under `core/` (`partsmith_core.mjs` and `partsmith_core.wasm`), which the worker loads and ui.md 5 excludes (3.5 gates its size); source maps; stylesheets; and every other `.js` file, such as a chunk Vite splits off for a dynamic `import()`.

The measure is the sum of each file's gzipped length, each file compressed on its own (it is a separate response) with `gzipSync` at level 9, zlib's highest, the same as `gzip -9`. 1 KB is 1,000 bytes, as in the sizes Vite prints after a build, so the limit is 250,000 bytes, and a total of exactly 250,000 bytes passes. Pages compresses responses itself, with its own encodings and levels, so the measure is a fixed and reproducible stand-in for the transfer size, not the number of bytes served.

**Measured (2026-10-13, a `vite build` of the current tree):** `index.html` names one script, `assets/index-*.js` (756,011 bytes, 232,509 gzipped), and no `modulepreload` link, because the app is one chunk; `assets/core.worker-*.js` is 7,286 bytes, 2,839 gzipped. The initial JavaScript is 235,348 bytes gzipped, 94.1 percent of the limit and 14,652 bytes under it (open issue 8). The gzip column of Vite's build report comes from another compressor and reads about 1 percent higher (235.11 kB for `assets/index-*.js`); the check's number is the one the limit applies to.

### 2.7 `ui/public/_headers`

Exactly `EXPECTED_HEADERS` (90 bytes, LF line endings, trailing newline). Vite copies it to `ui/dist/_headers`, and Pages applies it to every path. Both headers are needed for `SharedArrayBuffer` and WebAssembly threads (technical design section 4). Google Fonts sends `Cross-Origin-Resource-Policy: cross-origin`, so the fonts load under `require-corp` (ADR 0013). `vite.config.ts` sets the same two headers for `pnpm dev` and `pnpm preview`, so local runs and Playwright behave like production.

### 2.8 Examples corpus

This section settles the layout that features.md open issue 3 and cli.md open issue 3 left to it.

**The rule.** Every `.ptsm` file under `examples/` is an example. Next to `examples/<path>/<stem>.ptsm`:

- `<stem>.expected.json` holds its expected results (required).
- `<stem>.py` is its Python twin (required exactly when the example qualifies, see below; forbidden otherwise).
- `assets/` holds the files its features import, named by content hash (cli.md 2.3), shared by the examples in that folder.

Allowed locations: `examples/<type>/<stem>.ptsm`, where `<type>` is a feature type registered by `Registry::with_builtins()` and `<stem>` matches `[a-z0-9]+(-[a-z0-9]+)*` (the 165 existing examples, which `crates/partsmith-recompute/tests/feature_*.rs` load by path, all follow it and keep their names); and `examples/bench/<name>/model.ptsm` for benchmark documents (`examples/bench/fifty-features/model.ptsm`, engine.md section 5), with `<name>` matching the same pattern. No other files exist under `examples/` outside `assets/` folders. Examples do not set the rollback marker (rollback is covered by engine.md, recompute.md and python-api.md tests), so every feature of an example is built.

**Expected results.** `<stem>.expected.json` is the `partsmith rebuild --checks --json` report of the example (cli.md 3.3) with two members removed: the top-level `path` (it depends on where the command runs) and every finding's `triangles` (triangle ranges depend on the display tessellation; the finding's `faces`, `location`, `value` and `limit` stay). Everything else stays in the report's order: `printer`, `features` (each with `id`, `type`, `status`, `messages`), `parameters`, `bodies` (volume, area, center of mass, bounding box and face, edge and vertex counts, or `empty`) and `checks` (summary and per-body reports). The file is pretty JSON with two-space indentation and one trailing newline, as the CLI prints it.

**Qualifying for a Python twin.** An example qualifies when it has no `import` feature (a script cannot carry asset bytes, features/import.md) and no feature in its expected results has status `error` (a feature function that fails raises `FeatureError`, python-api.md 3.5). These are the conditions of python-api.md 3.9's first form.

**Rust runner (`crates/partsmith-cli/tests/examples.rs`).** One integration test file, run by `cargo test --workspace` (and alone by `cargo test -p partsmith-cli --test examples`). It finds the repository root from `CARGO_MANIFEST_DIR` and runs the built binary (`CARGO_BIN_EXE_partsmith`) with the repository root as working directory. Its tests:

1. `examples_follow_the_layout`: every file under `examples/` outside `assets/` is an allowed `.ptsm`, `.expected.json` or `.py` path; every `.expected.json` and `.py` has its `.ptsm`; no example has a `rollback` member.
2. `examples_are_canonical`: `partsmith fmt --check` over every example exits 0.
3. `every_feature_type_has_an_example`: for every type in `Registry::with_builtins().types()` (through `partsmith_engine::Session::new(SessionOptions::headless()).registry()`), some example under `examples/<type>/` has a feature record of that type.
4. `examples_rebuild_as_expected`: for each example, in parallel over `std::thread::available_parallelism()` workers, runs `partsmith rebuild --checks --json examples/<path>/<stem>.ptsm`, accepts exit status 0 or 1 (1 means a feature failed, which the expected results record), removes `path` and the findings' `triangles`, and compares the result with `<stem>.expected.json` by the rules below. It also checks that `<stem>.py` exists exactly when the example qualifies.
5. The comparison rules' own tests (section 6, tests 20 to 25), on literal JSON.

Each test collects every problem before failing and fails once, listing the problems sorted by path, one per line (section 4).

**Comparison.** Two JSON values match when they have the same type; objects have the same set of keys and matching values; arrays have the same length and matching elements; strings, booleans and `null` are equal; two numbers written as integers (no fraction or exponent) are equal; any other two numbers `actual` and `expected` satisfy `|actual - expected| <= max(1e-6 * |expected|, 1e-6)`. The tolerance is python-api.md's 1e-6 relative for volumes and areas with an absolute floor of `LINEAR_TOLERANCE` (1e-6) for values near zero; it absorbs floating-point differences between platforms and compilers while catching any real change. A mismatch is reported at its JSON Pointer.

**Regenerating expected results.** After an intended change, run `PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test examples`. With `PARTSMITH_BLESS` set to `1`, `examples_rebuild_as_expected` writes `<stem>.expected.json` for every example whose file is missing or does not match, prints `blessed examples/<path>/<stem>.expected.json` for each, and passes; missing Python twins are listed but do not fail (the Python bless below adds them). Layout, canonical form and coverage still fail. When the environment variable `CI` is set, a set `PARTSMITH_BLESS` fails the run with `PARTSMITH_BLESS is set in CI. Bless expected results locally and commit them.` Without `PARTSMITH_BLESS`, a missing expected file fails with `examples/<path>/<stem>.expected.json is missing. Run: PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test examples`. Blessed files are reviewed in the pull request like code; a kernel upgrade that changes them needs the sign-off of technical design section 16.

**Python twin checks (`python/tests/test_examples.py`).** Run by `pytest python/tests` in the `python` job. They find the repository root from the test file's location. For each example with a twin:

1. `run_script(<stem>.py)` returns exactly one document, and its `to_text()` equals the example's `.ptsm` text byte for byte, so the twin builds the identical model.
2. python-api.md 3.9's second form holds for the twin: `D1 = run_code(twin)`, `D2 = run_code(D1.to_python())`, `D2.to_text() == D1.to_text()`.
3. python-api.md 3.9's first form holds for the example's `.ptsm` text.

A twin may be the generated script or a hand-edited, more readable one, as long as check 1 holds. With `PARTSMITH_BLESS=1` (refused under `CI` as above), for every qualifying example (by its `.expected.json`) whose twin is missing or fails check 1, the test writes the generated script, `Document.open(<stem>.ptsm).to_python()`, to `<stem>.py`, prints `blessed examples/<path>/<stem>.py`, and passes.

**Adding an example.** Write the `.ptsm` (canonical: `partsmith fmt`), run `PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test examples`, then `PARTSMITH_BLESS=1 pytest python/tests/test_examples.py` when it qualifies, review the new files, and commit them together.

## 3. Behavior

### 3.1 Job graph and gating

- A pull request merges only when every CI job passes (branch protection on `main` requiring every job of 2.4 except `deploy`; a repository setting, section 8).
- Jobs without `needs` start at once. OCCT builds run only on a cache miss (a change to `scripts/build-occt.sh`, `scripts/occt-version.sh` or, for WebAssembly, `scripts/emsdk-version.sh`), which takes tens of minutes; otherwise they restore in about a minute.
- `deploy` runs after everything else, so production only ever receives a build whose tests passed, and a preview exists for every pull request whose checks pass.
- `cancel-in-progress` drops superseded runs of the same ref; a canceled deploy leaves the previous deployment active, because Pages activates a deployment only after its upload completes.

### 3.2 Caches and system packages

| Cache | Key | Path |
| --- | --- | --- |
| OCCT native | `occt-native-${{ runner.os }}-${{ hashFiles('scripts/build-occt.sh', 'scripts/occt-version.sh') }}` (as today; on Windows and macOS the same formula gives per-OS keys) | `.occt/native` |
| OCCT WebAssembly | `occt-wasm-${{ hashFiles('scripts/build-occt.sh', 'scripts/occt-version.sh', 'scripts/emsdk-version.sh') }}` (as today) | `.occt/wasm` |
| Cargo | `Swatinem/rust-cache` defaults | registry and `target/` |
| pnpm store | `actions/setup-node` `cache: pnpm` on `ui/pnpm-lock.yaml` | pnpm store |
| `tauri-cli` | `tauri-cli-2.12.2-${{ runner.os }}-${{ runner.arch }}` | `~/.cargo/bin/cargo-tauri` (`cargo-tauri.exe` on Windows) |

Tag-triggered release runs restore caches saved on `main`; a run's own caches are scoped to its ref, so no later tag restores what a tag run saves, and GitHub drops caches unused for 7 days. CI runs only on Linux, so it saves only the Linux caches on `main`. The Windows and macOS caches reach `main` through a release dry run (`workflow_dispatch`) started from `main` before each tag. The `bundle` job's `Swatinem/rust-cache` uses `save-if: ${{ github.ref == 'refs/heads/main' }}`, so only that dry run saves its Rust cache. The Tauri Linux packages, installed with `sudo apt-get update && sudo apt-get install -y libwebkit2gtk-4.1-dev libxdo-dev libssl-dev librsvg2-dev` in `rust`, `desktop` and the Linux `bundle` job, are Tauri's documented prerequisites without the tray library (the app has no tray icon). The `rust` job needs them because `partsmith-desktop` is a workspace member.

**Disk space.** About 170 test binaries each link OCCT statically. Without debug info they are 55 to 80 MB each; with Cargo's default debug info they outgrow the free space of an `ubuntu-24.04` runner, and the linker dies with a bus error while writing its output (the first CI run, 2026-10-13). So the workflow sets `CARGO_PROFILE_DEV_DEBUG: "0"` for every job (panic messages keep their file and line; a developer who needs a debugger builds locally with debug info), and the `rust` job first removes the preinstalled SDKs it does not use (`sudo rm -rf /usr/share/dotnet /usr/local/lib/android /opt/ghc /opt/hostedtoolcache/CodeQL`) and prints `df -h /`. Fewer, larger integration test binaries per crate would cut the space and link time at the root (open issue 9).

`deny.toml` keeps its license allow list: every license in the Tauri dependency tree is already allowed (ADR 0014). Two RustSec advisories apply to the Linux GTK3 stack Tauri uses, RUSTSEC-2024-0429 (`glib` 0.18, unsound `VariantStrIter`) and RUSTSEC-2024-0370 (`proc-macro-error`, unmaintained, a build-time dependency of the gtk-rs 0.18 macros); `deny.toml` lists both in `[advisories] ignore` with that reason and a note to remove them when Tauri moves past gtk-rs 0.18 (section 8).

### 3.3 Cloudflare Pages deploy

1. The deploy runs only when both secrets are present. On forks and in repositories without them (including Dependabot pull requests, which get no secrets) the job succeeds with the notice of 2.4 and deploys nothing.
2. A push to `main` deploys with `--branch=main`; the Pages project's production branch is `main`, so that deployment becomes production.
3. A pull request deploys with `--branch=<head branch>`; Pages makes it a preview with a stable alias `https://<sanitized branch>.<project>.pages.dev` and a unique deployment URL. The alias URL is the `preview` environment's URL on the pull request and is written to the job summary.
4. The artifact deployed is exactly the `web-dist` that `web` checked (`check:dist`) and that `e2e` tested against the same core build.
5. The project name comes from the repository variable `CLOUDFLARE_PAGES_PROJECT`, else `partsmith`. Creating the project (`wrangler pages project create <name> --production-branch main`) and the token are one-time human steps (section 8).
6. Only this job builds what Pages serves. Cloudflare's own build image cannot build the WebAssembly core (it needs Emscripten and OCCT built for WebAssembly, longer than a Pages build may run), and `ui/public/core` is not in git, so a Pages build of `ui/` alone serves an app whose core fails to load. A project connected to the repository in the Cloudflare dashboard therefore keeps its automatic builds paused (Settings, Build, Branch control: "Enable automatic production branch deployments" off and preview branches None), and this job's Wrangler deploys go to it as to a project created with Wrangler.

### 3.4 Generated files

The `schema` job fails when regenerating changes or adds a file: after `cargo run -p partsmith-cli -- schema --write` and `pnpm -C ui generate:protocol`, `git status --porcelain -- docs/format ui/src/generated` must print nothing. On failure it prints `::error::Generated files are out of date. Run: cargo run -p partsmith-cli -- schema --write && pnpm -C ui generate:protocol` and the diff (`git diff -- docs/format ui/src/generated`). cli.md test 53 (`schema --check` in the repository root) checks the same within `cargo test`; the job also covers the TypeScript types and names the fix.

### 3.5 The 25 MiB per-file gate

Cloudflare Pages serves files of at most 25 MiB each (technical design section 4). `pnpm check:dist` in the `web` job fails the build when any file in `ui/dist` is larger, naming each one, so a deploy can never silently drop or break the core.

**Measured sizes (2026-10-12).**

- `ui/dist` without the core (the latest local build): `assets/index-*.js.map` 1,045,216 bytes, `assets/index-*.js` 219,690 bytes, `assets/index-*.css` 4,356 bytes, `index.html` 467 bytes, `_headers` 90 bytes.
- The WebAssembly core: `ui/public/core/partsmith_core.wasm`, linked with the engine by `scripts/build-wasm.sh` (`wasm-release` profile: `opt-level = "s"`, fat LTO), is 18,050,908 bytes (17.2 MiB), about 7.8 MiB under the 25 MiB limit.
- The kernel alone: the largest kernel test module (the `partsmith-occt-sys` unit tests, which reach the whole facade), built with the same profile, is 14.08 MiB, 13.5 MiB after `wasm-opt -Oz`, 4.9 MiB gzip and 3.6 MiB brotli (ADR 0017).

At these sizes the core fits in one module: splitting it into lazily loaded modules is not needed now, and R2 is a fallback only (ADR 0017).

**The plan when the gate fails**, in order; each step is a decision recorded in an ADR with human review, not something CI does on its own:

1. Size options within one module: `-Oz` for Rust and for the OCCT build, Binaryen `-Oz` with `--converge`, and linking only the OCCT toolkits the engine calls. This changes the build flags of ADR 0003 and ADR 0009.
2. Lazily loaded modules (technical design section 4): a base kernel module, booleans and fillets, STEP I/O and mesh tools, each linked with only the OCCT toolkits it needs, loaded by the worker on first use. This changes the bridge of protocol.md 2.10 and ADR 0009.
3. Cloudflare R2 for any file still over the limit: a bucket on a custom domain serving content-hashed core files with `Cross-Origin-Resource-Policy: cross-origin` and CORS for the Pages origin (both needed under `require-corp`), uploaded by CI and loaded by the worker from a build-time base URL. It needs a domain, an R2 API token as a new secret, cost approval, and a ui.md change to the worker's `load` (ui.md 2.3).

### 3.6 Desktop release

Matrix of the `bundle` job (desktop.md 3.8):

| `runner.os` | Runner | `--bundles` | Uploaded files |
| --- | --- | --- | --- |
| Linux | `ubuntu-24.04` | `deb,rpm,appimage` | `target/release/bundle/deb/*.deb`, `rpm/*.rpm`, `appimage/*.AppImage` |
| Windows | `windows-2025` | `nsis` | `target/release/bundle/nsis/*-setup.exe` |
| macOS | `macos-15` | `app,dmg` | `target/release/bundle/dmg/*.dmg` |

Steps per runner, all with `shell: bash` (Git Bash on Windows):

1. Checkout. Linux: the Tauri packages and `ninja-build`. Windows: `ilammy/msvc-dev-cmd@v1.13.0` (MSVC, CMake and Ninja from Visual Studio on `PATH`), then a step that writes `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER` (the `link.exe` next to `cl.exe`, the one CMake uses for OCCT) to `$GITHUB_ENV`, because Git Bash puts GNU `link` first on `PATH` and rustc takes the first `link.exe` when `VCINSTALLDIR` is set. macOS: `brew install ninja`.
2. OCCT native: `actions/cache@v6.1.0` with the key of 3.2; on a miss, `scripts/build-occt.sh native "$OCCT_ROOT"` with `OCCT_JOBS: 4` in the environment (so the script does not need `nproc`, which macOS lacks). On macOS the job sets `MACOSX_DEPLOYMENT_TARGET: "13.0"` for OCCT, Rust and the bundle (desktop.md 2.8).
3. Rust toolchain and `Swatinem/rust-cache@v2.9.2` with `save-if: ${{ github.ref == 'refs/heads/main' }}`, so only a dry run from `main` saves the Rust cache, where every tag run can restore it (3.2); pnpm and Node 22; `pnpm -C ui install --frozen-lockfile`; `pnpm -C ui build`.
4. `tauri-cli` from its cache, else `cargo install tauri-cli --version 2.12.2 --locked`.
5. `cargo tauri build --bundles <bundles> --ci` in `desktop/src-tauri`.
6. Copy the files of the table into one flat `bundles/` folder (each pattern expands unquoted, so a pattern that matches nothing fails the copy), and upload `bundles/` as `bundle-<runner.os>` (`if-no-files-found: error`). Flat assets let `draft` run `gh release create` on `release/*` and write `SHA256SUMS` over plain file names.

`draft` then creates the draft release with every bundle and `SHA256SUMS`. A human downloads and tries the bundles, edits the notes, and publishes the release (technical design section 17: "a human approves each tagged version before it is published"). Nothing is signed: Windows SmartScreen warns and macOS needs the user's approval in System Settings, which the release notes state.

**Prerequisites in the build scripts**, made by the implementer of `release.yml` and proven by a `workflow_dispatch` dry run on all three runners before the first tag: `scripts/build-occt.sh` passes `-DINSTALL_DIR_LAYOUT=Unix` on Windows, so the install has the `include/opencascade` and `lib` folders `partsmith-occt-sys/build.rs` expects; the Windows build uses Ninja with MSVC from the developer environment; and the native OCCT and the shim must build with MSVC and with Apple Clang for arm64, which CI has not yet exercised (open issue 1).

**Where signing will plug in** (later, 1.2): the `bundle` job's environment gets the Apple variables of desktop.md 1.2 on macOS and the Windows certificate variables on Windows; the Tauri bundler signs and notarizes when they are present. The updater adds `bundle.createUpdaterArtifacts` and the signing key variables, and `draft` uploads `latest.json`. Each needs the secrets and a human decision first.

### 3.7 Examples in CI

The `rust` job runs the Rust runner as part of `cargo test --workspace` (with the debug CLI binary); the `python` job runs the twin checks as part of `pytest python/tests`. A failing example names its file and the first differing JSON Pointer, so the reviewer sees exactly what changed.

## 4. Errors

| Situation | Where | Message |
| --- | --- | --- |
| A file in `ui/dist` over the limit | `check:dist`, standard error, exit 1 | `{directory}/{path} is {size} bytes, over the Cloudflare Pages limit of 26214400 bytes (25 MiB). Split the core into lazily loaded modules or serve the file from R2 (docs/spec/delivery.md section 3.5).` |
| `_headers` missing | `check:dist` | `{directory}/_headers is missing. It must set Cross-Origin-Opener-Policy and Cross-Origin-Embedder-Policy (docs/spec/delivery.md section 2.7).` |
| `_headers` different | `check:dist` | `{directory}/_headers does not match the cross-origin isolation headers of docs/spec/delivery.md section 2.7.` |
| A script or preload of `index.html` that is not a file of the build (2.6) | `check:dist`, exit 1 | `{directory}/index.html loads {reference}, which is not a file in {directory}. Make it the path of a file of the build, with no scheme, query or fragment, and keep base in vite.config.ts as '/' or './'.` (`{reference}` exactly as written in the tag) |
| Initial JavaScript over the limit (2.6) | `check:dist`, exit 1 | `{directory} has {total} bytes of initial JavaScript gzipped, over the limit of {limit} bytes (docs/spec/ui.md section 5): {files}. Move code that startup does not need into modules loaded with import() (docs/spec/delivery.md section 2.6).` (`{limit}` is 250000 from `checkDist`; `{files}` is each initial file as `{path} ({gzippedSize} bytes)`, in path order, joined by `, `) |
| Directory missing | `check:dist`, exit 1 | `{directory} does not exist. Run pnpm build first.` |
| Wrong arguments | `check:dist`, exit 2 | `usage: node scripts/check-dist.mjs [directory]` |
| Generated files drifted | `schema` job | `Generated files are out of date. Run: cargo run -p partsmith-cli -- schema --write && pnpm -C ui generate:protocol` |
| Deploy secrets absent | `deploy` job notice, success | `Cloudflare secrets are not available to this run (a fork, or the repository is not configured), so nothing was deployed.` |
| Tag and version differ | `verify` | `Tag v{tag} does not match the version {version} of partsmith-desktop and ui/package.json.` |
| No green CI for the tagged commit | `verify` | `CI has not passed for {sha}. Tag a commit whose CI run succeeded.` |
| Example file in a wrong place | examples runner | `examples/{path}: not an allowed example path (docs/spec/delivery.md section 2.8).` |
| Orphan companion file | examples runner | `examples/{path}: has no .ptsm next to it.` |
| Example sets the rollback marker | examples runner | `examples/{path}: examples must not set the rollback marker.` |
| Example not canonical | examples runner | `examples/{path}: not canonical. Run: partsmith fmt examples/{path}` |
| Feature type without an example | examples runner | `Feature type {type} has no example. Add one under examples/{type}/.` |
| Expected file missing | examples runner | `examples/{path}.expected.json is missing. Run: PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test examples` |
| Mismatch | examples runner | `examples/{path}.ptsm: {pointer} is {actual}, expected {expected}` (for a missing or extra key: `{pointer} is missing` or `{pointer} is unexpected`) |
| Rebuild failed to run | examples runner | `examples/{path}.ptsm: partsmith rebuild exited with {status}: {standard error}` |
| Twin missing | examples runner | `examples/{path}.ptsm: qualifies for a Python twin but {stem}.py is missing. Run: PARTSMITH_BLESS=1 pytest python/tests/test_examples.py` |
| Twin not allowed | examples runner | `examples/{path}.py: the example has an import feature or a failing feature, so it cannot have a Python twin.` |
| Twin builds a different model | `test_examples.py` | `examples/{path}.py does not rebuild examples/{path}.ptsm byte for byte.` with a unified diff |
| Bless in CI | both runners | `PARTSMITH_BLESS is set in CI. Bless expected results locally and commit them.` |

## 5. Budgets

On GitHub's standard runners, with warm caches:

| Measure | Budget |
| --- | --- |
| CI wall time from push to `deploy` finished | 25 minutes |
| The Rust examples runner over the corpus (debug binary, inside `cargo test`) | 3 minutes |
| `pnpm check:dist` | 2 s |
| `deploy` job | 3 minutes |
| A release from tag to draft, OCCT caches warm | 60 minutes (cold: 120 minutes) |

A cold OCCT build is excluded from the CI budget; it happens only when the OCCT scripts change.

## 6. Acceptance tests

Each job's checks are its acceptance tests; they pass on every pull request and push to `main`. Script and runner tests are listed with them.

1. `occt-native`, `occt-wasm`: the install exists at `.occt/native` or `.occt/wasm` (cache hit or built) and contains `lib/libTKernel.a`.
2. `kernel-wasm`: `cargo test -p partsmith-occt-sys --target wasm32-unknown-emscripten` passes.
3. `rust`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, the same clippy run of `partsmith-recompute` with the feature `stability-study`, and `cargo test --workspace` pass, the latter including `partsmith-desktop` (desktop.md section 6) and the examples runner (2.8).
4. `deny`: `cargo deny check` passes with `deny.toml`.
5. `ui`: `pnpm lint`, `pnpm typecheck`, `pnpm test` and `pnpm test:scripts` pass.
6. `schema`: after regeneration, `git status --porcelain -- docs/format ui/src/generated` prints nothing.
7. `core-wasm`: `scripts/build-wasm.sh` exits 0 and writes `ui/public/core/partsmith_core.mjs` and `partsmith_core.wasm`; `cargo test -p partsmith-wasm --test web_module` passes without skipping (`PARTSMITH_REQUIRE_WEB_MODULE=1` turns a skip into a failure).
8. `web`: `pnpm build` and `pnpm check:dist` exit 0; `ui/dist/_headers` exists (checked by `check:dist`), and `ui/dist/core/partsmith_core.wasm` and `ui/dist/core/partsmith_core.mjs` exist (checked by the job after `check:dist`).
9. `e2e`: `pnpm e2e` passes against the real core (ui.md 6.5).
10. `python`: `maturin develop --release` succeeds and `pytest python/tests` passes, including `test_examples.py`.
11. `deploy`: with secrets, the Wrangler step succeeds and the job summary contains a `pages.dev` URL; on a fork pull request, the job succeeds and its log contains the "Deploy skipped" notice.
12. `desktop`: `cargo tauri build --bundles deb` exits 0, exactly one `.deb` exists under `target/release/bundle/deb/`, and its file list holds `usr/bin/partsmith-desktop` and exactly one `.desktop` file under `usr/share/applications/` (desktop.md test 49). The file list is `dpkg-deb --fsys-tarfile` piped to `tar -t` with a leading `./` removed from each entry: Tauri's bundler writes entries without the `./` that `dpkg-deb --build` writes, so the check accepts both forms.
13. `release.yml` dry run (`workflow_dispatch`): the three `bundle` jobs upload `bundle-Linux` (a `.deb`, an `.rpm` and an `.AppImage`), `bundle-Windows` (one `-setup.exe`) and `bundle-macOS` (one `.dmg`); `verify` and `draft` are skipped. On a tag, `draft` creates a draft release holding those files and `SHA256SUMS`, whose lines verify with `sha256sum -c`.

**Distribution check (`ui/scripts/check-dist.test.mjs`, `node:test`, temporary directories)**

Run by `pnpm test:scripts` in `ui/`. `node --test` takes files or glob patterns, not a folder, so the script passes the quoted glob `"scripts/**/*.test.mjs"`.

14. `PAGES_FILE_LIMIT` is 26214400 and `EXPECTED_HEADERS` equals the bytes of `ui/public/_headers`.
15. `oversizedFiles` of a directory with a 10-byte file and a sparse file of exactly 26,214,400 bytes returns `[]`.
16. A sparse file of 26,214,401 bytes at `core/big.wasm` makes `oversizedFiles` return `[{ path: 'core/big.wasm', size: 26214401 }]`; two oversized files in nested folders come back sorted by path.
17. A symbolic link to an oversized file outside the directory is not reported.
18. `checkDist` returns `[]` for a directory with a small file and the exact `_headers`; the missing-headers message when `_headers` is absent; the mismatch message when it lacks its trailing newline.
19. The command on a ready directory (its `index.html` names no script and it has no worker script) exits 0 and prints the ready line with the file count and the largest file, then `Initial JavaScript: 0 bytes gzipped, 0.0 percent of the limit of 250000 bytes.`; on a directory with an oversized file exits 1 and prints the oversized message naming `{directory}/core/big.wasm`; on a missing directory exits 1 with the missing message; with two arguments exits 2 with the usage line.

**Examples comparison (`crates/partsmith-cli/tests/examples.rs`, on literal JSON)**

20. `{"volume": 24000.0}` matches `{"volume": 24000.02}` (within 1e-6 relative) and does not match `{"volume": 24000.03}`, reported as `/volume is 24000.03, expected 24000.0`.
21. `{"x": 0.0}` matches `{"x": 0.0000009}` (absolute floor) and not `{"x": 0.000002}`.
22. `{"faces": 6}` does not match `{"faces": 7}`; integers are never compared with a tolerance.
23. A missing key and an extra key are reported as `is missing` and `is unexpected` at their pointers; arrays of different lengths do not match.
24. The projection of a report removes `path` and every finding's `triangles` and keeps every other member in order.
25. With `PARTSMITH_BLESS=1` and `CI` set, the bless entry point fails with the bless-in-CI message.

**Initial JavaScript (`ui/scripts/check-dist.test.mjs`, `node:test`, temporary directories, run by `pnpm test:scripts`)**

26. `INITIAL_JAVASCRIPT_LIMIT` is 250000.
27. Which files count: a directory whose `index.html` holds, in this order, `<script type="module" crossorigin src="/assets/index-a1.js"></script>`, `<link rel="modulepreload" crossorigin href="/assets/vendor-b2.js">`, `<link rel="modulepreload" crossorigin href="./assets/vendor-b2.js">`, `<link rel="stylesheet" crossorigin href="/assets/index-c3.css">` and `<script type="module">import './assets/inline-f6.js'</script>`, next to the files `assets/index-a1.js`, `assets/index-a1.js.map`, `assets/vendor-b2.js`, `assets/index-c3.css`, `assets/inline-f6.js`, `assets/lazy-d4.js`, `assets/core.worker-e5.js`, `assets/core.worker-e5.js.map`, `workers/core.worker-g7.js` and `core/partsmith_core.mjs`, each holding a different short text: `initialJavaScript` returns `unresolved` `[]` and `files` with exactly `assets/core.worker-e5.js`, `assets/index-a1.js` and `assets/vendor-b2.js`, in that order, each with its byte length as `size` and the length of `gzipSync(contents, { level: 9 })` as `gzippedSize`. A directory holding only `assets/core.worker-e5.js` (no `index.html`) returns that file alone, and one holding only `assets/core.worker-e5/nested.js` returns no file. An `index.html` holding `<!-- <script type="module" src="/assets/commented-h8.js"></script> -->`, `<script type="module">console.log('<link rel="modulepreload" href="/assets/text-i9.js">')</script>` and the script tag of `/assets/index-a1.js`, next to those three files, gives `assets/index-a1.js` alone.
28. The gzip level: `assets/index-a1.js`, named by `index.html`, holds the text `const value{i} = {(i * 7919) % 10007};\n` for `i` from 0 to 9999 (237,788 bytes), whose `gzipSync` lengths at levels 1, 6 and 9 are three different numbers (the test asserts it first). Its `gzippedSize` is the level 9 length.
29. Unresolved references: next to the exact `_headers` and the files `assets/index-a1.js` and `assets/folder/x.js`, with `assets/link.js` a symbolic link to a file outside the directory, an `index.html` naming, in this order, `<script type="module" src="/assets/missing.js"></script>`, `<script type="module" src="https://example.com/remote.js"></script>`, `<link rel="modulepreload" href="/assets/folder">`, `<link rel="modulepreload" href="/assets/link.js">`, `<script type="module" src="/assets/index-a1.js?v=1"></script>` and `<script type="module" src="/assets/missing.js"></script>` again: `initialJavaScript` returns `files` `[]` and `unresolved` `['/assets/missing.js', 'https://example.com/remote.js', '/assets/folder', '/assets/link.js', '/assets/index-a1.js?v=1']`, and `checkDist` returns the unresolved message of each, in that order. Script tags naming `data:text/javascript,1`, `/assets/index-a1.js?v=1` and `/assets/index-a1.js#top`, next to regular files whose paths are those references with any leading `/` removed, leave all three unresolved, in that order, and count no file.
30. The limit: a directory with the exact `_headers`, an `index.html` naming `/assets/index-a1.js` (260,000 bytes from `randomBytes`) and the worker script `assets/core.worker-e5.js` (1,000 bytes from `randomBytes`), where `g1` and `g2` are the level 9 lengths of the worker and of `index-a1.js` and `T = g1 + g2` (over 250,000), all computed by the test. `checkDist` returns exactly the over-limit message with `{total}` T, `{limit}` 250000 and `{files}` `assets/core.worker-e5.js ({g1} bytes), assets/index-a1.js ({g2} bytes)`. `initialJavaScriptProblems(directory, T)` returns `[]`, and `initialJavaScriptProblems(directory, T - 1)` returns the same message with `{limit}` T - 1. With 200,000 random bytes in `assets/index-a1.js` instead, `checkDist` returns `[]`. With the 260,000 bytes, a sparse file of 26,214,401 bytes at `core/big.wasm`, no `_headers` and a second script `/assets/missing.js` after the first, `checkDist` returns the oversized, missing-headers, unresolved and over-limit messages, in that order.
31. The command on a ready directory whose `index.html` names `/assets/index-a1.js` holding the text of test 28 exits 0 and prints the ready line (the largest file is `assets/index-a1.js`) and then `Initial JavaScript: {T} bytes gzipped, {percent} percent of the limit of 250000 bytes.`, with `T` the level 9 length of that text and `{percent}` `(T * 100 / 250000).toFixed(1)`, both computed by the test. On the over-limit directory of test 30 it exits 1, prints nothing on standard output and the over-limit message on standard error.
32. The build keeps the shape 2.6 relies on. `resolveConfig({ root, configFile }, 'build')` from `vite`, with `root` the `ui` folder and `configFile` its `vite.config.ts`, gives `build.modulePreload` equal to Vite's default `{ polyfill: true }` (with `false` Vite writes no `modulepreload` link, and a `resolveDependencies` function can leave chunks out), `build.assetsDir` `assets` and no `worker.rolldownOptions.output`, so the worker script keeps Vite's name `assets/[name]-[hash].js`. `ui/src/core/core.worker.ts` is a regular file, and `ui/src/core/transport.ts` passes `new URL('./core.worker.ts', import.meta.url)` to `new Worker`.
33. Attributes in any order: an `index.html` holding `<link crossorigin href="/assets/vendor-b2.js" rel="modulepreload">` and `<script src="/assets/index-a1.js" type="module"></script>`, next to those two files, each holding a different short text: `initialJavaScript` returns `unresolved` `[]` and `files` with exactly `assets/index-a1.js` and `assets/vendor-b2.js`, in that order.

## 7. Open issues

1. **OCCT on Windows and macOS.** `scripts/build-occt.sh` and `partsmith-occt-sys` have been built only on Linux and Emscripten. The release dry run is the first MSVC and Apple Clang build; it may need script changes beyond `INSTALL_DIR_LAYOUT` (3.6), which this spec lists but does not prove.
2. **Core size.** The linked core is 17.2 MiB, about 7.8 MiB under the limit (3.5), so splitting it into lazily loaded modules is not needed now. If it grows past the limit, the `web` job fails, ADR 0017 is revisited, and the plan of 3.5 starts with an ADR; R2 (step 3) is a fallback only, for a file the first two steps cannot bring under the limit.
3. **Local OCCT installs.** A local OCCT WebAssembly install built before the `OCC_CONVERT_SIGNALS` fix of ADR 0003 makes optimized links fail in `wasm-opt` (in `ShapeUpgrade_ShapeDivide::Perform`). CI's cache key includes the script hash, so CI rebuilds; developers must rebuild local installs after the script changes.
4. **Examples and debug builds.** The runner uses the debug CLI binary inside `cargo test --workspace`. If the corpus grows past the 3-minute budget, it moves to its own job with `--release`.
5. **Import examples.** The three `examples/import/` examples reference the placeholder asset name of features/import.md (64 zeros), which the feature tests supply at run time. Through the CLI they have no asset, so they rebuild with `asset_missing` errors (recorded in their expected results) and have no Python twin. An example that imports a real STEP file needs an `examples/import/assets/<hash>.step` and a new example name.
6. **Source maps.** `vite.config.ts` emits source maps, which deploy with the app (about 1 MB today). That is fine for an open-source app and helps bug reports; dropping them from production is a one-line change if wanted.
7. **features.md and cli.md open issues 3.** Both point at the examples layout, which section 2.8 now defines; their text can be updated to point here when those specs are next edited.
8. **Initial JavaScript headroom.** The app is one chunk, and its initial JavaScript is 94.1 percent of the 250 KB limit (2.6), so a few more features will fail the `web` job. The first response is moving code that startup does not need into chunks loaded with a dynamic `import()` (candidates: the Print context, sketch mode, export), with the loading state each needs specified in ui.md. Raising the limit changes the budget of ui.md 5 and needs human review.
9. **One integration test binary per crate.** Each file in a crate's `tests/` is its own binary, and each links OCCT: about 170 binaries of 55 to 80 MB. Gathering a crate's integration tests into one binary (`tests/main.rs` with a module per file) would cut disk use and link time several times over, locally and in CI. It moves no test and changes no assertion, but touches every crate, so it waits until no other change is in flight there.

## 8. Needs human review

- **Cloudflare setup** (one time): create the Pages project with production branch `main`, create an API token with only "Account, Cloudflare Pages, Edit", add `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` as repository secrets, and optionally `CLOUDFLARE_PAGES_PROJECT`. Decide whether the account belongs to a person or a project organization (technical design section 19). A project connected to the repository in the dashboard has its automatic builds paused (3.3 item 6). Status 2026-10-13: `main` exists and a Git-connected project builds `ui/` with `pnpm build`; pausing its builds, the token and the secrets (and the project name variable, if it is not `partsmith`) remain.
- **Branch protection** on `main` requiring the CI jobs of 2.4.
- **`deny.toml` advisory exceptions** RUSTSEC-2024-0429 and RUSTSEC-2024-0370 for Tauri's GTK3 stack (3.2), a change to the dependency policy.
- **Release policy**: tags `vX.Y.Z` on commits with green CI, a dry run from `main` before each tag, draft releases published by a human, unsigned bundles until signing is set up, macOS arm64 only.
- **Build tools pinned here** (`tauri-cli` 2.12.2, Wrangler 4.149.0, maturin 1.15.0, pytest 9.1.1) and the new actions (`upload-artifact`, `download-artifact`, `setup-python`, `wrangler-action`, `msvc-dev-cmd`).
- **Examples rules**: the expected-results format and the 1e-6 tolerance, and the bless workflow.
- **The 25 MiB plan** (3.5): each step's ADR when the gate first fails.
