# CLI specification

Crate: `partsmith-cli`, binary `partsmith`. The CLI is a thin host over `partsmith_engine::Session` (engine.md): it reads and writes files, parses arguments and formats output. It contains no modeling logic.

## 1. Purpose and scope

The CLI serves makers who script batch exports, CI jobs that validate models and regenerate schemas, and the git workflow (validate after merges, canonical formatting).

**In M1**

- `partsmith validate`: load, check expressions, references, sketches, assets, optionally rebuild and canonical form; a rebuild with a lost, ambiguous or guessed reference fails the file.
- `partsmith fmt`: rewrite files in canonical form, or check that they are.
- `partsmith rebuild`: full rebuild with per-feature status, per-body metrics and optional print checks.
- `partsmith export`: STL, 3MF, OBJ, STEP and Python output.
- `partsmith schema`: print, write or check `ptsm.schema.json` and `protocol.schema.json`.
- Feature messages with their location, then the actions that fix them and the candidates that could replace a lost reference; in JSON with codes, fields, actions, candidates, loss reasons, roots and details ([the error guidelines](../guidelines/errors.md), "Hosts").
- `--version`, `--help`, human and JSON output, fixed exit codes.

**Later (not in M1)**

- `partsmith merge-driver` for git, with validation after merge.
- `partsmith diff` (visual and record-level diff).
- `partsmith import` (STEP or mesh to a new `.ptsm`).
- `.ptsz` pack and unpack.
- Watch mode and slicer handoff (`partsmith send`).
- Running Python scripts (done by `python -m partsmith`, python-api.md).
- Colored output and progress bars.

## 2. Public API

The public API is the command line. `crates/partsmith-cli/src/main.rs` parses arguments with `clap` (derive) and calls the engine's typed API with `SessionOptions::headless()`; one session serves all files of one invocation.

```text
partsmith [--version] [--help] <COMMAND>

partsmith validate [--rebuild] [--canonical] [--deny-warnings] [--json] <FILE>...
partsmith fmt [--check] <FILE>...
partsmith rebuild [--checks] [--printer <ID>] [--rotate <AXIS:DEGREES>]... [--deny-warnings] [--json] <FILE>
partsmith export <FILE> --output <PATH> [--format <FORMAT>] [--body <BODY>]... [--per-body] [--ascii]
                 [--chord <MM>] [--angle <DEGREES>] [--printer <ID>] [--rotate <AXIS:DEGREES>]...
                 [--allow-errors] [--json]
partsmith schema [--write | --check] [--out-dir <DIR>] [<NAME>]
```

| Option | Applies to | Meaning |
| --- | --- | --- |
| `<FILE>` | all but `schema` | A `.ptsm` path. `-` means standard input for `validate` and `fmt` (at most once). |
| `--rebuild` | `validate` | Also rebuild geometry and report every feature message, with its actions and candidates (3.1). |
| `--canonical` | `validate` | Report `not_canonical` when the file is not in canonical form. |
| `--deny-warnings` | `validate`, `rebuild` | Treat every warning as a failure for the exit code. `validate` fails on a lost, ambiguous or guessed reference without it (3.1). |
| `--json` | `validate`, `rebuild`, `export` | Print one JSON document on standard output instead of text. |
| `--check` | `fmt`, `schema` | Do not write; fail when a file would change. |
| `--checks` | `rebuild` | Also run print analysis and report findings. |
| `--printer <ID>` | `rebuild`, `export` | Use this printer profile instead of the document's `printer` (for `printer.*` values, tessellation and checks). Not saved. |
| `--rotate <AXIS:DEGREES>` | `rebuild`, `export` | Rotation about the model axis `x`, `y` or `z` (case-insensitive) by a finite number of degrees, for example `x:90`. Repeatable; each rotation applies after the previous ones, about fixed model axes. Gives an `Orientation::Matrix` for checks and export. |
| `-o`, `--output <PATH>` | `export` | Output file, or output directory with `--per-body`. Required. |
| `--format <FORMAT>` | `export` | `stl`, `3mf`, `obj`, `step` or `py`. Default: from the output extension. |
| `--body <BODY>` | `export` | Export only this body, by exact id or else exact name. Repeatable. |
| `--per-body` | `export` | One file per body (STL only). |
| `--ascii` | `export` | ASCII STL (STL only). |
| `--chord <MM>` | `export` | Chord deviation in mm, positive (mesh formats only). |
| `--angle <DEGREES>` | `export` | Angular deviation in degrees, greater than 0 and at most 90 (mesh formats only). |
| `--allow-errors` | `export` | Export even when features failed. |
| `--write` | `schema` | Write both schema files. |
| `--out-dir <DIR>` | `schema` | Directory for `--write` and `--check`. Default `docs/format`, relative to the current directory. |
| `<NAME>` | `schema` | `ptsm` or `protocol`: print that schema to standard output. |

`partsmith --version` prints exactly `partsmith 0.1.0 (protocol 1, format 2, OCCT 7.9.3)` followed by a newline, with the engine version, `PROTOCOL_VERSION`, the format version and the kernel string from `engine_info`.

### 2.1 Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Success. |
| 1 | The model has problems: validation errors, warnings of a lost, ambiguous or guessed reference (3.1), other warnings with `--deny-warnings`, a file that is not canonical (`fmt --check`, `validate --canonical`), failed features in `rebuild` or `export` (without `--allow-errors`), an unknown or ambiguous `--body`, an unknown `--printer`, an engine error while exporting, or schema drift (`schema --check`). |
| 2 | Usage error: unknown command or option, missing or invalid value, or an option that does not apply to the chosen format. Printed by clap or in the same style. |
| 3 | Input or output error: a file or directory cannot be read or written. |
| 4 | Internal error: a panic. `main` installs `std::panic::catch_unwind` around the run and prints `internal error: <message>` to standard error. |

With several files, each file is processed independently and the exit code is the largest code any file produced.

### 2.2 Output conventions

- Requested output (reports, formatted text from `fmt -`, schemas, written paths) goes to standard output. Errors that stop a file or the command (I/O, usage, engine errors) go to standard error as `error: <message>`. Warnings that are not part of a report go to standard error as `warning[<code>]: <message>`.
- With `--json`, standard output holds exactly one JSON document, pretty-printed with 2-space indentation and a trailing newline, keys in the order shown below, numbers in `serde_json` notation. Every per-file problem, including I/O errors (code `io_error`), is inside the document; standard error stays empty except for usage errors and panics.
- Paths are printed exactly as given on the command line; standard input is shown as `<stdin>`.
- Human numbers are printed with at most 6 decimal places, trailing zeros and a trailing point removed, and `-0` printed as `0` (`24000`, `5163.946891`, `0.5`).
- Human diagnostic lines have one of these forms:
  - `<path>:<line>:<column>: <severity>[<code>]: <message>` when the line and column are known.
  - `<path>:<line>: <severity>[<code>]: <message>` when only the line is known (for example `not_canonical`, engine.md section 3.19 step 9).
  - `<path>: <severity>[<code>] at <pointer>: <message>` when only the JSON Pointer is known.
  - `<path>: <severity>[<code>]: <message>` otherwise.
  - `<severity>` is `error`, `warning` or `info`.
- **Feature messages.** A feature message is one of the error, warnings and info notices that recompute rendered for a feature (engine.md section 3.4, `FeatureStatus`), with the same text in every host. Its *location* is the JSON Pointer of the field it is about in the document: `/features/<index>` (the feature's position in the timeline) followed by the message's `field`, for example `/features/3/edges/0`, or `/features/<index>` alone when it has no field. Its *candidates* are those of the feature's element repairs (protocol.md 2.7, `Repair::Element`) whose `path` is the message's `field` or lies under it (`/edges/0` under `/edges`), in repair order, then candidate order: the possible replacements of a lost, ambiguous, guessed or widened reference. Region candidates reach the user only as actions.
- **Actions and candidates.** Under the line of a feature message, indented two spaces more, come one line `suggested: <label>` per action of the message, in the order offered, then one line `candidate: <q>, <relation>` per candidate, where `<q>` is the candidate reference's query (the element name a reference to it stores) and `<relation>` says what it keeps and how far it is, for example `candidate: edge(f-box1/front|f-ext2/lateral/e-1), on the front side of 'Box 1', 5 mm from where it was`. Labels and relations are the protocol's (engine.md sections 3.4 and 3.22). The CLI opens every document fresh, so no message has a cause line (engine.md section 3.21); a message's `detail` appears only in JSON.

### 2.3 Assets

For a document at path `P`, the CLI reads every regular file in `P`'s directory under `assets/` whose name matches `^[0-9a-f]{64}\.[a-z0-9]+$` and passes them to the engine, which uses only the referenced ones (engine.md 3.14). A missing `assets/` directory is not an error. A document read from standard input gets no assets.

## 3. Behavior

### 3.1 `validate`

1. Reads each file (exit 3 on failure, reported as `error: cannot read "<path>": <os message>`), then calls `Session::validate` with `rebuild` and `canonical` from the flags. With `--rebuild`, its rebuild step reports every feature message (2.2) as one diagnostic, features in timeline order and each feature's messages in their order (error, warnings, then info notices): the message's own `code`, `severity` and text, its location as `path`, and the members of a message in `rebuild` (3.3). Element references resolve only against built geometry, so lost references are reported only with `--rebuild`.
2. Human output: every diagnostic of the file in report order, in the formats of 2.2, each feature message followed by its actions and candidates (2.2); a file without diagnostics prints `<path>: ok`.
3. A file fails when it has an error diagnostic, a warning of a lost, ambiguous or guessed reference, or, with `--deny-warnings`, any warning. The warnings of a lost, ambiguous or guessed reference are those with the codes `reference_missing`, `reference_ambiguous`, `reference_guessed`, `region_missing`, `region_ambiguous`, `region_remapped` and `plane_fallback`: the document rebuilds, but a stored reference no longer means what it meant, so the file is not ok. Other warnings (a widened reference, a resized region, a feature that changes nothing) fail only with `--deny-warnings`, and info notices never fail.
4. JSON output:

```json
{
  "files": [
    {
      "path": "box.ptsm",
      "ok": true,
      "diagnostics": []
    },
    {
      "path": "bad.ptsm",
      "ok": false,
      "diagnostics": [
        {
          "severity": "error",
          "code": "parse_error",
          "message": "The file is not a valid Partsmith document: EOF while parsing an object at line 1 column 23.",
          "line": 1,
          "column": 23
        }
      ]
    }
  ]
}
```

Diagnostic objects have `severity`, `code`, `message`, then `line`, `column` and `path` (JSON Pointer) when known. A feature message then has `feature` (its id), `field`, `suggestion`, `actions`, `candidates`, `reason`, `root` and `detail`, as a message of `rebuild` has them (3.3). Absent fields and empty lists are omitted. `ok` is false exactly when the file fails (step 3).

### 3.2 `fmt`

1. For each file, calls `Session::format` (engine.md 2) to compute the canonical text. A file that does not load prints the load error as in 2.2 and is left untouched (exit 1).
2. Without `--check`: when the canonical text differs from the file, writes it atomically (a temporary file in the same directory, then rename), keeping the file's permissions, and prints `formatted <path>`. Unchanged files print nothing and are not rewritten (their modification time is unchanged).
3. With `--check`: writes nothing, prints `<path>` for each file that is not canonical, and exits 1 if there is any.
4. With `-`: reads standard input and writes the canonical text to standard output (nothing else). With `--check`, writes nothing and exits 1 when the input is not canonical.
5. `fmt` keeps unknown feature types and invalid params exactly as loaded (in their original key order), so formatting never loses data.

### 3.3 `rebuild`

1. Opens the file with its assets, applies `--printer` as an in-memory `set_printer` when given (exit 1 with `error: Printer profile "<id>" is not installed.` when unknown), and recomputes.
2. Measures every body with geometry (engine `measure`).
3. With `--checks`, runs `analyze_print` with the effective profile and the orientation from `--rotate` (default `as_modeled`) on the default bodies.
4. Human output:

```text
box.ptsm
  f-box1 box ok
  b-1 "Block": volume 24000 mm^3, area 5200 mm^2, bounding box (-10, -15, 0) to (10, 15, 40) mm, 6 faces, 12 edges, 8 vertices
```

One line per feature in timeline order: `  <id> <type> <status>`, followed by one line per message `    <severity>[<code>] at <location>: <message>`, with the message's location (2.2), each message followed by its actions and candidates indented 6 spaces (2.2). For example, the lines of the fillet of corpus R16 (test 55), with the lines left out here marked `...`:

```text
  f-fil1 fillet error
    error[references_not_found] at /features/3/edges: The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet.
      suggested: Use the front edge of the cut made by 'Extrude 1'
      suggested: Use the top edge of the cut made by 'Extrude 1'
      ...
      suggested: Pick an edge again
      candidate: edge(f-box1/front|f-ext2/lateral/e-1), on the front side of 'Box 1', 5 mm from where it was
      candidate: edge(f-box1/top|f-ext2/lateral/e-2), on the top of 'Box 1', 5 mm from where it was
      ...
```

Then one line per body in document order; a body without geometry prints `  <id> "<name>": no geometry`. With `--checks`, then `  print health: <summary label> (<profile id>)` and one line per finding `    <severity>[<check>] <body id>: <message>`, in body order, then check order, then the check's own order.

5. JSON output:

```json
{
  "path": "box.ptsm",
  "printer": "generic-fdm",
  "features": [
    {
      "id": "f-box1",
      "type": "box",
      "status": "ok",
      "messages": []
    }
  ],
  "parameters": [
    {
      "id": "p-h",
      "name": "height",
      "value": 40.0,
      "dimension": {
        "length": 1,
        "angle": 0
      }
    }
  ],
  "bodies": [
    {
      "id": "b-1",
      "name": "Block",
      "volume": 24000.0,
      "area": 5200.0,
      "center_of_mass": [
        0.0,
        0.0,
        20.0
      ],
      "bounding_box": {
        "min": [
          -10.0,
          -15.0,
          0.0
        ],
        "max": [
          10.0,
          15.0,
          40.0
        ]
      },
      "faces": 6,
      "edges": 12,
      "vertices": 8
    }
  ]
}
```

`printer` is the effective profile id, the document's resolved profile after `--printer` (engine `printer_profile`, engine.md 3.7): the `--printer` value, else the document's `printer` when it is installed, else the default profile. `messages` holds the feature's messages (2.2), each an object with these members in this order, each omitted when absent or empty:

- `severity`, `code`, `message`, `field` and `suggestion`: those of the protocol's `Diagnostic` (protocol.md 2.4).
- `actions`: the message's actions in the order offered, each `{"label": <label>, "kind": <kind>}` with the `FixAction`'s label and the `kind` of its `run` (`apply`, `pick` or `show`; engine.md section 3.22), for example `{"label": "Pick an edge again", "kind": "pick"}`.
- `candidates`: the message's candidates (2.2), each with the members `reference`, `description`, `relation`, `meaning`, `distance` and `score` of the protocol's `RepairCandidate` (protocol.md 2.7, filled by engine.md section 3.4), in that order; the score is for reports and is never printed in the text output. For example `{"reference": {"q": "edge(f-box1/front|f-ext2/lateral/e-1)", "fp": "line|n=1,0,0|c=0,-10,15|l=20|adj=2,2"}, "description": "the front edge of the cut made by 'Extrude 1'", "relation": "on the front side of 'Box 1', 5 mm from where it was", "meaning": true, "distance": 5.0, "score": 0.9139780823055461}`.
- `reason`: why the reference the message names was lost, or which feature a followed or widened reference followed (recompute.md section 3.10, `Message::reason`): an object whose `kind` is the `LossReason` variant's name in snake case, followed by the variant's fields in declaration order, ids and element names as strings, `how` as an object of the same form for the `Removal` (`absence` `through` or `full_turn`), `instance` as `{"first": <n>, "second": <n>}`, and a field that is None omitted. For example `{"kind": "removed", "by": "f-ext2", "how": {"kind": "cut_away"}}`, `{"kind": "removed", "by": "f-ext2", "how": {"kind": "no_longer_made", "absence": "through"}}`, `{"kind": "other_body", "body": "b-2", "by": "f-split1", "by_suppressed": false}` or `{"kind": "followed", "by": "f-sk1"}`.
- `root`: the first failing feature of a chain, or the sketch that lost its face (recompute.md `Message::root`).
- `detail`: the technical text for reports (features.md section 2.8, `detail`), never printed in the text output.

A message never has a `cause` here: the CLI opens the document fresh, so no edit of the session started it (engine.md section 3.21). For example, the extrude that depends on a failed datum plane in corpus R54 (test 61), shown compactly:

```json
{"severity": "error", "code": "upstream_failed", "message": "'Sketch 2' cannot be built because 'Datum plane 1' has an error, so this extrude cannot use it either. Fix 'Datum plane 1' first.", "actions": [{"label": "Show 'Datum plane 1'", "kind": "show"}], "root": "f-dp1"}
```

A parameter that failed to evaluate has `error` instead of `value` and `dimension`. A body without geometry has only `id`, `name` and `"empty": true`. With `--checks` a final key `checks` holds the protocol `PrintReport` (protocol.md 2.7: `profile`, `orientation`, `summary`, `bodies` with each body's `transform` and print.md `report`), without `mesh_id` and `overhang`.

6. Exit 1 when any feature has status `error`, or `warning` with `--deny-warnings`. Print findings never change the exit code.

### 3.4 `export`

1. Format: `--format`, else the output extension (case-insensitive): `.stl`, `.3mf`, `.obj`, `.step` or `.stp`, `.py`. Neither gives exit 2: `error: Cannot tell the export format from "<path>". Use --format.`
2. Option checks (exit 2): `--ascii` only with STL; `--per-body` only with STL; `--chord`, `--angle` only with mesh formats; `--body`, `--chord`, `--angle`, `--rotate`, `--per-body` and `--ascii` are invalid with `py`. Invalid numbers: `--chord` not finite and positive, `--angle` not in (0, 90].
3. Opens the file with its assets, applies `--printer` as in `rebuild`, and recomputes. If any feature has status `error` and `--allow-errors` is absent, prints `error: <path>: <n> feature(s) failed (<ids, comma-separated>). Fix them or pass --allow-errors to export anyway.` and exits 1.
4. `--body` values are matched to bodies by exact id, else by exact name. No match: `error: There is no body named or with id "<value>".`; two bodies with that name: `error: Two bodies are named "<value>". Use the body id.`; both exit 1.
5. Calls `Session::export` (mesh and STEP formats) with `ExportRequest { format, bodies, tolerance, orientation }`, where `tolerance` is present when `--chord` or `--angle` is given (the other value from `Session::tolerance(doc, Standard)`) and `orientation` is present when `--rotate` is given. For `py`, calls `export_python`; with `--json` the file's `media_type` is `text/x-python`.
6. Without `--per-body`, writes the single file to `--output`; the parent directory must exist (exit 3 otherwise). With `--per-body`, `--output` is a directory, created with its parents when missing, and each engine file is written inside it under its engine name. Existing files are overwritten. Writes are atomic (temporary file, then rename).
7. Human output: each written path on its own line on standard output; engine warnings on standard error as `warning[<code>]: <message>`.
8. JSON output:

```json
{
  "files": [
    {
      "path": "out/model.stl",
      "media_type": "model/stl",
      "bytes": 684
    }
  ],
  "warnings": []
}
```

9. Engine errors (`nothing_to_export`, `body_empty`, `export_failed`) print `error: <message>` and exit 1.

### 3.5 `schema`

1. `partsmith schema ptsm` prints `partsmith_engine::ptsm_schema()`; `partsmith schema protocol` prints `partsmith_engine::protocol_schema()`. Output is exactly the schema text (which ends in one newline).
2. `--write` writes `<out-dir>/ptsm.schema.json` and `<out-dir>/protocol.schema.json`, creating `<out-dir>` when missing. For each file it prints `wrote <path>` when the content changed or the file was missing, else `unchanged <path>` and leaves the file untouched.
3. `--check` compares both files with the generated text and prints `<path> is out of date. Run: partsmith schema --write` for each missing or different file; exit 1 if any.
4. `<NAME>` cannot be combined with `--write` or `--check`, and `partsmith schema` without `<NAME>`, `--write` or `--check` is a usage error (exit 2).

### 3.6 General

- Files are processed sequentially in argument order; output for a file is complete before the next file starts.
- The CLI never modifies an input file except through `fmt` without `--check`.
- Output is deterministic: the same inputs give byte-identical standard output, standard error and written files.

## 4. Errors

| Situation | Message (standard error unless `--json`) | Exit |
| --- | --- | --- |
| Unreadable input | `error: cannot read "<path>": <os message>` | 3 |
| Unwritable output | `error: cannot write "<path>": <os message>` | 3 |
| Document does not load | the load diagnostic (2.2): the engine's `parse_error` or `unsupported_format_version` with the line, column and JSON Pointer the error carries (engine.md section 4), the same diagnostic `validate` reports for the text | 1 |
| Unknown `--printer` | `error: Printer profile "<id>" is not installed.` | 1 |
| Unknown `--body` | `error: There is no body named or with id "<value>".` | 1 |
| Ambiguous `--body` | `error: Two bodies are named "<value>". Use the body id.` | 1 |
| Failed features in `export` | `error: <path>: <n> feature(s) failed (<ids>). Fix them or pass --allow-errors to export anyway.` | 1 |
| Engine export error | `error: <engine message>` | 1 |
| Format not inferable | `error: Cannot tell the export format from "<path>". Use --format.` | 2 |
| Option not valid for format | `error: --<option> does not apply to <format> export.` | 2 |
| Invalid `--rotate` | `error: --rotate takes <axis>:<degrees>, for example x:90.` | 2 |
| Second `-` | `error: Standard input can be read only once.` | 2 |
| Schema drift | `<path> is out of date. Run: partsmith schema --write` (standard output) | 1 |
| Panic | `internal error: <message>` | 4 |

## 5. Performance budgets

Release build, reference machine, wall time including process start:

| Command | Budget |
| --- | --- |
| `partsmith --version` | 50 ms |
| `partsmith validate` of a 200 KB document without `--rebuild` | 200 ms |
| `partsmith fmt --check` of a 200 KB document | 150 ms |
| `partsmith schema --check` | 200 ms |
| `partsmith rebuild` and `export` | the engine budgets (engine.md section 5) plus 100 ms |

## 6. Acceptance tests

Tests live in `crates/partsmith-cli/tests/` and run the built binary (`env!("CARGO_BIN_EXE_partsmith")`) with `std::process::Command` in a temporary directory (`tempfile`), copying fixtures from `crates/partsmith-engine/tests/fixtures/`. "Prints" means the exact standard output unless stated.

**General**

1. `partsmith --version` prints `partsmith 0.1.0 (protocol 1, format 2, OCCT 7.9.3)\n` and exits 0.
2. `partsmith` with no arguments exits 2.
3. `partsmith frobnicate` exits 2.

**validate**

4. `partsmith validate box.ptsm` prints `box.ptsm: ok\n` and exits 0.
5. `partsmith validate box.ptsm plate.ptsm` prints `box.ptsm: ok\nplate.ptsm: ok\n`.
6. A file containing `{"format": "partsmith",` gives one line starting `bad.ptsm:1:` and containing `error[parse_error]`, exit 1.
7. `partsmith validate missing.ptsm` prints `error: cannot read "missing.ptsm": ` followed by the OS message on standard error and exits 3.
8. `partsmith validate box.ptsm missing.ptsm bad.ptsm` exits 3 and still prints `box.ptsm: ok` and the `bad.ptsm` diagnostic.
9. `partsmith validate conflict.ptsm` exits 1 with a line containing `error[sketch_conflict]`; with `--rebuild` that line is printed once.
10. `box.ptsm` with `printer` set to `nope` validates with exit 0 and a line containing `warning[unknown_printer_profile]`; with `--deny-warnings` it exits 1.
11. `box.ptsm` with `height` set to `width * 2` gives a line that starts with `box.ptsm: error[`, contains `] at /parameters/0/expr: ` and contains `width`; exit 1.
12. `partsmith validate --canonical box.ptsm` exits 0; the same file re-indented with 4 spaces exits 1 with a line containing `error[not_canonical]`.
13. `partsmith validate --json box.ptsm bad.ptsm` prints a JSON document with `files[0]` `{"path": "box.ptsm", "ok": true, "diagnostics": []}` and `files[1].ok` false with a `parse_error` diagnostic that has `line` and `column`, and exits 1.
14. `partsmith validate --json missing.ptsm` prints a document whose only file has `ok` false and a diagnostic with code `io_error`, standard error is empty, exit 3.
15. `partsmith validate - < box.ptsm` prints `<stdin>: ok\n`.
16. `partsmith validate - -` exits 2.
17. A document with an `import` feature whose asset is in `assets/` validates with exit 0; after deleting the asset, it exits 1 with a line containing `error[asset_missing]`.
18. `partsmith validate --rebuild` of `box.ptsm` with the box length set to `-5 mm` exits 1 with a line starting `box.ptsm: error[out_of_range] at /features/0/size/0: Length must be greater than 0 mm` (the box's own message and code, 3.1); without `--rebuild` it exits 0.

**fmt**

19. `partsmith fmt box.ptsm` on the canonical fixture prints nothing, exits 0, and leaves the file's modification time unchanged.
20. On `box.ptsm` re-indented with 4 spaces, `partsmith fmt box.ptsm` prints `formatted box.ptsm\n`, exits 0, and the file then equals the canonical fixture byte for byte.
21. `partsmith fmt --check reindented.ptsm box.ptsm` prints `reindented.ptsm\n`, exits 1, and changes neither file.
22. `partsmith fmt - < reindented.ptsm` prints exactly the canonical fixture text.
23. `partsmith fmt bad.ptsm` exits 1, prints a `parse_error` diagnostic, and leaves `bad.ptsm` unchanged.
24. A document whose `f-box1` has type `wedge_v9` and params in non-canonical order formats with exit 0 and keeps those params and their order.
25. On a read-only directory, `partsmith fmt reindented.ptsm` exits 3 with `error: cannot write "reindented.ptsm": ...`. Skipped when the effective user id is 0 (root ignores directory permissions).

**rebuild**

26. `partsmith rebuild box.ptsm` prints exactly the human example of 3.3 and exits 0.
27. `partsmith rebuild --json box.ptsm` prints the JSON example of 3.3 with the same layout and keys, numbers equal within 1e-9 relative.
28. `partsmith rebuild --json fillet.ptsm` reports `b-1` volume 23965.66370614359 and area 5163.946891450772 within 1e-6 relative, 7 faces, 15 edges, 10 vertices.
29. `partsmith rebuild` of `box.ptsm` with the box length set to `-5 mm` exits 1; its JSON has `f-box1` status `error` and `b-1` with `"empty": true`.
30. `partsmith rebuild --printer nope box.ptsm` exits 1 with `error: Printer profile "nope" is not installed.`
31. `partsmith rebuild --checks --json box.ptsm` has `checks.summary.health` `ok`, `checks.summary.label` `Ready to print` and `checks.orientation` `{"kind": "as_modeled"}`.
32. `partsmith rebuild --checks --rotate x:30 --json box.ptsm` has at least one finding under the `overhangs` check of `b-1`'s report, and exits 0.
33. `partsmith rebuild --rotate x box.ptsm` exits 2.

**export**

34. `partsmith export box.ptsm -o out.stl` writes a 684-byte file, prints `out.stl\n`, exits 0.
35. `partsmith export box.ptsm -o out.stl --ascii` writes a file whose first line starts with `solid` and whose last line starts with `endsolid`, with 12 `facet normal` lines.
36. `partsmith export two-bodies.ptsm -o parts --per-body --format stl` creates `parts/Block.stl` and `parts/Peg.stl` and prints those two paths in that order.
37. `partsmith export two-bodies.ptsm -o peg.stl --body Peg` writes the cylinder only; `--body b-2` gives the same bytes.
38. `--body Nope` exits 1 with `error: There is no body named or with id "Nope".`
39. A document with two bodies named `Part` and `--body Part` exits 1 with the ambiguity message.
40. `partsmith export box.ptsm -o out.3mf` writes a file starting with `50 4b 03 04`.
41. `partsmith export box.ptsm -o out.step`, then a new document importing it (engine `import_step`), measures volume 24000 within 1e-6 relative.
42. `partsmith export box.ptsm -o out.py` writes exactly the script of python-api.md section 6, test 1.
43. `partsmith export box.ptsm -o out.dat` exits 2 with the format message; `--format stl` makes it succeed.
44. `partsmith export box.ptsm -o out.3mf --ascii` exits 2, and so does `partsmith export two-bodies.ptsm -o parts --per-body --format obj` (OBJ always writes one file with one object per body).
45. Exporting `fillet.ptsm` with the fillet radius set to `30 mm` (so it fails) exits 1 with a message naming `f-fil1`; adding `--allow-errors` exits 0 and writes the unfilleted box.
46. `partsmith export circle.ptsm -o coarse.stl --chord 0.5 --angle 45` writes a smaller file than `partsmith export circle.ptsm -o default.stl`; for `box.ptsm` both give 684 bytes, because planar faces do not depend on the tolerance. (The angle is raised too: for a radius-5 circle the default 5 degree angular limit is stricter than either chord, so `--chord 0.5` alone would not change the mesh.)
47. `partsmith export box.ptsm -o missing-dir/out.stl` exits 3.
48. `partsmith export --json box.ptsm -o out.stl` prints `{"files": [{"path": "out.stl", "media_type": "model/stl", "bytes": 684}], "warnings": []}` in the pretty layout of 2.2.
49. Running test 34 twice gives byte-identical files.

**schema**

50. `partsmith schema protocol` prints exactly `partsmith_protocol::protocol_schema()`.
51. `partsmith schema --write --out-dir s` creates `s/ptsm.schema.json` and `s/protocol.schema.json`, prints `wrote s/ptsm.schema.json\nwrote s/protocol.schema.json\n`; running it again prints `unchanged` for both and keeps their modification times.
52. `partsmith schema --check --out-dir s` exits 0 after test 51; after appending a space to `s/ptsm.schema.json` it prints `s/ptsm.schema.json is out of date. Run: partsmith schema --write` and exits 1.
53. `partsmith schema --check` in the repository root exits 0 (the checked-in files match).
54. `partsmith schema` without arguments exits 2; `partsmith schema ptsm --write` exits 2.

**Feature messages, actions and candidates**

Tests 55 to 62 write the documents of scenarios of the reference stability corpus ([reference-stability-corpus.md](../design/reference-stability-corpus.md)) after their edits as `.ptsm` files named after the scenario (`r16.ptsm`), with references picked as the corpus's "Picked references" rule says. Each is the document of the recompute.md test named, which states its message texts, fixes and reasons; these tests check how the CLI prints them.

55. `rebuild` prints actions and candidates (corpus R16, recompute.md test 52, with the rabbet drawn as `e-1` (-11, 15)-(-5, 15), `e-2` (-5, 15)-(-5, 21), `e-3` (-5, 21)-(-11, 21) and `e-4` (-11, 21)-(-11, 15)). `partsmith rebuild r16.ptsm` exits 1 and prints, right after `  f-fil1 fillet error`: the line `    error[references_not_found] at /features/3/edges: The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet.`; six action lines indented 6 spaces, the first two `      suggested: Use the front edge of the cut made by 'Extrude 1'` and `      suggested: Use the top edge of the cut made by 'Extrude 1'` (the two candidates 5 mm from where the edge was, in candidate order: their distances are equal, so their names decide), then one `Use` line for each of the other three candidates, and the last `      suggested: Pick an edge again`; then five candidate lines, the first two `      candidate: edge(f-box1/front|f-ext2/lateral/e-1), on the front side of 'Box 1', 5 mm from where it was` and `      candidate: edge(f-box1/top|f-ext2/lateral/e-2), on the top of 'Box 1', 5 mm from where it was`. No line contains `This started after`, `resolution=` or a score.
56. `rebuild --json` carries the message's members (corpus R16). `features[3].messages[0]` has exactly the members `severity`, `code`, `message`, `field`, `actions`, `candidates`, `reason` and `detail`, in this order: `code` `references_not_found`; `field` `/edges`; six `actions`, each with exactly `label` and `kind`, `kind` `apply` for the five `Use` actions and `pick` for the last; five `candidates`, the first with `reference` `{"q": "edge(f-box1/front|f-ext2/lateral/e-1)", "fp": "line|n=1,0,0|c=0,-10,15|l=20|adj=2,2"}`, `description` `the front edge of the cut made by 'Extrude 1'`, `relation` `on the front side of 'Box 1', 5 mm from where it was`, `meaning` true, `distance` 5 and `score` 0.913978 within 1e-6; `reason` `{"kind": "removed", "by": "f-ext2", "how": {"kind": "cut_away"}}`; and `detail` starting `edges[0] q=edge(f-box1/front|f-box1/top) resolution=missing lost.unmet=true candidates=edge(f-box1/front|f-ext2/lateral/e-1):0.913978,`.
57. `validate` fails on a lost reference (corpus R03, recompute.md test 59). `partsmith validate --rebuild r03.ptsm`, without `--deny-warnings`, exits 1 and prints no `ok` line but the line `r03.ptsm: warning[reference_missing] at /features/2/edges/0: ` followed by the message of recompute.md test 59, then lines `  suggested: <label>` ending with `  suggested: Remove from Edges` and `  suggested: Pick an edge again`, then one line `  candidate: <q>, <relation>` per candidate. `partsmith rebuild r03.ptsm` exits 0, with `  f-fil1 fillet warning`, and 1 with `--deny-warnings`.
58. `validate --json` (corpus R03). `partsmith validate --rebuild --json r03.ptsm` exits 1; `files[0].ok` is false, and the diagnostic of 'Fillet 1' has `severity` `warning`, `code` `reference_missing`, `path` `/features/2/edges/0`, `feature` `f-fil1`, `field` `/edges/0`, `actions` whose last two are `{"label": "Remove from Edges", "kind": "apply"}` and `{"label": "Pick an edge again", "kind": "pick"}`, `reason` `{"kind": "entity_deleted", "feature": "f-ext1", "sketch": "f-sk1", "entity": "e-5"}` and `detail` starting `edges[0] q=edge(f-ext1/end-cap|f-ext1/lateral/e-5) resolution=missing lost.parts=f-ext1/lateral/e-5`, and no `line`, `column`, `root` or `cause`.
59. `validate` fails on a guessed reference (corpus R08 (a) at width 200, with f-fil1's reference stored without `nb`, as a script or a build before format version 2 writes it). `partsmith validate --rebuild r08.ptsm` exits 1 with a line starting `r08.ptsm: warning[reference_guessed] at /features/5/edges/0: `.
60. Warnings that keep the user's intent and notices do not fail `validate`. Corpus R66 (recompute.md test 64): `partsmith validate --rebuild r66.ptsm` prints `r66.ptsm: warning[reference_widened] at /features/2/edges/0: ` followed by the message of corpus R66, whose action lines are `  suggested: Keep all 5`, `  suggested: Use only the back top edge of 'Extrude 1' along the line from (80, 50) to (50, 50) in 'Sketch 1'` and `  suggested: Pick an edge again`, in this order, and exits 0; with `--deny-warnings` it exits 1. Corpus R06 (a) with the document of recompute.md test 41 without its second entry: `partsmith validate --rebuild r06.ptsm` prints `r06.ptsm: info[reference_followed] at /features/2/edges/0: ` followed by the notice, then `  suggested: Keep this match` and `  suggested: Pick an edge again`, and no `ok` line, and exits 0, also with `--deny-warnings`; with `--json`, `files[0].ok` is true.
61. Upstream chains (corpus R54, recompute.md test 46). In `partsmith rebuild --json r54.ptsm`, the message of 'Extrude 2' (f-ext3) is exactly the compact example of 3.3: no `field`, `candidates`, `reason` or `detail`, the action `{"label": "Show 'Datum plane 1'", "kind": "show"}` and `root` `f-dp1`; the text output prints `      suggested: Show 'Datum plane 1'` under its message line.
62. Reasons in JSON (3.3). In `rebuild --json`, the first message of 'Fillet 1' in corpus R12 at distance 30 (recompute.md test 57) has `reason` `{"kind": "removed", "by": "f-ext2", "how": {"kind": "no_longer_made", "absence": "through"}}`; that of 'Chamfer 1' in corpus R31 (a) (recompute.md test 51) `{"kind": "other_body", "body": "b-2", "by": "f-split1", "by_suppressed": false}`; that of 'Fillet 1' in corpus R41 (recompute.md test 61) `{"kind": "copy_gone", "feature": "f-lp1", "instance": {"first": 2, "second": 0}}`; and the notice of corpus R06 (a) (test 60) `{"kind": "followed", "by": "f-sk1"}`. An ambiguous loss (corpus R63 (a), recompute.md test 67) has no `reason`.

## 7. Open issues

1. **Command list.** The architecture names `validate | fmt | rebuild | export | schema`. Print checks are an option of `rebuild` (`--checks`) rather than a separate command, and STEP import is left to the UI and Python in M1.
2. **Asset discovery.** The CLI passes every hash-named file in `assets/` and relies on the engine ignoring unreferenced ones (engine.md 3.14). A large shared asset folder is read fully; a lazy asset source can come later.
3. **`rebuild --json` and expected results.** Resolved by [delivery.md](delivery.md) section 2.8: an example's `<stem>.expected.json` is the `rebuild --checks --json` report without `path` and the findings' triangle lists.
4. **Exit code for feature errors in `validate --rebuild` versus `rebuild`.** Both use exit 1; print findings never affect the exit code. A future `--deny-findings` can change that.
5. **Feature messages in `validate`.** Resolved by engine.md section 3.19 step 8: the rebuild step reports every feature message, info notices included, with its own code, its rendered text and its location, and its `feature` member (`FeatureMessage`) carries the projected message with its actions, reason, root and detail and the candidates of its element repairs, as `FeatureStatus` projects them (engine.md section 3.4), so that `validate --rebuild` and `rebuild` print the same message (tests 18 and 57 to 60).
6. **Loss reasons reach the CLI.** Resolved by protocol.md section 2.4: `Diagnostic::reason` carries recompute's `Message::reason` in the wire form section 3.3 prints, and engine.md section 3.4 projects it (protocol.md open issue 4, recompute.md open issue 11).
7. **Lost references without `--rebuild`.** Element references resolve only against built geometry, so `validate` without `--rebuild` cannot tell that a fillet's edge is gone and prints `ok` for such a document, which the study's inventory names as a fault (reference stability and errors study, section 4.2 item 7). Making the rebuild the default for `validate` would close that gap at the cost of the 200 ms budget of section 5; left for review.
8. **Which warnings fail `validate` (needs review).** The study's CLI1 names lost, ambiguous and guessed references. Section 3.1 also counts their region forms (`region_missing`, `region_ambiguous`, and `region_remapped`, which features.md section 3.7 treats as a guess) and `plane_fallback` (a sketch whose face reference is lost or no longer usable), because each is a stored reference that no longer means what it meant. A widened reference (`reference_widened`) builds on every replacement and waits for the user's choice, so it fails only with `--deny-warnings`, like a resized region.
9. **Location and action lines (needs review with the error guidelines).** The error guidelines show a location with a line (`model.ptsm:17: /features/1/edges/0`) and an action as `suggested: radius = 2.3 mm`. A feature message carries only a JSON Pointer, and finding the line of a pointer in the input needs a parser that keeps positions, which partsmith-model does not have, so section 2.2 writes the pointer alone. Actions are written with their labels (`suggested: Use 2.3 mm`), as the study's CLI1 says, so the CLI says the same words as every other host.
10. **Expected results of the examples.** The messages of `rebuild --json` gain `actions`, `candidates`, `reason`, `root` and `detail`, so the expected results of every example whose features have messages change and are blessed again (delivery.md section 2.8).
