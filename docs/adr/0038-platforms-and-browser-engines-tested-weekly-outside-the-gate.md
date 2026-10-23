# 0038: Windows, macOS, Firefox and WebKit are tested every week outside the deploy gate, and the browsers and webviews Partsmith supports

Status: Proposed, needs human review: the runner time and caches of the new workflow, the two browser builds it installs, the release checklist's new item, and the minimum versions as a promise to users (the macOS one adds to desktop.md's macOS minimum, a review item). The rest (the workflow's jobs, the engine subset, the promotion rule and the desktop host's rename retry) needs no review.

## Context

Gap list items B-8 and B-9 ([m1-gap-list.md](../design/m1-gap-list.md)).

- **Platforms.** `ci.yml` runs every job on `ubuntu-24.04`. The desktop bundles ship for Windows (`windows-2025`) and macOS (`macos-15`) too (delivery.md 3.6), but nothing builds or tests the kernel, the engine or the desktop host there: the release dry run is the first MSVC and Apple Clang build, and it bundles without running a test (delivery.md open issue 1). Code that reads platform rules is in the desktop host's file access (atomic replace, read-only files, links, junctions, path parsing), in `partsmith-occt-sys/build.rs` (system libraries) and in `scripts/build-occt.sh` (the Windows install layout).
- **Browser engines.** Playwright runs only the `chromium` project (`ui/playwright.config.ts`), with Chromium 156.0.8078.4 of `@playwright/test` 1.64.0, whose `browsers.json` also pins Firefox 157.0 and WebKit 27.2. The desktop webviews are WebView2 (Chromium) on Windows, WKWebView on macOS and WebKitGTK on Linux (desktop.md 1), so two of the three are WebKit, which no test runs. Nothing states which browsers the web app supports or from which version, except the unsupported page's "a current version of Chrome, Edge, Firefox or Safari" (ui.md 4.3).
- **What the UI needs.** Every menu, popover and tooltip is a native popover (`showPopover`, `:popover-open`; Chrome 114, Firefox 125, Safari 17.0). Tailwind CSS 4's stylesheet needs Chrome 111, Firefox 128 and Safari 16.4. The web core needs WebAssembly with the legacy exception handling that Emscripten 4.0.15 emits by default (`WASM_LEGACY_EXCEPTIONS`), a module worker and cross-origin isolation (Chrome 95, Firefox 114, Safari 15.2). Recovery and recent files need IndexedDB and Web Locks. `@starting-style` (Chrome 117, Firefox 129, Safari 17.5) only animates popups as they open, and the `source` of `showPopover` only keeps a popup's own anchor from light-dismissing it. The first versions come from MDN's compatibility data; nothing here tests them.
- **Tests that cannot run elsewhere.** E-150 to E-154 test the File System Access API, which only Chromium has; `stallCore` relies on `page.route` serving the core worker's requests, `watchLongTasks` on `longtask` entries and E-146 and V-22 on `forcedColors` emulation, all of which Playwright offers in Chromium alone; the `@visual` baselines and `@perf` budgets are Chromium's.
- **The gate.** `deploy.needs` lists every CI job (delivery.md 2.4), and branch protection is to require them (delivery.md 8), so a job in `ci.yml` that fails blocks every merge and deploy. CI's wall time budget is 25 minutes (delivery.md 5). The repository is public, so standard runners, Windows and macOS included, bill nothing (ADR 0034). GitHub keeps at most 10 GB of caches per repository and drops caches unused for 7 days.
- **Windows file replacement.** The host writes a file by renaming a new temporary file over it (desktop.md 3.4 step 4). On Windows a rename fails while another program holds the target or the new file open without sharing its deletion, as antivirus scanners and indexers do for a moment after a file is written; rustup and npm's `graceful-fs` retry such renames for this reason.

## Decision

1. **A workflow of its own, outside the gate.** `.github/workflows/platforms.yml` (delivery.md 2.14) runs on `workflow_dispatch` and every Monday at 05:41 UTC, never on `push`, `pull_request` or `workflow_call`, so no merge, deploy or release run waits for it. Its jobs: `occt-platforms` builds or restores OCCT on `windows-2025` and `macos-15` with the setup and cache key of `release.yml`'s `bundle` job; `rust-platforms` runs `cargo test --no-fail-fast -p partsmith-occt-sys -p partsmith-engine -p partsmith-desktop` and the examples runner there; `occt-wasm` and `core-wasm` build the web core as CI does; `e2e-engines` runs the engine subset in Firefox and in WebKit on `ubuntu-24.04`. Every expected result is the same on every platform: the examples keep one expected file each.
2. **Promotion.** A job, or one row of its matrix, moves into `ci.yml` unchanged once it has passed in the two latest runs of `platforms.yml` on `main`, joins `deploy.needs`, and leaves `platforms.yml`; the owner adds it to the required checks. A job that would make CI longer than its 25-minute budget moves only with a budget change, which needs review.
3. **The engine subset.** The E-tests tagged `@engines` (ui.md 6.8), 25 tests that run as written on every engine and reach every platform feature the UI needs: the core, WebGL2 drawing and picking, pointer input, popovers, sketch mode, the file chooser, drag and drop and downloads, IndexedDB and Web Locks, `beforeunload`, color schemes and settings, and fonts. They run in all three projects; every other E-test runs in Chromium alone. `ui/playwright.config.ts` adds the `firefox` and `webkit` projects only when `PARTSMITH_ENGINES=1`, so `pnpm e2e` keeps needing Chromium alone.
4. **Supported browsers and webviews** (ui.md 1.1, desktop.md 1): the current Chrome, Edge, Firefox and Safari on desktop, at least Chrome and Edge 114, Firefox 128 and Safari 17.0, the first versions with every feature the UI relies on; on the desktop, WebView2 114 or later, WKWebView of Safari 17.0 or later on macOS 13 or later, and WebKitGTK 4.1 2.44 or later. macOS stays at 13.0 (option (a) below), and the README and desktop.md say that the app needs Safari 17 or later there.
5. **Releases wait for it.** The release checklist (delivery.md 3.6, item 3) asks for a dispatched platforms run that passed on the commit to release, beside CI.
6. **The desktop host retries a busy rename on Windows** (desktop.md 3.4 step 4.5): after `ERROR_ACCESS_DENIED`, `ERROR_SHARING_VIOLATION` or `ERROR_LOCK_VIOLATION`, it tries again after 10, 20, 40, 80, 160 and 320 ms (630 ms in all); every other error, and every error on other systems, fails at once.

## Options considered

**Where the platform jobs run.**

- (a) A workflow of its own, on demand and weekly, whose jobs move into CI once they pass (chosen). Nothing turns red on pull requests while the platforms are unproven, the orchestrator dispatches it from `main` and iterates on its findings, and the weekly run keeps the Windows and macOS OCCT caches that tag runs restore alive on `main`.
- (b) A matrix in `ci.yml` now. Every pull request would wait for a cold OCCT build on two more platforms and fail on findings nobody has had a chance to fix, and CI's wall time would pass its 25-minute budget.
- (c) Tests in `release.yml` before bundling. They would run only before a tag, months apart, and an untested platform would be found at release time.
- (d) Jobs in `ci.yml` with `continue-on-error`. A job that is allowed to fail shows green, so its findings go unread.

**How a job moves into CI.**

- (a) Its YAML moves into `ci.yml` (chosen). The jobs read as every other CI job, and nothing is left in two places.
- (b) `platforms.yml` as a reusable workflow (`workflow_call`) that `ci.yml` calls. No YAML moves, but a local `uses:` names no release (delivery.md 2.2), inputs would choose its jobs, and scheduled runs have no inputs.

**Which E-tests run in Firefox and WebKit.**

- (a) A tagged subset that runs as written everywhere (chosen). It covers every platform feature at a fraction of the time, and its failures are the app's.
- (b) Every E-test. About a dozen rely on what only Chromium has and would need engine-specific skips, and most of the others repeat in two more engines the logic Chromium already covers, at about three times the time.
- (c) Chromium only, as today. Two of the three desktop webviews and two of the four browsers stay untested.

**macOS minimum.**

- (a) Keep macOS 13.0 and require Safari 17 or later (chosen). macOS 13 runs Safari 17 and 18, and WKWebView uses the WebKit that the installed Safari brings, so an updated Mac 13 has every feature; nothing in the build changes.
- (b) Raise `minimumSystemVersion` and `MACOSX_DEPLOYMENT_TARGET` to 14.0, which shipped Safari 17. The requirement becomes one number, but it drops every Mac on 13 and changes `tauri.conf.json`, the release build and the OCCT cache key.

**A busy rename on Windows.**

- (a) Retry a few times over less than a second (chosen), as rustup and `graceful-fs` do. A rename that another program blocks for a moment succeeds, and a lasting block fails within 630 ms.
- (b) Report the first failure. A save fails now and then for a reason the user cannot see.
- (c) `ReplaceFileW`, which keeps the target's attributes and security. It needs `unsafe` or the `windows` crate as a dependency, and fails with its own errors for the same busy files.

## Consequences

- CI stays as it is; the first runs of `platforms.yml` are expected to find build and test failures on Windows and macOS (delivery.md open issue 1) and in Firefox and WebKit, which are fixed in follow-up changes until each job passes twice and moves.
- Weekly runner time grows by roughly an hour, free on this public repository, and four more caches (two OCCT installs and two Rust caches) share the 10 GB limit with CI's; their sizes are recorded with the first runs (delivery.md open issue 16).
- Playwright's Firefox (MPL 2.0) and WebKit (LGPL 2.1 and BSD) builds are installed on the runners as test tools, as its Chromium is today; they are never distributed.
- A browser below the minimums that has WebAssembly and WebGL2 still opens the app, with popups it cannot show (ui.md open issue 40). Playwright's WebKit runs on Linux, nearest to WebKitGTK; WKWebView on macOS is still covered only by the desktop checks at the gate (ui.md open issue 41).
