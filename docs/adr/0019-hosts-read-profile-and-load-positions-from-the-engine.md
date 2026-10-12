# 0019: Hosts read the resolved printer profile and load error positions from the engine

Status: Accepted, needs human review

## Context

Two pieces of host logic in `partsmith-cli` repeated what the engine already knows. To report the printer profile in effect, the CLI listed the installed profiles and fell back to `partsmith_print::DEFAULT_PROFILE_ID` itself, which was the only reason it depended on `partsmith-print` and copied the engine's profile resolution (engine.md section 3.7). To report the JSON Pointer of a document that does not load, it validated the text a second time, because `EngineError::Parse` carried only a line and column.

## Decision

- `Session::printer_profile(doc)` returns the id of the document's resolved profile (engine.md sections 2 and 3.7). The CLI reports it as `printer` and no longer depends on `partsmith-print`; architecture.md's crate graph lists `partsmith-cli -> engine, model, protocol`.
- `EngineError::Parse` carries `path`, the JSON Pointer of the field a structure error names, next to `line` and `column`, exactly as `validate` reports the same text (engine.md section 4). The CLI builds its load diagnostic from the error alone. The protocol's `ErrorInfo` is unchanged.

## Consequences

Every host gets the profile in effect and the position of a load error from one place, so they cannot drift from the engine. `EngineError::Parse` gains a field, which code matching it with all fields named must add; the wire format, the file format and every message are unchanged. The dependency change only removes an edge, but it changes architecture.md's crate graph, so it is recorded here for the next gate.
