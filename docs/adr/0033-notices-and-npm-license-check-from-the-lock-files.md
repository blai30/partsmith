# 0033: Third-party notices and the npm license check, generated from the lock files by two Node scripts

Status: Proposed, needs human review (licenses: what the notices hold, the OCCT statements, the texts given to crates that ship none, and the wording of the em dash rule; tools: no new tool, two scripts; dependency log: the npm packages that ship, below)

## Context

Partsmith distributes two kinds of artifacts in M1: the web app on Cloudflare Pages (the UI and the WebAssembly core) and the desktop bundles of a release. Neither carries third-party notices today (m1-gap-list B-2), and no check covers the licenses of npm packages (DEL-2). The licenses involved ask for different things:

- MIT, BSD, ISC and Unicode-3.0 ask that their text and copyright line travel with every copy, binaries included; Zlib and BSL-1.0 ask nothing for binaries, and their texts are listed anyway. Apache-2.0 asks for its text and any NOTICE file.
- MPL-2.0 (five crates under Tauri on every desktop platform: `cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext` and `selectors`) asks that recipients of an executable be told how to obtain the Source Code Form of those files.
- The SIL Open Font License 1.1 of Geist and JetBrains Mono (ADR 0031) asks that its text travel with the fonts, which it already does in `ui/public/fonts`.
- OCCT's LGPL 2.1 with the Open CASCADE exception asks for a copy of the license, a prominent notice that OCCT is used, and a way for the user to relink with a modified OCCT. ADR 0034 sets out how Partsmith links OCCT and the options for that last part.

**Crates.** Measured on 2026-10-16 with `cargo metadata --offline --locked --all-features --filter-platform <triple>` (the crate sources of every platform are on the build machine), following normal and build dependencies from each artifact's root crate, and counting the license files at the top of each crate's folder and in its `LICENSES` folder:

| Artifact | Root crate | Triple | External crates | License files | Distinct texts | Bytes of distinct texts |
| --- | --- | --- | --- | --- | --- | --- |
| Desktop, Linux | `partsmith-desktop` | `x86_64-unknown-linux-gnu` | 308 | 536 | 173 | 552,237 |
| Desktop, Windows | `partsmith-desktop` | `x86_64-pc-windows-msvc` | 270 | 485 | 161 | 509,355 |
| Desktop, macOS | `partsmith-desktop` | `aarch64-apple-darwin` | 265 | 458 | 164 | 511,713 |
| Desktop, the three together | `partsmith-desktop` | | 362 | 614 | 191 | 601,848 |
| Web core | `partsmith-wasm` | `wasm32-unknown-emscripten` | 54 | 109 | 45 | 160,487 |

The web crates are all among the desktop's 362. Texts were compared after the whitespace normalization of delivery.md 2.9; without deduplication the Linux texts are 2.5 MB. Three license files sit deeper in a crate: `regex-syntax`'s `src/unicode_tables/LICENSE-UNICODE`, the Unicode data license (Unicode-DFS-2016) of the tables every desktop build holds, which `cargo-deny` never sees because the crate declares `MIT OR Apache-2.0`; `tracing-core`'s `src/spin/LICENSE`, the MIT text of the `spin` code it compiles without its `std` feature, which no crate enables on Windows; and `libdbus-sys`'s `vendor/dbus/`, a vendored libdbus that only its `vendored` feature builds, which Partsmith does not enable.

Twenty-two desktop crates ship no license file:

- Linux, Windows and macOS: `alloc-stdlib` 0.3.0 (BSD-3-Clause), `defmt-parser` 1.0.0 (`MIT OR Apache-2.0`), `selectors` 0.38.0 (MPL-2.0), `tauri-plugin` 2.7.1 (`Apache-2.0 OR MIT`).
- Linux: `dlopen2` 0.8.2 and `dlopen2_derive` 0.4.3 (MIT), `libappindicator-sys` 0.9.0 (`Apache-2.0 OR MIT`).
- Windows: `webview2-com` 0.39.1, `webview2-com-macros` 0.8.1 and `webview2-com-sys` 0.39.1 (MIT).
- macOS: `block2` 0.6.2, `objc2` 0.6.5, `objc2-encode` 4.1.0 and `objc2-foundation` 0.3.2 (MIT); `dispatch2` 0.3.1, `objc2-exception-helper` 0.1.1 and the six `objc2-*` 0.3.2 crates `app-kit`, `core-foundation`, `core-graphics`, `io-surface`, `quartz-core` and `web-kit` (`Zlib OR Apache-2.0 OR MIT`).

The SPDX License List texts of MIT, BSD-3-Clause and Zlib leave the copyright line as a placeholder (`Copyright (c) <year> <copyright holders>`), while MIT and BSD ask for "the above copyright notice", so a standard text does not meet them. The Apache-2.0 and MPL-2.0 texts have no line to fill in.

**Prebuilt libraries.** Two crates of the graph ship prebuilt libraries outside a test folder. `webview2-com-sys` 0.39.1 ships Microsoft's WebView2 loader (`x64/WebView2LoaderStatic.lib` and DLLs, for three architectures) and links it statically under MSVC, while it declares only MIT, for its bindings, and ships no license text. The loader is under the license of the `Microsoft.Web.WebView2` NuGet package; for SDK 1.0.3650.58 that is a BSD-3-Clause text with Microsoft's copyright line, whose binary clause asks for the notice in the documentation. `windows_x86_64_msvc` 0.53.1 ships import libraries that the windows-rs project generates, under the crate's own `MIT OR Apache-2.0`. (`system-deps` keeps a test library in `src/tests/lib`, which nothing links.)

**OCCT's bundled code.** The OCCT toolkits that `partsmith-occt-sys` links compile code from four other projects whose licenses ask for their notice with binaries: flex's `FlexLexer.h` and scanner skeleton in the STEP reader (copyright The Regents of the University of California, a Berkeley license), DELABELLA in the mesher (MIT, Marcin Sokalski), David M. Gay's `strtod` in TKernel (copyright Lucent Technologies, whose notice must be "included ... in all copies of the supporting documentation"), and Unicode, Inc.'s ConvertUTF in `NCollection_UtfIterator.lxx`. The Bison skeleton of the STEP parser carries a special exception that asks nothing, and the DejaVu Sans font OCCT embeds is compiled only with FreeType, which `scripts/build-occt.sh` turns off.

**Toolchain code.** Rust's `core` holds Unicode data tables under Unicode-3.0 (the toolchain's `share/doc/rust/COPYRIGHT-library.html`); the other standard library crates all offer Apache-2.0. The web core links Emscripten's libc++, libc++abi, libunwind and compiler-rt, whose `LICENSE.TXT` files (about 16.7 KB each) hold the Apache 2.0 text with the LLVM exception and the NCSA and MIT texts that still cover older code, which ask for their notice with binaries.

**npm.** `pnpm licenses list --prod --json` in `ui/` lists eight production packages, and three development dependencies write code into `ui/dist`, so eleven packages ship, every one with a license file:

| Package | Version | License | Ships in the build |
| --- | --- | --- | --- |
| `react` | 19.3.0 | MIT | yes |
| `react-dom` | 19.3.0 | MIT | yes |
| `scheduler` | 0.28.0 | MIT | yes (through `react-dom`) |
| `zustand` | 5.0.15 | MIT | yes |
| `clsx` | 2.1.1 | MIT | yes |
| `lucide-react` | 1.55.0 | ISC | yes (the icons the UI imports) |
| `@types/react` | 19.3.0 | MIT | no (types only, a peer of `zustand`) |
| `csstype` | 3.2.3 | MIT | no (types only, through `@types/react`) |
| `rolldown` | 1.2.13 | MIT | yes (development: the CommonJS interop helpers at the top of `assets/index-*.js`) |
| `tailwindcss` | 4.3.3 | MIT | yes (development: the CSS, which starts with its license banner) |
| `vite` | 8.3.4 | MIT | yes (development: the module preload polyfill) |

Vite's `LICENSE.md` is 108,786 bytes, since it holds the licenses of the code Vite bundles into itself. The other development dependencies are under MIT, ISC, Apache-2.0, BSD-2-Clause, BSD-3-Clause, CC0-1.0 and MPL-2.0, and under three licenses `deny.toml` does not allow: MIT-0 (two packages), Python-2.0 (`argparse` 2.0.1) and BlueOak-1.0.0 (`lru-cache` 11.5.3). None of them ships.

**Em dashes.** The Unicode-3.0 texts of eighteen ICU4X crates (`icu_*`, `zerovec`, `yoke` and their kin) contain the em dash character, which the root AGENTS.md forbids "in any file".

Three ways to produce the notices were weighed:

1. **cargo-about** (Embark Studios, MIT OR Apache-2.0), the usual tool for Rust notices. It is a new CI tool to install and pin, with its own configuration and Handlebars templates, and it covers only crates: the npm packages, the fonts, OCCT and the other libraries would still need a script.
2. **`cargo deny list`**, already in use through `cargo-deny`, reports each crate's license but not its text.
3. **Two small Node scripts** over `cargo metadata` and `pnpm licenses list`, both of which CI already runs or can run, next to `check-dist.mjs` in `ui/scripts`.

## Decision

- **Generator.** `ui/scripts/notices.mjs` (Node's standard library only) writes `THIRD-PARTY-NOTICES.txt` for the desktop app and `ui/public/THIRD-PARTY-NOTICES.txt` for the web app from `Cargo.lock` (through `cargo metadata`), `ui/pnpm-lock.yaml` (through `pnpm licenses list`), the license files those packages ship (nested ones included, one listed exclusion aside), the two font licenses, the toolchain pins, the root `LICENSE`, and committed texts under `ui/scripts/notices/`: the standard Apache-2.0 and MPL-2.0 texts, copies from the repositories of crates that ship none, OCCT's license, exception and bundled notices for each OCCT version, Rust's Unicode-3.0 text for each channel, the Emscripten texts for each SDK version, and the WebView2 SDK license for each `webview2-com-sys` version. It needs no OCCT install. Both files are committed. `pnpm -C ui check:notices` in CI's `schema` job fails when either differs from what the inputs give. delivery.md 2.9 specifies the inputs, the layout, the OCCT statements and where each file ships; desktop.md 2.8 adds the desktop bundle's `resources`.
- **No license agreement.** The notices are a bundle resource and a release asset, not the bundle's `licenseFile`: the Tauri bundler turns that file into a license the user must accept, on the NSIS installer's license page and as the DMG's license agreement, which neither Apache-2.0 nor the LGPL asks for.
- **npm license check.** `ui/scripts/check-licenses.mjs` checks every npm package that ships (the production dependencies and the three build packages above) against the `allow` list of `deny.toml`, the same list `cargo-deny` applies to crates, so the policy has one source. It runs in CI's `ui` job (delivery.md 2.10). The other development dependencies are not checked: they do not ship, and three of them are under licenses the list does not allow.
- **No new tool.** cargo-about is not adopted. The scripts need Node, `cargo` and `pnpm`, which every CI job that runs them has already.
- **Crates without a license file** get the standard Apache-2.0 or MPL-2.0 text when one of those alone satisfies their expression (12 crates), and otherwise a committed copy of the license file in their repository, kept by crate name (10 crates); the entry says which. The cost is a manual step when a new crate under only MIT, BSD or Zlib ships no text: the generator fails until someone commits its file.
- **Prebuilt libraries.** The generator fails on a crate that ships a prebuilt library and is not on its list, and the WebView2 loader gets its own Windows-only entry with Microsoft's license.
- **Verbatim texts.** Third-party license texts are reproduced as their packages ship them, apart from whitespace normalization (line endings, trailing spaces, blank lines at the ends). The em dashes in the ICU4X texts stay, which breaks the root AGENTS.md rule as written. This ADR proposes, for the owner to accept, the wording: "Never write the em dash character in any file, except third-party license texts copied verbatim: the notices that `pnpm -C ui notices` writes and the copies it reads under `ui/scripts/notices/` (delivery.md 2.9)."
- **npm dependency log.** The eleven packages above, with their licenses, are the log this ADR records for the UI; later additions are new rows or a new ADR, as ADR 0020 does for crates.

## Consequences

- Every pull request that changes a lock file, a manifest, a toolchain pin, OCCT, the fonts, `LICENSE` or the generator also regenerates the notices (`pnpm -C ui notices`); CI says so when it does not. The change that first commits the notices adds that command to the root AGENTS.md, which every other change reads.
- Two generated text files join the repository, about 0.8 MB for the desktop and 0.4 MB for the web. They hold more than ships: build dependencies, the two type-only npm packages and, in one desktop file, the crates of all three platforms are listed, which is harmless; nothing that ships is left out.
- About thirty small committed texts join `ui/scripts/notices/`, each made once per crate, pin or version and reviewed in the pull request that adds it.
- The `schema` job downloads the crate sources of all four platforms through `cargo metadata`; the notices need no OCCT install there or locally.
- The web build gains one 0.4 MB text file; it is not initial JavaScript and is far from the 25 MiB per-file limit.
- The notices do not cover the system libraries and the runtime that the AppImage bundles, the NSIS installer's own code, nor the CLI and the Python wheel, which M1 does not distribute (delivery.md 2.9 and open issue 10, ADR 0034).
