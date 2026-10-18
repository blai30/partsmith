# Architecture decision records

Every lasting decision is recorded here, one file per decision, numbered in order. Read the ADRs for an area before changing it, and write a new ADR when you change a decision (mark the old one superseded rather than editing its decision).

Format: title, status (proposed, accepted, superseded by NNNN), context, decision, consequences. Decisions marked *needs human review* change something the technical design proposed and wait for sign-off at the next milestone gate.

| ADR | Decision | Status |
| --- | --- | --- |
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted |
| [0002](0002-react-for-the-ui.md) | React for the UI | Accepted |
| [0003](0003-pin-occt-and-build-from-source.md) | Pin OCCT 7.9.3, built from source as static libraries | Accepted; amended 2026-10-17 (fillets and chamfers build one at a time), the amendment needs human review |
| [0004](0004-native-rust-sketch-solver.md) | Native Rust sketch solver instead of PlaneGCS | Accepted, needs human review |
| [0005](0005-feature-registry-with-generic-params.md) | Feature registry with generically stored params | Accepted |
| [0006](0006-protocol-framing-and-schema.md) | Protocol: JSON header plus binary buffers, schema-generated types | Accepted |
| [0007](0007-webgl2-renderer-first.md) | WebGL2 renderer first, WebGPU behind the same interface | Accepted, needs human review |
| [0008](0008-short-random-ids.md) | Short random ids with kind prefixes | Accepted, needs human review |
| [0009](0009-emscripten-c-abi-bridge.md) | Web core as an Emscripten module with a C ABI | Accepted |
| [0010](0010-unsafe-c-abi-exports-in-partsmith-wasm.md) | `unsafe` on the three C ABI exports of `partsmith-wasm` | Accepted, needs human review |
| [0011](0011-feature-contract-extensions.md) | Feature contract extensions and manifests in the model crate | Accepted, needs human review |
| [0012](0012-dependency-log-m1.md) | New dependencies for M1: blake3, zip and faer | Accepted, needs human review |
| [0013](0013-oxc-tooling-and-theme-stylesheet.md) | oxlint and oxfmt for the UI, and a minimal theme stylesheet | Accepted; font loading superseded by 0031 |
| [0014](0014-desktop-host-and-dependencies.md) | Tauri 2 desktop host with tauri-build and tauri-plugin-dialog, and no Tauri npm package | Proposed, needs human review |
| [0015](0015-sketch-solver-own-sparse-qr.md) | Sketch solver uses its own sparse rank-revealing QR instead of faer | Accepted, needs human review |
| [0016](0016-generated-schema-check-ins.md) | Generated schema check-ins (log) | Accepted, needs human review |
| [0017](0017-keep-the-web-build-desktop-leads.md) | Keep the web build; the desktop app leads where the platforms differ | Accepted, needs human review |
| [0018](0018-body-updates-carry-measurements.md) | Body updates carry the measurements of their shape | Accepted, needs human review |
| [0019](0019-hosts-read-profile-and-load-positions-from-the-engine.md) | Hosts read the resolved printer profile and load error positions from the engine; the CLI drops `partsmith-print` | Accepted, needs human review |
| [0020](0020-dependency-log.md) | Dependency log of the workspace, and removal of four unused workspace dependencies | Accepted, needs human review |
| [0021](0021-resolve-references-by-meaning.md) | Resolve references by meaning, never by a fingerprint guess: anchors, merged names, canonical names, losses with candidates | Accepted, needs human review (amended 2026-10-15) |
| [0022](0022-split-regions-keep-their-area.md) | A profile region follows its boundary and keeps covering its area when split | Accepted, needs human review |
| [0023](0023-frames-live-on-the-model.md) | Sketch frames, axis directions and chamfer sides live on the model; a turned-over face mirrors the drawing with "Turn the drawing back" | Accepted, needs human review (amended 2026-10-17) |
| [0024](0024-diagnostics-with-actions-causes-and-ghosts.md) | Diagnostics carry actions, causes, details, candidates and ghosts (protocol schema) | Accepted, needs human review |
| [0025](0025-file-format-version-2.md) | File format version 2 for anchors, merged names, region outlines and model frames, migrated from 1 without change | Accepted, needs human review |
| [0026](0026-kernel-facade-additions-for-errors.md) | Kernel facade additions for ghosts, limits, void checks and true hints | Accepted, needs human review |
| [0027](0027-dev-only-workspace-crates.md) | Dev-only workspace crates: `partsmith-budgets` for the budget benchmarks and `partsmith-mesh-fixtures` for the mesh.md section 6.1 test fixtures; amended 2026-10-18 with `partsmith-message-lint` for the message lint of the kernel, sketch, features and recompute tests | Accepted, needs human review (amended 2026-10-18) |
| [0028](0028-single-license-apache-2.md) | One license, Apache-2.0, for code, specifications, examples and test corpus | Accepted |
| [0029](0029-z-up-right-handed.md) | Right-handed Z-up coordinates everywhere, with no up-axis option | Accepted |
| [0030](0030-islands-over-a-full-window-viewport-and-rgb-axes.md) | Floating islands over a full-window viewport, and X, Y, Z in red, green and blue | Accepted, needs human review; outlines and surfaces amended by 0032 |
| [0031](0031-self-hosted-geist-and-jetbrains-mono-for-code.md) | Geist with a serifed I and tabular figures for interface text and numbers, JetBrains Mono for code only, both self-hosted | Accepted, needs human review |
| [0032](0032-flat-islands-without-outlines.md) | Flat islands: no outlines, surfaces a step lighter than one flat viewport, and edges drawn only in forced colors | Accepted, needs human review |
| [0033](0033-notices-and-npm-license-check-from-the-lock-files.md) | Third-party notices and the npm license check, generated from the lock files by two Node scripts, with no new tool and no license agreement in the installers; the npm packages that ship logged | Proposed, needs human review |
| [0034](0034-occt-linking-and-lgpl-obligations.md) | OCCT stays linked statically everywhere, and LGPL 2.1 section 6 is met with published source (option A of four): the repository made public, a commit stamp in every web deployment and the OCCT source archive in every release | Accepted (owner decision, 2026-10-16); the AppImage choice and four questions for a lawyer, the first deciding whether the web's OCCT source falls short now, need human review |
