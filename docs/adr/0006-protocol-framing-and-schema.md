# 0006: Protocol framing and schema-generated types

Status: Accepted

## Context

The UI talks to the core in a Web Worker (web) or in-process through Tauri (desktop). Messages must be typed on both sides, and mesh buffers must move without per-element encoding.

## Decision

- Messages are JSON, defined as Rust types in `partsmith-protocol` deriving `serde` and `schemars::JsonSchema`.
- A frame is `PSM1`, a little-endian `u32` header length, the JSON header, padding to 8 bytes, then raw little-endian buffers listed in the header with offset, length and element type.
- The protocol schema is generated to `docs/format/protocol.schema.json`; the UI generates TypeScript types from it with `json-schema-to-typescript`. CI regenerates both and fails on drift.

## Consequences

One source of truth (the Rust types) for the file format, the protocol and the UI types. Buffers transfer without copies in the worker and with one copy over Tauri IPC.
