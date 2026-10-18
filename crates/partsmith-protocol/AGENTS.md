# partsmith-protocol

The message protocol between the UI (and every host) and the core: `Request`, `Response`, `Command`, `CommandResult`, `Event`, the binary `PSM1` frame codec, error codes, `PROTOCOL_VERSION` and the generated JSON Schema.

Spec (the definition of done): `docs/spec/protocol.md`. Acceptance tests in `tests/` come from its section 6; do not weaken them. ADR 0006 records the framing and schema decision.

## Public interface (crate root)

- Constants: `PROTOCOL_VERSION` (1), `MAGIC` (`PSM1`), `MAX_HEADER_LENGTH` (64 MiB), `MAX_SAFE_INTEGER`.
- Frames: `Frame`, `FrameHeader`, `ResponseFrame`, `BufferInfo`, `ElementType`, `Buffer` (`element_type`, `len`, `is_empty`, `byte_length`, `as_u8`, `as_u32`, `as_f32`), `BufferIndex`; `encode_frame`, `encode_request_frame`, `encode_response_frame`, `error_response_frame` (never fails; cuts messages over 8 MiB); `decode_frame`, `decode_request_frame`, `decode_response_frame`; `FrameError` (`code`, `request_id`).
- Envelope: `Message` (`id`), `Request`, `Response`, `Outcome`; errors `ErrorInfo` (`new`; `detail` and `actions`, the latter omitted when empty), `ErrorCode` (the 40 codes of section 4, `stale_fix` included), `Span` (`From<partsmith_model::Span>`).
- Commands and results: `Command` (33 commands, `apply_fix` and `set_document_units` included, the latter's `keep_sizes` false when absent; `list_uses` arrives in phase 4 of the reference stability plan; `type_name`, its wire `type`), `CommandResult` (one per command), `CatalogEntry`, `MeasureTarget`, `Measurement`, `ExportFormat`, `Resolution`, `ResolutionStatus`.
- Events: `Event` (6 types; `DocumentChanged` always writes `bare_numbers`, last), `FeatureStatus`, `FeatureState`, `Repair` (with `description`, `candidates` and `ghost`), `RepairCandidate`, `RegionCandidate`, `ParameterValue`, `MeshBuffers`, `PrintReport`, `BodyPrint`.
- Diagnostics (ADR 0024): `Diagnostic` (`new`; `actions`, omitted when empty, `cause`, `detail`, `root`, `reason`), `Cause`, `FixAction`, `FixKind` (`apply`, `pick`, `show`, `undo`, `finish_transaction`), `FixRef` (rejects unknown fields everywhere, since `apply_fix` sends it back); loss reasons `LossReason`, `Removal`, `Absence`, `InstanceIndex`; ghosts `Ghost`, `GhostKind`, `GhostFrame`.
- Sketch solutions: `SketchSolution` (with `messages`, always written; the engine fills it from phase 3 of the reference stability plan), `SolvedEntity`, `SolvedPoint`, `ConstraintState`, `ConstraintSolveState`, `RegionInfo`, `SketchDisplay`.
- Shared values: `DocHandle`, `Vec2`, `Vec3`, `Axis`, `Layer`, `Pick` (with the optional `body` of headless picks), `MeshElement`, `SurfaceKind`, `CurveKind`, `ElementRef`, `ElementDescriptor` (with `fallback` and `axis_direction`), `Orientation`, `TessellationQuality`, `AssetData`, `ExportFile`.
- Re-exports of core types the protocol carries: `BoundingBox`, `TessellationTolerance` (mesh), `ElementKind` (naming), `Severity` (print), `Placement`, `SketchStatus`, `EntityState` (sketch).
- Schema: `protocol_schema()`, the draft 2020-12 schema of `FrameHeader` that `partsmith schema --write` writes to `docs/format/protocol.schema.json`.

## Internals

- `src/frame/`: `buffer.rs` (buffer types, little-endian element reads and writes), `encode.rs` (borrowed `MessageRef` and `HeaderRef` so encoding never clones messages; the header is serialized in place after the prefix, and its length is patched in afterwards, which saves a separate header buffer and its copy into the frame), `decode.rs` (the twelve checks of section 3.2 in order: the header is parsed to a `serde_json::Value` first so version and layout are checked before messages are typed). The header parse (`UniqueKeys`) refuses any object that repeats a key, as `InvalidMessage` without a request id, because a plain `Value` would keep only the last value and hide the repeat from serde. Request ids above `MAX_SAFE_INTEGER` are never put in a `FrameError`, so a host answers them with id 0.
- `src/strict.rs`: `no_fields`, put on the unit variants of request enums (`Command`, `ExportFormat`) because serde ignores unknown fields of unit variants in internally tagged enums. Also the request side of types that results and events share: `Orientation`, `AssetData` and partsmith-mesh's `TessellationTolerance` accept unknown fields, and the request fields holding them use strict private mirrors (`deserialize_with`) and closed schemas (`schema_with = "strict::closed..."`, `allOf` around the `$ref` plus `unevaluatedProperties: false`, which keeps the generated TypeScript type a reference).
- `src/message.rs`: two schemars transforms keep `Request` a named `$defs` entry while the `request` variant of `Message` stays closed (`unevaluatedProperties: false`).
- Modules: `shared.rs` (section 2.4's values), `diagnostic.rs` (diagnostics, fixes and causes), `loss.rs` (loss reasons), `ghost.rs`, `command.rs`, `event.rs` (with repairs and their candidates), `solution.rs`, `error.rs`.
- Request types reject unknown fields (`deny_unknown_fields`, or `strict.rs` for shared types); result and event types accept them, so additive changes stay compatible (section 3.6). `FixRef` is the one exception: it travels in results and in `apply_fix` and rejects unknown fields in both. Core types keep the strictness their own specs give them; `partsmith_model::Edit` accepts unknown fields in `apply_edits` until model.md decides otherwise.

## Commands

```sh
cargo test -p partsmith-protocol
cargo clippy -p partsmith-protocol --all-targets -- -D warnings
cargo fmt -p partsmith-protocol
# Section 5 native budgets (benches/budgets.rs); fails when one is exceeded
cargo bench -p partsmith-protocol --bench budgets
```

Test 45 compares `protocol_schema()` with the checked-in `docs/format/protocol.schema.json`; regenerate it with `cargo run -p partsmith-cli -- schema --write` after any type change.

## Rules

- Any breaking change bumps `PROTOCOL_VERSION` and needs human review; any schema change needs human review (AGENTS.md).
- Dependencies: the five core crates it carries types from (`partsmith-model`, `partsmith-mesh`, `partsmith-sketch`, `partsmith-naming`, `partsmith-print`), plus `serde`, `serde_json`, `schemars` and `thiserror`. No `unsafe`, no global state.
