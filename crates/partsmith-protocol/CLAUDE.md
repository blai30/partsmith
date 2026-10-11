# partsmith-protocol

`Request`, `Response`, `Command`, `Event`, binary frame encoding and decoding, `PROTOCOL_VERSION`, JSON Schema generation.

Spec: `docs/spec/protocol.md`.

## Commands

```sh
cargo test -p partsmith-protocol
cargo clippy -p partsmith-protocol --all-targets -- -D warnings
```

## Rules

- Any breaking change bumps `PROTOCOL_VERSION` and needs human review.
