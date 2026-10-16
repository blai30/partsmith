# 0034: OCCT is linked statically while the project's rules say "a separate library": options for meeting LGPL 2.1 section 6, and a recommendation

Status: Proposed, needs human review (license and linking: the owner chooses an option; nothing changes until then)

## Context

**The contradiction.** Four documents describe how Partsmith uses OCCT, and they disagree:

- CLAUDE.md, invariants: "License policy: only permissive dependencies, plus OCCT (LGPL, linked as a separate library)."
- Technical design section 3: "OCCT and PlaneGCS stay LGPL as separate libraries; publishing their source and allowing relinking, which an open project does anyway, meets the LGPL." Section 17 adds that cargo-deny allows "the listed LGPL exceptions (OCCT, PlaneGCS)".
- README.md: "OCCT is used under the LGPL 2.1 with the OCCT exception, as a separately buildable library."
- [ADR 0003](0003-pin-occt-and-build-from-source.md): OCCT 7.9.3 is built "from source with `scripts/build-occt.sh` for every target: native static libraries with position-independent code, and Emscripten static libraries".

ADR 0003 is what the code does. `partsmith-occt-sys/build.rs` links the 19 OCCT toolkits with `rustc-link-lib=static`, so OCCT is part of every binary that uses the kernel: the desktop app on each platform, the `partsmith` CLI, the Python extension module `partsmith._core` and the web core `partsmith_core.wasm`. PlaneGCS is not used at all ([ADR 0004](0004-native-rust-sketch-solver.md)), and `deny.toml` lists no LGPL exception, because OCCT is not a crate.

**What the license asks.** OCCT is under the GNU LGPL 2.1 with the Open CASCADE exception 1.0 (the texts are in `share/doc/opencascade` of every OCCT install). An executable that contains OCCT falls under LGPL 2.1 section 6. Its terms must allow modification for the user's own use and reverse engineering to debug it (Apache-2.0 does). Each copy needs a prominent notice that OCCT is used and is under the LGPL, with a copy of the license. The OCCT exception lets object code that includes material from OCCT's headers carry any terms, given a prominent notice in the supporting documentation that it uses OCCT. Beyond the notice, section 6 asks for one of:

- (a) the complete source of OCCT as used, and, for an executable linked with it, the "work that uses the Library" as object code or source, so the user can relink a modified OCCT;
- (b) a shared library mechanism that uses a copy already on the user's computer and works with a modified, interface-compatible version;
- (c) a written offer, valid for at least three years, to give the same materials;
- (d) when the work is distributed by offering access from a designated place, equivalent access to those materials from the same place;
- (e) proof that the user already has them.

Distributing OCCT's own object code also falls under section 4: its source must come with it, or be available from the same place. This ADR reads the license as engineers do. It is not legal advice, and the owner may want a lawyer to confirm the choice before the first public release.

**Facts on 2026-10-16.**

- The GitHub repository `blai30/partsmith` is private. Release assets of a private repository are visible only to people with access to it.
- The Pages site is reachable by anyone who has its URL, since no Cloudflare Access policy protects the project. Every visitor receives `partsmith_core.wasm`, which contains OCCT. Production has not been deployed yet, because CI on `main` has not passed (m1-gap-list B-1), but every pull request whose CI passes deploys a preview.
- The OCCT in every build is the unmodified source of the upstream tag `V7_9_3`; `scripts/build-occt.sh` changes only build options. The script skips its clone when `$OCCT_WORK_DIR/src` exists, so it builds any OCCT source put there.
- Partsmith's own source, the repository at one commit as `git archive` writes it, is 10.3 MB gzipped.
- M1 distributes the web app and the desktop bundles. The CLI and the Python wheel are built from source only (python-api.md: publishing wheels is later).
- The Tauri AppImage also bundles WebKitGTK, GTK and their dependencies from the Ubuntu 24.04 build machine, many of them under the LGPL. The `.deb` and `.rpm` depend on the distribution's packages instead, and the Windows and macOS bundles use the system webview.

## Options

**A. Keep static linking everywhere and meet section 6 with source (6(a) and 6(d)).** Relinking a statically linked program means rebuilding it, which section 6(a) allows ("as object code and/or source code").

- What it needs:
  1. Partsmith's source for each distributed build, from the place that distributes it: either the repository made public, so each release's tag (with GitHub's source archives on the release page) and `main`'s history are there, or, while it stays private, a source archive attached to each release and written into each web deployment (about 10 MB, under the 25 MiB per-file limit).
  2. OCCT's source from the same place: a `git archive` of the tag `V7_9_3` (tens of MB) attached to each release by the `draft` job, with the web notices linking to it or to the upstream tag.
  3. The notices of [ADR 0033](0033-notices-and-npm-license-check-from-the-lock-files.md), whose OCCT statement already says how to relink.
  4. A way for a web user to find the exact commit a deployment was built from (a stamp the `web` job writes, or the version and commit in the UI), since one committed notices file cannot name it.
- Cost: no change to the kernel, the hosts or the build. The `draft` job gains a clone and an archive step (a minute or two) and the release gains the archives. Making the repository public is the owner's decision; under GitHub's current billing it would also make the standard hosted runners free, macOS included, which removes the CI-minutes concern.
- It covers the WebAssembly core, which no option can link dynamically at a reasonable cost.

**B. Link OCCT dynamically on the desktop, the CLI and Python; keep it static only in the WebAssembly core.**

- Section 6(b) then covers those three: the 19 OCCT shared libraries ship next to the program and a user can replace them. OCCT's own source still has to be offered for the libraries that ship (section 4), and the web core still needs option A's source route.
- Cost: `build-occt.sh` gains a shared native build with its own cache; `partsmith-occt-sys/build.rs` links dynamically (ADR 0003 superseded); each bundle carries and finds the libraries: `$ORIGIN` run paths in the Linux packages, install names and `bundle.macOS.frameworks` or resources on macOS, DLLs next to the executable on Windows, each with new desktop.md packaging tests; a published wheel would vendor them. A user who swaps OCCT can change results that the recompute cache keys, which name `OCCT_VERSION` rather than the loaded library, do not notice.
- Gain: the roughly 170 test binaries, each of which links OCCT statically at 55 to 80 MB (delivery.md 3.2), would shrink and link faster. delivery.md open issue 9 attacks the same disk problem without touching the license question.

**C. A mix: dynamic only in the desktop app, static elsewhere.** It carries both mechanisms and keeps option A's requirements for every static artifact (web, CLI, wheel), so it gains little over A.

**Not an option: leaving things as they are.** The preview deployments already distribute OCCT object code without the license text, the notice or a way to the source.

**The AppImage**, separately: (1) drop it from the M1 release matrix, leaving `.deb` and `.rpm` for Linux; or (2) keep it and add, for each library it bundles, the Debian copyright file to its notices and the Ubuntu source package to the release (hundreds of MB per release).

## Recommendation

Option A, with:

- the repository made public before the Pages site or a release is shared beyond the owner and collaborators, or, until then, the Pages project protected by Cloudflare Access;
- the OCCT source archive attached to every release next to `SHA256SUMS`, and named by the notices;
- the AppImage dropped from M1 (choice 1), since `.deb` and `.rpm` cover the supported distributions and choice 2 costs more than the format is worth now;
- the wording made true: CLAUDE.md's invariant becomes "plus OCCT (LGPL 2.1 with the Open CASCADE exception, linked statically; its source and Partsmith's are published so users can relink, ADR 0034)"; technical design section 3 says the same and drops PlaneGCS; section 17 says OCCT is not a crate and is covered by the notices of delivery.md 2.9 rather than by cargo-deny; README.md's license section follows delivery.md 2.12.

Why: it changes no code, the web core needs the source route under every option, and one rule then covers every artifact. Option B's real benefit, smaller and faster test binaries, is a build question that can be decided on its own merits later.

## Consequences

- Until the owner decides, nothing about the linking changes, and the notices of ADR 0033 describe today's static linking.
- If A is accepted: delivery.md 2.5 gains the OCCT source archive in `draft`, the repository visibility change is made by the owner, the web build gains its commit stamp (a ui.md or delivery.md change), and the four documents above are corrected.
- If B or C is accepted: a new ADR supersedes ADR 0003 for native builds, and desktop.md 2.8 and 3.8 and delivery.md 2.4 to 3.6 change for the shared libraries; the notices statements change with them.
- If the AppImage is dropped: desktop.md 1.1 and 3.8 and delivery.md 3.6 lose it, and `bundle.targets` loses `appimage`.
