# CLI specification

Crate: `partsmith-cli`, binary `partsmith`. The CLI is a thin host over `partsmith_engine::Session` (engine.md): it reads and writes files, parses arguments and formats output. It contains no modeling logic.

## 1. Purpose and scope

The CLI serves makers who script batch exports, CI jobs that validate models and regenerate schemas, and the git workflow (validate after merges, canonical formatting).

**In M1**

- `partsmith validate`: load, check expressions, references, sketches, assets, optionally rebuild and canonical form.
- `partsmith fmt`: rewrite files in canonical form, or check that they are.
- `partsmith rebuild`: full rebuild with per-feature status, per-body metrics and optional print checks.
- `partsmith export`: STL, 3MF, OBJ, STEP and Python output.
- `partsmith schema`: print, write or check `ptsm.schema.json` and `protocol.schema.json`.
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
| `--rebuild` | `validate` | Also rebuild geometry and report feature errors and warnings. |
| `--canonical` | `validate` | Report `not_canonical` when the file is not in canonical form. |
| `--deny-warnings` | `validate`, `rebuild` | Treat warnings as failures for the exit code. |
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

`partsmith --version` prints exactly `partsmith 0.1.0 (protocol 1, format 1, OCCT 7.9.3)` followed by a newline, with the engine version, `PROTOCOL_VERSION`, the format version and the kernel string from `engine_info`.

### 2.1 Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Success. |
| 1 | The model has problems: validation errors (or warnings with `--deny-warnings`), a file that is not canonical (`fmt --check`, `validate --canonical`), failed features in `rebuild` or `export` (without `--allow-errors`), an unknown or ambiguous `--body`, an unknown `--printer`, an engine error while exporting, or schema drift (`schema --check`). |
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
  - `<path>:<line>:<column>: <severity>[<code>]: <message>` when the line is known.
  - `<path>: <severity>[<code>] at <pointer>: <message>` when only the JSON Pointer is known.
  - `<path>: <severity>[<code>]: <message>` otherwise.
  - `<severity>` is `error`, `warning` or `info`.

### 2.3 Assets

For a document at path `P`, the CLI reads every regular file in `P`'s directory under `assets/` whose name matches `^[0-9a-f]{64}\.[a-z0-9]+$` and passes them to the engine, which uses only the referenced ones (engine.md 3.14). A missing `assets/` directory is not an error. A document read from standard input gets no assets.

## 3. Behavior

### 3.1 `validate`

1. Reads each file (exit 3 on failure, reported as `error: cannot read "<path>": <os message>`), then calls `Session::validate` with `rebuild` and `canonical` from the flags.
2. Human output: every diagnostic of the file in report order, in the formats of 2.2; a file without diagnostics prints `<path>: ok`.
3. A file fails when it has an error diagnostic, or a warning with `--deny-warnings`.
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

Diagnostic objects have `severity`, `code`, `message`, then `line`, `column` and `path` (JSON Pointer) when known; absent fields are omitted.

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

One line per feature in timeline order: `  <id> <type> <status>`, followed by one line per message `    <severity>[<code>]: <message>`. Then one line per body in document order; a body without geometry prints `  <id> "<name>": no geometry`. With `--checks`, then `  print health: <summary label> (<profile id>)` and one line per finding `    <severity>[<check>] <body id>: <message>`, in body order, then check order, then the check's own order.

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

`printer` is the effective profile id. `messages` entries are `Diagnostic`s (protocol.md 2.4). A parameter that failed to evaluate has `error` instead of `value` and `dimension`. A body without geometry has only `id`, `name` and `"empty": true`. With `--checks` a final key `checks` holds the protocol `PrintReport` (protocol.md 2.7: `profile`, `orientation`, `summary`, `bodies` with each body's `transform` and print.md `report`), without `mesh_id` and `overhang`.

6. Exit 1 when any feature has status `error`, or `warning` with `--deny-warnings`. Print findings never change the exit code.

### 3.4 `export`

1. Format: `--format`, else the output extension (case-insensitive): `.stl`, `.3mf`, `.obj`, `.step` or `.stp`, `.py`. Neither gives exit 2: `error: Cannot tell the export format from "<path>". Use --format.`
2. Option checks (exit 2): `--ascii` only with STL; `--per-body` only with STL; `--chord`, `--angle` only with mesh formats; `--body`, `--chord`, `--angle`, `--rotate`, `--per-body` and `--ascii` are invalid with `py`. Invalid numbers: `--chord` not finite and positive, `--angle` not in (0, 90].
3. Opens the file with its assets, applies `--printer` as in `rebuild`, and recomputes. If any feature has status `error` and `--allow-errors` is absent, prints `error: <path>: <n> feature(s) failed (<ids, comma-separated>). Fix them or pass --allow-errors to export anyway.` and exits 1.
4. `--body` values are matched to bodies by exact id, else by exact name. No match: `error: There is no body named or with id "<value>".`; two bodies with that name: `error: Two bodies are named "<value>". Use the body id.`; both exit 1.
5. Calls `Session::export` (mesh and STEP formats) with `ExportRequest { format, bodies, tolerance, orientation }`, where `tolerance` is present when `--chord` or `--angle` is given (the other value from `Session::tolerance(doc, Standard)`) and `orientation` is present when `--rotate` is given. For `py`, calls `export_python`.
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
| Document does not load | the load diagnostic (2.2) | 1 |
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

1. `partsmith --version` prints `partsmith 0.1.0 (protocol 1, format 1, OCCT 7.9.3)\n` and exits 0.
2. `partsmith` with no arguments exits 2.
3. `partsmith frobnicate` exits 2.

**validate**

4. `partsmith validate box.ptsm` prints `box.ptsm: ok\n` and exits 0.
5. `partsmith validate box.ptsm plate.ptsm` prints `box.ptsm: ok\nplate.ptsm: ok\n`.
6. A file containing `{"format": "partsmith",` gives one line starting `bad.ptsm:1:` and containing `error[parse_error]`, exit 1.
7. `partsmith validate missing.ptsm` prints `error: cannot read "missing.ptsm": ` followed by the OS message on standard error and exits 3.
8. `partsmith validate box.ptsm missing.ptsm bad.ptsm` exits 3 and still prints `box.ptsm: ok` and the `bad.ptsm` diagnostic.
9. `partsmith validate conflict.ptsm` exits 1 with a line containing `error[sketch_conflicting]`.
10. `box.ptsm` with `printer` set to `nope` validates with exit 0 and a line containing `warning[unknown_printer_profile]`; with `--deny-warnings` it exits 1.
11. `box.ptsm` with `height` set to `width * 2` gives a line that starts with `box.ptsm: error[`, contains `] at /parameters/0/expr: ` and contains `width`; exit 1.
12. `partsmith validate --canonical box.ptsm` exits 0; the same file re-indented with 4 spaces exits 1 with a line containing `error[not_canonical]`.
13. `partsmith validate --json box.ptsm bad.ptsm` prints a JSON document with `files[0]` `{"path": "box.ptsm", "ok": true, "diagnostics": []}` and `files[1].ok` false with a `parse_error` diagnostic that has `line` and `column`, and exits 1.
14. `partsmith validate --json missing.ptsm` prints a document whose only file has `ok` false and a diagnostic with code `io_error`, standard error is empty, exit 3.
15. `partsmith validate - < box.ptsm` prints `<stdin>: ok\n`.
16. `partsmith validate - -` exits 2.
17. A document with an `import` feature whose asset is in `assets/` validates with exit 0; after deleting the asset, it exits 1 with a line containing `error[asset_missing]`.
18. `partsmith validate --rebuild` of `box.ptsm` with the box length set to `-5 mm` exits 1 with a line containing `error[feature_error]` and `f-box1`; without `--rebuild` it exits 0.

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

## 7. Open issues

1. **Command list.** The architecture names `validate | fmt | rebuild | export | schema`. Print checks are an option of `rebuild` (`--checks`) rather than a separate command, and STEP import is left to the UI and Python in M1.
2. **Asset discovery.** The CLI passes every hash-named file in `assets/` and relies on the engine ignoring unreferenced ones (engine.md 3.14). A large shared asset folder is read fully; a lazy asset source can come later.
3. **`rebuild --json` and `examples/*/expected.json`.** The rebuild report carries volume, area, bounding box, element counts and findings, which are what `expected.json` stores. The examples spec may adopt this report as its format; this spec does not require it.
4. **Exit code for feature errors in `validate --rebuild` versus `rebuild`.** Both use exit 1; print findings never affect the exit code. A future `--deny-findings` can change that.
