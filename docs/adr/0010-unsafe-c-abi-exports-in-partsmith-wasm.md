# 0010: `unsafe` on the three C ABI exports of `partsmith-wasm`

Status: Accepted (owner, 2026-11-04), signed off as built with the test helper of the note below

## Context

The architecture allows C++ and `unsafe` only in `partsmith-occt-sys`, and the workspace lint table forbids `unsafe_code` everywhere else. The web core is an Emscripten module with a C ABI (ADR 0009): `partsmith-wasm` exports `ps_alloc`, `ps_free` and `ps_call`. Those exports cannot be written without `unsafe`: Rust 2024 requires `#[unsafe(no_mangle)]` on them, which the `unsafe_code` lint reports, and `ps_free` and `ps_call` read and write memory through raw pointers that JavaScript passes in.

`ps_call` also has no session parameter, so the module keeps its one `Session` between calls in a `thread_local!` cell. The core rule is "no global mutable state; every API takes a document or session handle". Separately, `partsmith-occt-sys` serializes STEP reading and writing with a mutex and runs OCCT's one-time data exchange initialization under `std::call_once`, because OCCT's data exchange layer keeps process-wide state (kernel.md).

## Decision

- `partsmith-wasm` defines its own lint table with `unsafe_code = "deny"` instead of the workspace `forbid`, and puts `#[allow(unsafe_code)]` on exactly `ps_alloc`, `ps_free` and `ps_call`. No other item in the crate may use `unsafe`, and every `unsafe` block has a `// SAFETY:` comment stating the caller contract it relies on (protocol.md section 2.10).
- The exports stay thin: they convert between raw pointers and slices and call the safe `call(request: &[u8]) -> Vec<u8>`, which native tests exercise directly.
- The `thread_local!` `Session` in `partsmith-wasm` is host state, like the `Session` a Tauri or Python host owns. The core crates stay free of global mutable state.
- The STEP mutex and the one-time OCCT initialization in `partsmith-occt-sys` guard OCCT's own process-wide state, not Partsmith state, and are invisible to callers. They do not break the "no global mutable state in the core" invariant, which stays as written.
- The AGENTS.md invariant on C++ and `unsafe` names this exception.

## Consequences

`unsafe` outside the kernel crate is limited to three small, reviewed functions, and the lint fails the build if `unsafe` appears anywhere else in `partsmith-wasm`. The web host needs no generated glue (ADR 0009). Any further exception needs a new ADR.

## Note (2026-11-04)

The decision names only the three exports. One more item of the crate uses `unsafe`: `Block`, the test helper in `crates/partsmith-wasm/tests/common/block.rs`. The integration tests and the budget benchmark (`benches/budgets.rs`, which includes `tests/common` with `#[path]`) call the exports through it with raw pointers, as JavaScript does. Five of its methods (`holding`, `call`, `call_without_length_slot`, `bytes` and `Drop::drop`) carry `#[allow(unsafe_code)]`. Each `unsafe` block has a `// SAFETY:` comment that states the part of the caller contract it relies on (protocol.md section 2.10). It is test code and never part of the module. The crate's AGENTS.md names it as the only `unsafe` code outside `src/exports.rs`. The owner signed it off with this ADR on 2026-11-04.

## Note (2026-11-07)

The cancel route of [ADR 0063](0063-cancel-through-a-host-flag-outside-the-session.md) adds no export and no `unsafe` to `partsmith-wasm`. The module imports one JavaScript function, `partsmith_cancel_word`, which partsmith-occt-sys defines in its shim and reads through a C++ function declared in its bridge's existing `unsafe extern "C++"` block (kernel.md section 3.13 item 7). `partsmith-wasm`'s `session_options()` only passes the safe `CancelFlag::page()` to its session, on `wasm32-unknown-emscripten` (protocol.md section 2.10). `-sEXPORTED_FUNCTIONS` keeps `_main` and the three exports, and the module is still linked without shared memory. The decision and its three exports stand unchanged.
