# Architecture decision records

Every lasting decision is recorded here, one file per decision, numbered in order. Read the ADRs for an area before changing it, and write a new ADR when you change a decision (mark the old one superseded rather than editing its decision).

Format: title, status (proposed, accepted, superseded by NNNN), context, decision, consequences. Decisions marked *needs human review* change something the technical design proposed and wait for sign-off at the next milestone gate.

| ADR | Decision | Status |
| --- | --- | --- |
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted |
| [0002](0002-react-for-the-ui.md) | React for the UI | Accepted |
| [0003](0003-pin-occt-and-build-from-source.md) | Pin OCCT 7.9.3, built from source as static libraries | Accepted |
| [0004](0004-native-rust-sketch-solver.md) | Native Rust sketch solver instead of PlaneGCS | Accepted, needs human review |
| [0005](0005-feature-registry-with-generic-params.md) | Feature registry with generically stored params | Accepted |
| [0006](0006-protocol-framing-and-schema.md) | Protocol: JSON header plus binary buffers, schema-generated types | Accepted |
| [0007](0007-webgl2-renderer-first.md) | WebGL2 renderer first, WebGPU behind the same interface | Accepted, needs human review |
| [0008](0008-short-random-ids.md) | Short random ids with kind prefixes | Accepted, needs human review |
| [0009](0009-emscripten-c-abi-bridge.md) | Web core as an Emscripten module with a C ABI | Accepted |
