# 0009: Web core as an Emscripten module with a C ABI

Status: Accepted

## Context

OCCT requires Emscripten, so the web core targets `wasm32-unknown-emscripten`, where `wasm-bindgen` is not available.

## Decision

`partsmith-wasm` is an Emscripten executable whose runtime stays alive after `main`. It exports `ps_alloc`, `ps_free` and `ps_call`. JavaScript writes a request frame into WebAssembly memory, calls `ps_call`, copies the response frame out, and frees it. The module is built as an ES module for a Web Worker, with memory growth up to 4 GB and native wasm exceptions. A build spike confirmed that Rust 1.97 with Emscripten 4.0.15 links `cxx` C++ code compiled with `-fwasm-exceptions`, that C++ exceptions are caught inside the shim, and that Rust panics unwind. The phase 0 spike then linked the pinned OCCT 7.9.3 static libraries into `partsmith-occt-sys` for `wasm32-unknown-emscripten`, and its tests (a box volume and a translated `Standard_DomainError`) pass under Node, matching the native build.

## Consequences

The same protocol frames flow on web and desktop. The bridge is three functions, so there is no generated glue to maintain.
