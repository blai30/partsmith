# partsmith-cli

The `partsmith` binary: `validate`, `fmt`, `rebuild`, `export`, `schema`. A thin host over `partsmith_engine::Session` (one headless session per invocation): it parses arguments, reads and writes files and formats reports, with no modeling logic.

Spec: `docs/spec/cli.md`. The examples runner in `tests/examples.rs` follows `docs/spec/delivery.md` section 2.8.

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
- Reports go to standard output; errors that stop a file go to standard error as `error: <message>`, or as a diagnostic line for a document that does not load. With `--json`, standard output holds one pretty JSON document and every per-file problem is inside it: `validate` as an `io_error` diagnostic, `rebuild` as `{"path", "error"}` and `export` as an `error` member next to `files` and `warnings`, each `error` a diagnostic object (`severity`, `code`, `message`, `line`, `column`).
- `rebuild --json` is the report the examples runner compares with `examples/**/<stem>.expected.json`.
- `schema --write` in the repository root regenerates `docs/format/ptsm.schema.json` and `docs/format/protocol.schema.json`; `tests/schema.rs` fails when the checked-in files are stale.

## Internals

- `main.rs`: panic containment and exit status; `arguments.rs`: the clap grammar, `Format`, `SchemaAction` and the version text from `engine_info`.
- `commands/`: one module per command; `commands/mod.rs` opens a document with its assets and applies `--printer` for `rebuild` and `export`.
- `error.rs`: `CliError` and `UsageError` with exit status, code and report line; `status.rs`: `Status`, ordered so the largest wins.
- `files.rs`: inputs (`-` is standard input), asset discovery in `assets/`, atomic writes that keep an existing file's permissions.
- `output.rs`: standard output and error, human numbers, diagnostic lines, JSON diagnostics; `rotation.rs`: `--rotate` values to an `Orientation::Matrix`.

## Commands

```sh
export OCCT_ROOT=/opt/occt/7.9.3/native
cargo test -p partsmith-cli
cargo test -p partsmith-cli --bins                 # unit tests only
cargo test -p partsmith-cli --test validate        # one acceptance test file
PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test examples   # regenerate examples' expected results
cargo clippy -p partsmith-cli --all-targets -- -D warnings
cargo fmt -p partsmith-cli
cargo run -p partsmith-cli -- schema --write       # regenerate docs/format/*.schema.json
```

## Rules

- Acceptance tests in `tests/` come from the spec; do not weaken them.
- Hosts contain no modeling logic: anything a model needs belongs in the engine.
