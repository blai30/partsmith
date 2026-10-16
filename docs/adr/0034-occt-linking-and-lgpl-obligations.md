# 0034: OCCT is linked statically while the project's rules say "a separate library": options for meeting LGPL 2.1 section 6, and a recommendation

Status: Proposed, needs human review (license and linking: the owner chooses an option; nothing changes until then)

## Context

**The contradiction.** Four documents describe how Partsmith uses OCCT, and they disagree:

- AGENTS.md (CLAUDE.md until the owner renamed the instruction files on 2026-10-16), invariants: "License policy: only permissive dependencies, plus OCCT (LGPL, linked as a separate library)."
- Technical design section 3: "OCCT and PlaneGCS stay LGPL as separate libraries; publishing their source and allowing relinking, which an open project does anyway, meets the LGPL." Section 17 adds that cargo-deny allows "the listed LGPL exceptions (OCCT, PlaneGCS)", "protecting the MIT/Apache license".
- README.md on `main` (the sentence that delivery.md 2.12 part 7 replaces with today's static linking): "OCCT is used under the LGPL 2.1 with the OCCT exception, as a separately buildable library."
- [ADR 0003](0003-pin-occt-and-build-from-source.md): OCCT 7.9.3 is built "from source with `scripts/build-occt.sh` for every target: native static libraries with position-independent code, and Emscripten static libraries".

ADR 0003 is what the code does. `partsmith-occt-sys/build.rs` links the OCCT toolkits with `rustc-link-lib=static`, so OCCT is part of every binary that uses the kernel: the desktop app on each platform, the `partsmith` CLI, the Python extension module `partsmith._core` and the web core `partsmith_core.wasm`. PlaneGCS is not used at all ([ADR 0004](0004-native-rust-sketch-solver.md)), and `deny.toml` lists no LGPL exception, because OCCT is not a crate.

**What the license asks.** OCCT is under the GNU LGPL 2.1 with the Open CASCADE exception 1.0 (the texts are in `share/doc/opencascade` of every OCCT install). An executable that contains OCCT falls under LGPL 2.1 section 6. Its terms must allow modification for the user's own use and reverse engineering to debug it (Apache-2.0 does). Each copy needs a prominent notice that OCCT is used and is under the LGPL, with a copy of the license. The OCCT exception lets object code that includes material from OCCT's headers carry any terms, given a prominent notice in the supporting documentation that it uses OCCT. Beyond the notice, section 6 asks for one of:

- (a) the complete source of OCCT as used, and, for an executable linked with it, the "work that uses the Library" as object code or source, so the user can relink a modified OCCT;
- (b) a shared library mechanism that uses a copy already on the user's computer and works with a modified, interface-compatible version;
- (c) a written offer, valid for at least three years, to give the same materials;
- (d) when the work is distributed by offering access from a designated place, equivalent access to those materials from the same place;
- (e) proof that the user already has them.

Distributing OCCT's own object code also falls under section 4: its source must come with it, or be available from the same place. LGPL 2.1, unlike GPLv3, has no provision for pointing at a third party's server. This ADR reads the license as engineers do. It is not legal advice; the questions for a lawyer are listed with the recommendation.

**Facts on 2026-10-16.**

- The GitHub repository `blai30/partsmith` is private. Its release assets and Actions artifacts are visible only to people with access to it; once it is public, anyone can download the assets of its published releases without signing in, and any signed-in GitHub user its Actions artifacts.
- Nothing has been deployed yet. The four CI runs so far were pushes to `main`, all red, so `deploy` was skipped, and the repository has no pull requests. The first green push to `main` deploys production at once (delivery.md 3.3), reachable by anyone who has its URL, since no Cloudflare Access policy protects the project, and every visitor then receives `partsmith_core.wasm`, which contains OCCT. Each pull request whose CI passes would deploy a public preview the same way.
- The OCCT in every build is the unmodified source of the upstream tag `V7_9_3`; `scripts/build-occt.sh` applies no patches and changes only build options. The script skips its clone when `$OCCT_WORK_DIR/src` exists, so it builds any OCCT source put there.
- OCCT includes code from four other projects under their own notices (flex, DELABELLA, David M. Gay's strtod and Unicode's ConvertUTF), which the notices of [ADR 0033](0033-notices-and-npm-license-check-from-the-lock-files.md) carry.
- Sizes, gzipped: Partsmith's source at one commit, as `git archive` writes it, is 10.3 MB. OCCT's tag without its Git data is 49.5 MB, of which the files the build reads (`src`, `adm`, `CMakeLists.txt` and the root text files) are 18.3 MB. Pages serves files of at most 25 MiB.
- M1 distributes the web app and the desktop bundles. The CLI and the Python wheel are built from source only (python-api.md: publishing wheels is later).
- The Tauri AppImage also bundles WebKitGTK, GTK and their dependencies from the Ubuntu 24.04 build machine, many of them under the LGPL, and the AppImage runtime, which reportedly links libfuse (LGPL) statically. The `.deb` and `.rpm` depend on the distribution's packages instead, and the Windows and macOS bundles use the system webview. The NSIS installer carries NSIS's own code, its LZMA decompressor and Tauri's `nsis_tauri_utils` plugin, none of them a Cargo or npm package.

## Options

**A. Keep static linking everywhere and meet section 6 with source (6(a) and 6(d)).** Relinking a statically linked program means rebuilding it, which section 6(a) allows ("as object code and/or source code").

- What it needs:
  1. Partsmith's source for each distributed build, from the place that distributes it: either the repository made public, so each release's tag (with GitHub's source archives on the release page) and `main`'s history are there, or, while it stays private, a source archive attached to each release and written into each web deployment (about 10 MB, under the 25 MiB per-file limit). The private variant thereby publishes Partsmith's source at each deployed commit, previews included, to anyone who has a deployment's URL.
  2. OCCT's source from the same place: an archive of the tag `V7_9_3` attached to each release by the `draft` job and, in the private variant, written into each web deployment too. An archive of the files the build reads is 18.3 MB and fits the 25 MiB limit (that the build works from it alone is checked when the step is implemented); the whole tag, at 49.5 MB, would have to be split into parts under 25 MiB or served from a public place the owner controls. A link to the upstream tag alone is not relied on, since section 6(d) asks for access "from the same place" (question 1 below).
  3. The notices of ADR 0033, whose OCCT statements already say how to relink and point at the exact build steps of each workflow.
  4. A way for a web user to find the exact commit a deployment was built from, previews included (a stamp the `web` job writes, or the version and commit in the UI), since one committed notices file cannot name it.
- Cost: no change to the kernel, the hosts or the build. The `draft` job gains a clone and an archive step (a minute or two) and the release gains the archives (about 30 MB). Making the repository public is the owner's decision; under GitHub's current billing it would also make the standard hosted runners free, macOS included, which removes the CI-minutes concern (a release costs about 520 billed minutes on a private repository, delivery.md section 8).
- It covers the WebAssembly core, which no option can link dynamically at a reasonable cost.

**B. Link OCCT dynamically on the desktop, the CLI and Python; keep it static only in the WebAssembly core.**

- Section 6(b) then covers those three: the OCCT shared libraries ship next to the program and a user can replace them. OCCT's own source still has to be offered for the libraries that ship (section 4), and the web core still needs option A's source route.
- Cost: `build-occt.sh` gains a shared native build with its own cache; `partsmith-occt-sys/build.rs` links dynamically (ADR 0003 superseded); each bundle carries and finds the libraries: `$ORIGIN` run paths in the Linux packages, install names and `bundle.macOS.frameworks` or resources on macOS, DLLs next to the executable on Windows, each with new desktop.md packaging tests; a published wheel would vendor them. A user who swaps OCCT can change results that the recompute cache keys, which name `OCCT_VERSION` rather than the loaded library, do not notice.
- Gain: the roughly 170 test binaries, each of which links OCCT statically at 55 to 80 MB (delivery.md 3.2), would shrink and link faster. delivery.md open issue 9 attacks the same disk problem without touching the license question.

**C. A mix: dynamic only in the desktop app, static elsewhere.** It carries both mechanisms and keeps option A's requirements for every static artifact (web, CLI, wheel), so it gains little over A.

**D. Keep static linking everywhere and give a written offer of the source (6(c)).** Each distributed build comes with a written offer, valid for at least three years, to give any third party the materials of 6(a) for no more than the cost of providing them.

- What it needs: the offer in both notices statements, in place of the pointers to the source, with a contact address that stays valid for three years and someone who answers it; and the exact source of every distributed build kept reproducible for three years after its last distribution: every release tag, and the head commit of every pull request whose preview was deployed (its branch or a tag kept, so the commit is not garbage-collected).
- Cost: while the repository is private it is the cheapest route, with no archive in releases or deployments and nothing published. But every build distributed starts a three-year obligation, one for each preview as well (each push to a pull request whose CI passes deploys one), and the offer binds whoever is named in it. Under this ADR's reading of section 4, OCCT's own source must still accompany each build or be available from the same place (option A item 2), so the offer replaces only Partsmith's source; if a 6(c) offer is enough for an executable that contains OCCT (question 4 below), it replaces both. Once the repository is public, option A costs nothing more and the offer is not needed.

**Not an option: leaving things as they are.** Nothing has been deployed yet, but the first green run on `main` deploys production with OCCT, and without B-2's notices and one of these options in place it would ship OCCT without the license text, the notice or a way to the source.

**The AppImage**, separately: (1) drop it from M1, which means removing `appimage` from the release matrix, the dry run included (a public repository's Actions artifacts are downloadable), and from `bundle.targets`, leaving `.deb` and `.rpm` for Linux; or (2) keep it and add, for each library it bundles and for its runtime, the Debian copyright file to its notices and the Ubuntu source package to the release (hundreds of MB per release).

## Recommendation

Option A, with:

- the repository made public before the Pages site or a release is shared beyond the owner and collaborators. Until then: the Pages project's access policy for preview deployments turned on (Settings, General), the cheapest step, which limits previews to the account's members; and production held back, by a Cloudflare Access application on its host or by removing the two deploy secrets, which makes `deploy` skip cleanly (delivery.md 3.3), until B-2's notices are on `main` and this option's source route is in place;
- the OCCT source archive (the files the build reads) attached to every release next to `SHA256SUMS` and named by the notices, and in the private variant also written into each web deployment;
- every release kept a draft until the source route is in place (delivery.md 3.6, checklist item 1);
- the AppImage dropped from M1 (choice 1), from the release matrix and the dry run alike, since `.deb` and `.rpm` cover the supported distributions and choice 2 costs more than the format is worth now;
- the wording made true:
  - AGENTS.md's invariant becomes "plus OCCT (LGPL 2.1 with the Open CASCADE exception, linked statically; every distributed build comes with a way to get its exact source and OCCT's so users can relink, ADR 0034)";
  - technical design section 3 says the same and drops PlaneGCS; section 17 says OCCT is not a crate and is covered by the notices of delivery.md 2.9 rather than by cargo-deny, and "protecting the MIT/Apache license" becomes "protecting the Apache-2.0 license" (ADR 0028);
  - README.md's license section follows delivery.md 2.12.

Questions for the owner's lawyer before the first public release:

1. Does a link to the upstream OCCT tag meet section 6(d)'s "from the same place", or must the archive come from Partsmith's own release and site? This ADR assumes the latter.
2. Do crates.io and npm, reached through the committed lock files, meet section 6(a)'s "complete machine-readable work that uses the Library", or should each release carry a vendored archive of the dependencies (`cargo vendor` and the pnpm store)?
3. Do the NSIS installer's code and the AppImage runtime need entries in the notices (delivery.md open issue 10)?
4. Is a written offer under section 6(c) enough for an executable that contains OCCT's object code, or does section 4 still require OCCT's source to accompany it or be available from the same place, as this ADR reads it? Option D depends on the answer.

Why: it changes no code, the web core needs the source route under every option, and one rule then covers every artifact. Option D costs less while the repository is private, but it binds the owner for three years per build, previews included, and rests on question 4. Option B's real benefit, smaller and faster test binaries, is a build question that can be decided on its own merits later.

## Consequences

- Until the owner decides, nothing about the linking changes, the notices of ADR 0033 describe today's static linking, releases stay drafts, and production waits as delivery.md section 8 describes.
- If A is accepted: delivery.md 2.5 gains the OCCT source archive in `draft` (and, in the private variant, both archives in the `web` job), the repository visibility change is made by the owner, the web build gains its commit stamp (a ui.md or delivery.md change), and the four documents above are corrected.
- If B or C is accepted: a new ADR supersedes ADR 0003 for native builds, and desktop.md 2.8 and 3.8 and delivery.md 2.4 to 3.6 change for the shared libraries; the notices statements change with them.
- If D is accepted: both notices statements of delivery.md 2.9 carry the offer and its contact address, and every release tag and the head commit of every deployed preview are kept for three years after their last distribution.
- If the AppImage is dropped: desktop.md 1.1, 2.8 (`bundle.targets`) and 3.8, delivery.md 3.6 (the matrix and the unsigned-bundle paragraph) and test 13 lose it.
