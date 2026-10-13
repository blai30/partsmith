# 0028: One license, Apache-2.0

Status: Accepted (decided by the project owner, 2026-10-13)

## Context

Partsmith was dual-licensed MIT OR Apache-2.0, the Rust ecosystem's convention for libraries, with the file-format specification and test corpus described as CC0 but shipped without a CC0 text. Two license files and a third license named only in prose made the terms harder to state than they need to be, and the M1 gap list (DEL-1) found the CC0 grant without its legal text.

## Decision

Everything in the repository that the project itself writes (code, specifications, the file format, examples and test corpus) is licensed under the Apache License 2.0, in one `LICENSE` file at the root. Every package manifest says `Apache-2.0`: the Cargo workspace (inherited by every crate), `ui/package.json`, `python/pyproject.toml` and the desktop bundle in `desktop/src-tauri/tauri.conf.json`. Models people make with Partsmith remain theirs.

Apache-2.0 over MIT: both are permissive, but Apache-2.0 carries an explicit patent grant from every contributor and ends it for anyone who sues over patents in the work. CAD and computational geometry are patent-heavy fields, so that protection is worth the longer text.

## Consequences

- `LICENSE-MIT` is removed and `LICENSE-APACHE` becomes `LICENSE`. The CC0 wording is gone from the README and the technical design.
- Dependency policy is unchanged: dependencies may be under any permissive license that `deny.toml` allows (CC0-1.0 stays allowed for dependencies such as `blake3`), plus OCCT under the LGPL with its exception. Apache-2.0 is compatible with that set.
- Apache-2.0 is incompatible with GPLv2-only code, which the policy already keeps out.
- Third-party notices (fonts, OCCT, crates and npm packages in the shipped bundles) are a separate obligation, tracked as B-2 in `docs/design/m1-gap-list.md`.
