# 0076: OCCT built for the web at -Oz

Status: Accepted (owner's rule of 2026-11-05); needs human review at the M2 gate (m2-plan.md gates 16 and 19). This ADR builds OCCT for the web at `-Oz`, the core size spike's V3. In the spike the core was 4,266,769 bytes (21.3 percent) smaller, its answers were byte-identical, and its code was 12.7 to 18.7 percent slower under Node. It takes step 1 of delivery.md section 3.5's plan, and every step of that plan needs human review. It changes [ADR 0003](0003-pin-occt-and-build-from-source.md)'s build flags for the WebAssembly OCCT build and sets when the next size ADR is due (decision 6). It changes no file format, protocol schema, naming algorithm, license or dependency.

## Context

- **The trigger.** delivery.md section 3.5 holds every served file to Cloudflare Pages' 25 MiB (26,214,400 bytes). Its rule for the six kernel spikes of M2 wave 1: when their size deltas together would leave the web core within 2 MiB of the limit, the section's plan starts with its ADR before wave 2 (m2-plan.md risk 8). The six deltas add up to 4,216,043 bytes. On the spikes' base they give a core of 24,213,008 bytes, within the 2 MiB margin by 95,760 bytes. The sum overstates what M2 links: each branch keeps constructions that no feature takes, and code that several branches share counts more than once. With the two constructions the spikes' decisions take, each built alone, and the other four whole branches, the core would be 22,607,991 bytes, outside the margin. Section 3.5 asks this ADR to weigh both figures.
- **The plan.** Section 3.5's plan has three steps, each an ADR with human review. (1) Size options within one module: `-Oz` for Rust and for the OCCT build, Binaryen `-Oz` with `--converge`, and linking only the OCCT toolkits the engine calls. (2) Lazily loaded modules: a base kernel module, booleans and fillets, STEP I/O and mesh tools, each loaded by the worker on first use. (3) Cloudflare R2 for any file still over the limit, with a custom domain, an R2 API token as a new secret and cost approval.
- **The spike.** On 2026-11-19 the core size spike measured step 1's options. Branch `spike/core-size` (local), last commit `79dfbac`, base `48832d4`. Machine: the 4-core development container (Intel Xeon at 2.10 GHz, 15 GB RAM, Linux 6.18.44), with Rust 1.97.0, Emscripten 4.0.15 and its clang 22.0.0git, Binaryen `wasm-opt` version 124, Node 22.23.4 for the tests and timings, Chrome for Testing 153.0.8010.12 with Playwright 1.64.0, and OCCT 7.9.3 with `scripts/occt-7.9.3.patch`. It built five variants with `scripts/build-wasm.sh`, V2 by `wasm-opt` on V0's file, and four analysis builds, compared every variant's answers with V0's, timed each under Node and timed V0 against V4 in Chromium. The branch's one file is the harness `spike/core-size/harness.mjs`, and its diff against the base (`git diff 48832d4 79dfbac`) is kept as [`docs/design/spikes/core-size.patch`](../design/spikes/core-size.patch). Every sample, size and SHA-256 is in [`docs/design/spikes/core-size.json`](../design/spikes/core-size.json), where `<scratch>` stands for the spike's scratch folder in the container. Each variant's change is a diff or a command, not a commit:

| Variant | Change from V0 |
| --- | --- |
| V0 | `scripts/build-wasm.sh` as is at `48832d4`: Rust at opt-level `"s"`, OCCT at `-Os` |
| V1 | `opt-level = "z"` in `[profile.wasm-release]` |
| V2 | V0's `.wasm` through `wasm-opt -Oz --converge` with the features emcc gives its own wasm-opt calls (2 min 29 s) |
| V3 | OCCT built by a copy of `scripts/build-occt.sh` whose two release flags read `-Oz -DNDEBUG` (20 min 0 s), then V0's build against it |
| V4 | V1 and V3 together, then V2's pass |
| V5 | V4 with OCCT at `-Oz -flto -DNDEBUG` (archives of LLVM bitcode, LTO at the link) |
| V1-link-Os (analysis) | V1 with emcc's link at `-Os` (`-C link-arg=-Os`): Rust and the shim at `z` |
| V0-22-toolkits (analysis) | V0 without the six toolkits that give no code |
| V0-profiling, V4-profiling (analysis) | V0's and V4's configurations linked with `--profiling-funcs` and a wasm-ld map, for the attribution |

- **What emcc receives.** rustc passes its optimization level to emcc, and cc-rs compiles the shim at the same level: `-Os` for opt-level `"s"`, `-Oz` for `"z"`. At `-Oz` emcc runs Binaryen at `-Oz` with `--shrink-level=2` and links `libc_optz` before libc. `libc_optz`'s `pow`, `log` and `log2` (musl's small implementations) and its `emscripten_memcpy`, `memset`, `memmove` and `memcmp` then override libc's. emcc at `-Oz` also writes a loader 417 bytes smaller. V3 links as V0 does, with V0's loader (69,156 bytes).
- **Sizes.** As `scripts/build-wasm.sh` computes them (raw, `gzip -9`, Node zlib brotli at maximum quality):

| Variant | Raw (bytes) | Raw change | gzip -9 | gzip change | brotli | brotli change |
| --- | --- | --- | --- | --- | --- | --- |
| V0 | 19,996,965 | | 7,031,373 | | 5,040,947 | |
| V1 | 18,820,139 | -1,176,826 (-5.89 percent) | 6,781,201 | -250,172 (-3.56 percent) | 4,902,899 | -138,048 (-2.74 percent) |
| V2 | 19,575,809 | -421,156 (-2.11 percent) | 6,971,718 | -59,655 (-0.85 percent) | 5,020,424 | -20,523 (-0.41 percent) |
| V3 | 15,730,196 | -4,266,769 (-21.34 percent) | 6,184,552 | -846,821 (-12.04 percent) | 4,523,647 | -517,300 (-10.26 percent) |
| V4 | 14,573,086 | -5,423,879 (-27.12 percent) | 5,902,641 | -1,128,732 (-16.05 percent) | 4,359,168 | -681,779 (-13.52 percent) |
| V5 | 14,887,107 | -5,109,858 (-25.55 percent) | 5,971,003 | -1,060,370 (-15.08 percent) | 4,443,827 | -597,120 (-11.85 percent) |
| V1-link-Os | 19,241,971 | -754,994 (-3.78 percent) | 6,828,523 | | 4,915,178 | |
| V0-22-toolkits | 19,996,965 | 0, byte-identical to V0 | | | | |

- **Answers.** For every example (294 documents, the benchmark among them) the harness sends `open_document` with its assets, `measure` and `list_elements` of each body, `analyze_print`, a binary STL `export` and `close_document`. That gives 1,882 response frames (60,536,518 bytes), compared frame by frame with V0's. `web_module` runs with `PARTSMITH_REQUIRE_WEB_MODULE=1`, and `web_parity` is engine.md test 154 (`--ignored`). The kernel tests ran under Node against the two new OCCT builds. Their 11 ignored tests are the threaded ones.

| Variant | Frames equal to V0's | `web_module` | `web_parity` | Kernel tests under Node |
| --- | --- | --- | --- | --- |
| V0 | 1,882 (a second V0 run) | 5 passed | passed | |
| V1 | 1,878 | 5 passed | passed | |
| V2 | 1,882 | 5 passed | passed | |
| V3 | 1,882 | 5 passed | passed | OCCT at `-Oz`: 196 unit tests passed; 541 integration tests passed, 11 ignored, 0 failed |
| V4 | 1,878 | 5 passed | passed | same OCCT as V3 |
| V5 | 1,878 | 5 passed | passed | OCCT at `-Oz -flto`: the same counts |
| V1-link-Os | 1,882 | | | |

V1, V4 and V5 answer alike. Their 4 frames that differ from V0's are all of `examples/fillet/top-loop.ptsm`: its `measure` (2 numbers, at most 3.8e-16 apart), its `list_elements` (8 numbers, at most 1.4e-14 apart), its `open_document` (one face triangulated differently, with the same 416 vertices and 396 triangles) and its STL export (19,884 bytes differ, same length). engine.md 3.18's tolerances hold. The cause is emcc's link at `-Oz`: V1-link-Os and V2 answer as V0 does. The likely source is `libc_optz`'s `pow`, `log` and `log2`. No build isolated it.
- **Speed under Node.** Four workloads: (a) instantiate, `createPartsmithCore({ wasmBinary })` until it resolves; (b) cold open, `open_document` of `examples/bench/fifty-features/model.ptsm` as an instance's first request; (c) full rebuild, the same open in a second instance in the same process (the protocol has no empty-cache rebuild, and V8 reuses the compiled code); (d) feature preview, in that second instance `begin_transaction` and `apply_edits` of `lid_width` from 24 mm to 24.5 mm (technical design section 14). The spike ran four sets in two passes, and the second pass's sets 3 and 4 are the reported ones. Each set is one untimed round, then 7 rounds of one new Node process per variant. Set 3 ran V0 to V5 in each round, set 4 V5 to V0. Medians of their 14 samples together (harness `79dfbac`), with the change from V0's:

| Variant | instantiate (ms) | cold open (ms) | full rebuild (ms) | feature preview (ms) |
| --- | --- | --- | --- | --- |
| V0 | 88.4 | 3,909.9 | 3,358.9 | 88.5 |
| V1 | 88.0 (-0.5 percent) | 4,337.8 (+10.9 percent) | 3,947.9 (+17.5 percent) | 100.6 (+13.7 percent) |
| V2 | 86.5 (-2.2 percent) | 3,981.1 (+1.8 percent) | 3,566.2 (+6.2 percent) | 91.0 (+2.8 percent) |
| V3 | 81.5 (-7.9 percent) | 4,405.9 (+12.7 percent) | 3,987.1 (+18.7 percent) | 103.3 (+16.7 percent) |
| V4 | 74.3 (-15.9 percent) | 4,880.2 (+24.8 percent) | 4,604.1 (+37.1 percent) | 112.8 (+27.5 percent) |
| V5 | 76.7 (-13.2 percent) | 4,680.1 (+19.7 percent) | 4,266.2 (+27.0 percent) | 106.5 (+20.4 percent) |

The first pass's sets 1 and 2 (harness `bc67bd5`, whose timing code differs only in one field's name) agree on the order of the costs of cold open, full rebuild and feature preview (core-size.json).
- **P-1 in Chromium.** ui.md 6.7's P-1 (a reload until `ps:usable`, budget 4,000 ms) ran V0 against V4, the smallest variant, in one untimed round and 7 alternating rounds, from builds whose files' SHA-256 values were recorded. V0's median was 504.13 ms (468.81 to 548.77), V4's 649.95 ms (603.96 to 720.21): 145.82 ms (28.9 percent) slower. The first pass's set read 540.79 against 706.18 ms (30.6 percent). P-1 did not time V3 (test 155 timed this change's core in Chromium, Consequences).
- **Spike details.** How the savings split, where V0's bytes are, the toolkit list, the step 2 estimate and the reproducibility of the build are in Spike details below.

## Decision

1. **OCCT is built for the web at `-Oz`.** `scripts/build-occt.sh`'s `wasm` case compiles OCCT's C and C++ with `-DCMAKE_CXX_FLAGS_RELEASE="-Oz -DNDEBUG"` and `-DCMAKE_C_FLAGS_RELEASE="-Oz -DNDEBUG"` in place of `-Os -DNDEBUG`. This is the spike's V3. The case's other flags stay (`-fwasm-exceptions -UOCC_CONVERT_SIGNALS`, ADR 0003). The native build is unchanged.
2. **The rest of the build stays.** Rust stays at opt-level `"s"` in `[profile.wasm-release]`. The shim and emcc's link then stay at `-Os`, and no `libc_optz` is linked. No `wasm-opt` pass is added. The toolkit list stays: V0-22-toolkits showed that fewer toolkits give a byte-identical core.
3. **Steps 2 and 3 of delivery.md 3.5's plan do not start.**
4. **Why V3, under the owner's rule.** Every option of step 1 is built with the same open-source tools, and the first criterion, the most open source, picks none of them. It rules out step 3 for now: R2 would put the core on a hosted service account that the owner must create and pay for. V3 is the safest option that saves size. Its answers are byte-identical to V0's in all 1,882 frames over 294 documents, and every kernel test passes under Node (196 unit and 541 integration tests, the 11 threaded ones ignored). It adds no tool, build step or dependency, leaves the desktop untouched, and two flags reverse it. V2 also keeps the answers, but it adds a build step and saves a tenth as much. V1, V4 and V5 change 4 frames. V3 is also the largest single saving, 4,266,769 bytes (21.3 percent). Its cost is speed: under Node (sets 3 and 4 pooled) its code is 12.7 to 18.7 percent slower, and in Chromium every row of engine.md test 155 stayed within its web budget in four runs off the reference machine (Consequences). On the industry standard: `-Oz` is the level emcc and clang document as reducing code size beyond `-Os`, at some cost in speed. It does not separate V3 from V1. The safest criterion already decides between them. This work recommends adopting it now, on either figure of the Context. By the six spikes' sum the core would be within the 2 MiB margin by 95,760 bytes. With the decided constructions it would be outside the margin by 1,509,257 bytes, room that M2's unmeasured features (sweep, coil, the emboss's font reader and built-in font, conic and B-spline sketch entities) and M3 could use up. The web numbers that waves 2 to 4 record are then taken with the flags that ship (web budgets are 3 times the desktop's, ADR 0057 decision 5).
5. **Levers left for the next size ADR, as measured.** First V2's `wasm-opt -Oz --converge` pass: answers byte-identical, 1.8 to 6.2 percent slower, 421,156 bytes saved on V0 (its saving on V3 was not measured). Then Rust and the shim at opt-level `z` with emcc's link kept at `-Os`: V1-link-Os saved 754,994 bytes on V0 with byte-identical answers, and its speed was not measured. Then step 2. emcc's link at `-Oz` is not on the list: it changes answers in the last bits.
6. **When the next size ADR is due.** The next size ADR is due when a build of `main`, or a kernel PR's recorded size, comes within 2 MiB of the limit (a core above 24,117,248 bytes). delivery.md 3.5 set this margin for the six kernel spikes alone. This decision extends it to every later build of `main` and every kernel PR.

## Alternatives considered

Each choice is Accepted (owner's rule of 2026-11-05): the most open-source approach, then the safest, then the industry standard, then this work's recommendation.

1. *What step 1 takes.* Options: (i) V3, OCCT at `-Oz`; (ii) V1, Rust and the shim at opt-level `z` with emcc's link at `-Oz`; (iii) V2, an extra `wasm-opt -Oz --converge` pass; (iv) V4, V1 and V3 together with V2's pass; (v) V5, V4 with OCCT at `-Oz -flto`; (vi) flags per OCCT toolkit; (vii) no change until the core comes nearer the limit; (viii) Rust and the shim at opt-level `z` with emcc's link at `-Os` (V1-link-Os); (ix) linking only the OCCT toolkits the engine calls. All are open source. The rule takes (i), for the reasons of decision 4. The others, with what the spike measured:
   - (ii) saves 1,176,826 bytes. Its `-Oz` link adds `libc_optz` and changes top-loop's 4 frames in the last bits. Its code is 10.9 to 17.5 percent slower under Node.
   - (iii) keeps the answers byte-identical but saves only 421,156 bytes. It adds a build step of 2 min 29 s, with a feature list copied from emcc's own wasm-opt calls.
   - (iv) is the smallest, 5,423,879 bytes saved. Its code is 24.8 to 37.1 percent slower under Node, P-1 is 28.9 percent slower in Chromium, and it changes the 4 frames.
   - (v) is 314,021 bytes larger than V4, its code is 19.7 to 27.0 percent slower under Node, and it changes the 4 frames.
   - (vi), for example `-Oz` for STEP and `-Os` for booleans, was not measured.
   - (vii) rests on the core with the decided constructions, 22,607,991 bytes, outside the margin by 1,509,257 bytes. M2's unmeasured features and M3 could use up that room and force the change in the middle of a milestone, after the baselines are set.
   - (viii) saved 754,994 bytes with byte-identical answers, but its speed was not measured. Decision 5 keeps it for the next size ADR.
   - (ix) gives a core byte-identical to V0 (V0-22-toolkits): the shim's calls reach every toolkit that gives code.
2. *Steps 2 and 3.* Options: (i) neither starts; (ii) step 2, lazily loaded modules; (iii) step 3, R2. The rule takes (i). (iii) is the least open: it puts the core on a hosted service account, and it needs a domain, an R2 API token as a new secret and cost approval. (ii) is less safe than (i). By the spike's estimate (Spike details) the booleans module would load on nearly every open, and what splitting costs was not measured. It also changes the bridge of protocol.md 2.10 and ADR 0009.

## Consequences

- The web code is slower. In the spike under Node (sets 3 and 4 pooled), V3's cold open took 12.7 percent longer, the full rebuild 18.7 percent and the feature preview 16.7 percent. Instantiation was 7.9 percent faster. In Chromium, engine.md test 155 ran four times on 2026-11-19 in the same 4-core development container, alternating V0 and the main tree's core with this change. Every row wrote its annotation, and every median was within its web budget (engine.md section 5) in all four runs. The cold open (row 3) read about 9 percent slower with this change: 2,485.17 against 2,278.08 ms, the mean of each core's two medians, against a web budget of 3,000 ms. Two runs per core cannot tell the other rows' moves from noise. The runs are off the reference machine and decide no budget. engine.md open issue 19 records every median, each core's SHA-256 and the load before each run.
- The download is smaller: in the spike, 846,821 bytes less with gzip and 517,300 bytes less with brotli.
- The desktop, the CLI and Python are unchanged: the native build's flags stay.
- The main tree's core is 15,720,762 bytes (6,184,342 with gzip, 4,525,181 with brotli), 10,493,638 bytes under the limit. That is 4,276,193 bytes (21.4 percent) less than the main tree's build of `48832d4` at `-Os` (19,996,955 bytes). Its loader is V0's. Its SHA-256, `8c2ef79abaebc8823ba9ef22810bd12a871d626821ba1ce74bb05715e919c941`, identifies that file only. Its answers are byte-identical to V0's in all 1,882 frames over 294 documents, frame by frame (the spike's harness at `79dfbac`; the frames' SHA-256 is V0's, `2cba60666f6ebc735c240a5eda0c4baa113a4274de7429682b6f1fda769022a5`). `web_module` (5 passed) and `web_parity` (engine.md test 154) pass with it. Under Node against the rebuilt install, `partsmith-occt-sys` passes 196 unit and 541 integration tests (11 threaded ones ignored), `partsmith-sketch` 237 and 539 (1 ignored), `partsmith-naming` 315 and 531, and the engine's 11 `dispatch` tests pass.
- A core's size can change with the path of OCCT's work directory. The main tree's core is 9,430 bytes smaller than the same tree linked against the spike's `-Oz` install (15,730,192 bytes, V3's code section exactly). OCCT's source path, which `__FILE__` embeds in 7 strings of the core, is 12 characters shorter in this build's work directory than in the spike's. That changes how emcc's Binaryen merges duplicate functions and inlines a few STEP functions: the code section is 13,201,373 bytes against V3's 13,210,704. The answers do not change. CI builds OCCT in the script's default work directory and checks Partsmith out elsewhere. Its core and a local one can then differ slightly, and sizes are compared only between builds against the same OCCT install (delivery.md open issue 23).
- Both OCCT cache keys, `occt-native-<os>-<hash>` and `occt-wasm-<hash>`, hash `scripts/build-occt.sh`. The jobs that build OCCT on a cache miss (`occt-native`, `occt-wasm`, `occt-platforms` and the release's `bundle`) then build it once more, the native installs included, although the native output does not change. The nightly `fuzz` job restores the native key. A night that runs before CI's first run on `main` fails the row on the cache miss, as after ADR 0073. The OCCT source archive's key does not hash the script, and the archive does not change.
- Each local WebAssembly OCCT install needs a rebuild with the script (here `/opt/occt/7.9.3/wasm`). `partsmith-occt-sys`'s `build.rs` reruns when `OCCT_ROOT_<TARGET>` or `OCCT_ROOT` changes value or the shim's sources change, never when the install's files change. A local build after an in-place OCCT rebuild needs a forced rebuild of that crate (`touch crates/partsmith-occt-sys/build.rs`). CI's rust-cache keeps no workspace crate, and CI links afresh (delivery.md open issue 24).
- Sizes recorded after this ADR are at `-Oz`: `main`'s and each kernel PR's. The kernel spikes' deltas of delivery.md 3.5 were measured at `-Os`.
- The docs that change with it: delivery.md 3.5 (Step 1 taken, and the plan's step 1), open issues 2, 23 and 24, and section 8. m2-plan.md: risk 8 and section 5.3's row for lazily loaded modules and R2. engine.md open issue 19. ADR 0003 gains an amendment row.

## Spike details

- **How the savings split.** OCCT at `-Oz` saves 4,266,769 bytes alone (V3). Rust and the shim at opt-level `z` save 754,994 bytes with V0's answers (V1-link-Os). emcc's link at `-Oz` saves the other 421,832 bytes of V1 and changes top-loop's frames. V2's pass saves 421,156 bytes on V0 but only 14,219 on V4's `-Oz` link and 11,515 on V5's: emcc's own `-Oz` run already does nearly all of it. V1 and V3 together save 5,409,660 bytes, 33,935 less than their sum. V5's LTO adds 314,021 bytes against V4. In the profiling builds (V4-profiling against V0-profiling), V4's configuration shrinks OCCT's code by 4,548,696 bytes (39 percent), the Partsmith crates by 839,925 (17 percent), the shim and cxx glue by 91,279 (30 percent) and std and the other crates by 22,989 (6 percent).
- **Where V0's bytes are.** Its code section holds 17,483,407 bytes (87.4 percent) and its data section 2,434,355 (12.2 percent). It has no custom section to drop. V0-profiling attributes each function body by the input file the wasm-ld map names for it, and the Rust LTO object's functions by their demangled names:

| Origin | Code (bytes) | Share |
| --- | --- | --- |
| OCCT | 11,609,160 | 66.40 percent |
| Rust: Partsmith crates | 5,006,463 | 28.63 percent |
| Rust: std and other crates | 402,809 | 2.30 percent |
| partsmith-occt-sys C++ shim and cxx glue | 299,527 | 1.71 percent |
| Emscripten runtime, libc, libc++ | 165,846 | 0.95 percent |

The largest Partsmith crates are partsmith-features (1,029,088 bytes), partsmith-sketch (913,850), partsmith-protocol (849,841) and partsmith-engine (639,605). The largest OCCT toolkits by code, with their change in V4-profiling (V3's configuration was not attributed):

| Toolkit | Code in V0 (bytes) | Change in V4 (bytes) |
| --- | --- | --- |
| TKDESTEP | 2,397,303 | -1,009,968 (-42 percent) |
| TKGeomAlgo | 1,313,262 | -368,926 (-28 percent) |
| TKFillet | 1,160,756 | -452,145 (-39 percent) |
| TKShHealing | 1,123,385 | -517,534 (-46 percent) |
| TKGeomBase | 1,022,812 | -313,334 (-31 percent) |
| TKBO | 744,931 | -313,129 (-42 percent) |
| TKTopAlgo | 674,965 | -277,391 (-41 percent) |
| TKBool | 635,217 | -308,971 (-49 percent) |
| TKOffset | 626,858 | -338,652 (-54 percent) |

- **Fixed OCCT data.** About 1.0 MiB of the file (1,043,756 bytes) is OCCT tables that no compiler option shrinks: TKGeomBase's approximation matrices (`VBMatrix` of `AppCont_ContMatrices_4.cxx`, 839,994 bytes in the file) and the Unicode code page tables of TKernel's `Resource` package (203,762 bytes in the file, 651,296 before Binaryen, mostly zeros; only STEP packages call them).
- **The toolkit list.** The install and the link list hold 28 toolkits: the 18 of `build-occt.sh`'s `toolkits=` line and 10 that CMake adds. Of these, 22 give code. The other six (TKDESTL, TKFeat, TKHLR, TKV3d, TKVCAF and TKDE) are never extracted from their archives. V0 linked without them is byte-identical to V0. The shim's calls reach the 22, directly or through OCCT's own references, and leaving one out fails the link. Code goes away only when the calls that reach it go away. For example, the shim's STEP calls go through STEPCAFControl (366,558 bytes of code) to keep colors and names in an XCAF document. That document needs TKXCAF, TKLCAF, TKCAF, TKCDF and TKService (277,185 bytes of code). That code goes only if STEP stops keeping colors and names.
- **Step 2, estimated with no build.** The shares of V0's attribution that lazily loaded modules could move out of the first module, each row on top of the rows above it, with no allowance for what splitting costs:

| Module | What it holds | Code + data in V0's file (bytes) | First module left, from V0 (bytes) | First module left, from V4 before wasm-opt (bytes) |
| --- | --- | --- | --- | --- |
| (today) | | | 19,996,965 | 14,587,305 |
| STEP I/O | TKDESTEP, TKXSBase, TKXCAF, TKLCAF, TKCAF, TKCDF, TKService, the shim's `exchange.cpp` | 2,888,329 + 155,630 | 16,953,006 | 12,739,352 |
| booleans and fillets | TKBO, TKBool, TKFillet, TKOffset, the shim's `blends.cpp` and `booleans.cpp` | 3,242,150 + 52,417 | 13,658,439 | 10,882,638 |
| mesh tools | partsmith-print; zip, flate2, zlib-rs, crc32fast | 251,241 + 0 | 13,407,198 | 10,662,124 |
| STEP's Unicode tables too | TKernel's `Resource` tables | 0 + 203,762 | 13,203,436 | 10,458,362 |

- **The estimate's limits.** Booleans run in most documents' first open (an extrude that joins or cuts, holes, patterns). That module would load on nearly every open. Booleans use TKGeomAlgo's intersections (IntPatch 467,598 bytes, GeomInt 189,020), and these stay in the first module: sections and projections use them too. Display meshes need TKMesh and the shim's mesher. The "mesh tools" module holds only print analysis and 3MF export. TKShHealing serves STEP import and the kernel's repairs and stays. The Rust data (198,064 bytes in V0's file) belongs to one LTO object and is not split by crate. Emscripten's dynamic linking (`MAIN_MODULE` and `SIDE_MODULE`) compiles relocatable code with a table of symbols, and code that two modules share stays in the first one or is copied. Binaryen's `wasm-split` splits one linked module without relocations, guided by a profile. Neither was built.
- **Reproducibility.** A rebuild from a recipe above, against the same OCCT install, does not always give the same `.wasm`. The spike rebuilt V0 and V4 twice each, and V1 and V3 once. Every rebuild had the kept file's size and loader, and the V0 and V4 rebuilds also gave the kept files' frames. Every rebuild differed from the kept file in exactly one function of the same length, in the try/delegate nesting of legacy wasm exception handling. In V0 it is function 20308 (3,253 bytes, 19 bytes differ). By its bytes it is the shim's `partsmith::occt::shape_element_display` in `cpp/mesh.cpp` (98.5 percent of the bytes match position by position in V0-profiling). Standalone Binaryen was deterministic: V2 made again from the kept V0 is byte-identical. Which tool varies, clang compiling the shim or emcc's own Binaryen passes, was not isolated. Two links of the main tree against one OCCT install also differed in one function of the same length. The SHA-256 values of the spike identify its kept files only: V0's `.wasm` is `da10b8f2b35924770f3fd3c80be1fa8031d5745acf88e429730fe693b5b2be88` and V3's `8b5c25b09ea0606cff2446c57fb5991b7c91a917caae56199caee29a28f281be` (delivery.md open issue 23).
