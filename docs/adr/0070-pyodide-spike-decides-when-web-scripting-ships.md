# 0070: A Pyodide spike in Chromium decides whether web scripting ships in v1.0 or v1.1

Status: Proposed (the wave 1 spike records its result). It is the M2 plan's choice 25, option (a): the spike runs on a throwaway branch, merges nothing, and records its results here, reproducible from this record. The choices below are Accepted (owner's rule of 2026-11-05). The spike sets the outcome under the same rule. Once it fills the result section, the decision rule below names the release, and this status becomes `Accepted (owner's rule of 2026-11-05): web scripting in v1.0` or `Accepted (owner's rule of 2026-11-05): web scripting in v1.1`. Nothing changes on `main`: no code, schema, protocol, dependency, license or capability. Pyodide 314.0.7 and its MPL-2.0 license go to human review at the M2 gate with this record, as the dependency M4 would add (AGENTS.md Process, m2-plan.md gates 18 and 19), whatever the result.

## Context

- **What the design asks.** Technical design section 10: on the web, Python runs in Pyodide in a worker and calls the WebAssembly core through a JavaScript bridge, with pure-Python packages only. Section 4: Pyodide is loaded lazily. Section 19 lists the interop of Pyodide and the web core as a medium risk. Its mitigation is a prototype in phase 1, and if needed web scripting in v1.1, with code-first files on the desktop only at launch. The plan reads phase 1 as M2 (section 7.2 item 2) and holds the code workflow in M4 (section 5.2). This spike tells M4 which release web scripting is for (item 11, risk 19).
- **The web core.** `partsmith-wasm` builds the Emscripten module `createPartsmithCore` with `-sMODULARIZE=1`, `-sEXPORT_ES6=1`, `-sENVIRONMENT=worker` and `-sMAXIMUM_MEMORY=4GB` (`crates/partsmith-wasm/build.rs`). `scripts/build-wasm.sh` copies it to `ui/public/core/partsmith_core.mjs` and `partsmith_core.wasm`. Its three C ABI exports `ps_alloc`, `ps_free` and `ps_call` trade protocol frames with the module's one `Session`, kept in a `thread_local!` (ADR 0010). That session has display meshes on and random ids (`session_options` in `crates/partsmith-wasm/src/lib.rs`). `callCore` in `ui/src/core/bridge.ts` copies a request frame in, calls `ps_call` and copies the response out. The core is built without shared memory.
- **The Python package needs little from its extension.** `python/partsmith` is pure Python. It imports only the standard library (`dependencies = []` in `python/pyproject.toml`). It reaches the engine only through `partsmith._core`: `_client.py` makes a `Session(display=False, deterministic_ids=True)` for each document and sends frames with `call(frame: bytes) -> bytes`. `__main__.py` reads `ENGINE_VERSION`, `PROTOCOL_VERSION` and `FORMAT_VERSION`. The package makes the ids of its records itself (`Document._new_id` in `document.py`). The engine's id generator (`SessionOptions::id_generator` in `crates/partsmith-engine/src/options.rs`) names the records that `import_step` adds (`crates/partsmith-engine/src/exchange.rs`) and those that `apply_fix` adds, such as a new body or a coincident constraint (`crates/partsmith-engine/src/fixes/`). The package sends `import_step` but never `apply_fix`. M2's item 24 moves Python's `Rectangle` and `Polygon` onto core recipes whose ids come from the same generator.
- **The page's rules.** `ui/public/_headers` sends `Cross-Origin-Opener-Policy: same-origin`, `Cross-Origin-Embedder-Policy: require-corp` and a content security policy whose `script-src` is `'self' 'wasm-unsafe-eval'`, with `connect-src 'self'` and `worker-src 'self'` (ADR 0049). It allows no host and no `'unsafe-eval'`. Every served file stays under 25 MiB (`check:dist`, delivery.md section 3.5). The initial JavaScript stays under 250 KB gzipped (ui.md section 5).
- **Budgets to compare with.** Technical design section 14 and ui.md section 5 give the web 4 s from cold start to usable, measured on a reload with the core module in the HTTP cache. python-api.md section 5 row 2 gives the desktop 2 ms of Python overhead per API call. A web budget is 3 times the desktop's (technical design section 14, ADR 0057 decision 5). The engine's caches may hold 512 MiB on wasm32 (`DEFAULT_CACHE_BUDGET` in `crates/partsmith-engine/src/options.rs`).
- **Pyodide today** (the npm registry, read on 2026-11-07). The newest release is 314.0.7, published on 2026-09-14 under MPL-2.0. Its `pyodide-lock.json` names Python 3.14.2 and the platform `emscripten_5_0_3`. Its runtime files are `pyodide.mjs` (17,931 bytes), `pyodide.asm.mjs` (1,250,344 bytes), `pyodide.asm.wasm` (9,598,218 bytes), `python_stdlib.zip` (2,545,637 bytes) and `pyodide-lock.json` (119,077 bytes). The largest is 9.2 MiB. The npm package depends on `ws` and `@types/emscripten`. The maintenance lines are 0.29.5 and 0.27.8, both published on 2026-09-16. The core and OCCT are built with Emscripten 4.0.15 (`scripts/emsdk-version.sh`).
- **A risk visible before the spike.** A text search of 314.0.7's `pyodide.asm.mjs` finds `eval` in the dynamic linker's paths for the `EM_ASM` and `EM_JS` code of side modules, and in `emscripten_run_script`. The page's policy blocks `eval`. Whether loading the standard library and running a script reaches those paths is question 4.
- **MPL-2.0 in the repository.** `deny.toml`'s list of allowed licenses already holds MPL-2.0 and Python-2.0, and the npm license check reads the same list (delivery.md section 2.10). ADR 0033 lists five crates under Tauri that come under MPL-2.0 on every desktop platform. As a production dependency, Pyodide would pass `check:licenses`: MPL-2.0 is on the list. The spike adds it as a development dependency and copies its files into `ui/public` by hand. Neither `check:licenses` nor the web notices that `pnpm notices` writes see those files: both take the production dependencies and `EMITTED_BUILD_PACKAGES` (`shippedPackages` in `ui/scripts/check-licenses.mjs`). The review the plan asks for is of Pyodide as a new runtime dependency of the web build, and of what MPL-2.0 asks of a web deployment that serves it.

## Decision

**The questions.**

1. Does Pyodide load in a module worker beside the shipped web core, from the app's own origin? How long does it take with an empty HTTP cache, and with its files in the cache?
2. Does the unchanged `partsmith` package run on Pyodide when `partsmith._core` is a pure-Python module that calls `ps_call` synchronously through JavaScript?
3. Does a script that builds a box through the web core give the native result? What does Python add to each API call?
4. Does it all run under the web app's content security policy and cross-origin isolation as they are?
5. How much memory does Pyodide take?
6. Which files would the web build gain? Is each under the 25 MiB limit?
7. Which licenses come with the files served, and what does MPL-2.0 ask of a deployment that serves them?
8. What would M4 need from the core that the shipped core lacks? The spike records findings here and measures nothing for this question. Three are known. A Python document needs a session of its own with display meshes off and `SequentialIds` for the records the engine creates: those of STEP import now, and every rectangle and polygon once item 24 lands. A script feature that runs during a build would need a call from the core into Python through a JavaScript import. Calling an import is `unsafe` code, which ADR 0010 allows only in partsmith-occt-sys and on the three exports. Python on the web is 3.14, beside CPython 3.12 or later on the desktop.

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
4. The gate's review decides on Pyodide as a dependency. If it rejects Pyodide or its license terms, web scripting ships in v1.1 whatever the measurements, and the status says so.

**What the branch does** (choice 3). `spike/pyodide` is pushed to the repository, where reviewers can read the code behind the numbers until the gate. It is never opened as a pull request and never merged. `ci.yml` runs only on pushes to `main` and on pull requests. The branch therefore builds nothing in CI and deploys nothing to Pages. It commits no file of Pyodide: step 2 fetches them. The branch is deleted after the gate's review. Before that, the result section takes the full text of `spike/pyodide/_core.py`, `ui/public/pyodide-spike/index.html`, `ui/public/pyodide-spike/main.mjs`, `ui/public/pyodide-spike/worker.mjs` and `ui/e2e/pyodide-spike.spec.ts`. Every number, and the code that measured it, then lives in this record.

**The license review** (question 7). The result names each file served, the licenses of the code in it, and where its source is: Pyodide's own code under MPL-2.0 (its repository at the tag 314.0.7), CPython and its standard library under the Python license, and the libraries and runtime code compiled into `pyodide.asm.wasm`, listed from Pyodide's build at that tag. It states what MPL-2.0 section 3.2 asks of a web deployment that serves Pyodide's files in executable form: the source code form of those files must be available, and recipients must be told how to get it, at no more than the cost of distribution. For M4 that means Pyodide's source at its tag in the web deployment's dependency source archives (delivery.md section 2.13), and an entry in `ui/public/THIRD-PARTY-NOTICES.txt` (delivery.md section 2.9). M4 makes Pyodide a production dependency, or adds it to `EMITTED_BUILD_PACKAGES`. The license check and the notices then cover the files the web build serves.

## Result

The wave 1 spike fills this section. Until it does, every row reads "not measured". Below the table, the spike adds the full text of the five files that "What the branch does" names.

| Value | Measured | Criterion | Passes |
| --- | --- | --- | --- |
| First load with an empty cache, ms, and bytes transferred | not measured | recorded | |
| Cold load, ms | not measured | C1: at most 4,000 ms | |
| Box script on the web, ms, and its time inside `callCore` | not measured | recorded | |
| Box script on the desktop, ms | not measured | recorded | |
| `volume` on the web and on the desktop, mm³ | not measured | C2 | |
| `text` on the web equal to the desktop's | not measured | C2 | |
| Python overhead per API call (`box`, `faces().top()`, `volume()`), ms | not measured | C3: at most 6 ms | |
| Policy violations, and `crossOriginIsolated` in the worker | not measured | C4: none, true | |
| Pyodide's WebAssembly memory, MiB | not measured | C5: at most 256 MiB | |
| The core instance's memory and the page's total, MiB | not measured | recorded | |
| Largest served file, bytes, and `check:dist` | not measured | C6 | |
| Crate changes | not measured | C7: none | |
| Findings for M4 (question 8) | not measured | recorded | |
| Files served and their licenses (question 7) | not measured | gate review | |

## Alternatives considered

The plan settled the form of the spike (choice 25). Its options: (a) a throwaway branch, with the results recorded in an ADR and the measurements reproducible from it; (b) the prototype merged behind a flag. It picked (a), the safest: `main` stays production-grade, with no temporary workaround left behind (AGENTS.md principle 1). This record does not reopen it.

The choices below are each Accepted (owner's rule of 2026-11-05): the most open-source approach, then the safest, then the industry standard, then this work's recommendation.

1. *The route.* (i) Pyodide in the same worker as the web core, with synchronous calls through a JavaScript module; (ii) Pyodide in a worker of its own, with synchronous calls to the core's worker through a SharedArrayBuffer and `Atomics.wait`; (iii) `partsmith-py` built as a Pyodide package, a PyO3 extension for Pyodide's Emscripten 5.0.3 with its own copy of the core and OCCT. All three are equally open. The rule takes (i), the safest. It uses the one core binary the app ships and changes no crate. Under (ii), every frame crosses a second worker. A protocol over shared memory would carry it, and the spike would have to build that protocol first. Under (iii), the core and OCCT are built a second time with an Emscripten other than 4.0.15, and the web build serves a second core of about 19 MiB (19,710,890 bytes on 2026-10-23, delivery.md section 7 open issue 2). (i) is also the industry standard: `registerJsModule` and the `js` module are Pyodide's documented way to give Python a JavaScript API. M4's sandbox ADR may still move untrusted scripts into a worker of their own, route (ii). The Python side of this spike's numbers holds for that route too.
2. *The Pyodide release.* (i) 314.0.7, the newest release; (ii) 0.29.5, the newest of the 0.29 line; (iii) 0.27.8. All three are maintained and equally open. Safety does not separate them. The rule takes (i), the industry standard: the current line gets new fixes first, and v1.0 would start on it.
3. *Where the branch lives.* (i) Pushed to the repository, never a pull request, deleted after the gate; (ii) only on the machine that runs the spike. The rule takes (i), the most open: reviewers at the gate can read the code the numbers came from. The result section keeps that code after the branch is gone. A push of a branch starts no workflow and deploys nothing.
4. *Where it is measured.* (i) Playwright's Chromium, headless, on the 4-core development container, where ADR 0057 measures the desktop rows; (ii) Chrome on the reference laptop. The rule takes (i), this work's recommendation: the agents of wave 1 can run it, and the spike sets no budget row. The reference laptop runs gate 20's web rows by hand. When M4 builds web scripting, its budget rows follow ADR 0057's method on the machines that rule names.
5. *The bounds of the criteria.* For the cold load: (i) the web's cold start budget, 4 s; (ii) the web's warm open budget, 1 s; (iii) no bound. The rule takes (i), the industry standard of this repository: loading Pyodide starts a runtime cold, and ui.md section 5 measures cold start the same way. (ii) bounds opening a file in a running app. For the overhead per call, the rule takes 3 times row 2, as every web column scales the desktop's. For memory, no budget covers a second runtime. The rule takes this work's recommendation, half the core's cache budget on wasm32. The bound keeps the second runtime at half of what the core's own caches may hold on the web. M4 sets the budget row from the measured value.
6. *The decision rule.* (i) Every criterion passes for v1.0, else v1.1; (ii) a weighted score over the criteria; (iii) the owner decides at the gate. The rule takes (i), the safest: the outcome follows from the record alone, and an unknown counts as a failure. (ii) needs weights nobody has set. (iii) asks the owner. The standing rule of 2026-11-05 replaced such questions.

## Consequences

- No code merges, and `main` does not change. The spike fills the result section and sets the status by the decision rule. A records step writes the ADR log row.
- Web scripting in v1.0: M4's code workflow adds Pyodide 314.0.7 or a later release on the route measured here. It adds the per-document session options of question 8, Pyodide's entries in the notices and the dependency source archives, and its sandbox ADR.
- Web scripting in v1.1: M4's code workflow keeps CPython on the desktop and the `partsmith` package. The web app's answer for a document with a script feature comes in M4's spec. Either answer changes only M4's plan (m2-plan.md risk 19).
- The gate reviews Pyodide with this record either way.
- python-api.md section 1 and protocol.md section 1 name this record where they defer Pyodide, by handoff from this step.
