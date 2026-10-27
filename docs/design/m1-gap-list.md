# Partsmith M1 gap work list (after the reference stability phases)

This list merges the 43 confirmed gaps and the 11 critic additions. Duplicates are combined (see "Merged inputs" at the end). Two things are left out: the reference stability plan (phases 1 to 4) and ui.md open issue 17.

It was produced on 2026-10-13 by comparing the technical design and every spec with the code, area by area, with each gap checked by a reviewer who tried to refute it. Items R-1 and R-2 are already folded into section 10 of [reference-stability-and-errors.md](reference-stability-and-errors.md). Items marked as needing a human (B-1 and B-2) wait for the repository owner.

Legend:
- **Sizes:** S is one small PR. M is several PRs in one area. L spans hosts or crates and needs a spec round.
- **Spec first:** the spec text that must merge before any code.
- **Human gate:** a license, dependency, schema, public API or capability change that waits for review (AGENTS.md Process).

These claims were spot-checked against the repo while planning, and all hold:
- The only branch is `claude/tender-sagan-4ek456`.
- There is no notices file at the root.
- `ui/src/files/recovery.ts` has a single `SLOT = 'current'`.
- `ui/src/core/api.ts:223` sends `assets: []`.
- `scheduleRecovery` in `ui/src/app/createApp.ts` never checks `transactionOpen`.
- `Viewport.tsx` shows "Restart core" only when the status is `failed`.
- `ui/index.html` has no color-scheme meta.
- `ui/playwright.config.ts` defines only a chromium project.

## Ranked index

| Rank | ID | Item | Owner | Impact | Size |
| --- | --- | --- | --- | --- | --- |
| 0 | R-1 | Mirrored drawing API (sketch.md 3.17) missing from the plan | partsmith-sketch, plan 10.4 | low (blocks the phase 2 exit) | S plan, M code |
| 0 | R-2 | P-9 fixture and `ps:drop-check` mark belong to no phase | ui, plan 10.6 | low | S |
| 1 | B-1 | First CI run, `main`, Pages deploy, release dry run | repo setup, .github/workflows | blocker | M |
| 2 | B-2 | Third-party notices and OCCT LGPL obligations | root, desktop bundle, ui/public, release.yml | high | M |
| 3 | B-3 | Recovery keeps one slot, so unsaved work gets overwritten | ui/src/files, ui/src/app | high | M |
| 4 | B-4 | Recovery copy stores uncommitted preview edits | ui/src/app | medium | S |
| 5 | B-5 | Web and desktop hosts ignore document assets | ui/src/core, ui/src/files, desktop/src-tauri | medium | L |
| 6 | B-6 | Save overwrites a file that changed on disk | ui/src/files, desktop/src-tauri | medium | M |
| 7 | B-7 | No way out of a core stuck in a long call | ui/src/app, ui/src/core, ui/src/viewport | medium | M |
| 8 | B-8 | Windows and macOS are never tested | release.yml or ci.yml | medium | M |
| 9 | B-9 | UI is tested only in Chromium | ui/e2e, playwright config, delivery | medium | M |
| 10 | B-10 | Wheel packaging test 76 is not in CI | ci.yml python job | medium | S |
| 11 | B-11 | Release-gate perf tests P-1 to P-8 | ui/e2e | medium | M |
| 12 | B-12 | 250 KB initial JavaScript check (already at 92 percent) | ui/scripts/check-dist.mjs | medium | S |
| 13 | B-13 | README, install notes, release checklist | README.md, .github/workflows/AGENTS.md | medium | S |
| 14 | PRINT-1 | Show split-plane and drain-hole suggestions | ui/src/print, ui/src/viewport, python | medium | M |
| 15 | PRINT-2 | Custom printer profile route, or mark it Later | partsmith-cli, python | low | S or M |
| 16 | PRINT-3 | Protocol schema of listed printer profiles | partsmith-print, partsmith-protocol, ui | low | M |
| 17 | EX-1 | STEP import examples that build, intersect and ImportWarning tests | examples/import, recompute, occt-sys | medium | M |
| 18 | EX-2 | Bed fit, islands, tall-and-thin and resin examples | examples/box, boolean, shell | medium | M |
| 19 | EX-3 | Sketch constraint and tool examples, arc-bounded profile | examples/sketch, examples/extrude | medium | L |
| 20 | EX-4 | Shell concave-edge join test | feature_shell.rs, examples/shell | medium | S |
| 21 | EX-5 | Feature option examples (primitive ops and placements, axes, hole arc points) | examples/<type>, feature_<type>.rs | medium | L |
| 22 | EX-6 | Export coverage in the examples corpus | delivery.md 2.8, examples.rs | medium | M |
| 23 | EX-7 | Chamfer corner faces test | occt-sys, feature_chamfer.rs, examples/chamfer | low | S |
| 24 | EX-8 | Turn inline geometry tests into examples | examples/*, feature tests | low | M |
| 25 | EX-9 | Corpus coverage checks | crates/partsmith-cli/tests/examples.rs | low | M |
| 26 | EX-10 | GUI to Python round trip on documents the UI writes | python/tests, ui/e2e fixtures | low | M |
| 27 | ENG-1 | Native and WebAssembly result parity | partsmith-wasm tests, ci.yml | medium | M |
| 28 | ENG-2 | Engine web-column benchmark | ui/e2e or partsmith-wasm | medium | M |
| 29 | ENG-3 | `cache_budget` does not bound the total | partsmith-engine | low | S |
| 30 | ENG-4 | Panic containment on the wasm build | partsmith-wasm, ci.yml | low | S |
| 31 | ENG-5 | `display_failed` and `duplicate_element_name` | partsmith-engine | low | S |
| 32 | ENG-6 | Byte-for-byte golden 3MF | partsmith-engine tests | low | S |
| 33 | UI-1 | color-scheme meta | ui/index.html | low | S |
| 34 | UI-2 | Web CSP and hardening headers | ui/public/_headers, check-dist, vite | low | S |
| 35 | UI-3 | Tab order and target size tests | ui/e2e | low | M |
| 36 | UI-4 | Measurement points for two ui.md 5 rows | ui/src/state, ui/src/expression, marks.ts | low | S |
| 37 | UI-5 | Worker transfer budget bench | ui/src/core | low | S |
| 38 | PY-1 | Save writes assets before the document | python/partsmith/document.py | low | S |
| 39 | PY-2 | Dedicated Python keywords for three GUI options | python/partsmith/features.py, feature hooks | low | M |
| 40 | PY-3 | Python budgets bench | python/ | low | M |
| 41 | SK-1 | Sketch results on wasm within 1e-9 | partsmith-sketch, ci.yml | low | S |
| 42 | SK-2 | Nightly 4096-case property run | .github/workflows | low | S |
| 43 | CLI-1 | Correct cli.md open issue 1 | docs/spec/cli.md | low | S |
| 44 | CLI-2 | CLI budgets bench | partsmith-cli | low | M |
| 45 | PROTO-1 | Stale `Edit` note in the protocol AGENTS.md, plus a test for it | partsmith-protocol | low | S |
| 46 | DEL-1 | CC0 legal text and scope | root, README, ADR | low | S |
| 47 | DEL-2 | npm license check in CI | ui/scripts, ci.yml, ADR | low | S |
| 48 | IMP-1 | Import warnings show the kernel's sentences | partsmith-occt-sys, partsmith-features | low | S or M |

## 0. Fold into the reference stability plan now

These two are gaps in `docs/design/reference-stability-and-errors.md` section 10 itself. Fix them in the plan text, not after the phases.

**R-1. Mirrored drawing API** (partsmith-sketch)
- **Spec:** sketch.md 1.1, 2.2, 3.17, 6.21 (tests 248 to 250).
- **Spec first:** no spec change, the spec is already written. Plan text only.
- **Work:**
  - Add a phase 2 step before 10.4 step 1: `SketchParams::mirrored`, `mirrored_point` and `mirrored_region`, tests in a new `tests/mirrored.rs`, and the interface in the crate's AGENTS.md.
  - Cite that step from 10.4 step 1 (`TurnDrawingBack`, features/sketch.md test 24) and step 3 (engine `apply_fix`, engine.md test 135).
  - Add sketch.md tests 252 and 253 to 10.3 step 2.
  - Give test 251 (NC08 `NotConverged`) to the phase 3 sketch messages step.
- **Impact:** low, but phase 2 cannot exit without it. **Size:** S for the plan, M for the code.
- **Depends on:** `mirrored_region` needs phase 1 step 2 (`RegionRef.outline`, `RegionLoop`).

**R-2. P-9 belongs to no phase** (ui)
- **Spec:** ui.md 2.18, 5, 6.7 P-9.
- **Spec first:** no.
- **Work:**
  - `ui/e2e/fixtures/part-300.ptsm` does not exist.
  - `ps:drop-check` is missing from `ui/src/performance/marks.ts` `TIMING_NAMES` and nothing emits it.
  - Add both, and test P-9, to the phase 4 ui step next to the drop warning.
- **Impact:** low. **Size:** S.
- **Depends on:** phase 4 E6 (`list_uses`) and B-11 (the shared perf spec file).

## 1. Blockers for a usable M1 release

B-1, B-2 and B-3 can start alongside the phases, since they do not touch phase code. B-1 and B-2 also need a human now.

**B-1. CI has never run: no `main`, no deploy, no release dry run** (repo setup, `.github/workflows`)
- **Spec:** delivery.md 1, 3.1, 3.3, 3.6, 6 (tests 1 to 13), 8. Technical design 4 and 17.
- **Spec first:** small. Add "create `main` and make it the default branch" to delivery.md 8.
- **Work:**
  1. Human setup:
     - Create `main` from the current head and make it the default branch.
     - Protect it, requiring occt-native, occt-wasm, kernel-wasm, rust, deny, ui, schema, core-wasm, web, e2e, python and desktop.
     - Create the Pages project with production branch `main`.
     - Add the `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` secrets.
  2. Open a PR into `main`. That gives the first full CI run and a preview URL (tests 1 to 12).
  3. Merge it. That gives the production deploy and saves the Linux caches on `main`.
  4. Dispatch release.yml from `main`. That runs test 13 and saves the Windows and macOS caches.
  5. Record the outcome in delivery.md open issue 1, and fix whatever the first runs expose.
- **Impact:** blocker. **Size:** M. The setup is small, but the fixes the first runs need are unknown.
- **Depends on:** a human repository owner. Every item that adds a CI job depends on this one.
- **Status (2026-10-16):** the human setup of step 1 is done, apart from branch protection. `main` is the default branch, the Git-connected Pages project has its own builds paused and takes CI's Wrangler deploys, and both Cloudflare secrets are set. The owner made the repository public and accepted ADR 0034 option A, B-2's notices and `check:dist` rule are on `main`, and no guard holds deployments back (the preview access policy and the production hold-back that ADR 0034's recommendation named as interim guards for the private period are not set up; production deployed before the source stamp of delivery.md 2.13, so until 2.13 lands deployments carry none, and its change is best the first pull request to run CI). After four red runs on pushes to `main` (38431025954, 38446384392, 38492295214 and 38857833462; the yanked `zip` 9.0.1 that failed deny in the last one is now 9.0.3), CI passed on `main` in run 38939853875 (commit 55dde8a, the push that brought reference stability phase 1 to `main`), and its `deploy` job made the first production deployment. That covers step 3 without the pull request of step 2: no pull request has run CI yet, so no preview exists. `main` is not protected yet; now that it is green, the jobs can be required. Step 4, the release dry run, waits for the AppImage choice of ADR 0034 (delivery.md 3.6, checklist item 2: the run's public `bundle-Linux` artifact would hold the AppImage), then step 5. delivery.md 8 records this. Every job sets `timeout-minutes` (150 for the OCCT builds and the release `bundle`, 60 for the others, 20 for the nightly; delivery.md 2.4 and 2.5, test 49), so a hung job ends long before GitHub's default of 360 minutes; the values come from the measured runs (cold OCCT builds of 56 and 88 minutes, no other job above 16). On the public repository, standard runners bill nothing.

**B-2. Third-party notices and OCCT LGPL compliance** (repo root, desktop bundle, web build, release)
- **Spec:** technical design 3 and 17, ADR 0003, LGPL 2.1 section 6.
- **Spec first:** yes.
  - A new delivery.md notices section, with section 6 tests.
  - The desktop.md `bundle` object, plus the exact-object test in `desktop/src-tauri/tests/config.rs`.
  - An ADR for the generator tool, or a row in ADR 0020.
  - A new ADR that resolves a contradiction: AGENTS.md and technical design 3 say OCCT is "linked as a separate library", but ADR 0003 links it statically. Either the wording or the linking changes.
- **Human gate:** licenses and a new tool.
- **Work:**
  - Collect the notices:
    - OCCT's `LICENSE_LGPL_21.txt` and `OCCT_LGPL_EXCEPTION.txt` from `$OCCT_ROOT/share/doc/opencascade`.
    - A statement that OCCT is statically linked, with where its source is and how to relink it.
    - The licenses of the Cargo.lock crates, per platform.
    - The licenses of the npm runtime dependencies.
    - The two self-hosted fonts of ADR 0031, both SIL Open Font License 1.1 with no Reserved Font Name: Geist 1.800 ("Copyright (c) 2023 Vercel, in collaboration with basement.studio", `ui/public/fonts/Geist-OFL.txt`) and JetBrains Mono 2.304 ("Copyright 2020 The JetBrains Mono Project Authors", `ui/public/fonts/JetBrainsMono-OFL.txt`, shipped as a subset). Each license text already ships next to its font in `ui/dist/fonts` and so in the desktop bundle (ui.md 2.16.7); the notices list both.
  - Ship them:
    - In desktop `resources` and `licenseFile` (desktop.md package test 49 can check the .deb).
    - Next to the web core, checked by check-dist.
    - As a release draft asset next to SHA256SUMS.
- **Impact:** high. Distributing without these breaks the LGPL. **Size:** M.
- **Depends on:** the human license gate, and B-1 for the CI checks.
- **Status (2026-10-16):** on `main` since the push of 55dde8a, which brought reference stability phase 1 there too, as the landing order asked; ADR 0034 decided, ADR 0033's gate open. Spec: delivery.md 2.9, 2.6, 2.5, 2.13, 3.6, 4, 6 (tests 6, 8, 12, 13, 34 to 42 and 52 to 59), 7 and 8; desktop.md 2.8, tests 49 and 53; ADR 0033 Proposed, ADR 0034 Accepted. Code: `ui/scripts/notices.mjs` with `pnpm -C ui notices` and `pnpm -C ui check:notices` (tests 35 to 42); the committed texts under `ui/scripts/notices/` (Apache-2.0 and MPL-2.0 from SPDX License List 3.29.0, ten copies from crate repositories at the commits their packages name, OCCT 7.9.3's license, exception and four bundled notices, Rust 1.97.0's Unicode-3.0 text, Emscripten 4.0.15's six texts and the WebView2 SDK license); both notices files, the desktop one with 362 crates and the web one with 54, each with the 11 npm packages that ship, generated twice byte for byte the same; the notices rule of `check:dist` (test 34); the bundle resource in `tauri.conf.json` (tests 33 and 53 pass); `check:notices` in the `schema` job; the notices check in the `desktop` job's package check (test 12, run against built `.deb` files); and the notices as a `draft` release asset covered by `SHA256SUMS`. Found while implementing: the icon `dist/esm/icons/copyright.mjs` of `lucide-react` matched the nested-file rule, so input 3 now leaves out JavaScript, TypeScript and source map files; the objc2 repository's only license file names MIT without its text or a copyright line; `webview2-com-sys` 0.39.1 ships the loader of SDK 1.0.3800.47, not 1.0.3650.58 as ADR 0033 said (now corrected there, with how it was matched and why the package's `NOTICE.txt`, for its `wv2winrt` tool, is left out). Found in review: `rolldown`'s `THIRD-PARTY-LICENSE`, the MIT notices of rollup and of esbuild, whose runtime helpers start `assets/index-*.js`, matched no prefix of input 3, so input 3 now also takes names starting with `THIRD-PARTY`, `THIRD_PARTY` or `THIRDPARTY` (test 38), and both notices files gained that text on rolldown's entry and changed nowhere else; the schema job now installs the packages before the schema steps and runs `check:notices` only when the install passed (test 50); and a change to the toolkits or options of `scripts/build-occt.sh` or `partsmith-occt-sys/build.rs` now repeats the OCCT notice search (2.9, input 4 and "Kept current"), which no check enforces. `ui/AGENTS.md` lists `notices`, `check:notices` and `check:dist`. The owner made the repository public and chose ADR 0034 option A on 2026-10-16. Its follow-through is written, not on `main` yet: the source stamp `/SOURCE.txt` that the `web` job writes into every deployment and `deploy` requires, the OCCT source archive `occt-7.9.3-source.tar.gz` that `draft` attaches to every release, made by `scripts/occt-source-archive.sh` from the tag and the commit `scripts/occt-version.sh` pins, with the same commit check after `scripts/build-occt.sh` clones (delivery.md 2.13), the notices statements that name them (2.9; both files regenerated, each changing only that paragraph), README.md's OCCT sentences (2.12 part 7), and the web notices in the `core-wasm` artifact with 7-day `bundle-*` and `playwright-report` artifacts (2.4, 2.5). Tests 52 to 55 and 57 to 59 pass in `pnpm test:scripts` (124 tests), and a test failed for each line of the stamp step, its check, the archive script, the commit check of `scripts/build-occt.sh` and the artifact settings that was broken on purpose, apart from `gzip -n`, which GNU gzip 1.12 does not need for piped input. Run here on 2026-10-16: the archive script on the local clone of `V7_9_3` and on a fresh clone of the upstream tag wrote the 17,457,358 bytes and SHA-256 of 2.13, also under `core.autocrlf=true`, `tar.umask=0077` and a global `*.cmake export-ignore`; the stamp step on a shallow fetch of merge commit 5a6a193 named its two parents and its tree; `pnpm build`, the stamp as CI writes it, `pnpm check:dist` and the deploy job's check passed. Test 56 passed: OCCT built from the extracted archive alone, without a clone, for native and for WebAssembly (Emscripten 4.0.15), and `cargo test -p partsmith-occt-sys` passed against each install (472 tests native; 467 on `wasm32-unknown-emscripten`, 2 ignored). The pin changes the OCCT cache keys, so the first CI run with it builds OCCT cold once. Left for the owner: the root instruction file's notices command line of delivery.md 2.9 "Kept current"; the em dash choice of ADR 0033, which sets out three ways; the Apple SDK caveat of the objc2 crates and whether their four folders get the MIT text (delivery.md 8); of ADR 0034, the AppImage choice and the four questions for a lawyer, of which question 1 decides whether the web's OCCT source falls short now (delivery.md open issue 15); and two of the owner's instruction files, the root AGENTS.md, whose invariant still says OCCT is "linked as a separate library", and `.github/workflows/AGENTS.md`, whose release checklist items 1 and 2, artifact lines and rule about a private repository delivery.md 3.6, 2.4 and 2.5 now word differently. Still to specify: the in-app notice of delivery.md open issue 11, the macOS About panel included.

**B-3. Recovery keeps one slot, so unsaved work is lost** (`ui/src/files/recovery.ts`, `ui/src/app/actions/document.ts`, `ui/src/app/createApp.ts`)
- **Spec:** ui.md 2.15 (line 1852), 3.19 (lines 3032 and 3043-3044); desktop.md line 590 and open issue 7.
- **Spec first:** yes.
- **Work:**
  - Move to IndexedDB version 2, with entries keyed per document session and a migration of `current`.
  - A `replaced` stash survives later autosaves and saves of other documents.
  - A save clears only its own entry.
  - Startup offers every entry.
  - Tabs and desktop instances never overwrite or "Discard" each other's live copies.
  - Call `navigator.storage.persist()`.
  - Correct desktop.md's "at most 2 s lost" claim.
  - New U-tests: replaced, then autosave, then save; and two sessions at once.
- **Impact:** high (data loss). **Size:** M.
- **Depends on:** nothing. Do it together with B-4. B-5 later adds asset bytes to these entries.
- **Status (2026-10-16):** done in the UI wave of 2026-10-13. IndexedDB version 2 keys recovery entries by id and moves the `current` record over (`ui/src/files/database.ts`, U-155 and U-170). Web Locks keep pages from offering, overwriting or discarding each other's live copies, and the first write asks once for `navigator.storage.persist()` (`recovery.ts`, U-156). The `replaced` stash survives later autosaves, a save removes only its own entry (U-157), two pages run at once (U-158), and startup offers every entry (U-159). desktop.md no longer claims "at most 2 s lost"; its open issue 7 states what two instances share. The tests pass (Vitest, `src/files` and `src/app`, 113 tests).

**B-4. The recovery copy stores uncommitted preview edits** (`ui/src/app/createApp.ts`)
- **Spec:** ui.md 3.19 against 4.4 (line 3253).
- **Spec first:** yes. One sentence: the recovery copy is the settled recipe.
- **Work:** have `scheduleRecovery` reuse `followSettledRecipe` so it never writes `transactionOpen` edits. Add a U-test next to U-77.
- **Impact:** medium. Today, crash recovery can reopen a preview as committed features, including a preview that caused the crash. **Size:** S.
- **Depends on:** do it with B-3.

**B-5. Web and desktop hosts ignore document assets** (`ui/src/core/api.ts`, `ui/src/files`, the open, save and restart actions, `ui/src/test/fakes/core.ts`, `desktop/src-tauri/src/files.rs` and `commands.rs`)
- **Spec:** engine.md 3.14, protocol.md commands 6 and 7, file-format.md 3.11, technical design 6.
- **Spec first:** yes.
  - ui.md: `CoreApi` (lines 374-375), 3.19 (Open reads the sibling `assets/`; Save and Save as write the assets as file-format.md 3.11 says (assets first, a wrong entry replaced, then the document, then `markSaved`); on the web without a parent folder, either a directory pick or a notification naming the asset), 4.4 (restart keeps the assets), and new U- and E-tests.
  - desktop.md 1.1, 2.6, 3.2 to 3.4 and 3.7: a command and grant rule for the granted document's sibling `assets/` folder, with the matching `COMMAND_NAMES`, permissions and capability.
- **Human gate:** the capability review, including the two desktop options of ADR 0036.
- **Impact:** medium, but it breaks an M1 feature. Every document with an `import` feature, including those Python writes, opens with `asset_missing`, and Save as silently drops the assets. **Size:** L.
- **Depends on:**
  - B-3 (the recovery entry format).
  - PY-1 (spec the asset write order once, for every host).
  - EX-1 (a real STEP asset to use as an E-test fixture).
  - Batch the desktop command review with B-6.
- **Status (2026-10-23):** code done, not on `main` yet; review findings 1, 3 and 4 fixed; human gate open (ADR 0037 decisions 2 to 6, the capability change of `read_asset` and `write_asset` with B-6, and which stored grants reach the assets folder, desktop.md open issue 5). Review fixes: "Choose folder" takes the settled recipe inside its mutation-queue task, after every edit queued before it, refuses with `finish_command_first` and offers `assets_folder_needed` again when a feature command opened meanwhile, reopens nothing for a document that replaced it, and `runChange` follows the document session, so an edit made during the reopen goes to the reopened document (ui.md 2.9 edits rule, 3.19, U-222 amended); desktop.md open issue 5 and ADR 0037 state the reach of persistent grants with option (ii), grants used since the app started; `document-assets.ts` no longer imports `document.ts`. The desktop host has both commands (`desktop/src-tauri/src/files.rs` and `commands.rs`, `COMMAND_NAMES`, the two `permissions/autogenerated/` files and `capabilities/main.json`; tests 11, 34 and 54 to 63 pass), and the UI has `src/files/assets.ts`, the folder methods of both file services, the assets of `CoreApi` and `DocumentState`, Open, Save, Save as, Choose folder and the restart with assets (`src/app/actions/document.ts` and `document-assets.ts`); U-217 to U-222, U-226 to U-232, desktop UI tests 64 to 66 and E-150, E-151, E-153 and E-154 pass. Spec and tests were written before the code: ADR 0037 (Proposed, needs human review for decisions 2 to 6) records the capability change and its options. In it, the desktop gets `read_asset` and `write_asset` with ADR 0036 option (a), for `.ptsm` grants and `step` assets only, next to the granted path (a granted link's own folder, where the CLI and Python read), checks the bytes against the name, follows no link in the assets folder and refuses a read-only wrong asset; the web keeps a folder the user chose and asks for its permission before the file's. ui.md: `CoreApi` and the core client's buffers (2.5, 2.6), `DocumentState.assets` and `FileReference` (2.8), `FileService` with `readAssets`, `writeAssets`, `chooseFolder` and `reachesFolder`, plus `src/files/assets.ts` (2.15), 3.19 (Open reads the assets the text names, Save writes them in the order of file-format.md 3.11, the web's "Choose folder", `asset_mismatch` opens without assets), 4.3 (eight messages), 4.4 (restart keeps the assets), fixture `step-block.ptsm`, helpers `useFolders` and others over the origin private file system (6.3), tests U-217 to U-222 and U-226 to U-232, E-150, E-151, E-153 and E-154, and open issues 36 and 37. desktop.md: 1, 1.1, 2.1, 2.2, 2.4, 2.6, 2.7, 2.9, 2.11, 3.2 step 8, 3.4 steps 7 to 11, 3.5, 3.7 steps 9 to 12, 4, 5, 8, tests 11 and 54 to 66, and open issues 10 and 11. file-format.md 3.11 now cites both hosts.

**B-6. Save overwrites a file that changed on disk** (`ui/src/files/file-service.ts`, `desktop-file-service.ts`, `document.ts`, desktop `files.rs` and `commands.rs`)
- **Spec:** ui.md 3.19; desktop.md 2.x (`read_file` returns the modification time).
- **Spec first:** yes.
- **Work:**
  - Remember `lastModified` (or mtime or a content hash) at open and at save.
  - On a mismatch, warn with Overwrite, Save as, or Open the newer file.
  - New U- and E-tests.
- **Impact:** medium. A git pull, a Python `Document.save` or another tab is silently discarded today. **Size:** M.
- **Depends on:** share the desktop command change and its review with B-5.
- **Status (2026-10-23):** code done, not on `main` yet (`src/files/digest.ts`, the file reference's `digest`, and Save's change check with `save_conflict` and its three actions in `src/app/actions/document.ts`); U-216, U-223 to U-225, U-86 and E-152 pass; review finding 2 fixed: writes run one at a time (`writeInTurn` over `createOneAtATime` in `src/core/queue.ts`), each built when its turn comes, and the change check reads the digest from the file reference as it runs, so a second Save during a save shows no false `save_conflict` (ui.md 3.19, U-223 (f)). Spec and tests were written before the code: ADR 0037 decision 1 compares a SHA-256 of the bytes Partsmith last read or wrote, taken with Web Crypto, with the file read again right before Save writes anything. Modification time and a host-side compare and write are listed as options. No desktop command changes: the check reads through `read_file`, so B-6 waits for no human review (ADR 0037 decisions 1, 7 and 8 need none). ui.md 2.8 `FileReference.digest`, 2.15 `fileDigest` and `readHandle`'s mode, the recent and recovery entries' `folder`, the recovery entries' `digest`, 3.19 (the change check, `save_conflict` with "Overwrite {file}", "Save as" and "Open the newer file"), tests U-216, U-223 to U-225 and E-152, U-86 amended (the check's read comes between `saveDocument` and `write`), and open issue 38.

**B-7. No way out of a core stuck in a long call** (`ui/src/app/createApp.ts`, `ui/src/viewport/Viewport.tsx`, `ui/src/core/transport.ts`)
- **Spec:** technical design 14 ("never freezes"), ui.md 2.5, 3.21, 4.4.
- **Spec first:** yes.
  - ui.md 3.21 and 4.4: a "still working" state after N seconds, with "Stop and restart core". It terminates the worker, drops the queued mutations with a notice, and reopens the settled recipe without the open command.
  - desktop.md: an open issue, until request cancellation (later in protocol.md) lands.
- **Impact:** medium. **Size:** M.
- **Depends on:** B-4 (settled recipe) and B-3 (the recovery entry survives the restart).
- **Status (2026-10-16):** done. After 10 s without an answer, the status bar and the viewport say the core is still working (U-161, U-171, U-172). "Stop and restart core" terminates the worker, drops the waiting changes with a notice and reopens the settled recipe without the open command (U-162, U-165, U-166, U-169). The desktop shows the panel without the button, and desktop.md open issue 9 explains why until request cancellation lands.

**B-8. Windows and macOS builds are never tested** (`.github/workflows/release.yml` bundle job, or a ci.yml matrix job)
- **Spec:** delivery.md 1 (line 9: "every platform the project ships"), 1.1, 2.5, 6; open issue 1.
- **Spec first:** yes.
- **Work:**
  - Run `cargo test -p partsmith-occt-sys -p partsmith-engine -p partsmith-desktop` and the examples runner on windows-2025 and macos-15, before bundling or as a scheduled or main-only job.
  - Expect platform fixes: the atomic replace in `files.rs` on Windows, and grant paths with backslashes and drive letters.
- **Impact:** medium. It blocks shipping those bundles with confidence. **Size:** M.
- **Depends on:** B-1.
- **Status (2026-10-24):** spec and code written, the first run next: the orchestrator dispatches `gh workflow run platforms.yml --ref main` once this is on `main` (ADR 0038, proposed; its runner time, caches, release checklist item, promotion rule and the C runtime of the Windows app need human review). delivery.md 2.14 adds `.github/workflows/platforms.yml`, on `workflow_dispatch` and every Monday, outside the gate: `occt-platforms` and `rust-platforms` build OCCT with the `bundle` job's setup and key and run `cargo test --no-fail-fast -p partsmith-occt-sys -p partsmith-engine -p partsmith-desktop` and the examples runner on `windows-2025` and `macos-15`, then clippy for those crates, since CI's clippy never sees Windows-only code (tests 49, 59 to 64); `rust-platforms` runs once both OCCT rows have ended, so one runner's failure skips nothing on the other; a job moves into `ci.yml` after two passing runs on `main` in a row. Checking the code found the Windows atomic replace sound but for renames that another program blocks for a moment, which desktop.md 3.4 step 4.5 now retries (tests 67 and 68); grant paths go through `std::path` everywhere, so backslashes, drive letters and UNC paths need no change (test 70 checks the store on Windows); and the socket tests 56 and 59 bind a path too long for macOS, which they now shorten. Junctions get test 69. Reading tauri-build found that its Common Controls manifest reaches only the app binary, so no desktop test binary could start on Windows: `build.rs` now embeds a committed copy into every MSVC executable (desktop.md 2.2, test 71). What reading cannot settle for OCCT and the shim is delivery.md open issue 1, for the first runs, among them two C runtimes in one Windows binary, whose fix is the owner's choice (ADR 0038, needs human review), and the Windows main thread's 1 MiB stack in the CLI. Code: `.github/workflows/platforms.yml` with the five jobs of 2.14, its rules, commands and checklist item 3 in `.github/workflows/AGENTS.md`; `retry_while_busy`, `is_busy` and `RENAME_WAITS` around the rename of `desktop/src-tauri/src/files.rs`; and `build.rs` with the committed `windows-app-manifest.xml`, a byte-for-byte copy of tauri-build 2.7.2's. After review: `rust-platforms` saves its Rust cache on `main` also when its tests fail (`cache-on-failure: true`, delivery.md 2.14, 3.2, test 61), so a failing first run does not throw away its Windows and macOS dependency builds; test 71 also checks that `Cargo.lock` locks tauri-build 2.7.2, the release `windows-app-manifest.xml` copies, so a tauri-build update fails until the copy is compared again; the grant store's write keeps the rename waits under the grants lock, now stated in desktop.md 3.2 step 7, section 5 and open issue 12; and README.md carries 2.12's browser sentence, Safari 17 on macOS 13 and the platforms workflow. Verified here (Linux): tests 49 and 59 to 63 in `pnpm test:scripts` (144 pass), the workflow files parse as YAML and `actionlint` 1.7.7 finds nothing in them, desktop.md tests 67 and 71 pass with the crate's other tests (119 in all), and clippy is clean. Left for the first runs (test 64): tests 68 to 70, which compile only for Windows, the manifest's embedding (shown by the Windows test binaries starting at all), every Windows and macOS build and test, and the job times and cache sizes of open issue 16.
- **Status (2026-10-27):** the first run (40315773128) failed on macOS examples, a Windows hang, headless Firefox and two test-server defects in WebKit; fixes in delivery.md open issue 1 and ui.md open issue 41; the next run decides.
- **Status (2026-10-27, second run 40478176115):** the crate tests and clippy pass on `windows-2025` and `macos-15`, the Windows hang is gone, and the examples runner failed only on the thin walls of the four cone examples, whose rim triangles the platform's math decides; fixed by print.md 3.9.6 (ADR 0041, its 30 degree angle needs human review), with `shell/thin-wall` added so the platforms compare a real thin wall; the next run decides.

**B-9. The UI is tested only in Chromium** (`ui/playwright.config.ts`, `ui/e2e`, delivery)
- **Spec:** ui.md 1.1 and 4.3 (lines 3154-3155); desktop.md line 9.
- **Spec first:** yes.
  - ui.md 1.1 and 6: the supported browsers and webviews, with minimum versions, and which E-tests run on each engine.
  - delivery.md 2.4 and 6: install and run firefox and webkit, at least a smoke subset with `fileSystemAccess` false.
- **Impact:** medium. Two of the three desktop webviews are WebKit. **Size:** M, or L if WebKit needs fixes (Popover API, `@starting-style`, wasm exceptions).
- **Depends on:** B-1.
- **Status (2026-10-24):** spec and code written, the first run next, with B-8's (ADR 0038, proposed; the two browser builds and the minimum versions need human review). ui.md 1.1 lists the supported browsers and webviews with minimums from the features the UI needs (Chrome and Edge 114, Firefox 128, Safari 17.0; WebView2 114, the WebKit of Safari 17.0 on macOS 13 or later, WebKitGTK 2.44), and 4.3 keeps its texts, which already name those browsers (open issue 40: no startup check for the minimums). ui.md 6.8: `ui/playwright.config.ts` always defines the `firefox` and `webkit` projects with Playwright's `default: false`, so `pnpm e2e` still runs Chromium alone, each running the 24 tests tagged `@engines`, which run as written in every engine, with `fileSystemAccess` false or no file picker; E-156 (the projects) and E-157 (the subset each project lists, which launches no browser) are new. The `e2e-engines` job of the platforms workflow (delivery.md 2.14) runs them weekly in Playwright's Firefox 157.0 and WebKit 27.2; WebKit on macOS is ui.md open issue 41. Code: the three projects of ui.md 6.8 in `ui/playwright.config.ts`, the Chromium executable path moved into the `chromium` project, and `e2e-engines` in `platforms.yml`. Verified here, where only Chromium is installed: E-156 and E-157 pass, the 24 tests of the subset pass in Chromium (`pnpm e2e --project chromium --grep @engines`), the `firefox` and `webkit` listings each hold those 24, and `pnpm e2e` still runs Chromium alone with every test. README.md now names the supported browsers and their minimums (delivery.md 2.12), the only place a user learns them until ui.md open issue 40's startup check exists. The runs in Firefox and WebKit are the first platforms run's.
- **Status (2026-10-27):** the first run (40315773128) failed on macOS examples, a Windows hang, headless Firefox and two test-server defects in WebKit; fixes in delivery.md open issue 1 and ui.md open issue 41; the next run decides.
- **Status (2026-10-27, second run 40478176115):** all 24 `@engines` tests pass in Firefox (headed under `xvfb-run`) and WebKit; one more passing run on `main` makes both rows eligible for promotion (delivery.md 2.14).

**B-10. Wheel packaging (python-api.md test 76) never runs in CI** (ci.yml `python` job)
- **Spec:** python-api.md 1, 2.1, 6 test 76.
- **Spec first:** yes, small. delivery.md 2.4 `python` row and section 6 test 10, recorded in section 8.
- **Work:**
  - Install CPython 3.12 and 3.13.
  - Add `pytest python/tests -m packaging` after the existing pytest step, or as a separate job listed in `deploy.needs`.
  - Publishing wheels stays out of M1.
- **Impact:** medium. **Size:** S.
- **Depends on:** B-1.
- **Status (2026-10-16):** workflow written, not on `main` yet, human gate open (delivery.md 8). Test 76 is `python/tests/test_core.py`, marked `packaging`, which `python/pyproject.toml` leaves out by default, and parametrized over `python3.12` and `python3.13`. CI's `python` job now installs CPython 3.13 and 3.12 (3.12 last, so it is the default `python`), makes its virtual environment with `python3.12` and runs `pytest python/tests -m packaging` after `pytest python/tests`, as delivery.md 2.2, the 2.4 `python` row and test 10 say; `.github/workflows/AGENTS.md` lists it. Run locally the way the step runs it: 3 passed in 2 min 32 s with the release engine crates already built, giving `partsmith-0.1.0-cp312-abi3-manylinux_2_38_x86_64.whl` (21.7 MB), which installed and imported on 3.12 and 3.13. python-api.md 2.1 now matches `python/`: its layout lists `_files.py`, and its `pyproject.toml` holds the `[tool.ruff]` and `[tool.pytest.ini_options]` tables, the latter leaving the `packaging` marker out by default as test 76 says.

**B-11. Release-gate perf tests P-1 to P-8** (new `ui/e2e/perf.spec.ts`, `ui/e2e/helpers.ts`)
- **Spec:** ui.md 5, 6.7; technical design 14 (the web column is a release gate).
- **Spec first:** no.
- **Work:**
  - Write P-1 to P-8 with `@perf` titles.
  - P-2 uses a `longtask` PerformanceObserver installed with `addInitScript`.
  - Add shared percentile and long-task helpers in helpers.ts.
  - The fixed-hardware CI gate stays deferred (delivery.md 1.2). Run these by hand on the reference machine before a release.
- **Impact:** medium. **Size:** M.
- **Depends on:** nothing. The fixtures exist, apart from P-9's, which is in R-2.
- **Status (2026-10-16):** done. `ui/e2e/perf.spec.ts` holds P-1 to P-8, and also P-10 to P-12, all tagged `@perf` and run only with `PARTSMITH_PERF=1` (`playwright.config.ts` leaves them out otherwise); `ui/e2e/helpers.ts` has `percentile`, `watchLongTasks` (the `addInitScript` observer P-2 uses) and `longTasks`. Whether they meet their budgets is measured by hand on the reference machine before a release (ui.md 6.7). P-9 and its fixture stay with R-2.

**B-12. No check enforces the 250 KB gzipped initial JavaScript budget** (`ui/scripts/check-dist.mjs` and its tests)
- **Spec:** ui.md 5.
- **Spec first:** yes.
  - delivery.md 2.6 must define:
    - the limit constant, and whether KB is 1,000 or 1,024 bytes;
    - what "initial" means: the entry script, its static imports and modulepreload links, and whether the core worker chunk counts (source maps and `dist/core/` excluded);
    - the gzip level.
  - Add the message to delivery.md 4, a test to section 6, and a cross-reference in ui.md 6.7.
- **Work:** implement it with `node:zlib`. No new dependency is needed.
- **Impact:** medium. The bundle is already at 230,462 bytes gzipped (233,328 with the worker), 92 to 93 percent of the budget, and the app is not split into chunks. **Size:** S.
- **Depends on:** nothing. The CI web job already runs `check:dist`.
- **Status (2026-10-16):** done. delivery.md 2.6 defines the limit (250,000 bytes, 1 KB being 1,000 bytes), what counts as initial (the scripts and `modulepreload` links of `index.html`, plus the core worker script), and gzip level 9; section 4 has its messages and tests 26 to 33 its cases; ui.md 6.7 points at it. `ui/scripts/check-dist.mjs` implements it with `node:zlib`, and `pnpm test:scripts` passes (37 tests). The headroom is delivery.md open issue 8: 94.1 percent of the limit on 2026-10-13, and 96.8 percent on 2026-10-16 (242,039 bytes gzipped, 7,961 under the limit, `check:dist` on a build of the tree then), against the 92 percent of the index row, which is the gap analysis's figure.

**B-13. README, install notes and release checklist** (README.md, `.github/workflows/AGENTS.md`)
- **Spec:** delivery.md 3.6 (the release notes say nothing is signed); desktop.md 3.8 and line 615.
- **Spec first:** no. An optional `python/README.md` would need a python-api.md 2.1 amendment.
- **Work:**
  - Replace "Status: early development (phase 0)".
  - Build prerequisites: ninja, plus the WebKitGTK, xdo, ssl and rsvg dev packages.
  - Building the web core: emsdk, `build-occt.sh wasm`, `build-wasm.sh`. Without it, `pnpm -C ui dev` has no core.
  - How to run the web app, the desktop app, the CLI (its five subcommands) and Python (`maturin develop`, `python -m partsmith build`).
  - Download and install: the Pages URL, GitHub Releases, unsigned-install steps, glibc 2.39 and WebKitGTK 4.1.
  - A License section linking the notices (B-2) and the CC0 scope (DEL-1).
  - Contributor pointers to AGENTS.md, the spec README, the ADR README and the bless command.
  - A release checklist with the exact unsigned-bundle paragraph. `--generate-notes` stays, per delivery.md 2.5.
- **Impact:** medium. **Size:** S.
- **Depends on:** B-1 (URLs) and B-2.
- **Status (2026-10-16):** written, not on `main` yet. `README.md` follows delivery.md 2.12 part by part, and `.github/workflows/AGENTS.md` holds the release checklist of delivery.md 3.6 (items 1 and 2 still in their wording from before ADR 0034's decision, which the owner replaces), with the unsigned-bundle paragraph (the same in README.md and the spec) and the commands of checklist items 5 to 7; both files become AGENTS.md when this branch merges `main` (5e32ada), where README's link to AGENTS.md resolves. Every command was run here except four that need what this machine lacks or would overwrite another pipeline's files: `cargo test --workspace` (about 170 test binaries that link OCCT, more than the free disk), the two OCCT builds and `scripts/build-wasm.sh` (their usage and precondition messages were checked, and CI's `occt-native`, `occt-wasm` and `core-wasm` jobs run them), `cargo tauri dev` and `cargo tauri build` (`tauri-cli` is not installed; 2.12.2 is on crates.io and its source has both commands), and the bless and generate commands, which write `examples/`, `docs/format` and `ui/src/generated` (their targets exist). Checking the commands changed the spec in three places: pnpm comes from `corepack enable && corepack install -g pnpm@10.28.0`, since corepack alone takes `ui/package.json`'s pnpm only inside `ui/` and from the root picks pnpm 12.11.2, which the corepack of Node 22.22.0 fails to start; the install line gains `python3-venv`, without which Ubuntu's `python3 -m venv` fails; and the web part names `pnpm -C ui build`, which the web notices point at. Review fixes: README.md now says desktop builds appear on GitHub Releases once the first release is published (none is yet, and the repository was private then), its install line runs `sudo apt-get update` first, as CI does, and before the OCCT builds it makes `/opt/occt` the user's (`sudo mkdir -p /opt/occt && sudo chown "$USER" /opt/occt`), since the build ends by installing into its prefix; delivery.md 2.12 parts 3 and 4 say the same. Left for the owner: the web app's address. Until the owner confirms the `*.pages.dev` subdomain, which waits for the first production deployment, README.md says the address follows that deployment (delivery.md 2.12 part 3). If ADR 0034 drops the AppImage, README.md's download list and paragraph follow desktop.md 3.8 and delivery.md 3.6.

## 2. After the blockers, by owner

Add examples after phases 1 to 3, so their expected results are blessed once, against the final message texts.

### Print (ui/src/print, partsmith-print, python reports)

**PRINT-1. Show split-plane and drain-hole suggestions**
- **Spec:** technical design 11; print.md 2.6, 3.9.2, 3.9.8.
- **Spec first:** yes. ui.md 3.17, python-api.md 2.9, and optionally cli.md 3.3.
- **Work:**
  - Add `suggestion` to `HealthRow` (health.ts lines 11-17), with row text such as "Split at x = 150 mm" or "Drain hole near (5, 5, 5)".
  - In `focusedFindingPoint`, mark `drain_hole` at `suggestion.at`.
  - Draw split planes in the viewport (`scene.ts`, `overlay.ts`), clipped to the body's box and framed when the row is focused.
  - Add `Finding.suggestion` in `python/partsmith/reports.py`.
  - Optional: a CLI text line (rebuild.rs lines 337-344), and a one-click route to `split_body`.
- **Impact:** medium. **Size:** M, or L with the one-click split.
- **Depends on:** nothing.
- **Status (2026-10-27):** implemented: `Suggestion::text` in `partsmith-print` and the CLI's `suggested:` line, `Finding.suggestion` in `partsmith.reports`, and in the UI `suggestion.ts`, the row's `health-suggestion` line, the drain hole marker, box framing and the split planes in the overlay pass. Checked against the repository first: `partsmith-print` already computes both suggestions and the protocol and `ui/src/generated` already carry them, so no schema changes; no host showed them (`HealthRow` has no suggestion, Python's `Finding` drops it, `rebuild --checks` prints only the message). ADR 0040 decides that `partsmith-print` words a suggestion in millimeters (`Suggestion::text`, the words of the new print.md 3.12), which the CLI prints, that Python and the UI word it themselves in the same words, kept in step by shared test cases (no protocol label, so no schema review and no expected result changes), and leaves the one-click route to `split_body` for an engine fix after M1 (ui.md 1.2). The track's paths therefore include `crates/partsmith-print` (`Suggestion::text` and its tests). Spec: print.md 2.6, 3.12, open issue 7 and tests 51, 52, 55, 56, 94, 113 and 114; cli.md 3.3 and test 63; python-api.md 2.1, 2.9, 3.7 and tests 104 to 106; ui.md 2.1, 2.16.4, 3.12 pass 7, 3.17 (rows, focus, split planes, the marker's lifetime), 1.2, 6.1, 6.2 (`hollow-resin.ptsm`), E-108, U-233 to U-237, E-158 and E-159.
- **Status (2026-10-27, review):** implemented and checked, waiting for human review of its Python API only. Review findings 1 to 4 fixed: the focused finding's marker and split planes also go when a new report replaces the one the finding was taken from, since a stale report's rows stay listed and the same key can name another finding in the next report (`focusOutdated`, ui.md 3.17, U-236); `findingAt` is imported from `src/print/checks.ts` only; the Python tests share `StubPart` and `only_finding` through `support.py`. ADR 0040 is accepted except its Python API (`Finding.suggestion`, `Suggestion` and `SplitPlane` in `partsmith.reports`), which is a public API change and so proposed, needs human review, with options (a) to (d) and (a), as implemented, recommended.

**PRINT-2. Custom printer profile route, or mark it Later**
- **Spec:** print.md 1.1, 2.2, 4; protocol.md command 4.
- **Spec first:** yes, as a decision. Either:
  - a CLI `--printer-file` on `rebuild` and `export` (cli.md 2, 3.3, 4, 6), plus a Python profile argument or `Document.add_printer` mapping `invalid_profile` and `profile_id_reserved` (python-api.md 1 to 3); or
  - an explicit "Later" in cli.md 1 and python-api.md 1.
- **Human gate:** public API, with an ADR entry.
- **Impact:** low. **Size:** S to mark Later, M to implement.
- **Depends on:** nothing.

**PRINT-3. Protocol schema describes listed printer profiles in their input form**
- **Spec:** protocol.md command 3 and 2.9; print.md 2.2, 3.1.
- **Spec first:** yes. Say which form each direction uses, and log it in ADR 0016.
- **Human gate:** protocol schema.
- **Work:**
  - Give results the written form (required fields, `FitClasses`). Requests keep `ProfileInput`.
  - Update `profile_schema_reads_the_input_form` and add a protocol schema test.
  - Regenerate `protocol.schema.json` and `ui/src/generated`.
  - Remove the workarounds in `printer.ts:106-114` and `bed.ts:90-92`.
- **Impact:** low. **Size:** M.
- **Depends on:** the phase 1 schema regeneration (PR1 to PR5), so both reviews do not touch the schema at once.

### Examples corpus (examples/, the per-feature tests in partsmith-recompute, the runner in partsmith-cli)

**EX-1. STEP import examples that build, plus intersect and ImportWarning tests**
- **Spec:** technical design 16, features/import.md 1 and 6, delivery.md 2.8 and open issue 5, kernel.md 3.8.1 item 3.
- **Spec first:** yes for the new tests:
  - import.md 6: (a) op intersect; (b) a STEP file with a free face next to a solid gives warning `import_warning` "Skipped 1 shapes that are not closed solids.";
  - a kernel test in kernel.md 6 and `occt-sys/tests/exchange.rs`.
  - The examples themselves need no spec change.
- **Work:**
  - Add `examples/import/assets/<blake3>.step` (the 10x20x30 "Block") and new stems: step-block, join-, cut- and intersect-step-block. They have no Python twins.
  - Bless their expected results (volumes from import.md tests 1, 2 and 7).
  - Close delivery.md open issue 5, and fix the stale `asset_missing` text in import.md test 3 and section 4.
- **Impact:** medium. **Size:** M.
- **Depends on:** nothing. It also supplies the fixture for B-5.
- **Status (2026-10-23):** done, not on `main` yet. No production code changed: the kernel and the feature already meet the new tests, and the four examples are blessed with the volumes of their tests. features/import.md tests 9 (intersect, `import/intersect-step-block`), 10 (`import_warning`, its text also run through the message lint) and 11 (tests 1, 2 and 7 on `import/step-block`, `import/join-step-block` and `import/cut-step-block`, and the asset's name checked against its BLAKE3 hash), kernel.md test 135, and delivery.md open issue 5 resolved. Tests 10 and 135 skip two free faces, not one: the kernel's one-shape text `Skipped 1 shapes that are not closed solids.` is not grammatical, so no test pins it; its fix is features/import.md open issue 3, now item IMP-1 (a human gate for its recommended option). The `asset_missing` text of import.md test 3 and section 4 was already current (commit 3f1bc05). Review fixes: the face checks of tests 1, 7 and 11 also check each face's normal (`assert_face_in_plane`, as test 9 does), `box_step_asset` moved to the import fixtures it builds on, and a committed asset that no example imports, which no runner test hashes, is left to EX-9.

**EX-2. Bed fit, islands, tall-and-thin and resin examples**
- **Spec:** technical design 16; print.md 1.1, 3.8, 3.9.
- **Spec first:** no. delivery.md 2.8 already allows these under feature folders. A new `examples/checks/` folder would need a 2.8 amendment.
- **Work:**
  - FDM: `examples/box/too-tall`, `wide-footprint`, `tall-and-thin`, and `examples/boolean/island` (the print.md test 66 shape).
  - Resin, with `"printer": "generic-resin"`:
    - `examples/box/fits-rotated`: a footprint that fits the 130 x 80 bed only turned (print.md test 121: a square bed fits no turned footprint);
    - `examples/shell/closed-hollow-resin`: `enclosed_hollow` as a warning, with `drain_hole`;
    - `examples/box/resin`: the 30 degree overhang limit, nozzle-free thresholds, the 130 x 80 bed, and `printer.*` without a nozzle.
  - Python twins carry `printer="generic-resin"`.
- **Impact:** medium. **Size:** M.
- **Depends on:** nothing.
- **Status (2026-10-27):** done, not on `main` yet. No production code changed. The seven examples above, each with its Python twin and blessed expected results, are print.md 6.18 tests 117 to 123, which name every finding, value, limit, location and suggestion of their checks; no new `examples/` folder, so delivery.md 2.8 is unchanged. Review fixes: the tests now have test functions (`crates/partsmith-cli/tests/print_examples.rs`, rebuilding each example with `partsmith rebuild --checks --json`), so a bless that rewords a message or moves a value fails there instead of passing the examples runner unnoticed; `fits-rotated` is listed with the resin examples. The libm perturbation check of ADR 0041 moved none of their results in its ten modes (kernel.md open issue 14).

**EX-3. Sketch constraint and tool examples, plus an arc-bounded profile**
- **Spec:** technical design 16; sketch.md 1.1; ui.md 3.16.4 and 3.16.5.
- **Spec first:** only the arc-bounded profile, as a new extrude.md 6 test that `feature_extrude.rs` loads. It pins `f-ext1/lateral/<arc id>`.
- **Work:**
  - One small example per constraint missing from `examples/sketch`. Each extrudes the sketch, so the volume and bounding box pin the solved position.
    - No example at all: point_on_object, parallel, perpendicular, tangent, collinear, concentric, midpoint, diameter, angle.
    - Only in the benchmark: coincident, vertical, equal, symmetric, radius, horizontal_distance, vertical_distance.
  - The point-point and point-line distance forms.
  - Constraints that use `origin`, `x_axis` and `y_axis`.
  - The output of each M1 tool: constrained 2-point rectangle, center rectangle, circle with a point snap, 3-point arc, tangent inference, angle to X_AXIS, diameter.
  - Optional outcome examples: `sketch_conflict`, `sketch_invalid`, `sketch_not_solved`, and an open outline that the extrude reports as `profile_not_found`.
  - Slot and tangent-arc tools stay out, since they come after M1.
- **Impact:** medium. **Size:** L (about 20 examples).
- **Depends on:** phases 1 and 3 (region outline, sketch messages).

**EX-4. Shell concave-edge join test**
- **Spec:** features/shell.md 1, 3.4; naming.md 3.6.7 rules 4 and 5.
- **Spec first:** yes. A new shell.md 6 test:
  - Body: the L of fillet.md test 4, with `f-box1/bottom` removed at 2 mm.
  - Expect: status ok, the volume and face count, exactly one `f-sh1/inner/edge(f-box1/top|f-box2/left)` of kind Cylinder with radius 2, and no `f-sh1/face` fallback names.
  - Either a rule 5 (vertex) case, or a spec note that no M1 geometry produces one.
- **Work:** `examples/shell/concave-edge-join` with its twin, loaded by `feature_shell.rs` through `Run::example`. Optionally, a unit test in `shell.rs`.
- **Impact:** medium. **Size:** S.
- **Depends on:** phase 3 (shell round faces).
- **Status (2026-10-27):** done, not on `main` yet. No production code changed: the kernel's arc and sphere joins already meet the new tests. features/shell.md test 24 (`examples/shell/concave-edge-join`: the arc join `f-sh1/inner/edge(f-box1/top|f-box2/left)`, a cylinder of radius 2, as the only edge join, no vertex join and no `f-sh1/face` name) and test 25, the rule 5 case (`examples/shell/concave-vertex-join`: three arc joins and the sphere join `f-sh1/inner/vertex(f-box2/front|f-box2/left|f-box2/top)` at a notch's corner), each with its twin and loaded by `feature_shell.rs` (`t024`, `t025`); shell.md section 1 and its naming table say which join each concave edge and vertex gets. Review fixes: test 24 no longer prescribes a process step; kernel.md open issue 14 records instead that the libm check moved nothing on these examples and that the example's first overhang depends on OCCT dividing the join's quarter-turn edges into exactly 36 steps (π/2 over 2.5° is exactly 36.0, one ulp more gives 37 and other overhang numbers), which only a platforms run can confirm.

**EX-5. Feature option examples**
- **Spec:** technical design 16; the In M1 lists of the per-feature specs.
- **Spec first:** yes. An acceptance test for each, in `features/<type>.md` section 6.
- **Work:** each case gets an example plus a `feature_<type>.rs` test that loads it through `Run::example`.
  - Primitives:
    - a successful intersect for box, cylinder, sphere, cone and torus;
    - join and cut for cone and torus;
    - placement on a datum plane and on a flat face for cylinder, sphere, cone and torus.
  - revolve: op join; an axis from a round face.
  - linear_pattern: a direction from a round face; `second_direction.reverse`.
  - circular_pattern: an axis from a sketch line and from a straight edge; a cancellation test (a new circular_pattern.md test, the counterpart of linear_pattern t013).
  - move_body: rotation about a sketch line and about a round face.
  - split_body: a split by a flat face, without `merged`.
  - hole: arc start, end and center points; successful blind counterbore and countersink.
  - sketch: a sketch on YZ.
  - Before blessing, run the libm perturbation check of kernel.md open issue 14 (its ten modes) over the new examples, as EX-2 and EX-4 did: their print checks hold mesh-dependent findings at cone apexes, sphere poles and the overhangs of curved faces. Reshape an example whose `/checks/` move in any mode; do not bless it. Record the outcome in the EX-5 status and in one sentence in kernel.md open issue 14.
  - The Windows and macOS comparison of the new expected files runs only in CI, in the next platforms run (delivery.md 2.14); the EX-5 status says so. A mismatch there goes to delivery.md open issue 1 (d), never to expected files for one platform.
- **Impact:** medium. **Size:** L.
- **Depends on:** phase 2 (axis `direction`, face placement) and phase 3 (hole points).

**EX-6. Export coverage in the corpus**
- **Spec:** technical design 16 ("each import and export path"; the STEP round trip within 1e-6).
- **Spec first:** yes. delivery.md 2.8 and 6 must first say what an export example is. Either:
  - a runner test that runs `partsmith export --json` for stl, 3mf, obj and step and compares the report; or
  - a STEP export, import and volume check for every example that builds.
- **Work:** then implement it in `examples.rs`.
- **Impact:** medium. **Size:** M.
- **Depends on:** EX-1 and B-1.

**EX-7. Chamfer corner faces are never produced in any test**
- **Spec:** features/chamfer.md 1, 3.4; kernel.md 3.4.7.
- **Spec first:** yes.
  - A kernel test in kernel.md 6 and `blends.rs`: chamfer every BOX edge; each of the 8 vertices generates exactly one face.
  - A chamfer.md 6 test: CUBE20, all 12 edges, equal 2 mm. Expect status ok, 26 faces, 12 `f-cha1/chamfer/edge(...)` and 8 `f-cha1/corner/vertex(...)`, with an OCCT note like fillet.md open issue 2.
- **Work:**
  - `examples/chamfer/every-edge` with its twin, and a `feature_chamfer.rs` test.
  - Optionally, a `dressup.rs` unit test.
  - If OCCT does not report the corners as generated from vertices, fix `name_blend`.
- **Impact:** low. It may expose a silent naming defect. **Size:** S.
- **Depends on:** phase 2 (the chamfer side rule).

**EX-8. Inline geometry tests have no matching example**
- **Spec:** features.md 6.1 (line 1778).
- **Spec first:** no. An optional 6.1 clarification would say whether error and warning tests that assert volumes count.
- **Work:**
  - Turn these into examples:
    - extrude t041 to t044;
    - revolve t025;
    - hole t024;
    - shell t014, plus the 0.5 mm `thin` run;
    - chamfer t012 and fillet t018: one example each, with the reversed edge order derived in code.
  - Switch the tests to `Run::example`.
- **Impact:** low. **Size:** M.
- **Depends on:** the phases (to avoid re-blessing).

**EX-9. Corpus coverage checks**
- **Spec:** technical design 16.
- **Spec first:** yes. New delivery.md 2.8 runner tests.
- **Work:** new checks in `examples.rs`:
  - every sketch constraint and entity type appears in an example outside `examples/bench`;
  - every manifest option value and reference kind appears in an example of that feature type;
  - every `CheckKind` has at least one non-ok finding per technology, which needs resin examples with a non-ok `mesh_validity`, `first_layer_contact`, `islands` and `small_features` finding that EX-2 does not add;
  - every file in an `assets/` folder is imported by an example of that folder (delivery.md 2.8). Runner test 4 then checks each asset's name against its bytes, since `partsmith rebuild` fails with `asset_mismatch` on an asset that does not hash to its name (engine.md 3.14); today the bytes of an asset that no example imports are never checked (found by the review of EX-1).
- **Impact:** low. **Size:** M.
- **Depends on:** EX-2, EX-3 and EX-5. Without them, the checks fail on arrival.

**EX-10. The GUI to Python round trip never runs on a document the UI wrote**
- **Spec:** technical design 10 (line 350), 16; architecture.md "Testing".
- **Spec first:** yes. delivery.md 2.8 and 6, and ui.md 6.5.
- **Work:**
  - Check in GUI-built fixtures, or add an E-test that builds and downloads them. They must cover random ids, starting values, reverse and two-sided extents, new-body targets, references and axis directions copied from picks, and rename transactions.
  - In python/tests: `export_python`, run the script, then compare canonical text, volume and face count.
- **Impact:** low. **Size:** M.
- **Depends on:** PY-2 and phase 2.

### Kernel exchange and the import feature (partsmith-occt-sys, partsmith-features)

**IMP-1. Import warnings show the kernel's sentences** (added on 2026-10-23 by the review of EX-1)
- **Spec:** features/import.md open issue 3; kernel.md 2.12 and 3.8.1 read item 3; features.md 4.2 (`ImportWarning`) and test 14; docs/guidelines/errors.md.
- **Problem:** `ImportWarning` shows each string of `StepImport::warnings` as it is. A STEP file with one free face beside a solid reads `Skipped 1 shapes that are not closed solids.`, and a solid that repair could not fix is named by its position in the file (`Solid 2 is not valid after repair; booleans with it may fail.`). Both are kernel text, which features.md test 14 keeps out of rendered messages, and the first is not grammatical. features/import.md test 10 and kernel.md test 135 skip two faces, so no test pins either text.
- **Spec first:** yes, one of these options:
  - (a) Reword only the kernel's two strings in kernel.md 3.8.1 read item 3, for example `Skipped 1 shape that is not a closed solid.` and a repair warning without the solid's number. No public API changes, but users still read kernel text, so features.md 4.2 still conflicts with test 14.
  - (b) `StepImport` returns counts (shapes skipped, solids still invalid after repair) and the feature words them, for example `This import left out 1 shape of the STEP file that is not a closed solid.`, with the kernel's text in the detail. It changes kernel.md 2.12 and 3.8.1 and features.md 4.2 together.
  - Either way, new tests for one skipped shape and for the repair warning in kernel.md 6 and features/import.md 6.
  - Recommended: (b), the only option that keeps kernel text out of messages.
- **Human gate:** public API, for (b) only (the fields of `StepImport` and `ImportWarning`), with an ADR entry.
- **Work:** `cpp/exchange.cpp` and `src/exchange.rs` in partsmith-occt-sys. For (b), also the `ImportWarning` rendering in partsmith-features and the texts pinned by features/import.md test 10 and kernel.md test 135.
- **Impact:** low. **Size:** S for (a), M for (b).
- **Depends on:** nothing.

### Engine and WebAssembly (partsmith-engine, partsmith-wasm)

ENG-1, ENG-4 and SK-1 can share one wasm-target CI job.

**ENG-1. Native and WebAssembly results are never compared**
- **Spec:** engine.md 3.18 (line 514); recompute.md 3.9 (line 399).
- **Spec first:** yes. A new engine.md 6 test, a recompute.md test, and a delivery.md 2.4 row and section 6 test.
- **Work:**
  - Add a Node script in `crates/partsmith-wasm/tests/node` with a Rust driver like `web_module.rs`, gated by `PARTSMITH_REQUIRE_WEB_MODULE`.
  - Send open_document, recompute and measure through `ps_call` for the engine fixtures and `examples/**`, and compare with native `Session::call`: statuses, messages and ids exactly, floats within 1e-9 relative.
  - Let the test select `SequentialIds`, since the web session uses random ids (`lib.rs:21`).
- **Impact:** medium. **Size:** M.
- **Depends on:** B-1.

**ENG-2. The engine's web-column budgets have no benchmark**
- **Spec:** engine.md 5 (line 806).
- **Spec first:** small. Say where the harness lives and how it runs.
- **Work:**
  - A Playwright page or worker driver that sends each web-column row to `ui/public/core/partsmith_core.mjs` and compares medians to the budgets, mirroring `Report` and `median` in `partsmith-engine/benches/budgets.rs`.
  - Include wasm memory under 4 GB with a 512 MiB `cache_budget`.
- **Impact:** medium. **Size:** M.
- **Depends on:** phase 4 (the `apply_fix`, `list_uses` and ghost rows), and B-11's helpers.

**ENG-3. `cache_budget` does not bound the total cache size**
- **Spec:** engine.md 3.17, 5.
- **Spec first:** yes. Either:
  - (a) state the M1 approximation in engine.md 3.17, section 2, `options.rs` and an open issue; or
  - (b) split the budget between `Recompute::with_cache_limit` and `DisplayCache::new` in `Session::new`, with a unit test.
- **Impact:** low. Today the bound is about twice the budget, which on wasm is 512 MiB of display data plus 2048 results. **Size:** S.
- **Depends on:** nothing. ENG-2's memory row checks the result.

**ENG-4. Panic containment is never tested on the wasm build**
- **Spec:** engine.md 3.15 (line 492); protocol.md line 1394.
- **Spec first:** yes. A protocol.md 6 numbered test and delivery.md 2.4.
- **Work:** run the dispatch panic test, or a test-only panic trigger built with the core's link arguments, on `wasm32-unknown-emscripten` in CI.
- **Impact:** low. **Size:** S.
- **Depends on:** B-1, and the shared job with ENG-1.

**ENG-5. `display_failed` has no spec, and `duplicate_element_name` names the body by id**
- **Spec:** engine.md 3.4 and 3.8; docs/guidelines/errors.md.
- **Spec first:** yes.
- **Work:**
  - Specify `display_failed` with a compliant sentence that names the body. The kernel text moves to `Diagnostic.detail`. Add an exact-text test.
  - Delete the paragraph and the dead check for `duplicate_element_name`, or keep it only as a `debug_assert`. Naming already makes names unique (naming.md line 571).
- **Impact:** low. **Size:** S.
- **Depends on:** phase 1 PR1 (`Diagnostic.detail`) and the phase 3 message lint.

**ENG-6. No byte-for-byte golden 3MF**
- **Spec:** technical design 17; io.md 3.9.
- **Spec first:** yes. A new engine.md 6 test next to tests 87 and 88. It says when a regenerated golden is expected (a zip or deflate change, or an OCCT bump) and which target it is pinned on.
- **Work:**
  - Add `tests/fixtures/box.3mf` and `two-bodies.3mf`, checked in `export_and_import.rs`.
  - Regeneration is opt-in and refused in CI, like `bless_mode`. Document it in the engine's AGENTS.md.
- **Impact:** low. **Size:** S.
- **Depends on:** nothing.

### ui/ (outside the blockers)

**UI-1. `index.html` lacks the color-scheme meta**
- **Spec:** ui.md 2.16.8 (line 2169).
- **Spec first:** no.
- **Work:** add `<meta name="color-scheme" content="light dark" />`, and a Vitest test that reads index.html.
- **Impact:** low. Without it, a dark-mode OS sees a light flash at cold start. **Size:** S.
- **Depends on:** nothing. It can land any time.

**UI-2. The web build ships without a CSP or hardening headers**
- **Spec:** desktop.md 2.8 (line 381) and open issue 5; delivery.md 2.7.
- **Spec first:** yes. delivery.md 2.7: a CSP adapted from desktop (with `'wasm-unsafe-eval'`, `worker-src 'self'` and the ADR 0013 font hosts), `frame-ancestors 'none'`, nosniff and Referrer-Policy.
- **Human gate:** security review.
- **Work:**
  - Update `_headers`, the pinned text in `check-dist.mjs` and its tests, and the vite preview headers.
  - Add an E-test that sees no `securitypolicyviolation`.
- **Impact:** low. **Size:** S.
- **Depends on:** B-1 (a preview deploy to check it) and B-9.

**UI-3. Tab order and target size have no tests**
- **Spec:** ui.md 1.1, 3.24; technical design 13.
- **Spec first:** yes. Two new E-tests in ui.md 6.5:
  - tab through the regions of 3.24 on cube.ptsm and check the order and focus return;
  - every visible button, `[role=button]` and input is at least 24 by 24 CSS px at scale 1.
- **Work:** optionally, `@axe-core/playwright` as a dev dependency.
- **Human gate:** only if the axe dependency is added (with an ADR entry).
- **Impact:** low. **Size:** M.
- **Depends on:** nothing.

**UI-4. Two ui.md 5 budget rows have no measurement point**
- **Spec:** ui.md 5, 2.18, 6.7.
- **Spec first:** yes. Add 2.18 names (for example `ps:apply-events`) and P-tests in 6.7 for:
  - applying `document_changed` plus `recomputed` for 50 features, 8 ms;
  - the expression validation round trip, 50 ms.
- **Work:** add the marks in `core-events.ts`, `useExpression.ts` and `marks.ts` `TIMING_NAMES`.
- **Impact:** low. **Size:** S.
- **Depends on:** B-11.

**UI-5. The worker transfer budget (2 ms for a 50 MB frame) is not measured**
- **Spec:** protocol.md 5.
- **Spec first:** small. The protocol.md 5 preamble names where it is measured.
- **Work:** a Vitest browser-mode bench next to `frame.bench.ts`, or an `@perf` e2e test. It does a real Worker `postMessage` with a transfer list and asserts the sent buffer is detached afterwards.
- **Impact:** low. **Size:** S.
- **Depends on:** B-11 if it goes the e2e route.

### partsmith-py and python/

**PY-1. `Document.save` replaces the .ptsm before writing its assets**
- **Spec:** python-api.md line 578; engine.md 3.14.
- **Spec first:** yes. The order becomes: assets first, written atomically, replacing any file whose BLAKE3 does not match its name; then the document; then `mark_saved`.
- **Work:** change `document.py:149-163` and add tests in `test_documents.py`.
- **Impact:** low. **Size:** S.
- **Depends on:** schedule the spec text with B-5, so the hosts copy the right rule.
- **Status (2026-10-23):** done, not on `main` yet; review findings 1 to 8 fixed. file-format.md 3.11 holds the write order for every host to cite (ADR 0036, accepted for Python): the order and the failure rules bind every host, while step 1's entry check (read a regular file of the asset's size, replace any other entry) binds Python and is proposed for the desktop and the web until B-5's capability review, and the web's limits (no link detection, no flush) and the linked `assets/` folder (Python follows it) are stated there. python-api.md 2.10, 3.1 and 3.7 hold the Python rule; tests 97 to 102 (the save) and 103 (export errors) pass. `Document.save` (`python/partsmith/document.py`) writes the assets, then the document, then sends `mark_saved`; `python/partsmith/_files.py` flushes each file with `os.fsync` before its `os.replace`, raises a failed write again under the target's name, and has `holds`, which looks with `os.lstat` and reads a regular file of the asset's size through one descriptor opened with `O_NOFOLLOW` and `O_NONBLOCK` and checked with `os.path.samestat`, so an entry swapped for a link or a pipe after the look is never read; `python -m partsmith build` names the failed file. python-api.md open issue 10 records that Python replaces a linked document file while the desktop writes through it, and points at ADR 0037's options (A) to (C). The desktop options of ADR 0036 wait for B-5's capability review.
- **Follow-up (only if ADR 0037 option (A) is accepted):** `Document.save` and the export functions write a document file that is a symbolic link through the link to the file it points to, keeping `assets/` next to the link (python-api.md 3.1 and 3.7, open issue 10, a test next to tests 97 to 102). Size S.

**PY-2. Three GUI options have no dedicated Python keyword**
- **Spec:** technical design 10 (line 350); python-api.md 2.7 and 3.5 (line 623); features/extrude.md 98, chamfer.md 80, linear_pattern.md 91; features.md test 23.
- **Spec first:** yes, or an ADR that keeps the generic `feature()` form on purpose.
- **Human gate:** public API.
- **Work:**
  - extrude: two-sided distances. chamfer: `second_distance` and `flip`. linear_pattern: `reverse`, including inside `second`.
  - Emit them from the hooks at `extrude.rs:169`, `chamfer.rs:232` and `linear_pattern.rs:221`.
  - Regenerate the affected twins.
- **Impact:** low. **Size:** M.
- **Depends on:** phase 1 step 8 (its Python changes).

**PY-3. The Python budgets are not measured**
- **Spec:** python-api.md 5, rows 1 to 4. Row 5 depends on the plan's PY2 and stays out.
- **Spec first:** small. Say how the rows are measured.
- **Work:**
  - `python/benches/budgets.py`, or a `budgets` pytest marker beside `packaging`.
  - It measures: import, per-call overhead, the fifty-features twin, and Selection filtering over 1000 elements.
  - Run it in the python CI job, and list it in the partsmith-py AGENTS.md.
- **Impact:** low. **Size:** M.
- **Depends on:** B-1 and B-10.

### partsmith-sketch

**SK-1. Native and WebAssembly sketch results are never compared**
- **Spec:** sketch.md 3.12.
- **Spec first:** yes. A sketch.md 6 test: the benchmark sketches, drag frames and profiles on wasm, compared with a native baseline within 1e-9.
- **Work:**
  - A CI step: `cargo test -p partsmith-sketch --target wasm32-unknown-emscripten`.
  - Relax bitwise literal asserts, such as test 249, to 1e-9.
- **Impact:** low. **Size:** S.
- **Depends on:** B-1 and the shared wasm job.

**SK-2. No nightly job runs the property tests at 4096 cases**
- **Spec:** sketch.md 6.13; crates/partsmith-sketch/AGENTS.md.
- **Spec first:** yes. delivery.md 1.1 plus a section 6 test, or a 1.2 deferral.
- **Work:** a scheduled workflow (cron and `workflow_dispatch`) that runs `PROPTEST_CASES=4096 cargo test -p partsmith-sketch --release --test integration properties::`. It needs no OCCT cache. List it in `.github/workflows/AGENTS.md`.
- **Impact:** low. **Size:** S.
- **Depends on:** B-1.
- **Status (2026-10-16):** workflow written, not on `main` yet. `.github/workflows/nightly.yml` follows delivery.md 1.1 and 2.11 (cron `23 4 * * *` and `workflow_dispatch`; a scheduled run skips when the newest concluded nightly run already tested the head of `main`; `timeout-minutes: 20`, now in 2.11, caps a run that never finishes), with the skip notice of section 4, the budget of 5 and section 8; sketch.md 6.13 points at it, and `.github/workflows/AGENTS.md` lists it. Changed in review: the skip check counted only runs that passed or failed, so a run stopped by `timeout-minutes`, which does not conclude `failure`, was repeated every night on an unchanged `main`, and a failing `gh` failed the step, recording a run that later nights counted as tested. It now asks for the newest finished run (`gh run list --status completed --limit 1`), whatever its conclusion, and on a `gh` failure warns and tests (2.11, the warning of section 4). Test 51 in `ui/scripts/workflows.test.mjs` runs the step's script with `bash -e` and a stub `gh` (the head tested, another commit, no finished run, `gh` failing); the real `gh` 2.89.0 accepts the same arguments on this repository's `ci.yml` runs, and the 4096-case run passes (15 tests in 3.5 s). Test 48 needs the workflow on `main`: one `workflow_dispatch` run, then a scheduled night with `main` unchanged and one after a new commit. Measured on a 4-core machine: a cold release build of the sketch tests takes 2 min 26 s and the 4096-case run 1.6 s, so a night costs about 3 billed minutes after a change and 1 when skipped.

### partsmith-cli

**CLI-1. cli.md open issue 1 is wrong about the UI**
- **Spec:** cli.md 7 open issue 1 (line 410) against ui.md 1.2.
- **Spec first:** this is the spec change itself.
- **Work:** reword it to say STEP import is left to Python in M1, and the UI import and `partsmith import` come later. Do not add a route.
- **Impact:** low. **Size:** S.
- **Depends on:** nothing. It can land any time.

**CLI-2. The CLI budgets of cli.md 5 are not measured**
- **Spec:** cli.md 5.
- **Spec first:** small. A line on how they are measured, plus an ADR 0027 list entry or a new ADR.
- **Work:**
  - `benches/budgets.rs` on partsmith-budgets, timing the release binary through `std::process::Command`.
  - A 200 KB workload, generated at run time or added as an `examples/bench/<name>`.
  - rebuild and export on fifty-features, at the engine budget plus 100 ms.
  - Cargo.toml bench wiring, and the command in the crate's AGENTS.md.
- **Impact:** low. **Size:** M.
- **Depends on:** nothing.

### partsmith-protocol

**PROTO-1. The protocol AGENTS.md says `Edit` accepts unknown fields**
- **Spec:** model.md 2.9 (line 568); protocol.md 2.3.
- **Spec first:** no.
- **Work:**
  - Fix line 24 of `crates/partsmith-protocol/AGENTS.md`: `Edit` rejects unknown fields, while the `FeatureRecord` inside it keeps unknown keys as params.
  - Add an acceptance test that `apply_edits` rejects an unknown `Edit` field, if none exists.
- **Impact:** low. **Size:** S.
- **Depends on:** nothing.

### Delivery and licensing (outside the blockers)

**DEL-1. The CC0 grant has no license text** (resolved: ADR 0028 puts everything under Apache-2.0)
- **Spec:** technical design 3.
- **Spec first:** an ADR that records the grant and its scope.
- **Human gate:** license.
- **Work:**
  - A root `LICENSE-CC0`. It cannot go under `examples/`, because 2.8 forbids it there.
  - A README list of the exact paths: `docs/spec/file-format.md`, `docs/format/ptsm.schema.json`, `examples/**`.
  - Optional: an SPDX note in file-format.md.
- **Impact:** low. **Size:** S.
- **Depends on:** the same license gate as B-2.

**DEL-2. No CI check covers npm licenses**
- **Spec:** technical design 17; the AGENTS.md license invariant.
- **Spec first:** yes. delivery.md 1.1, 2.4 and 6, using the deny.toml SPDX allow list.
- **Human gate:** dependencies and licenses.
- **Work:**
  - A `ui/scripts` node script over `pnpm licenses list --prod --json`, with a `node:test` test and a `ui/package.json` script.
  - A step in the `ui` job.
  - An ADR entry that logs the npm packages and their licenses.
- **Impact:** low. **Size:** S.
- **Depends on:** B-1. Share the license data with B-2.
- **Status (2026-10-16):** code written with B-2, which needs its exports, not on `main` yet, human gate open. `ui/scripts/check-licenses.mjs` with `pnpm check:licenses` (tests 43 to 47 pass; it reports 11 npm packages that ship, each under a license `deny.toml` allows) and its step in the `ui` job. It also exports `compareText`, `compareNameAndVersion`, `runCommand` and `commandFailureMessage`, which the notices generator shares (delivery.md 2.10). ADR 0033 logs the eleven packages: eight production packages (seven MIT, one ISC) and `rolldown`, `tailwindcss` and `vite` (all MIT), whose code the build writes into `ui/dist`. The other development dependencies hold MIT-0, Python-2.0 and BlueOak-1.0.0; they do not ship, and the check leaves them out. Rechecked by the DEL-2 implementer: 84 script tests pass (90 with the workflow tests 49 to 51 and the new cases of test 38), the command takes about 2 s (budget 10 s), and on a copy of the script with a narrowed `deny.toml` it exits 1 with one not-allowed message per package, the deny.toml message, or the `pnpm` failure message.

## Suggested sequence

1. **Now, alongside the phases:**
   - Human: B-1 setup; the B-2 and DEL-1 license gate.
   - Plan edits: R-1, R-2.
   - Small items: CLI-1, UI-1, PROTO-1, B-12.
   - Draft B-13.
2. **Wave A, right after the phases (data safety and hosts):**
   - B-3 and B-4 together, then B-5 (with PY-1's spec) and B-6 sharing one desktop capability review, then B-7.
   - In parallel: B-8, B-9, B-10, B-11.
3. **Wave B (M1 behavior and corpus):** PRINT-1; EX-1 to EX-5; ENG-1, ENG-4 and SK-1 as one wasm CI job; ENG-2.
4. **Wave C (hardening):** EX-6 to EX-10, IMP-1, PRINT-2, PRINT-3, ENG-3, ENG-5, ENG-6, UI-2 to UI-5, PY-2, PY-3, CLI-2, SK-2, DEL-2.

These items wait for a human: B-1, B-2, B-5, B-6 (only with option (ii) or (iii) of ADR 0037), B-8 and B-9 (ADR 0038, recorded in delivery.md 8), B-10 (recorded in delivery.md 8), PRINT-2, PRINT-3, PY-2, UI-2, UI-3 (only with axe), IMP-1 (only with option (b)), DEL-1 and DEL-2.

## Merged inputs and items left out

**Merged:**
- EX-1 merges the three import-example gaps (modeling, print and delivery). Its export half became EX-6.
- EX-3 merges the three sketch constraint gaps.
- EX-5 merges the modeling and delivery option gaps.
- EX-2 merges the print and delivery print-check gaps.
- B-10 merges the hosts and delivery wheel gaps.
- B-13 merges the hosts and delivery documentation gaps, including the release-notes checklist and the contributor pointer.
- The sketch P-2 gap, the ui P-1 to P-8 gap and part of the engine web gap became B-11. Their leftovers became UI-4, ENG-2 and R-2.
- The 250 KB row in the ui perf gap is B-12.
- Each merged item takes the higher impact of its inputs.

**Left out on purpose:**
- Slot and tangent-arc tool examples: later than M1 (ui.md 1.2, sketch.md 1.2).
- Wheel publishing: not in M1.
- Fixed-hardware perf and visual gates: deferred by delivery.md 1.2.
- A UI or CLI STEP import route: later per cli.md 1 and ui.md 1.2. CLI-1 only corrects the spec text.
- P-9 and the drop-check mark: reference stability scope, assigned to a phase in R-2.
- PY-3's row 5: depends on the plan's PY2.