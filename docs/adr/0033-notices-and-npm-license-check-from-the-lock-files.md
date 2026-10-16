# 0033: Third-party notices and the npm license check, generated from the lock files by two Node scripts

Status: Proposed, needs human review (licenses: what the notices hold, the OCCT statement and the texts of packages that ship none; tools: no new tool, two scripts; dependency log: the npm production packages below)

## Context

Partsmith distributes two kinds of artifacts in M1: the web app on Cloudflare Pages (the UI and the WebAssembly core) and the desktop bundles of a release. Neither carries third-party notices today (m1-gap-list B-2), and no check covers the licenses of npm packages (DEL-2). The licenses involved ask for different things:

- MIT, BSD, ISC, Zlib, Unicode-3.0 and BSL-1.0 ask that their text and copyright line travel with every copy, binaries included; Apache-2.0 asks for its text and any NOTICE file.
- MPL-2.0 (five crates under Tauri on every desktop platform: `cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext` and `selectors`) asks that recipients of an executable be told where the source of those files is.
- The SIL Open Font License 1.1 of Geist and JetBrains Mono (ADR 0031) asks that its text travel with the fonts, which it already does in `ui/public/fonts`.
- OCCT's LGPL 2.1 with the Open CASCADE exception asks for a copy of the license, a prominent notice that OCCT is used, and a way for the user to relink with a modified OCCT. ADR 0034 sets out how Partsmith links OCCT and the options for that last part.

Measured on 2026-10-16 with `cargo metadata --locked --all-features --filter-platform <triple>`, following normal and build dependencies from each artifact's root crate:

| Artifact | Root crate | Triple | External crates | License files | Distinct texts | Bytes of distinct texts |
| --- | --- | --- | --- | --- | --- | --- |
| Desktop, Linux | `partsmith-desktop` | `x86_64-unknown-linux-gnu` | 308 | 536 | 172 | 540,097 |
| Desktop, Windows | `partsmith-desktop` | `x86_64-pc-windows-msvc` | 270 | | | |
| Desktop, macOS | `partsmith-desktop` | `aarch64-apple-darwin` | 265 | | | |
| Web core | `partsmith-wasm` | `wasm32-unknown-emscripten` | 54 | 109 | 45 | 160,139 |

The four sets hold 362 distinct crates. Texts were compared after the whitespace normalization of delivery.md 2.9; without deduplication the Linux texts are 2.5 MB. Seven Linux crates ship no license file in their package (`alloc-stdlib` 0.3.0, `defmt-parser` 1.0.0, `dlopen2` 0.8.2, `dlopen2_derive` 0.4.3, `libappindicator-sys` 0.9.0, `selectors` 0.38.0, `tauri-plugin` 2.7.1); the Windows and macOS counts of files were not measured, because this machine has not downloaded those platforms' crate sources. The Unicode-3.0 texts of eighteen ICU4X crates (`icu_*`, `zerovec`, `yoke` and their kin) contain the em dash character.

`pnpm licenses list --prod --json` in `ui/` lists eight production packages, every one with a license file:

| Package | Version | License | Ships in the bundle |
| --- | --- | --- | --- |
| `react` | 19.3.0 | MIT | yes |
| `react-dom` | 19.3.0 | MIT | yes |
| `scheduler` | 0.28.0 | MIT | yes (through `react-dom`) |
| `zustand` | 5.0.15 | MIT | yes |
| `clsx` | 2.1.1 | MIT | yes |
| `lucide-react` | 1.55.0 | ISC | yes (the icons the UI imports) |
| `@types/react` | 19.3.0 | MIT | no (types only, a peer of `zustand`) |
| `csstype` | 3.2.3 | MIT | no (types only, through `@types/react`) |

The development dependencies are under MIT, ISC, Apache-2.0, BSD-2-Clause, BSD-3-Clause, CC0-1.0 and MPL-2.0, and under three licenses `deny.toml` does not allow: MIT-0 (two packages), Python-2.0 (`argparse` 2.0.1) and BlueOak-1.0.0 (`lru-cache` 11.5.3). None of them ships in the web build or the desktop bundles.

Three ways to produce the notices were weighed:

1. **cargo-about** (Embark Studios, MIT OR Apache-2.0), the usual tool for Rust notices. It is a new CI tool to install and pin, with its own configuration and Handlebars templates, and it covers only crates: the npm packages, the fonts, OCCT and the toolchain libraries would still need a script.
2. **`cargo deny list`**, already in use through `cargo-deny`, reports each crate's license but not its text.
3. **Two small Node scripts** over `cargo metadata` and `pnpm licenses list`, both of which CI already runs or can run, next to `check-dist.mjs` in `ui/scripts`.

## Decision

- **Generator.** `ui/scripts/notices.mjs` (Node's standard library only) writes `THIRD-PARTY-NOTICES.txt` for the desktop app and `ui/public/THIRD-PARTY-NOTICES.txt` for the web app from `Cargo.lock` (through `cargo metadata`), `ui/pnpm-lock.yaml` (through `pnpm licenses list --prod --json`), the license files those packages ship, OCCT's license texts in the install, the two font licenses, the toolchain pins and the root `LICENSE`. Both files are committed. `pnpm -C ui check:notices` in CI's `schema` job fails when either differs from what the lock files give. delivery.md 2.9 specifies the inputs, the layout, the OCCT statements and where each file ships; desktop.md 2.8 adds the desktop bundle's `resources` and `licenseFile`.
- **npm license check.** `ui/scripts/check-licenses.mjs` checks every production npm package against the `allow` list of `deny.toml`, the same list `cargo-deny` applies to crates, so the policy has one source. It runs in CI's `ui` job (delivery.md 2.10). Development dependencies are not checked: they never ship, and three of them are under licenses the list does not allow.
- **No new tool.** cargo-about is not adopted. The scripts need Node, `cargo` and `pnpm`, which every CI job that runs them has already.
- **Packages without a license file** get the standard text of the first license in their expression that `deny.toml` allows, from the SPDX License List text committed under `ui/scripts/notices/licenses/`, and their entry says so. The alternative, a hand-kept copy of each one's upstream file at each version, costs a manual step at every update of those seven crates.
- **Verbatim texts.** Third-party license texts are reproduced as their packages ship them, apart from whitespace normalization (line endings, trailing spaces, blank lines at the ends). The em dashes in the ICU4X texts stay: CLAUDE.md's style rule governs text the project writes, not license texts it must copy.
- **npm dependency log.** The eight production packages above, with their licenses, are the log this ADR records for the UI; later additions are new rows or a new ADR, as ADR 0020 does for crates.

## Consequences

- Every pull request that changes a lock file, a toolchain pin, OCCT, the fonts or `LICENSE` also regenerates the notices (`pnpm -C ui notices`); CI says so when it does not.
- Two generated text files join the repository, about 0.6 MB for the desktop and 0.2 MB for the web. They hold more than ships: build dependencies and the two type-only npm packages are listed, which is harmless; nothing that ships is left out.
- The `schema` job downloads the crate sources of all four platforms through `cargo metadata`.
- The web build gains one 0.2 MB text file; it is not initial JavaScript and is far from the 25 MiB per-file limit.
- The notices do not cover the system libraries that the AppImage bundles, nor the CLI and the Python wheel, which M1 does not distribute (delivery.md 2.9 and open issue 10, ADR 0034).
