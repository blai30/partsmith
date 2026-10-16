# partsmith-protocol

The message protocol between the UI (and every host) and the core: `Request`, `Response`, `Command`, `CommandResult`, `Event`, the binary `PSM1` frame codec, error codes, `PROTOCOL_VERSION` and the generated JSON Schema.

Spec (the definition of done): `docs/spec/protocol.md`. Acceptance tests in `tests/` come from its section 6; do not weaken them. ADR 0006 records the framing and schema decision.

## Public interface (crate root)

- Constants: `PROTOCOL_VERSION` (1), `MAGIC` (`PSM1`), `MAX_HEADER_LENGTH` (64 MiB), `MAX_SAFE_INTEGER`.
- Frames: `Frame`, `FrameHeader`, `ResponseFrame`, `BufferInfo`, `ElementType`, `Buffer` (`element_type`, `len`, `is_empty`, `byte_length`, `as_u8`, `as_u32`, `as_f32`), `BufferIndex`; `encode_frame`, `encode_request_frame`, `encode_response_frame`, `error_response_frame` (never fails; cuts messages over 8 MiB); `decode_frame`, `decode_request_frame`, `decode_response_frame`; `FrameError` (`code`, `request_id`).
- Envelope: `Message` (`id`), `Request`, `Response`, `Outcome`; errors `ErrorInfo` (`new`), `ErrorCode` (the 39 codes of section 4), `Span` (`From<partsmith_model::Span>`).
- Commands and results: `Command` (31 commands), `CommandResult` (one per command), `CatalogEntry`, `MeasureTarget`, `Measurement`, `ExportFormat`, `Resolution`, `ResolutionStatus`.
- Events: `Event` (6 types), `FeatureStatus`, `FeatureState`, `Repair`, `ParameterValue`, `MeshBuffers`, `PrintReport`, `BodyPrint`.
- Sketch solutions: `SketchSolution`, `SolvedEntity`, `SolvedPoint`, `ConstraintState`, `ConstraintSolveState`, `RegionInfo`, `SketchDisplay`.
- Shared values: `DocHandle`, `Vec2`, `Vec3`, `Axis`, `Layer`, `Pick`, `MeshElement`, `SurfaceKind`, `CurveKind`, `ElementRef`, `ElementDescriptor`, `Orientation`, `TessellationQuality`, `Diagnostic`, `AssetData`, `ExportFile`.
- Re-exports of core types the protocol carries: `BoundingBox`, `TessellationTolerance` (mesh), `ElementKind` (naming), `Severity` (print), `Placement`, `SketchStatus`, `EntityState` (sketch).
- Schema: `protocol_schema()`, the draft 2020-12 schema of `FrameHeader` that `partsmith schema --write` writes to `docs/format/protocol.schema.json`.

## Internals

- `src/frame/`: `buffer.rs` (buffer types, little-endian element reads and writes), `encode.rs` (borrowed `MessageRef` and `HeaderRef` so encoding never clones messages; one zeroed allocation, then header and buffers copied in), `decode.rs` (the twelve checks of section 3.2 in order: the header is parsed to a `serde_json::Value` first so version and layout are checked before messages are typed). The header parse (`UniqueKeys`) refuses any object that repeats a key, as `InvalidMessage` without a request id, because a plain `Value` would keep only the last value and hide the repeat from serde. Request ids above `MAX_SAFE_INTEGER` are never put in a `FrameError`, so a host answers them with id 0.
- `src/strict.rs`: `no_fields`, put on the unit variants of request enums (`Command`, `ExportFormat`) because serde ignores unknown fields of unit variants in internally tagged enums. Also the request side of types that results and events share: `Orientation`, `AssetData` and partsmith-mesh's `TessellationTolerance` accept unknown fields, and the request fields holding them use strict private mirrors (`deserialize_with`) and closed schemas (`schema_with = "strict::closed..."`, `allOf` around the `$ref` plus `unevaluatedProperties: false`, which keeps the generated TypeScript type a reference).
- `src/message.rs`: two schemars transforms keep `Request` a named `$defs` entry while the `request` variant of `Message` stays closed (`unevaluatedProperties: false`).
- Request types reject unknown fields (`deny_unknown_fields`, or `strict.rs` for shared types); result and event types accept them, so additive changes stay compatible (section 3.6). Core types keep the strictness their own specs give them; `partsmith_model::Edit` accepts unknown fields in `apply_edits` until model.md decides otherwise.

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
