# partsmith-cli

The `partsmith` binary: `validate`, `fmt`, `rebuild`, `export`, `schema`. A thin host over `partsmith_engine::Session` (one headless session per invocation): it parses arguments, reads and writes files and formats reports, with no modeling logic.

Spec: `docs/spec/cli.md`. The examples runner in `tests/examples.rs` follows `docs/spec/delivery.md` section 2.8, and `tests/print_examples.rs` holds `docs/spec/print.md` section 6.18 (tests 117 to 123), which rebuild the print check examples.

## Public interface

The command line is the public interface:

```text
partsmith [--version] [--help] <COMMAND>
partsmith validate [--rebuild] [--canonical] [--deny-warnings] [--json] <FILE>...
partsmith fmt [--check] <FILE>...
partsmith rebuild [--checks] [--printer <ID>] [--rotate <AXIS:DEGREES>]... [--deny-warnings] [--json] <FILE>
partsmith export <FILE> -o <PATH> [--format stl|3mf|obj|step|py] [--body <BODY>]... [--per-body] [--ascii]
                 [--chord <MM>] [--angle <DEGREES>] [--printer <ID>] [--rotate <AXIS:DEGREES>]... [--allow-errors] [--json]
partsmith schema [--write | --check] [--out-dir <DIR>] [<NAME>]
```

- Exit codes: 0 success, 1 model problems (or schema drift), 2 usage errors, 3 unreadable or unwritable files, 4 panics (`internal error: <message>`). With several files the largest code wins.
- Reports go to standard output; errors that stop a file go to standard error as `error: <message>`, or as a diagnostic line for a document that does not load. With `--json`, standard output holds one pretty JSON document and every per-file problem is inside it: `validate` as an `io_error` diagnostic, `rebuild` as `{"path", "error"}` and `export` as an `error` member next to `files` and `warnings`, each `error` a diagnostic object (`severity`, `code`, `message`, then `line`, `column` and the JSON Pointer `path` when known).
- Feature messages (cli.md section 2.2): `rebuild` prints `    <severity>[<code>] at <location>: <message>` under its feature, with the location `/features/<index>` followed by the message's field, and `validate --rebuild` prints them as diagnostic lines; under each come one `suggested: <label>` line per action and one `candidate: <q>, <relation>` line per candidate, indented two spaces more. In JSON a message carries `field`, `suggestion`, `actions` (`label` and `kind`), `candidates` (`reference`, `description`, `relation`, `meaning`, `distance`, `score`), `reason`, `root` and `detail`, after `feature` in `validate`; `detail` is never printed as text and no message has a `cause`.
- `validate` fails a file on an error, on a warning whose code says a stored reference no longer means what it meant (`reference_missing`, `reference_ambiguous`, `reference_guessed`, `region_missing`, `region_ambiguous`, `region_remapped`, `plane_fallback`), and on any warning with `--deny-warnings`; info notices never fail it.
- Print findings (cli.md section 3.3): `rebuild --checks` prints `    <severity>[<check>] <body id>: <message>`, followed by `      suggested: <text>` when the finding has a suggestion with words (`partsmith_print::Suggestion::text`, reached through the protocol's `PrintReport`); the JSON carries the suggestion's geometry only.
- `rebuild --json` is the report the examples runner compares with `examples/**/<stem>.expected.json`, and `export --json` the report its runner test 5 compares with the file each run writes, for every example that builds and each of `stl`, `3mf`, `obj` and `step` (delivery.md section 2.8, "Export examples"; nothing is stored or blessed).
- `schema --write` in the repository root regenerates `docs/format/ptsm.schema.json` and `docs/format/protocol.schema.json`; `tests/schema.rs` fails when the checked-in files are stale.

## Internals

- `main.rs`: panic containment (with `RUST_BACKTRACE` set, the standard panic report with its location comes before `internal error:`) and exit status; a broken standard output pipe ends the run with exit 3 and no message. `arguments.rs`: the clap grammar, `Format`, `SchemaAction` and the version text from `engine_info`.
- `commands/`: one module per command; `commands/mod.rs` opens a document with its assets (file and assets first, then `open_document`, then the `--printer` check, as cli.md 3.3 orders them), applies `--printer` for `rebuild` and `export`, and reports the profile in effect from `Session::printer_profile`. A document that does not load is reported with the line, column and JSON Pointer its `EngineError::Parse` carries (`CliError::load`), the diagnostic `validate` reports for the same text, so `fmt`, `rebuild` and `export` name the same position `validate` does.
- `error.rs`: `CliError` and `UsageError` with exit status, code and report line; `status.rs`: `Status`, ordered so the largest wins.
- `files.rs`: inputs (`-` is standard input) read as bytes, where bytes that are not UTF-8 are a `parse_error` load error with the file format's `NotUtf8` message; asset discovery in `assets/`; `write` (atomic, keeping an existing file's permissions) and `create_directory`, whose failures are `CliError::Write` naming the path.
- `output.rs`: standard output and error, human numbers, diagnostic lines and JSON diagnostics (with a feature message's `feature` and members); `rotation.rs`: `--rotate` values to an `Orientation::Matrix`.
- `messages.rs`: `MessageMembers`, what a feature message adds to its diagnostic (field, suggestion, actions as `Action` with `label` and `kind`, candidates without their mesh element, reason, root, detail), with `write_lines` for its action and candidate lines; `validate` builds it from the engine's `FeatureMessage`, and `rebuild` from each status message with the location `partsmith_engine::message_location` gives it and the candidates `partsmith_engine::message_candidates` picks out of the feature's repairs.
- `test_support.rs` (unit tests only): `BAD_ID`, `EMPTY`, `session()`, `id()`, `capture` (runs a writer on memory and returns what it wrote), `pick_edge_again()` and `candidate(query, relation)`.

## Commands

```sh
export OCCT_ROOT=/opt/occt/7.9.3/native
cargo test -p partsmith-cli
cargo test -p partsmith-cli --bins                 # unit tests only
cargo test -p partsmith-cli --test integration validate::   # one acceptance test file, a module of tests/integration.rs
PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test integration examples::   # regenerate examples' expected results, then run again without it
cargo clippy -p partsmith-cli --all-targets -- -D warnings
cargo fmt -p partsmith-cli
cargo run -p partsmith-cli -- schema --write       # regenerate docs/format/*.schema.json
```

## Rules

- Acceptance tests in `tests/` come from the spec; do not weaken them. `tests/regressions.rs` holds end-to-end tests for spec behavior that section 6 does not test.
- `tests/feature_messages.rs` (tests 55 to 62) runs documents of the reference stability corpus after their edits, which `tests/corpus` (a module of the test binary, beside `tests/common`) writes with the engine: each setup's references picked in a headless session as the corpus's "Picked references" rule says, then the edit applied as one `apply_edits`.
- Hosts contain no modeling logic: anything a model needs belongs in the engine.
