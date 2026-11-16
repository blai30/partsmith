# 0070: A Pyodide spike in Chromium decides whether web scripting ships in v1.0 or v1.1

Status: Accepted (owner's rule of 2026-11-05): web scripting in v1.0. The wave 1 spike ran on 2026-11-16. Every criterion from C1 to C7 passed (Result). Decision rule item 1 then names v1.0. The v1.0 outcome amends the Pyodide bullet of [ADR 0017](0017-keep-the-web-build-desktop-leads.md), "Python on the web (Pyodide) is not planned until there is demand", a decision the owner made on 2026-11-04. This ADR is the M2 plan's choice 25, option (a): the spike ran on a throwaway branch, merged nothing, and recorded its results here, reproducible from this record. The choices below are Accepted (owner's rule of 2026-11-05), choice 3 as the note of 2026-11-16 amends it. Nothing changes on `main`: no code, schema, protocol, dependency, license or capability. Pyodide 314.0.7 and its MPL-2.0 license go to human review at the M2 gate with this record, as the dependency M4 would add (AGENTS.md Process, m2-plan.md gates 18 and 19). The amendment of ADR 0017 goes to the same review, with the other ADRs that amend an owner's decision (m2-plan.md gate 19). Under decision rule item 4, web scripting ships in v1.1 if that review rejects Pyodide or its license terms, whatever the measurements. This status then says so.

## Context

- **What the design asks.** Technical design section 10: on the web, Python runs in Pyodide in a worker and calls the WebAssembly core through a JavaScript bridge, with pure-Python packages only. Section 4: Pyodide is loaded lazily. Section 19 lists the interop of Pyodide and the web core as a medium risk. Its mitigation is a prototype in phase 1, and if needed web scripting in v1.1, with code-first files on the desktop only at launch. The plan reads phase 1 as M2 (section 7.2 item 2) and holds the code workflow in M4 (section 5.2). This spike tells M4 which release web scripting is for (item 11, risk 19).
- **The web core.** `partsmith-wasm` builds the Emscripten module `createPartsmithCore` with `-sMODULARIZE=1`, `-sEXPORT_ES6=1`, `-sENVIRONMENT=worker` and `-sMAXIMUM_MEMORY=4GB` (`crates/partsmith-wasm/build.rs`). `scripts/build-wasm.sh` copies it to `ui/public/core/partsmith_core.mjs` and `partsmith_core.wasm`. Its three C ABI exports `ps_alloc`, `ps_free` and `ps_call` trade protocol frames with the module's one `Session`, kept in a `thread_local!` (ADR 0010). That session has display meshes on and random ids (`session_options` in `crates/partsmith-wasm/src/lib.rs`). `callCore` in `ui/src/core/bridge.ts` copies a request frame in, calls `ps_call` and copies the response out. The core is built without shared memory.
- **The Python package needs little from its extension.** `python/partsmith` is pure Python. It imports only the standard library (`dependencies = []` in `python/pyproject.toml`). It reaches the engine only through `partsmith._core`: `_client.py` makes a `Session(display=False, deterministic_ids=True)` for each document and sends frames with `call(frame: bytes) -> bytes`. `__main__.py` reads `ENGINE_VERSION`, `PROTOCOL_VERSION` and `FORMAT_VERSION`. The package makes the ids of its records itself (`Document._new_id` in `document.py`). The engine's id generator (`SessionOptions::id_generator` in `crates/partsmith-engine/src/options.rs`) names the records that `import_step` adds (`crates/partsmith-engine/src/exchange.rs`) and those that `apply_fix` adds, such as a new body or a coincident constraint (`crates/partsmith-engine/src/fixes/`). The package sends `import_step` but never `apply_fix`. M2's item 24 moves Python's `Rectangle` and `Polygon` onto core recipes whose ids come from the same generator.
- **The page's rules.** `ui/public/_headers` sends `Cross-Origin-Opener-Policy: same-origin`, `Cross-Origin-Embedder-Policy: require-corp` and a content security policy whose `script-src` is `'self' 'wasm-unsafe-eval'`, with `connect-src 'self'` and `worker-src 'self'` (ADR 0049). It allows no host and no `'unsafe-eval'`. Every served file stays under 25 MiB (`check:dist`, delivery.md section 3.5). The initial JavaScript stays under 250 KB gzipped (ui.md section 5).
- **Budgets to compare with.** Technical design section 14 and ui.md section 5 give the web 4 s from cold start to usable, measured on a reload with the core module in the HTTP cache. python-api.md section 5 row 2 gives the desktop 2 ms of Python overhead per API call. A web budget is 3 times the desktop's (technical design section 14, ADR 0057 decision 5). The engine's caches may hold 512 MiB on wasm32 (`DEFAULT_CACHE_BUDGET` in `crates/partsmith-engine/src/options.rs`).
- **Pyodide today** (the npm registry, read on 2026-11-07). The newest release is 314.0.7, published on 2026-09-14 under MPL-2.0. Its `pyodide-lock.json` names Python 3.14.2 and the platform `emscripten_5_0_3`. Its runtime files are `pyodide.mjs` (17,931 bytes), `pyodide.asm.mjs` (1,250,344 bytes), `pyodide.asm.wasm` (9,598,218 bytes), `python_stdlib.zip` (2,545,637 bytes) and `pyodide-lock.json` (119,077 bytes). The largest is 9.2 MiB. The npm package depends on `ws` and `@types/emscripten`. The maintenance lines are 0.29.5 and 0.27.8, both published on 2026-09-16. The core and OCCT are built with Emscripten 4.0.15 (`scripts/emsdk-version.sh`).
- **A risk visible before the spike.** A text search of 314.0.7's `pyodide.asm.mjs` finds `eval` in the dynamic linker's paths for the `EM_ASM` and `EM_JS` code of side modules, and in `emscripten_run_script`. The page's policy blocks `eval`. Whether loading the standard library and running a script reaches those paths is question 4.
- **MPL-2.0 in the repository.** `deny.toml`'s list of allowed licenses already holds MPL-2.0 and Python-2.0. The npm license check reads the same list (delivery.md section 2.10). ADR 0033 lists five crates under Tauri that come under MPL-2.0 on every desktop platform. As a production dependency, Pyodide would pass `check:licenses`: MPL-2.0 is on the list. The spike adds it as a development dependency and copies its files into `ui/public` by hand. Neither `check:licenses` nor the web notices that `pnpm notices` writes see those files: both take the production dependencies and `EMITTED_BUILD_PACKAGES` (`shippedPackages` in `ui/scripts/check-licenses.mjs`). The review the plan asks for is of Pyodide as a new runtime dependency of the web build, and of what MPL-2.0 asks of a web deployment that serves it.

## Decision

**The questions.**

1. Does Pyodide load in a module worker beside the shipped web core, from the app's own origin? How long does it take with an empty HTTP cache, and with its files in the cache?
2. Does the unchanged `partsmith` package run on Pyodide when `partsmith._core` is a pure-Python module that calls `ps_call` synchronously through JavaScript?
3. Does a script that builds a box through the web core give the native result? What does Python add to each API call?
4. Does it all run under the web app's content security policy and cross-origin isolation as they are?
5. How much memory does Pyodide take?
6. Which files would the web build gain? Is each under the 25 MiB limit?
7. Which licenses come with the files served, and what does MPL-2.0 ask of a deployment that serves them?
8. What would M4 need from the core that the shipped core lacks? The spike records findings here and measures nothing for this question. Three are known. A Python document needs a session of its own with display meshes off and `SequentialIds` for the records the engine creates: those of STEP import now, and every rectangle and polygon once item 24 lands. A script feature that runs during a build would need a call from the core into Python through a JavaScript import. Calling an import is `unsafe` code. ADR 0010 allows such code only in partsmith-occt-sys and on the three exports. Python on the web is 3.14, beside CPython 3.12 or later on the desktop.

**The route** (choice 1). Pyodide runs in the same module worker as a new instance of the shipped web core. The worker registers a JavaScript module named `partsmith_web_core` with `pyodide.registerJsModule`. It has two functions:

- `call(frame)` takes a request frame as a `Uint8Array`, runs `callCore` on the core instance and returns the response frame as a `Uint8Array`. The worker holds a JavaScript copy of `callCore` from `ui/src/core/bridge.ts`, line for line. It adds the time `callCore` took to a running total. `callCore` copies the frames in and out of the core's memory and calls `ps_call`, the path the app's core worker takes. ADR 0043's engine page times each request around it the same way.
- `core_milliseconds()` returns that running total.

A pure-Python module takes the place of the extension module. The branch keeps it as `spike/pyodide/_core.py`, and step 3 below zips it as `partsmith/_core.py`:

- `PROTOCOL_VERSION`, `ENGINE_VERSION` and `FORMAT_VERSION` are the `protocol_version`, `engine_version` and `format_version` of one `engine_info` request, sent when the module is imported.
- `Session(*, display=False, deterministic_ids=True)` takes both keywords and ignores them. Every `Session` sends its frames to the core instance's one session.
- `Session.call(frame: bytes) -> bytes` passes `frame` to `partsmith_web_core.call` as a `Uint8Array` (`pyodide.ffi.to_js`) and returns the response with `to_bytes()`.
- `ptsm_schema()` and `protocol_schema()` raise `NotImplementedError`. No question uses them.

No crate changes. The core keeps its three exports and its `unsafe` code as it is.

**The box script.** Every timed run on the web and the native baseline run exactly this text:

```python
from partsmith import Document, Part, box

document = Document()
with Part("Box") as part:
    box(20, 30, 40)
volume = part.volume()
text = document.to_text()
document.close()
```

**How the spike measures.** On the branch `spike/pyodide`, made from the commit wave 1 has reached when the spike starts:

1. Build the web core with `scripts/build-wasm.sh`, with Emscripten 4.0.15 active and OCCT built for WebAssembly by `scripts/build-occt.sh wasm <prefix>`.
2. Add Pyodide as a development dependency with `pnpm -C ui add -D pyodide@314.0.7`. The lock file's integrity must equal the registry's, `sha512-0YvXxEhfEdpLfb/XkM2BFAeMROq0iMUX2bzzH9pOttyMcWkwq+HbE5uyuGD82LN7y2q+SNvi/6V5JEsOlD2R1A==`. Copy the five runtime files above from `ui/node_modules/pyodide/` to `ui/public/pyodide/`.
3. Zip every `.py` file of `python/partsmith` under `partsmith/`, with `spike/pyodide/_core.py` as `partsmith/_core.py`, into `ui/public/pyodide/partsmith.zip`. The zip holds no `_core.pyi`, no `_core.abi3.so` and no `__pycache__`. `python/partsmith` itself stays unchanged on the branch.
4. Add the page `ui/public/pyodide-spike/index.html`, its script `main.mjs` and its module worker `worker.mjs`, in plain JavaScript that `pnpm -C ui build` copies into `ui/dist` as it is. The page loads its script from a file: the policy allows no inline script. The worker first instantiates the core with `createPartsmithCore` from `/core/partsmith_core.mjs`, untimed. It then imports `/pyodide/pyodide.mjs`, calls `loadPyodide({ indexURL: '/pyodide/' })`, registers `partsmith_web_core`, unpacks `partsmith.zip` into the interpreter's `site-packages` with `pyodide.unpackArchive`, and imports `partsmith`.
5. Run `ui/e2e/pyodide-spike.spec.ts`, tagged `@perf`, with `PARTSMITH_PERF=1 pnpm -C ui exec playwright test pyodide-spike --project=chromium`, headless, on the 4-core development container (choice 4). It opens `/pyodide-spike/index.html` on the server of `ui/playwright.config.ts`, which runs `pnpm build` and `pnpm preview`. The preview server sends every header of `ui/public/_headers`, the content security policy included (ADR 0049). The Vite dev server that `serveTestPage` starts for ADR 0043's engine page sends every header but the policy. The spike therefore does not use it.
6. Run the native baseline on the same container: the release build of `partsmith-py` that `maturin develop --release` installs (python-api.md section 5), the box script, and row 2 of `python/tests/test_budgets.py`.

The spike takes these samples. A time is the difference of two `performance.now()` calls in the worker, or of two `time.perf_counter()` calls in Python. Each timed value is the median of 7 samples after one untimed run, as python-api.md section 5 takes them.

- **First load:** in a new browser context with an empty HTTP cache, from the statement before `import('/pyodide/pyodide.mjs')` to the return of `import partsmith`, with the bytes the files transferred (the sum of `transferSize` over the worker's resource timing entries). Recorded, not judged. No budget of the repository assumes a network.
- **Cold load:** the same two points, in a new worker of a page reloaded with Pyodide's files and `partsmith.zip` in the HTTP cache, the way ui.md section 5 measures cold start.
- **Box script:** its time from `Document()` to the return of `document.close()`, the time inside `callCore` during it (the change of `core_milliseconds()`), its `volume` and its `text`.
- **Python overhead per API call:** row 2 of python-api.md section 5, measured the way row 2 measures it. Its three operations are `box(20, 30, 40)` as a part's first feature, and `faces().top()` and `part.volume()` after an untimed box. A sample is the operation's time less the time inside `callCore` during it. On the desktop, row 2 takes off the time inside `Session.call`. That time includes decoding and encoding the frames in Rust. On the web, `callCore` holds the same work. The conversions between Python and JavaScript count as Python overhead. The largest of the three medians counts.
- **Memory:** after the box script, the byte length of Pyodide's WebAssembly memory and of the core instance's (each its Emscripten module's `HEAPU8`; the spike reads Pyodide's through its private `_module`), and the page's total from `performance.measureUserAgentSpecificMemory()`.
- **Policy:** every `securitypolicyviolation` event of the page and the worker, and every console message, from the page's load to the end of the last sample.
- **Files:** `pnpm -C ui build`, then `pnpm -C ui check:dist`, with Pyodide's files and `partsmith.zip` in `ui/dist/pyodide/` and the spike's page in `ui/dist/pyodide-spike/`, and the size of each file.

With the results the spike records the base commit, the branch's last commit, the Chromium version (`browser.version()`), the container's CPU count and load average, the CPython version of the baseline, and every sample.

**Pass criteria.**

| Criterion | Question | Passes when |
| --- | --- | --- |
| C1 | 1 | Cold load takes at most 4,000 ms, the web's cold start budget (technical design section 14). |
| C2 | 2 and 3 | The box script runs on the unchanged package. Its `volume` is within 1e-9 relative of the native run's, and its `text` is byte for byte the native run's. |
| C3 | 3 | Python overhead per API call is at most 6 ms, 3 times row 2's 2 ms. |
| C4 | 4 | No policy violation occurs, `ui/public/_headers` is unchanged, and `self.crossOriginIsolated` is true in the worker. |
| C5 | 5 | Pyodide's WebAssembly memory after the box script is at most 256 MiB, half the core's cache budget on wasm32. |
| C6 | 6 | `check:dist` passes with every served file under 25 MiB. The app's initial JavaScript is unchanged: only a page that runs a script loads Pyodide. |
| C7 | 2 | No crate changes. The route uses the shipped core with its three exports. |

**The decision rule** (choice 6).

1. Every criterion from C1 to C7 passes: web scripting ships in v1.0. M4's code workflow builds it on this route, behind its runtime ADR, its license review and its sandbox (m2-plan.md section 5.2). The status becomes `Accepted (owner's rule of 2026-11-05): web scripting in v1.0`.
2. A criterion fails, or a question has no answer when wave 1 lands: web scripting ships in v1.1. v1.0 runs scripts on the desktop and through the `partsmith` package, as technical design section 19 allows. The status becomes `Accepted (owner's rule of 2026-11-05): web scripting in v1.1`. M4's spec then says what the web app does with a document that holds a script feature.
3. A failed criterion may be measured once more on the same branch after a change. The change counts only when it keeps the invariants of AGENTS.md, adds no host, no `'unsafe-eval'` and no other directive to the policy, and touches no crate. This record then describes the change, and M4's spec inherits it.
4. The gate's review decides on Pyodide as a dependency. If it rejects Pyodide or its license terms, web scripting ships in v1.1 whatever the measurements. The status says so.

**What the branch does** (choice 3). `spike/pyodide` is pushed to the repository, where reviewers can read the code behind the numbers until the gate. Amended on 2026-11-16 by this ADR's note of that date: the branch stays in the development container, unpushed, and its diff against `d978be2` is committed as [`docs/design/spikes/pyodide.patch`](../design/spikes/pyodide.patch). It is never opened as a pull request and never merged. `ci.yml` runs only on pushes to `main` and on pull requests. The branch therefore builds nothing in CI and deploys nothing to Pages. It commits no file of Pyodide: step 2 fetches them. The branch is deleted after the gate's review. Before that, the result section takes the full text of `spike/pyodide/_core.py`, `ui/public/pyodide-spike/index.html`, `ui/public/pyodide-spike/main.mjs`, `ui/public/pyodide-spike/worker.mjs` and `ui/e2e/pyodide-spike.spec.ts`. Every number, and the code that measured it, then lives in this record.

**The license review** (question 7). The result names each file served, the licenses of the code in it, and where its source is: Pyodide's own code under MPL-2.0 (its repository at the tag 314.0.7), CPython and its standard library under the Python license, and the libraries and runtime code compiled into `pyodide.asm.wasm`, listed from Pyodide's build at that tag. It states what MPL-2.0 section 3.2 asks of a web deployment that serves Pyodide's files in executable form: the source code form of those files must be available, and recipients must be told how to get it, at no more than the cost of distribution. For M4 that means Pyodide's source at its tag in the web deployment's dependency source archives (delivery.md section 2.13), and an entry in `ui/public/THIRD-PARTY-NOTICES.txt` (delivery.md section 2.9). M4 makes Pyodide a production dependency, or adds it to `EMITTED_BUILD_PACKAGES`. The license check and the notices then cover the files the web build serves.

## Result

Run on 2026-11-16 (UTC). Machine: the 4-core development container (Intel Xeon at 2.10 GHz, 15 GB RAM, Linux 6.18.44). Browser: Chromium 153.0.8010.12 (`browser.version()`), Chrome for Testing, headless, under `@playwright/test` 1.64.0. Tools: Node 22.23.4, pnpm 10.28.0, Emscripten 4.0.15 for the core, Rust 1.97.0 and OCCT 7.9.3. Native baseline: CPython 3.12.3 with the release build of `partsmith-py` that maturin 1.15.0 installs. Pyodide 314.0.7 runs Python 3.14.2 on the platform `emscripten_5_0_3`. Its integrity in the lock file equals step 2's. Branch `spike/pyodide`, last commit `08f36b7`, base `d978be2`, kept as [`docs/design/spikes/pyodide.patch`](../design/spikes/pyodide.patch) (the note of 2026-11-16). The native baseline ran at `6fde35c`, whose `python/` and `crates/` are those of `d978be2` and `08f36b7`. [`docs/design/spikes/pyodide.json`](../design/spikes/pyodide.json) holds every sample, the resources of each load and the load averages. Each web set started with a one-minute load average under 0.5 (0.47 to 0.49). Nothing else ran during a set. Each value is the median of 7 samples after one untimed run.

Every criterion from C1 to C7 passes. Decision rule item 1 names v1.0 (Status). pnpm reported 314.0.8 as available on 2026-11-16. This record measures 314.0.7, the release choice 2 picks.

| Value | Measured | Criterion | Passes |
| --- | --- | --- | --- |
| First load with an empty cache, ms, and bytes transferred | 2,167.81 ms (samples 1,848.66 to 2,280.85). 12,492,170 bytes in every sample: Pyodide's five files and `partsmith.zip`. With the core's untimed files, the worker fetched 32,509,392 bytes. | recorded | |
| Cold load, ms | 2,113.09 ms in a persistent context, with every file in the HTTP cache (samples 1,765.50 to 2,245.97). Each load transferred 1,800 bytes: six 304 responses of 300 bytes. In Playwright's default context: 2,118.93 ms. | C1: at most 4,000 ms | yes |
| Box script on the web, ms, and its time inside `callCore` | 3.265 ms, of which 1.480 ms inside `callCore` | recorded | |
| Box script on the desktop, ms | 3.252 ms | recorded | |
| `volume` on the web and on the desktop, mm³ | 24000 on the web and 24000.0 on the desktop, in every run. Relative difference 0. | C2 | yes |
| `text` on the web equal to the desktop's | Yes. 283 bytes, byte for byte equal, SHA-256 `5e44187254b76716036a8b1825b32f9443a838162c313c4db0c723d6c9b72510`, the same in every run. `python/` is unchanged on the branch. | C2 | yes |
| Python overhead per API call (`box`, `faces().top()`, `volume()`), ms | Web: 0.360, 0.565 and 0.135. The largest, 0.565, counts. Desktop: 0.106, 0.284 and 0.052. Row 2 as `test_budgets.py` runs it printed 0.30. | C3: at most 6 ms | yes |
| Policy violations, and `crossOriginIsolated` in the worker | None, in any page load of the three measuring tests, as events or as reports, of the page or the worker. True. `ui/public/_headers` is unchanged. | C4: none, true | yes |
| Pyodide's WebAssembly memory, MiB | 30 (31,457,280 bytes). It is Pyodide's initial memory: it did not grow. | C5: at most 256 MiB | yes |
| The core instance's memory and the page's total, MiB | 16 (16,777,216 bytes), the core's initial memory. The page's total: 63.3 (66,421,737 bytes). | recorded | |
| Largest served file, bytes, and `check:dist` | 19,996,965, the core's `partsmith_core.wasm`. Pyodide's largest is `pyodide.asm.wasm`, 9,598,218. `check:dist` passes with 25 files. The initial JavaScript is 246,228 bytes gzipped on the branch and on `d978be2`. `dist/index.html` and every file of `dist/assets` are byte-identical to a build of `d978be2`. | C6 | yes |
| Crate changes | None. `git diff d978be2 08f36b7 -- crates/` is empty. The worker calls the shipped core's `_ps_alloc`, `_ps_free` and `_ps_call` through `callCore`. | C7: none | yes |
| Findings for M4 (question 8) | Ten, below | recorded | |
| Files served and their licenses (question 7) | Seven entries, below. No license text is served. | gate review | |

In the Passes column, an empty cell does not apply: the value is recorded, not judged.

**Question 4.** Every response carried the five headers of `ui/public/_headers`, the policy included. The only console message was the 404 of the page's `/favicon.ico` (`ui/e2e/README.md`). The text search of the Context finds `eval` in `pyodide.asm.mjs`'s dynamic linker, in `emscripten_run_script` and in its Node script loader. The spike reached none of them. It loads no side module, never calls `emscripten_run_script`, and runs in a browser, not on Node. `pyodide.mjs` probes for a classic worker with `importScripts("data:text/javascript,")`. A module worker refuses that call before any policy check. No violation follows.

**Spread between runs.** After the recorded set, the spec ran twice more in full, and its second test (cold load, box script, Python overhead, memory and policy) once alone. The load averages before each run were 0.19 to 0.40. Every criterion passed in every run. Per-call overhead medians of 0.1 to 1 ms vary by up to about 1.7 times between runs on this container. In those three runs, `faces().top()` gave 0.94, 0.565 and 0.565 ms, and `part.volume()` gave 0.16, 0.145 and 0.18 ms. C3 holds by more than 6 times even at 0.94 ms. The cold load gave 1,800.99, 2,114.95 and 1,692.40 ms in the persistent context, and 2,223.09 and 2,240.21 ms in the default context.

**How the run departs from the method.**

1. The branch is not pushed. Its diff is committed as `docs/design/spikes/pyodide.patch` (the note of 2026-11-16).
2. C1's cold load runs in a persistent Chromium context, with its profile on disk. Playwright's default context keeps its HTTP cache in memory. There, every reload fetched `pyodide.asm.wasm` (9.2 MiB) and the core's `partsmith_core.wasm` (19.1 MiB) again in full. That context does not meet the cold load's "with Pyodide's files and `partsmith.zip` in the HTTP cache". Its cold load is recorded beside C1's and passes too. P-1 of ui.md section 6.7 measures section 5's cold start "on a reload with the core module in the HTTP cache". P-1 reloads in the default context, where its reload fetches the core's module again. P-1 therefore does not meet section 5's condition either (ui.md open issue 46).
3. The preview server was started by hand with the configuration's own command, `pnpm build && pnpm preview --port 4173 --strictPort`. Playwright reused it. Each test then ran alone with `-g`, after the build had ended and the load average had settled.
4. Every measured call in the worker starts in a new task. Playwright's `Worker.evaluate` and `Page.evaluate` run their code, and the microtasks it queues, with DevTools' exemption from the `eval` check. A fourth test, the control, evaluates a string in a new task of the worker. It gets an `EvalError`. The worker's events and reports hold exactly that refusal, `script-src eval`.
5. The box script's first statement, the import, runs untimed on the web and natively. The harnesses split the script's statements with `ast`. The time runs from `Document()` to the return of `document.close()`. The text that runs is exactly the box script.
6. The native box script harness and a per-operation copy of row 2 are not on the branch. Row 2 prints only its largest median. Both are below the five files. Row 2 itself ran from `python/tests/test_budgets.py` as it is.

**Question 7: the files served and their licenses.**

| File | Bytes | Code in it | Licenses | Source |
| --- | --- | --- | --- | --- |
| `pyodide/pyodide.mjs` | 17,931 | Pyodide's loader, minified. StackFrame and ErrorStackParser, which Pyodide vendors in `src/js/vendor/stackframe`: the file's source map lists `stackframe.ts` and `error-stack-parser.ts`. | MPL-2.0 (Pyodide). MIT (StackFrame and ErrorStackParser, "Copyright (c) 2017 Eric Wendelin and other contributors"). | github.com/pyodide/pyodide at tag 314.0.7 (`b1e4fcc2488962f6360a97f19b9982d6fdb5d16f`), `src/js` and `src/js/vendor/stackframe` |
| `pyodide/pyodide.asm.mjs` | 1,250,344 | Emscripten 5.0.3's generated glue and JavaScript libraries, with Pyodide's JavaScript core. The same StackFrame and ErrorStackParser. Emscripten's MiniLZ4 from `third_party/mini-lz4.js`, which `-s LZ4=1` brings in as `LZ4.codec`. | MIT or UIUC (Emscripten). MPL-2.0 (Pyodide). MIT (StackFrame and ErrorStackParser). MIT (MiniLZ4, "Copyright (c) 2012 Pierre Curto"). | Pyodide at 314.0.7. Emscripten 5.0.3 with the five patches of Pyodide's `emsdk/patches`. |
| `pyodide/pyodide.asm.wasm` | 9,598,218 | CPython 3.14.2 with Pyodide's nine patches. Pyodide's C core (`src/core`) and libhiwire (`6a1e67280a15d929ebeceee54a6358c9c8d5f697`). libffi (`f08493d249d2067c8b3207ba46693dd858f95db3`). xz 5.2.2 (liblzma). SQLite 3.39.0. zstd 1.5.7. zlib 1.3.1 and bzip2, Emscripten ports. expat 2.7.3, libmpdec and HACL* inside CPython. Emscripten's system libraries: musl, libc++, libc++abi, compiler-rt and Emscripten's own C code. | Python-2.0, with the licenses of CPython's incorporated software below. MPL-2.0 (Pyodide's core and libhiwire). MIT (libffi, expat, musl). Public domain (xz, SQLite). BSD-3-Clause (zstd, also offered under GPLv2). zlib License (zlib). bzip2's BSD-style license. BSD-2-Clause (libmpdec). MIT or Apache-2.0 (HACL*). Apache-2.0 WITH LLVM-exception (libc++, libc++abi, compiler-rt). MIT or UIUC (Emscripten's C code). | Pyodide 314.0.7's `Makefile`, `Makefile.envs` (`-lffi -lhiwire -llzma -lsqlite3 -lzstd`, `-s USE_ZLIB`, `-s USE_BZIP2`, `-s LZ4=1`) and `cpython/Makefile` (`Python-3.14.2.tgz`, `xz-5.2.2`, `zstd-1.5.7`, `sqlite-autoconf-3390000`). The binary holds the strings `deflate 1.3.1`, `inflate 1.3.1`, `expat_2.7.3`, `3.39.0` and `libbzip2`. |
| `pyodide/python_stdlib.zip` | 2,545,637 | CPython 3.14.2's standard library in Python (614 entries), with Pyodide's packages `pyodide` and `_pyodide` | Python-2.0, with the licenses of the incorporated software below that is Python code, such as `asyncio`. MPL-2.0 (Pyodide's `src/py`). | CPython 3.14.2. Pyodide at 314.0.7, `src/py` and `tools/create_zipfile.py`. |
| `pyodide/pyodide-lock.json` | 119,077 | Pyodide's index of 357 packages. The spike serves none of them. | MPL-2.0 (Pyodide's build output) | Pyodide at 314.0.7 |
| `pyodide/partsmith.zip` | 52,323 | The `.py` files of `python/partsmith`, and `spike/pyodide/_core.py` | Apache-2.0 | This repository |
| `pyodide-spike/index.html`, `main.mjs` and `worker.mjs` | 10,708 | The spike's page | Apache-2.0 | This repository |

CPython's `LICENSE` holds the PSF License Version 2 with the BeOpen, CNRI and CWI agreements. Its `Doc/license.rst` lists the licenses of the software CPython incorporates. Less its OpenSSL entry, that list applies here: Mersenne Twister, Sockets, Asynchronous socket services, Cookie management, Execution tracing, UUencode and UUdecode functions, XML Remote Procedure Calls, test_epoll, Select kqueue, SipHash24, strtod and dtoa, expat, libffi, zlib, cfuhash, libmpdec, W3C C14N test suite, mimalloc, asyncio, Global Unbounded Sequences (GUS) and Zstandard bindings. OpenSSL is not linked: `_ssl` and `_hashlib` are disabled in Pyodide's `cpython/Setup.local`.

No license text is served. The npm package `pyodide@314.0.7` ships no license file. Its `package.json` names the license `MPL-2.0`. `python_stdlib.zip` holds no `LICENSE`. Neither `pyodide.mjs` nor `pyodide.asm.mjs` carries a license or copyright comment, not even for the MIT code they bundle. Besides Pyodide's and Emscripten's own code, StackFrame, ErrorStackParser and MiniLZ4 are the only JavaScript in Pyodide's served files. At the tag, `src/js/vendor/stackframe` is Pyodide's only vendored directory. `src/js/package.json` depends only on `@types/emscripten` and `ws`. The files import `ws` on Node alone. It is not bundled. On the branch, `check:licenses` counts 11 shipped npm packages. `check:notices` reports the notices up to date. Both read only the production dependencies and `EMITTED_BUILD_PACKAGES`. `ui/public/THIRD-PARTY-NOTICES.txt` names no Pyodide file. MPL-2.0 and Python-2.0 are already on `deny.toml`'s list.

What MPL-2.0 section 3.2 asks, quoted from the `LICENSE` at the tag: "If You distribute Covered Software in Executable Form then: (a) such Covered Software must also be made available in Source Code Form, as described in Section 3.1, and You must inform recipients of the Executable Form how they can obtain a copy of such Source Code Form by reasonable means in a timely manner, at a charge no more than the cost of distribution to the recipient; and (b) You may distribute such Executable Form under the terms of this License, or sublicense it under different terms, provided that the license for the Executable Form does not attempt to limit or alter the recipients' rights in the Source Code Form under this License."

A web deployment that serves `pyodide.mjs`, `pyodide.asm.mjs` and `pyodide.asm.wasm` distributes Pyodide in Executable Form. It must make Pyodide's source at tag 314.0.7 available. It must tell its users how to get that source, at no more than the cost of distribution. MPL-2.0 works file by file (section 3.3, Larger Work). Partsmith's own files stay Apache-2.0. CPython's PSF license asks that its license agreement and copyright notice be kept in any derivative version. The MIT, BSD, zlib and bzip2 licenses of the linked libraries, of Emscripten's runtime and of the bundled JavaScript ask for their notices. The files the spike serves carry none. M4 therefore adds:

- Pyodide's source at 314.0.7, with the sources it links at the versions above, to the web deployment's dependency source archives (delivery.md section 2.13).
- Entries in `ui/public/THIRD-PARTY-NOTICES.txt` (delivery.md section 2.9) for Pyodide, CPython, the libraries in `pyodide.asm.wasm`, Emscripten 5.0.3's runtime and the bundled JavaScript. That runtime is Emscripten's JavaScript in `pyodide.asm.mjs` and its system libraries in `pyodide.asm.wasm`, under the licenses of the table above. The notices name only the core's Emscripten 4.0.15 today. The bundled JavaScript is StackFrame and ErrorStackParser (MIT, Eric Wendelin and other contributors) and MiniLZ4 (MIT, Pierre Curto).
- Pyodide as a production dependency, or a name in `EMITTED_BUILD_PACKAGES`. `check:licenses` and the notices then cover the files the web build serves.

**Question 8: findings for M4.**

1. A Python document needs a session of its own, with display meshes off and `SequentialIds` (known, confirmed in the code). `partsmith-py`'s `Session(display=False, deterministic_ids=True)` builds `SessionOptions::headless()` with `SequentialIds`, one per `Document`. The web core's one session, the `thread_local!` of `partsmith-wasm`, has display on and `RandomIdGenerator`. The spike's `_core.Session` ignores both keywords. Every `Document` shares that session. The box script's text matched only because the package names the records it adds itself (`Document._new_id`). On the web, a body that `import_step` adds would get a random id. Every rectangle and polygon would too, once item 24 lands. Even `SequentialIds` in one shared session would number across documents. `ps_call` has no session parameter. M4 therefore needs one of three:
   - a new export, a change to ADR 0010;
   - a protocol command that opens a session or a document with options, a change to the protocol schema;
   - one core instance per Python document, at 16,777,216 bytes of WebAssembly memory each to start (measured).
2. With display on, every recompute that Python drives on the web core also tessellates display meshes the package never reads (new). That work is part of the 1.480 ms inside `callCore` per box script. The headless session of item 1 removes it.
3. A script feature that runs during a build needs the core to call Python through a JavaScript import (known). That call is `unsafe` code outside ADR 0010's three exports. It needs a new ADR. The measured route needs no such call: Python calls the core, never the reverse.
4. Python on the web is 3.14.2 (`pyodide-lock.json`), beside CPython 3.12.3 for this baseline (known, confirmed). The unchanged package imported and ran the box script and row 2 on 3.14.2.
5. The two Emscripten runtimes share one module worker without conflict (new). The core uses Emscripten 4.0.15, `-sENVIRONMENT=worker` and 16 MiB of initial memory. Pyodide uses Emscripten 5.0.3 and 30 MiB. The route needs no change to the core's link arguments.
6. `ptsm_schema()` and `protocol_schema()` have no protocol command (new). The web `_core` raises `NotImplementedError` for both. No module of `python/partsmith` calls them. Only `_core.pyi` and `python/tests/test_core.py` name them.
7. The package reads and writes files through `os` and `pathlib` (new, not measured): `Document.save`, `import_step(path)`, the exports, and `_files.write_atomic` with `fsync` and `os.replace`. On Pyodide these reach Emscripten's in-memory file system, not the app's files. M4 maps them, for example with Pyodide's `mountNativeFS` over File System Access handles, or with bytes through the bridge.
8. Each call copies its frames between Python and JavaScript twice more, with `to_js` and `to_bytes`, besides the two copies of `callCore` (new). The overhead medians count those copies: 0.135 to 0.565 ms on the web, against 0.052 to 0.284 ms natively.
9. The spike's core instance never receives the page's cancel word (new). The worker does not call `useCancelWord`. A long rebuild that Python drives could not be stopped. M4 wires the word to Python's core instance if scripts can run long.
10. Playwright's `Worker.evaluate`, like `Page.evaluate`, runs its code and the microtasks it queues with DevTools' exemption from the `eval` check (method, for M4's tests). A policy test that runs code synchronously inside `evaluate` cannot see an `eval` refusal. The spike runs every measured call in a new task and adds a control.

**Re-measurement under decision rule item 3.** None. No criterion failed. Two earlier sets, at `6fde35c` and `db08bd5`, are superseded. Their box script and overhead samples ran synchronously inside `Worker.evaluate`, where DevTools allows `eval`. They could not have shown an `eval` refusal. `08f36b7` moved every measured call into a new task. This Result reports its set. `pyodide.json` keeps the superseded sets.

**To reproduce.** Apply the patch at the root of a checkout of `d978be2`. Build the core with `scripts/build-wasm.sh` (step 1). Run `pnpm -C ui install --frozen-lockfile`, then `python3 spike/pyodide/prepare.py`. It does steps 2 and 3: it copies Pyodide's five runtime files from `ui/node_modules/pyodide/` and writes `partsmith.zip`. Then run step 5's command and `pnpm -C ui check:dist`. The native baseline is step 6, in the `.venv` that python-api.md's commands make at the repository root. Row 2 runs with `.venv/bin/python -m pytest python/tests/test_budgets.py -m budgets -k row_2 -s`. The two harnesses below run with `.venv/bin/python -I`.

**The five files**, at `08f36b7`. The branch's other four files are in the patch: `.gitignore` (one line, `/ui/public/pyodide`), `spike/pyodide/prepare.py`, and Pyodide's entries in `ui/package.json` and `ui/pnpm-lock.yaml`.

**`spike/pyodide/_core.py`**

```python
"""A pure-Python `partsmith._core` for Pyodide, the spike of ADR 0070.

It takes the place of the `partsmith-py` extension module (python-api.md section 2.2). Every frame goes to the web
core through `partsmith_web_core`, the JavaScript module that the worker of ui/public/pyodide-spike/worker.mjs
registers. The worker runs one instance of the core, whose one session holds every document of every `Session`.
"""

import partsmith_web_core
from pyodide.ffi import to_js

from ._frames import decode_response, encode_request


class Session:
    """One engine session as the extension's `Session` offers it; on the web, a handle on the core's one session."""

    def __init__(self, *, display: bool = False, deterministic_ids: bool = True) -> None:
        # The core's session keeps its own options, display meshes on and random ids (ADR 0070 question 8), so both
        # keywords are taken and ignored.
        pass

    def call(self, frame: bytes) -> bytes:
        """Handles one request frame and returns the response frame. Engine errors are inside the frame."""
        return partsmith_web_core.call(to_js(frame)).to_bytes()


def _engine_info() -> dict:
    """The result of one `engine_info` request, sent when the module is imported."""
    return decode_response(Session().call(encode_request(1, {"type": "engine_info"}))).response["result"]


_ENGINE_INFO = _engine_info()

PROTOCOL_VERSION: int = _ENGINE_INFO["protocol_version"]
"""The protocol version of the frames `Session.call` takes and returns."""
ENGINE_VERSION: str = _ENGINE_INFO["engine_version"]
"""The engine version."""
FORMAT_VERSION: int = _ENGINE_INFO["format_version"]
"""The newest `.ptsm` format version the engine reads and writes."""


def ptsm_schema() -> str:
    """The .ptsm JSON Schema text, which the spike does not offer."""
    raise NotImplementedError("The Pyodide spike's partsmith._core has no ptsm_schema.")


def protocol_schema() -> str:
    """The protocol JSON Schema text, which the spike does not offer."""
    raise NotImplementedError("The Pyodide spike's partsmith._core has no protocol_schema.")
```

**`ui/public/pyodide-spike/index.html`**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <title>Pyodide spike</title>
    <!-- The policy of public/_headers allows no inline script, so the page's script is a file (ADR 0070). -->
    <script type="module" src="./main.mjs"></script>
  </head>
  <body></body>
</html>
```

**`ui/public/pyodide-spike/main.mjs`**

```js
// The script of the Pyodide spike page (ADR 0070). It keeps every content security policy violation
// event of the page from its start and starts the module worker that runs Pyodide beside the web
// core. The body's data-state turns "ready" once the worker's core runs, or "failed" with the reason
// in data-error. The spike's test drives the worker through Playwright's Worker.evaluate.

/** Every `securitypolicyviolation` event of the page, as `{directive} {URL}`. */
const violations = []
document.addEventListener('securitypolicyviolation', (event) => {
  violations.push(`${event.effectiveDirective} ${event.blockedURI}`)
})
globalThis.pyodideSpikeViolations = violations

const worker = new Worker(new URL('worker.mjs', import.meta.url), { type: 'module' })
worker.addEventListener(
  'message',
  () => {
    document.body.dataset.state = 'ready'
  },
  { once: true },
)
worker.addEventListener(
  'error',
  (event) => {
    document.body.dataset.state = 'failed'
    // An exception while the worker's module ran comes as an ErrorEvent; a script that did not
    // load or parse comes as a plain event without a message.
    document.body.dataset.error =
      event instanceof ErrorEvent && event.message !== ''
        ? event.message
        : 'its script did not load'
    event.preventDefault()
  },
  { once: true },
)
```

**`ui/public/pyodide-spike/worker.mjs`**

```js
// The module worker of the Pyodide spike page (ADR 0070, the route of choice 1). It runs one instance
// of the shipped web core and, beside it, Pyodide, whose Python reaches the core through the
// JavaScript module `partsmith_web_core`. The spike's test calls the functions on `pyodideSpike`
// through Playwright's Worker.evaluate; the worker posts one message once they are there.

/** Every `securitypolicyviolation` event of the worker, as `{directive} {URL}`. */
const violations = []
self.addEventListener('securitypolicyviolation', (event) => {
  violations.push(`${event.effectiveDirective} ${event.blockedURI}`)
})

// From here to wrapEmscriptenModule: ui/src/core/bridge.ts, line for line in plain JavaScript.

/** The core could not allocate memory for a request or its response. */
class CoreOutOfMemoryError extends Error {
  name = 'CoreOutOfMemoryError'
}

const LENGTH_SLOT_SIZE = 4

/** Copies a request frame into wasm memory, calls ps_call, copies the response out and frees both blocks. */
function callCore(core, request) {
  const requestLength = request.byteLength
  const requestPointer = core.alloc(requestLength)
  if (requestPointer === 0) {
    throw new CoreOutOfMemoryError(
      `The geometry core could not allocate ${String(requestLength)} bytes for a request.`,
    )
  }
  const slotPointer = core.alloc(LENGTH_SLOT_SIZE)
  if (slotPointer === 0) {
    core.free(requestPointer, requestLength)
    throw new CoreOutOfMemoryError('The geometry core could not allocate a response length slot.')
  }
  // Memory may have grown during either allocation, so every access reads core.memory() anew.
  core.memory().set(new Uint8Array(request), requestPointer)
  const responsePointer = core.call(requestPointer, requestLength, slotPointer)
  const memory = core.memory()
  const responseLength = new DataView(
    memory.buffer,
    memory.byteOffset,
    memory.byteLength,
  ).getUint32(slotPointer, true)
  core.free(requestPointer, requestLength)
  core.free(slotPointer, LENGTH_SLOT_SIZE)
  if (responsePointer === 0) {
    throw new CoreOutOfMemoryError('The geometry core could not allocate a response.')
  }
  const response = memory.slice(responsePointer, responsePointer + responseLength).buffer
  core.free(responsePointer, responseLength)
  return response
}

/** Wraps an instantiated Emscripten module as a CoreModule. */
function wrapEmscriptenModule(module) {
  return {
    // Emscripten prefixes C exports with an underscore. Pointers come back as signed i32, so
    // `>>> 0` reads them as unsigned addresses (memory can grow past 2 GiB).
    alloc: (length) => module['_ps_alloc'](length) >>> 0,
    free: (pointer, length) => {
      module['_ps_free'](pointer, length)
    },
    call: (request, requestLength, responseLengthOut) =>
      module['_ps_call'](request, requestLength, responseLengthOut) >>> 0,
    memory: () => module.HEAPU8,
    useCancelWord: (word) => {
      module.partsmithCancelWord = word
    },
  }
}

// The core instance, made before anything is timed. Its `main` creates its one session.
const { default: createPartsmithCore } = await import('/core/partsmith_core.mjs')
const coreModule = await createPartsmithCore()
const core = wrapEmscriptenModule(coreModule)

/** The time spent inside callCore so far, in milliseconds. */
let millisecondsInsideCore = 0

/**
 * The JavaScript module `partsmith_web_core` that Python imports. `call` takes a request frame as
 * a Uint8Array, which callCore copies as it copies an ArrayBuffer, and returns the response frame.
 */
const partsmithWebCore = {
  call(frame) {
    const started = performance.now()
    const response = callCore(core, frame)
    millisecondsInsideCore += performance.now() - started
    return new Uint8Array(response)
  },
  core_milliseconds: () => millisecondsInsideCore,
}

/**
 * The Python the timed functions call, defined once `partsmith` is imported: `run_box_script` times
 * the box script, and `overhead` takes one sample of row 2 of python-api.md section 5 the way
 * python/tests/test_budgets.py takes it, with the time inside callCore in place of the time inside
 * `Session.call`.
 */
const HARNESS = `
import ast
import time

import partsmith_web_core
from partsmith import Document, Part, box, faces


def milliseconds(operation):
    """The time operation() takes, between two time.perf_counter() calls, in milliseconds."""
    started = time.perf_counter()
    operation()
    return (time.perf_counter() - started) * 1000


def run_box_script(source):
    """Runs the box script once: its time from Document() to the return of document.close(), the time
    inside callCore during it, and its volume and text. Its first statement, the import, runs untimed."""
    statements = ast.parse(source).body
    imports = compile(ast.Module(statements[:1], []), "<box script>", "exec")
    rest = compile(ast.Module(statements[1:], []), "<box script>", "exec")
    namespace = {}
    exec(imports, namespace)
    core_started = partsmith_web_core.core_milliseconds()
    elapsed = milliseconds(lambda: exec(rest, namespace))
    core = partsmith_web_core.core_milliseconds() - core_started
    return [elapsed, core, namespace["volume"], namespace["text"]]


# Row 2's operations by name, each with whether an untimed box comes before it.
OPERATIONS = {
    "box": (lambda part: box(20, 30, 40), False),
    "top": (lambda part: faces().top(), True),
    "volume": (lambda part: part.volume(), True),
}


def overhead(name):
    """One sample of row 2's operation name in a new document with an untimed part P: its time less the
    time inside callCore during it."""
    operation, with_box = OPERATIONS[name]
    document = Document()
    try:
        with Part("P") as part:
            if with_box:
                box(20, 30, 40)
            core_started = partsmith_web_core.core_milliseconds()
            elapsed = milliseconds(lambda: operation(part))
        return elapsed - (partsmith_web_core.core_milliseconds() - core_started)
    finally:
        document.close()
`

/** The interpreter, once `load` has run. */
let pyodide = null

/**
 * Waits for a new task. DevTools lets the code that Playwright's Worker.evaluate runs, and the
 * microtasks it queues, evaluate strings whatever the policy says; a new task runs under the
 * worker's policy. So every function the test calls that runs Pyodide starts in a new task.
 */
function nextTask() {
  return new Promise((resolve) => {
    setTimeout(resolve, 0)
  })
}

/**
 * Loads Pyodide and the `partsmith` package, timed from the statement before the import of
 * pyodide.mjs to the return of `import partsmith`, and returns that time with every resource the
 * worker has fetched, each marked when it was fetched during that time.
 */
async function load() {
  await nextTask()
  const started = performance.now()
  const { loadPyodide } = await import('/pyodide/pyodide.mjs')
  pyodide = await loadPyodide({ indexURL: '/pyodide/' })
  pyodide.registerJsModule('partsmith_web_core', partsmithWebCore)
  const archive = await (await fetch('/pyodide/partsmith.zip')).arrayBuffer()
  const sitePackages = pyodide.runPython('import site; site.getsitepackages()[0]')
  pyodide.unpackArchive(archive, 'zip', { extractDir: sitePackages })
  pyodide.runPython('import partsmith')
  const milliseconds = performance.now() - started
  pyodide.runPython(HARNESS)
  const resources = performance.getEntriesByType('resource').map((entry) => ({
    path: new URL(entry.name).pathname,
    transferSize: entry.transferSize,
    duringLoad: entry.startTime >= started,
  }))
  return { milliseconds, resources }
}

/** Calls the harness's Python function `name` with `values` and converts its result to JavaScript. */
function callPython(name, ...values) {
  const python = pyodide.globals.get(name)
  try {
    const result = python(...values)
    if (typeof result !== 'object') {
      return result
    }
    const converted = result.toJs()
    result.destroy()
    return converted
  } finally {
    python.destroy()
  }
}

/** Runs the box script once: its time, the time inside callCore during it, its `volume` and `text`. */
async function runBoxScript(source) {
  await nextTask()
  const [milliseconds, coreMilliseconds, volume, text] = callPython('run_box_script', source)
  return { milliseconds, coreMilliseconds, volume, text }
}

/** One sample of row 2's operation `name` ("box", "top" or "volume"), in milliseconds. */
async function overhead(name) {
  await nextTask()
  return callPython('overhead', name)
}

self.pyodideSpike = {
  load,
  runBoxScript,
  overhead,
  /** The byte lengths of Pyodide's WebAssembly memory and of the core instance's. */
  memory: () => ({
    // Pyodide's Emscripten module is private; ADR 0070 reads its memory there.
    // oxlint-disable-next-line no-underscore-dangle
    pyodideBytes: pyodide._module.HEAPU8.byteLength,
    coreBytes: coreModule.HEAPU8.byteLength,
  }),
  violations: () => [...violations],
  crossOriginIsolated: () => self.crossOriginIsolated,
}
self.postMessage('ready', [])
```

**`ui/e2e/pyodide-spike.spec.ts`**

```ts
/// <reference lib="dom" />
import { expect, type BrowserContext, type Page, type Worker } from '@playwright/test'
import { percentile, reportedViolations, test } from './helpers'
import { expectWithinBudgets, type Measured } from './support/perf'

// The Pyodide spike of ADR 0070 (m2-plan.md choice 25, risk 19). Pyodide runs beside an instance of
// the shipped web core in the module worker of public/pyodide-spike/, served by the preview with
// every header of public/_headers, the content security policy included. Each test reports its
// samples as annotations and holds the values that ADR 0070's criteria bound. Tagged @perf, so it
// runs only by hand: PARTSMITH_PERF=1 pnpm -C ui exec playwright test pyodide-spike --project=chromium,
// after spike/pyodide/prepare.py has put Pyodide's files and partsmith.zip in public/pyodide.

// One test after another in one worker: tests side by side would time each other's load.
test.describe.configure({ mode: 'default' })

/** The spike's page. */
const PAGE_PATH = '/pyodide-spike/index.html'

/** The path of the page's module worker, which runs the core and Pyodide. */
const WORKER_PATH = '/pyodide-spike/worker.mjs'

/** The timed samples of each value, after one untimed run; their median counts (python-api.md 5). */
const SAMPLES = 7

/** The box script of ADR 0070, which the native baseline runs too. */
const BOX_SCRIPT = [
  'from partsmith import Document, Part, box',
  '',
  'document = Document()',
  'with Part("Box") as part:',
  '    box(20, 30, 40)',
  'volume = part.volume()',
  'text = document.to_text()',
  'document.close()',
  '',
].join('\n')

/** The volume of a 20 by 30 by 40 box in cubic millimeters. */
const BOX_VOLUME = 24_000

/** C1: the web's cold start budget in milliseconds (technical design section 14). */
const COLD_LOAD_BUDGET = 4_000

/** C3: 3 times row 2's 2 ms of Python overhead per API call (python-api.md 5). */
const OVERHEAD_BUDGET = 6

/** C5: half of the core's 512 MiB cache budget on wasm32, in bytes. */
const PYODIDE_MEMORY_LIMIT = 256 * 1024 * 1024

/** How long one test may take: every sample loads Pyodide, and a first load fetches 12 MB. */
const TEST_TIMEOUT_MILLISECONDS = 600_000

/** How long the page may take until its worker's core runs. */
const READY_TIMEOUT_MILLISECONDS = 60_000

/** Row 2's three operations (python-api.md 5), by the names the worker's harness gives them. */
const OPERATIONS = ['box', 'top', 'volume'] as const

/** One of row 2's operations. */
type Operation = (typeof OPERATIONS)[number]

/** A resource the worker fetched, and whether it was fetched during the timed load. */
type Resource = {
  readonly path: string
  readonly transferSize: number
  readonly duringLoad: boolean
}

/** What the worker's `load` gives. */
type Load = { readonly milliseconds: number; readonly resources: readonly Resource[] }

/** What the worker's `runBoxScript` gives. */
type BoxScriptRun = {
  readonly milliseconds: number
  readonly coreMilliseconds: number
  readonly volume: number
  readonly text: string
}

/** The functions the worker puts on its global scope (public/pyodide-spike/worker.mjs). */
type PyodideSpike = {
  readonly load: () => Promise<Load>
  readonly runBoxScript: (source: string) => Promise<BoxScriptRun>
  readonly overhead: (operation: Operation) => Promise<number>
  readonly memory: () => { readonly pyodideBytes: number; readonly coreBytes: number }
  readonly violations: () => string[]
  readonly crossOriginIsolated: () => boolean
}

// The global of the spike's worker, and the global of its page (public/pyodide-spike/main.mjs).
declare const self: { readonly pyodideSpike: PyodideSpike }
declare const window: { readonly pyodideSpikeViolations: readonly string[] }

/** Adds every console message of a context, from pages and workers, to `messages` as `{type} {text} ({URL})`. */
function watchConsole(context: BrowserContext, messages: string[]): void {
  context.on('console', (message) => {
    messages.push(`${message.type()} ${message.text()} (${message.location().url})`)
  })
}

/** Opens the spike's page, or reloads it, and returns its worker once the worker's core runs. */
async function openSpike(page: Page, reload: boolean): Promise<Worker> {
  await (reload ? page.reload() : page.goto(PAGE_PATH))
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', {
    timeout: READY_TIMEOUT_MILLISECONDS,
  })
  const workers = page.workers().filter((worker) => new URL(worker.url()).pathname === WORKER_PATH)
  expect(workers).toHaveLength(1)
  const [worker] = workers
  if (worker === undefined) {
    throw new Error('The spike page has no worker.')
  }
  return worker
}

/** Loads Pyodide and `partsmith` in the worker. */
function load(worker: Worker): Promise<Load> {
  return worker.evaluate(() => self.pyodideSpike.load())
}

/**
 * Every content security policy violation of the page and its worker since they started: the
 * `securitypolicyviolation` events their scripts keep and the reports of `reportedViolations`.
 */
async function violations(page: Page, worker: Worker): Promise<string[]> {
  return [
    ...(await page.evaluate(() => window.pyodideSpikeViolations)).map((line) => `page ${line}`),
    ...(await page.evaluate(reportedViolations)).map((line) => `page report ${line}`),
    ...(await worker.evaluate(() => self.pyodideSpike.violations())).map(
      (line) => `worker ${line}`,
    ),
    ...(await worker.evaluate(reportedViolations)).map((line) => `worker report ${line}`),
  ]
}

/** Takes one untimed sample, then `SAMPLES` timed ones, one after another, and returns the timed ones. */
async function samples<Sample>(take: () => Promise<Sample>): Promise<Sample[]> {
  await take()
  const taken: Sample[] = []
  for (let i = 0; i < SAMPLES; i++) {
    // One sample at a time: samples side by side would time each other.
    // oxlint-disable-next-line no-await-in-loop
    taken.push(await take())
  }
  return taken
}

/** The median of `values`, the 4th smallest of 7 (python-api.md 5). */
function median(values: readonly number[]): number {
  return percentile(values, 0.5)
}

/** The sum of the bytes `resources` transferred. */
function transferred(resources: readonly Resource[]): number {
  return resources.reduce((total, resource) => total + resource.transferSize, 0)
}

/** Adds an annotation of type `type` that reads `{name}: {value}`; values are written as they are. */
function record(type: 'samples' | 'value', name: string, value: unknown): void {
  test.info().annotations.push({ type, description: `${name}: ${JSON.stringify(value)}` })
}

/**
 * Records the samples of a load named `name`: their times, the bytes transferred during each load
 * and by every resource of the worker, the core's included, and the last sample's resources.
 * Returns the median time.
 */
function recordLoads(name: string, loads: readonly Load[]): number {
  const milliseconds = median(loads.map((sample) => sample.milliseconds))
  record(
    'samples',
    `${name}, ms`,
    loads.map((sample) => sample.milliseconds),
  )
  record('value', `${name}, median ms`, milliseconds)
  record(
    'samples',
    `${name}, bytes transferred during the load`,
    loads.map((sample) => transferred(sample.resources.filter((resource) => resource.duringLoad))),
  )
  record(
    'samples',
    `${name}, bytes transferred by every resource of the worker`,
    loads.map((sample) => transferred(sample.resources)),
  )
  record('value', `${name}, last sample's resources`, loads.at(-1)?.resources)
  return milliseconds
}

/**
 * Takes the cold load's samples in `page`, whose context keeps its HTTP cache across reloads: one
 * open fills the cache, then each sample reloads the page and loads Pyodide in the new worker. Every
 * page load's violations go to `seen`. Returns the median time and the last worker, with Pyodide
 * loaded.
 */
async function coldLoad(
  page: Page,
  seen: string[],
): Promise<{ readonly milliseconds: number; readonly worker: Worker }> {
  let worker = await openSpike(page, false)
  await load(worker)
  const loads = await samples(async () => {
    seen.push(...(await violations(page, worker)))
    worker = await openSpike(page, true)
    return load(worker)
  })
  return { milliseconds: recordLoads('cold load', loads), worker }
}

test.describe('Pyodide spike (ADR 0070)', () => {
  test('first load in a new context with an empty HTTP cache @perf', async ({
    browser,
    baseURL,
  }) => {
    test.setTimeout(TEST_TIMEOUT_MILLISECONDS)
    const seen: string[] = []
    const messages: string[] = []
    const loads = await samples(async () => {
      const context = await browser.newContext({ baseURL })
      try {
        watchConsole(context, messages)
        const page = await context.newPage()
        const worker = await openSpike(page, false)
        const sample = await load(worker)
        seen.push(...(await violations(page, worker)))
        return sample
      } finally {
        await context.close()
      }
    })

    recordLoads('first load', loads)
    record('value', 'browser.version()', browser.version())
    record('value', 'console messages', messages)
    expect(seen, 'the content security policy violations').toEqual([])
  })

  test('cold load, box script, Python overhead, memory and policy, with every file in the HTTP cache @perf', async ({
    playwright,
    launchOptions,
    baseURL,
  }) => {
    test.setTimeout(TEST_TIMEOUT_MILLISECONDS)
    // A context with a profile on disk: Playwright's default context keeps its HTTP cache in memory,
    // where neither pyodide.asm.wasm nor the core's module stays cached across a reload.
    const context = await playwright.chromium.launchPersistentContext(
      test.info().outputPath('profile'),
      { ...launchOptions, baseURL },
    )
    try {
      const messages: string[] = []
      watchConsole(context, messages)
      const page = context.pages()[0] ?? (await context.newPage())
      const seen: string[] = []
      const { milliseconds, worker } = await coldLoad(page, seen)
      expectWithinBudgets('C1', [
        { statistic: 'cold load', value: milliseconds, budget: COLD_LOAD_BUDGET },
      ])

      const runs = await samples(() =>
        worker.evaluate((source) => self.pyodideSpike.runBoxScript(source), BOX_SCRIPT),
      )
      record(
        'samples',
        'box script, ms',
        runs.map((run) => run.milliseconds),
      )
      record(
        'samples',
        'box script, ms inside callCore',
        runs.map((run) => run.coreMilliseconds),
      )
      record('value', 'box script, median ms', median(runs.map((run) => run.milliseconds)))
      record(
        'value',
        'box script, median ms inside callCore',
        median(runs.map((run) => run.coreMilliseconds)),
      )
      record(
        'samples',
        'box script, volume',
        runs.map((run) => run.volume),
      )
      const [firstRun] = runs
      if (firstRun === undefined) {
        throw new Error('The box script ran no sample.')
      }
      await test
        .info()
        .attach('box script text', { body: firstRun.text, contentType: 'text/plain' })
      for (const run of runs) {
        expect(Math.abs(run.volume - BOX_VOLUME) / BOX_VOLUME).toBeLessThanOrEqual(1e-9)
        expect(run.text).toBe(firstRun.text)
      }

      const memory = await worker.evaluate(() => self.pyodideSpike.memory())
      const pageMemory = await page.evaluate(async () => {
        const measured = await (
          performance as unknown as {
            measureUserAgentSpecificMemory: () => Promise<{ readonly bytes: number }>
          }
        ).measureUserAgentSpecificMemory()
        return measured.bytes
      })
      record('value', "Pyodide's WebAssembly memory, bytes", memory.pyodideBytes)
      record('value', "the core instance's WebAssembly memory, bytes", memory.coreBytes)
      record('value', "the page's total from measureUserAgentSpecificMemory, bytes", pageMemory)
      expect
        .soft(memory.pyodideBytes, "C5 Pyodide's WebAssembly memory")
        .toBeLessThanOrEqual(PYODIDE_MEMORY_LIMIT)

      const overheads: Measured[] = []
      for (const operation of OPERATIONS) {
        // One operation after another, as row 2 takes them.
        // oxlint-disable-next-line no-await-in-loop
        const taken = await samples(() =>
          worker.evaluate((name) => self.pyodideSpike.overhead(name), operation),
        )
        record('samples', `Python overhead of ${operation}, ms`, taken)
        overheads.push({
          statistic: `Python overhead of ${operation}`,
          value: median(taken),
          budget: OVERHEAD_BUDGET,
        })
      }
      expectWithinBudgets('C3', overheads)

      const isolated = await worker.evaluate(() => self.pyodideSpike.crossOriginIsolated())
      seen.push(...(await violations(page, worker)))
      record('value', 'crossOriginIsolated in the worker', isolated)
      record('value', 'content security policy violations', seen)
      record('value', 'console messages', messages)
      expect.soft(isolated, 'C4 crossOriginIsolated in the worker').toBe(true)
      expect.soft(seen, 'C4 the content security policy violations').toEqual([])
    } finally {
      await context.close()
    }
  })

  test("cold load in Playwright's default context, which reloads as P-1 does @perf", async ({
    page,
  }) => {
    test.setTimeout(TEST_TIMEOUT_MILLISECONDS)
    const messages: string[] = []
    watchConsole(page.context(), messages)
    const seen: string[] = []
    const { worker } = await coldLoad(page, seen)
    seen.push(...(await violations(page, worker)))
    record('value', 'content security policy violations', seen)
    record('value', 'console messages', messages)
    expect(seen, 'the content security policy violations').toEqual([])
  })

  test('control: the worker reports a string evaluated in a new task, which the policy refuses @perf', async ({
    page,
  }) => {
    // DevTools lets the code that Worker.evaluate runs, and its microtasks, evaluate strings, so the
    // worker runs every measured call in a new task. This evaluates a string in a new task, as
    // Pyodide's run_js would, and expects the worker's events and reports to hold that refusal alone.
    const worker = await openSpike(page, false)
    const outcome = await worker.evaluate(
      () =>
        new Promise<string>((resolve) => {
          setTimeout(() => {
            try {
              // On purpose, so the policy has something to refuse.
              // oxlint-disable-next-line no-eval
              globalThis.eval('1 + 1')
              resolve('evaluated')
            } catch (error) {
              resolve(error instanceof Error ? error.name : String(error))
            }
          }, 0)
        }),
    )
    expect(outcome).toBe('EvalError')
    await expect
      .poll(() => violations(page, worker))
      .toEqual(['worker script-src eval', 'worker report script-src eval'])
  })
})
```

**The native harnesses.** They are not on the branch. They ran with `.venv/bin/python -I` in the `.venv` that python-api.md's commands make at the repository root, beside the release build of `partsmith-py` that `maturin develop --release` installs.

**`native_box.py`**, the box script's baseline. Its argument is the file it writes the text to.

```python
"""The native baseline of ADR 0070's box script: the release build of partsmith-py, one untimed run, then 7 timed runs."""

import ast
import json
import platform
import sys
import time

import partsmith

BOX_SCRIPT = """\
from partsmith import Document, Part, box

document = Document()
with Part("Box") as part:
    box(20, 30, 40)
volume = part.volume()
text = document.to_text()
document.close()
"""

RUNS = 7


def milliseconds(operation):
    started = time.perf_counter()
    operation()
    return (time.perf_counter() - started) * 1000


def run_box_script(source):
    """Runs the box script once: its time from Document() to the return of document.close(), and its volume and
    text. Its first statement, the import, runs untimed (the web harness of worker.mjs does the same)."""
    statements = ast.parse(source).body
    imports = compile(ast.Module(statements[:1], []), "<box script>", "exec")
    rest = compile(ast.Module(statements[1:], []), "<box script>", "exec")
    namespace = {}
    exec(imports, namespace)
    elapsed = milliseconds(lambda: exec(rest, namespace))
    return elapsed, namespace["volume"], namespace["text"]


run_box_script(BOX_SCRIPT)
runs = [run_box_script(BOX_SCRIPT) for _ in range(RUNS)]
times = [run[0] for run in runs]
texts = {run[2] for run in runs}
assert len(texts) == 1
(text,) = texts
with open(sys.argv[1], "w", encoding="utf-8", newline="") as output:
    output.write(text)
print(
    json.dumps(
        {
            "python": platform.python_version(),
            "partsmith_file": partsmith.__file__,
            "core_file": partsmith._core.__file__,
            "samples_ms": times,
            "median_ms": sorted(times)[len(times) // 2],
            "volumes": [run[1] for run in runs],
            "volume_hex": [float.hex(run[1]) for run in runs],
        },
        indent=1,
    )
)
```

**`native_row2_each.py`**, row 2 with each operation's median.

```python
"""Row 2 of python/tests/test_budgets.py natively, with each operation's median printed (the test prints only the largest)."""

import json
import time

import partsmith._core
from partsmith import Document, Part, box, faces

ORIGINAL_SESSION = partsmith._core.Session
RUNS = 7


class TimedSession:
    def __init__(self, **options):
        self._session = ORIGINAL_SESSION(**options)
        self.engine_milliseconds = 0.0

    def call(self, frame):
        started = time.perf_counter()
        try:
            return self._session.call(frame)
        finally:
            self.engine_milliseconds += (time.perf_counter() - started) * 1000


def milliseconds(operation):
    started = time.perf_counter()
    operation()
    return (time.perf_counter() - started) * 1000


def overhead(operation, with_box):
    def measure():
        sessions = []

        def wrapped(**options):
            session = TimedSession(**options)
            sessions.append(session)
            return session

        partsmith._core.Session = wrapped
        try:
            document = Document()
        finally:
            partsmith._core.Session = ORIGINAL_SESSION
        [session] = sessions
        try:
            with Part("P") as part:
                if with_box:
                    box(20, 30, 40)
                session.engine_milliseconds = 0.0
                elapsed = milliseconds(lambda: operation(part))
            return elapsed - session.engine_milliseconds
        finally:
            document.close()

    return measure


def samples(measure):
    measure()
    return [measure() for _ in range(RUNS)]


result = {}
for name, operation, with_box in [
    ("box", lambda part: box(20, 30, 40), False),
    ("top", lambda part: faces().top(), True),
    ("volume", lambda part: part.volume(), True),
]:
    taken = samples(overhead(operation, with_box))
    result[name] = {"samples_ms": taken, "median_ms": sorted(taken)[len(taken) // 2]}
print(json.dumps(result, indent=1))
```

## Alternatives considered

The plan settled the form of the spike (choice 25). Its options: (a) a throwaway branch, with the results recorded in an ADR and the measurements reproducible from it; (b) the prototype merged behind a flag. It picked (a), the safest: `main` stays production-grade, with no temporary workaround left behind (AGENTS.md principle 1). This record does not reopen it.

The choices below are each Accepted (owner's rule of 2026-11-05): the most open-source approach, then the safest, then the industry standard, then this work's recommendation.

1. *The route.* (i) Pyodide in the same worker as the web core, with synchronous calls through a JavaScript module; (ii) Pyodide in a worker of its own, with synchronous calls to the core's worker through a SharedArrayBuffer and `Atomics.wait`; (iii) `partsmith-py` built as a Pyodide package, a PyO3 extension for Pyodide's Emscripten 5.0.3 with its own copy of the core and OCCT. All three are equally open. The rule takes (i), the safest. It uses the one core binary the app ships and changes no crate. Under (ii), every frame crosses a second worker. A protocol over shared memory would carry it. The spike would have to build that protocol first. Under (iii), the core and OCCT are built a second time with an Emscripten other than 4.0.15. The web build then serves a second core of about 19 MiB (19,710,890 bytes on 2026-10-23, delivery.md section 7 open issue 2). (i) is also the industry standard: `registerJsModule` and the `js` module are Pyodide's documented way to give Python a JavaScript API. M4's sandbox ADR may still move untrusted scripts into a worker of their own, route (ii). The Python side of this spike's numbers holds for that route too.
2. *The Pyodide release.* (i) 314.0.7, the newest release; (ii) 0.29.5, the newest of the 0.29 line; (iii) 0.27.8. All three are maintained and equally open. Safety does not separate them. The rule takes (i), the industry standard: the current line gets new fixes first, and v1.0 would start on it.
3. *Where the branch lives.* (i) Pushed to the repository, never a pull request, deleted after the gate; (ii) only on the machine that runs the spike. The rule takes (i), the most open: reviewers at the gate can read the code the numbers came from. The result section keeps that code after the branch is gone. A push of a branch starts no workflow and deploys nothing. Amended on 2026-11-16 by this ADR's note of that date. The note takes its option (iii), the branch's diff committed as a patch.
4. *Where it is measured.* (i) Playwright's Chromium, headless, on the 4-core development container, where ADR 0057 measures the desktop rows; (ii) Chrome on the reference laptop. The rule takes (i), this work's recommendation: the agents of wave 1 can run it, and the spike sets no budget row. The reference laptop runs gate 20's web rows by hand. When M4 builds web scripting, its budget rows follow ADR 0057's method on the machines that rule names.
5. *The bounds of the criteria.* For the cold load: (i) the web's cold start budget, 4 s; (ii) the web's warm open budget, 1 s; (iii) no bound. The rule takes (i), the industry standard of this repository: loading Pyodide starts a runtime cold, and ui.md section 5 measures cold start the same way. (ii) bounds opening a file in a running app. For the overhead per call, the rule takes 3 times row 2, as every web column scales the desktop's. For memory, no budget covers a second runtime. The rule takes this work's recommendation, half the core's cache budget on wasm32. The bound keeps the second runtime at half of what the core's own caches may hold on the web. M4 sets the budget row from the measured value.
6. *The decision rule.* (i) Every criterion passes for v1.0, else v1.1; (ii) a weighted score over the criteria; (iii) the owner decides at the gate. The rule takes (i), the safest: the outcome follows from the record alone, and an unknown counts as a failure. (ii) needs weights nobody has set. (iii) asks the owner. The standing rule of 2026-11-05 replaced such questions.

## Consequences

- No code merges, and `main` does not change. The spike fills the result section and sets the status by the decision rule. A records step writes the ADR log row.
- Web scripting in v1.0: M4's code workflow adds Pyodide 314.0.7 or a later release on the route measured here. It adds the per-document session options of question 8, Pyodide's entries in the notices and the dependency source archives, and its sandbox ADR.
- Web scripting in v1.1: M4's code workflow keeps CPython on the desktop and the `partsmith` package. The web app's answer for a document with a script feature comes in M4's spec. Either answer changes only M4's plan (m2-plan.md risk 19).
- The gate reviews Pyodide with this record either way.
- python-api.md section 1 and protocol.md section 1 name this record where they defer Pyodide, by handoff from this step.

## Note (2026-11-16): the spike's code kept as a patch file

Choice 3 had `spike/pyodide` pushed to the repository and kept until the gate's review. The session that runs wave 1 may push only its own branch. The spike's branch therefore stays in the development container, unpushed, like the six kernel spike branches ([ADR 0071](0071-kernel-spikes-and-facade-calls-that-poll-an-interrupt.md)'s note of 2026-11-16). This note settles where the spike's code is kept. It is Accepted (owner's rule of 2026-11-05). It replaces choice 3's push and amends alternative 3. It changes no file format, protocol schema, naming algorithm, license or dependency.

*Where the spike's code is kept.* Options: (i) the branch pushed to the repository and deleted after the gate's review, choice 3 as written; (ii) the branch kept only in the development container; (iii) the branch kept in the development container, and its diff against its base commit committed on the wave branch as `docs/design/spikes/pyodide.patch` with that base commit. The rule takes (iii). It is the most open. Like (i), it publishes every line of the code behind every number. Unlike (i), it stays in the repository after the gate. Under (ii) only the container holds the code. The code goes with the container. (iii) is also the safest. The wave's own session can make it. The gate's review reads the patch with this record. (i) needs a push that session may not make. (iii) is the industry standard too: a diff against a named base commit is git's own exchange format. `git apply` rebuilds the tree from it.

*What stays of choice 3.* The branch is never opened as a pull request and never merged. It stays local to the development container. Nothing keeps it until the gate: it goes with the container. Its diff against `d978be2` is committed as [`docs/design/spikes/pyodide.patch`](../design/spikes/pyodide.patch). The patch's first lines name the spike, this ADR, the branch, its last commit `08f36b7` and the base commit `d978be2`. They say how to apply it: check out `d978be2` and run `git apply` with the file at the root of the repository. `git apply` skips those lines. On 2026-11-16 the patch passed `git apply --check` on `d978be2`. Applied there, it gave `08f36b7`'s tree exactly. The patch holds no file of Pyodide. `spike/pyodide/prepare.py` copies them from `ui/node_modules/pyodide/` after `pnpm -C ui install`. [`docs/design/spikes/pyodide.json`](../design/spikes/pyodide.json) beside the patch holds every sample. The Result keeps the full text of the five files that choice 3 names, as choice 3 asks. Every number, and the code that measured it, still lives in this record.
