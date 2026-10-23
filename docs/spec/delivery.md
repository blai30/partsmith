# Delivery specification

Scope: continuous integration (`.github/workflows/ci.yml`), the Cloudflare Pages deploy of the web app, the desktop release workflow (`.github/workflows/release.yml`), the nightly workflow (`.github/workflows/nightly.yml`), the distribution check `ui/scripts/check-dist.mjs`, the cross-origin isolation headers `ui/public/_headers`, the third-party notices and their generator `ui/scripts/notices.mjs`, the npm license check `ui/scripts/check-licenses.mjs`, the source of each distributed build (the commit stamp of every web deployment and the OCCT source archive of every release, made by `scripts/occt-source-archive.sh`), what `README.md` tells users and contributors, and the examples corpus (its layout, expected results and runners). Read with [technical design](../design/technical-design.md) sections 4 (build and deployment), 16 (testing) and 17 (guardrails in CI, where humans decide), [architecture.md](../design/architecture.md) section "Testing", ADRs [0003](../adr/0003-pin-occt-and-build-from-source.md), [0006](../adr/0006-protocol-framing-and-schema.md), [0009](../adr/0009-emscripten-c-abi-bridge.md), [0012](../adr/0012-dependency-log-m1.md), [0013](../adr/0013-oxc-tooling-and-theme-stylesheet.md), [0014](../adr/0014-desktop-host-and-dependencies.md), [0028](../adr/0028-single-license-apache-2.md), [0031](../adr/0031-self-hosted-geist-and-jetbrains-mono-for-code.md), [0033](../adr/0033-notices-and-npm-license-check-from-the-lock-files.md) and [0034](../adr/0034-occt-linking-and-lgpl-obligations.md), and the specs whose checks CI runs: [cli.md](cli.md) (`schema`, `rebuild --json`), [protocol.md](protocol.md) 2.9 and 2.10, [ui.md](ui.md) 2.2 and 6, [python-api.md](python-api.md) 3.9, [features.md](features.md) (examples) and [desktop.md](desktop.md).

This spec is the definition of done for delivery. The checks of section 6 are its acceptance tests; implementers do not weaken them.

## 1. Purpose and scope

Every change is built and tested on every platform the project ships before it merges, and every merge to `main` deploys the web app, so reviewers always try the current state (technical design section 4). Nothing reaches users without a green build, and desktop releases wait for a human (technical design section 17).

### 1.1 In this milestone

- CI jobs: OCCT native and WebAssembly builds with caches; Rust format, lint and tests (including the desktop crate and the examples runner); `cargo-deny`; kernel tests on WebAssembly; the WebAssembly core built with `scripts/build-wasm.sh`; UI lint, typecheck, unit tests and script tests, and the licenses of the npm packages that ship checked against the allow list of `deny.toml` (2.10); the web build with the distribution check; Playwright end-to-end tests against the real WebAssembly core; schema, generated-type and third-party notices drift; Python package tests including the examples' Python twins, and the wheel built and installed on CPython 3.12 and 3.13 (python-api.md test 76); a Linux desktop build.
- Third-party notices for the desktop app and the web app, generated from the lock files, committed, checked in CI and shipped in the desktop bundles, the web build and each release (2.9).
- The source of every distributed build (ADR 0034, option A): a commit stamp in every web deployment, which `deploy` requires, and the OCCT source archive in every release (2.13).
- A nightly run of the sketch property tests at 4096 cases, which skips a night when `main` has not changed (2.11).
- What `README.md` says, and the release checklist (2.12, 3.6).
- Cloudflare Pages: production on every push to `main`, a preview per pull request, through the Wrangler action, skipped cleanly when the secrets are absent (forks, unconfigured repositories).
- The 25 MiB per-file gate on `ui/dist`, with the measured sizes and the plan when the core outgrows it (3.5).
- The initial JavaScript limit of ui.md 5, 250 KB gzipped, checked on `ui/dist` by the same script (2.6).
- `ui/public/_headers` with the cross-origin isolation headers.
- The desktop release workflow: on a version tag, bundles on Linux, Windows and macOS with the Tauri bundler and a draft GitHub Release that a human publishes.
- The examples corpus: one rule for every file under `examples/`, the expected-results format, the Rust runner with a documented way to regenerate expected files, the feature coverage check and the Python twin checks (section 2.8).
- Every action pinned to an exact version, and every job capped by `timeout-minutes` (2.4, 2.5, 2.11).

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
| Published Python wheels and CLI downloads | A publishing job (PyPI trusted publishing for the wheel), notices for each artifact like those of 2.9 and a way to its source like that of 2.13 (both link OCCT statically, ADR 0034), and a human decision. |

## 2. Public interface

### 2.1 Files

```text
.github/workflows/ci.yml          CI and the Pages deploy (2.4)
.github/workflows/release.yml     desktop release (2.5)
.github/workflows/nightly.yml     nightly property run (2.11)
ui/scripts/check-dist.mjs         distribution check (2.6)
ui/scripts/check-dist.test.mjs    its unit tests (node:test)
ui/public/_headers                cross-origin isolation headers (2.7)
THIRD-PARTY-NOTICES.txt           notices of the desktop app, generated (2.9)
ui/public/THIRD-PARTY-NOTICES.txt notices of the web app, generated (2.9)
ui/scripts/notices.mjs            notices generator and check (2.9)
ui/scripts/notices.test.mjs       its unit tests (node:test)
ui/scripts/notices/               committed license texts the generator reads: standard texts, copies from crate repositories, OCCT's, Rust's, Emscripten's and WebView2's (2.9)
ui/scripts/check-licenses.mjs     npm license check (2.10)
ui/scripts/check-licenses.test.mjs  its unit tests (node:test)
ui/scripts/workflows.test.mjs     tests of the three workflow files (node:test, tests 49 to 54, 57 and 59)
scripts/occt-source-archive.sh    the OCCT source archive of a release (2.13)
ui/scripts/occt-source-archive.test.mjs  its tests and those of the commit check of scripts/build-occt.sh (node:test, tests 55 and 58)
ui/scripts/isolated-git.mjs       the Git environment of tests 52, 55 and 58 (section 6)
ui/package.json                   scripts "check:dist", "test:scripts", "notices", "check:notices" and "check:licenses"
README.md                         what users and contributors read first (2.12)
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
| Python | 3.12 and 3.13 | `python-version` input of the `python` job, 3.13 first and 3.12 last, so 3.12 is the default `python` and `python3.13` is on `PATH` |
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
| `rust` | `occt-native` | Frees disk space and installs the Tauri Linux packages (3.2), then `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and prints the free disk space after the tests, also when they fail. The workspace includes `partsmith-desktop` and the examples runner. |
| `deny` | | `cargo-deny` with `deny.toml` (unchanged action; the policy now covers the Tauri tree, 3.2). |
| `ui` | | In `ui/`: `pnpm install --frozen-lockfile`, `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm test:scripts`, `pnpm check:licenses` (2.10). |
| `schema` | `occt-native` | `pnpm -C ui install --frozen-lockfile` (step `id: install`), `cargo run -p partsmith-cli -- schema --write`, `pnpm -C ui generate:protocol`, the drift check of 3.4, then `pnpm -C ui check:notices` (2.9) in a step with `if: ${{ !cancelled() && steps.install.outcome == 'success' }}`, so that a failure of the schema steps or the drift check does not hide a notices failure, while the notices check, which reads `ui/node_modules`, never runs without the packages. The install comes first because it needs nothing the schema steps make. |
| `core-wasm` | `occt-native`, `occt-wasm` | Restores both OCCT caches, sets up Emscripten 4.0.15, runs `scripts/build-wasm.sh` with `OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN=$GITHUB_WORKSPACE/.occt/wasm`, then `cargo test -p partsmith-wasm --test integration web_module::` (the Node round trip against the module just built) with `PARTSMITH_REQUIRE_WEB_MODULE=1`, so the test fails instead of skipping when Node or the module is missing, and uploads `ui/public/core/` and `ui/public/THIRD-PARTY-NOTICES.txt` as the artifact `core-wasm` (one `path` per line, so the artifact is rooted at their common folder `ui/public/` and holds `core/` and the web notices, which go wherever the core goes; retention 7 days, `if-no-files-found: error`). |
| `web` | `core-wasm` | Downloads `core-wasm` into `ui/public/` (the notices file it writes is the checkout's, from the same commit), then in `ui/`: `pnpm install --frozen-lockfile`, `pnpm build`, the step `Write the source stamp`, which writes `dist/SOURCE.txt` (2.13), `pnpm check:dist`, then checks that `dist/core/partsmith_core.wasm` and `dist/core/partsmith_core.mjs` exist (`check:dist` does not look for the core); uploads `ui/dist/` as the artifact `web-dist` (retention 7 days, `if-no-files-found: error`). |
| `e2e` | `core-wasm` | Downloads `core-wasm` into `ui/public/`, then in `ui/`: `pnpm install --frozen-lockfile`, `pnpm exec playwright install --with-deps chromium`, `pnpm e2e`. On failure uploads `ui/playwright-report/` and `ui/test-results/` as the artifact `playwright-report` (retention 7 days: its traces can hold the files the page loaded, the core among them). |
| `python` | `occt-native` | Python 3.12 and 3.13 (`actions/setup-python@v7.0.0` with `python-version` `3.13` and then `3.12`, one per line), a virtual environment made with 3.12, `pip install maturin==1.15.0 pytest==9.1.1`, `maturin develop --release` in `python/`, `pytest python/tests`, then `pytest python/tests -m packaging` (python-api.md test 76: `maturin build --release` gives one abi3 wheel, which installs and imports on `python3.12` and `python3.13`). The wheel build uses the profile and features of `maturin develop --release`, so Cargo reuses the engine crates that step compiled; at most `pyo3` and `partsmith-py` compile again. |
| `desktop` | `occt-native` | The Tauri Linux packages, `pnpm -C ui install --frozen-lockfile`, `pnpm -C ui build` (no WebAssembly core: the desktop app does not load it), `tauri-cli` from the cache or `cargo install`, `cargo tauri build --bundles deb` in `desktop/src-tauri`, then the package check of section 6, test 12. |
| `deploy` | all jobs above | The Pages deploy of 3.3, on `pull_request` and on pushes to `main`. |

Every job sets `timeout-minutes`, so a hung job ends long before GitHub's default of 360 minutes: 150 for `occt-native` and `occt-wasm`, which build OCCT on a cache miss (the cold builds of 2026-10-13 took 56 and 88 minutes), and 60 for every other job, nearly four times the longest run of any of them so far (`desktop`, 16 minutes on 2026-10-13). Every job that needs OCCT restores its cache with `actions/cache/restore@v6.1.0` and `fail-on-cache-miss: true`, as `rust` and `kernel-wasm` do today. Every job that runs cargo uses `Swatinem/rust-cache@v2.9.2` after `rustup show active-toolchain || rustup toolchain install`. Every job that runs pnpm sets up pnpm with `package_json_file: ui/package.json` and Node 22 with the pnpm cache keyed on `ui/pnpm-lock.yaml`, as `ui` does today.

The `deploy` job, exactly in structure:

```yaml
deploy:
  name: Deploy to Cloudflare Pages
  needs: [kernel-wasm, rust, deny, ui, schema, web, e2e, python, desktop]
  if: github.event_name == 'pull_request' || github.ref == 'refs/heads/main'
  runs-on: ubuntu-24.04
  timeout-minutes: 60
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
      name: Check the source stamp
      run: |
        if ! grep -qxF "Commit: $GITHUB_SHA" ui/dist/SOURCE.txt 2>/dev/null; then
          echo "::error title=Source stamp missing::ui/dist/SOURCE.txt does not name the commit $GITHUB_SHA, so nothing was deployed. The web job writes it (docs/spec/delivery.md section 2.13)."
          exit 1
        fi
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

Triggers: `push` of tags matching `v[0-9]+.[0-9]+.[0-9]+`, and `workflow_dispatch` (a dry run that builds the bundles without a release). Workflow `permissions: contents: read`; `concurrency` `release-${{ github.ref }}` with `cancel-in-progress: false`. `timeout-minutes` is 150 for `bundle`, above the cold release budget of section 5 (120 minutes, OCCT built on the runner), so a slow first build is not cut off while a hung job still ends long before GitHub's default of 360 minutes; and 60 for `verify` and `draft`.

| Job | Needs | Runs on | What it runs |
| --- | --- | --- | --- |
| `verify` | | `ubuntu-24.04` | For a tag only (`permissions: contents: read, actions: read`), with `shell: bash`, whose `pipefail` makes a failing `cargo metadata` fail the step instead of reading as an empty version: the tag without `v` is compared separately with the version of `partsmith-desktop` (`cargo metadata --no-deps --format-version 1`) and with the version of `ui/package.json`, and each wrong version is reported once with the message of section 4 (a wrong version both files share gives one message); and `gh run list --workflow ci.yml --commit "$GITHUB_SHA" --status success --limit 1 --json databaseId --jq length` prints `1`. |
| `bundle` | `verify` | matrix of 3.6 | OCCT native from its cache or built, the UI build, `tauri-cli`, `cargo tauri build --bundles <matrix bundles>`, copies the bundle files into one flat `bundles/` folder, and uploads it as the artifact `bundle-<runner.os>`, kept 7 days (3.6, step 6). `fail-fast: false`. |
| `draft` | `bundle` | `ubuntu-24.04` | For a tag only (`permissions: contents: write`): checks out the tagged commit, downloads every `bundle-*` artifact into `release/` (merged, so `release/` holds the flat bundle files), copies `THIRD-PARTY-NOTICES.txt` into `release/` (2.9), runs `scripts/occt-source-archive.sh release`, which writes the OCCT source archive `release/occt-{OCCT_VERSION}-source.tar.gz` (2.13), writes `release/SHA256SUMS` (`sha256sum` of each file, sorted by name, so it covers the notices and the archive too), and runs `gh release create "$GITHUB_REF_NAME" --draft --verify-tag --title "Partsmith $VERSION" --generate-notes release/*`. |

### 2.6 Distribution check (`ui/scripts/check-dist.mjs`)

An ES module with no dependencies beyond Node's standard library and the notices generator, from which it takes one constant, usable as a library and as a command:

```js
/** Cloudflare Pages' per-file limit: 25 MiB. */
export const PAGES_FILE_LIMIT = 26_214_400

/** The initial JavaScript limit of ui.md section 5: 250 KB gzipped, with 1 KB = 1,000 bytes. */
export const INITIAL_JAVASCRIPT_LIMIT = 250_000

/** The exact text of `_headers` (2.7). */
export const EXPECTED_HEADERS = '/*\n  Cross-Origin-Opener-Policy: same-origin\n  Cross-Origin-Embedder-Policy: require-corp\n'

/** The first line of the web app's notices (2.9), defined once in the generator. */
export { WEB_NOTICES_HEADING } from './notices.mjs'

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
 * `_headers`, then the notices, then `initialJavaScriptProblems(directory)`; only the missing-directory message when
 * `directory` does not exist; empty when it is ready.
 */
export async function checkDist(directory)
```

Command: `node scripts/check-dist.mjs [directory]` (default `dist`, relative to the working directory). `ui/package.json` gains `"check:dist": "node scripts/check-dist.mjs dist"` and `"test:scripts": "node --test \"scripts/**/*.test.mjs\""`.

- Exit 0 and two lines on standard output, `{directory} is ready for Cloudflare Pages: {n} files, the largest is {path} ({size} bytes).` and `Initial JavaScript: {total} bytes gzipped, {percent} percent of the limit of 250000 bytes.`, when `checkDist` returns no problems. `{total}` is the sum of the `gzippedSize` values of `initialJavaScript(directory).files` (0 when there are none) and `{percent}` is `(total * 100 / INITIAL_JAVASCRIPT_LIMIT).toFixed(1)`.
- Exit 1 with every problem on standard error, one per line, when it returns any.
- Exit 2 with `usage: node scripts/check-dist.mjs [directory]` when given more than one argument.

`build-wasm.sh` keeps its informational size line and warning; the gate is this script.

**Notices.** The build must hold the web app's notices (2.9): a regular file `THIRD-PARTY-NOTICES.txt` directly in `directory` (not a symbolic link), whose first line, the text before the first LF, equals `WEB_NOTICES_HEADING`. Vite copies it from `ui/public`. A missing file gives the missing-notices message of section 4, and any other first line, including the desktop heading, the wrong-notices message. The check reads only the first line; whether the content is current is the job of `check:notices` in CI (2.9).

**Initial JavaScript** is the JavaScript a browser downloads and runs before the app is usable (ui.md 3.1 and 5). In `directory` it is:

1. Every file that the regular file `index.html` names in the `src` of a `<script>` tag or in the `href` of a `<link>` tag whose `rel` is `modulepreload`. Vite writes a `modulepreload` link for every chunk the entry script imports statically, directly or through other chunks, so these tags cover the whole static import graph; `vite.config.ts` keeps Vite's default `build.modulePreload` for that reason (test 32). Tags are read as Vite writes them: lowercase tag and attribute names, attribute values in double quotes, attributes in any order. An inline script (a `<script>` without `src`) is part of `index.html` and does not count, and neither its text nor the text of an HTML comment is read for tags. Without a regular file `index.html`, nothing is read.
2. The core worker script: every regular file whose path starts with `assets/core.worker-`, ends with `.js` and contains no further `/`. That is the name Vite gives `src/core/core.worker.ts` (its default `assets/[name]-[hash].js`). The app starts the worker during startup (ui.md 3.1), before it is usable, so its script counts; a change that renames the worker entry or its output name changes this pattern and test 32 too.

A reference of `index.html` is a file of the build when, with one leading `/` or `./` removed, it equals the `path` of a regular file of `directory` as `oversizedFiles` writes paths. Any other reference is unresolved: a URL with a scheme or starting with `//`, a missing file, a folder, a symbolic link, or a path with a query or a fragment. Each file counts once, even when it is named twice or is both referenced and the worker script.

Not initial: the core module under `core/` (`partsmith_core.mjs` and `partsmith_core.wasm`), which the worker loads and ui.md 5 excludes (3.5 gates its size); source maps; stylesheets; and every other `.js` file, such as a chunk Vite splits off for a dynamic `import()`.

The measure is the sum of each file's gzipped length, each file compressed on its own (it is a separate response) with `gzipSync` at level 9, zlib's highest, the same as `gzip -9`. 1 KB is 1,000 bytes, as in the sizes Vite prints after a build, so the limit is 250,000 bytes, and a total of exactly 250,000 bytes passes. Pages compresses responses itself, with its own encodings and levels, so the measure is a fixed and reproducible stand-in for the transfer size, not the number of bytes served.

**Measured (2026-10-13, a `vite build` of the current tree):** `index.html` names one script, `assets/index-*.js` (756,011 bytes, 232,509 gzipped), and no `modulepreload` link, because the app is one chunk; `assets/core.worker-*.js` is 7,286 bytes, 2,839 gzipped. The initial JavaScript is 235,348 bytes gzipped, 94.1 percent of the limit and 14,652 bytes under it (open issue 8). The gzip column of Vite's build report comes from another compressor and reads about 1 percent higher (235.11 kB for `assets/index-*.js`); the check's number is the one the limit applies to.

### 2.7 `ui/public/_headers`

Exactly `EXPECTED_HEADERS` (90 bytes, LF line endings, trailing newline). Vite copies it to `ui/dist/_headers`, and Pages applies it to every path. Both headers are needed for `SharedArrayBuffer` and WebAssembly threads (technical design section 4). Fonts are same-origin files of the build (ui.md 2.16.7, ADR 0031), so they need no CORP header. `vite.config.ts` sets the same two headers for `pnpm dev` and `pnpm preview`, so local runs and Playwright behave like production.

### 2.8 Examples corpus

This section settles the layout that features.md open issue 3 and cli.md open issue 3 left to it.

**The rule.** Every `.ptsm` file under `examples/` is an example. Next to `examples/<path>/<stem>.ptsm`:

- `<stem>.expected.json` holds its expected results (required).
- `<stem>.py` is its Python twin (required exactly when the example qualifies, see below; forbidden otherwise).
- `assets/` holds the files its features import, named by content hash (cli.md 2.3), shared by the examples in that folder.

Allowed locations: `examples/<type>/<stem>.ptsm`, where `<type>` is a feature type registered by `Registry::with_builtins()` and `<stem>` matches `[a-z0-9]+(-[a-z0-9]+)*` (the 165 existing examples, which `crates/partsmith-recompute/tests/feature_*.rs` load by path, all follow it and keep their names); `examples/references/<stem>.ptsm` for scenarios of [the reference stability corpus](../design/reference-stability-corpus.md), which span several feature types, each the scenario's document after its edit with the references its setup picked (`references/line-redrawn` is R06 (a), `references/slot-inserted` is R17 (a) and `references/notch-replaces-line` is R66, [the reference stability study](../design/reference-stability-and-errors.md) section 10.3 item 11; `references/moved-face-sketch` is R32 (a) and `references/sketch-after-cut` is R33, its section 10.4 item 6, which adds R38, a scenario about one feature type, as `linear_pattern/leaning-edge`; its section 10.5 item 8 adds R43 (b) as `hole/deleted-point`, the hole alone, since the error of the scenario's chamfer would forbid the twin), with `<stem>` matching the same pattern; and `examples/bench/<name>/model.ptsm` for benchmark documents (`examples/bench/fifty-features/model.ptsm`, engine.md section 5), with `<name>` matching the same pattern. No other files exist under `examples/` outside `assets/` folders. Examples do not set the rollback marker (rollback is covered by engine.md, recompute.md and python-api.md tests), so every feature of an example is built.

**Expected results.** `<stem>.expected.json` is the `partsmith rebuild --checks --json` report of the example (cli.md 3.3) with two members removed: the top-level `path` (it depends on where the command runs) and every finding's `triangles` (triangle ranges depend on the display tessellation; the finding's `faces`, `location`, `value` and `limit` stay). Everything else stays in the report's order: `printer`, `features` (each with `id`, `type`, `status`, `messages`), `parameters`, `bodies` (volume, area, center of mass, bounding box and face, edge and vertex counts, or `empty`) and `checks` (summary and per-body reports). The file is pretty JSON with two-space indentation and one trailing newline, as the CLI prints it.

**Qualifying for a Python twin.** An example qualifies when it has no `import` feature (a script cannot carry asset bytes, features/import.md) and no feature in its expected results has status `error` (a feature function that fails raises `FeatureError`, python-api.md 3.5). These are the conditions of python-api.md 3.9's first form.

**Rust runner (`crates/partsmith-cli/tests/examples.rs`).** One integration test file, a module of the CLI's one test binary, run by `cargo test --workspace` (and alone by `cargo test -p partsmith-cli --test integration examples::`). It finds the repository root from `CARGO_MANIFEST_DIR` and runs the built binary (`CARGO_BIN_EXE_partsmith`) with the repository root as working directory. Its tests:

1. `examples_follow_the_layout`: every file under `examples/` outside `assets/` is an allowed `.ptsm`, `.expected.json` or `.py` path; every `.expected.json` and `.py` has its `.ptsm`; no example has a `rollback` member.
2. `examples_are_canonical`: `partsmith fmt --check` over every example exits 0.
3. `every_feature_type_has_an_example`: for every type in `Registry::with_builtins().types()` (through `partsmith_engine::Session::new(SessionOptions::headless()).registry()`), some example under `examples/<type>/` has a feature record of that type.
4. `examples_rebuild_as_expected`: for each example, in parallel over `std::thread::available_parallelism()` workers, runs `partsmith rebuild --checks --json examples/<path>/<stem>.ptsm`, accepts exit status 0 or 1 (1 means a feature failed, which the expected results record), removes `path` and the findings' `triangles`, and compares the result with `<stem>.expected.json` by the rules below. It also checks that `<stem>.py` exists exactly when the example qualifies.
5. The comparison rules' own tests (section 6, tests 20 to 25), on literal JSON.

Each test collects every problem before failing and fails once, listing the problems sorted by path, one per line (section 4).

**Comparison.** Two JSON values match when they have the same type; objects have the same set of keys and matching values; arrays have the same length and matching elements; strings, booleans and `null` are equal; two numbers written as integers (no fraction or exponent) are equal; any other two numbers `actual` and `expected` satisfy `|actual - expected| <= max(1e-6 * |expected|, 1e-6)`. The tolerance is python-api.md's 1e-6 relative for volumes and areas with an absolute floor of `LINEAR_TOLERANCE` (1e-6) for values near zero; it absorbs floating-point differences between platforms and compilers while catching any real change. A mismatch is reported at its JSON Pointer.

**Regenerating expected results.** After an intended change, run `PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test integration examples::`. With `PARTSMITH_BLESS` set to `1`, `examples_rebuild_as_expected` writes `<stem>.expected.json` for every example whose file is missing or does not match, prints `blessed examples/<path>/<stem>.expected.json` for each, and passes; missing Python twins are listed but do not fail (the Python bless below adds them). Layout, canonical form and coverage still fail. When the environment variable `CI` is set, a set `PARTSMITH_BLESS` fails the run with `PARTSMITH_BLESS is set in CI. Bless expected results locally and commit them.` Without `PARTSMITH_BLESS`, a missing expected file fails with `examples/<path>/<stem>.expected.json is missing. Run: PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test integration examples::`. Blessed files are reviewed in the pull request like code; a kernel upgrade that changes them needs the sign-off of technical design section 16.

**Python twin checks (`python/tests/test_examples.py`).** Run by `pytest python/tests` in the `python` job. They find the repository root from the test file's location. For each example with a twin:

1. `run_script(<stem>.py)` returns exactly one document, and its `to_text()` equals the example's `.ptsm` text byte for byte, so the twin builds the identical model.
2. python-api.md 3.9's second form holds for the twin: `D1 = run_code(twin)`, `D2 = run_code(D1.to_python())`, `D2.to_text() == D1.to_text()`.
3. python-api.md 3.9's first form holds for the example's `.ptsm` text.

A twin may be the generated script or a hand-edited, more readable one, as long as check 1 holds. With `PARTSMITH_BLESS=1` (refused under `CI` as above), for every qualifying example (by its `.expected.json`) whose twin is missing or fails check 1, the test writes the generated script, `Document.open(<stem>.ptsm).to_python()`, to `<stem>.py`, prints `blessed examples/<path>/<stem>.py`, and passes.

**Adding an example.** Write the `.ptsm` (canonical: `partsmith fmt`), run `PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test integration examples::`, then `PARTSMITH_BLESS=1 pytest python/tests/test_examples.py` when it qualifies, review the new files, and commit them together.

### 2.9 Third-party notices

Every artifact Partsmith distributes carries the notices of the third-party software in it (ADR 0033): the license texts of every Rust crate and npm package it is built from, the two self-hosted fonts and the other libraries linked into it, and, for OCCT, its LGPL 2.1 text and exception, the notices of the code it includes from other projects, and a statement of how it is linked, where its source is and how to relink it. OCCT is linked statically (ADR 0003), and LGPL 2.1 section 6 is to be met with published source (ADR 0034, option A): the public repository, the source stamp of every web deployment and the OCCT source archive of every release (2.13), which the statements below name; open issue 15 records where the web still falls short.

**Files.** Two generated files, committed, in UTF-8 with LF line endings and one final newline:

| File | Artifact | Ships |
| --- | --- | --- |
| `THIRD-PARTY-NOTICES.txt` (repository root) | The desktop app for Linux (`x86_64-unknown-linux-gnu`), Windows (`x86_64-pc-windows-msvc`) and macOS (`aarch64-apple-darwin`), root crate `partsmith-desktop` | As a resource of every bundle (desktop.md 2.8), and as an asset of every release next to `SHA256SUMS` (2.5) |
| `ui/public/THIRD-PARTY-NOTICES.txt` | The web app, whose core is built for `wasm32-unknown-emscripten` from root crate `partsmith-wasm` | At `/THIRD-PARTY-NOTICES.txt` of every deployment: Vite copies it into `ui/dist`, and `check:dist` requires it (2.6) |

The desktop app embeds `ui/dist`, so it also carries the web file among its assets, where nothing shows it.

**Commands.** `ui/scripts/notices.mjs` is an ES module that uses only Node's standard library and the exports of `check-licenses.mjs` (2.10). Like `check-dist.mjs`, it runs its command only when it is the script Node started, so importing it has no side effect. `ui/package.json` gains `"notices": "node scripts/notices.mjs write"` and `"check:notices": "node scripts/notices.mjs check"`, both run in `ui/`:

- `write` writes both files and prints `Wrote THIRD-PARTY-NOTICES.txt and ui/public/THIRD-PARTY-NOTICES.txt.`
- `check` generates both texts in memory and compares each with its committed file byte for byte. It exits 0 and prints `THIRD-PARTY-NOTICES.txt and ui/public/THIRD-PARTY-NOTICES.txt are up to date.` when both match, and exits 1 with the out-of-date message of section 4 for each file that differs or is missing, in the order of the table above.
- Any other argument list exits 2 with `usage: node scripts/notices.mjs write|check`.
- A failing input (section 4) exits 1 with its message, and `write` then writes nothing.

Both commands need `cargo` and `pnpm` on `PATH` and `ui/node_modules` installed (`pnpm install --frozen-lockfile`). They need no OCCT install: every text they read is in the repository or in a package folder. The module finds the repository root from its own location and runs every command through one function, `run(command, args, directory)`, which resolves to the command's standard output and which the tests replace; the command passes `runCommand` of `check-licenses.mjs`.

```js
/** The first line of each notices file. */
export const DESKTOP_NOTICES_HEADING = 'Third-party notices for the Partsmith desktop app'
export const WEB_NOTICES_HEADING = 'Third-party notices for the Partsmith web app'

/** The desktop triples with the platform names the notices use, in this order. */
export const DESKTOP_TARGETS = [
  { triple: 'x86_64-unknown-linux-gnu', platform: 'Linux' },
  { triple: 'x86_64-pc-windows-msvc', platform: 'Windows' },
  { triple: 'aarch64-apple-darwin', platform: 'macOS' },
]

/**
 * Nested license files of code that no artifact builds (input 3), as `{ crate, path, reason }`, with `path` relative
 * to the crate's folder: a file, or a folder ending in `/` for every file below it.
 */
export const EXCLUDED_LICENSE_FILES = [
  {
    crate: 'libdbus-sys',
    path: 'vendor/dbus/',
    reason: 'the vendored libdbus, compiled only with the vendored feature, which no Partsmith build enables',
  },
]

/** The crates allowed to ship prebuilt libraries (input 8), as `{ crate, reason }`. */
export const PREBUILT_LIBRARY_CRATES = [
  {
    crate: 'webview2-com-sys',
    reason: 'links the WebView2 SDK loader statically under MSVC; the loader has its own entry (input 6)',
  },
  {
    crate: 'windows_x86_64_msvc',
    reason: 'import libraries that the windows-rs project generates, under the crate license',
  },
]

/** `text` normalized as below; '' when nothing is left. */
export function normalizeText(text)

/**
 * The external crates that `rootName` builds in one parsed `cargo metadata` output (input 1), each once as
 * `{ name, version, license, licenseFile, folder, repository }`, sorted by name and then by version precedence.
 */
export function artifactCrates(metadata, rootName)

/**
 * The paths of the license files of a package folder (input 3), relative to it with `/`, in order, without the
 * paths `excluded` names (written as the `path` of EXCLUDED_LICENSE_FILES).
 */
export async function licenseFiles(folder, licenseFile, excluded = [])

/** The prebuilt libraries of a package folder (input 8), relative to it with `/`, in byte order. */
export async function prebuiltLibraries(folder)

/** The text of one notices file from its parts, in the layout below, with no input or output. */
export function renderNotices(notices)

/** Both texts as `{ desktop, web }`, read from the repository at `root`, with every command sent through `run`. */
export async function generateNotices(root, { run })
```

`renderNotices` takes `{ heading, scope, partsmith, occt, fonts, crates, packages, libraries }`. `partsmith` is the text of `LICENSE`; `occt` is `{ statement, entries }`; `occt.entries` and the other four are lists of entries `{ name, version, license, url, platforms, textSource, texts }`, where `texts` holds the entry's normalized license texts in order, `platforms` the desktop platforms whose graph holds it (`null` in the web file), and `textSource` where its texts come from: `shipped` (from the package itself or a file inputs 4 to 7 name), `standard` or `repository` (input 3).

**Inputs.** Every input is fixed by committed files:

1. **Crates.** For each triple, `cargo metadata --format-version 1 --locked --all-features --filter-platform <triple>` in the repository root, which is the graph `cargo-deny` checks (`deny.toml` sets `[graph] all-features`). An artifact's crates are the packages with a non-null `source` that `resolve` reaches from its root crate through dependencies whose `dep_kinds` include a normal one (`kind` null) or a `build` one. Dev-dependencies are left out, and the workspace crates are Partsmith itself. Release bundles build on their own operating system, so the target filter is right for build dependencies too. The desktop crates are the union over the three triples, each with the platforms whose graph holds it. The root crate's `repository` names the Partsmith repository in the statements.
2. **npm packages.** `shippedPackages` of 2.10: the production packages that `pnpm licenses list --prod --json` lists in `ui/`, and the build packages of `EMITTED_BUILD_PACKAGES` whose code the build writes into `ui/dist`, each name and version once. Both files list them, since the desktop app embeds the same `ui/dist`.
3. **License texts of a crate or package**, in this order, each group in byte order of paths and each file once: the file its manifest names as `license_file` (crates only); every regular file directly in its folder (the folder of `manifest_path`, or the npm package's path) whose name starts with `LICENSE`, `LICENCE`, `COPYING`, `COPYRIGHT`, `NOTICE`, `UNLICENSE`, `THIRD-PARTY`, `THIRD_PARTY` or `THIRDPARTY` in any letter case (the last three for the notices of code a package bundles from other projects, such as the rollup and esbuild texts of `rolldown`'s `THIRD-PARTY-LICENSE`); every regular file directly in a folder `LICENSES` there; and every other regular file below the folder, at any depth, whose name starts with one of those prefixes and does not end in `.rs`, `.c`, `.h`, `.map` or a JavaScript or TypeScript extension (`.js`, `.mjs`, `.cjs`, `.jsx`, `.ts`, `.mts`, `.cts`, `.tsx`), unless `EXCLUDED_LICENSE_FILES` names it for that crate. Those are source code, such as the icon `dist/esm/icons/copyright.mjs` of `lucide-react`. Symbolic links are not followed. Nested files hold the licenses of code or data inside a crate, such as the Unicode data license of `regex-syntax` in `src/unicode_tables/LICENSE-UNICODE`. A package with none of these files gets one of two kinds of text, and its entry says which:
   - **Standard text**, when `Apache-2.0` or `MPL-2.0` alone satisfies its expression (the whole expression, or an `OR` alternative at any position), `Apache-2.0` first: that license's text from `ui/scripts/notices/licenses/<identifier>.txt`, a committed copy of the SPDX License List's plain text. These two texts have no copyright line to fill in.
   - **Text from its repository** otherwise: every regular file in `ui/scripts/notices/crates/<name>/`, in byte order of names, a committed copy of the license file in the crate's repository, kept by name so that a version update keeps it. A copy of a Markdown or JSON file gets `.txt` appended (the `LICENSE.md` of the objc2 repository is `LICENSE.md.txt`), because oxfmt formats those files under `ui/` and would change the text. The SPDX texts of MIT, BSD and Zlib leave the copyright line as a placeholder, and those licenses ask for "the above copyright notice", so only the holder's own file meets them.

   On 2026-10-16, 22 desktop crates ship no license file (ADR 0033). Twelve take a standard text: `Apache-2.0` for `defmt-parser`, `dispatch2`, `libappindicator-sys`, `objc2-app-kit`, `objc2-core-foundation`, `objc2-core-graphics`, `objc2-exception-helper`, `objc2-io-surface`, `objc2-quartz-core`, `objc2-web-kit` and `tauri-plugin`, and `MPL-2.0` for `selectors`. Ten take a text from their repository: `alloc-stdlib` (BSD-3-Clause), and `block2`, `dlopen2`, `dlopen2_derive`, `objc2`, `objc2-encode`, `objc2-foundation`, `webview2-com`, `webview2-com-macros` and `webview2-com-sys` (MIT). Every web crate ships its texts.
4. **OCCT.** `OCCT_VERSION` and `OCCT_TAG` from `scripts/occt-version.sh`, and the folder `ui/scripts/notices/occt-{OCCT_VERSION}/`, made once per OCCT pin and committed:
   - `LICENSE_LGPL_21.txt` and `OCCT_LGPL_EXCEPTION.txt`, copies of the files of those names in `share/doc/opencascade` of an install that `scripts/build-occt.sh` makes (the root of the tag's source holds the same two files).
   - `bundled.json`, an array of `{ name, license, path, text }`, one element for each piece of code from another project that the toolkits `partsmith-occt-sys/build.rs` links compile and whose license asks for its notice with binaries. `license` is an SPDX expression or `null`, `path` is the file in OCCT's source, and `text` names the file of this folder that holds its notice: the lines below, with each line's comment marker (`//`, `/*` or `*`) and the spaces after it removed.

   For 7.9.3, in this order:

   | `name` | `license` | `path` | `text`, from the lines |
   | --- | --- | --- | --- |
   | `flex, in OCCT's STEP reader (FlexLexer.h and the generated scanner)` | `null` | `src/FlexLexer/FlexLexer.h` | `FlexLexer.txt`, lines 5 to 28 |
   | `DELABELLA, in OCCT's mesher` | `MIT` | `src/BRepMesh/delabella.cpp` | `delabella.txt`, lines 3 to 24 |
   | `strtod of David M. Gay, in OCCT's number parsing` | `null` | `src/Standard/Standard_Strtod.cxx` | `strtod.txt`, lines 3 to 16 |
   | `ConvertUTF of Unicode, Inc., in OCCT's UTF iterator` | `null` | `src/NCollection/NCollection_UtfIterator.lxx` | `ConvertUTF.txt`, lines 16 to 36 |

   Not listed: the Bison skeleton of the STEP parser (`src/StepFile/step.tab.cxx`), whose special exception asks nothing, and the DejaVu Sans font of `src/Font`, compiled only with FreeType, which `build-occt.sh` turns off. Whoever adds the folder for a new pin also searches that version's sources of the linked toolkits for other third-party notices, and so does a change to the toolkits or the `-DUSE_*` options of `scripts/build-occt.sh` or to the toolkits that `crates/partsmith-occt-sys/build.rs` links, since either changes what OCCT compiles in (`USE_FREETYPE=ON` would add DejaVu Sans). A missing folder or file fails the generator.
5. **Fonts** (ADR 0031), named in the generator in this order: `Geist` `1.800`, `OFL-1.1`, `https://github.com/vercel/geist-font`, text `ui/public/fonts/Geist-OFL.txt`; `JetBrains Mono` `2.304`, `OFL-1.1`, `https://github.com/JetBrains/JetBrainsMono`, text `ui/public/fonts/JetBrainsMono-OFL.txt`.
6. **Other libraries**, in this order:
   - In both files: `Rust standard library` with the `channel` of `rust-toolchain.toml` as its version, `(MIT OR Apache-2.0) AND Unicode-3.0`, `https://www.rust-lang.org`, with two texts: the Apache License 2.0 (the text of `LICENSE`), and `ui/scripts/notices/rust-{channel}/Unicode-3.0.txt`, a copy of the toolchain's `share/doc/rust/licenses/Unicode-3.0.txt` (under `rustc --print sysroot`), for the Unicode data tables of `core` that the toolchain's `share/doc/rust/COPYRIGHT-library.html` names.
   - In the desktop file only, for Windows only: `Microsoft Edge WebView2 SDK loader, in webview2-com-sys` with the locked version of that crate, `BSD-3-Clause`, `https://www.nuget.org/packages/Microsoft.Web.WebView2`, text `ui/scripts/notices/webview2-com-sys-{version}/LICENSE.txt`, a copy of the `LICENSE.txt` of the `Microsoft.Web.WebView2` NuGet package whose `WebView2LoaderStatic.lib` that crate version ships and links statically under MSVC. The crate declares only MIT, which covers its bindings, and ships no text for the loader.
   - In the web file only, with `{emsdk}` the version in `scripts/emsdk-version.sh`: `Emscripten runtime` `{emsdk}` (the JavaScript of `core/partsmith_core.mjs` and the system libraries in `core/partsmith_core.wasm`), `MIT OR NCSA`, `https://emscripten.org`, text `ui/scripts/notices/emscripten-{emsdk}/LICENSE`; `musl libc, in the Emscripten runtime` with no version, `MIT`, `https://musl.libc.org`, text `ui/scripts/notices/emscripten-{emsdk}/musl-COPYRIGHT`; and `LLVM libc++, libc++abi, libunwind and compiler-rt, in the Emscripten runtime` with no version, `Apache-2.0 WITH LLVM-exception AND (NCSA OR MIT)`, `https://llvm.org`, texts `ui/scripts/notices/emscripten-{emsdk}/libcxx-LICENSE.TXT`, `libcxxabi-LICENSE.TXT`, `libunwind-LICENSE.TXT` and `compiler-rt-LICENSE.TXT`. Each LLVM file holds the Apache 2.0 text with the LLVM exception and the NCSA and MIT texts that still cover older code, which ask for their notice with binaries.

   The Emscripten files are copies of the SDK's `upstream/emscripten/LICENSE`, `upstream/emscripten/system/lib/libc/musl/COPYRIGHT` and `upstream/emscripten/system/lib/{libcxx,libcxxabi,libunwind,compiler-rt}/LICENSE.TXT`. A new Rust, Emscripten or `webview2-com-sys` version needs new copies, or the generator fails.
7. **Partsmith's own license**: the root `LICENSE`.
8. **Prebuilt libraries.** Every crate of input 1 is searched for regular files ending in `.lib`, `.a`, `.dll`, `.so` or `.dylib` below its folder, outside any folder named `tests` (`system-deps` keeps a test library in `src/tests/lib`). A crate that ships one and is not in `PREBUILT_LIBRARY_CRATES` fails the generator: a prebuilt library is under the license of whoever built it, which the crate's own license may not cover.

**Normalized text.** A license text is decoded as UTF-8, with U+FFFD for an invalid byte sequence. Then a leading U+FEFF is removed, CRLF and a lone CR become LF, form feeds are removed, spaces and tabs at the end of each line are removed, blank lines at the start and the end are removed, and one LF ends the text. Two texts are the same text when their normalized forms are equal. Nothing else changes: the texts keep every other character as their packages ship it, including the em dash that AGENTS.md's style rules keep out of the project's own text (ADR 0033 proposes the wording that allows it, section 8).

**Layout.** In this order, with `{...}` filled in and the blank lines shown:

```text
{heading}

Generated by pnpm -C ui notices from the lock files (docs/spec/delivery.md section 2.9). Do not edit it by hand.

Partsmith is licensed under the Apache License 2.0, text [1]. Models you make with Partsmith belong to you. {scope} also contains the software listed below, each part under its own license; the license texts follow the list.

== Open CASCADE Technology ==

{OCCT statement}

{one entry line per OCCT entry}

== Fonts ==

{one entry line per font}

== Rust crates ==

Each crate's source code is at its crates.io URL; for the crates under MPL-2.0 that is where to obtain their Source Code Form.

{one entry line per crate}

== npm packages ==

{one entry line per package}

== Other libraries ==

{one entry line per library}

== License texts ==

[1]

{text 1}

[2]

{text 2}
```

- `{heading}` is `DESKTOP_NOTICES_HEADING` or `WEB_NOTICES_HEADING`, and `{scope}` is `The Partsmith desktop app for Linux, Windows and macOS` or `The Partsmith web app`.
- An entry line is `{name}`, then ` {version}` when it has one, then `, {license}, {url}`; then `, only {platforms}` for a desktop entry that some platform lacks (`{platforms}` the platforms that hold it, in the order of `DESKTOP_TARGETS`, joined by ` and `); then `, standard text` or `, text from its repository` when its `textSource` is `standard` or `repository`; then `, texts` followed by ` [{n}]` for each of its texts. `{license}` is the package's license field as written (`MIT/Apache-2.0` stays as it is), or `see texts` when it has none. `{url}` is `https://crates.io/crates/{name}/{version}` for a crate, `https://www.npmjs.com/package/{name}/v/{version}` for an npm package, and the URL of inputs 4 to 6 for the others.
- The OCCT entries are, first, `Open CASCADE Technology {OCCT_VERSION}, LGPL-2.1-only WITH OCCT-exception-1.0, https://github.com/Open-Cascade-SAS/OCCT/tree/{OCCT_TAG}, texts [2] [3]`, then one line per element of `bundled.json`, in its order, whose URL is `https://github.com/Open-Cascade-SAS/OCCT/blob/{OCCT_TAG}/{path}`.
- Fonts and other libraries come in the order of inputs 5 and 6. Crates and npm packages are sorted by name (byte order) and then by version precedence (Semantic Versioning 2.0.0).
- Texts are numbered from 1 in the order the file first refers to them, reading from the top, so `LICENSE` is always [1], OCCT's license and exception are [2] and [3], and the notices of `bundled.json` follow. Each distinct text appears once, as a line `[{n}]`, a blank line and the normalized text, with one blank line before the next `[{n}]`; the file ends with the last text's final LF.

**The OCCT statement.** `{OCCT_VERSION}`, `{OCCT_TAG}` and `{repository}` come from the inputs; the rest is literal, and each paragraph is one line. In the desktop file:

```text
The Partsmith desktop app uses Open CASCADE Technology (OCCT) {OCCT_VERSION}, https://dev.opencascade.org, copyright OPEN CASCADE SAS and the OCCT contributors. OCCT is free software under the GNU Lesser General Public License version 2.1 with the Open CASCADE exception version 1.0. It includes code from the other projects listed after it, under their own notices. All these texts are below.

OCCT is linked statically into the app. It is the unmodified source of the tag {OCCT_TAG} of https://github.com/Open-Cascade-SAS/OCCT, built as static libraries by the script scripts/build-occt.sh in the Partsmith source, which lists the toolkits and options used.

You may modify OCCT and use your version with Partsmith. The source of each release is on its page under {repository}/releases: the Partsmith source is the release's tag, v followed by the app's version (for example v0.1.0), with every script, tool version and lock file the build uses, and the OCCT source the build reads is the release's asset occt-{OCCT_VERSION}-source.tar.gz, which scripts/occt-source-archive.sh makes from the tag {OCCT_TAG}. To relink: put your OCCT source in <work>/src (from the asset: mkdir -p <work>/src && tar -xzf occt-{OCCT_VERSION}-source.tar.gz --strip-components=1 -C <work>/src), run OCCT_WORK_DIR=<work> scripts/build-occt.sh native <prefix>, and build the app with OCCT_ROOT=<prefix> as README.md describes (pnpm -C ui build, then cargo tauri build in desktop/src-tauri). The exact commands for each platform, including the environment Windows and macOS need, are the steps of the bundle job in .github/workflows/release.yml at that tag.
```

In the web file:

```text
The Partsmith web app uses Open CASCADE Technology (OCCT) {OCCT_VERSION}, https://dev.opencascade.org, copyright OPEN CASCADE SAS and the OCCT contributors. OCCT is free software under the GNU Lesser General Public License version 2.1 with the Open CASCADE exception version 1.0. It includes code from the other projects listed after it, under their own notices. All these texts are below.

OCCT is linked statically into the app's geometry core, core/partsmith_core.wasm. It is the unmodified source of the tag {OCCT_TAG} of https://github.com/Open-Cascade-SAS/OCCT, built as static WebAssembly libraries by the script scripts/build-occt.sh in the Partsmith source, which lists the toolkits and options used.

You may modify OCCT and use your version with Partsmith. Each deployment of this site names the commit of {repository} it was built from in /SOURCE.txt, with a link to that commit's source archive; that commit holds every script, tool version and lock file the build uses. Production is built from the main branch, and a preview from the commit GitHub makes by merging its pull request into the base branch; for a preview, /SOURCE.txt also names the two commits that merge joins and its tree: if the merge commit is gone, check out the first, run git merge --no-ff with the second, and compare the tree that git log -1 --format=%T prints with that tree. The OCCT source the build reads is occt-{OCCT_VERSION}-source.tar.gz, which scripts/occt-source-archive.sh at that commit makes from the tag {OCCT_TAG}; each Partsmith desktop release built with OCCT {OCCT_VERSION} carries that file as an asset under {repository}/releases. To relink: put your OCCT source in <work>/src (from the archive: mkdir -p <work>/src && tar -xzf occt-{OCCT_VERSION}-source.tar.gz --strip-components=1 -C <work>/src), run OCCT_WORK_DIR=<work> scripts/build-occt.sh wasm <prefix> with the Emscripten SDK of scripts/emsdk-version.sh active, and build the core with OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN=<prefix> and then the app as README.md describes (scripts/build-wasm.sh, then pnpm -C ui build). The exact commands are the steps of the core-wasm and web jobs in .github/workflows/ci.yml at that commit.
```

**Kept current.** A pull request that changes `Cargo.lock`, a `Cargo.toml` (the root crates' `repository`), `ui/pnpm-lock.yaml`, `rust-toolchain.toml`, `scripts/occt-version.sh`, `scripts/emsdk-version.sh`, a font license text, `LICENSE`, `ui/scripts/notices.mjs`, `ui/scripts/check-licenses.mjs` or a file under `ui/scripts/notices/` runs `pnpm -C ui notices` and commits both files with the change; otherwise `check:notices` fails the `schema` job (2.4). A pull request that changes the toolkits or `-DUSE_*` options of `scripts/build-occt.sh`, or the toolkits of `crates/partsmith-occt-sys/build.rs`, repeats the search of input 4 and updates `bundled.json` with what it finds; no check catches that one, since the generator does not read those files. The statements name what 2.13 makes, `/SOURCE.txt`, `occt-{OCCT_VERSION}-source.tar.gz` and `scripts/occt-source-archive.sh`, so a change to one of those names changes the statements in the same pull request (test 57). `ui/AGENTS.md` lists `notices`, `check:notices` and `check:dist` among its commands. Since other changes read only the root `AGENTS.md`, its Commands block gains the line `pnpm -C ui notices   # after changing Cargo.lock, a Cargo.toml, ui/pnpm-lock.yaml, rust-toolchain.toml or scripts/*-version.sh` when this work merges `main`, where the instruction files are named AGENTS.md; that file is the owner's, so the owner adds the line (section 8). The release asset is the file of the tagged commit, whose CI passed (`verify`, 2.5).

**Not covered** (open issue 10): the system libraries and the runtime that the AppImage bundles (desktop.md 3.8), the code the NSIS installer adds, and the CLI and the Python wheel, which M1 does not distribute (1.2).

### 2.10 npm license check (`ui/scripts/check-licenses.mjs`)

An ES module with no dependencies beyond Node's standard library, usable as a library and as a command; like `check-dist.mjs`, it runs its command only when it is the script Node started. It holds the npm packages that ship to the license allow list that `cargo-deny` applies to crates, so the policy has one source, `deny.toml` (ADR 0033). The packages that ship are the production dependencies of `ui/package.json` and theirs, and the development dependencies whose code the build writes into `ui/dist`. The other development dependencies are not checked: they do not ship.

```js
/**
 * The development dependencies whose code the build writes into `ui/dist`: Rolldown's CommonJS interop helpers at the
 * top of `assets/index-*.js`, Vite's module preload polyfill (test 32), and Tailwind's CSS with its license banner.
 */
export const EMITTED_BUILD_PACKAGES = ['rolldown', 'tailwindcss', 'vite']

/** The strings of the `allow` array of the `[licenses]` table of `denyToml` (the text of deny.toml), in order. */
export function allowedLicenses(denyToml)

/** Whether the SPDX license expression `expression` is satisfied by the identifiers `allowed` (below). */
export function licenseAllowed(expression, allowed)

/**
 * The packages of `listing`, the standard output of `pnpm licenses list --json` with or without `--prod`, as
 * `{ name, version, license, path }`, one per name and version, sorted by name and then by version precedence.
 */
export function productionPackages(listing)

/**
 * The packages that ship: `productionPackages(production)` and, from `productionPackages(all)`, every version of each
 * name of EMITTED_BUILD_PACKAGES, each name and version once, sorted as `productionPackages` sorts. `production` and
 * `all` are the standard outputs of `pnpm licenses list --prod --json` and `pnpm licenses list --json`. Throws an
 * `Error` with the emitted-package message of section 4 for the first name, in list order, that `all` lacks.
 */
export function shippedPackages(production, all)

/** One message (section 4) for each package whose license `licenseAllowed` rejects, in the order of `packages`. */
export function licenseProblems(packages, allowed)

/** Orders two strings by code units, the same in every locale; the notices sort paths and names with it. */
export function compareText(first, second)

/** Orders `{ name, version }` records by name (byte order) and then by version precedence; the notices sort crates with it. */
export function compareNameAndVersion(first, second)

/**
 * Runs `command` with `args` in `directory` and resolves to its standard output; rejects with an `Error` whose
 * `status` is the exit status (or why it did not start) and whose `stderr` is its standard error.
 */
export function runCommand(command, args, directory)

/** The command-failure message of section 4 for a rejection of `runCommand`, shared by both scripts. */
export function commandFailureMessage(command, args, error)
```

Command: `node scripts/check-licenses.mjs` in `ui/` (`ui/package.json` gains `"check:licenses": "node scripts/check-licenses.mjs"`). It reads `deny.toml` from the repository root, found from the module's location, runs `pnpm licenses list --prod --json` and `pnpm licenses list --json` in `ui/`, and checks `shippedPackages` of the two:

- Exit 0 and `{n} npm packages that ship, each under a license deny.toml allows.` when there is no problem.
- Exit 1 with every problem on standard error, one per line, or with the message of section 4 when `deny.toml` has no allow list, a package of `EMITTED_BUILD_PACKAGES` is not installed, or `pnpm` fails.
- Exit 2 with `usage: node scripts/check-licenses.mjs` when given any argument.

**Reading `deny.toml`.** The `[licenses]` table runs from its header line to the next table header. Its `allow` key holds one array of double-quoted strings, which may span lines, hold `#` comments and end with a trailing comma. Anything else (no `[licenses]` table, no `allow` key in it, an unterminated array, an element that is not a double-quoted string) throws an `Error` with the deny.toml message of section 4. The repository's list is `MIT`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, `Unicode-3.0`, `CC0-1.0`, `MPL-2.0` and `BSL-1.0`.

**Expressions** (SPDX license expressions). Spaces and parentheses separate tokens: `(`, `)`, the operators `AND`, `OR` and `WITH` (upper case only), and identifiers made of letters, digits, `.`, `-` and `+`. `WITH` binds tighter than `AND`, and `AND` tighter than `OR`; parentheses group. An identifier is satisfied when it equals an element of `allowed`, ignoring case; `A WITH B` when the text `A WITH B` equals an element, ignoring case (only `Apache-2.0 WITH LLVM-exception` today); `A AND B` when both are; `A OR B` when either is. Any other text is not satisfied: an empty license, `UNLICENSED`, `SEE LICENSE IN LICENSE.txt`, `Unknown` (what pnpm reports for a package without a license field), or an expression with a syntax error. The notices generator uses the same parser to find whether `Apache-2.0` or `MPL-2.0` alone satisfies an expression (2.9, input 3), by calling `licenseAllowed(expression, [identifier])`.

**Measured (2026-10-16).** Eight production packages: `react`, `react-dom`, `scheduler`, `zustand`, `clsx`, `@types/react` and `csstype` under MIT, and `lucide-react` under ISC; and three build packages whose code ships: `rolldown` 1.2.13, `tailwindcss` 4.3.3 and `vite` 8.3.4, all MIT. ADR 0033 logs the eleven. The other development dependencies include MIT-0, Python-2.0 and BlueOak-1.0.0, which the list does not allow and which do not ship.

### 2.11 Nightly workflow (`nightly.yml`)

Runs the sketch property tests (sketch.md 6.13, tests 189 to 196) with 4096 cases each, sixteen times the 256 that `cargo test` runs on every pull request, where the default keeps CI fast.

Triggers: `schedule` with the cron `23 4 * * *` (04:23 UTC, off the hour, when scheduled runs are delayed least) and `workflow_dispatch`. A scheduled run tests the head of `main`. Workflow `permissions: contents: read, actions: read`; `concurrency` `nightly-${{ github.ref }}` with `cancel-in-progress: false`; `env` `CARGO_TERM_COLOR: always`.

One job, `properties` (`name: Sketch property tests (4096 cases)`, `runs-on: ubuntu-24.04`, `timeout-minutes: 20`, so a property that never finishes ends after 20 minutes, not GitHub's default of 360), with these steps:

1. On `schedule` only, the skip check (`id: skip`, with `GH_TOKEN: ${{ github.token }}` and `GH_REPO: ${{ github.repository }}`, in the default shell `bash -e`): `gh run list --workflow nightly.yml --branch main --status completed --limit 1 --json headSha --jq '.[0].headSha // ""'` prints the commit of the newest finished nightly run on `main`, whatever its conclusion, or nothing when there is none. When it equals `$GITHUB_SHA`, the step writes `skip=true` to `$GITHUB_OUTPUT` and prints the skip notice of section 4; otherwise it writes `skip=false`. When `gh` fails, the step prints the skip-check warning of section 4 and writes `skip=false`, so the night tests: failing the step instead would record a failed run on that commit, which later nights would count as tested without a test having run. The running run has not finished, so it never counts. Every finished run counts: a skipped run concludes `success` on the same commit, so later nights keep skipping until `main` moves; a run that failed, timed out or was canceled is not repeated on the same commit, so a property that never finishes takes 20 minutes once, not every night (a `workflow_dispatch` run repeats any of them).
2. Only when `steps.skip.outputs.skip != 'true'`: `actions/checkout@v7.0.1`; `rustup show active-toolchain || rustup toolchain install`; `Swatinem/rust-cache@v2.9.2`; and `cargo test -p partsmith-sketch --release --test integration properties::` with `PROPTEST_CASES: "4096"` in its environment.

The crate is pure Rust, so the job needs no OCCT cache, no system packages and no Node. A failing property prints proptest's smallest failing case and its `cc` line; the fix commits that line to `crates/partsmith-sketch/tests/properties.proptest-regressions`, so every later `cargo test` replays it.

**Cost.** On 2026-10-16, on a 4-core machine, a release build of the sketch tests from a cold target took 2 min 26 s, the 4096-case run 1.6 s and the default 256-case run 0.1 s: the build is nearly all of the cost. With the job's Rust cache warm, a night after a change to `main` takes about 3 minutes of runner time (setup, cache restore, an incremental release build and the run), a night with a cold cache about 6, and a skipped night under 1, since the job stops after the skip check: about 90 minutes in a month with a change every day and about 30 in a quiet month. The repository is public, and standard GitHub-hosted runners are free for public repositories (ADR 0034), so none of it is billed; the skip still keeps the run from repeating work. The cron is the one place to change the cadence, for example to weekly with `23 4 * * 1`.

### 2.12 README.md

`README.md` is what a visitor reads first. It points at the specs for detail rather than repeating them, and it has these parts, in this order (m1-gap-list B-13):

1. **Title and summary**: the current heading and paragraph, unchanged.
2. **Status**: one line in place of "Status: early development (phase 0)": `> Status: milestone 1 in progress. What remains before the first release is in [the M1 gap list](docs/design/m1-gap-list.md).`
3. **Try it.**
   - The web app at `https://{subdomain}`, the production deployment of `main` (3.3), where `{subdomain}` is the `*.pages.dev` subdomain that the Cloudflare dashboard (or `wrangler pages project list`) shows for the project, as the owner confirms it. Pages fixes the subdomain when the project is created, adding a suffix to the project name when that name is taken, so it cannot be derived from `CLOUDFLARE_PAGES_PROJECT`; it is `partsmith.pages.dev` only if the dashboard says so. Until the owner confirms it, which also waits for the first production deployment (section 8), the bullet's first sentence gives no address: `The web app deploys to Cloudflare Pages from main once CI passes there; its address goes here with the first production deployment.` A pull request's preview is at `https://{branch}.{subdomain}` and is linked from the pull request. CI tests the web app in Chromium (m1-gap-list B-9 adds Firefox and WebKit).
   - The desktop app: desktop builds appear on GitHub Releases, `https://github.com/blai30/partsmith/releases`, once the first release is published, which waits for items 1 and 2 of the release checklist (3.6; until then every release is a draft, which only people with write access to the repository see, so the page holds nothing for visitors); then the files of desktop.md 3.8 and what each needs (Windows x64, where the installer fetches WebView2 when it is missing; macOS 13 or later on Apple silicon; Linux x86_64 with glibc 2.39 or later and WebKitGTK 4.1, such as Ubuntu 24.04), then the unsigned-bundle paragraph of 3.6 word for word, and `SHA256SUMS` to check a download (`sha256sum -c SHA256SUMS --ignore-missing`).
4. **Building.**
   - Prerequisites on Linux, the only platform CI builds from source (the release workflow proves Windows and macOS, open issue 1): Git, CMake, Ninja and a C++17 compiler for OCCT; rustup, which installs the Rust of `rust-toolchain.toml` with its WebAssembly target (`rustup toolchain install` in the repository); Node 22 and pnpm 10.28.0 (`corepack enable && corepack install -g pnpm@10.28.0`: corepack takes the `packageManager` of `ui/package.json` only inside `ui/`, and from the repository root, where the `pnpm -C ui` commands run, it takes its own default instead, on 2026-10-16 pnpm 12.11.2, which the corepack of Node 22.22.0 fails to start); Python 3.12 or later for the Python package; and one install line, `sudo apt-get update && sudo apt-get install -y build-essential cmake ninja-build python3-venv libwebkit2gtk-4.1-dev libxdo-dev libssl-dev librsvg2-dev` (`python3-venv` gives Ubuntu's Python its `venv` module).
   - OCCT and the workspace: once, `sudo mkdir -p /opt/occt && sudo chown "$USER" /opt/occt`, because the build ends by installing into its prefix and `/opt` belongs to root (the prefix stays the `OCCT_ROOT` of AGENTS.md); then `scripts/build-occt.sh native /opt/occt/7.9.3/native`, `export OCCT_ROOT=/opt/occt/7.9.3/native`, `cargo test --workspace`.
   - The web core: the Emscripten SDK of `scripts/emsdk-version.sh` (4.0.15) installed and activated (in a clone of `https://github.com/emscripten-core/emsdk`, `./emsdk install 4.0.15 && ./emsdk activate 4.0.15 && source ./emsdk_env.sh`), `scripts/build-occt.sh wasm /opt/occt/7.9.3/wasm`, `export OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN=/opt/occt/7.9.3/wasm`, then `scripts/build-wasm.sh`, which writes `ui/public/core/`. Without it the web app opens but its geometry core does not load.
5. **Running**, one command block each:
   - The web app: `pnpm -C ui install`, `pnpm -C ui dev`, then `http://localhost:5173`; and the production build, `pnpm -C ui build`, which writes `ui/dist` (the web statement of 2.9 points here for relinking).
   - The desktop app: once, `cargo install tauri-cli --version 2.12.2 --locked`; then `pnpm -C ui dev`, and in a second terminal `cargo tauri dev` in `desktop/src-tauri`. A bundle: `pnpm -C ui build`, then `cargo tauri build` in `desktop/src-tauri`.
   - The CLI: `cargo run -p partsmith-cli -- <command>`, with its five commands `validate`, `fmt`, `rebuild`, `export` and `schema`, one line each as cli.md section 1 describes them.
   - Python: a virtual environment, `pip install maturin pytest`, `(cd python && maturin develop --release)`, then `python -m partsmith build script.py -o part.ptsm` (python-api.md 2.1 and 2.10).
6. **Contributing**: [AGENTS.md](AGENTS.md) (rules, commands and invariants), [the spec index](docs/spec/README.md), [the ADR log](docs/adr/README.md) and the [technical design](docs/design/technical-design.md); and the regeneration commands: examples, `PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test integration examples::` and `PARTSMITH_BLESS=1 pytest python/tests/test_examples.py` (2.8); schemas, `cargo run -p partsmith-cli -- schema --write && pnpm -C ui generate:protocol` (3.4); notices, `pnpm -C ui notices` (2.9).
7. **License**: Partsmith is under the Apache License 2.0 ([LICENSE](LICENSE)), one license for the code, the specifications, the file format, the examples and the test corpus (ADR 0028), and models made with Partsmith belong to their makers. The third-party software and its licenses are in [THIRD-PARTY-NOTICES.txt](THIRD-PARTY-NOTICES.txt) for the desktop app and [ui/public/THIRD-PARTY-NOTICES.txt](ui/public/THIRD-PARTY-NOTICES.txt) for the web app. In place of README.md's current two OCCT sentences ("OCCT 7.9.3 is used ... with a modified OCCT."), the OCCT sentences are: `OCCT 7.9.3 is used under the LGPL 2.1 with the Open CASCADE exception. It is linked statically, and every build comes with a way to its source: each web deployment names the commit it was built from in /SOURCE.txt, and each release carries OCCT's source as occt-7.9.3-source.tar.gz next to its tag (ADR 0034). The notices say how to build Partsmith with a modified OCCT.` The file names may be set as code. The sentences follow ADR 0034 and 2.13, and change with them. There is no CC0 grant to scope: ADR 0028 replaced it.

The release checklist is not in `README.md`; it is in `.github/workflows/AGENTS.md` (3.6).

### 2.13 Source of each build (`SOURCE.txt`, `scripts/occt-source-archive.sh`)

Every build Partsmith distributes links OCCT statically, and ADR 0034 (option A, the owner's decision of 2026-10-16) is to meet LGPL 2.1 section 6 with published source. The repository `https://github.com/blai30/partsmith` is public, so every build's commit or tag, or for a preview its two parents, is there. This section adds what makes each build's source findable: a stamp in every web deployment that names its commit, and the OCCT source archive attached to every release. The statements of 2.9 name both. A local build carries neither, since it is not distributed.

**The source stamp.** Every build the `web` job uploads as `web-dist`, and so every deployment, previews included, holds the regular file `SOURCE.txt` at its root, served at `/SOURCE.txt`. It is UTF-8 with LF line endings and one final newline, and holds these lines, in this order:

```text
Source of this deployment of the Partsmith web app
Repository: {repository}
Commit: {commit}
Source archive: {repository}/archive/{commit}.tar.gz
```

For a build of a pull request, three more lines follow:

```text
Pull request: {pull request}
Parents: {base} {head}
Tree: {tree}
```

- `{repository}` is `${{ github.server_url }}/${{ github.repository }}`, `https://github.com/blai30/partsmith`; the source archive is GitHub's `.tar.gz` of the commit.
- `{commit}` is `$GITHUB_SHA`, the commit the run checked out and built, as 40 lowercase hexadecimal digits. On a push to `main` it is the pushed commit. On a pull request it is the merge commit GitHub makes for the pull request's checks (`refs/pull/{number}/merge`, the pull request's head merged into its base branch), which every job of the run checks out, builds and tests. Pages records the pull request's head commit for its dashboard instead (`--commit-hash`, 2.4); the stamp names what was built.
- `{pull request}` is `${{ github.event.pull_request.html_url }}`, and `{base}` and `{head}` are the two parents of `{commit}` in the order its commit object lists them: the base branch's commit, then the pull request's head. GitHub moves `refs/pull/{number}/merge` to a new merge commit whenever either side changes, so the merge commit of an older preview may no longer be on any ref and may be dropped in time, while its parents stay reachable: the base in its branch's history, the head through `refs/pull/{number}/head`, which GitHub keeps. From them the merge can be made again. `{tree}` is the merge commit's tree, so whoever makes the merge again can check that its tree is the one built.

The writer is the `web` job's step `Write the source stamp`, between the steps that run `pnpm build` and `pnpm check:dist`, in the job's working directory `ui/` and in GitHub's shell for a step without `shell`, `bash -e`. Its `env` holds `REPOSITORY_URL: ${{ github.server_url }}/${{ github.repository }}` and `PULL_REQUEST_URL: ${{ github.event.pull_request.html_url }}`, which is empty on a push; `GITHUB_SHA` is GitHub's. It writes the four lines to `dist/SOURCE.txt`, and when `PULL_REQUEST_URL` is not empty it appends the three pull request lines. The checkout is shallow, so `git log` and `git rev-parse` see no parents; the step reads the parents and the tree from the header of the commit object, the lines of `git cat-file commit "$GITHUB_SHA"` before the first blank line (the message below it may hold a line starting with `parent` or `tree`), and fails when that command fails. `check:dist` does not look for the stamp, so `pnpm build && pnpm check:dist` keeps working where nothing is deployed.

The check is the `deploy` job's step `Check the source stamp` (2.4), after `web-dist` is downloaded and before Wrangler runs: unless `ui/dist/SOURCE.txt` holds the whole line `Commit: $GITHUB_SHA`, it prints the source-stamp error of section 4 and fails the job, so no deployment goes out without a stamp that names the run's commit. The stamp and its check land in the same commit as the statements of 2.9 that name them, and so does the archive step of `draft` below.

**The OCCT source archive.** `scripts/occt-source-archive.sh <directory>` writes `<directory>/occt-{OCCT_VERSION}-source.tar.gz`, the OCCT source that `scripts/build-occt.sh` reads, from the tag that `scripts/occt-version.sh` pins (`{OCCT_VERSION}` and `{OCCT_TAG}` come from that file, as in 2.9). Like `build-occt.sh`, it runs with `set -euo pipefail` and sources `occt-version.sh` from its own folder.

- **Clone.** It reads the OCCT clone at `$OCCT_WORK_DIR/src`, with `OCCT_WORK_DIR` and its default (`${TMPDIR:-/tmp}/occt-{OCCT_VERSION}`) as in `build-occt.sh`. When that folder does not exist, it first clones it exactly as `build-occt.sh` does, `git clone --depth 1 --branch {OCCT_TAG} https://github.com/Open-Cascade-SAS/OCCT.git "$OCCT_WORK_DIR/src"`, so after a local OCCT build it reuses that build's clone.
- **Pinned commit.** `scripts/occt-version.sh` also sets `OCCT_COMMIT=a016080bf6738d6aeae020badee4e888ad1540a5`, the commit the tag names (measured below). Before archiving, the script checks that `git -C "$OCCT_WORK_DIR/src" rev-parse "$OCCT_TAG^{commit}"` prints `OCCT_COMMIT`, and `scripts/build-occt.sh` makes the same check right after its own clone (not when `<work>/src` already exists, which is how a modified OCCT is built). Each exits 1 with the tag-commit message of section 4 otherwise, so a tag moved upstream stops the archive and a cold OCCT build instead of making them from a source other than that of the cached builds; each first removes the clone it made, so that the next run of either clones and checks again instead of building or archiving from it (tests 55 and 58). The change that adds the pin changes the OCCT cache keys (3.2), so OCCT is built once more for each key; it changes no notices text.
- **Contents.** The tree of the tag `{OCCT_TAG}` in that clone, never its working tree or another commit, limited to `.clang-format`, `CMakeLists.txt`, `LICENSE_LGPL_21.txt`, `OCCT_LGPL_EXCEPTION.txt`, `README.md`, `adm` and `src`, all under the folder `occt-{OCCT_VERSION}/`. With the options of `build-occt.sh`, CMake reads `CMakeLists.txt`, `adm` (its modules and templates) and `src` (the sources, the file lists of each toolkit and package, and the resources), copies `.clang-format` (its configure step fails without it), and installs the two license files; `README.md` is upstream's description of the source. The rest of the tag (`data`, `dox`, `samples`, `tests`, `tools`, `.github` and the other dot files at its root) is read only with options that `build-occt.sh` turns off.
- **Bytes.** `git -C "$OCCT_WORK_DIR/src" -c core.autocrlf=false -c tar.umask=0002 -c core.attributesFile=/dev/null archive --format=tar --prefix=occt-{OCCT_VERSION}/ {OCCT_TAG} -- .clang-format CMakeLists.txt LICENSE_LGPL_21.txt OCCT_LGPL_EXCEPTION.txt README.md adm src`, piped to `gzip -9 -n`. The three `-c` options fix what a user's or the system's Git configuration would otherwise change: the line endings of files without an `eol` attribute (`core.autocrlf`; OCCT's `.gitattributes` sets `eol` only for some patterns, and `adm/cmake/*.cmake` has none), the file modes (`tar.umask`), and attributes such as `export-ignore` from a global attributes file (`core.attributesFile`, whose default, `$XDG_CONFIG_HOME/git/attributes` or `~/.config/git/attributes`, is read even when the option is unset). `git archive` of a tag gives every entry the tagged commit's time and stores the commit's id in the archive's pax header (`git get-tar-commit-id` reads it back), and `-n` keeps the name and the time out of the gzip header, so the same tag gives the same bytes on every run with the same `git` and `gzip`, whatever the Git configuration: anyone can make the archive again from the pin and compare.
- **Output.** It writes to a temporary file in `<directory>` and renames it to the archive's name when `gzip` has finished; on any failure an `EXIT` trap removes the temporary file, so a failure leaves `<directory>` as it was. Then it prints `Wrote {directory}/occt-{OCCT_VERSION}-source.tar.gz from {OCCT_TAG}, commit {commit}.` (`{directory}` as given, `{commit}` the tagged commit's full id) and exits 0.
- **Failures.** With other than one argument, it exits 2 with `usage: scripts/occt-source-archive.sh <directory>` on standard error. When `$OCCT_WORK_DIR/src` holds no tag `{OCCT_TAG}`, or is not a Git clone (such as an extracted archive), it exits 1 with the no-tag message of section 4; when the tag names another commit than `OCCT_COMMIT`, it exits 1 with the tag-commit message. Any other failure, such as a failed clone, a missing `<directory>` or a listed path missing from the tag (a pin whose OCCT drops or renames one), exits with a status other than 0: `pipefail` makes the failure of `git archive` fail the pipeline, whose `gzip` alone would end with 0.

`draft` runs `scripts/occt-source-archive.sh release` after copying the notices and before writing `SHA256SUMS` (2.5), so the archive is an asset of every release next to `SHA256SUMS`, which covers it. The job needs only `git` and `gzip`, which the runner has; the clone takes about a minute. The archive comes from the OCCT pin of the tagged commit, the same pin the `bundle` jobs build OCCT from (3.2).

**Measured** (2026-10-16, the tag `V7_9_3`, commit `a016080bf6738d6aeae020badee4e888ad1540a5`, git 2.43.0 and gzip 1.12): `occt-7.9.3-source.tar.gz` holds 15,712 entries (15,224 files and 488 folders), 120,401,920 bytes as tar, and is 17,457,358 bytes (16.6 MiB) with SHA-256 `bb71bf7473f9a278648f3ff96e28bd50afe49bf8d6e53ab55e883b54d0220155`, the same on two runs and from a fresh clone of the upstream tag, which named the pinned commit. Without the three `-c` options, `core.autocrlf=true` (the default of Git for Windows), `tar.umask=0077` and a global attributes file holding `*.cmake export-ignore` each change the bytes; with them, each of the three gives that SHA-256 again. The whole tag, packed the same way, is 48,068,564 bytes. CMake's configure step, with the toolkits and options of `build-occt.sh`, succeeds on the extracted archive; without `.clang-format` it fails (`file COPY cannot find .../.clang-format`, `CMakeLists.txt` line 771), which is why the file is in the list. The full build from the archive is test 56.

**That the build works from the archive alone** (ADR 0034) is test 56, by hand: when the archive step is first implemented, and again in every pull request that changes `scripts/occt-source-archive.sh`, `scripts/build-occt.sh` or `scripts/occt-version.sh`, which records the result. No CI check catches a file the build needs and the archive lacks, since CI builds OCCT from a clone (3.2). Test 56 is also the one run of the script's real clone before a tag: `draft` runs only for a tag, and tests 55 and 58 replace the clone with a stub.

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

**Disk space.** Each crate builds its integration tests as one test binary (open issue 9, ADR 0035): `autotests = false`, one `[[test]]` named `integration`, and `tests/integration.rs`, which declares the other `.rs` files directly in `tests/` and the helper directories as modules. Two test targets stay separate: the release-only budgets of `partsmith-features` and of `partsmith-recompute`, each crate's `budgets` (`tests/budgets.rs`), never timed beside the crate's other tests (`partsmith-recompute`'s also one at a time). The reference stability corpus, a third target behind the `stability-study` feature until every scenario passed, is part of `partsmith-recompute`'s `integration`. One binary per test file made 102 integration test executables that link OCCT statically in a default build, 42 to 112 MB each without debug info and 7.0 GB together. With Cargo's default debug info they outgrew the free space of an `ubuntu-24.04` runner, and the linker died with a bus error while writing its output (the first CI run, 2026-10-13). Merged, those 102 became 11 integration test executables, 46 to 112 MB each and 0.88 GB together; with the unit tests, a default build links OCCT into 21 test executables (those of `partsmith-occt-sys` and the eight crates that depend on it), 1.5 GB together (measured on 2026-10-22). The workflow still sets `CARGO_PROFILE_DEV_DEBUG: "0"` for every job, which keeps the test executables small and quick to link (panic messages keep their file and line; a developer who needs a debugger builds locally with debug info), and the `rust` job still first removes the preinstalled SDKs it does not use (`sudo rm -rf /usr/share/dotnet /usr/local/lib/android /opt/ghc /opt/hostedtoolcache/CodeQL`) and prints `df -h /` before the build and after the tests; dropping either waits for a CI run that measures the space it saves.

`deny.toml` keeps its license allow list: every license in the Tauri dependency tree is already allowed (ADR 0014). Two RustSec advisories apply to the Linux GTK3 stack Tauri uses, RUSTSEC-2024-0429 (`glib` 0.18, unsound `VariantStrIter`) and RUSTSEC-2024-0370 (`proc-macro-error`, unmaintained, a build-time dependency of the gtk-rs 0.18 macros); `deny.toml` lists both in `[advisories] ignore` with that reason and a note to remove them when Tauri moves past gtk-rs 0.18 (section 8).

### 3.3 Cloudflare Pages deploy

1. The deploy runs only when both secrets are present. On forks and in repositories without them (including Dependabot pull requests, which get no secrets) the job succeeds with the notice of 2.4 and deploys nothing.
2. A push to `main` deploys with `--branch=main`; the Pages project's production branch is `main`, so that deployment becomes production.
3. A pull request deploys with `--branch=<head branch>`; Pages makes it a preview with a stable alias `https://<sanitized branch>.<subdomain>`, where `<subdomain>` is the project's `*.pages.dev` host (2.12), and a unique deployment URL. The alias URL is the `preview` environment's URL on the pull request and is written to the job summary.
4. The artifact deployed is exactly the `web-dist` that `web` checked (`check:dist`) and that `e2e` tested against the same core build.
5. The project name comes from the repository variable `CLOUDFLARE_PAGES_PROJECT`, else `partsmith`. Creating the project (`wrangler pages project create <name> --production-branch main`) and the token are one-time human steps (section 8).
6. Only this job builds what Pages serves. Cloudflare's own build image cannot build the WebAssembly core (it needs Emscripten and OCCT built for WebAssembly, longer than a Pages build may run), and `ui/public/core` is not in git, so a Pages build of `ui/` alone serves an app whose core fails to load. A project connected to the repository in the Cloudflare dashboard therefore keeps its automatic builds paused (Settings, Build, Branch control: "Enable automatic production branch deployments" off and preview branches None), and this job's Wrangler deploys go to it as to a project created with Wrangler.
7. Before Wrangler runs, the job checks the source stamp (2.13): a build whose `SOURCE.txt` does not name the run's commit is not deployed, and the job fails.

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

Steps per runner, all with `shell: bash` (Git Bash on Windows), in a job with `timeout-minutes: 150` (2.5):

1. Checkout. Linux: the Tauri packages and `ninja-build`. Windows: `ilammy/msvc-dev-cmd@v1.13.0` (MSVC, CMake and Ninja from Visual Studio on `PATH`), then a step that writes `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER` (the `link.exe` next to `cl.exe`, the one CMake uses for OCCT) to `$GITHUB_ENV`, because Git Bash puts GNU `link` first on `PATH` and rustc takes the first `link.exe` when `VCINSTALLDIR` is set. macOS: `brew install ninja`.
2. OCCT native: `actions/cache@v6.1.0` with the key of 3.2; on a miss, `scripts/build-occt.sh native "$OCCT_ROOT"` with `OCCT_JOBS: 4` in the environment (so the script does not need `nproc`, which macOS lacks). On macOS the job sets `MACOSX_DEPLOYMENT_TARGET: "13.0"` for OCCT, Rust and the bundle (desktop.md 2.8).
3. Rust toolchain and `Swatinem/rust-cache@v2.9.2` with `save-if: ${{ github.ref == 'refs/heads/main' }}`, so only a dry run from `main` saves the Rust cache, where every tag run can restore it (3.2); pnpm and Node 22; `pnpm -C ui install --frozen-lockfile`; `pnpm -C ui build`.
4. `tauri-cli` from its cache, else `cargo install tauri-cli --version 2.12.2 --locked`.
5. `cargo tauri build --bundles <bundles> --ci` in `desktop/src-tauri`.
6. Copy the files of the table into one flat `bundles/` folder (each pattern expands unquoted, so a pattern that matches nothing fails the copy), and upload `bundles/` as `bundle-<runner.os>` (`retention-days: 7`, so a run's bundles do not stay downloadable for the repository's default of 90 days; `if-no-files-found: error`). Flat assets let `draft` run `gh release create` on `release/*` and write `SHA256SUMS` over plain file names.

`draft` then creates the draft release with every bundle, `THIRD-PARTY-NOTICES.txt` (2.9), the OCCT source archive `occt-{OCCT_VERSION}-source.tar.gz` (2.13) and `SHA256SUMS`. A human downloads and tries the bundles, edits the notes, and publishes the release (technical design section 17: "a human approves each tagged version before it is published"). Nothing is signed: Windows SmartScreen warns and macOS needs the user's approval in System Settings, which the release notes state with the paragraph below.

**The unsigned-bundle paragraph**, word for word, at the top of every release's notes, above the notes that `--generate-notes` writes, and in README.md (2.12):

```text
These builds are not signed yet. On Windows, SmartScreen may say "Windows protected your PC": choose More info, then Run anyway. On macOS, the first open is blocked: open System Settings, then Privacy & Security, and choose Open Anyway for Partsmith. On Linux, the .deb, .rpm and .AppImage need glibc 2.39 or later and WebKitGTK 4.1, as on Ubuntu 24.04. To check a download, run sha256sum -c SHA256SUMS --ignore-missing in the folder that holds it.
```

**Release checklist**, kept in `.github/workflows/AGENTS.md` with this text:

1. The source route of ADR 0034 is in place for this commit: the repository is public, so the tag holds the Partsmith source, and `draft` attaches the OCCT source archive (2.13); a release without that archive stays a draft.
2. Until the AppImage question of ADR 0034 is decided, no release whose assets include an `.AppImage` is published, and no release run, dry run or tag, starts while the Linux row of the matrix builds one: the repository is public, so any signed-in GitHub user can download a run's `bundle-Linux` artifact.
3. CI passed on the `main` commit to release, including `check:notices` (2.4).
4. The version is the same in the workspace `Cargo.toml` (`[workspace.package] version`) and `ui/package.json`, as `verify` checks; a version bump is its own pull request.
5. A release dry run (`workflow_dispatch` of `release.yml`) from `main` passed on that commit (test 13), and each bundle installed and started on its OS (the manual checks of desktop.md section 6).
6. Push the tag `vX.Y.Z` on that commit.
7. When `draft` finishes, download the assets, check them with `sha256sum -c SHA256SUMS`, and install each bundle once more.
8. Edit the draft's notes: the unsigned-bundle paragraph first, then the generated notes.
9. Publish the release.

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
| Notices missing from the build (2.6) | `check:dist`, exit 1 | `{directory}/THIRD-PARTY-NOTICES.txt is missing. Keep ui/public/THIRD-PARTY-NOTICES.txt, which pnpm notices writes (docs/spec/delivery.md section 2.9).` |
| Notices of the build not the web app's (2.6) | `check:dist`, exit 1 | `{directory}/THIRD-PARTY-NOTICES.txt does not start with "Third-party notices for the Partsmith web app" (docs/spec/delivery.md section 2.9).` |
| A script or preload of `index.html` that is not a file of the build (2.6) | `check:dist`, exit 1 | `{directory}/index.html loads {reference}, which is not a file in {directory}. Make it the path of a file of the build, with no scheme, query or fragment, and keep base in vite.config.ts as '/' or './'.` (`{reference}` exactly as written in the tag) |
| Initial JavaScript over the limit (2.6) | `check:dist`, exit 1 | `{directory} has {total} bytes of initial JavaScript gzipped, over the limit of {limit} bytes (docs/spec/ui.md section 5): {files}. Move code that startup does not need into modules loaded with import() (docs/spec/delivery.md section 2.6).` (`{limit}` is 250000 from `checkDist`; `{files}` is each initial file as `{path} ({gzippedSize} bytes)`, in path order, joined by `, `) |
| Directory missing | `check:dist`, exit 1 | `{directory} does not exist. Run pnpm build first.` |
| Wrong arguments | `check:dist`, exit 2 | `usage: node scripts/check-dist.mjs [directory]` |
| Generated files drifted | `schema` job | `Generated files are out of date. Run: cargo run -p partsmith-cli -- schema --write && pnpm -C ui generate:protocol` |
| A notices file differs from what the inputs give, or is missing (2.9) | `check:notices`, standard error, exit 1, one line per file | `{file} is out of date. Run: pnpm -C ui notices` (`{file}` is `THIRD-PARTY-NOTICES.txt` or `ui/public/THIRD-PARTY-NOTICES.txt`) |
| A command the generator runs fails (2.9) | `notices`, `check:notices`, exit 1 | `{command} {arguments} failed with exit status {status}: {standard error}` |
| A package ships no license text and its standard text is missing (2.9, input 3) | `notices`, `check:notices`, exit 1 | `{name} {version} ships no license text, and ui/scripts/notices/licenses/{identifier}.txt is missing. Add the SPDX License List text of {identifier} there.` |
| A package ships no license text, takes none of the two standard texts, and has no copy from its repository (2.9, input 3) | `notices`, `check:notices`, exit 1 | `{name} {version} ships no license text, and ui/scripts/notices/crates/{name}/ is missing. Commit a copy of the license file of its repository there (docs/spec/delivery.md section 2.9).` |
| A committed copy that inputs 4 or 6 name is missing (2.9) | `notices`, `check:notices`, exit 1 | `ui/scripts/notices/{path} is missing. Make it as docs/spec/delivery.md section 2.9 describes, from the version that the pins name.` (`{path}` relative to `ui/scripts/notices/`, for example `occt-7.9.3/bundled.json` or `emscripten-4.0.15/LICENSE`) |
| A crate ships a prebuilt library and is not in `PREBUILT_LIBRARY_CRATES` (2.9, input 8) | `notices`, `check:notices`, exit 1 | `{name} {version} ships the prebuilt library {path}. Find its license, give it an entry if it needs one, and add the crate to PREBUILT_LIBRARY_CRATES (docs/spec/delivery.md section 2.9).` (`{path}` the first of `prebuiltLibraries`) |
| Wrong arguments | `notices`, exit 2 | `usage: node scripts/notices.mjs write\|check` |
| An npm package that ships under a license the list does not allow (2.10) | `check:licenses`, standard error, exit 1 | `npm package {name} {version} is licensed {license}, which deny.toml does not allow. Replace the package, or allow the license in deny.toml with an ADR (docs/spec/delivery.md section 2.10).` (`{license}` exactly as pnpm reports it) |
| `deny.toml` has no readable allow list (2.10) | `check:licenses`, exit 1 | `deny.toml has no allow array of license strings in its [licenses] table (docs/spec/delivery.md section 2.10).` |
| A package of `EMITTED_BUILD_PACKAGES` is not installed (2.10) | `check:licenses`, `notices`, `check:notices`, exit 1 | `npm package {name} of EMITTED_BUILD_PACKAGES is not in pnpm licenses list --json. Remove it from the list if the build no longer uses it (docs/spec/delivery.md section 2.10).` |
| `pnpm licenses list` fails (2.10) | `check:licenses`, exit 1 | `{command} failed with exit status {status}: {standard error}` (`{command}` is `pnpm licenses list --prod --json` or `pnpm licenses list --json`) |
| Wrong arguments | `check:licenses`, exit 2 | `usage: node scripts/check-licenses.mjs` |
| A scheduled night after no change to `main` (2.11) | `nightly.yml` notice, success | `::notice title=Nightly skipped::The last nightly run already tested {sha}, the head of main, so the property tests did not run.` |
| `gh run list` fails in the skip check (2.11) | `nightly.yml` warning, the tests run | `::warning title=Nightly skip check failed::gh run list failed, so the property tests run without checking whether an earlier night tested {sha}.` |
| Deploy secrets absent | `deploy` job notice, success | `Cloudflare secrets are not available to this run (a fork, or the repository is not configured), so nothing was deployed.` |
| The build's source stamp is missing or names another commit (2.13) | `deploy` job, standard output, the job fails before Wrangler runs | `::error title=Source stamp missing::ui/dist/SOURCE.txt does not name the commit {sha}, so nothing was deployed. The web job writes it (docs/spec/delivery.md section 2.13).` |
| Tag and version differ | `verify` | `Tag v{tag} does not match the version {version} of partsmith-desktop and ui/package.json.` |
| No green CI for the tagged commit | `verify` | `CI has not passed for {sha}. Tag a commit whose CI run succeeded.` |
| The OCCT clone holds no pinned tag, or is not a Git clone (2.13) | `scripts/occt-source-archive.sh`, standard error, exit 1 | `{src} holds no tag {OCCT_TAG}. Point OCCT_WORK_DIR at a folder whose src is a clone that scripts/build-occt.sh made, or at one without src, where the script clones the tag (docs/spec/delivery.md section 2.13).` (`{src}` is `$OCCT_WORK_DIR/src` as the script resolves it) |
| The pinned tag names another commit than `OCCT_COMMIT` (2.13) | `scripts/occt-source-archive.sh` before archiving, and `scripts/build-occt.sh` after its own clone; standard error, exit 1 | `The tag {OCCT_TAG} in {src} names the commit {commit}, not {OCCT_COMMIT}, which scripts/occt-version.sh pins. Upstream may have moved the tag; nothing was built or archived from it (docs/spec/delivery.md section 2.13).` (`{src}` as above, `{commit}` what `rev-parse` prints) |
| Wrong arguments | `scripts/occt-source-archive.sh`, standard error, exit 2 | `usage: scripts/occt-source-archive.sh <directory>` |
| Example file in a wrong place | examples runner | `examples/{path}: not an allowed example path (docs/spec/delivery.md section 2.8).` |
| Orphan companion file | examples runner | `examples/{path}: has no .ptsm next to it.` |
| Example sets the rollback marker | examples runner | `examples/{path}: examples must not set the rollback marker.` |
| Example not canonical | examples runner | `examples/{path}: not canonical. Run: partsmith fmt examples/{path}` |
| Feature type without an example | examples runner | `Feature type {type} has no example. Add one under examples/{type}/.` |
| Expected file missing | examples runner | `examples/{path}.expected.json is missing. Run: PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test integration examples::` |
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
| `pnpm check:licenses` | 10 s |
| `pnpm check:notices`, crate sources already downloaded | 60 s |
| `deploy` job | 3 minutes |
| Nightly `properties` job with its Rust cache warm, on a night that runs the tests (2.11) | 5 minutes |
| A release from tag to draft, OCCT caches warm | 60 minutes (cold: 120 minutes) |

A cold OCCT build is excluded from the CI budget; it happens only when the OCCT scripts change.

## 6. Acceptance tests

Each job's checks are its acceptance tests; they pass on every pull request and push to `main`. Script and runner tests are listed with them.

1. `occt-native`, `occt-wasm`: the install exists at `.occt/native` or `.occt/wasm` (cache hit or built) and contains `lib/libTKernel.a`.
2. `kernel-wasm`: `cargo test -p partsmith-occt-sys --target wasm32-unknown-emscripten` passes.
3. `rust`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass, the latter including `partsmith-desktop` (desktop.md section 6) and the examples runner (2.8).
4. `deny`: `cargo deny check` passes with `deny.toml`.
5. `ui`: `pnpm lint`, `pnpm typecheck`, `pnpm test` and `pnpm test:scripts` pass, and `pnpm check:licenses` exits 0 (2.10).
6. `schema`: after regeneration, `git status --porcelain -- docs/format ui/src/generated` prints nothing, and `pnpm -C ui check:notices` exits 0 (2.9).
7. `core-wasm`: `scripts/build-wasm.sh` exits 0 and writes `ui/public/core/partsmith_core.mjs` and `partsmith_core.wasm`; `cargo test -p partsmith-wasm --test integration web_module::` passes without skipping (`PARTSMITH_REQUIRE_WEB_MODULE=1` turns a skip into a failure); the artifact `core-wasm` holds `core/partsmith_core.mjs`, `core/partsmith_core.wasm` and `THIRD-PARTY-NOTICES.txt` at its root.
8. `web`: `pnpm build` and `pnpm check:dist` exit 0; `ui/dist/_headers` exists and `ui/dist/THIRD-PARTY-NOTICES.txt` starts with the web heading (both checked by `check:dist`), and `ui/dist/core/partsmith_core.wasm` and `ui/dist/core/partsmith_core.mjs` exist (checked by the job after `check:dist`).
9. `e2e`: `pnpm e2e` passes against the real core (ui.md 6.5).
10. `python`: `maturin develop --release` succeeds, `pytest python/tests` passes, including `test_examples.py`, and `pytest python/tests -m packaging` passes: python-api.md test 76 builds one wheel `partsmith-0.1.0-cp312-abi3-<platform>.whl` and installs and imports it with `python3.12` and with `python3.13`, both found on `PATH` (the test fails, rather than skips, when either is missing).
11. `deploy`: with secrets, the Wrangler step succeeds and the job summary contains a `pages.dev` URL; on a fork pull request, the job succeeds and its log contains the "Deploy skipped" notice.
12. `desktop`: `cargo tauri build --bundles deb` exits 0, exactly one `.deb` exists under `target/release/bundle/deb/`, and its file list holds `usr/bin/partsmith-desktop`, exactly one `.desktop` file under `usr/share/applications/` and `usr/lib/Partsmith/THIRD-PARTY-NOTICES.txt`, whose bytes equal the repository's `THIRD-PARTY-NOTICES.txt` (desktop.md test 49). The file list is `dpkg-deb --fsys-tarfile` piped to `tar -t` with a leading `./` removed from each entry: Tauri's bundler writes entries without the `./` that `dpkg-deb --build` writes, so the check accepts both forms. The notices are extracted by their member name as the list writes it, from the repository root, with `deb` the path of that `.deb`:

    ```sh
    member="$(dpkg-deb --fsys-tarfile "$deb" | tar -t | grep -xE '(\./)?usr/lib/Partsmith/THIRD-PARTY-NOTICES\.txt')"
    dpkg-deb --fsys-tarfile "$deb" | tar -xO "$member" | cmp - THIRD-PARTY-NOTICES.txt
    ```

13. `release.yml` dry run (`workflow_dispatch`): the three `bundle` jobs upload `bundle-Linux` (a `.deb`, an `.rpm` and an `.AppImage`), `bundle-Windows` (one `-setup.exe`) and `bundle-macOS` (one `.dmg`); `verify` and `draft` are skipped. The reviewer of the dry run records, for each bundle, that it holds `THIRD-PARTY-NOTICES.txt` at the place of desktop.md 2.8, equal to the repository's file: in the `.rpm` (`rpm -qlp` lists `/usr/lib/Partsmith/THIRD-PARTY-NOTICES.txt`), in the AppImage (`--appimage-extract`, then `squashfs-root/usr/lib/Partsmith/`), in the mounted `.dmg` (`Partsmith.app/Contents/Resources/`) and in the folder the NSIS installer installs to (next to `partsmith-desktop.exe`); and that neither the installer nor the disk image asks the user to accept a license (desktop.md 2.8). On a tag, `draft` creates a draft release holding those files, `THIRD-PARTY-NOTICES.txt` (equal to the tagged commit's file), `occt-{OCCT_VERSION}-source.tar.gz` (2.13) and `SHA256SUMS`, whose lines verify with `sha256sum -c` and include the notices and the archive.

**Distribution check (`ui/scripts/check-dist.test.mjs`, `node:test`, temporary directories)**

Run by `pnpm test:scripts` in `ui/`. `node --test` takes files or glob patterns, not a folder, so the script passes the quoted glob `"scripts/**/*.test.mjs"`. Every directory that a test of `check-dist.test.mjs` (tests 14 to 19, 26 to 33 and 34) expects to be ready, or to have exactly the problems it lists, also holds a regular file `THIRD-PARTY-NOTICES.txt` whose first line is `WEB_NOTICES_HEADING` (2.6), unless the test says otherwise.

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

**Notices in the build (`ui/scripts/check-dist.test.mjs`, `node:test`, temporary directories, run by `pnpm test:scripts`)**

34. Next to a small file and the exact `_headers`: without `THIRD-PARTY-NOTICES.txt`, `checkDist` returns exactly the missing-notices message; with one whose first line is `Third-party notices for the Partsmith desktop app`, exactly the wrong-notices message; with one that holds only the web heading and no LF, `[]`; with `THIRD-PARTY-NOTICES.txt` a symbolic link to a valid notices file outside the directory, the missing-notices message. With a sparse file of 26,214,401 bytes at `core/big.wasm`, no `_headers` and no notices, `checkDist` returns the oversized, missing-headers and missing-notices messages, in that order. The command on the directory without notices exits 1 with the missing-notices message on standard error.

**Notices generator (`ui/scripts/notices.test.mjs`, `node:test`, temporary directories and literal inputs, run by `pnpm test:scripts`)**

No test runs `cargo` or `pnpm`: the tests pass a fake `run` that answers each command with a literal output, and they fail on a command they did not expect.

35. `DESKTOP_NOTICES_HEADING` and `WEB_NOTICES_HEADING` are the first lines of `THIRD-PARTY-NOTICES.txt` and `ui/public/THIRD-PARTY-NOTICES.txt`. `DESKTOP_TARGETS` lists `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc` and `aarch64-apple-darwin` with the platforms `Linux`, `Windows` and `macOS`, in that order.
36. `normalizeText('\uFEFF\r\n  \nMIT License \t\r\n\fCopyright\rline\n\n\n')` is `'MIT License\nCopyright\nline\n'`; `normalizeText(' \n\t\n')` is `''`; `normalizeText('a \u2014 b')` is `'a \u2014 b\n'` (the character is kept).
37. `artifactCrates` of literal metadata: the workspace crate `app` (`source` null) has a normal dependency `a 1.0.0`, a build dependency `b 1.0.0` and a dev dependency `c 1.0.0`; `a` has a normal dependency `d 2.0.0` and one on the workspace crate `lib`, which depends on `e 0.10.0` and `e 0.9.1`; only `c` reaches `f 1.0.0`; `app` reaches `g 1.0.0` through an edge whose `dep_kinds` holds a `dev` entry and a normal one. `artifactCrates(metadata, 'app')` returns `a 1.0.0`, `b 1.0.0`, `d 2.0.0`, `e 0.9.1`, `e 0.10.0` and `g 1.0.0`, in that order, each with its `license`, its `license_file` as `licenseFile`, the folder of its `manifest_path` and the `repository` of `app`; never `app`, `lib`, `c` or `f`.
38. `licenseFiles` of a folder holding the regular files `LICENSE-MIT`, `license`, `COPYING.txt`, `NOTICE`, `UNLICENSE`, `Licence.md`, `THIRD-PARTY-LICENSE`, `ThirdPartyNotices.txt`, `README.md`, `LICENSES/MIT.txt`, `LICENSES/sub/x.txt`, `docs/THIRD_PARTY.md`, `src/LICENSE`, `src/unicode/LICENSE-UNICODE`, `src/license.rs`, `vendor/lib/COPYING`, `legal/TERMS.txt`, `dist/icons/copyright.mjs`, `dist/icons/copyright.mjs.map` and `dist/icons/copyright.d.ts`, a folder `COPYRIGHT`, a symbolic link `LICENSE-APACHE` to a file outside and a symbolic link `linked` to a folder outside that holds a `LICENSE`, with `licenseFile` `legal/TERMS.txt` and `excluded` `['vendor/']`, returns, relative to the folder with `/`: `legal/TERMS.txt`, `COPYING.txt`, `LICENSE-MIT`, `Licence.md`, `NOTICE`, `THIRD-PARTY-LICENSE`, `ThirdPartyNotices.txt`, `UNLICENSE`, `license`, `LICENSES/MIT.txt`, `docs/THIRD_PARTY.md`, `src/LICENSE`, `src/unicode/LICENSE-UNICODE`. With `licenseFile` `LICENSE-MIT`, that file comes first and only once; with `excluded` `[]`, `vendor/lib/COPYING` comes last; with `excluded` `['src/LICENSE']`, only that file is left out. `prebuiltLibraries` of a folder holding `x64/a.lib`, `lib/b.a`, `c.dll`, `d.so`, `e.dylib`, `src/tests/lib/f.a`, `tests/g.a` and `h.lib.txt` returns `c.dll`, `d.so`, `e.dylib`, `lib/b.a` and `x64/a.lib`, in that order.
39. `renderNotices` of a literal fixture, compared with the whole expected text written in the test: heading and scope of the desktop file; `partsmith` `Apache text\n`; the OCCT statement `Statement.` and two OCCT entries, the first with texts `LGPL\n` and `Exception\n`, the second `x, in OCCT's y` with no version, no license and the URL `https://example.com/y` (`Bundled\n`); one font with `OFL\n`; the crates, in this order, `b 1.0.0` (`MIT`, all platforms, `MIT b\n`), `a 1.0.0` (`MIT OR Apache-2.0`, Windows only, `Apache text\n` and `MIT a\n`), `c 2.0.0` (`BSD-3-Clause`, Linux and macOS, text from its repository `BSD c\n`), `d 0.1.0` (no license field, all platforms, `Custom\n`) and `e 0.2.0` (`Zlib OR Apache-2.0 OR MIT`, macOS only, standard text `Apache text\n`); one npm package `p 0.1.0` (`MIT`, `MIT b\n`); and one other library `LLVM libc++, libc++abi, libunwind and compiler-rt, in the Emscripten runtime` with no version (`Apache-2.0 WITH LLVM-exception AND (NCSA OR MIT)`, `https://llvm.org`, `LLVM\n`). Expected: the layout of 2.9, with the line `Each crate's source code is at its crates.io URL; for the crates under MPL-2.0 that is where to obtain their Source Code Form.` and a blank line after `== Rust crates ==`; entries in the order given; the texts [1] `Apache text`, [2] `LGPL`, [3] `Exception`, [4] `Bundled`, [5] `OFL`, [6] `MIT b`, [7] `MIT a`, [8] `BSD c`, [9] `Custom` and [10] `LLVM`, each once; the lines `x, in OCCT's y, see texts, https://example.com/y, texts [4]`, `a 1.0.0, MIT OR Apache-2.0, https://crates.io/crates/a/1.0.0, only Windows, texts [1] [7]`, `c 2.0.0, BSD-3-Clause, https://crates.io/crates/c/2.0.0, only Linux and macOS, text from its repository, texts [8]`, `d 0.1.0, see texts, https://crates.io/crates/d/0.1.0, texts [9]`, `e 0.2.0, Zlib OR Apache-2.0 OR MIT, https://crates.io/crates/e/0.2.0, only macOS, standard text, texts [1]`, `p 0.1.0, MIT, https://www.npmjs.com/package/p/v/0.1.0, texts [6]` and `LLVM libc++, libc++abi, libunwind and compiler-rt, in the Emscripten runtime, Apache-2.0 WITH LLVM-exception AND (NCSA OR MIT), https://llvm.org, texts [10]`; and the text ends with `LLVM\n`.
40. `generateNotices` of a temporary repository root holding copies of `scripts/occt-version.sh`, `scripts/emsdk-version.sh`, `rust-toolchain.toml`, `LICENSE`, the two font license texts and `ui/scripts/notices/`, with no `OCCT_ROOT` in the environment, and a `run` that answers literal metadata and literal pnpm listings. The metadata has the root crates `partsmith-desktop` and `partsmith-wasm` with `repository` `https://github.com/blai30/partsmith`, one crate with a license file in a temporary folder on every platform, and `webview2-com-sys` at its locked version in the Windows graph only, with `x64/WebView2LoaderStatic.lib` in its temporary folder. The production listing has one package and the full listing adds every name of `EMITTED_BUILD_PACKAGES`. `run` receives `cargo metadata --format-version 1 --locked --all-features --filter-platform <triple>` once for each triple of `DESKTOP_TARGETS` and once for `wasm32-unknown-emscripten`, and `pnpm licenses list --prod --json` and `pnpm licenses list --json` once each in `ui/`. The desktop text's statement is exactly the desktop statement of 2.9 with `7.9.3`, `V7_9_3` and that repository, and the web text's exactly the web statement. Both texts have the OCCT entry with `texts [2] [3]` followed by the four entries of `bundled.json`, in its order, the crates' source line, the packages of `EMITTED_BUILD_PACKAGES`, and the Rust standard library line with the `channel` of `rust-toolchain.toml`, `(MIT OR Apache-2.0) AND Unicode-3.0` and two texts. The desktop text has the WebView2 loader line, marked `only Windows`, and no Emscripten line; the web text has the Emscripten, musl and LLVM lines, the last with four texts, and no WebView2 line. A license file holding the bytes `61 ff 62 0a` gives the text `a\uFFFDb\n`.
41. Failing inputs, each rejecting with its message of section 4 and writing nothing: a crate without license files under `MIT` and without `ui/scripts/notices/crates/<name>/`; a crate without license files under `BSD-3-Clause OR MPL-2.0` while `ui/scripts/notices/licenses/MPL-2.0.txt` is missing; `scripts/occt-version.sh` naming `9.9.9`, for which `ui/scripts/notices/occt-9.9.9/` is missing, and an `occt-7.9.3/` folder without `delabella.txt`; `scripts/emsdk-version.sh` naming `9.9.9`, for which `ui/scripts/notices/emscripten-9.9.9/LICENSE` is missing; a `rust-toolchain.toml` with the channel `9.9.9` and no `ui/scripts/notices/rust-9.9.9/Unicode-3.0.txt`; a `webview2-com-sys` version without its `ui/scripts/notices/webview2-com-sys-{version}/LICENSE.txt`; a crate `z 1.0.0` not in `PREBUILT_LIBRARY_CRATES` with `lib/z.a` in its folder (the message names `z 1.0.0` and `lib/z.a`); a full pnpm listing without `rolldown`; a `run` that rejects `cargo metadata` with exit status 101 and standard error `error: x`. And the choice of text for crates without license files: under `MIT OR Apache-2.0` and under `Zlib OR Apache-2.0 OR MIT`, the `Apache-2.0` standard text; under `MPL-2.0`, the `MPL-2.0` standard text; under `MIT AND Apache-2.0` and under `Apache-2.0 WITH LLVM-exception`, the files of `ui/scripts/notices/crates/<name>/`, marked `text from its repository`.
42. `node scripts/notices.mjs` with no argument, with `wrong` and with `write check` exits 2 with the usage line. (The `schema` job runs `check` on the real repository, test 6.)

**npm license check (`ui/scripts/check-licenses.test.mjs`, `node:test`, run by `pnpm test:scripts`)**

43. `allowedLicenses` of the repository's `deny.toml` is the eleven identifiers of 2.10, in that order. On literal texts: an `allow` array with a comment line, a comment after an element and a trailing comma gives its strings; an `allow` key in a table before `[licenses]` is ignored; a text without `[licenses]`, a `[licenses]` table without `allow`, an unterminated array and an element `MIT` without quotes each throw the deny.toml message.
44. `licenseAllowed` with the repository's list is true for `MIT`, `mit`, `(MIT OR Apache-2.0)`, `MIT OR GPL-3.0-only`, `GPL-3.0-only OR MIT`, `Apache-2.0 WITH LLVM-exception`, `MIT AND (Apache-2.0 OR BSD-3-Clause)`, `MIT OR GPL-3.0-only AND Zlib` and `((ISC))`, and false for `GPL-3.0-only`, `MIT AND GPL-2.0-only`, `(GPL-3.0-only OR MIT) AND BlueOak-1.0.0`, `MIT WITH LLVM-exception`, `GPL-2.0-only WITH Classpath-exception-2.0`, `BlueOak-1.0.0`, `UNLICENSED`, `SEE LICENSE IN LICENSE.txt`, `Unknown`, the empty string, `MIT OR`, `MIT or Apache-2.0`, `(MIT` and `MIT)`.
45. `productionPackages` of a literal listing with the groups `MIT` (`a` with versions `1.0.0` and `2.0.0` and a path for each, and `c 0.1.0`) and `ISC` (`b 1.0.0`) returns `a 1.0.0`, `a 2.0.0`, `b 1.0.0` and `c 0.1.0`, in that order, each with its path and its `license` field; a name and version listed twice comes back once. `EMITTED_BUILD_PACKAGES` is `['rolldown', 'tailwindcss', 'vite']`. `shippedPackages` of a production listing with `react 19.3.0` and a full listing that also holds `vite` 8.3.4 and 7.0.0, `tailwindcss 4.3.3`, `rolldown 1.2.13` and `typescript 6.0.0` returns `react 19.3.0`, `rolldown 1.2.13`, `tailwindcss 4.3.3`, `vite 7.0.0` and `vite 8.3.4`, in that order; with a full listing that lacks `rolldown` and `vite`, it throws the emitted-package message naming `rolldown`.
46. `licenseProblems` of `react 19.3.0` under `MIT` and `lru-cache 11.5.3` under `BlueOak-1.0.0`, with the repository's list, returns exactly the not-allowed message for `lru-cache 11.5.3` and `BlueOak-1.0.0`.
47. The command in `ui/` of this repository exits 0 and prints the success line whose `{n}` is the length of `shippedPackages` of the same two listings, which the test runs itself; with any argument it exits 2 with the usage line.

**Nightly workflow (`nightly.yml`)**

48. A `workflow_dispatch` run from `main` runs `cargo test -p partsmith-sketch --release --test integration properties::` with `PROPTEST_CASES` `4096` and passes, its log showing sketch.md tests 189 to 196 passing. The next scheduled run with `main` unchanged succeeds with the skip notice of section 4 and runs no checkout and no cargo step; the first scheduled run after a new commit on `main` runs the tests.

**Workflow files (`ui/scripts/workflows.test.mjs`, `node:test`, run by `pnpm test:scripts`)**

The tests read `.github/workflows/ci.yml`, `release.yml` and `nightly.yml` as text, by the two-space indentation those files use: a job is a line `  <id>:` under `jobs:`, with its keys four spaces in and its steps as `      - ` items. A job or step the test cannot find fails it, naming the file.

49. Timeouts (2.4, 2.5, 2.11): the jobs of `ci.yml` are `occt-native`, `occt-wasm`, `kernel-wasm`, `rust`, `deny`, `ui`, `schema`, `core-wasm`, `web`, `e2e`, `python`, `desktop` and `deploy`, in that order, with `timeout-minutes` 150 for the first two and 60 for every other; the jobs of `release.yml` are `verify`, `bundle` and `draft`, with 60, 150 and 60; and the one job of `nightly.yml`, `properties`, has 20.
50. The `schema` job (2.4): its step with `id: install` runs `pnpm -C ui install --frozen-lockfile` and comes before the step that runs `cargo run -p partsmith-cli -- schema --write`, and its last step runs `pnpm -C ui check:notices` with `if: ${{ !cancelled() && steps.install.outcome == 'success' }}`.
51. The nightly skip check (2.11): the `run` script of the step with `id: skip` in `properties`, run with `bash -e` and a stub `gh` first on `PATH` that records its arguments and prints one literal line or fails, with `GITHUB_SHA` a 40-digit commit and `GITHUB_OUTPUT` a file of the test. In every case `gh` receives exactly the arguments of 2.11, once, and the script exits 0. When `gh` prints `GITHUB_SHA`, the file holds `skip=true` and standard output is the skip notice of section 4 for that commit; when it prints another commit, or an empty line (no finished run), the file holds `skip=false` and standard output is empty; when `gh` exits 1, the file holds `skip=false` and standard output is the skip-check warning of section 4 for that commit.

**Source of each build (2.13)**

Tests 52 to 54, 57 and 59 are in `ui/scripts/workflows.test.mjs` and read the workflow files as tests 49 to 51 do; tests 55 and 58 are in `ui/scripts/occt-source-archive.test.mjs`. All seven run in `pnpm test:scripts`, with no network, and use `bash`, `git` and `tar` from `PATH`. Test 56 is run by hand. Tests 52, 55 and 58 run every git command they issue, and the scripts under test, with `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL` set to an empty file of the test, `XDG_CONFIG_HOME` set to an empty folder of the test (so no global attributes file applies), `GIT_AUTHOR_NAME`, `GIT_AUTHOR_EMAIL`, `GIT_COMMITTER_NAME`, `GIT_COMMITTER_EMAIL` and fixed `GIT_AUTHOR_DATE` and `GIT_COMMITTER_DATE`, and create repositories with `git init --initial-branch=main`, so the machine's Git configuration (an identity, commit signing, a default branch) neither stops nor changes them. Tests 52, 53 and 57 take the stamp's file name from one constant of `workflows.test.mjs`, and tests 54 and 57 the script's path from another.

52. The source stamp's writer. The `web` job of `ci.yml` has a step named `Write the source stamp` after the step that runs `pnpm build` and before the one that runs `pnpm check:dist`; it sets no `shell`, and its `env` sets `REPOSITORY_URL` to `${{ github.server_url }}/${{ github.repository }}` and `PULL_REQUEST_URL` to `${{ github.event.pull_request.html_url }}`. Its `run` script, run with `bash -e` in a temporary folder holding an empty `dist/`, with `REPOSITORY_URL` `https://github.com/blai30/partsmith`:
    - for a push, with `GITHUB_SHA` forty `a` digits and `PULL_REQUEST_URL` empty, exits 0 and writes `dist/SOURCE.txt` as exactly the four lines of 2.13 for that commit;
    - for a pull request, run in a shallow clone (`git clone --depth 1` from a `file://` URL), which also holds an empty `dist/`, of a repository the test makes, whose tip is a merge, made with `--no-ff`, of a head commit on another branch into a base commit, with a message whose second paragraph is the line `parent` followed by a space and forty `c` digits and the line `tree` followed by a space and forty `d` digits; with `GITHUB_SHA` that merge commit and `PULL_REQUEST_URL` `https://github.com/blai30/partsmith/pull/7`, it exits 0 and writes the four lines followed by `Pull request: https://github.com/blai30/partsmith/pull/7`, `Parents: {base} {head}` and `Tree: {tree}`, with `{tree}` what `git rev-parse {commit}^{tree}` prints in the repository the test made;
    - with `PULL_REQUEST_URL` set and `GITHUB_SHA` a commit the clone does not hold, exits with a status other than 0.
53. The source stamp's check. In the `deploy` job of `ci.yml`, the step named `Check the source stamp`, with `if: steps.secrets.outputs.available == 'true'`, comes after the step that downloads `web-dist` and before the step with `id: pages`. Its `run` script, run with `bash -e` in a temporary folder with `GITHUB_SHA` forty `a` digits, exits 0 and prints nothing when `ui/dist/SOURCE.txt` is the stamp of 2.13 for that commit, with and without the pull request lines. It exits 1 and prints exactly the source-stamp error of section 4 for that commit, one line on standard output, when `ui/dist/SOURCE.txt` is missing, when `ui/dist` is missing, when the stamp names forty `b` digits, and when its commit line has one more character after the commit.
54. The archive in the release. The `draft` job of `release.yml` has a step whose `run` is `scripts/occt-source-archive.sh release`, after the step whose `run` is `cp THIRD-PARTY-NOTICES.txt release/` and before the step named `Write SHA256SUMS`, and its last step's `run` script runs `gh release create` on `release/*`. `scripts/occt-source-archive.sh` is a regular file with the owner's execute bit set.
55. The archive script (`ui/scripts/occt-source-archive.test.mjs`, temporary directories). The test reads `OCCT_VERSION` and `OCCT_TAG` from `scripts/occt-version.sh` (`{v}` and `{tag}` below), and checks that the file sets `OCCT_COMMIT` to 40 lowercase hexadecimal digits. It makes a fixture repository at `<work>/src`: a first commit holding `.clang-format`, `CMakeLists.txt`, `LICENSE_LGPL_21.txt`, `OCCT_LGPL_EXCEPTION.txt`, `README.md`, `adm/cmake/occt_macros.cmake`, `src/TKernel/PACKAGES`, `src/Standard/Standard.cxx`, `data/step/box.stp`, `dox/index.md`, `samples/sample.cxx`, `tests/bugs/begin.tcl`, `tools/tool.cxx`, `.github/workflows/build.yml` and `.gitattributes`, each holding a short text of its own with LF line endings (`.gitattributes` the line `*.bat eol=crlf`, so no attribute names the others, and `adm/cmake/occt_macros.cmake` two lines), with an annotated tag `{tag}`; a second commit that changes `src/Standard/Standard.cxx`; and an uncommitted change to `CMakeLists.txt`. The fixture's commits cannot be `OCCT_COMMIT`, so the test copies `scripts/occt-source-archive.sh` into a temporary folder next to an `occt-version.sh` that sets `{v}`, `{tag}` and, as `OCCT_COMMIT`, the fixture's tagged commit, and runs that copy, with `bash` and `OCCT_WORK_DIR=<work>`:
    - Into an empty folder `<out>`, it exits 0, prints exactly `Wrote <out>/occt-{v}-source.tar.gz from {tag}, commit {commit}.` with `{commit}` the tagged commit, and `<out>` then holds only that file.
    - The archive's files, as `tar -tzf` lists them without the folder entries, are exactly `.clang-format`, `CMakeLists.txt`, `LICENSE_LGPL_21.txt`, `OCCT_LGPL_EXCEPTION.txt`, `README.md`, `adm/cmake/occt_macros.cmake`, `src/Standard/Standard.cxx` and `src/TKernel/PACKAGES`, each under `occt-{v}/`; its `CMakeLists.txt` and `src/Standard/Standard.cxx` hold the texts of the tagged commit, not the working tree's or the second commit's.
    - Its gzip header holds no file name and a modification time of 0 (byte 3, the flags, and bytes 4 to 7 are 0), and `git get-tar-commit-id` reads the tagged commit from it.
    - A second run into another empty folder writes the same bytes. So does a run with `<work>` holding no `src` and a stub `git` first on `PATH` that records the arguments of `git clone`, clones the fixture repository from its `file://` URL in their place and passes every other command to the real `git`: the stub receives `clone --depth 1 --branch {tag} https://github.com/Open-Cascade-SAS/OCCT.git <work>/src`, once. So does a run with `GIT_CONFIG_COUNT=3`, `GIT_CONFIG_KEY_0=core.autocrlf`, `GIT_CONFIG_VALUE_0=true`, `GIT_CONFIG_KEY_1=tar.umask`, `GIT_CONFIG_VALUE_1=0077`, `GIT_CONFIG_KEY_2=core.attributesFile` and `GIT_CONFIG_VALUE_2` a file of the test holding `*.cmake export-ignore` in its environment; the test first checks that each of the three settings alone changes the bytes of a plain `git archive` of the fixture's tag, so the fixture exercises all three.
    - Failures, each leaving `<out>` empty: with no argument and with two arguments, exit 2 and the usage line of section 4 on standard error; with `<work>/src` a plain folder holding the same files and no Git data, and with a clone of the fixture repository without the tag, exit 1 and the no-tag message of section 4 on standard error; with the copy's `OCCT_COMMIT` the second commit, exit 1 and the tag-commit message of section 4 on standard error, leaving `<work>/src` in place; with `<work>` holding no `src`, the stub `git` above and the copy's `OCCT_COMMIT` the second commit, exit 1 with the tag-commit message (for `<work>/src` and the tagged commit) as the last line of standard error, below the lines of `git clone`, and no `<work>/src` left; with a second fixture repository whose tagged tree lacks `README.md` (and the copy's `OCCT_COMMIT` its tagged commit), and with `<out>` missing, a status other than 0.
56. Building from the archive, by hand (2.13, ADR 0034), recorded in the pull request that first implements the archive step and in each one that changes `scripts/occt-source-archive.sh`, `scripts/build-occt.sh` or `scripts/occt-version.sh`. On Linux, with `<archive>` the file `scripts/occt-source-archive.sh` writes and `<work>` an empty folder: `mkdir -p <work>/src && tar -xzf <archive> --strip-components=1 -C <work>/src`; then `OCCT_WORK_DIR=<work> scripts/build-occt.sh native <prefix>` exits 0 without cloning (`<work>/src` still holds no `.git`) and writes `<prefix>/lib/libTKernel.a`, and `OCCT_ROOT=<prefix> cargo test -p partsmith-occt-sys` passes. With the Emscripten SDK of `scripts/emsdk-version.sh` active, `OCCT_WORK_DIR=<work> scripts/build-occt.sh wasm <prefix-wasm>` exits 0 the same way, and `OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN=<prefix-wasm> cargo test -p partsmith-occt-sys --target wasm32-unknown-emscripten` passes.
57. The statements name what the workflows make (2.9). With `{v}` the `OCCT_VERSION` of `scripts/occt-version.sh`: `ui/public/THIRD-PARTY-NOTICES.txt` holds `/SOURCE.txt`, the file that tests 52 and 53 write and read, `occt-{v}-source.tar.gz` and `scripts/occt-source-archive.sh`; `THIRD-PARTY-NOTICES.txt` holds `occt-{v}-source.tar.gz`, the name test 55 sees the script write, and `scripts/occt-source-archive.sh`, the script test 54 sees `draft` run. The stamp's name and the script's path come from the constants that tests 52 to 54 use, and the archive's name is built from `OCCT_VERSION` as test 55 builds it. Test 40 holds the statements' exact text; this test ties their names to the workflows.
58. The commit check of `scripts/build-occt.sh` (2.13, in `ui/scripts/occt-source-archive.test.mjs` with the fixture repository and the stub `git` of test 55). The test copies `scripts/build-occt.sh` into a temporary folder next to an `occt-version.sh` that sets `{v}`, `{tag}` and an `OCCT_COMMIT`, and runs that copy with `bash`, the arguments `native <prefix>`, `OCCT_WORK_DIR=<work>`, `OCCT_JOBS=1`, the stub `git` and a stub `cmake` first on `PATH` that records its arguments and exits 0:
    - With `<work>` holding no `src` and `OCCT_COMMIT` the tagged commit, it exits 0, the stub `git` receives `clone --depth 1 --branch {tag} https://github.com/Open-Cascade-SAS/OCCT.git <work>/src` once, `cmake` is called three times, the first call starting `-S <work>/src -B <work>/build-native`, and `<work>/src` is still there.
    - With `<work>` holding no `src` and `OCCT_COMMIT` the second commit, it exits 1 with the tag-commit message of section 4 (for `<work>/src` and the tagged commit) as the last line of standard error, below the lines of `git clone`, `cmake` is never called, and no `<work>/src` is left.
    - With `<work>/src` a plain folder holding the fixture's files and no Git data, and `OCCT_COMMIT` the second commit, it exits 0 without calling `git clone`, and `cmake` is called three times, the first call starting `-S <work>/src`: an existing `src` is built as it is, as test 56 builds the extracted archive.
59. The artifacts (2.4, 2.5, ADR 0034). Every step of `ci.yml` and `release.yml` that uses `actions/upload-artifact` sets `retention-days: 7` in its `with`; their names are `core-wasm`, `web-dist` and `playwright-report` in `ci.yml`, and `bundle-${{ runner.os }}` in `release.yml`. The `path` of the `core-wasm` upload is a block of exactly the two lines `ui/public/core/` and `ui/public/THIRD-PARTY-NOTICES.txt`, and the `web` and `e2e` jobs each have a step that uses `actions/download-artifact` with `name: core-wasm` and `path: ui/public`. The jobs alone would not catch a `core-wasm` artifact without the notices (test 7): `web` and `e2e` take the notices from their checkout.

## 7. Open issues

1. **OCCT on Windows and macOS.** `scripts/build-occt.sh` and `partsmith-occt-sys` have been built only on Linux and Emscripten. The release dry run is the first MSVC and Apple Clang build; it may need script changes beyond `INSTALL_DIR_LAYOUT` (3.6), which this spec lists but does not prove.
2. **Core size.** The linked core is 17.2 MiB, about 7.8 MiB under the limit (3.5), so splitting it into lazily loaded modules is not needed now. If it grows past the limit, the `web` job fails, ADR 0017 is revisited, and the plan of 3.5 starts with an ADR; R2 (step 3) is a fallback only, for a file the first two steps cannot bring under the limit.
3. **Local OCCT installs.** A local OCCT WebAssembly install built before the `OCC_CONVERT_SIGNALS` fix of ADR 0003 makes optimized links fail in `wasm-opt` (in `ShapeUpgrade_ShapeDivide::Perform`). CI's cache key includes the script hash, so CI rebuilds; developers must rebuild local installs after the script changes.
4. **Examples and debug builds.** The runner uses the debug CLI binary inside `cargo test --workspace`. If the corpus grows past the 3-minute budget, it moves to its own job with `--release`.
5. **Import examples.** The three `examples/import/` examples reference the placeholder asset name of features/import.md (64 zeros), which the feature tests supply at run time. Through the CLI they have no asset, so they rebuild with `asset_missing` errors (recorded in their expected results) and have no Python twin. An example that imports a real STEP file needs an `examples/import/assets/<hash>.step` and a new example name. `sketch/imported-face-ambiguous` is the first example with a real STEP file: a 10 mm cube in `examples/sketch/assets/`, written by the engine's STEP export and named by its import, so it has no twin.
6. **Source maps.** `vite.config.ts` emits source maps, which deploy with the app (about 1 MB today). That is fine for an open-source app and helps bug reports; dropping them from production is a one-line change if wanted.
7. **features.md and cli.md open issues 3.** Both point at the examples layout, which section 2.8 now defines; their text can be updated to point here when those specs are next edited.
8. **Initial JavaScript headroom.** The app is one chunk, and its initial JavaScript is 96.8 percent of the 250 KB limit: 242,039 bytes gzipped, 7,961 under it, measured on 2026-10-16 by `check:dist` on a build of the tree then (94.1 percent on 2026-10-13, 2.6). A few more features will fail the `web` job. The first response is moving code that startup does not need into chunks loaded with a dynamic `import()` (candidates: the Print context, sketch mode, export), with the loading state each needs specified in ui.md. Raising the limit changes the budget of ui.md 5 and needs human review.
9. **One integration test binary per crate** (resolved by 3.2 and ADR 0035). Each file of a crate's `tests/` was its own binary: 177 binaries (174 without the `stability-study` feature), 105 of them linking OCCT, 7.5 GB of integration test executables. 3.2 and ADR 0035 gathered them into one binary per crate, beside the targets of their own that 3.2 names: `tests/integration.rs` declares the files as modules; they stay where they were, so every path that names a test file still holds. A test's id starts with its file's name (`examples::examples_rebuild_as_expected`), and `cargo test -p <crate> --test integration <file>::` runs the tests of one file (and of any file whose name ends in `_<file>`, such as `error_messages.rs` for `messages::`), except the files of the separate targets, which select nothing there: `budgets.rs` runs with `--test budgets` (in `partsmith-recompute`, `--test budgets feature_box::` runs one feature's budgets). The reference stability corpus, first a target of its own behind the `stability-study` feature, joined `partsmith-recompute`'s `integration` when every scenario passed (recompute.md test 69), so a third runs with `--test integration reference_stability_N::`. Tests of different files of a crate now share one process, so their OCCT calls overlap on the test threads. The integration test executables went from 7.5 to 1.03 GB (18 binaries, 17 without the feature, and 17 since the corpus joined `integration`). Linking those of `partsmith-recompute` (`integration` and `budgets`) from scratch takes 7.8 to 8.0 s instead of 21.5 to 26.3 s, and building them in release about 2 minutes instead of about 21. The locks of kernel.md 3.10 cover the overlap: 495 debug runs of the merged binaries that link OCCT, 278 of them at 8 or 16 threads, passed (2026-10-22). In release, `partsmith-recompute`'s 22 budgets are its test target `budgets` (`tests/budgets.rs`, a module per feature file), as `partsmith-features`' are, so they are never timed beside the tests of the other files, where `feature_shell::t012_performance` missed its 250 ms median with twice as many test threads as cores (`RUST_TEST_THREADS=8` on 4 cores). In their own target the budgets still slowed each other at 8 threads (`feature_boolean::t016_join_builds_within_15_ms` missed its 15 ms in 8 of 8 runs), so `median_duration` of `tests/feature_support` times one median at a time; every budget but features/hole.md test 33 (features.md open issue 5) then holds at any thread count on an otherwise idle machine (ADR 0035).
10. **What the notices do not cover.** The AppImage bundles WebKitGTK, GTK and their dependencies from the Ubuntu 24.04 build machine, many under the LGPL, and the AppImage runtime, which reportedly links libfuse (LGPL) statically; the NSIS installer carries NSIS's own code, its LZMA decompressor (CPL-1.0 with a special exception) and Tauri's `nsis_tauri_utils` plugin. None of these is a Cargo or npm package, so the generator lists none of them, and the AppImage's sources are not offered. ADR 0034 sets out the AppImage choice (drop it from M1, or add its libraries' copyright files and source packages to each release) for the owner, and section 8 asks whether the installer code and the runtime need entries among the other libraries (Windows only, Linux only). The CLI and the Python wheel need notices of their own when they are distributed (1.2).
11. **No way to the notices inside the app.** LGPL 2.1 section 6 asks for a prominent notice with each copy. The bundles and the web app carry the file, but nothing in the UI names it. A command in the palette (on the web, opening `/THIRD-PARTY-NOTICES.txt`; on the desktop, naming where the file is installed) would fix that; it is for ui.md to specify, with ADR 0034. Section 6 also asks that a work which shows copyright notices while it runs show OCCT's among them, with a reference to the license. On macOS, Tauri's default menu, which the app keeps (it sets no menu of its own), shows an About Partsmith panel with `bundle.copyright` of `tauri.conf.json` (desktop.md 2.8), today `Copyright the Partsmith contributors`; so that text gains OCCT's notice, for example `Copyright the Partsmith contributors. Includes Open CASCADE Technology, copyright OPEN CASCADE SAS, under the GNU LGPL 2.1; see THIRD-PARTY-NOTICES.txt.`, a desktop.md change.
12. **The web notices name no commit** (resolved by 2.13). One committed file serves every deployment, production and previews alike, so the web statement cannot name the commit itself; it names `/SOURCE.txt`, the stamp the `web` job writes into each deployment, previews included, and `deploy` requires (ADR 0034 option A, item 4). Deployments made before the stamp landed have none.
13. **Two bundle builds per release.** The checklist (3.6) runs a dry run before the tag, and the tag run builds every bundle again from the same commit, so each release runs the Windows and macOS jobs twice (section 8). The tag run could take the dry run's `bundle-*` artifacts of that commit instead (`actions/download-artifact` with the dry run's `run-id`), which halves the cost; that needs `verify` to find the dry run and a change to 2.5, and is left for the implementer of `release.yml` to propose with the first measured figures.
14. **`sketch/face-ambiguous` shows a lost face.** Since phase 1 of [the reference stability study](../design/reference-stability-and-errors.md), the resolver never takes a face by its fingerprint alone, and a face reference is ambiguous only between imported faces, whose fallback names are confirmed by their shape (naming.md section 3.9, step 1). The stored face of `sketch/face-ambiguous`, `{q: f-box1/nothing, fp: plane|a=100}`, is therefore missing, and its expected results equal those of `sketch/face-lost`. `sketch/imported-face-ambiguous` shows the ambiguous case of `plane_fallback` (features/sketch.md section 3.5) on an imported cube instead. `sketch/face-ambiguous` is to be removed, or renamed after what it shows (a lost face whose stored size fits every face of the box stays lost and is never replaced by a lookalike); either way its three files go, which waits for the review of section 8.
15. **The web's OCCT source.** ADR 0034 reads sections 4 and 6(d) as asking for OCCT's source from the place that distributes the build, and its question 1 for a lawyer assumes a link to the upstream tag is not enough. Under that reading the web falls short today: production, every preview and CI's public artifacts (`core-wasm`, `web-dist`, the `bundle-*` artifacts of release runs, and the `playwright-report` of a failed `e2e` run, whose traces can hold the core) carry OCCT's object code, while a deployment's commit reaches OCCT's source only through `scripts/occt-source-archive.sh` and `scripts/build-occt.sh`, which clone the upstream tag. No published release carries `occt-{OCCT_VERSION}-source.tar.gz` yet, and after a change of the OCCT pin none carries the new version's until the next release. The owner chooses, and nothing here decides: write the archive into every deployment (17.5 MB, under the 25 MiB limit of 3.5), as the private variant of option A planned; publish a release carrying each pin's archive before `main` deploys that pin; or rely on the upstream tag if the lawyer answers question 1 that way.

## 8. Needs human review

- **Cloudflare setup** (one time): create the Pages project with production branch `main`, create an API token with only "Account, Cloudflare Pages, Edit", add `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` as repository secrets, and optionally `CLOUDFLARE_PAGES_PROJECT`. Decide whether the account belongs to a person or a project organization (technical design section 19). A project connected to the repository in the dashboard has its automatic builds paused (3.3 item 6). Status 2026-10-16: done. `main` is the default branch, the Git-connected Pages project has its own builds paused and receives CI's Wrangler deploys, and both secrets are set. The owner made the repository public and accepted ADR 0034 option A, B-2's notices and the notices rule of `check:dist` are on `main`, and no guard holds deployments back: the preview access policy and the production hold-back that ADR 0034's recommendation named as interim guards for the private period are not set up. After four red runs, CI passed on `main` in run 38939853875 (commit 55dde8a), whose `deploy` job made the first production deployment on 2026-10-16, before the source route of 2.13 was in place, which the recommendation's hold-back waited for. That deployment, like every one before the source stamp of 2.13 lands, has no `/SOURCE.txt`; the repository's deployments page lists the commit of each production deployment. Until 2.13 is on `main`, a pull request that does not carry it deploys a preview with the web statement now on `main`, which says a preview is built from the head commit of its pull request (it is built from GitHub's merge commit, 2.13), and with no stamp; so the change that lands 2.13 is best the first pull request to run CI, or else the owner turns on the preview access policy (Settings, General) until 2.13 is on `main`. README.md gives the production address once the owner confirms the subdomain (2.12 part 3).
- **Branch protection** on `main` requiring the CI jobs of 2.4. Status 2026-10-16: not set (`main` is unprotected). `main` passed CI in run 38939853875, so the jobs can be required now without blocking every merge. Protection also keeps force pushes and deletion of `main` off (GitHub's default for a protected branch), which the stamps of production deployments rely on; a tag ruleset that blocks deleting or moving `v*` tags does the same for releases.
- **Third-party notices** (2.9, ADR 0033): what they list and leave out; the two OCCT statements; the four notices of code OCCT includes from other projects (flex, DELABELLA, David M. Gay's strtod, Unicode's ConvertUTF); the texts given to the 22 crates that ship none (a standard Apache-2.0 or MPL-2.0 text, or a committed copy from the crate's repository); the WebView2 SDK loader that `webview2-com-sys` links statically on Windows, whose license the owner confirms against the NuGet package that crate version ships (checked 2026-10-16: the `x64/WebView2LoaderStatic.lib` of `webview2-com-sys` 0.39.1 is byte for byte the one of `Microsoft.Web.WebView2` 1.0.3800.47, the SDK version its repository's `update-bindings` names at the packaged commit, and that package's `LICENSE.txt`, a BSD-3-Clause text with Microsoft's copyright line, is the committed `ui/scripts/notices/webview2-com-sys-0.39.1/LICENSE.txt`; the package's `NOTICE.txt`, for Antlr3.Runtime and StringTemplate4 under BSD-3-Clause, covers its `tools/wv2winrt` code generator, not the loader, so the notices leave it out); the text of `block2`, `objc2`, `objc2-encode` and `objc2-foundation`, whose repository's only license file, `LICENSE.md`, names MIT and links to it but holds neither the MIT text nor a copyright line, so their entries carry that file as it is (the owner may add, in each of the four `ui/scripts/notices/crates/<name>/` folders, the MIT text that `LICENSE.md` links to, `https://opensource.org/license/MIT`, which still has no copyright line); the same `LICENSE.md` says the objc2 crates are derived from Apple SDKs and that it is unclear whether the Xcode license allows distributing such derived works, which concerns every crate of that repository the macOS bundle compiles in (`block2`, `dispatch2`, `objc2`, `objc2-encode`, `objc2-exception-helper`, `objc2-foundation` and the six `objc2-*` framework crates): a question for the owner's lawyer with those of ADR 0034; the Unicode data license of `regex-syntax`'s tables (Unicode-DFS-2016, which `deny.toml` does not allow and `cargo-deny` never sees, since the crate declares `MIT OR Apache-2.0`): allow it with an ADR, or accept it as data rather than code; whether the NSIS installer code (NSIS, its LZMA module, `nsis_tauri_utils`) and the AppImage runtime get entries (open issue 10); the shipping places (bundle resource, release asset, web build), with no license page or agreement in any installer; and the texts reproduced as their packages ship them. The ICU4X crates' Unicode-3.0 texts hold em dashes, which the root AGENTS.md forbids in any file; ADR 0033 sets out three ways (amend the rule with the wording `Never write the em dash character in any file, except third-party license texts copied verbatim: the notices that pnpm -C ui notices writes and the copies it reads under ui/scripts/notices/ (delivery.md 2.9).`, which it proposes; write U+2014 as `--`, which alters license texts; or hold the notices back, which blocks B-2) for the owner to choose. And the line `pnpm -C ui notices` in the Commands block of the root AGENTS.md (2.9, "Kept current"), which the owner adds when this work merges `main`.
- **How OCCT is linked** (ADR 0034). Decided by the owner on 2026-10-16: option A, static linking everywhere, to be met with published source, and the repository made public. This spec carries it out with the source stamp of every web deployment and the OCCT source archive of every release (2.13, tests 52 to 58), the statements of 2.9 that name them, the web notices in the `core-wasm` artifact (2.4) and 7-day artifacts for the bundles (2.5), which test 59 checks; until the archive step is in `draft`, a release stays a draft (3.6, checklist item 1). Still open for the owner: whether the AppImage stays in M1 (until then no release with an `.AppImage` is published and no release run, dry run or tag, builds one, checklist item 2), and the four questions ADR 0034 asks a lawyer, of which question 1 decides whether the web falls short now (open issue 15). Still to specify, in ui.md and desktop.md: the in-app notice of open issue 11, the macOS About panel included. Wording the owner changes: the invariant of the root AGENTS.md still says OCCT is "linked as a separate library", for which ADR 0034's recommendation gives the replacement, and `.github/workflows/AGENTS.md` still holds the old items 1 and 2 of the release checklist, describes the `core-wasm` and `draft` artifacts without the notices and the OCCT archive, and says the repository is private; technical design sections 3 and 17 follow in a change of their own. README.md's OCCT sentences follow 2.12 part 7.
- **npm license policy** (2.10, ADR 0033): the npm packages that ship (the production dependencies, and `rolldown`, `tailwindcss` and `vite`, whose code the build writes into `ui/dist`) are held to `deny.toml`'s allow list; the other development dependencies, three of which are under MIT-0, Python-2.0 and BlueOak-1.0.0, do not ship and are not checked. The eleven packages are logged in ADR 0033.
- **Python 3.13 on CI** (2.4 `python`, python-api.md test 76): a second interpreter on the runner, and the wheel build and two installs added to the job's time.
- **The nightly workflow** (2.11): its schedule, its runner time (about 30 to 90 minutes a month, not billed on this public repository) and its skip rule, under which any finished run, including a failed, timed-out or canceled one, counts as having tested its commit.
- **`deny.toml` advisory exceptions** RUSTSEC-2024-0429 and RUSTSEC-2024-0370 for Tauri's GTK3 stack (3.2), a change to the dependency policy.
- **Release policy**: tags `vX.Y.Z` on commits with green CI, a dry run from `main` before each tag, draft releases published by a human, unsigned bundles until signing is set up, macOS arm64 only. Its cost: each release builds the bundles twice, in the dry run and in the tag run, with bundle jobs of about 20 minutes each (an estimate the first dry run replaces with measured figures), and a cold OCCT build adds up to the cold budget of section 5. The repository is public, so its standard GitHub-hosted runners, Windows and macOS included, bill nothing (ADR 0034), and the cost is the release's wall time; open issue 13 would halve the bundle time. A hung job ends at its `timeout-minutes` (2.4, 2.5), 150 minutes for a bundle job.
- **Build tools pinned here** (`tauri-cli` 2.12.2, Wrangler 4.149.0, maturin 1.15.0, pytest 9.1.1) and the new actions (`upload-artifact`, `download-artifact`, `setup-python`, `wrangler-action`, `msvc-dev-cmd`).
- **Examples rules**: the expected-results format and the 1e-6 tolerance, and the bless workflow.
- **Examples blessed in phase 1 of the reference stability study** (its section 10.3 item 11, which asks for the sign-off of technical design section 16; [ADR 0021](../adr/0021-resolve-references-by-meaning.md), [ADR 0022](../adr/0022-split-regions-keep-their-area.md) and [ADR 0024](../adr/0024-diagnostics-with-actions-causes-and-ghosts.md)). Thirteen expected results were blessed again for the new messages (cli.md open issue 10): `chamfer/edge-lost`, `fillet/one-edge-lost` and `shell/one-face-lost` (lost references with their reason, candidates and fixes), `extrude/region-by-fingerprint` and `extrude/region-missing` (region texts and fixes), `import/cut-through-plate`, `import/join` and `import/new-body` (the asset name in `detail`), `sketch/face-ambiguous`, `sketch/face-lost`, `sketch/face-not-flat` and `sketch/face-plane-empty-body` (the Pick action, and the lost face of open issue 14), and `extrude/region-by-outer-boundary`; besides messages, they change only the last digits of volumes, areas and centers of mass. `extrude/region-by-outer-boundary` alone changes geometry: its redrawn hole circle now fills the hole (features/extrude.md test 31, features.md open issue 10), so its volume goes from 36858.407 to 40000, its faces from 7 to 6, its edges from 14 to 12, and its status from warning (`region_remapped`) to ok with the notice `region_split`. Its name describes the old match by outer boundary; it is renamed once features.md open issue 10 is decided. `sketch/face-ambiguous`, whose results now equal those of `sketch/face-lost`, is to be removed or renamed (open issue 14). New in the same step: the location `examples/references/` (2.8) with `line-redrawn`, `slot-inserted` and `notch-replaces-line`, each with its twin; `sketch/imported-face-ambiguous` with its STEP asset (open issues 5 and 14); and the `outline` of every stored region (`Region::reference()` of the unchanged sketch, [ADR 0025](../adr/0025-file-format-version-2.md)) in the 33 extrude and revolve examples whose regions are picks of their current sketch, so that their twins write regions as a host stores them: the short form `extrude(sketch_1, 30*mm, id="f-ext1")` where the profile is every region of the sketch, and the stored outline where the profile is a chosen region (`Region(..., outline=...)` in `extrude/region-with-hole`) or the extent has no dedicated form (the generic `feature("extrude", ...)` of `extrude/two-sides`, `two-sides-reverse` and `two-sides-one-zero`); their results are unchanged. Regions without outlines are left only where the stale key is the example's point (`extrude/region-by-fingerprint`, `extrude/region-missing` and `extrude/region-by-outer-boundary`) and in `bench/fifty-features`, which `ui/e2e/fixtures/part-50.ptsm` must equal byte for byte (ui.md 6.2), so its twin spells out its 13 regions until both files are updated together.
- **Examples blessed in phase 2 of the reference stability study** (its section 10.4 item 3, [ADR 0023](../adr/0023-frames-live-on-the-model.md)): `sketch/face-ambiguous`, `sketch/face-lost` and `sketch/imported-face-ambiguous` were blessed again because their `plane_fallback` warnings now offer `Attach to {description}` for each candidate before `Pick a face again` (engine.md section 3.22, features/sketch.md section 3.5); nothing else in them changes. New in its section 10.4 item 6: `references/moved-face-sketch` (R32 (a)), `references/sketch-after-cut` (R33) and `linear_pattern/leaning-edge` (R38), each with its twin and the references its setup picked (2.8). `sketch-after-cut` records `f-ext2` as ok with no message, where features/sketch.md test 22 expects `separate_solid`, which phase 3 step 4 adds; its expected results are blessed again in that step.
- **Examples blessed in phase 3 of the reference stability study** (its section 10.5 items 3 to 5 and 8, [ADR 0024](../adr/0024-diagnostics-with-actions-causes-and-ghosts.md)): twenty-one expected results were blessed again for the messages of features.md sections 3.15 and 4 and sketch.md section 4 and the fixes of engine.md section 3.22. `no_effect` says how nothing changed in `boolean/cut-misses` and `extrude/cut-misses` (`Beside`), `circular_pattern/count-one` and `linear_pattern/count-one` (`NoCopies`), `mirror/symmetric` (`Inside`) and `move_body/nothing-moves` (`NotMoved`); a lone smooth edge is skipped at its entry with Remove from Edges and no second `no_effect` in `chamfer/pipe-tee`, `chamfer/smooth-edge`, `fillet/pipe-tee`, `fillet/smooth-edge` and `fillet/smooth-edge-next-to-torus`; a hole that misses is named by its sketch coordinates, with Turn off Reverse where the other way reaches the body and no second `no_effect` for one point, in `hole/all-points-off-body`, `hole/point-off-body`, `hole/reverse` and `hole/swallowed-solid`; a join that floats warns `separate_solid` in `hole/swallowed-solid`, `shell/two-solids` and `references/sketch-after-cut` (Attach and the root `f-sk2`, features/sketch.md test 22); the three `import/` examples get the new missing-file text with the asset's path in `detail`; and `sketch/redundant-dimension` offers Make a reference and Delete. Besides messages and the statuses their warnings set (`f-box2` of `hole/swallowed-solid` and `shell/two-solids` and `f-ext2` of `references/sketch-after-cut` go from ok to warning), they change only the last digits of volumes, areas and centers of mass. New in its section 10.5 item 8: `hole/deleted-point` (R43 (b)), whose twin writes the hole in the generic form because no point of the sketch stands for the deleted `e-p2`. Like every twin, it equals the generated script: the generator gives no variable to a sketch or point that a feature looked up before falling back to the generic form (python-api.md section 3.8, rule 7), which also drops the unused `as sketch_1` from the twins of `extrude/two-sides`, `two-sides-reverse` and `two-sides-one-zero`.
- **The 25 MiB plan** (3.5): each step's ADR when the gate first fails.
