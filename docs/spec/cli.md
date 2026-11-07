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

**In M2** ([ADR 0062](../adr/0062-custom-printer-profiles-stored-in-documents-and-kept-by-every-host.md))

- `--printer-file <PATH>` on `rebuild` and `export`: a custom printer profile read from a file (sections 2 and 3.3).
- Documents that store their printer profile rebuild with it, with no `--printer` (engine.md section 3.7).

**In M2: cancel** ([ADR 0063](../adr/0063-cancel-through-a-host-flag-outside-the-session.md))

- Nothing stops a CLI rebuild early. The session keeps `CancelFlag::never()`, the default of `SessionOptions::headless()` (section 3.6, engine.md section 3.26). No status reads `not_run`.

**In M2: assets** ([ADR 0064](../adr/0064-assets-added-to-an-open-document-and-named-by-the-core.md))

- The CLI keeps passing every asset-named file of `assets/` to `open_document` (section 2.3). It sends no `add_assets`. Font assets (`<hash>.ttf`, `<hash>.otf`) already match the pattern of section 2.3.
- `asset_mismatch` has new words that name the feature that reads the wrong file, with the file's name in the detail (engine.md section 4). `validate` reports it at that feature's asset field, and `rebuild` and `export` print it as the reason the file did not open (tests 86 and 87).

**Later (not in M1)**

- `partsmith merge-driver` for git, with validation after merge.
- `partsmith diff` (visual and record-level diff).
- `partsmith import` (STEP or mesh to a new `.ptsm`).
- `.ptsz` pack and unpack.
- Watch mode and slicer handoff (`partsmith send`).
- Custom printer profiles: a route that puts a profile in print.md's input form into the session (`put_printer_profile`); [ADR 0047](../adr/0047-printer-profiles-listed-whole-and-no-custom-profiles-in-m1-hosts.md) lists the options. A CLI session has only the built-in profiles, so in M1 `--printer` names `generic-fdm` or `generic-resin`. M2 adds `--printer-file` (In M2, above).
- Running Python scripts (done by `python -m partsmith`, python-api.md).
- Colored output and progress bars.

## 2. Public API

The public API is the command line. `crates/partsmith-cli/src/main.rs` parses arguments with `clap` (derive) and calls the engine's typed API with `SessionOptions::headless()`; one session serves all files of one invocation.

```text
partsmith [--version] [--help] <COMMAND>

partsmith validate [--rebuild] [--canonical] [--deny-warnings] [--json] <FILE>...
partsmith fmt [--check] <FILE>...
partsmith rebuild [--checks] [--printer <ID> | --printer-file <PATH>] [--rotate <AXIS:DEGREES>]... [--deny-warnings]
                  [--json] <FILE>
partsmith export <FILE> --output <PATH> [--format <FORMAT>] [--body <BODY>]... [--per-body] [--ascii]
                 [--chord <MM>] [--angle <DEGREES>] [--printer <ID> | --printer-file <PATH>]
                 [--rotate <AXIS:DEGREES>]... [--allow-errors] [--json]
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
| `--printer <ID>` | `rebuild`, `export` | Use this printer profile instead of the document's `printer` (for `printer.*` values, tessellation and checks). Not saved. An installed profile, which in M1 is a built-in one (`generic-fdm` or `generic-resin`, print.md section 3.3; custom profiles are Later, section 1). From M2 it may also be the document's own `printer` when the document stores that profile. It then changes nothing (3.3). |
| `--printer-file <PATH>` | `rebuild`, `export` | Use the printer profile in this file instead of the document's `printer`: JSON text in print.md's input form (section 3.1 of print.md), such as a profile a script writes or a document's stored `printer_profile` saved as a file of its own. Not saved. Not with `--printer` (exit 2). `-` is not standard input here, but a file named `-`. |
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

`partsmith --version` prints exactly `partsmith 0.1.0 (protocol 1, format 3, OCCT 7.9.3)` followed by a newline (`format 2` before format version 3), with the engine version, `PROTOCOL_VERSION`, the format version and the kernel string from `engine_info`.

### 2.1 Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Success. |
| 1 | The model has problems: validation errors, warnings of a lost, ambiguous or guessed reference (3.1), other warnings with `--deny-warnings`, a file that is not canonical (`fmt --check`, `validate --canonical`), failed features in `rebuild` or `export` (without `--allow-errors`), an unknown or ambiguous `--body`, an unknown `--printer`, a `--printer-file` whose profile the engine refuses, an engine error while exporting, or schema drift (`schema --check`). |
| 2 | Usage error: unknown command or option, missing or invalid value, or an option that does not apply to the chosen format. Printed by clap or in the same style. |
| 3 | Input or output error: a file or directory cannot be read or written, or the write refuses it (a read-only file, a symbolic link that leads to no file, or a link that changed while the write followed it, 3.7). |
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
          "message": "The file is not valid JSON at line 1, column 23.",
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
2. Without `--check`: when the canonical text differs from the file, writes it with the write of 3.7 (atomically, keeping the file's permissions, through a symbolic link) and prints `formatted <path>`. Unchanged files print nothing and are not rewritten (their modification time is unchanged). A read-only file in canonical form is not refused.
3. With `--check`: writes nothing, prints `<path>` for each file that is not canonical, and exits 1 if there is any.
4. With `-`: reads standard input and writes the canonical text to standard output (nothing else). With `--check`, writes nothing and exits 1 when the input is not canonical.
5. `fmt` keeps unknown feature types and invalid params exactly as loaded (in their original key order), so formatting never loses data.

### 3.3 `rebuild`

1. Opens the file with its assets, applies `--printer` as an in-memory `set_printer` when given (exit 1 with `error: Printer profile "<id>" is not installed.` when unknown), and recomputes. `--printer <ID>` is installed when it is a built-in id, or the document's own `printer` while the document stores that profile. For the document's own `printer` the CLI applies no edit, and the stored profile stays in use. With `--printer-file <PATH>`, before it opens the file, the CLI reads `<PATH>` as UTF-8 text (exit 3 with `error: cannot read "<PATH>": <os message>` when it cannot). It puts the text with the engine's `put_printer_profile_text`. A profile the engine refuses exits 1 with `error: <PATH>: <engine message>` (`invalid_profile`, or `profile_id_reserved` for a built-in id). The CLI then applies `--printer <its id>`, as above: the in-memory `set_printer` stores the file's profile in the document (engine.md section 3.3) and overrides a profile the document stores. Nothing is saved.
2. Measures every body with geometry (engine `measure`). A body without volume (engine.md section 3.11) is measured too, without a center of mass.
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

Then one line per body in document order; a body without geometry prints `  <id> "<name>": no geometry`. With `--checks`, then `  print health: <summary label> (<profile id>)` and one line per finding `    <severity>[<check>] <body id>: <message>`, in body order, then check order, then the check's own order. A finding with a suggestion is followed by the line `      suggested: <text>`, indented as a feature message's action lines are, with the suggestion's words `Suggestion::text` (print.md section 3.12: millimeters, numbers with at most 3 decimals as in the finding messages); a split without planes has no words and no line. The CLI calls `text` on the suggestion it reaches through the protocol's `PrintReport`, so its dependencies stay `engine`, `model` and `protocol` (ADR 0019). For example:

```text
  print health: 1 error (generic-fdm)
    error[bed_fit] b-1: The part footprint is 300 x 30 mm, larger than the 256 x 256 mm bed. Split the part or scale it down.
      suggested: Split at x = 0 mm
```

The JSON output carries the suggestion only as the report's geometry (`suggestion` of the finding, item 5), with no text.

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
- `reason`: why the reference the message names was lost, or which feature a followed or widened reference followed (recompute.md section 3.10, `Message::reason`): an object whose `kind` is the `LossReason` variant's name in snake case, followed by the variant's fields in declaration order, ids and element names as strings, `how` as an object of the same form for the `Removal` (`absence` `through` or `full_turn`), `instance` as `{"first": <n>, "second": <n>}`, and a field that is None omitted. For example `{"kind": "removed", "by": "f-ext2", "how": {"kind": "cut_away"}}`, `{"kind": "removed", "by": "f-ext2", "how": {"kind": "no_longer_made", "absence": "through"}}`, `{"kind": "other_body", "body": "b-2", "by": "f-split1", "by_suppressed": false, "present": true}` or `{"kind": "followed", "by": "f-sk1"}`.
- `root`: the first failing feature of a chain, or the sketch that lost its face (recompute.md `Message::root`).
- `detail`: the technical text for reports (features.md section 2.8, `detail`), never printed in the text output.

A message never has a `cause` here: the CLI opens the document fresh, so no edit of the session started it (engine.md section 3.21). For example, the extrude that depends on a failed datum plane in corpus R54 (test 61), shown compactly:

```json
{"severity": "error", "code": "upstream_failed", "message": "'Sketch 2' cannot be built because 'Datum plane 1' has an error, so this extrude cannot use it either. Fix 'Datum plane 1' first.", "actions": [{"label": "Show 'Datum plane 1'", "kind": "show"}], "root": "f-dp1"}
```

A parameter that failed to evaluate has `error` instead of `value` and `dimension`. A body without geometry has only `id`, `name` and `"empty": true`. A body without volume (engine.md section 3.11) has every member but `center_of_mass`, and its text line is that of any measured body (`volume 0 mm^3`); the message that explains it is its import's `import_warning` (test 65). With `--checks` a final key `checks` holds the protocol `PrintReport` (protocol.md 2.7: `profile`, `orientation`, `summary`, `bodies` with each body's `transform` and print.md `report`), without `mesh_id` and `overhang`.

6. Exit 1 when any feature has status `error`, or `warning` with `--deny-warnings`. Print findings never change the exit code.

### 3.4 `export`

1. Format: `--format`, else the output extension (case-insensitive): `.stl`, `.3mf`, `.obj`, `.step` or `.stp`, `.py`. Neither gives exit 2: `error: Cannot tell the export format from "<path>". Use --format.`
2. Option checks (exit 2): `--ascii` only with STL; `--per-body` only with STL; `--chord`, `--angle` only with mesh formats; `--body`, `--chord`, `--angle`, `--rotate`, `--per-body` and `--ascii` are invalid with `py`. Invalid numbers: `--chord` not finite and positive, `--angle` not in (0, 90].
3. Opens the file with its assets, applies `--printer` as in `rebuild`, and recomputes. If any feature has status `error` and `--allow-errors` is absent, prints `error: <path>: <n> feature(s) failed (<ids, comma-separated>). Fix them or pass --allow-errors to export anyway.` and exits 1.
4. `--body` values are matched to bodies by exact id, else by exact name. No match: `error: There is no body named or with id "<value>".`; two bodies with that name: `error: Two bodies are named "<value>". Use the body id.`; both exit 1.
5. Calls `Session::export` (mesh and STEP formats) with `ExportRequest { format, bodies, tolerance, orientation }`, where `tolerance` is present when `--chord` or `--angle` is given (the other value from `Session::tolerance(doc, Standard)`) and `orientation` is present when `--rotate` is given. For `py`, calls `export_python`; with `--json` the file's `media_type` is `text/x-python`.
6. Without `--per-body`, writes the single file to `--output`; the parent directory must exist (exit 3 otherwise). With `--per-body`, `--output` is a directory, created with its parents when missing, and each engine file is written inside it under its engine name. Each file is written with the write of 3.7. It replaces an existing file atomically and refuses a read-only file and a symbolic link that leads to no file. The first file that fails stops the export: files written before it stay and are printed (with `--json`, listed in `files`), and nothing after it is written.
7. Human output: each written path on its own line on standard output; engine warnings on standard error as `warning[<code>]: <message>`.
8. JSON output, with one entry in `files` per written file: its path (2.2: the `--output` value exactly as given or, with `--per-body`, that directory joined with the file's engine name), its media type (engine.md section 3.13 item 5) and `bytes`, the length of the file written. The examples runner compares this report with the file written for every example that builds and each of `stl`, `3mf`, `obj` and `step`, and imports each STEP file back to check its volume (delivery.md section 2.8, runner test 5):

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
2. `--write` writes `<out-dir>/ptsm.schema.json` and `<out-dir>/protocol.schema.json`, creating `<out-dir>` when missing. For each file it prints `wrote <path>` when the content changed or the file was missing, else `unchanged <path>` and leaves the file untouched. It writes with the write of 3.7. An up-to-date read-only file is not written, and it is not refused. A file whose read or write fails prints its error (section 4) instead of `wrote <path>`. The other file is still compared and written, and the command exits 3.
3. `--check` compares both files with the generated text and prints `<path> is out of date. Run: partsmith schema --write` for each missing or different file; exit 1 if any.
4. `<NAME>` cannot be combined with `--write` or `--check`, and `partsmith schema` without `<NAME>`, `--write` or `--check` is a usage error (exit 2).

### 3.6 General

- Files are processed sequentially in argument order; output for a file is complete before the next file starts.
- The CLI never modifies an input file except through `fmt` without `--check`.
- Output is deterministic: the same inputs give byte-identical standard output, standard error and written files.

**No stop** (since M2, [ADR 0063](../adr/0063-cancel-through-a-host-flag-outside-the-session.md)). The CLI's session keeps `CancelFlag::never()`. Every rebuild, export and check runs to its end, and no status line or `status` field reads `not_run`. Ctrl+C ends the process as in M1, with the default action of the signal.

### 3.7 Writing files

`fmt` (3.2), `export` (3.4) and `schema --write` (3.5) write every file through one write, `files::write` in `crates/partsmith-cli/src/files.rs`. Each error names the file by the path as given or derived from a given path (section 4), never a temporary file or the file a link points to. The write stops at the first step that fails:

1. **The target.** When the path is a symbolic link (`std::fs::symlink_metadata`), the write first looks at the file through the link with `std::fs::metadata`. The operating system follows the link for that look, under its own limits. On Linux, `fs.protected_symlinks` (on by default in Ubuntu, Debian and Fedora) refuses a link in a sticky folder that everyone can write, such as `/tmp`, when another user owns the link. The write then fails with the system's text. A link that leads to no file (its chain ends at a missing entry) fails with `it is a symbolic link to a file that does not exist. Create that file first, or remove the link.`: the look's `NotFound` becomes `io::Error::new(io::ErrorKind::NotFound, <that text>)`. A loop of links fails with the system's text. The target is then the file `std::fs::canonicalize` names, which follows every link of the chain. On Linux and macOS it must be the file the look reached: the same device and inode (`std::os::unix::fs::MetadataExt`). The device and inode of the file `canonicalize` named come from `std::fs::metadata` of that path. That call's own error is the write's error. When it is another file (a link on the way changed between the two calls), the write fails with `the symbolic link changed while Partsmith was following it. Try again.`, an `io::Error` of kind `StaleNetworkFileHandle` with that text. On Windows, `canonicalize` opens the file through the link itself. The system's own rules apply to that open. No comparison is made there. Every failure of this step comes before anything is created. The link stays as it is: the same entry, pointing at the same path. A path that is not a link, does not exist or cannot be looked at is the target itself. For such a path, any error comes from the steps below. The step is this private item of `files.rs`. `files::write` calls it with `std::fs::metadata` and `std::fs::canonicalize`. Test 70 passes its own. The lifetime `'path` lets the two generic functions be passed as they are (with `impl FnOnce(&Path)`, which asks for every lifetime, the compiler refuses them):

   ```rust
   /// The file a write to `path` replaces: the file a symbolic link at `path` leads to, as `resolve` names it and `look`
   /// reached it through the link, else `path` itself (cli.md section 3.7 step 1).
   fn link_target<'path>(
       path: &'path Path,
       look: impl FnOnce(&'path Path) -> io::Result<Metadata>,
       resolve: impl FnOnce(&'path Path) -> io::Result<PathBuf>,
   ) -> io::Result<PathBuf>
   ```

   `fmt` and `schema --write` read the file before they write it (3.2, 3.5 step 2). A loop of links or a link the system refuses to follow fails that read with `cannot read "<path>": <os message>` (section 4, exit 3) before this step. For `fmt` a link that leads to no file fails there too. `schema --write` takes such a link for a missing file and reaches this step (test 67).
2. **A read-only file.** The write looks at the target with `std::fs::metadata`. An existing target that is a regular file and read-only (`std::fs::Permissions::readonly`: on Unix no write bit is set, on Windows the read-only attribute) fails with `the file is read-only. Make it writable first.`, an `io::Error` of kind `PermissionDenied` with that text, and is not changed. The check reads the permission bits. It holds for every user, root included. Without it the rename of step 4 would replace the file on Linux and macOS, where a rename needs write access to the folder only. A target that cannot be looked at for any reason but a missing file fails with the system's text. The desktop (desktop.md section 3.4 step 4.2) and Python (python-api.md section 3.1) refuse a read-only file too (open issue 14).
3. **The temporary file.** A new file whose name starts with `.partsmith-` in the target's folder (`tempfile::Builder`). When the target exists, the file is created with the target's permissions (on Unix its mode, which the umask can only narrow). `set_permissions` then gives it the target's permissions in full, before any byte is written, as the desktop does (desktop.md section 3.4 step 4.3). A new target gets the default permissions (`0o666` less the umask on Unix, as `File::create` gives). The file is then written and flushed (`sync_all`). A failure to create or write the temporary file is reported with the operating system's error alone, without the temporary file's path (`tempfile` adds ` at path "<path>"` to the errors of `Builder::tempfile_in` and of `NamedTempFile`'s own `Write` methods). The creation is this private item of `files.rs`, which test 70 calls:

   ```rust
   /// A new, empty temporary file for a write in `folder`, whose name starts with `.partsmith-`, with `permissions` in
   /// full when given (those of the file it replaces), else the default ones (cli.md section 3.7 step 3).
   fn create_temporary(folder: &Path, permissions: Option<&Permissions>) -> io::Result<NamedTempFile>
   ```
4. **The rename** over the target. On any failure the temporary file is removed, and the target is unchanged.

A link that leads to a file is written through: the file it points to is replaced, and the link stays (test 68). Before wave G, a link that led to no file was replaced by a regular file (open issue 12), the target was found with `canonicalize` alone (open issue 13), a read-only file was replaced and kept read-only (open issue 14), and a failure of the temporary file named that file as well (` at path "<folder>/.partsmith-<random>"`, test 71).

## 4. Errors

| Situation | Message (standard error unless `--json`) | Exit |
| --- | --- | --- |
| Unreadable input | `error: cannot read "<path>": <os message>` | 3 |
| Unwritable output | `error: cannot write "<path>": <os message>` | 3 |
| A symbolic link that leads to no file, as the file to write (3.7 step 1) | `error: cannot write "<path>": it is a symbolic link to a file that does not exist. Create that file first, or remove the link.` | 3 |
| A link that changed while the write followed it (3.7 step 1) | `error: cannot write "<path>": the symbolic link changed while Partsmith was following it. Try again.` | 3 |
| A read-only file the command would replace (3.7 step 2) | `error: cannot write "<path>": the file is read-only. Make it writable first.` | 3 |
| Document does not load | the load diagnostic (2.2): the engine's `parse_error` or `unsupported_format_version` with the line, column and JSON Pointer the error carries (engine.md section 4), the same diagnostic `validate` reports for the text | 1 |
| Unknown `--printer` | `error: Printer profile "<id>" is not installed.` | 1 |
| Unreadable `--printer-file` | `error: cannot read "<path>": <os message>` | 3 |
| A `--printer-file` profile the engine refuses | `error: <path>: <engine message>` (the print crate's message, or `"<id>" is a built-in profile. Save your changes under a new name.`) | 1 |
| `--printer` with `--printer-file` | clap's message for two options that cannot be used together | 2 |
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

Release build, reference machine (technical design section 14), wall time including process start:

| Command | Budget |
| --- | --- |
| `partsmith --version` | 50 ms |
| `partsmith validate` of a 200 KB document without `--rebuild` | 200 ms |
| `partsmith fmt --check` of a 200 KB document | 150 ms |
| `partsmith schema --check` | 200 ms |
| `partsmith rebuild` and `export` | the engine budgets (engine.md section 5) of the work they ask of the engine, plus 100 ms |
| `partsmith rebuild` of the reference part (engine.md section 5) | 2.3 s: `open_document` of the reference part, cold session (2.2 s), plus 100 ms |
| `partsmith export` of the reference part to binary STL at standard quality | 2.6 s: `open_document` of the reference part, cold session (2.2 s), and `export` of the reference part to binary STL at standard quality (300 ms), plus 100 ms |

The 100 ms covers starting the process, reading the document, writing what the command writes and, for `rebuild`, measuring the bodies, which has no engine budget. The CLI's session is headless (section 2), so its `open_document` meshes no body for display, which the engine's row, run in a display session, does; its `export` meshes the bodies instead. The last two rows of the table, rows 5 and 6 below, follow engine.md section 5's `open_document of the reference part, cold session` and `export of the reference part to binary STL at standard quality` rows: a change to either changes them in the same change. The first follows the full rebuild's budget, with the open's share added, since 2026-11-05 (engine.md open issue 6, [ADR 0057](../adr/0057-budgets-set-from-the-measured-median-plus-20-percent.md)).

**How they are measured.** By `crates/partsmith-cli/benches/budgets.rs` (`harness = false`, `std::time::Instant`, no benchmark framework, decision D7 of `docs/spec/README.md`), on `partsmith-budgets` as the other crates' budget benchmarks are ([ADR 0027](../adr/0027-dev-only-workspace-crates.md)). `cargo bench -p partsmith-cli --bench budgets` runs every row below, or those whose names contain one of the words given after `--` (`-- validate fmt`; row 1 is selected with `version`, since the harness skips every argument that starts with `--`, which Cargo's own flags do). Cargo builds the `partsmith` binary with the `bench` profile, which is the release profile, and the bench runs that binary (`env!("CARGO_BIN_EXE_partsmith")`) through `std::process::Command`, in a temporary directory (`tempfile`) that holds every file the rows write, with standard input empty and standard output and error captured. A sample is the wall time of one `Command::output` call, from just before the process starts until it has exited and its output has been read. Each row runs its command once untimed, so that the binary and the row's inputs are in the file cache, as for a user who runs a command a second time, then 11 times timed, and records the median of the 11 samples (`median` of `partsmith-budgets`, the 6th smallest) against its budget; `Report` prints every row with its budget and fails the run when a row is over budget or not measured. Writing the inputs and the untimed run are outside the samples. Every run, untimed or timed, must exit with code 0, the code of a run that did all its work (a file that validates, a canonical file, schemas that match, a reference part with no failed feature, a written file). A run that does not leaves its row unmeasured, and the rows before and after it are still measured, so a run off the reference machine keeps every median it can take: the bench prints on standard error `<command> did not succeed (<status>), so "<row>" cannot be measured. It printed:` followed by the lines `--- standard output ---`, the run's standard output, `--- standard error ---` and its standard error (the layout of `Run::assert_code` in `tests/common/mod.rs`), records the row with `record_unmeasured`, skips the row's remaining runs and goes on with the next row; `finish` then lists it as `Not measured: <row>` and fails the run. `<command>` is `partsmith` followed by the arguments the bench passed, with the reference part's path as passed (`partsmith validate budget.ptsm`), `<status>` the run's exit status as Rust displays it (`exit status: 1`) and `<row>` the row's name; for row 4's untimed `schema --write`, `<command>` is `partsmith schema --write` and `<row>` row 4's name. A run that cannot be started (`Command::output` returns an error, as for a missing binary) is handled the same way, with the message `cannot run <command>: <error>`. No workflow runs the bench: like the other crates' budget benchmarks it runs by hand, the budgets hold on the reference machine, and a run elsewhere records its medians in open issue 11 without deciding anything.

The rows, each with its name as `Report` prints it, its command, run in the temporary directory, and its budget:

1. `partsmith --version`: `partsmith --version`, 50 ms.
2. `partsmith validate of a 200 KB document`: `partsmith validate budget.ptsm`, without `--rebuild`, 200 ms. The name leaves out the table's `without --rebuild`, so that the word `rebuild` selects row 5 alone.
3. `partsmith fmt --check of a 200 KB document`: `partsmith fmt --check budget.ptsm`, 150 ms.
4. `partsmith schema --check`: `partsmith schema --check`, 200 ms. Before its runs, an untimed `partsmith schema --write` in the same directory writes `docs/format/ptsm.schema.json` and `docs/format/protocol.schema.json` there, so the row compares both schemas with files that match them whatever the checked-in files hold; test 53 checks those.
5. `partsmith rebuild of the reference part`: `partsmith rebuild <reference part>`, where `<reference part>` is the absolute path of `examples/bench/fifty-features/model.ptsm`, read where it is (it has no `assets/` folder), 2.3 s.
6. `partsmith export of the reference part to binary STL at standard quality`: `partsmith export <reference part> -o model.stl`, which writes every default body into one binary STL file at the standard tolerance, the request of the engine's row (`{"kind": "stl", "binary": true, "per_body": false}`), 2.6 s. Each run replaces `model.stl`.

**The 200 KB document** of rows 2 and 3, `budget.ptsm`, is written at run time from `budget_document()` (`pub fn budget_document() -> String`) in `crates/partsmith-cli/tests/common/workload.rs`. `tests/common/mod.rs` declares that file as `pub mod workload;`, and the bench includes it alone with `#[path = "../tests/common/workload.rs"]`, as the sketch solver bench includes `tests/common/benchmark.rs`. So the file uses nothing else of `tests/common`: it reads `plate.ptsm` itself from `crates/partsmith-engine/tests/fixtures/` (with `include_str!` relative to the file, or `env!("CARGO_MANIFEST_DIR")`), uses only `std`, `serde_json` and `partsmith_engine`, which the bench also has, and allows `dead_code` when the bench and the tests use different items of it, as `benchmark.rs` does. Test 64 runs it, so the document the bench times is checked on every test run. It is `plate.ptsm` (engine.md section 3.20) 133 times over: units `mm`, no parameters, bodies `b-1` to `b-133` named `Plate 1` to `Plate 133`, and, for each copy `k` from 1 to 133 in turn, plate's sketch as `f-sk<k>`, with its entities `e-<k>01` to `e-<k>04` for plate's `e-1` to `e-4` and its constraints `c-<k>01` to `c-<k>08` for `c-1` to `c-8`, then plate's extrude as `f-ext<k>`, into `b-<k>` and of `f-sk<k>`, with its region's `key` and `outline` both `e-<k>01,e-<k>02,e-<k>03,e-<k>04` and its region `fp`, extent and `op` unchanged. The two digits after `<k>` keep each copy's ids in the byte order a region key lists them in (sketch.md section 3.13.5), which `e-9` to `e-12` would not be. The text is that document's canonical form, as `Session::format` gives it and `partsmith fmt` writes it (engine.md section 2, file-format.md), so `fmt --check` prints nothing and `validate` prints `budget.ptsm: ok`; it has 200,745 bytes (2026-10-30), with 1 KB taken as 1,000 bytes as in delivery.md section 2.6, and test 64 holds it at 200,000 bytes or more and under 210,000. `validate` solves its 133 sketches, more sketches per kilobyte than the reference part has (10 in 31 KB).

## 6. Acceptance tests

Tests live in `crates/partsmith-cli/tests/` and run the built binary (`env!("CARGO_BIN_EXE_partsmith")`) with `std::process::Command` in a temporary directory (`tempfile`), copying fixtures from `crates/partsmith-engine/tests/fixtures/`. "Prints" means the exact standard output unless stated.

**General**

1. `partsmith --version` prints `partsmith 0.1.0 (protocol 1, format 3, OCCT 7.9.3)\n` and exits 0 (`format 2` before format version 3).
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
62. Reasons in JSON (3.3). In `rebuild --json`, the first message of 'Fillet 1' in corpus R12 at distance 30 (recompute.md test 57) has `reason` `{"kind": "removed", "by": "f-ext2", "how": {"kind": "no_longer_made", "absence": "through"}}`; that of 'Chamfer 1' in corpus R31 (a) (recompute.md test 51) `{"kind": "other_body", "body": "b-2", "by": "f-split1", "by_suppressed": false, "present": true}`; that of 'Fillet 1' in corpus R41 (recompute.md test 61) `{"kind": "copy_gone", "feature": "f-lp1", "instance": {"first": 2, "second": 0}}`; and the notice of corpus R06 (a) (test 60) `{"kind": "followed", "by": "f-sk1"}`. An ambiguous loss (corpus R63 (a), recompute.md test 67) has no `reason`.

**Print suggestions**

63. Suggestion lines (3.3, print.md section 3.12). (a) `box.ptsm` with `f-box1`'s `size` set to `["300 mm", "30 mm", "height"]` and parameter `height` set to `300 mm`: `partsmith rebuild --checks box.ptsm` exits 0 and ends with exactly these lines:

```text
  print health: 2 errors (generic-fdm)
    error[bed_fit] b-1: The part footprint is 300 x 30 mm, larger than the 256 x 256 mm bed. Split the part or scale it down.
      suggested: Split at x = 0 mm
    error[bed_fit] b-1: The part is 300 mm tall, taller than the 256 mm build height. Split the part or lay it on its side.
      suggested: Split at z = 150 mm
```

(b) `box.ptsm` with the feature `{"id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "2 mm"}` added after `f-box1` (a closed hollow x in [-8, 8], y in [-13, 13], z in [2, 38]): `partsmith rebuild --checks --printer generic-resin box.ptsm` exits 0; its line `    warning[trapped_volumes] b-1: An enclosed hollow of 14976 mm³ will trap liquid resin. Add a drain hole near its lowest point.` is followed by a line matching `^      suggested: Add a drain hole near \((-8|8), (-13|13), 2 mm\)$`, and its `warning[overhangs]` line (the hollow's ceiling) is followed by no `suggested:` line. With `--json`, the `trapped_volumes` finding of `b-1` has a `suggestion` with `kind` `drain_hole` and `at` within 1e-9 of [X, Y, 2], with X and Y the numbers of that text line, and the words `Add a drain hole near (` appear nowhere in the JSON output.

(c) A tilted plane. `box.ptsm` with `f-box1`'s `size` set to `["300 mm", "100 mm", "height"]` and parameter `height` set to `10 mm`: `partsmith rebuild --checks --rotate z:30 box.ptsm` exits 0, and the line after its `    error[bed_fit] b-1: ` line is exactly `      suggested: Split at the plane through (0, 0, 5 mm) with normal (0.866, -0.5, 0)`.

(d) Several planes. `box.ptsm` with `f-box1`'s `size` set to `["600 mm", "300 mm", "height"]` and parameter `height` set to `10 mm`: `partsmith rebuild --checks box.ptsm` exits 0, and the line after `    error[bed_fit] b-1: The part footprint is 600 x 300 mm, larger than the 256 x 256 mm bed. Split the part or scale it down.` is exactly `      suggested: Split at x = -100 mm, x = 100 mm and y = 0 mm`.

The other words, among them the shared cases of print.md 3.12, are print.md test 114's to check, since the CLI prints `Suggestion::text` as it is.

**Budgets**

Test 64 is in `crates/partsmith-cli/tests/budgets.rs`, declared `mod budgets;` in `tests/integration.rs` (ADR 0035), and runs with `cargo test -p partsmith-cli --test integration budgets::`.

64. The 200 KB document of section 5. The text `budget_document()` returns has 200,000 bytes or more and fewer than 210,000, and is the same on every call. Written as `budget.ptsm`, `partsmith validate budget.ptsm` prints `budget.ptsm: ok\n` and exits 0, and `partsmith fmt --check budget.ptsm` prints nothing and exits 0. Parsed as JSON, it has the members of `plate.ptsm` in their order, with `format`, `format_version` and `units` plate's (`partsmith`, 3, `mm`; 2 before format version 3) and `parameters` `[]`; `bodies` has 133 entries, entry `k` (from 1) `{"id": "b-<k>", "name": "Plate <k>"}`, so the first is `{"id": "b-1", "name": "Plate 1"}` and the last `{"id": "b-133", "name": "Plate 133"}`; and `features` has 266 entries, `f-sk1`, `f-ext1`, `f-sk2` and so on to `f-ext133` in this order, where, for each `k`, `f-sk<k>` and `f-ext<k>` equal the sketch and the extrude of `plate.ptsm` with `b-1`, `f-sk1`, `f-ext1`, `e-1` to `e-4` and `c-1` to `c-8` written `b-<k>`, `f-sk<k>`, `f-ext<k>`, `e-<k>01` to `e-<k>04` and `c-<k>01` to `c-<k>08` wherever they occur, region keys and point references such as `e-1.end` included, so the last copy's region key is `e-13301,e-13302,e-13303,e-13304` and its last constraint `c-13308`.

**Bodies without volume**

Test 65 is in `crates/partsmith-cli/tests/rebuild.rs`.

65. A body without volume (3.3, engine.md section 3.11). `store_self_void_asset` of `tests/common`, beside `store_box_step_asset`, stores `SELF_VOID` in the workspace's `assets/` and returns its asset name: the text that `partsmith export box.ptsm -o box.step` writes, edited as engine.md test 162 edits it into `SELF_VOID` (a copy of the engine's helper, with its check of exactly one `MANIFOLD_SOLID_BREP` and no `BREP_WITH_VOIDS`), imported into a headless session with the engine's `import_step` and named by the asset `save_document` returns, as `store_box_step_asset` names its asset (engine.md section 3.14), so no dependency is added. The test writes `model.ptsm` as `import_document` of `tests/common` makes it for that name: its one body `b-1` "Imported" made by one feature, `{"id": "f-imp1", "type": "import", "body": "b-1", "asset": "<name>", "op": "new"}`. `partsmith rebuild model.ptsm` exits 0, writes nothing to standard error and prints exactly:

```text
model.ptsm
  f-imp1 import warning
    warning[import_warning] at /features/0: This import kept 1 solid of the STEP file that is not valid and could not be repaired, so the shape this import gives may be wrong, and features that change it later may fail or give a wrong shape.
  b-1 "Imported": volume 0 mm^3, area 5200 mm^2, bounding box (-10, -15, 0) to (10, 15, 40) mm, 6 faces, 12 edges, 8 vertices
```

`partsmith rebuild --deny-warnings model.ptsm` prints the same and exits 1. In `partsmith rebuild --json model.ptsm`, which exits 0, `b-1` has exactly the members `id`, `name`, `volume`, `area`, `bounding_box`, `faces`, `edges` and `vertices`, in this order, with `volume` 0 within 1e-9, `area` 5200 and no `center_of_mass`. `partsmith rebuild --checks model.ptsm` exits 0 and ends with the line `  print health: Ready to print (generic-fdm)`. (Before engine.md section 3.11 said so, each of these runs exited 1: the text runs printed nothing to standard output and `error: Partsmith hit a bug while running this command. Your document is unchanged. Save a copy, and report this with the details.` to standard error, and the `--json` run printed `{"path": "model.ptsm", "error": {"severity": "error", "code": "internal_error", "message": <that message>}}`.)

**Linked and read-only files** (3.7, [ADR 0037](../adr/0037-save-checks-the-file-and-hosts-reach-the-assets-folder.md)'s amendments of 2026-11-04 and 2026-11-05)

Tests 66 to 69 and 71 are in `crates/partsmith-cli/tests/writes.rs`, declared `mod writes;` in `tests/integration.rs` (ADR 0035). They run with `cargo test -p partsmith-cli --test integration writes::`. Test 70 is unit tests in `src/files.rs`. Each case runs in its own temporary directory `tmp`, the binary's working directory, with `box.ptsm` copied there. "A link to `x`" is a symbolic link whose text is `x`, a path relative to the link's folder, made with `std::os::unix::fs::symlink`. "The same link" means that `std::fs::symlink_metadata` still shows a link and `std::fs::read_link` gives the text it gave before. A temporary file is an entry whose name starts with `.partsmith-`. "Read-only" means `Permissions::set_readonly(true)`, which on Unix clears every write bit. The tests and cases marked (Unix) build only there (`#[cfg(unix)]`). CI runs this section on Linux only (delivery.md 2.4): CI's `rust-platforms` runs the CLI's `examples::` alone on Windows and macOS (delivery.md 2.14). No case is skipped as root, since the read-only check reads the permission bits.

66. (Unix) Export onto a link that leads to no file (3.4, 3.7 step 1, open issue 12). (a) With `tmp/printer` an empty folder and `tmp/out.stl` a link to `printer/part.stl`, `partsmith export box.ptsm -o out.stl` exits 3, prints nothing on standard output and exactly `error: cannot write "out.stl": it is a symbolic link to a file that does not exist. Create that file first, or remove the link.\n` on standard error. `out.stl` is the same link, `printer` is still empty, and `tmp` holds exactly `box.ptsm`, `out.stl` and `printer`, with no temporary file. (b) In the setup of (a), `partsmith export --json box.ptsm -o out.stl` exits 3 with standard error empty, and its report has `files` `[]` and an `error` with `severity` `error`, `code` `io_error` and `message` `cannot write "out.stl": it is a symbolic link to a file that does not exist. Create that file first, or remove the link.` (c) A loop: with `tmp/a.stl` a link to `b.stl` and `tmp/b.stl` a link to `a.stl`, `partsmith export box.ptsm -o a.stl` exits 3, with standard error starting `error: cannot write "a.stl": `. Both are the same links, and `tmp` holds exactly `a.stl`, `b.stl` and `box.ptsm`. (d) With `--per-body`: `two-bodies.ptsm` copied to `tmp`, and `tmp/parts` a folder holding only `Peg.stl`, a link to `../gone/peg.stl` (`tmp/gone` does not exist). `partsmith export two-bodies.ptsm -o parts --per-body --format stl` exits 3, prints exactly `parts/Block.stl\n` on standard output (the file written before the failure), and prints exactly `error: cannot write "parts/Peg.stl": it is a symbolic link to a file that does not exist. Create that file first, or remove the link.\n` on standard error (this export prints no warning). `parts/Block.stl` is a binary STL file, `parts/Peg.stl` is the same link, `tmp/gone` still does not exist, and `parts` holds exactly `Block.stl` and `Peg.stl`. (Before wave G, (a) exited 0 and replaced the link with a regular file of 684 bytes.)
67. (Unix) `schema --write` onto a link that leads to no file (3.5, 3.7 step 1, open issue 12). With `tmp/s` a folder holding only `ptsm.schema.json`, a link to `../gone/ptsm.schema.json` (`tmp/gone` does not exist), `partsmith schema --write --out-dir s` exits 3. It prints exactly `wrote s/protocol.schema.json\n` on standard output and exactly `error: cannot write "s/ptsm.schema.json": it is a symbolic link to a file that does not exist. Create that file first, or remove the link.\n` on standard error. `s/ptsm.schema.json` is the same link, `s/protocol.schema.json` holds `partsmith_engine::protocol_schema()`, `tmp/gone` still does not exist, and `s` holds exactly `ptsm.schema.json` and `protocol.schema.json`, with no temporary file. `partsmith schema --check --out-dir s` then exits 1 and prints exactly `s/ptsm.schema.json is out of date. Run: partsmith schema --write\n` (its read finds no file, 3.5 step 3). (Before wave G, `--write` exited 0, printed `wrote` for both files and replaced the link with a regular file.)
68. (Unix) A link that leads to a file is written through, by each command that writes (3.7). In each case `tmp/real` is a folder. (a) With `tmp/real/part.stl` holding `old` and `tmp/out.stl` a link to `real/part.stl`, `partsmith export box.ptsm -o out.stl` exits 0 and prints `out.stl\n`. `real/part.stl` is then a file of 684 bytes, `out.stl` is the same link, and `real` holds only `part.stl`. (b) With `tmp/a.stl` a link to `b.stl`, `tmp/b.stl` a link to `real/part.stl` and `real/part.stl` holding `old`, the same command with `-o a.stl` exits 0 and prints `a.stl\n`. `real/part.stl` is then a file of 684 bytes, both are the same links, and `real` holds only `part.stl`. (c) With `tmp/real/reindented.ptsm` holding `box.ptsm` re-indented with 4 spaces and `tmp/link.ptsm` a link to `real/reindented.ptsm`, `partsmith fmt link.ptsm` exits 0 and prints `formatted link.ptsm\n`. `real/reindented.ptsm` then equals the canonical fixture byte for byte, `link.ptsm` is the same link, and `real` holds only `reindented.ptsm`. (d) With `tmp/s` a folder holding only `ptsm.schema.json`, a link to `../real/ptsm.schema.json`, and `real/ptsm.schema.json` holding `stale`, `partsmith schema --write --out-dir s` exits 0 and prints exactly `wrote s/ptsm.schema.json\nwrote s/protocol.schema.json\n`. `real/ptsm.schema.json` then holds `partsmith_engine::ptsm_schema()`, and `s/ptsm.schema.json` is the same link.
69. A read-only file is refused (3.7 step 2, open issue 14). (a) With `tmp/out.stl` holding `old` and read-only, `partsmith export box.ptsm -o out.stl` exits 3, prints nothing on standard output and exactly `error: cannot write "out.stl": the file is read-only. Make it writable first.\n` on standard error. `out.stl` still holds `old` and is read-only, and `tmp` holds exactly `box.ptsm` and `out.stl`. (b) With `tmp/reindented.ptsm` holding `box.ptsm` re-indented with 4 spaces and read-only, `partsmith fmt reindented.ptsm` exits 3, prints nothing on standard output and exactly `error: cannot write "reindented.ptsm": the file is read-only. Make it writable first.\n` on standard error, and the file still holds its text and is read-only. `partsmith fmt --check reindented.ptsm` prints `reindented.ptsm\n` and exits 1, since it writes nothing. With `tmp/box.ptsm` read-only, `partsmith fmt box.ptsm` prints nothing and exits 0, since a canonical file is not written (3.2). (c) With `tmp/s/ptsm.schema.json` holding `stale` and `tmp/s/protocol.schema.json` holding `partsmith_engine::protocol_schema()`, both read-only, `partsmith schema --write --out-dir s` exits 3. It prints exactly `unchanged s/protocol.schema.json\n` on standard output and exactly `error: cannot write "s/ptsm.schema.json": the file is read-only. Make it writable first.\n` on standard error. Both files keep their contents and stay read-only. (d) (Unix) Behind a link: with `tmp/real/part.stl` holding `old` and read-only, and `tmp/out.stl` a link to `real/part.stl`, `partsmith export box.ptsm -o out.stl` exits 3 with exactly the standard error of (a), which names `out.stl`, the link. `real/part.stl` still holds `old` and is read-only, `real` holds only `part.stl`, and `out.stl` is the same link. (Before wave G, each of (a) to (d) exited 0 and replaced the file, which stayed read-only.)
70. The look through a link and the temporary file (3.7 steps 1 and 3, open issue 13). Unit tests in `src/files.rs`, on Unix. They call `link_target` with the look and the resolve passed in, as desktop.md test 73 does on the desktop. A test can neither make a link that another user owns nor change a link between two calls. With `dir/link.stl` a link to `real/part.stl`, and `dir/real/part.stl` holding `old`: (a) With `std::fs::metadata` and `std::fs::canonicalize`, it gives the path `std::fs::canonicalize` gives for `dir/real/part.stl`. (b) A look that fails with `io::ErrorKind::PermissionDenied` and the text `Permission denied`, as Linux's `fs.protected_symlinks` makes it fail for a link another user owns in `/tmp`, gives that error, and the resolve is never called. (c) A look that fails with `io::ErrorKind::NotFound` gives an error of kind `io::ErrorKind::NotFound` whose text is `it is a symbolic link to a file that does not exist. Create that file first, or remove the link.`, and the resolve is never called. (d) A resolve that gives `dir/other.stl`, holding `other`, as if the link had changed after the look, gives an error of kind `io::ErrorKind::StaleNetworkFileHandle` whose text is `the symbolic link changed while Partsmith was following it. Try again.` (e) For `dir/plain.stl`, a regular file, and for `dir/missing.stl`, which does not exist, it gives the path itself and calls neither the look nor the resolve. (f) `create_temporary(dir, Some(&permissions))`, with `permissions` of mode `0o666` (`PermissionsExt::from_mode`), gives an empty file in `dir` whose name starts with `.partsmith-` and whose mode is `0o666`. That is more than the usual umask (`0o022` or `0o002`) lets a new file have. The mode shows that `set_permissions` ran before any byte was written. (g) `create_temporary` in `dir/missing`, a folder that does not exist, fails with an error of kind `io::ErrorKind::NotFound` whose text is exactly `No such file or directory (os error 2)`, the system's text without a path.
71. (Unix) A write error names the path as given, never the temporary file (3.7 step 3). With no folder `tmp/missing`, `partsmith export box.ptsm -o missing/out.stl` exits 3, prints nothing on standard output and exactly `error: cannot write "missing/out.stl": No such file or directory (os error 2)\n` on standard error, and `tmp` holds only `box.ptsm`. The case holds as root too. (Before wave G, the line went on with ` at path "<tmp>/missing/.partsmith-<random>"`, the temporary file the write had tried to create.)

**Printer profiles** ([ADR 0062](../adr/0062-custom-printer-profiles-stored-in-documents-and-kept-by-every-host.md)). The documents are engine.md section 3.20's fixtures, and `mk4.json` holds the text `{"id": "my-mk4", "name": "My MK4", "technology": "fdm", "bed": {"kind": "rectangle", "width": 250, "depth": 210, "height": 220}, "layer_height": 0.15, "fit": {"slip": 0.15, "loose": 0.3}}`.

72. A stored profile needs no flag: `partsmith rebuild --json custom-printer.ptsm` exits 0 with `printer` `my-mk4`, the parameter `clearance` 0.15 and `b-1` volume 24360 within 1e-6 relative (24480 with `generic-fdm`'s slip of 0.2), and standard error stays empty.
73. `--printer-file` puts a profile: `partsmith rebuild --json --printer-file mk4.json box.ptsm` exits 0 with `printer` `my-mk4`, and `box.ptsm` is byte for byte unchanged on disk.
74. `--printer-file` overrides a stored profile: with `slip.json` holding `{"id": "my-mk4", "name": "My MK4", "technology": "fdm", "fit": {"slip": 0.3, "loose": 0.4}}`, `partsmith rebuild --json --printer-file slip.json custom-printer.ptsm` exits 0 with `clearance` 0.3, and `custom-printer.ptsm` is byte for byte unchanged on disk.
75. `--printer` with the document's own stored printer: `partsmith rebuild --json --printer my-mk4 custom-printer.ptsm` exits 0 with `printer` `my-mk4` and `clearance` 0.15.
76. `--printer` with an id the document does not store: `partsmith rebuild --printer my-mk4 box.ptsm` exits 1 with `error: Printer profile "my-mk4" is not installed.`
77. A profile file the print crate refuses: with `bad.json` holding `{"id": "x", "name": "X", "technology": "fdm", "layer_height": 0.5}`, `partsmith rebuild --printer-file bad.json box.ptsm` exits 1 and standard error is exactly `error: bad.json: Layer height 0.5 mm is larger than the 0.4 mm nozzle. Use a layer height of 0.4 mm or less.` and a newline.
78. A profile file with a built-in id: with `fdm.json` holding `{"id": "generic-fdm", "name": "X", "technology": "fdm"}`, `partsmith rebuild --printer-file fdm.json box.ptsm` exits 1 with `error: fdm.json: "generic-fdm" is a built-in profile. Save your changes under a new name.`
79. A profile file that cannot be read: `partsmith rebuild --printer-file missing.json box.ptsm` exits 3 with a line on standard error that starts with `error: cannot read "missing.json": `.
80. `--printer` and `--printer-file` together: `partsmith rebuild --printer generic-fdm --printer-file mk4.json box.ptsm` exits 2.
81. A version 2 document that names an unknown profile (newcomer error corpus NC13): with `old.ptsm` the text of `box.ptsm` at `format_version` 2 with `"printer": "my-printer"`, `partsmith validate old.ptsm` exits 0 and prints the line `old.ptsm: warning[unknown_printer_profile]: Printer profile "my-printer" is not among your printers. Printer values and checks use the default FDM profile until you add that printer or pick another one.`
82. The profile that the warning asks for: with `mine.json` holding `{"id": "my-printer", "name": "Mine", "technology": "fdm"}`, `partsmith rebuild --json --printer-file mine.json old.ptsm` (test 81's document) exits 0 with `printer` `my-printer`, and `old.ptsm` is byte for byte unchanged on disk.
83. A stored profile the engine refuses: with `custom-printer.ptsm`'s stored profile given `"layer_height": 0.5`, `partsmith validate bad-store.ptsm` exits 1 with the line `bad-store.ptsm: error[parse_error] at /printer_profile: Layer height 0.5 mm is larger than the 0.4 mm nozzle. Use a layer height of 0.4 mm or less.`, and `partsmith fmt --check bad-store.ptsm` exits 1 with the same line.
84. `fmt` orders a stored profile: `partsmith fmt --check` of `custom-printer.ptsm` with the members of its stored profile in reversed order exits 1, and `partsmith fmt` of it rewrites the file to the fixture text.

**No stop** (since M2, [ADR 0063](../adr/0063-cancel-through-a-host-flag-outside-the-session.md))

85. The CLI never stops a rebuild (section 3.6). `partsmith rebuild --json examples/bench/fifty-features/model.ptsm` exits 0 and lists 50 features, none with `status` `not_run`.

**Assets** (since M2, [ADR 0064](../adr/0064-assets-added-to-an-open-document-and-named-by-the-core.md)). `step-block` is a folder holding a copy of `examples/import/step-block.ptsm` and an `assets/` folder whose file `b8d947e1aae9d107df35bea403414dc9ae3df630b84426c70e92770948bdfdb4.step` holds the bytes `hello`.

86. Run in the folder `step-block`, `partsmith validate step-block.ptsm` exits 1 and prints the line `step-block.ptsm: error[asset_mismatch] at /features/0/asset: The STEP file of 'Import 1' in the assets folder is not the file it imported. Copy the assets folder that came with the document next to it.` No line of its output holds the 64 hex digits of the asset's name.
87. Run in the folder `step-block`, `partsmith rebuild step-block.ptsm` exits 1 and prints the line `step-block.ptsm: error[asset_mismatch]: The STEP file of 'Import 1' in the assets folder is not the file it imported. Copy the assets folder that came with the document next to it.` Until M2 the message was `Asset b8d947e1aae9d107df35bea403414dc9ae3df630b84426c70e92770948bdfdb4.step does not match its content hash.`

## 7. Open issues

1. **Command list.** The architecture names `validate | fmt | rebuild | export | schema`. Print checks are an option of `rebuild` (`--checks`) rather than a separate command. STEP import is left to Python in M1 (`Document.import_step`, python-api.md); the UI's import (ui.md 1.2) and a `partsmith import` command (section 1, Later) come later, so the CLI has no import route in M1.
2. **Asset discovery.** The CLI passes every hash-named file in `assets/` and relies on the engine ignoring unreferenced ones (engine.md 3.14). A large shared asset folder is read fully; a lazy asset source can come later.
3. **`rebuild --json` and expected results.** Resolved by [delivery.md](delivery.md) section 2.8: an example's `<stem>.expected.json` is the `rebuild --checks --json` report without `path` and the findings' triangle lists.
4. **Exit code for feature errors in `validate --rebuild` versus `rebuild`.** Both use exit 1; print findings never affect the exit code. A future `--deny-findings` can change that.
5. **Feature messages in `validate`.** Resolved by engine.md section 3.19 step 8: the rebuild step reports every feature message, info notices included, with its own code, its rendered text and its location, and its `feature` member (`FeatureMessage`) carries the projected message with its actions, reason, root and detail and the candidates of its element repairs, as `FeatureStatus` projects them (engine.md section 3.4), so that `validate --rebuild` and `rebuild` print the same message (tests 18 and 57 to 60).
6. **Loss reasons reach the CLI.** Resolved by protocol.md section 2.4: `Diagnostic::reason` carries recompute's `Message::reason` in the wire form section 3.3 prints, and engine.md section 3.4 projects it (protocol.md open issue 4, recompute.md open issue 11).
7. **Lost references without `--rebuild`.** Element references resolve only against built geometry, so `validate` without `--rebuild` cannot tell that a fillet's edge is gone and prints `ok` for such a document, which the study's inventory names as a fault (reference stability and errors study, section 4.2 item 7). Making the rebuild the default for `validate` would close that gap at the cost of the 200 ms budget of section 5; left for review.
8. **Which warnings fail `validate` (signed off by the owner on 2026-11-05).** The study's CLI1 names lost, ambiguous and guessed references. Section 3.1 also counts their region forms (`region_missing`, `region_ambiguous`, and `region_remapped`, which features.md section 3.7 treats as a guess) and `plane_fallback` (a sketch whose face reference is lost or no longer usable), because each is a stored reference that no longer means what it meant. A widened reference (`reference_widened`) builds on every replacement and waits for the user's choice, so it fails only with `--deny-warnings`, like a resized region.
9. **Location and action lines (signed off by the owner on 2026-11-05).** The error guidelines show a location with a line (`model.ptsm:17: /features/1/edges/0`) and an action as `suggested: radius = 2.3 mm`. A feature message carries only a JSON Pointer, and finding the line of a pointer in the input needs a parser that keeps positions, which partsmith-model does not have, so section 2.2 writes the pointer alone. Actions are written with their labels (`suggested: Use 2.3 mm`), as the study's CLI1 says, so the CLI says the same words as every other host.
10. **Expected results of the examples.** The messages of `rebuild --json` gain `actions`, `candidates`, `reason`, `root` and `detail`, so the expected results of every example whose features have messages change and are blessed again (delivery.md section 2.8).
11. **Budgets off the reference machine.** The budgets of section 5 hold on the reference machine (technical design section 14), which does not exist yet (delivery.md section 1.2), so a run of the bench elsewhere records its medians here and decides nothing. On 2026-10-30, in this repository's 4-core Linux container, the bench's first two runs gave: `--version` 7.6 and 5.7 ms (50 ms), `validate` of the 200 KB document 20.9 and 18.9 ms (200 ms), `fmt --check` of it 15.4 and 14.9 ms (150 ms), `schema --check` 13.2 and 13.0 ms (200 ms), `rebuild` of the reference part 2,228 and 2,342 ms (1.1 s, over) and `export` of it 2,670 and 2,727 ms (1.4 s, over). So it fails here on its last two rows only, as the engine's bench fails here on its cold `open_document` row. The first four mostly time process start, which is about all `--version` does. The last two are over because the engine's work is over there too (engine.md open issue 6, recompute.md open issue 4): the same day, `cargo bench -p partsmith-engine --bench budgets -- cold "binary STL"` gave 2,389 ms for `open_document` of the reference part, cold session (1 s), and 8.5 ms for its `export` to binary STL (300 ms) in the bench's display session, which has meshed the bodies when it opened the document; the CLI's headless session meshes them in `export`, which took 385 to 442 ms longer than `rebuild`. On 2026-11-05 the last two rows read 2,347 and 2,733 ms here. Their budgets follow the cold `open_document`'s new 2.2 s: 2.3 and 2.6 s ([ADR 0057](../adr/0057-budgets-set-from-the-measured-median-plus-20-percent.md)). A later run read 2,256 and 2,566 ms, just under them. The CLI's headless open meshes nothing for display, which the engine's cold row does.
12. **A write onto a symbolic link that leads to no file.** Resolved: the owner chose option (a) on 2026-11-05. Wave G's link-writes track builds it. Before wave G, the atomic write (`write_atomically` in `crates/partsmith-cli/src/files.rs`) wrote to the path as given when `canonicalize` found no file. The rename then replaced the link with a regular file. That happened to `export --output` (section 3.4) and `schema --write` (section 3.5). `fmt` (section 3.2) failed before it wrote, since it reads the file first. Python and the desktop fail there and keep the link (python-api.md section 3.1, desktop.md section 3.4 step 4.1). [ADR 0037](../adr/0037-save-checks-the-file-and-hosts-reach-the-assets-folder.md)'s first amendment of 2026-11-04 gave the options: (a) the CLI fails like the hosts, exits 3 with its write error and keeps the link, or (b) the CLI stays as built. The owner chose (a) on 2026-11-05. The write now fails before anything is created, with a message that names the link (3.7 step 1, section 4). The link stays (tests 66 and 67). The read-only refusal that the owner's rule added the same day is open issue 14.
13. **A link the operating system would refuse to follow.** Resolved by the owner's rule of 2026-11-05 ([ADR 0037](../adr/0037-save-checks-the-file-and-hosts-reach-the-assets-folder.md)'s amendment of 2026-11-05, choice 4). Before wave G, `write_atomically` found the file a link points to with `std::fs::canonicalize` alone. On Linux that is glibc's `realpath`, which reads each link itself. Linux's `fs.protected_symlinks` never applied to it. `partsmith export --output /tmp/part.stl` onto a link another user owns there wrote the file the link names, where a plain write fails with `EACCES`. The write now looks through the link with `std::fs::metadata` first, under the system's own limits. On Linux and macOS it then checks that `canonicalize` names the file that look reached (3.7 step 1, test 70). Python does the same (python-api.md section 3.1). desktop.md open issue 14 was the same gap on the desktop and is closed the same way.
14. **A read-only file the CLI would replace (decided by the owner's rule of 2026-11-05: refuse it).** Before wave G, the CLI's write did not check the target's permissions. Where a rename can replace a read-only file (Linux and macOS, where it needs write access to the folder only), `fmt`, `export` and `schema --write` replaced one and kept it read-only. Once Python refuses a read-only document or export file (python-api.md open issue 12, the owner's option (a)), the CLI would be the one Partsmith program that replaces one. [ADR 0037](../adr/0037-save-checks-the-file-and-hosts-reach-the-assets-folder.md)'s second amendment of 2026-11-04 gives the options: (a) refuse every read-only file the CLI would replace, before it writes, as the desktop and Python do; (b) refuse a read-only document or export file and still replace a read-only schema file; (c) keep replacing them all. The owner's rule takes (a). It is the safest choice. Tools that rewrite files, such as rustfmt and Prettier, fail on a read-only file too. The write refuses one with `the file is read-only. Make it writable first.` (3.7 step 2, section 4, test 69). A file that needs no change is not written. A read-only file in canonical form and an up-to-date read-only schema file are not refused (3.2, 3.5). Closed: wave G's link-writes track builds it.
