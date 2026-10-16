# 0012: New dependencies for M1: blake3, zip and faer

Status: Accepted, needs human review

## Context

AGENTS.md requires every new dependency to be recorded in the ADR log and reviewed by a human at the next gate. The M1 specs need three crates that no earlier ADR names. The license policy (technical design section 17, enforced by `cargo-deny` with `deny.toml`) allows only permissive licenses.

## Decision

| Crate | License | Used by | Purpose |
| --- | --- | --- | --- |
| `blake3` | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | `partsmith-recompute`, `partsmith-engine` | Recompute cache keys (recompute.md section 3.4), the engine's caches keyed by those hashes, and the content-hash names of imported files in `assets/` (file-format.md section 3.11, engine.md section 3.14). `partsmith-features` does not depend on it: it exposes evaluated values through `Values::iter` and recompute hashes them. |
| `zip` | MIT | `partsmith-io` | Writing 3MF packages, which are ZIP archives (io.md section 3.6.5). Default features off, only `deflate`, and fixed entry timestamps so exports are deterministic. |
| `faer` | MIT | none | Superseded by ADR 0015: the sketch solver uses its own sparse QR, and `faer` is not a dependency. |

`quick-xml`, which an earlier draft of this ADR listed for the XML parts of 3MF, is not used in M1: io.md writes the three small 3MF parts as plain strings (io.md section 1.3, decision 6). The first XML reader (3MF mesh import, later) needs a new ADR before it adds an XML crate.

Each crate is added to `[workspace.dependencies]` and to the dependencies of only the crates listed above. `cargo-deny` checks the licenses of their transitive dependencies.

## Consequences

Content hashing, 3MF export and fast sketch solving use maintained, widely used crates instead of hand-written code, in line with "battle-tested pattern only when it solves a problem the code actually has". Each crate adds build time and supply-chain surface, which is why the decision waits for human review. Removing or replacing one later needs a new ADR.
