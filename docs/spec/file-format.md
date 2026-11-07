# File format (`.ptsm`)

Behavior spec for the native Partsmith file: canonical JSON layout, numbers and strings, loading and saving, round-trip guarantees, `format_version` and migrations, and JSON Schema generation. It is implemented in `partsmith-model`; see [model.md](model.md) for the document types and their structural rules, and [expressions.md](expressions.md) for expression text.

Contracts come from [architecture.md](../design/architecture.md) ("Canonical JSON (`.ptsm`)" and "Serialization"), technical design section 6, and [ADR 0005](../adr/0005-feature-registry-with-generic-params.md). Interpretations are listed under [Open issues](#7-open-issues).

## 1. Purpose and scope

A `.ptsm` file is the recipe only (parameters, bodies, feature records), never geometry. It is canonical JSON: one serializer writes every file, so saving an unchanged document is byte-identical and a single edit to a leaf record is a one-line diff. The CLI (`validate`, `fmt`), the engine, the web worker and Python all load and save through this one implementation.

In this milestone (M1):

- `load` and `load_bytes` (text to `Document`) and `save` (`Document` to text).
- The canonical writer for any `serde_json::Value`: key order, 2-space layout with one-line leaf records, number and string formatting.
- Number normalization in memory.
- Round-trip guarantees.
- `format_version` 2, version checks, and the migration chain (`migrate`, `migrate_with`) with its one step from version 1, which changes nothing but the version (section 3.9, [ADR 0025](../adr/0025-file-format-version-2.md)).
- JSON Schema generation for `.ptsm` from the model types plus every registered feature's params schema (`ptsm_schema`, `ptsm_schema_text`), written by the CLI to `docs/format/ptsm.schema.json`.
- The `assets/` folder next to the document and content-hash references to imported files (section 3.11).

In M2 ([ADR 0061](../adr/0061-format-version-3-for-the-records-of-m2.md)):

- `format_version` 3 and a second migration step, from version 2, which changes nothing but the version (section 3.9).
- The printer profile a document stores when it names a profile that is not built in (`printer_profile`, section 3.2, [ADR 0062](../adr/0062-custom-printer-profiles-stored-in-documents-and-kept-by-every-host.md)).
- The records each M2 spec adds to version 3 before its code, until the M2 gate freezes version 3 (section 3.9).
- The printer profile's schema in the `.ptsm` JSON Schema (section 3.10).

Explicitly later (not in M1):

- `.ptsz` bundles (ZIP of recipe, assets and optional cache).
- The `.ptsm-cache/` folder (per-feature B-rep, display meshes, thumbnail).
- Multi-document projects: references between files by relative path and id. [ADR 0069](../adr/0069-component-scopes-by-containment-and-occurrence-paths.md) fixes their scope rules now (section 3.12). M4's components spec adds their records with a format version of its own.
- The git merge driver.
- A migration that rewrites document content (the steps from versions 1 and 2 only change the version number).
- A published `$id` URL for the schema (waits on the project domain).
- Visual diff of two documents.

## 2. Public API

All items are re-exported from the crate root; the writer helpers live in the module `partsmith_model::canonical`.

```rust
/// Value of the `format` field.
pub const FORMAT: &str = "partsmith";
/// Format version this build writes (3 from M2, ADR 0061; 2 in M1).
pub const FORMAT_VERSION: u32 = 3;
/// Oldest format version this build can migrate from.
pub const OLDEST_FORMAT_VERSION: u32 = 1;
/// File extension, without the dot.
pub const FILE_EXTENSION: &str = "ptsm";

/// A loaded document.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    /// The document: structurally valid, numbers normalized, at FORMAT_VERSION.
    pub document: Document,
    /// The format version of the text when it was older and had to be migrated.
    pub migrated_from: Option<u32>,
}

/// Parses `.ptsm` text (section 3.7).
pub fn load(text: &str) -> Result<Loaded, LoadError>;
/// Checks UTF-8, drops a leading byte order mark, then loads.
pub fn load_bytes(bytes: &[u8]) -> Result<Loaded, LoadError>;
/// Writes canonical `.ptsm` text (sections 3.2 to 3.5); refuses structurally invalid documents.
pub fn save(document: &Document) -> Result<String, SaveError>;

/// One step of the migration chain: upgrades a document from version `from` to `from + 1`.
#[derive(Clone, Copy, Debug)]
pub struct MigrationStep {
    /// The version this step upgrades from.
    pub from: u32,
    /// Rewrites the raw JSON; returns a user-facing message on failure.
    pub migrate: fn(serde_json::Value) -> Result<serde_json::Value, String>,
}

/// The built-in chain: the steps from versions 1 and 2, each of which returns the document unchanged (section 3.9).
pub const MIGRATIONS: &[MigrationStep] = &[
    MigrationStep { from: 1, migrate: migrate_from_1 },
    MigrationStep { from: 2, migrate: migrate_from_2 },
];

/// Brings raw document JSON to FORMAT_VERSION with the built-in chain; returns the original version if it changed.
pub fn migrate(document: serde_json::Value) -> Result<(serde_json::Value, Option<u32>), LoadError>;
/// Same as `migrate` with an explicit current version, oldest version and chain (for tests and tools).
pub fn migrate_with(
    document: serde_json::Value,
    current: u32,
    oldest: u32,
    steps: &[MigrationStep],
) -> Result<(serde_json::Value, Option<u32>), LoadError>;

/// A registered feature type and the JSON Schema of its params.
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureParamsSchema {
    /// Feature type name, such as "extrude".
    pub kind: String,
    /// Root schema of the typed params struct, as schemars generates it.
    pub schema: schemars::Schema,
}

/// Builds the `.ptsm` JSON Schema (section 3.10). `printer_profile` is the schema of print.md's written form
/// (`PrinterProfile`), which partsmith-model cannot name itself: the engine passes it (engine.md section 2).
pub fn ptsm_schema(features: &[FeatureParamsSchema], printer_profile: &schemars::Schema) -> Result<schemars::Schema, SchemaError>;
/// The same schema as canonical text, ready to write to docs/format/ptsm.schema.json.
pub fn ptsm_schema_text(features: &[FeatureParamsSchema], printer_profile: &schemars::Schema) -> Result<String, SchemaError>;

pub mod canonical {
    /// Width limit for one-line containers, in characters.
    pub const MAX_LINE_WIDTH: usize = 120;
    /// Spaces per indentation level.
    pub const INDENT_WIDTH: usize = 2;

    /// Writes any JSON value in the canonical layout, with a trailing line feed (section 3.5).
    pub fn to_string(value: &serde_json::Value) -> String;
    /// Writes one number in canonical form (section 3.3).
    pub fn format_number(number: &serde_json::Number) -> String;
    /// Writes one string, quoted and escaped (section 3.4).
    pub fn format_string(text: &str) -> String;
    /// Serializes `value` to JSON, rejecting NaN and infinity, with numbers normalized.
    pub fn to_value<T: serde::Serialize + ?Sized>(value: &T) -> Result<serde_json::Value, CanonicalError>;
    /// Rewrites every number in `value` to its normalized form (section 3.3).
    pub fn normalize_numbers(value: &mut serde_json::Value);
    /// Strict JSON parse used by `load`: rejects duplicate keys and deep nesting, normalizes numbers.
    pub fn parse(text: &str) -> Result<serde_json::Value, LoadError>;
}
```

Error enums (`LoadError`, `SaveError`, `CanonicalError`, `SchemaError`) are listed with their fields and messages in section 4. The JSON representation of each document type is in model.md section 2.11; complete files are in section 3.6 below.

## 3. Behavior

### 3.1 Files

- Extension `.ptsm`. UTF-8 without a byte order mark. LF line endings only, and the text ends with exactly one line feed. Indentation is spaces (never tabs). No line ends with a space.
- Every container that fits prints on one line, so most lines are at most 120 characters. A line is longer only when it holds one scalar member or element that cannot be split (such as a long string), or when a one-line container that is exactly 120 characters wide is followed by its separator comma.
- JSON numbers stored directly in feature params are in internal units: millimeters for lengths and radians for angles, whatever the document units. Only expression strings use the document units for bare numbers (expressions.md section 3.10).

### 3.2 Document layout and field rules

Keys are written in struct declaration order. A field is omitted exactly when the table says so.

`Document` (the top-level object):

| Key | JSON type | Required on load | When absent | Written |
| --- | --- | --- | --- | --- |
| `format` | string, must be `"partsmith"` | yes | `NotPartsmith` | always |
| `format_version` | integer | yes | `MissingFormatVersion` | always |
| `units` | `"mm"`, `"cm"`, `"m"` or `"in"` | yes | `Invalid` | always |
| `printer` | string | no | no printer | when present |
| `printer_profile` | object, print.md's written form | no | no stored profile | when present |
| `parameters` | array of `Parameter` | no | empty | always, even when empty |
| `bodies` | array of `Body` | no | empty | always, even when empty |
| `features` | array of `FeatureRecord` | no | empty | always, even when empty |
| `rollback` | id string | no | nothing rolled back | when present |

`Parameter`:

| Key | JSON type | Required | When absent | Written |
| --- | --- | --- | --- | --- |
| `id` | id string | yes | `Invalid` | always |
| `name` | string | yes | `Invalid` | always |
| `expr` | string | yes | `Invalid` | always |
| `comment` | string | no | none | when present |

`Body`:

| Key | JSON type | Required | When absent | Written |
| --- | --- | --- | --- | --- |
| `id` | id string | yes | `Invalid` | always |
| `name` | string | yes | `Invalid` | always |
| `material` | string | no | none | when present |
| `color` | `"#rrggbb"` | no | none | when present, lowercase |
| `visible` | boolean | no | `true` | only when `false` |
| `role` | `"part"`, `"negative_volume"`, `"support_blocker"`, `"support_enforcer"`, `"settings_modifier"` | no | `"part"` | only when not `"part"` |

`FeatureRecord`:

| Key | JSON type | Required | When absent | Written |
| --- | --- | --- | --- | --- |
| `id` | id string | yes | `Invalid` | always, first |
| `type` | string | yes | `Invalid` | always, second |
| `name` | string | no | none | when present |
| `suppressed` | boolean | no | `false` | only when `true` |
| any other key | any JSON | no | | every param, in stored order, after the keys above |

Rules:

- On load, an explicit `null` for an optional string or id field (`printer`, `rollback`, `comment`, `material`, `color`, feature `name`) or for `printer_profile` means absent. `null` for `visible`, `suppressed`, `role`, `units` or an array is an `Invalid` error.
- On load, `Document`, `Parameter` and `Body` reject unknown keys (`Invalid` with the key's path). A `FeatureRecord` cannot: every key other than its four record keys is a param.
- `printer_profile` holds the printer profile that `printer` names, in print.md's written form (print.md sections 2.2 and 3.1), when that profile is not built in ([ADR 0062](../adr/0062-custom-printer-profiles-stored-in-documents-and-kept-by-every-host.md)). Any other JSON value than an object is an `Invalid` error with the path `printer_profile`. The model checks only how it relates to `printer`: `printer` is present, and the object's `id` member is a string equal to `printer` (model.md section 3.6, rules 9 and 10). `load` and `save` do not read the object's other members. They keep them in the order and form they have in memory, as they keep feature params. The engine checks the whole profile against print.md's written form when it opens, formats or validates a text. It also rewrites the members into the written form's key order (`id`, `name`, `technology`, `bed`, `nozzle_diameter`, `layer_height`, `max_overhang_angle`, `max_bridge_length`, `fit`; engine.md section 3.2). A version 2 document never has this key.
- Feature params keep the order they have in memory. The model never reorders them. The feature registry canonicalizes them (the order of the feature's typed params struct, then unknown keys in their original order) when it validates a loaded document or an edit; `partsmith fmt` runs load, registry canonicalization, then save.

### 3.3 Numbers

**Number model.** Every number in a document is an IEEE 754 double. `NaN` and infinities never appear: JSON text cannot express them, `canonical::to_value` rejects them, and numbers outside the double range are a syntax error on load.

**Normalized form in memory.** A finite number `v` is stored in a `serde_json::Number` as:

- an integer (`u64` when `v >= 0`, otherwise `i64`) when `v` has no fractional part and `abs(v) < 2^53`. Negative zero becomes the integer `0`;
- an `f64` otherwise. Integer literals with `abs(v) >= 2^53` are converted to the nearest `f64` (so `9007199254740993` becomes `9007199254740992.0`).

`canonical::parse` (and so `load`), `canonical::to_value`, `Document::normalize` and `Document::apply` produce normalized numbers. Because integral values are integers, a typed `u32` param (such as a pattern count) deserializes from them directly.

**Written form** (`format_number`), for the value `v` of the number as an `f64`:

1. `v == 0` (either sign): `0`.
2. `v` has no fractional part and `abs(v) < 2^53` (9007199254740992): the integer in decimal, with a leading `-` when negative, no fraction and no exponent.
3. Otherwise: the shortest decimal string that parses back to the same double, as written by `ryu::Buffer::format_finite`. Its exact shape: let the shortest digits be `d1 d2 ... dn` and the decimal exponent `E` such that `abs(v) = d1.d2...dn x 10^E`. When `-5 <= E <= 15` the number is written in plain decimal notation, with `.0` appended if it has no fractional digits (this happens only for integral values from 2^53 up to 1e16); otherwise it is written as `d1`, then `.d2...dn` if `n > 1`, then `e`, then `E` (with `-` when negative, never `+`, no leading zeros). A leading `-` marks negative values.

| Value | Written |
| --- | --- |
| `80.0` | `80` |
| `-0.0` | `0` |
| `1e2` | `100` |
| `-40` | `-40` |
| `0.1` | `0.1` |
| `-2.5` | `-2.5` |
| `12345678.9` | `12345678.9` |
| `0.30000000000000004` | `0.30000000000000004` |
| `0.000015` | `0.000015` |
| `1e-5` | `0.00001` |
| `1e-7` | `1e-7` |
| `-1e-7` | `-1e-7` |
| `9007199254740991` | `9007199254740991` |
| `9007199254740992` | `9007199254740992.0` |
| `1e16` | `1e16` |
| `1.5e300` | `1.5e300` |
| `5e-324` | `5e-324` |
| `1.7976931348623157e308` | `1.7976931348623157e308` |

### 3.4 Strings

`format_string` writes a string in double quotes. Inside it:

- `"` is written `\"` and `\` is written `\\`.
- U+0008 is `\b`, U+0009 is `\t`, U+000A is `\n`, U+000C is `\f`, U+000D is `\r`.
- Every other character from U+0000 to U+001F is `\u00XX` with lowercase hex digits (U+001F is `\u001f`).
- Every other character, including `/`, U+007F, U+2028 and all non-ASCII characters, is written as itself in UTF-8.

Object keys are written the same way. On load any valid JSON escape is accepted (`é`, `\/`, surrogate pairs), so a non-canonical spelling is rewritten to the canonical one on the next save. Example: the string with the characters `Lid "v2" \ draft`, a tab, `A`, `é`, `°`, `/x` and U+001F is written as `"Lid \"v2\" \\ draft\tAé°/x\u001f"`.

### 3.5 Canonical layout

`canonical::to_string(value)` writes `value` in this layout, and `save` is `canonical::to_string` of the document serialized with `canonical::to_value`.

Definitions:

- The **one-line form** of a value: `null`, `true` and `false`; numbers by `format_number`; strings by `format_string`; an array as `[`, its elements' one-line forms joined by `, `, then `]`; an object as `{`, each member as `format_string(key)`, `: ` and the value's one-line form, joined by `, `, then `}`. Empty containers are `[]` and `{}`.
- A value **contains an array of objects** when it is an array with at least one element that is an object, or when any element (of an array) or member value (of an object) contains an array of objects, at any depth. Scalars never do.
- **Width** of a candidate line: the number of Unicode scalar values in the indentation, plus the member's key prefix (`format_string(key)` followed by `: `, or nothing for an array element), plus the value's one-line form. The separator comma that may follow is not counted.

Algorithm (indentation in spaces):

```text
write(value, indent, prefix_width, is_root):
    if value is null, a boolean, a number or a string:
        emit one_line(value)
    else if value is an empty array or an empty object:
        emit "[]" or "{}"
    else if not is_root
            and not contains_array_of_objects(value)
            and indent + prefix_width + chars(one_line(value)) <= 120:
        emit one_line(value)
    else:
        emit "[" or "{", then a line feed
        for each element or member, in order:
            emit indent + 2 spaces
            if it is an object member:
                key_prefix = format_string(key) + ": "
                emit key_prefix
                write(child, indent + 2, chars(key_prefix), false)
            else:
                write(child, indent + 2, 0, false)
            emit "," if it is not the last one, then a line feed
        emit indent spaces, then "]" or "}"

to_string(value) = write(value, 0, 0, true) followed by one line feed
```

Consequences:

- The root container is always expanded (unless empty), so every `.ptsm` file starts with `{` on its own line followed by `  "format": "partsmith",` and `  "format_version": 3,`.
- Every record list (`parameters`, `bodies`, `features`, sketch `entities` and `constraints`) is an array of objects, so it is always expanded with one record per line, and each leaf record prints on one line when it fits.
- A record that contains an array of objects (a sketch with entities, a shell with face references) is expanded one member per line, and so is any record whose one-line form is too wide; its scalar and small members stay on one line each.
- The layout decision for a value depends only on the value, its indentation and its key, never on its siblings or on whether it is last.

### 3.6 Examples

Each example is the exact output of `save` (or `canonical::to_string`), shown without the final line feed that the text ends with. Byte counts include that line feed. Feature params in E2 and E3 follow the feature specs. E5, E7 and E8 use params no registered feature accepts (an extrude profile with a `note`, the unregistered type `polygon_demo`, a box with unknown keys), because loading and saving never consult the registry and the layout is the same whatever the params are. E10's params follow the feature specs with the version 2 fields of section 3.9.

**E1. Empty document.** `save(&Document::new(Units::Mm))`, 122 bytes:

```json
{
  "format": "partsmith",
  "format_version": 3,
  "units": "mm",
  "parameters": [],
  "bodies": [],
  "features": []
}
```

**E2. Full document**, 1614 bytes. Parameters and the body print one per line; the sketch is expanded because it contains arrays of objects; its plane stays on one line, and its entities and constraints print one per line; the extrude is expanded because its profile contains an array of region objects, so the profile and its `regions` list print expanded too; the shell is expanded because `remove_faces` is an array of objects.

```json
{
  "format": "partsmith",
  "format_version": 3,
  "units": "mm",
  "printer": "generic-fdm",
  "parameters": [
    {"id": "p-wall", "name": "wall", "expr": "2 mm", "comment": "Outer wall thickness"},
    {"id": "p-clear", "name": "clearance", "expr": "printer.fit.slip"}
  ],
  "bodies": [
    {"id": "b-1", "name": "Enclosure", "material": "PETG", "color": "#3b82f6"}
  ],
  "features": [
    {
      "id": "f-sk1",
      "type": "sketch",
      "plane": {"kind": "origin", "plane": "XY"},
      "entities": [
        {"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]},
        {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]},
        {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]},
        {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}
      ],
      "constraints": [
        {"id": "c-1", "type": "coincident", "points": ["e-1.end", "e-2.start"]},
        {"id": "c-2", "type": "horizontal", "line": "e-1"},
        {"id": "c-3", "type": "distance", "points": ["e-1.start", "e-1.end"], "expr": "80 mm"}
      ]
    },
    {
      "id": "f-ext1",
      "type": "extrude",
      "body": "b-1",
      "profile": {
        "sketch": "f-sk1",
        "regions": [
          {"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}
        ]
      },
      "extent": {"kind": "distance", "expr": "30 mm"},
      "op": "new"
    },
    {
      "id": "f-shell1",
      "type": "shell",
      "body": "b-1",
      "thickness": "wall",
      "remove_faces": [
        {"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}
      ]
    }
  ]
}
```

**E3. Defaults, record keys and rollback**, 630 bytes. Body `b-1` has every default; `b-2` is hidden with a role. `f-box1` has a name; its one-line form would be 122 characters wide, so it is expanded. `f-cyl1` is suppressed. `rollback` comes last (`f-cyl1` is rolled back).

```json
{
  "format": "partsmith",
  "format_version": 3,
  "units": "mm",
  "parameters": [],
  "bodies": [
    {"id": "b-1", "name": "Base"},
    {"id": "b-2", "name": "Keep out", "color": "#ff0000", "visible": false, "role": "support_blocker"}
  ],
  "features": [
    {
      "id": "f-box1",
      "type": "box",
      "name": "Base block",
      "body": "b-1",
      "size": ["40 mm", "30 mm", "10 mm"],
      "op": "new"
    },
    {
      "id": "f-cyl1",
      "type": "cylinder",
      "suppressed": true,
      "body": "b-1",
      "radius": "3 mm",
      "height": "10 mm",
      "op": "cut"
    }
  ],
  "rollback": "f-cyl1"
}
```

**E4. Width limit**, 399 bytes. Parameter `p-1` has the comment `Fits exactly ` followed by 49 `x` characters (62 characters); its line is exactly 120 characters wide, so it stays on one line (121 with the comma). Parameter `p-2` has the comment `One too long ` followed by 50 `x` characters (63 characters); its line would be 121 wide, so it is expanded.

```json
{
  "format": "partsmith",
  "format_version": 3,
  "units": "mm",
  "parameters": [
    {"id": "p-1", "name": "a", "expr": "1", "comment": "Fits exactly xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"},
    {
      "id": "p-2",
      "name": "b",
      "expr": "2",
      "comment": "One too long xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
    }
  ],
  "bodies": [],
  "features": []
}
```

**E5. The key prefix counts**, 463 bytes. The `profile` member's value is `{"sketch": "f-sk1", "note": <73 n characters>}`, whose one-line form is 104 characters. At indentation 6 it would fit alone (110), but with the key prefix `"profile": ` (11 characters) the line is 121, so the object is expanded.

```json
{
  "format": "partsmith",
  "format_version": 3,
  "units": "mm",
  "parameters": [],
  "bodies": [
    {"id": "b-1", "name": "Plate"}
  ],
  "features": [
    {
      "id": "f-ext1",
      "type": "extrude",
      "body": "b-1",
      "profile": {
        "sketch": "f-sk1",
        "note": "nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn"
      },
      "extent": {"kind": "distance", "expr": "30 mm"},
      "op": "new"
    }
  ]
}
```

**E6. Width counts characters, not bytes**, 337 bytes. The body name is 91 copies of `é` (2 bytes each in UTF-8). The body's line is 120 characters (211 bytes), so it stays on one line.

```json
{
  "format": "partsmith",
  "format_version": 3,
  "units": "mm",
  "parameters": [],
  "bodies": [
    {"id": "b-1", "name": "ééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééé"}
  ],
  "features": []
}
```

**E7. Arrays, empty containers and other JSON values**, 626 bytes, document units `in`. `points` of `f-poly` is an array of arrays with no objects; its one-line form is too wide, so it is expanded with one pair per line. `f-small` is too wide for one line, but its short `points` array stays on one line, and `[]`, `{}`, `true` and `null` are written as such.

```json
{
  "format": "partsmith",
  "format_version": 3,
  "units": "in",
  "parameters": [],
  "bodies": [],
  "features": [
    {
      "id": "f-poly",
      "type": "polygon_demo",
      "points": [
        [0, 0],
        [80, 0],
        [80, 50],
        [40, 70],
        [0, 50],
        [0, 25.5],
        [10.25, 12.125],
        [3.5, 1.75],
        [0.5, 0.25],
        [1, 2]
      ]
    },
    {
      "id": "f-small",
      "type": "polygon_demo",
      "points": [[0, 0], [80, 0], [80, 50]],
      "empty_list": [],
      "empty_map": {},
      "flag": true,
      "nothing": null
    }
  ],
  "rollback": "f-poly"
}
```

**E8. Normalization of a non-canonical file.** This input, with CRLF line endings (each line ends with `\r\n`):

```text
{"units":"mm","format_version":2.0,"format":"partsmith",
 "bodies":[{"name":"Lid","id":"b-1","visible":true,"role":"part","color":"#3B82F6","material":null}],
 "features":[{"type":"box","id":"f-1","suppressed":false,"name":null,"size":["10 mm","10 mm","2 mm"],"x":1.0,"label":"café \/ bar"}]}
```

loads, and `save` of the result is exactly (282 bytes): the version 2 input migrated to version 3 (section 3.9), record keys in declaration order, defaults and nulls dropped, the color lowercased, `1.0` written as `1`, escapes rewritten, the missing `parameters` written as `[]`, and the params `size`, `x`, `label` kept in their file order.

```json
{
  "format": "partsmith",
  "format_version": 3,
  "units": "mm",
  "parameters": [],
  "bodies": [
    {"id": "b-1", "name": "Lid", "color": "#3b82f6"}
  ],
  "features": [
    {"id": "f-1", "type": "box", "size": ["10 mm", "10 mm", "2 mm"], "x": 1, "label": "café / bar"}
  ]
}
```

**E9. Small values through `canonical::to_string`** (shown with `\n` for line feeds):

| Value | Output |
| --- | --- |
| `5` | `5\n` |
| `"a"` | `"a"\n` |
| `{}` | `{}\n` |
| `[]` | `[]\n` |
| `{"a": 1}` | `{\n  "a": 1\n}\n` |
| `[1, 2]` | `[\n  1,\n  2\n]\n` |
| `{"a": {"b": [{"c": 1}]}}` | `{\n  "a": {\n    "b": [\n      {"c": 1}\n    ]\n  }\n}\n` |
| `{"m": [1, {"x": 1}]}` | `{\n  "m": [\n    1,\n    {"x": 1}\n  ]\n}\n` |
| `{"g": [[{"x": 1}]]}` | `{\n  "g": [\n    [\n      {"x": 1}\n    ]\n  ]\n}\n` |
| `{"k\"ey": [1, 2], "n": null, "t": true}` | `{\n  "k\"ey": [1, 2],\n  "n": null,\n  "t": true\n}\n` |

**E10. Format version 2 fields**, 3081 bytes. Every field version 2 adds (section 3.9): a region reference with `outline` in each extrude, a face sketch plane with `fallback_frame`, a fillet edge reference with anchors (`nb`), an edge axis with `direction`, and a face reference with `merged`. The extrude profiles, the slot sketch's plane and the datum's face reference print as in E2; the fillet's edge reference and the datum's face reference, whose lines would be 179 and 152 characters wide, are expanded one member per line, and their `nb` and `merged` maps stay on one line each.

```json
{
  "format": "partsmith",
  "format_version": 3,
  "units": "mm",
  "parameters": [],
  "bodies": [
    {"id": "b-1", "name": "Block"}
  ],
  "features": [
    {
      "id": "f-sk1",
      "type": "sketch",
      "plane": {"kind": "origin", "plane": "XY"},
      "entities": [
        {"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]},
        {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]},
        {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]},
        {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}
      ],
      "constraints": []
    },
    {
      "id": "f-ext1",
      "type": "extrude",
      "body": "b-1",
      "profile": {
        "sketch": "f-sk1",
        "regions": [
          {"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-1,e-2,e-3,e-4"}
        ]
      },
      "extent": {"kind": "distance", "expr": "30 mm"},
      "op": "new"
    },
    {
      "id": "f-sk2",
      "type": "sketch",
      "plane": {
        "kind": "face",
        "body": "b-1",
        "face": {"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"},
        "fallback": {"origin": [0, 0, 30], "x_axis": [1, 0, 0], "normal": [0, 0, 1]},
        "fallback_frame": "body"
      },
      "entities": [
        {"id": "e-5", "type": "line", "start": [30, -1], "end": [50, -1]},
        {"id": "e-6", "type": "line", "start": [50, -1], "end": [50, 51]},
        {"id": "e-7", "type": "line", "start": [50, 51], "end": [30, 51]},
        {"id": "e-8", "type": "line", "start": [30, 51], "end": [30, -1]}
      ],
      "constraints": []
    },
    {
      "id": "f-ext2",
      "type": "extrude",
      "body": "b-1",
      "profile": {
        "sketch": "f-sk2",
        "regions": [
          {"key": "e-5,e-6,e-7,e-8", "fp": "c=40,25|a=1040", "outline": "e-5,e-6,e-7,e-8"}
        ]
      },
      "extent": {"kind": "distance", "expr": "5 mm"},
      "reverse": true,
      "op": "cut"
    },
    {
      "id": "f-fil1",
      "type": "fillet",
      "body": "b-1",
      "edges": [
        {
          "q": "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)",
          "fp": "line|n=1,0,0|c=65,0,30|l=30|adj=2,2",
          "nb": {"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]}
        }
      ],
      "radius": "2 mm"
    },
    {
      "id": "f-lp1",
      "type": "linear_pattern",
      "body": "b-1",
      "direction": {
        "axis": {
          "kind": "edge",
          "body": "b-1",
          "edge": {"q": "edge(f-ext1/lateral/e-1|f-ext1/start-cap)", "fp": "line|n=1,0,0|c=40,0,0|l=80|adj=2,2"},
          "direction": [1, 0, 0]
        },
        "count": "2",
        "spacing": "80 mm"
      }
    },
    {
      "id": "f-dp1",
      "type": "datum_plane",
      "base": {
        "kind": "face",
        "body": "b-1",
        "face": {
          "q": "f-ext1/start-cap",
          "fp": "plane|n=0,0,-1|c=80,25,0|a=8000|adj=4,4",
          "merged": {"f-ext1/start-cap": ["f-ext1/start-cap@f-lp1:*"]}
        }
      },
      "offset": "10 mm"
    }
  ]
}
```

**E11. A stored printer profile**, 588 bytes. The document names the printer `my-mk4`, which is not built in. It stores that profile in print.md's written form (section 3.2). The profile is the input `{"id": "my-mk4", "name": "My MK4", "technology": "fdm", "bed": {"kind": "rectangle", "width": 250, "depth": 210, "height": 220}, "layer_height": 0.15, "fit": {"slip": 0.15, "loose": 0.3}}` with every other value from `generic-fdm` (print.md section 3.1). It is expanded because its one-line form is too wide; `bed` and `fit` stay on one line each.

```json
{
  "format": "partsmith",
  "format_version": 3,
  "units": "mm",
  "printer": "my-mk4",
  "printer_profile": {
    "id": "my-mk4",
    "name": "My MK4",
    "technology": "fdm",
    "bed": {"kind": "rectangle", "width": 250, "depth": 210, "height": 220},
    "nozzle_diameter": 0.4,
    "layer_height": 0.15,
    "max_overhang_angle": 0.7853981633974483,
    "max_bridge_length": 10,
    "fit": {"press": 0.05, "snug": 0.1, "slip": 0.15, "loose": 0.3}
  },
  "parameters": [
    {"id": "p-clear", "name": "clearance", "expr": "printer.fit.slip"}
  ],
  "bodies": [],
  "features": []
}
```

### 3.7 Loading

`load(text)` runs these steps in order and returns the first error:

1. Drop one leading U+FEFF if present.
2. `canonical::parse`: strict JSON (RFC 8259: no comments, no trailing commas, no single quotes, no `NaN` or `Infinity`), any whitespace including CR and tabs between tokens, nesting of arrays and objects at most 127 levels deep (the JSON parser's recursion limit), numbers within the double range. Errors: `Syntax` (with the 1-based line and column of the problem) or `DuplicateKey` (a key repeated in one object, at any depth, including inside params). Every number is normalized.
3. The root must be an object with `"format": "partsmith"` (a string); otherwise `NotPartsmith`.
4. `migrate` (section 3.9): version checks and migration steps.
5. Deserialize the `Document` (section 3.2 rules). Errors are `Invalid { path, message }`, where `path` locates the offending key: object keys joined by `.`, array indices as `[i]`, the root as the empty string. Examples: `units`, `parameters[0].id`, `bodies[1].color`, `bodies[0].colour` (an unknown key), `features[2]` (a record missing `type`). For a missing required field the path is the object that lacks it and the message names the field.
6. `check_structure()` (model.md section 3.6). If it reports anything, return `Structure` with the full list.
7. Return `Loaded { document, migrated_from }`.

`load_bytes(bytes)` drops a leading `EF BB BF`, returns `NotUtf8 { offset }` (offset of the first invalid byte in `bytes`) if the rest is not UTF-8, and otherwise calls `load`.

Loading never consults the feature registry. A file with unknown feature types or invalid params loads; the registry reports those through `Document::validate` and recompute. Loading never consults partsmith-print either: a stored printer profile that print.md would refuse loads, and the engine refuses it when it opens the text (engine.md section 3.2).

### 3.8 Round-trip guarantees

1. `save` is deterministic: the same document gives the same bytes on every run, platform and build (no hash-map order, no locale, no time).
2. For every structurally valid, normalized document `doc` (anything `load`, `Document::new` or `apply` produces): `load(&save(&doc)?)?.document == doc`.
3. For every text `t` produced by `save`: `save(&load(t)?.document)? == t`. Such text is called canonical.
4. For every text `t` that loads: `save(load(t))` is canonical (loading and saving it again changes nothing).
5. An expression string is written exactly as stored, so `Expr` text with unusual spacing (`" 2 mm "`) round-trips unchanged.

These guarantees cover params key order as stored. Full canonical form for hand-written files also needs the registry's params ordering (section 3.2).

### 3.9 Versioning and migration

- `format_version` is a non-negative integer. This build reads and writes version 3 (`FORMAT_VERSION`) and can migrate from `OLDEST_FORMAT_VERSION` (1). An M1 build read and wrote version 2.
- Any change to the meaning or shape of saved documents that older readers would misread bumps `FORMAT_VERSION` and adds one `MigrationStep` from the old version. Changes need a format ADR and human review (AGENTS.md).
- Migrations work on raw JSON (`serde_json::Value`), before typed deserialization, so old shapes never need Rust types.

| Version | What it adds |
| --- | --- |
| 1 | The M1 document. |
| 2 | Optional fields inside feature params, each omitted when absent ([ADR 0025](../adr/0025-file-format-version-2.md)): `nb` (anchors) and `merged` (merged names) on face, edge and vertex references, in the key order `q`, `fp`, `nb`, `merged` (naming.md section 2.4); `outline` on region references, after `key` and `fp` (sketch.md section 2.7); `fallback_frame` on face sketch planes, after `fallback`, omitted for `"world"` (sketch.md section 2.2); `direction` on edge and face axes (features.md section 2.5). |
| 3 | Declared in M2's wave 1 and frozen at the M2 gate ([ADR 0061](../adr/0061-format-version-3-for-the-records-of-m2.md)), with these records so far. The printer profile a document stores ([ADR 0062](../adr/0062-custom-printer-profiles-stored-in-documents-and-kept-by-every-host.md)): `printer_profile`, after `printer`, an object in print.md's written form, present only when `printer` names a profile that is not built in (section 3.2). The sketch entity records of wave 1's sketch contracts step ([ADR 0066](../adr/0066-sketch-entities-of-format-version-3-operations-and-display-polylines.md), sketch.md sections 2.12 and 3.20), each written once the item that builds it lands: the entity types `ellipse`, `elliptical_arc`, `parabola`, `hyperbola`, `conic`, `bspline`, `fit_spline` and `fixed_spline` (M2 item 14) and `text` (item 27), each record's keys in the order of sketch.md section 2.12 with `link` and `construction` last; the member `link` of an entity, a `SketchLink` tagged `kind` (`edge`, `vertex`, `section`, `silhouette` or `sketch`), omitted when absent (item 26); and the point roles `vertex`, `focus` and `control` of `PointRef` (item 14). No new field holds a kernel index or indexes glyphs or characters. A text names its font by an asset name or a built-in name that fixes the font's bytes. The chain reference of a sketch's curves ([ADR 0059](../adr/0059-curve-chains-and-the-region-union-in-the-sketch-crate.md)): `ChainProfileRef`, an object with `sketch` and then `chains`, a list of `ChainRef` objects with `key` (the chain key text of sketch.md section 3.18.1), `fp` (the chain fingerprint text of sketch.md section 3.18.5) and `closed`, omitted when false (features.md section 2.5). Only a feature that takes curve chains stores it, from the sweep on (M2 item 18). Direct-generator names in the `q` of edge and vertex references: `edge:` or `vertex:`, then `<feature>/<role>[/<sketch-ref>]` and the name's `#n` and `@<feature>:<instance>` groups (naming.md section 3.12, [ADR 0060](../adr/0060-direct-generator-names-for-edges-and-vertices.md)). No field is added, and the schemas of `ElementName` and `Reference` do not change. Such a reference has no `nb` and no `merged`. A version 2 document holds none: M1 builds write none, and the step from version 2 changes only the version. Each later M2 spec adds here the feature records of its feature types and the new params and values of existing types, before its code (the rules below). Every key, record and value of version 3 keeps the scope rules of section 3.12 ([ADR 0069](../adr/0069-component-scopes-by-containment-and-occurrence-paths.md)). |

**The step from version 1** (`migrate_from_1`, private) returns the document unchanged; the chain then sets `format_version` to 2. Every version 1 document means the same at version 2, because each field version 2 adds keeps the version 1 meaning when it is absent: a face sketch's fallback is in world coordinates, a reference has no anchors or merged names, and a region reference has no outline. One rule is new: an edge or face axis without `direction` is oriented by the direction of its stored fingerprint (features.md section 3.6), where version 1 made the first nonzero component of the axis positive. The two agree except for an axis whose direction has crossed a coordinate plane since it was picked, such as an edge that leaned 1 mm the other way (reference stability corpus R38): there the version 1 rule flipped the axis, which was the bug, and version 2 keeps the side it was picked on ([ADR 0023](../adr/0023-frames-live-on-the-model.md)). So opening a version 1 document and saving it without edits changes only its `format_version` line, and it rebuilds with the same placements, volumes and statuses as before version 2 existed (test 57), except for such an axis. The loader never reads these fields: they live in feature params, which `load` and `save` keep as they are (section 3.2), and their shapes and checks belong to naming.md, sketch.md and features.md. New picks and attaches write them; nothing adds them to an existing reference on its own.

**The step from version 2** (`migrate_from_2`, private) returns the document unchanged; the chain then sets `format_version` to 3. Every version 2 document means the same at version 3. Version 3 adds only keys, records and values that a version 2 document cannot hold, and each keeps the version 2 meaning when it is absent. A version 2 document stores no printer profile. Its `printer` resolves by id as before: a profile the session holds, then a built-in profile, then the default FDM profile with the `unknown_printer_profile` warning (engine.md section 3.7). Opening a version 2 document and saving it without edits changes only its `format_version` line (test 59). It rebuilds with the same values, statuses and messages, except for the new words of that warning (engine.md section 3.4). A version 1 document passes both steps. The step from version 2 reads nothing. It keeps every key it is given, including keys no version defines.

**Version 3 grows until the M2 gate** ([ADR 0061](../adr/0061-format-version-3-for-the-records-of-m2.md)). Wave 1 declares it with the printer profile and the sketch entity records. Later M2 specs add their records before their code. These rules hold until the gate freezes it:

1. The spec that adds a key, record or value to version 3 lists it in the row of version 3 above, in the same change that adds it to that spec.
2. Each addition is an optional key that is omitted when absent and keeps the earlier meaning then, a new record type, or a new value of a tagged enum. A key, record or value that a version 3 build has written keeps its shape and meaning until the gate. A change that would alter one waits for version 4 and its own migration step.
3. Every version 3 file written before the gate therefore opens in every later build with the same meaning. This matters because the production web deployment of `main` writes version 3 from the merge of wave 1 on (delivery.md section 3.3).
4. No build that writes version 3 is tagged before the M2 gate commit. The first tag comes from a commit that writes version 2 (delivery.md section 3.6, release checklist item 3).
5. At the gate, human review accepts version 3 with every record in its row. From then on, a change that this section says needs a new version needs version 4.

**What needs a new version after the gate.** A new feature type alone does not. An older build loads its record, reports `unknown_type` on it with features.md's message ("This document has a feature of type 'sweep', which this version of Partsmith does not have. Update Partsmith, or delete the feature."), builds the rest, and saves the record unchanged (section 3.2, engine.md section 3.2). Nothing is misread. These need one:

- A new key in the params of an existing feature type. An older build ignores the key with the `unknown_params` warning and builds another shape.
- A new value of an enum that an existing record already holds, such as a new extent `kind`. An older build cannot read the record, and its message does not say to update.
- A new sketch entity or constraint type. An older build cannot read the sketch, and every feature that uses it fails.
- A new key of `Document`, `Parameter` or `Body`. An older build refuses the file with an `Invalid` error whose message does not say to update.
- A new form of element name in a reference's `q`. An older build cannot parse the name, and the feature that holds the reference fails with `invalid_params`, whose message does not say to update.
- A rule of print.md section 3.1 that refuses a stored printer profile that an earlier build accepted (print.md section 3.1, stored profiles).

A build that reads only version 1 rejects a version 2 file with `NewerFormatVersion` in step 2 below, before it deserializes anything, instead of failing on an unknown field inside a reference or a sketch plane. In the same way an M1 build, which reads up to version 2, rejects every version 3 file with `This file uses format version 3, which is newer than this version of Partsmith supports (up to 2). Update Partsmith to open it.` It never opens the file. It therefore never drops a stored printer profile or a record it cannot read when it saves.

`migrate_with(document, current, oldest, steps)`:

1. Read `format_version` from the root object. Missing: `MissingFormatVersion`. Not a non-negative integer (a string, a boolean, a fractional or negative number; the integer `1.0` is fine because numbers are already normalized): `InvalidFormatVersion { found }`, with `found` the value's one-line canonical form.
2. `version > current`: `NewerFormatVersion { found: version, supported: current }`. `version < oldest`: `UnsupportedFormatVersion { found: version, oldest }`. `version == current`: return the document unchanged with `None`.
3. For `v` from `version` to `current - 1`: find the step with `from == v` (the slice may be in any order); if there is none, `MissingMigration { from: v }`. Call its function; on `Err(message)` return `Migration { from: v, to: v + 1, message }`; if it returns something other than a JSON object, return `Migration { from: v, to: v + 1, message: "The upgraded document is not a JSON object." }`. Then set the root `format_version` to `v + 1`, whatever the step wrote there.
4. Return the migrated document with `Some(version)`.

`migrate(document)` is `migrate_with(document, FORMAT_VERSION, OLDEST_FORMAT_VERSION, MIGRATIONS)`. It returns version 3 documents unchanged with `None`, brings version 1 and version 2 documents to version 3 with `Some(1)` and `Some(2)`, and rejects every other version. When `load` migrates, the engine tells the user that saving will write the new version.

### 3.10 JSON Schema

`ptsm_schema(features, printer_profile)` returns a JSON Schema (draft 2020-12) of canonical `.ptsm` documents. It describes what `save` writes; the loader accepts slightly more (section 3.2: nulls, a BOM, non-canonical numbers and key order).

Input checks, returning the first error:

1. In input order: each `kind` must satisfy `is_valid_feature_type` (`InvalidKind`) and be unique (`DuplicateKind`).
2. In kind order (byte order): each params schema must have `"type": "object"` at its root (`NotAnObject`); must not set `additionalProperties` or `unevaluatedProperties` to `false` at its root (`ClosedParams`), because records keep unknown keys; and must not declare a reserved key (`id`, `type`, `name`, `suppressed`) in its root `properties` (`ReservedParamKey`).

Output, with root keys in this order:

- `$schema`: `"https://json-schema.org/draft/2020-12/schema"`.
- `title`: `"Partsmith document"`.
- `description`: `"A Partsmith .ptsm document, format version 3."` (the number is `FORMAT_VERSION`).
- `type`: `"object"`.
- `properties`, in this order: `format` is `{"const": "partsmith"}`; `format_version` is `{"const": 3}` (`FORMAT_VERSION`); `units` is `{"$ref": "#/$defs/Units"}`; `printer` is `{"type": "string"}`; `printer_profile` is `{"$ref": "#/$defs/PrinterProfile"}`; `parameters`, `bodies` and `features` are `{"type": "array", "items": {"$ref": "#/$defs/Parameter"}}` (and `Body`, `FeatureRecord`); `rollback` is `{"$ref": "#/$defs/Id"}`.
- `required`: `["format", "format_version", "units"]`.
- `additionalProperties`: `false`.
- `$defs`: every definition below, with keys sorted by byte order.

Model definitions:

| Definition | Content |
| --- | --- |
| `Id` | `{"type": "string", "pattern": "^[A-Za-z0-9_-]{1,64}$"}` |
| `Expr` | `{"type": "string"}` |
| `Color` | `{"type": "string", "pattern": "^#[0-9A-Fa-f]{6}$"}` |
| `Units` | `{"type": "string", "enum": ["mm", "cm", "m", "in"]}` |
| `BodyRole` | `{"type": "string", "enum": ["part", "negative_volume", "support_blocker", "support_enforcer", "settings_modifier"]}` |
| `Parameter` | object; properties `id` (`$ref` Id), `name` (`{"type": "string", "pattern": "^[A-Za-z_][A-Za-z0-9_]{0,63}$"}`), `expr` (`$ref` Expr), `comment` (string); `required` `["id", "name", "expr"]`; `additionalProperties` `false` |
| `Body` | object; properties `id` (`$ref` Id), `name` (string), `material` (string), `color` (`$ref` Color), `visible` (boolean), `role` (`$ref` BodyRole); `required` `["id", "name"]`; `additionalProperties` `false` |
| `FeatureRecord` | object; properties `id` (`$ref` Id), `type` (`{"type": "string", "pattern": "^[a-z][a-z0-9_]{0,63}$"}`), `name` (string), `suppressed` (boolean); `required` `["id", "type"]`; `allOf` with one entry per feature kind in byte order, each `{"if": {"properties": {"type": {"const": "<kind>"}}, "required": ["type"]}, "then": {"$ref": "#/$defs/feature.<kind>"}}`; `allOf` is omitted when there are no features; no `additionalProperties` (params are open) |

The object definitions `Parameter`, `Body` and `FeatureRecord` have their keys in this order, each only when present: `type`, `properties`, `required`, `additionalProperties`, `allOf`. Property schemas list `$ref` or `type` first, then `pattern`, `enum` or `const`.

Each feature's params schema is added as `$defs["feature.<kind>"]` with its root `$schema` and `$defs` keys removed. Its `$defs` entries are hoisted into the root `$defs` under the same names, so its `#/$defs/...` references keep working. A hoisted definition whose name is already present with an identical value is shared; with a different value (including a clash with a model definition) the result is `ConflictingDefinition { name }`.

Feature records of a type with no params schema (for example a plugin that is not installed) match no `allOf` branch and are validated only against the record keys.

The `printer_profile` schema is added as `$defs["PrinterProfile"]` with its root `$schema`, `$defs` and `title` keys removed. Its `$defs` entries are hoisted into the root `$defs` by the rule of the feature schemas above: an identical entry is shared, and a different one is `ConflictingDefinition { name }`. The engine passes the schema that schemars generates for print.md's `PrinterProfile`, whose JSON Schema is its written form in both contracts (print.md section 2.2). That schema requires `id`, `name`, `technology`, `bed`, `layer_height`, `max_overhang_angle`, `max_bridge_length` and `fit`. It closes the profile, `FitClasses` and `Bed` to unknown members. Its `id` refers to `#/$defs/Id`, which is identical to the model definition. The two share that one entry. A `PrinterProfile` already in the root `$defs` with another value is `ConflictingDefinition { name: "PrinterProfile" }`. No input check of the list above applies to this schema.

`ptsm_schema_text(features, printer_profile)` is `canonical::to_string` of the schema: deterministic, independent of the order of `features`, and ending with one line feed. The CLI's `schema --write` writes it to `docs/format/ptsm.schema.json`, and CI fails when the committed file differs.

### 3.11 Assets

Imported files (in M1, the STEP files of the `import` feature, features/import.md) are stored next to the document in a folder named `assets/`, each named `<64 lowercase hex digits of the BLAKE3 hash of its bytes>.<lowercase extension>`, for example `assets/<64 hex>.step`. Feature params refer to an asset by that file name, never by a path.

- The model never reads or writes assets: `load` and `save` work on the text alone, and a document whose assets are missing loads. The engine supplies asset bytes to recompute and returns the assets a document references when it saves (engine.md section 3.14); hosts read and write the folder.
- A document references exactly the assets named by its features' `Dependencies::assets` (features.md section 2.4). A missing referenced asset is reported on the feature that reads it (`asset_missing`), not as a load error.
- Because names are content hashes, a valid asset never changes in place, the same file imported twice is stored once, and several documents in one folder can share `assets/`.
- Since M2 an asset's extension is its kind: `step`, `ttf` or `otf` ([ADR 0037](../adr/0037-save-checks-the-file-and-hosts-reach-the-assets-folder.md)'s amendment of 2026-11-07 on font assets). The font kinds hold the fonts that wave 3's sketch text reads (m2-plan.md item 27). Nothing in the `.ptsm` text changes: the model reads no asset name and checks no kind (test 70). The hosts' pattern `^[0-9a-f]{64}\.[a-z0-9]+$` (cli.md section 2.3, python-api.md section 3.1) already matches them.
- Since M2 a host learns the asset names a document reads from the core (`missing_assets` of `document_changed`, protocol.md section 2.7) and adds files to an open document with `add_assets` (protocol.md command 38, [ADR 0064](../adr/0064-assets-added-to-an-open-document-and-named-by-the-core.md)). The UI reads no `.ptsm` text to find them (ui.md section 1).

**Writing a document with its assets.** A host that writes the result of `save_document` (engine.md section 3.14) to a file in a folder it can write writes it in this order, so the document is replaced only after every asset `save_document` returned is in `assets/`, and a failed or interrupted save leaves the previous document in place ([ADR 0036](../adr/0036-hosts-write-assets-first-and-check-them-against-the-engine.md)):

1. **Assets.** When the result has at least one asset, the host creates `assets/` next to the document if it does not exist; a result without assets creates no folder. It then writes each asset, in the result's order (sorted by name), to `assets/<name>`, after the entry check: the host looks at the entry under the asset's name without following a symbolic link. A regular file of the asset's size is read, and kept when its bytes equal the asset's. Any other entry is replaced: a regular file of another size or with other bytes, because its BLAKE3 hash does not match its name (a truncated copy, a Git LFS pointer, a file edited by hand) and the engine would refuse it with `asset_mismatch`; a symbolic link (whether or not it resolves) or a pipe, because the host neither reads nor writes through it. The write replaces the entry itself, never the file a link points to. A folder under the asset's name makes the step fail with the error for `assets/<name>`; it is never removed. Comparing the file with the bytes `save_document` returned is the hash check, since those bytes hash to the name, so a host needs no BLAKE3 of its own.
2. **Document.** The host writes the text to the document's file.
3. **`mark_saved`.** The host sends it with the revision `save_document` returned (engine.md section 3.2).

Each file is written with the host's own atomic write (python-api.md section 3.1, desktop.md section 3.4 step 4; on the web, the file system API's writable stream): a reader sees the old file or the whole new one, the file is flushed before the next step starts, and a temporary file's name never matches the asset name pattern. For an asset, the target is the entry under its name itself (step 1). A document file that is a symbolic link is written through. Python and the desktop follow every link of a chain and replace the file the link points to. They leave the link as it is. A link that leads to no file fails the document step. Nothing is created where it points (python-api.md section 3.1 and its test 120, desktop.md section 3.4 steps 3 and 4.1 and its test 72, [ADR 0037](../adr/0037-save-checks-the-file-and-hosts-reach-the-assets-folder.md) option (A)). The assets of a linked document are in `assets/` next to the link, not next to the file it points to (ADR 0037 decision 6). A host whose file interface creates the target before writing it (the browser's `getFileHandle` with `create: true`, which creates an empty file) removes the entry it created (`removeEntry`) when the write fails, so no empty file is left under an asset's name. The desktop's refusal of a read-only target (desktop.md section 3.4 step 4.2) also applies to an entry under an asset's name that holds other bytes (desktop.md section 3.4 step 9, [ADR 0037](../adr/0037-save-checks-the-file-and-hosts-reach-the-assets-folder.md)). Python refuses a read-only document file too, at the document step. On Linux and macOS it replaces a read-only entry under an asset's name that holds other bytes. On Windows the rename fails on that entry. The save fails at step 1 (python-api.md section 3.1, ADR 0037 decision 5 and its second amendment).

The first step that fails stops the save: the host removes that step's temporary file, reports which file or folder failed (the document, `assets/` or the asset), never the temporary file, worded as its host's spec says (python-api.md section 3.1 puts the path in `OSError.filename`; the UI words it as docs/guidelines/errors.md says), writes nothing after it and sends no `mark_saved`, so the document stays unsaved. Assets written before the failure stay, since they are valid. A save never deletes, renames or changes any other entry in `assets/`, because other documents in the folder may use it. A referenced asset the engine does not hold (the document was opened without it, so its import fails with `asset_missing`) is not in the result: it stays missing, and an entry of its name is left as it is.

A host that cannot write the document's folder (the web with only the file handle of `showSaveFilePicker`, or a download) does not run step 1. The web asks the user for that folder before it writes anything (Chromium's save dialog itself has already created the chosen file and, we believe, emptied one that existed, ui.md open issue 37), and a download carries no assets, since a page without the File System Access API reads no folder and so gives the engine none (ui.md section 3.19).

**What binds which host.** The order of steps 1 to 3 and the failure rules bind every host that writes `assets/`. The entry check of step 1 (reading a regular file of the asset's size, and replacing any other entry) binds Python (python-api.md section 3.1). The desktop (desktop.md section 3.4 step 9) and the web (ui.md section 2.15, `writeAssets`) follow it as ADR 0037 decides: option (a) of ADR 0036, not (b). Option (b) writes only an asset whose name is not in `assets/` yet. The owner accepted that choice with the desktop's capability change on 2026-11-04. The web's file system API cannot meet two duties of this section: a handle's `kind` is only `file` or `directory`, so the page cannot tell a symbolic link from what it points to, and the writable stream has no flush; it replaces the file whole when `close()` resolves, which is the web's atomic write. ui.md section 2.15 states what the web does about both: it compares and replaces what the browser shows as a file, and takes the replacement on `close()` as its atomic write. Whether a host follows `assets` when it is itself a symbolic link is that host's rule: Python follows a link to a folder, like any folder of the document's path (python-api.md section 3.1), and the desktop follows none (desktop.md section 3.4 step 8, ADR 0037).

Tests 97 to 102 of python-api.md test this order for Python, test 103 the export errors of the same atomic write, and tests 118 to 122 its write through a link. ui.md tests U-226, U-227, U-230, U-231 and E-151 and desktop.md tests 57 to 61 test it for the web and desktop hosts. desktop.md tests 51 and 72 test the desktop's write through a link.

### 3.12 Scope

A scope is one set of `parameters`, `bodies` and `features`, with the records nested in them, one origin and one timeline ([ADR 0069](../adr/0069-component-scopes-by-containment-and-occurrence-paths.md)). A document's top-level arrays are its root scope. Format versions 1 to 3 hold the root scope alone. M4's components add scopes, with a format version of their own. Every key, record and value of versions 1 to 3 keeps these rules, and the critic of each M2 spec checks them:

1. A record belongs to the scope whose array holds it. No record names its scope. A new record goes into `parameters`, `bodies` or `features`, or nests inside a feature record.
2. Every reference that versions 1 to 3 hold is unqualified. It resolves in the scope of the record that holds it. It names its target by id, `PointRef`, region key, element reference, parameter name, asset name or built-in name, never by its position in a list or by its display name. It holds no document, file, scope or occurrence.
3. A face, edge or vertex reference sits beside a `body` field in the same object (spec README decision D2). That object is where M4 puts the occurrence path of a reference that reads a body of another scope. The `body` a feature writes, and the references to that body's elements beside it, never take one ([ADR 0069](../adr/0069-component-scopes-by-containment-and-occurrence-paths.md) decision 4).
4. Ids are unique within a scope. Versions 1 to 3 hold one scope. For them model.md section 3.1's rule is unchanged.
5. Every stored coordinate, direction, placement and fingerprint is in sketch coordinates, in body coordinates, in a frame that an element the record references defines (such as a referenced face's parameter plane), or in world coordinates. World coordinates are the coordinates of the document's origin. They include a face sketch's `fallback` with `fallback_frame` `world`, a primitive's `plane` and `position`, the origin planes and axes, `move_body`'s `translation`, and a fingerprint's centroid and direction. A face sketch's `fallback` with `fallback_frame` `body` and the `direction` of an edge or face axis are in body coordinates (sketch.md section 2.2, features.md section 2.5). No record holds a coordinate in the frame of another document, file or occurrence.
6. `format`, `format_version`, `units`, `printer`, `printer_profile` and `rollback` belong to the file, not to a scope. A new document-level key holds a property of the file, never records that other records reference. No record stores a file path, a document name, a URL or a session handle. A file reaches a document only as an asset.
7. Reserved names. No key of a document is named `occurrence`, at any depth, and no document root holds `components`, `occurrences` or `joints` (tests 71 and 72). M4 adds them with a format version of its own. The step that migrates a document into that version changes only the version number.

## 4. Errors

`{m}` stands for a `StructureError` message (model.md section 4.5).

### 4.1 `LoadError`

| Variant | Message |
| --- | --- |
| `NotUtf8 { offset: usize }` | `The file is not valid UTF-8 text (the first bad byte is at offset {offset}). Save it as UTF-8.` |
| `Syntax { line: usize, column: usize, message: String }` | `The file is not valid JSON at line {line}, column {column}: {message}.` (`message` is the JSON parser's description) |
| `DuplicateKey { key: String, line: usize, column: usize }` | `The key "{key}" appears twice in the same object (line {line}). Remove one of them.` |
| `NotPartsmith` | `This is not a Partsmith document: the top level must be an object whose "format" is "partsmith".` |
| `MissingFormatVersion` | `The document has no "format_version". Add "format_version": 3 after "format".` (the number is `FORMAT_VERSION`) |
| `InvalidFormatVersion { found: String }` | `"format_version" must be a whole number such as 3, not {found}.` (the number is `FORMAT_VERSION`) |
| `NewerFormatVersion { found: u64, supported: u32 }` | `This file uses format version {found}, which is newer than this version of Partsmith supports (up to {supported}). Update Partsmith to open it.` |
| `UnsupportedFormatVersion { found: u64, oldest: u32 }` | `Format version {found} is too old to open. The oldest version this Partsmith can open is {oldest}.` |
| `MissingMigration { from: u32 }` | `There is no upgrade from format version {from} to {from + 1}. This is a bug in Partsmith; please report it.` |
| `Migration { from: u32, to: u32, message: String }` | `Could not upgrade the file from format version {from} to {to}: {message}` |
| `Invalid { path: String, message: String }` | `{path}: {message}`, or `{message}` when `path` is empty |
| `Structure(Vec<StructureError>)` with one entry | `This document has a problem: {m}` |
| `Structure(Vec<StructureError>)` with n entries | `This document has {n} problems:` then, for each, a line feed, `- ` and `{m}` |

For `Invalid`, `message` comes from deserialization: for ids, colors and units it is the `IdError`, `ColorError` or `UnitsError` message (model.md section 4); for other type errors it is the deserializer's description, which names the expected type or the missing or unknown field.

### 4.2 `SaveError`

| Variant | Message |
| --- | --- |
| `Structure(Vec<StructureError>)` with one entry | `The document cannot be saved: {m}` |
| `Structure(Vec<StructureError>)` with n entries | `The document cannot be saved because of {n} problems:` then, for each, a line feed, `- ` and `{m}` |

### 4.3 `CanonicalError`

| Variant | Message |
| --- | --- |
| `NonFiniteNumber { path: String }` | `{path}: NaN and infinity cannot be saved. Use a finite number.` (path format as in section 3.7; `value` when the root itself is not finite) |
| `Serialize { message: String }` | `The value cannot be written as JSON: {message}` |

### 4.4 `SchemaError`

| Variant | Message |
| --- | --- |
| `InvalidKind { kind: String }` | `'{kind}' is not a valid feature type name.` |
| `DuplicateKind { kind: String }` | `Two params schemas are registered for the feature type '{kind}'.` |
| `NotAnObject { kind: String }` | `The params schema of '{kind}' must describe an object ("type": "object").` |
| `ClosedParams { kind: String }` | `The params schema of '{kind}' forbids extra keys, but feature records keep unknown keys. Do not set additionalProperties or unevaluatedProperties to false.` |
| `ReservedParamKey { kind: String, key: String }` | `The params schema of '{kind}' declares '{key}', which the feature record itself uses.` |
| `ConflictingDefinition { name: String }` | `Two schemas define '{name}' differently. Give one of the types a different name.` |

## 5. Performance budgets

Release build on the reference desktop (2022 mid-range laptop); web budgets are 3 times these. Checked by CI benchmarks on a generated 1 MB document (2,000 features, each a sketch-like record with 20 one-line entities) and on the E2 example.

| Operation | Desktop budget |
| --- | --- |
| `load` of the 1 MB document | 30 ms |
| `save` of the 1 MB document | 30 ms |
| `load` then `save` of E2 | 0.2 ms |
| `ptsm_schema_text` with 30 feature schemas | 20 ms |

The layout algorithm must be linear in the output size times the nesting depth; computing one-line forms again at every level is acceptable only within that bound.

## 6. Acceptance tests

E1 to E11 are the examples in section 3.6. "The E2 text" means the exact text shown plus a final line feed. `load(t)` stands for `load(t).unwrap().document` where a test expects success.

### Writer and layout

1. `save(&Document::new(Units::Mm))` is exactly the E1 text (122 bytes), and `Document::new(Units::Mm)` equals `load(E1)`.
2. `load(E2)` succeeds with `migrated_from == None`, 2 parameters, 1 body and 3 features; `save` of it is exactly the E2 text (1614 bytes).
3. E3, E4, E5, E6 and E7 each load and save back to exactly themselves (630, 399, 463, 337 and 626 bytes).
4. A document built in code equal to E3 (bodies `b-1` named `Base`; `b-2` named `Keep out` with color `#ff0000`, `visible` false and role `SupportBlocker`; features as shown with params inserted in the shown order; rollback `f-cyl1`) saves to exactly the E3 text.
5. Loading the E8 input (with CRLF line endings) and saving gives exactly the E8 output (282 bytes); loading that output and saving again gives the same bytes.
6. The E2 output has no `\r`, no `\t`, no byte order mark, no line ending in a space, ends with exactly one `\n`, and no line is longer than 121 characters.
7. Every row of the table in section 3.3: `format_number(&Number::from_f64(value).unwrap())` (or the integer `Number` for integer rows) gives the written form.
8. `format_number` of the `u64` number `9007199254740993` is `9007199254740992.0`; of the `i64` number `-5` is `-5`.
9. `format_string` of the string in section 3.4 gives exactly `"Lid \"v2\" \\ draft\tAé°/x\u001f"`; `format_string("\u{7f}")` is a quote, the raw U+007F, and a quote; `format_string("\u{8}\u{c}\n\r")` is `"\b\f\n\r"`.
10. Every row of the E9 table: `canonical::to_string(&json)` gives exactly the output.
11. Saving the same document twice, and saving a clone of it, give identical strings.
12. Defaults are omitted: a body with `visible == true` and role `Part`, a feature with `suppressed == false` and `name == None`, a parameter with `comment == None`, and a document with `printer == None` and `rollback == None` produce none of the keys `visible`, `role`, `suppressed`, `name` (on the feature), `comment`, `printer`, `rollback`.
13. Changing parameter `p-wall`'s expr in E2 from `2 mm` to `3 mm` and saving changes exactly one line of the text.

### Loading

14. A document whose keys are in a different order at every level (root, parameters, bodies, record keys of features) loads and saves with keys in declaration order; params keep their file order.
15. A document with no `parameters`, `bodies` or `features` keys loads with empty lists, and saves them as `[]`.
16. `load("\u{FEFF}" + E1)` succeeds; `load_bytes` of `EF BB BF` followed by E1's bytes succeeds; `save` output never starts with a byte order mark.
17. `load_bytes(&[0x7B, 0xFF, 0x7D])` is `Err(NotUtf8 { offset: 1 })`.
18. Each of these is `Err(Syntax { .. })`: the empty string; E1 with a trailing comma after `"features": []` (the error's `line` is 8, where the parser meets the `}`); E1 with a `// comment` line; a param value `NaN`; a param value `1e400`; a string in single quotes.
19. E1 with a second `"units": "cm"` line added is `Err(DuplicateKey { key: "units", .. })`; a feature `{"id": "f-1", "type": "box", "x": 1, "x": 2}` is `Err(DuplicateKey { key: "x", .. })`.
20. A feature param holding 100 nested arrays loads; one holding 200 nested arrays is `Err(Syntax { .. })` and does not overflow the stack.
21. `load("[]")`, `load("{}")`, and E1 with `"format": "other"` or `"format": 1` are each `Err(NotPartsmith)`.
22. E1 without `format_version` is `Err(MissingFormatVersion)`; with `"format_version": "1"` it is `Err(InvalidFormatVersion { found: "\"1\"" })`; with `1.5`, `InvalidFormatVersion { found: "1.5" }`; with `-1`, `InvalidFormatVersion { found: "-1" }`; with `1.0` it loads with `migrated_from == Some(1)`.
23. E1 with `"format_version": 4` is `Err(NewerFormatVersion { found: 4, supported: 3 })` with message `This file uses format version 4, which is newer than this version of Partsmith supports (up to 3). Update Partsmith to open it.`; with `0` it is `Err(UnsupportedFormatVersion { found: 0, oldest: 1 })`. (Before format version 3, the same test used `3` and `supported: 2`.)
24. Each of these is `Err(Invalid { path, .. })` with the given path: root key `"colour": 1` added (`colour`); `"units": "mmm"` (`units`, message contains `'mmm' is not a document unit`); body `{"id": "b-1", "name": "A", "colour": "#000000"}` (`bodies[0].colour`); parameter id `"p 1"` (`parameters[0].id`, message contains `The id 'p 1' contains ' '`); body color `"#12345"` (`bodies[0].color`); parameter `"expr": 2` (`parameters[0].expr`); body `"visible": null` (`bodies[0].visible`); a feature without `type` as the third feature (`features[2]`, message contains `type`).
25. E1 without `units` is `Err(Invalid { path: "", .. })` whose message contains `units`.
26. E1 with `"bodies": [{"id": "b-1", "name": "A"}, {"id": "b-1", "name": "B"}]` is `Err(Structure([DuplicateId { id: b-1 }]))` with message `This document has a problem: The id b-1 is used by more than one record. Give the later record a new id.`
27. E1 with parameters `{"id": "p-1", "name": "pi", "expr": "1"}` and `{"id": "p-2", "name": "x", "expr": "1"}` plus `"rollback": "f-9"` is `Err(Structure(..))` with two entries, and its message is `This document has 2 problems:\n- Parameter p-1: 'pi' is a reserved word. Pick another name, such as pi_1.\n- The rollback marker points at f-9, which is not a feature in this document. Move the marker to an existing feature or to the end.`
28. After loading a feature with params `{"a": 80.0, "b": 1e2, "c": -0.0, "d": 0.5, "e": 9007199254740993}`: `a` equals `json!(80)` and `is_u64()`; `b` equals `json!(100)`; `c` equals `json!(0)` and is an integer; `d` is the `f64` 0.5; `e` `is_f64()` with value `9007199254740992.0`.
29. A feature with unknown params keys `{"zeta": 1, "alpha": 2, "mid": 3}` loads and saves with keys in the order `zeta`, `alpha`, `mid`.
30. A parameter with expr `" 2 mm "` (spaces kept) round-trips through `save` and `load` unchanged.
31. A file with a feature of an unregistered type `gear_demo` loads (the model does not consult the registry).

### Saving and canonical helpers

32. `save` of a document with two parameters named `wall` (built in code) is `Err(SaveError::Structure([DuplicateParameterName { .. }]))`; with a feature whose params contain the key `id` it is `Err(SaveError::Structure([ReservedParamKey { .. }]))`.
33. `canonical::to_value` of a struct `{ x: f64::NAN }` is `Err(NonFiniteNumber { path: "x" })`; of a struct `{ values: vec![1.0, f64::INFINITY] }` is `Err(NonFiniteNumber { path: "values[1]" })`; of `f64::NAN` alone is `Err(NonFiniteNumber { path: "value" })`.
34. `canonical::to_value` of a struct `{ depth: 80.0, ratio: 0.5 }` is `{"depth": 80, "ratio": 0.5}` with `depth` an integer `Number`.
35. `canonical::normalize_numbers` on `json!([80.0, -0.0, 1.5, [2.0]])` gives a value equal to `json!([80, 0, 1.5, [2]])` whose integers are integer `Number`s.
36. `canonical::parse("{\"a\": 1, \"a\": 2}")` is `Err(DuplicateKey { key: "a", .. })`; `canonical::parse("[1.0]")` equals `json!([1])`.
37. Property (proptest, 512 cases): for random structurally valid documents (random parameters, bodies and features with params made of random JSON values up to depth 4, random strings including control and non-ASCII characters, random finite numbers): `load(save(doc))` equals `doc` after `doc.normalize()`, and `save(load(save(doc)))` equals `save(doc)`.
38. Property (proptest, 512 cases): for random JSON values, `canonical::to_string` output parses back (with `canonical::parse`) to a value equal to the normalized input, and every output line that holds a one-line container is at most 120 characters wide not counting a final comma.

### Migration

39. `migrate(serde_json::from_str(E1))` returns the value unchanged and `None`; with E1's `format_version` set to 1, `migrate` returns E1's value, at version 3, and `Some(1)`; with it set to 2, E1's value at version 3 and `Some(2)`.
40. With steps `S1 = { from: 1, migrate: adds root key "a": 1 }` and `S2 = { from: 2, migrate: renames root key "a" to "b" }`, `migrate_with(doc at version 1, 3, 1, &[S2, S1])` returns a document with `"b": 1`, no `"a"`, `"format_version": 3`, and `Some(1)`.
41. The same with the document at version 2 applies only `S2` (the result has no `"b"` because there was no `"a"`) and returns `Some(2)`.
42. `migrate_with(doc at version 1, 3, 1, &[S1])` is `Err(MissingMigration { from: 2 })`.
43. A step `{ from: 1, migrate: returns Err("Unknown units 'yd'.") }` makes `migrate_with(doc at 1, 2, 1, ..)` return `Err(Migration { from: 1, to: 2, message: "Unknown units 'yd'." })` with message `Could not upgrade the file from format version 1 to 2: Unknown units 'yd'.`
44. A step that sets `format_version` to 99 still yields `"format_version": 2` after `migrate_with(doc at 1, 2, 1, ..)`.
45. `migrate_with(doc at 0, 3, 1, ..)` is `Err(UnsupportedFormatVersion { found: 0, oldest: 1 })`; `migrate_with(doc at 4, 3, 1, ..)` is `Err(NewerFormatVersion { found: 4, supported: 3 })`.

### JSON Schema

In tests 46 to 53, `P` is the stand-in printer profile schema `{"type": "object"}`, and `ptsm_schema(x)` and `ptsm_schema_text(x)` stand for `ptsm_schema(x, &P)` and `ptsm_schema_text(x, &P)`.

46. `ptsm_schema(&[])` has, at these JSON pointers: `/$schema` = `"https://json-schema.org/draft/2020-12/schema"`; `/title` = `"Partsmith document"`; `/type` = `"object"`; `/properties/format/const` = `"partsmith"`; `/properties/format_version/const` = `3`; `/required` = `["format", "format_version", "units"]`; `/additionalProperties` = `false`; `/properties/rollback/$ref` = `"#/$defs/Id"`; `/properties/printer_profile/$ref` = `"#/$defs/PrinterProfile"`.
47. In `ptsm_schema(&[])`: `/$defs/Id/pattern` = `"^[A-Za-z0-9_-]{1,64}$"`; `/$defs/Color/pattern` = `"^#[0-9A-Fa-f]{6}$"`; `/$defs/Units/enum` = `["mm", "cm", "m", "in"]`; `/$defs/BodyRole/enum` has the five roles in declaration order; `/$defs/Parameter/required` = `["id", "name", "expr"]`; `/$defs/Body/required` = `["id", "name"]`; `/$defs/FeatureRecord/required` = `["id", "type"]`; `/$defs/FeatureRecord/allOf` is absent.
48. The keys of `/$defs` in `ptsm_schema(&[])` are exactly `["Body", "BodyRole", "Color", "Expr", "FeatureRecord", "Id", "Parameter", "PrinterProfile", "Units"]` in that order, `/$defs/PrinterProfile` is `{"type": "object"}` (`P`), and the root keys are in the order of section 3.10.
49. With params schemas for kinds `extrude` and `box` (given in that order, each `{"type": "object", "properties": {"depth": {"type": "string"}}}`): `/$defs/FeatureRecord/allOf/0/if/properties/type/const` = `"box"`, `/$defs/FeatureRecord/allOf/0/then/$ref` = `"#/$defs/feature.box"`, `/allOf/1` is for `extrude`, and `/$defs/feature.box/properties/depth/type` = `"string"`.
50. A params schema for `extrude` with `"$schema"`, a `"$defs": {"Extent": {"type": "object"}}` and a property `{"$ref": "#/$defs/Extent"}` yields root `/$defs/Extent` = `{"type": "object"}`, and `/$defs/feature.extrude` has neither `$schema` nor `$defs`.
51. Two params schemas whose `$defs` both contain an identical `Extent` give one shared `Extent`; if the two differ, the result is `Err(ConflictingDefinition { name: "Extent" })`; a params schema defining `Id` as `{"type": "integer"}` gives `Err(ConflictingDefinition { name: "Id" })`.
52. Errors: kinds `box` twice is `Err(DuplicateKind { kind: "box" })`; kind `Box` is `Err(InvalidKind { kind: "Box" })`; a schema `{"type": "array"}` is `Err(NotAnObject { .. })`; `{"type": "object", "additionalProperties": false}` is `Err(ClosedParams { .. })`; `{"type": "object", "properties": {"name": {"type": "string"}}}` is `Err(ReservedParamKey { kind, key: "name" })`.
53. `ptsm_schema_text` of the same inputs twice, and of the inputs in reversed order, gives identical text; the text ends with exactly one line feed and `canonical::to_string(&canonical::parse(&text)?)` equals it.

### Format version 2

54. A version 1 document changes only its version: E2 with its line 3 changed to `  "format_version": 1,` loads with `migrated_from == Some(1)` and a document equal to `load(E2)`, and `save` of it is exactly the E2 text, so input and output differ only in that line. The same holds for E10 at version 1: the step from version 1 never reads feature params.
55. E10 loads with `migrated_from == None`, and `save` of it is exactly the E10 text (3081 bytes): `nb`, `merged`, `outline`, `fallback_frame` and `direction` round-trip byte for byte, in their key order.
56. A build that reads only version 1 stops cleanly: `migrate_with(serde_json::from_str(E10 at version 2), 1, 1, &[])`, with E10's line 3 changed to `  "format_version": 2,`, is `Err(NewerFormatVersion { found: 2, supported: 1 })` with the message `This file uses format version 2, which is newer than this version of Partsmith supports (up to 1). Update Partsmith to open it.`
57. A version 1 document rebuilds as before (in `crates/partsmith-recompute/tests/format_version.rs`; it passes from phase 2 of the plan in the reference stability study, section 10.4): the timeline of reference stability corpus scenario R32 variant (a) after its edit (a box 40 x 40 x 10, a Move body of (100, 0, 0), then a face sketch on the box top with its point at (15, 15) and a hole of diameter 4 through all), written at format version 1 with the face sketch's fallback as the corpus gives it and no `fallback_frame`, loads with `migrated_from == Some(1)` and rebuilds as before version 2 existed: the face sketch's placement has origin (0, 0, 10), x axis (1, 0, 0) and normal (0, 0, 1) (the fallback means world coordinates); the hole misses the moved box, so the hole feature has status warning with a `hole_misses_body` message and the body's volume stays 16000; saving the document without edits changes only its `format_version` line. The same timeline at version 2 with `"fallback_frame": "body"` drills the hole at world (115, 15) with volume 15874.336294, as corpus R32a expects. The one exception: the timeline of corpus R38 after its edit (the left wall leaning to (-1, 20)), written at format version 1 with the pattern's axis reference `{q: edge(f-ext1/lateral/e-4|f-ext1/start-cap), fp: line|n=0,1,0|c=0,10,0|l=20|adj=2,2}` and no `direction`, keeps the copy on +Y: direction (-0.049938, 0.998752, 0), bounding box [-2.498129,0,0]-[40,49.962570,10], where the version 1 rule turned it to (0.049938, -0.998752, 0) and put the copy on -Y.
58. The schema describes the version 2 fields: in `docs/format/ptsm.schema.json` as `partsmith schema --write` generates it with the built-in features (checked in `crates/partsmith-cli/tests/`), `/properties/format_version/const` is `3` (it was `2` before format version 3); `/$defs/Reference/properties` has `nb` and `merged` and `/$defs/Reference/required` is `["q", "fp"]`; `/$defs/RegionRef/properties` has `outline` and `/$defs/RegionRef/required` is `["key", "fp"]`; the `face` branch of `/$defs/SketchPlane/oneOf` has the property `fallback_frame` with the enum `["world", "body"]`, and the `edge` and `face` branches of `/$defs/AxisRef/oneOf` have the property `direction`, an array of 3 numbers, none of them listed as required.

### Format version 3

59. A version 2 document changes only its version: E2 with its line 3 changed to `  "format_version": 2,` loads with `migrated_from == Some(2)` and a document equal to `load(E2)`, and `save` of it is exactly the E2 text. The same holds for E10 at version 2 (3081 bytes).
60. E11 loads with `migrated_from == None`. Its document has `printer == Some("my-mk4")` and a `printer_profile` whose members are, in this order, `id`, `name`, `technology`, `bed`, `nozzle_diameter`, `layer_height`, `max_overhang_angle`, `max_bridge_length` and `fit`. `save` of it is exactly the E11 text (588 bytes).
61. The model keeps a stored profile as it is: E11 with the members of `printer_profile` in reversed order loads, and `save` writes them in that reversed order. The engine writes the written form's order (engine.md section 3.2).
62. A stored profile without a printer: E11 without its `printer` line is `Err(Structure([PrinterProfileWithoutPrinter]))` with the message `This document has a problem: The document stores a printer profile but names no printer. Set "printer" to the profile's id, or remove "printer_profile".`
63. A stored profile with another id: E11 with `"printer": "my-mk3"` is `Err(Structure([PrinterProfileIdMismatch { printer: "my-mk3", found: Some("my-mk4") }]))` with the message `This document has a problem: The stored printer profile has the id "my-mk4", but the document names the printer "my-mk3". Make them the same.`
64. A stored profile without an id: E11 with the profile's `id` member removed is `Err(Structure([PrinterProfileIdMismatch { printer: "my-mk4", found: None }]))` with the message `This document has a problem: The stored printer profile needs the id "my-mk4", the printer the document names. Set its "id" to "my-mk4".` The same holds with `"id": 7` in place of the string.
65. A stored profile that is not an object: E11 with `"printer_profile": []` is `Err(Invalid { path: "printer_profile", .. })`, and E11 with `"printer_profile": null` loads with `printer_profile == None`.
66. A build that reads up to version 2 stops cleanly on version 3: `migrate_with(serde_json::from_str(E11), 2, 1, &[S])`, with `S` a step from 1 that returns its input, is `Err(NewerFormatVersion { found: 3, supported: 2 })` with the message `This file uses format version 3, which is newer than this version of Partsmith supports (up to 2). Update Partsmith to open it.`
67. The built-in chain: `MIGRATIONS` has exactly two steps, with `from` 1 and 2. `migrate` of E1 at version 2 with the extra root member `"x": 1` returns a value at version 3 that still has `"x": 1`, and `Some(2)`: the step from 2 keeps every member it is given (`load` of that text then fails with `Invalid { path: "x", .. }`).
68. The printer profile schema is hoisted: with `Q` = `{"type": "object", "title": "PrinterProfile", "properties": {"bed": {"$ref": "#/$defs/Bed"}}, "$defs": {"Bed": {"type": "object"}}}`, `ptsm_schema(&[], &Q)` has `/$defs/Bed` = `{"type": "object"}` and `/$defs/PrinterProfile` = `{"type": "object", "properties": {"bed": {"$ref": "#/$defs/Bed"}}}`, with neither `title` nor `$defs`. With a params schema for `box` whose `$defs` hold `"Bed": {"type": "string"}`, `ptsm_schema(&[box], &Q)` is `Err(ConflictingDefinition { name: "Bed" })`.
69. The generated schema describes the stored profile: in `docs/format/ptsm.schema.json` as `partsmith schema --write` generates it (checked in `crates/partsmith-cli/tests/`), `/properties/printer_profile` is `{"$ref": "#/$defs/PrinterProfile"}`; `/required` is still `["format", "format_version", "units"]`; `/$defs/PrinterProfile/required` is `["id", "name", "technology", "bed", "layer_height", "max_overhang_angle", "max_bridge_length", "fit"]` and `/$defs/PrinterProfile/additionalProperties` is `false`; `/$defs/PrinterProfile/properties/id/$ref` is `"#/$defs/Id"`; `/$defs/FitClasses/required` is `["press", "snug", "slip", "loose"]`; and `/$defs` holds `Bed` and `Technology`.

**Asset kinds** (since M2, [ADR 0037](../adr/0037-save-checks-the-file-and-hosts-reach-the-assets-folder.md)'s amendment of 2026-11-07 on font assets)

70. The model takes any asset kind (3.11). E1 with one more feature, `{"id": "f-txt1", "type": "text", "font": "<64 a digits>.ttf"}`, loads. The loaded record's `font` is that string, and `canonical::to_string` of the document writes it unchanged. The same holds with `.otf`.

### Scope and indexes

71. The generated schema leaves `occurrence` free (section 3.12 rule 7): in `docs/format/ptsm.schema.json` as `partsmith schema --write` generates it with the built-in features (checked in `crates/partsmith-cli/tests/`), no `properties` object at any depth has the key `occurrence`.
72. The generated schema leaves the component keys free (section 3.12 rule 7): in the same file, `/properties` has none of the keys `components`, `occurrences` and `joints`.
73. No params field indexes elements, glyphs or characters (m2-plan.md gate 4): in the same file, no `properties` object at any depth has the key `index` or `indices`, a key that ends with `_index` or `_indices`, or the key `glyph`, `glyphs`, `character` or `characters`. Every schema of type `integer` in it is at a property this test lists: none at a3f4573, and from M2 item 14 on the `degree` of the `bspline` and `fixed_spline` entity records (sketch.md section 2.12). A spec that adds another integer property adds it to this list in the same change and says there why it indexes nothing. Every M2 spec that adds a record to version 3 keeps this test passing.

## 7. Open issues

1. **The root is always expanded.** The architecture's layout rule does not exempt the root, so a nearly empty document could print on one line. This spec always expands a non-empty root so every file starts the same way (`{`, then `"format"` and `"format_version"` on their own lines) and tools can sniff the version by line.
2. **What the 120-character width counts.** Interpreted as Unicode scalar values of indentation, key prefix and one-line form, excluding the separator comma, so a member's layout never depends on whether it is last. A one-line member can therefore end in column 121.
3. **"Contains no array of objects"** is read as "at any depth, including the value itself", and an array counts as an array of objects when at least one element is an object.
4. **Empty top-level arrays are written.** The architecture omits fields equal to their default and names `suppressed`, `visible` and `None`; this spec writes `parameters`, `bodies` and `features` even when empty so every file has the same skeleton.
5. **Integral values at or above 2^53** print as ryu writes them (`9007199254740992.0`, `1e16`), following the number rule literally. Integer literals above 2^53 in input change on load because the number model is double precision.
6. **Lenient reading.** The loader accepts a BOM, CRLF, `null` for optional fields, uppercase colors, non-canonical numbers and escapes, and any key order, and normalizes them on save. It still rejects unknown keys in `Document`, `Parameter` and `Body`, and duplicate keys anywhere.
7. **Structural errors stop loading** (model.md open issue 1), so `partsmith validate` reports them from `LoadError::Structure` and reference problems from `Document::validate`.
8. **Field-level error paths.** The paths in `Invalid` need per-field deserialization. The crate `serde_path_to_error` would provide them but is a new dependency needing human review; a hand-written traversal over the three record types also works.
9. **Schema details.** Feature params schemas must stay open (no `additionalProperties: false`) because unknown keys are preserved; definitions are named `feature.<kind>`; `$id` is omitted until the project domain is decided; the schema describes canonical files, not everything the loader accepts.
10. **Where documents are validated against the schema.** Nothing in M1 validates files against the generated schema at runtime (the typed loader is the authority). The schema serves editors, documentation and drift checks.
11. **The printer profile's schema is an argument.** partsmith-print depends on partsmith-model. partsmith-model therefore cannot name print.md's `PrinterProfile`. `ptsm_schema` takes the profile's schema as its second argument, and the engine passes the one schemars generates (engine.md section 2, `ptsm_schema()`). The alternative, a hand-written copy of the written form in this crate, would need a test to keep it in step with print.md.
