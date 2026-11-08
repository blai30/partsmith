# Model corpus and naming metric

Scope: the model corpus under `corpus/`, the scripted edits that the examples and the corpus carry, the oracle the test author writes with each edit, the naming metric computed from them, the stored baselines that track drift, and the checks of the examples runner that run all of it in CI. Read with [technical design](../design/technical-design.md) sections 1 (fewer than 1 percent of upstream edits break a downstream reference), 16 (model corpus, topological naming tests, kernel upgrades) and 17 (test oracles, the held-out corpus), [naming.md](naming.md) (the names an oracle writes and the resolver the metric measures), [delivery.md](delivery.md) section 2.8 (the examples corpus and its runner, whose comparison rules, bless and parallel runs this spec reuses), [engine.md](engine.md) sections 3.5, 3.9 and 3.24 (focus, element listing, reference resolution and stored references), [features.md](features.md) section 2 (`Registry::dependencies`), the [M2 plan](../design/m2-plan.md) item 9 and gates 4 and 6, and ADRs [0028](../adr/0028-single-license-apache-2.md) (one license for the test corpus), [0051](../adr/0051-corpus-coverage-checks.md) (coverage read from the engine) and [0068](../adr/0068-model-corpus-and-naming-metric-scored-against-a-written-oracle.md) (this spec's choices).

This spec is the definition of done for the corpus and the metric. Its acceptance tests are in section 6. Implementers do not weaken them. The test author writes every oracle. An implementer never edits one (AGENTS.md Process).

## 1. Purpose and scope

Technical design section 1 sets a success criterion for v1: fewer than 1 percent of upstream edits in the regression corpus break a downstream reference. The naming metric measures it. A scripted edit changes a model upstream: a dimension, a feature added before others, a feature moved, removed or suppressed. Every stored reference that the edit can reach is then resolved again. The metric is the share of those downstream references that resolve to the element the test author wrote for that edit, or that are lost where the author wrote "lost". Its v1 target is above 99 percent (technical design section 16). The oracle is written by hand. The stored outcomes only track drift between runs. They never decide what is correct.

### 1.1 In M2

- Wave 1 (m2-plan.md item 9): the corpus layout and its file formats (section 2), the nine corpus checks of the examples runner (2.7), the scripted edits of every example that stores a reference, the 46 models of 2.6's table that use only M1's feature types, each with its scripted edits and oracle, and the stored baselines. CI runs the checks on every pull request and every push to `main` (3.7).
- Waves 2 to 4: each track adds its row's models of 2.6's table as its features land, with their scripted edits and oracles. A feature type registered without a corpus model fails corpus check 5. A type whose params can hold a reference fails corpus check 9 until a corpus script measures one.
- Wave 5 (gates 4 and 6): the corpus holds the 100 models of 2.6. The four success parts of gate 1, examples under `examples/`, carry scripted edits as a corpus model does (2.4). On the gate commit the metric over the examples and the corpus is above 99 percent (test 44).

### 1.2 Later

- The 500 models of the launch corpus and the held-out set that implementers never see (technical design sections 16 and 17). M3 and M4 grow the corpus (m2-plan.md item 9 and section 5.2). The held-out set needs storage outside the repository. A later revision of this spec specifies it.
- Differential twins: each corpus model also as a build123d script that CI builds and compares (technical design section 17). It needs a build123d environment. It comes with M4's release hardening (m2-plan.md section 5.2).
- Region references of extrude and revolve profiles in the metric (open issue 1).
- Scripted edits of the units, the printer and the bodies beyond `add_body` (2.4).

## 2. Public interface

### 2.1 Files

```text
corpus/<category>/<stem>.ptsm            a corpus model (2.2)
corpus/<category>/<stem>.expected.json   its rebuild baseline (2.3)
corpus/<category>/<stem>.edits.json      its scripted edits and their oracle (2.4)
corpus/<category>/<stem>.outcomes.json   the stored outcomes of those edits (2.5)
corpus/<category>/assets/<name>          the files its import features read (2.2)
examples/<path>/<stem>.edits.json        the scripted edits of an example that stores a reference (2.4)
examples/<path>/<stem>.outcomes.json     their stored outcomes (2.5)
crates/partsmith-cli/tests/examples/model_corpus.rs   corpus checks 1 to 6 (2.7)
crates/partsmith-cli/tests/examples/naming_metric.rs  corpus checks 7 to 9 and the metric (2.7, 3.3)
```

`<category>` is a category of 2.6's table. `<stem>` matches `[a-z0-9]+(-[a-z0-9]+)*`, as an example's stem does (delivery.md 2.8). `<name>` is an asset name: the BLAKE3 hash of the file's bytes, then its extension (cli.md 2.3, engine.md 3.14). No other file exists under `corpus/`. Next to an example, the two companions above follow delivery.md 2.8's rule, which names them.

Every file under `corpus/` is written for the project and is under the repository's license, Apache-2.0, as the examples are (ADR 0028; the M2 plan's choice 24, option (a)). No model is copied or traced from another project or from a person outside the project. A STEP asset is a file that `partsmith export` wrote from a model of this repository.

### 2.2 Models

A corpus model is a `.ptsm` document in canonical form at the current format version (`partsmith fmt --check` exits 0). It is a whole part, not a one-feature example:

- it holds at least 5 feature records;
- its features hold at least 2 stored references (3.1);
- it has no `rollback` member: every feature of it is built;
- its expected results hold no feature whose status is `error` (2.3).

An asset follows delivery.md 2.8's rule for an example's assets: it is committed once, under the hash of its bytes, and never rewritten. Other bytes for the same part are a new asset. Every file in `corpus/<category>/assets/` is the `asset` of an `import` feature record of a model in `corpus/<category>/` (corpus check 6).

### 2.3 Expected results

`<stem>.expected.json` is the rebuild baseline of technical design section 16: volume, surface area, bounding box and face count, with each feature's status. It is the report of `partsmith rebuild --json corpus/<category>/<stem>.ptsm` (cli.md 3.3), run with the repository root as working directory and ending with exit status 0 or 1. The report is reduced to these members, in this order:

- `features`: for each entry of the report's `features`, in its order, an object with exactly `id`, `type` and `status`, copied.
- `bodies`: for each entry of the report's `bodies`, in its order, an object with `id`, then `"empty": true` when the entry has `empty`, otherwise each of `volume`, `area`, `bounding_box` and `faces` that the entry has, copied in that order.

For example:

```json
{
  "features": [
    {
      "id": "f-box1",
      "type": "box",
      "status": "ok"
    }
  ],
  "bodies": [
    {
      "id": "b-1",
      "volume": 24000.0,
      "area": 5200.0,
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
      "faces": 6
    },
    {
      "id": "b-2",
      "empty": true
    }
  ]
}
```

The file is pretty JSON with two-space indentation and one trailing newline, as `serde_json::to_string_pretty` writes it. A rebuild is compared with the file by delivery.md 2.8's comparison rules: integers and strings exactly, other numbers within `max(1e-6 * |expected|, 1e-6)`. The report's messages, parameters, centers of mass, edge and vertex counts and print checks are not stored. The examples hold those (delivery.md 2.8).

### 2.4 Scripted edits

`<stem>.edits.json` holds a model's scripted edits and the oracle the test author writes for each. It is written by hand and never by a run. For example, for the model BLOCK of the reference stability corpus with its height a parameter `height` (`f-ext1` extrudes the outline of `f-sk1`), its front top edge filleted by `f-fil1` and a sketch `f-sk2` on its top face:

```json
{
  "scripts": [
    {
      "name": "taller-base",
      "description": "The base becomes 40 mm tall instead of 30 mm.",
      "edits": [
        {"type": "update_parameter", "parameter": {"id": "p-height", "name": "height", "expr": "40 mm"}}
      ],
      "expect": {
        "f-fil1": {"/edges/0": "edge(f-ext1/end-cap|f-ext1/lateral/e-1)"},
        "f-sk2": {"/plane/face": "f-ext1/end-cap"}
      }
    }
  ]
}
```

The rules:

1. The file is one JSON object with exactly one member, `scripts`: an array of at least one script.
2. A script is an object with exactly the members `name`, `description`, `edits` and `expect`, in any order.
3. `name` matches `[a-z0-9]+(-[a-z0-9]+)*` and is unique within the file.
4. `description` is a string that is not empty: one sentence that says what the edit does, in the user's words. A reviewer reads it to judge the oracle.
5. `edits` is an array of at least one edit, each in the JSON form that `apply_edits` takes (model.md section 3.9). They are applied in order, as one `apply_edits` with the script's `name` as its label. An edit's `type` is one of `update_parameter`, `add_parameter`, `add_body`, `add_feature`, `update_feature`, `remove_feature` and `move_feature`. A suppression is an `update_feature` that sets `suppressed`.
6. `expect` is an object. Each member's key is the id of a feature of the edited document. Its value is an object whose keys are fields of that feature's stored references (3.1) and whose values are expectations. Together they name exactly the script's downstream references (3.1), each once.
7. An expectation is one of: the string `"lost"`; one element name (naming.md section 3.1); an array of at least two distinct element names, in any order. No element name is `lost`: every element name holds a `/`.

An expectation means:

- an element name: the reference resolves to exactly that element;
- an array: the reference resolves to exactly those elements, the pieces of an element that split (naming.md section 3.9);
- `"lost"`: the reference does not resolve. The feature reports it lost or ambiguous (features.md section 3.8).

Which models carry the file:

- **A corpus model** carries it, with at least three scripts. At least one is an *update script*: each of its edits is an `update_parameter` or an `update_feature`. At least one is an *insertion script*: one of its edits is an `add_feature`. Where the model allows it, the test author also writes a script that moves a feature and one that removes or suppresses one (not checked).
- **An example** outside `examples/bench/` carries it exactly when one of its feature records holds a stored reference (3.1), with at least one script. The benchmark document carries none: it is a performance document, not documentation (ADR 0051 decision 4).
- **The four success parts** of gate 1 are examples. Each carries the file with at least three scripts, as a corpus model does (wave 5).

Every script has at least one downstream reference (3.1). A script that changes none measures nothing.

### 2.5 Stored outcomes

`<stem>.outcomes.json` holds the outcome of every downstream reference of every script of `<stem>.edits.json`, as the last bless stored it. The bless writes it (3.4). No person edits it. For the script of 2.4:

```json
{
  "scripts": [
    {
      "name": "taller-base",
      "references": [
        {
          "feature": "f-fil1",
          "field": "/edges/0",
          "status": "resolved",
          "elements": [
            "edge(f-ext1/end-cap|f-ext1/lateral/e-1)"
          ]
        },
        {
          "feature": "f-sk2",
          "field": "/plane/face",
          "status": "resolved",
          "elements": [
            "f-ext1/end-cap"
          ]
        }
      ]
    }
  ]
}
```

- `scripts` lists every script of the edits file, in its order, each with its `name` and `references`.
- `references` lists every downstream reference of the script (3.1): first by the timeline position of its feature in the edited document, then in the order the edited document's canonical text writes the feature's stored references.
- Each reference is an object with exactly `feature` (the feature's id), `field` (the reference's field, 3.1), `status` (`resolved`, `ambiguous` or `missing`, as `resolve_references` reports it, engine.md 3.9) and `elements` (the names of 3.2, in byte order; `[]` when `missing`), in that order.
- The file is pretty JSON with two-space indentation and one trailing newline, as `serde_json::to_string_pretty` writes it.

The file never holds an expectation, a match or a count. Those come from the oracle on every run (3.3).

### 2.6 The first 100 models

The corpus starts at 100 models in M2, written for the project (m2-plan.md item 9). Each category is a folder of `corpus/`. A row lists the kinds of parts its models are and the features they use between them. The wave columns say how many of the row's models land in each wave, with the features that wave brings (m2-plan.md section 3).

| Category | Models | Parts | Features | Wave 1 | Wave 2 | Wave 3 | Wave 4 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `enclosures` | 12 | Electronics boxes, lids with lips, battery boxes, housings with screw bosses, vent slots and cable openings | `sketch` on faces, `extrude` (new, join, cut), `shell` with removed faces, `fillet`, `chamfer`, `hole`, `linear_pattern`, `boolean`; then a lip from a projected outline and an offset (items 25 and 26), thin-wall extrude (item 29) and embossed labels (item 31) | 8 | 0 | 2 | 2 |
| `brackets` | 12 | L-brackets, angle and shelf brackets, motor and sensor mounts, gussets | `sketch`, `extrude`, `fillet`, `chamfer`, `hole` (counterbore, countersink), `mirror`, `datum_plane`, `split_body`, `move_body`; then tapped and clearance holes (item 21), extents up to a face and taper (item 17), and rib and web (item 28) | 8 | 2 | 2 | 0 |
| `plates` | 10 | Mounting plates, panels with hole grids, perforated covers, rail mounts, spacer plates | `box`, `sketch`, `extrude`, `hole`, `linear_pattern`, `circular_pattern`, `fillet`, `chamfer`, `split_body` | 10 | 0 | 0 | 0 |
| `revolved` | 10 | Knobs, spacers, bushings, pulleys, standoffs, wheels | `revolve`, `cylinder`, `cone`, `torus`, `sphere`, `chamfer`, `fillet`, `circular_pattern`, `hole`, `datum_plane`; then revolve on two sides, symmetric and up to a face (item 17) | 8 | 2 | 0 | 0 |
| `gears` | 8 | Spur gears, gear blanks, racks, pinions with hubs and keyways | `sketch` of lines and arcs, `extrude`, `circular_pattern`, `boolean`, `hole`, `chamfer`; then involute flanks as B-splines by fit points (item 14), and flanks mirrored and patterned in the sketch (items 24 and 25) | 2 | 4 | 2 | 0 |
| `threaded` | 10 | Bottle caps, nuts, bolts, jar lids, threaded inserts, tapped blocks | Helix and coil (item 19), modeled and cosmetic threads, external and internal (item 20), and tapped holes (item 21), with `revolve`, `cylinder`, `chamfer` and `fillet` | 0 | 10 | 0 | 0 |
| `swept` | 8 | Handles, hooks, pipes, cable guides, springs | Sweep along lines, arcs and model edges with its orientation modes (item 18), coil (item 19), `fillet`, `boolean`; then the rail, twist and scale modes and spline paths | 0 | 5 | 0 | 3 |
| `lofted` | 6 | Funnels, nozzles, duct transitions, vases | Loft with rails, a centerline and point ends (item 30), `shell`, `fillet` | 0 | 0 | 6 | 0 |
| `labeled` | 8 | Name tags, keychains, embossed boxes, debossed lids, labels on cylinders and cones | Sketch text (item 27) and its extrude, then emboss and deboss on planar, cylindrical and conical faces (item 31) | 0 | 0 | 2 | 6 |
| `imported` | 6 | STEP parts exported from this repository's models, then filleted, chamfered, drilled, cut and shelled | `import` (fallback names, naming.md section 3.6.14), `fillet`, `chamfer`, `hole`, `boolean`, `shell` | 6 | 0 | 0 | 0 |
| `profiles` | 10 | Sketch-heavy parts: slotted links, polygon nuts, cams, clips, outlines of arcs | M1's sketch entities and constraints with `extrude` and `revolve`; then conics, B-splines and the remaining constraints (items 14 and 15), and the drawing tools, modify tools and projected geometry (items 24 to 26) | 4 | 3 | 3 | 0 |
| Total | 100 | | | 46 | 26 | 17 | 11 |

Between them, the 46 models of wave 1 use each of M1's 20 feature types (corpus check 5). Their scripts measure a reference of each of the 16 types whose params can hold one (corpus check 9, test 8). Each later feature spec names its feature type in its row when its type lands. Its track's pull request adds that row's models of its wave. A cut ADR of the M2 plan's section 6 item 11 that moves a feature to M3 replaces that feature's models in this table with models of features M2 keeps, in the same change. The corpus then still holds 100 models at the gate (test 44).

### 2.7 The corpus checks

Nine checks extend the examples runner of delivery.md 2.8. They are tests in two modules of `crates/partsmith-cli/tests/examples.rs`. The runner's filter `examples::` then runs them with its own tests. Its helpers serve them: `Problems`, the comparison of 2.8, `bless_mode`, `announce` and the parallel runs. Module `examples::model_corpus` (`tests/examples/model_corpus.rs`):

1. `corpus_follows_the_layout`: every file under `corpus/` is at a path of 2.1; every `.expected.json`, `.edits.json` and `.outcomes.json` has its `.ptsm` next to it; every model has its `.edits.json`.
2. `corpus_is_canonical`: `partsmith fmt --check` over every model exits 0.
3. `corpus_models_are_whole_parts`: every model holds at least 5 feature records and at least 2 stored references, and has no `rollback` member (2.2).
4. `corpus_rebuilds_as_expected`: every model, rebuilt with `partsmith rebuild --json` in parallel over `std::thread::available_parallelism()` workers, matches its expected results (2.3). They hold no feature with status `error`. With `PARTSMITH_BLESS=1` it writes a missing or differing expected results file (3.4).
5. `every_feature_type_has_a_corpus_model`: for every type of `Registry::types()` of the engine's registry (as delivery.md's runner test 3 reads it), some model holds a feature record of that type.
6. `every_corpus_asset_is_imported`: every file under `corpus/<category>/assets/` is the `asset` of an `import` feature record of a model in `corpus/<category>/`.

Module `examples::naming_metric` (`tests/examples/naming_metric.rs`):

7. `scripted_edits_are_well_formed`: from the files alone, with no session: every example of 2.4 that must carry an edits file carries one, no other example carries one, no example carries an outcomes file without its edits file next to it, every edits file follows 2.4's rules 1 to 5 and 7 and has the shape of rule 6, and every corpus model's file has its three scripts with an update script and an insertion script.
8. `scripted_edits_run_as_stored`: for every script of every edits file under `examples/` and `corpus/`, the script applies (3.2), the edit changes no record that it does not name and the edited document saves the same after a rebuild (3.5), it has at least one downstream reference, its `expect` names exactly its downstream references (3.1), and the outcomes match the stored outcomes file (3.4). The check runs the scripts in parallel over `std::thread::available_parallelism()` workers. Each worker has one headless session (`Session::new(SessionOptions::headless())`) for its base models and a new one for each edited document (3.2 step 5). With `PARTSMITH_BLESS=1` the check writes a missing or differing outcomes file. It then prints the metric, its mismatches and the longest element name (3.3).
9. `every_reference_holding_type_is_measured`: from the files alone: for every feature type that can hold a reference, a stored outcomes file under `corpus/` lists a reference of a feature of that type. A type *can hold a reference* when, in the `.ptsm` schema that `partsmith schema` prints (`ptsm_schema_json()` of `tests/examples/coverage.rs`), `/$defs/feature.<type>` reaches `/$defs/Reference` through `$ref`s at any depth. A feature's type is read from the model's `.ptsm` by the feature's id.

Each check collects every problem before it fails. It fails once, with the problems sorted by path, one per line (delivery.md 2.8). The problems of checks 5 and 9 name no file and are sorted by their text. A model that cannot be read or is not JSON is reported by checks 2 and 4, as delivery.md's runner tests 2 and 4 report such an example. An edits file that cannot be read or is not JSON is reported by check 7. Each counts for nothing in the other checks, except that check 8 reports a model that does not open at step 1 of section 3.2 (test 53). An outcomes file that cannot be read or is not JSON is a file that does not match: check 8 reports it, and a bless rewrites it. Check 9 counts it for nothing. The modules add no third-party crate. They use the workspace crate `partsmith-naming` as a development dependency of `partsmith-cli`, to parse element names and stored references. A development dependency is not an edge of architecture.md's crate graph, as partsmith-recompute's development dependencies on partsmith-occt-sys and partsmith-sketch are not.

## 3. Behavior

### 3.1 Stored and downstream references

A *stored reference* is a JSON object, at any depth of a feature record, that has a `q` member and deserializes as a naming `Reference` (naming.md section 2.4). These are the references `list_uses` reads (engine.md section 3.24). Its *field* is its JSON Pointer relative to the feature record (RFC 6901), such as `/edges/0`, `/plane/face` or `/second_direction/axis/edge`. Its *body* is the `body` member of the nearest object above it that has a string `body` member: `/plane/body` for `/plane/face`, the record's `/body` for `/edges/0` (spec README decision D2). A stored reference with no such object above it is a problem of check 8.

Let `B` be the base document, read from the model's `.ptsm`, and `E` the edited document, the text `save_document` returns after the script is applied (3.2). A feature record `F` of `E` is *downstream* of the script when all of these hold:

1. `B` has a feature record with `F`'s id. A feature the script adds is never downstream: its references are picks the author wrote.
2. `F` is active in `E`: it has no `"suppressed": true`.
3. The state before `F` may have changed. That is: (a) the records before `F` in `E`'s `features` are not the records before the record with `F`'s id in `B`'s `features`, compared as lists of JSON values with each record's `name` member left out; or (b) a record before `F` in `E`'s `features` uses a changed parameter.

The records compared include the record's `suppressed` member. A feature's `name` is its label and never changes geometry (features.md section 3.12). Each record is compared as a whole JSON value: members by key, arrays in order, numbers as the canonical text writes them.

A *changed parameter* is a parameter of `E` whose `expr` differs from the `expr` of the parameter with the same id in `B`. A parameter of `E` whose expression names a changed parameter is changed too, repeated until no parameter is added. An expression names the parameters of its `Expr::references` (expressions.md section 2). A parameter that `B` does not have is not changed by itself. A record *uses* a parameter when one of the expressions that `Registry::dependencies` lists for it (features.md section 2, `Dependencies::expressions`), from the engine's registry, names that parameter. A record that `Registry::dependencies` refuses uses none.

The *downstream references* of a script are the stored references of its downstream features, less the references the script writes itself. A stored reference of `F` whose value in `E` differs from the value at the same field of `F`'s record in `B`, or that `B`'s record does not have, is a pick the script wrote. It is never a downstream reference. The features' own references resolve against the state before them. That state is the part an edit can move:

- an edit of `F` itself (an `update_feature`) does not make `F` downstream: the state before `F` is unchanged;
- a feature added at position `i` makes the features after it downstream, not itself;
- a removed feature makes every feature after its old place downstream;
- a feature moved from `i` to `j` makes every feature from `min(i, j)` on downstream, the moved feature among them in either direction;
- an `update_parameter` makes every feature after the first record that uses a changed parameter downstream, not that record itself.

### 3.2 Resolving a downstream reference

Each worker of check 8 runs every script it takes. Steps 1 to 4 run in the worker's one headless session (`Session::new(SessionOptions::headless())`). `{message}` is always the engine error's message.

1. `open_document` with the model's text and the assets it imports, read from the `assets/` folder next to it as `partsmith rebuild` reads them (cli.md 2.3). A failing open is the problem `{file}: the model does not open: {message}`, given once for its edits file. None of the file's scripts then runs.
2. Outside any transaction, `list_elements` of each body of the document, in the order of its `bodies`, all kinds. A body that fails with `body_empty` has no geometry and is skipped. These are the base model's names for the longest name of 3.3.
3. `apply_edits` with the script's `name` as `label` and its `edits`. A failing apply is the problem `{file}: script {name} does not apply: {message}`. The script's other steps then do not run.
4. `save_document`. Its text is `E`. `close_document`.
5. In a new headless session, `open_document` with `E` and the same assets. The session holds none of the results of steps 1 to 4: the rebuild is cold, as when a user opens the saved file. The document has no undo history. A failing open is the problem `{file}: script {name}: the edited document does not open: {message}`. The script's other steps then do not run. Steps 6 to 12 and 3.5 run in this session.
6. Outside any transaction, `list_elements` of each body of `E` as in step 2. These are the edited document's names for the longest name of 3.3.
7. For each downstream feature `F`, in the timeline order of `E`: `begin_transaction` (label `Corpus`), then `set_focus` on `F`. The display state is then the state just before `F` (engine.md 3.5), the one `F` resolves its references against.
8. `resolve_references` with every downstream reference of `F` that has a body (3.1), each as `{body, reference}`, in the order of 2.5. Each `Resolution` gives the reference's `status`.
9. For each body whose resolutions hold a match: `list_elements` of that body, all kinds. A match of kind `k` and index `n` is the `n`-th descriptor of kind `k` in that list (the headless convention of engine.md 3.9). Its `name` is the element's name.
10. `cancel_transaction`.
11. The reference's outcome is its `status` with the names of its matches in byte order: `resolved` and its elements, `ambiguous` and its candidates, or `missing` with none.
12. `close_document` of `E`. The session of step 5 ends with the script.

The session resolves with the resolver of partsmith-naming against the element set the feature reads (engine.md 3.9). The outcome holds what the feature would use, from the document alone. A guess resolves to elements with `resolved`. The protocol's `Resolution` carries no match method. A guess that names the oracle's element counts as a match (open issue 2).

### 3.3 The metric

For every downstream reference of every script of every edits file under `examples/` and `corpus/`, the expectation and the outcome *match* when:

- the expectation is `"lost"` and the outcome is `missing` or `ambiguous`;
- the expectation is an element name and the outcome is `resolved` with exactly that one element;
- the expectation is an array and the outcome is `resolved` with exactly its elements, in any order.

A downstream reference is *scored* when its script applied (3.2 step 3), its edited document opened again (step 5), it has a body (3.1), and the script's `expect` names it with an expectation of a form of 2.4 rule 7. A downstream reference with no expectation is not scored, and neither are the references of a script whose steps stop at step 1, 3 or 5. Both are problems of check 8 (section 4), as is an expectation that names no downstream reference. `{examples scored}` and `{corpus scored}` count the scored references of the edits files under `examples/` and under `corpus/`. `{examples matched}` and `{corpus matched}` count the matches among those references.

The metric is `matched / scored`: the matches over every scored downstream reference, the examples and the corpus pooled. Its target is above 99 percent: `100 * matched > 99 * scored`, in integers. Check 8 prints, past the harness's capture (`announce`), once its run has ended:

```text
Naming metric: {matched} of {scored} downstream references resolve as written, {percent} percent ({examples matched} of {examples scored} in examples/, {corpus matched} of {corpus scored} in corpus/). {verdict}
```

- `{percent}` is `100 * matched / scored`, rounded down to two decimals and written with two: `99.04`, `100.00`, `0.00`.
- `{verdict}` is `Above the M2 target of 99 percent.` when `100 * matched > 99 * scored`, and `Not above the M2 target of 99 percent.` otherwise.
- With no scored reference, the line is `Naming metric: no script scores a downstream reference.`

Then one line per reference that does not match, sorted by the edits file's path, then by script order, then by the order of 2.5:

```text
{file}: script {name}: {feature} {field} should be {expected}, and is {outcome}.
```

`{file}` is the edits file's path from the repository root. `{expected}` is `lost`, the element name, or the array's names in byte order joined by `, `. `{outcome}` is `missing`, `resolved to {elements}` or `ambiguous between {elements}`, with the names joined by `, `.

Last, one line with the longest element name that `list_elements` gives in steps 2 and 6 of 3.2: for any body with geometry of any base model or edited document, at its model state, in the sessions of check 8 (naming.md open issue 6). The names of step 9, at focused states, do not count.

```text
Longest element name: {bytes} bytes, in {path}.
```

`{path}` is the `.ptsm` path of the base model, or `{file}, script {name}` for an edited document. The documents are met in this order: the edits files by path, and for each its base model, then its scripts' edited documents in script order. Within a document the names are met in the order of its `bodies`, then in `list_elements` order. Of names of equal length, the first met wins. With no name at all, the line is not printed.

A mismatch is not a problem: check 8 does not fail on the metric. The metric is judged against its target at the M2 gate (gate 6, test 44). Until then every run measures it, and every change to an outcome is seen through the drift of 3.4.

### 3.4 Drift, bless and review

The stored outcomes and the corpus's expected results are baselines. They track drift between runs and never decide what is correct.

- **Drift.** Check 8 compares each script's computed outcomes with its stored outcomes file, as a whole JSON value, by delivery.md 2.8's comparison rules. Each mismatch is a problem with 2.8's mismatch message, `{path}.outcomes.json: {pointer} is {actual}, expected {expected}`, with the stored value as expected. A new script, a script that is no longer in the edits file, a new downstream reference and a changed outcome all show as mismatches. Check 4 does the same for the expected results of the corpus models.
- **Incomplete outcomes.** Check 8 compares and writes an outcomes file only when it computed every outcome of its edits file: the model opened (3.2 step 1), each script applied (step 3), each edited document opened again (step 5), and each downstream reference has a body (3.1). Otherwise it neither compares nor writes that file, and does not report it missing. The problems of those steps fail the check, with or without `PARTSMITH_BLESS`.
- **Bless.** With `PARTSMITH_BLESS` set to `1`, checks 4 and 8 write each file of theirs that is missing or does not match, print `blessed {path}` for each, and pass for those problems. Their other problems still fail. The command is the examples' own: `PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test integration examples::`. Then run `cargo test -p partsmith-cli --test integration examples::` again without `PARTSMITH_BLESS` before committing: check 9 reads the outcomes files that a bless run may rewrite. When the environment variable `CI` is set, a set `PARTSMITH_BLESS` fails each check with `PARTSMITH_BLESS is set in CI. Bless expected results locally and commit them.` (delivery.md 2.8). Without `PARTSMITH_BLESS`, a missing file fails with `{path} is missing. Run: PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test integration examples::`. Before each bless of the corpus's expected results, the libm perturbation check of [kernel.md](kernel.md) open issue 14 runs on the corpus models, as m2-plan.md gate 3 asks before every bless.
- **Never blessed.** No run writes an edits file. The oracle changes only by hand (3.6).
- **Review.** Blessed files are reviewed in the pull request like code. A naming rule that changes an outcome shows the change there, with the metric line of the run before and after. A kernel upgrade that changes a corpus baseline or a stored outcome needs the sign-off of technical design section 16, as the examples' expected results do (delivery.md 2.8).

### 3.5 The edited text

Gate 4 asks that a rebuild after each scripted edit leaves the saved text byte-identical to the edited document: recompute writes nothing, projected coordinates included. Check 8 checks it for every script, in two parts.

- **The rebuild.** In step 5 of 3.2, `save_document` of the newly opened `E`, before any transaction, returns `E` byte for byte. The rebuild is cold: its session holds no result of steps 1 to 4. Otherwise the problem is `{file}: script {name}: a rebuild changes the saved text of the edited document at line {line}.` `{line}` is the first line, counted from 1, at which the two texts differ.
- **The edit.** A value that the recompute of 3.2 step 3 wrote into the recipe is already in `E`. The rebuild writes it again. The first part cannot see it. `apply_edits` changes only the records its edits name (model.md section 3.9, engine.md section 3.3). Check 8 therefore also compares `E` with `B`, which is in canonical form (check 2, delivery.md's runner test 2). These parts of `E` equal, as JSON values, the part of `B` with the same id or key: each feature record whose id no `add_feature` or `update_feature` edit of the script names; each parameter whose id no `add_parameter` or `update_parameter` edit names; each body whose id no `add_body` edit names; and each top-level member other than `features`, `parameters` and `bodies`. A part that differs is the problem `{file}: script {name}: applying the edits changes {part}, which no edit of the script names.` `{part}` is `feature {id}`, `parameter {id}`, `body {id}` or `member {key}`. The problems of one script are sorted by `{part}` in byte order.

### 3.6 Writing an oracle

The test author writes each expectation from the model, the script and naming.md, without applying the script with the engine, running check 8 or reading a stored outcome (technical design section 17, test author). The author may list the elements of the base model, an input of the script, with the engine.

1. The expected element is the one the user meant when the reference was picked, as it stands after the edit: the same face, edge or vertex of the part, judged by its role in the design (the top of the lid, the edge the fillet rounds), not by its position or its number.
2. When the edit removes that element, the expectation is `"lost"`: it was cut away, its feature was removed or suppressed, or it no longer exists in the state the feature reads.
3. When the element splits into pieces that each keep its role, the expectation lists every piece. When it merges into a larger face, the expectation is the face it became.
4. When a sketch entity the element came from was replaced or continued (naming.md section 3.6.2), the expectation is the element made from what replaced or continued it.
5. When the model cannot tell which of several elements the user meant, the expectation is `"lost"`. Partsmith must ask, never guess (the error guidelines, "Never guess silently").
6. Each name is the element's name as `list_elements` gives it at the state its feature reads (`ElementInfo::name`, naming.md section 2.7), by naming.md sections 3.1 to 3.12 in the edited model. That name is its face-rule name when it has one, otherwise its first direct-generator name (naming.md section 3.12.2 step 4), with its `#n` by the order of section 3.4. A direct-generator name that the element holds only as an alias is never written.

A later change to an expectation states its reason in its pull request. A reviewer judges it there against these rules. An implementer never changes an expectation, a script or a model to raise the metric (AGENTS.md Process).

### 3.7 In CI

`ci.yml`'s `rust` job runs the nine checks in `cargo test --workspace`, with the examples runner (delivery.md 3.7). `rust-platforms` runs them on `windows-2025` and `macos-15` in its step `cargo test -p partsmith-cli --test integration examples::`. Their tests are in that module (delivery.md 2.14). Both run on every pull request and every push to `main`. Every merge is such a push. No workflow changes. A problem of any check fails the job and holds the deploy (delivery.md 2.4). The metric line is in the log of every run. The baselines are the same on every platform: the expected results and outcomes blessed on Linux, at delivery.md 2.8's tolerance.

## 4. Problems and messages

Every problem names the file it is about, from the repository root: `{path}` is a model's or an example's path without its extension, and `{file}` an edits file's path. `{name}` is a script's name, `{feature}` a feature id and `{field}` a reference's field. Within one file, and in check 8 within one script, a check lists its problems in the order of this table. It lists the downstream references with no expectation in the order of 2.5. It lists the expectations that name no downstream reference by feature id, then field, in byte order. When a script's `name` is not a string that matches 2.4 rule 3, its other problems write `/scripts/{position}` in place of `script {name}`.

| Situation | Check | Message |
| --- | --- | --- |
| A file under `corpus/` at no path of 2.1 | 1 | `corpus/{relative}: not an allowed corpus path (docs/spec/corpus.md section 2.1).` |
| A companion with no `.ptsm` next to it | 1 | `corpus/{relative}: has no .ptsm next to it.` |
| A model with no edits file | 1 | `{path}.edits.json is missing. Write the model's scripted edits and what each downstream reference should resolve to (docs/spec/corpus.md section 2.4).` |
| A model not in canonical form, or one that does not load | 2 | `{path}.ptsm: not canonical. Run: partsmith fmt {path}.ptsm`, or the load error, as delivery.md's runner test 2 words them |
| Fewer than 5 feature records | 3 | `{path}.ptsm: has fewer than 5 features. A corpus model is a whole part with at least 5 (docs/spec/corpus.md section 2.2).` |
| Fewer than 2 stored references | 3 | `{path}.ptsm: stores fewer than 2 references. A corpus model stores at least 2 (docs/spec/corpus.md section 2.2).` |
| A `rollback` member | 3 | `{path}.ptsm: corpus models must not set the rollback marker.` |
| Expected results missing | 4 | `{path}.expected.json is missing. Run: PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test integration examples::` |
| A rebuild that differs | 4 | `{path}.ptsm: {pointer} is {actual}, expected {expected}` (for a missing or extra key: `{pointer} is missing` or `{pointer} is unexpected`), as delivery.md 2.8's mismatch |
| A rebuild that fails to run | 4 | delivery.md 2.8's message, `{path}.ptsm: partsmith rebuild exited with {status}: {reason}` |
| A feature with status `error` in the expected results | 4 | `{path}.ptsm: feature {feature} has status error. A corpus model builds without an error (docs/spec/corpus.md section 2.2).` |
| A registered type with no corpus model | 5 | `Feature type {type} has no model in corpus/. Add one with its scripted edits (docs/spec/corpus.md section 2.6).` |
| An asset that no model of its folder imports | 6 | `corpus/{category}/assets/{asset}: no model in corpus/{category}/ imports it. Import it there or delete it.` |
| An example that stores a reference with no edits file | 7 | `{path}.ptsm: stores references but has no edits file. Write its scripted edits (docs/spec/corpus.md section 2.4).` |
| An example outside `examples/bench/` that stores no reference, with an edits file | 7 | `{path}.edits.json: the example stores no reference. Delete the edits file (docs/spec/corpus.md section 2.4).` |
| An outcomes file next to an example with no edits file | 7 | `{path}.outcomes.json: has no edits file next to it.` |
| An edits file that cannot be read or is not JSON | 7 | `{file}: cannot be read as JSON: {reason}` |
| No `scripts` array, or an empty one | 7 | `{file}: has no scripts.` |
| A member that 2.4 does not name, at the top level or in a script | 7 | `{file}: {pointer} is not a member of an edits file (docs/spec/corpus.md section 2.4).` |
| A script without one of its four members | 7 | `{file}: /scripts/{position} has no {member}.` (`{position}` its 0-based position in `scripts`, `{member}` the first missing of `name`, `description`, `edits`, `expect`) |
| A name that is not a string, or does not match 2.4 rule 3 | 7 | `{file}: /scripts/{position}/name is not lowercase words and digits joined by hyphens.` |
| A name used twice | 7 | `{file}: script {name} is written more than once.` |
| A description that is not a string, or an empty one | 7 | `{file}: script {name} has no description.` |
| `edits` that is not an array, or an empty one | 7 | `{file}: script {name} has no edits.` |
| An edit that is not an edit | 7 | `{file}: script {name}: edit {number} is not an edit: {reason}` (`{number}` counted from 1, `{reason}` the deserialization error) |
| An edit of a type 2.4 does not allow | 7 | `{file}: script {name}: edit {number} is a {type} edit, which a scripted edit cannot use (docs/spec/corpus.md section 2.4).` |
| `expect` that is not an object of objects | 7 | `{file}: script {name}: expect is not an object of feature ids to objects of fields.` |
| An expectation of another form | 7 | `{file}: script {name}: {feature} {field} should be "lost", an element name or a list of two or more different element names.` |
| A corpus model's file with fewer than three scripts | 7 | `{file}: has fewer than 3 scripts. A corpus model has at least 3 (docs/spec/corpus.md section 2.4).` |
| No update script | 7 | `{file}: has no script that only updates parameters or features (docs/spec/corpus.md section 2.4).` |
| No insertion script | 7 | `{file}: has no script that adds a feature (docs/spec/corpus.md section 2.4).` |
| The model does not open | 8 | `{file}: the model does not open: {message}` |
| The script does not apply | 8 | `{file}: script {name} does not apply: {message}` |
| The edited document does not open | 8 | `{file}: script {name}: the edited document does not open: {message}` |
| The edit changes a part of the document that no edit names | 8 | `{file}: script {name}: applying the edits changes {part}, which no edit of the script names.` |
| The edited text changes after a rebuild | 8 | `{file}: script {name}: a rebuild changes the saved text of the edited document at line {line}.` |
| A stored reference with no body above it | 8 | `{file}: script {name}: {feature} {field} has no body next to it.` |
| No downstream reference | 8 | `{file}: script {name} changes no downstream reference (docs/spec/corpus.md section 3.1).` |
| A downstream reference with no expectation | 8 | `{file}: script {name}: {feature} {field} is a downstream reference with no expectation.` |
| An expectation for a reference that is not downstream | 8 | `{file}: script {name}: {feature} {field} is not a downstream reference of this script.` |
| Stored outcomes missing | 8 | `{path}.outcomes.json is missing. Run: PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test integration examples::` |
| Stored outcomes that differ | 8 | `{path}.outcomes.json: {pointer} is {actual}, expected {expected}`, as delivery.md 2.8's mismatch |
| Stored outcomes that cannot be read or are not JSON | 8 | `{path}.outcomes.json: cannot be read as JSON: {reason}` |
| A reference-holding type that no corpus script measures | 9 | `Feature type {type} can hold a reference, but no script in corpus/ measures one. Add a script whose edit comes before a {type} feature that stores one (docs/spec/corpus.md section 2.6).` |
| Bless in CI | 4, 8 | `PARTSMITH_BLESS is set in CI. Bless expected results locally and commit them.` |

## 5. Budgets

| Measure | Budget |
| --- | --- |
| Checks 1 to 9 together in `rust`, on `ubuntu-24.04`, with the debug build inside `cargo test --workspace` | 3 minutes. An estimate: on 2026-11-07, on the 4-core development container, a debug `partsmith rebuild --json` took 0.06 to 0.66 s for four examples and 3.6 s for the 50-feature benchmark, which check 8 leaves out (2.4). A script costs about three rebuilds: the base, the applied edit and the new open. The first CI run of the change measures the checks (test 43). The row then becomes the measured value plus 20 percent (ADR 0057). A measurement over 3 minutes moves the checks into a job of their own with `--release` by a new ADR, as delivery.md open issue 4 says for the examples. |

## 6. Acceptance tests

Tests 1 to 42 and 46 to 53 run in the two modules of 2.7, on literal files that each test writes in a temporary directory, on literal JSON and on the engine's registry and schemas, unless a test says otherwise. Each checks one behavior. In tests 3, 17 to 34, 46, 47, 50 and 52, the base document `BOX` is: units `mm`; parameters `p-height` (`height`, `10 mm`), `p-base` (`base`, `1 mm`) and `p-radius` (`radius`, `base * 2`); bodies `b-1`; features, in this order:

- `f-box1`: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "20", "height"], "op": "new"}`, the box from x -20 to 20, y -10 to 10 and z 0 to 10;
- `f-fil1`: a `fillet` of `b-1` with `edges` `[{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,10|l=40|adj=2,2"}]` and `radius` `radius`;
- `f-sk1`: a `sketch` whose `plane` is `{"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=800|adj=4,4"}, "fallback": {"origin": [0, 0, 10], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}, "fallback_frame": "body"}`, with the one entity `{"id": "e-p1", "type": "point", "position": [15, 0]}` and no constraint;
- `f-hole1`: a `hole` of `b-1` with `sketch` `f-sk1`, `points` `["e-p1"]`, `diameter` `4` and `extent` `{"kind": "through_all"}`;
- `f-cha1`: a `chamfer` of `b-1` with `edges` `[{"q": "edge(f-box1/back|f-box1/top)", "fp": "line|n=1,0,0|c=0,10,10|l=40|adj=2,2"}]` and `distance` `{"kind": "equal", "expr": "1"}`.

Its stored references are `f-fil1` `/edges/0`, `f-sk1` `/plane/face` and `f-cha1` `/edges/0`. A script named in a test as `[edit, edit]` holds those edits, in that order.

1. Allowed corpus paths. `corpus/brackets/l-bracket.ptsm`, `corpus/brackets/l-bracket.expected.json`, `corpus/brackets/l-bracket.edits.json`, `corpus/brackets/l-bracket.outcomes.json` and `corpus/imported/assets/<64 lowercase hex digits>.step` are allowed. `corpus/l-bracket.ptsm`, `corpus/hinges/pin.ptsm`, `corpus/brackets/L-Bracket.ptsm`, `corpus/brackets/l-bracket.py`, `corpus/brackets/part/l-bracket.ptsm` and `corpus/imported/assets/notes.txt` each give `corpus/{relative}: not an allowed corpus path (docs/spec/corpus.md section 2.1).` with its path below `corpus/`.
2. Companions and the edits file. `corpus/plates/grid.outcomes.json` with no `grid.ptsm` gives `corpus/plates/grid.outcomes.json: has no .ptsm next to it.` `corpus/plates/grid.ptsm` with no `grid.edits.json` gives `corpus/plates/grid.edits.json is missing. Write the model's scripted edits and what each downstream reference should resolve to (docs/spec/corpus.md section 2.4).` A model with no `.expected.json` or `.outcomes.json` gives no problem in check 1.
3. Whole parts. `BOX` with `f-cha1` left out gives `{path}.ptsm: has fewer than 5 features. A corpus model is a whole part with at least 5 (docs/spec/corpus.md section 2.2).` `BOX` with `f-fil1` and `f-cha1` left out and a `box` and a `cylinder` added at the end gives `{path}.ptsm: stores fewer than 2 references. A corpus model stores at least 2 (docs/spec/corpus.md section 2.2).` `BOX` with `"rollback": "f-cha1"` gives `{path}.ptsm: corpus models must not set the rollback marker.` `BOX` as it is gives none.
4. The expected results. The report `{"path": "x.ptsm", "printer": "generic-fdm", "features": [{"id": "f-box1", "type": "box", "status": "ok", "messages": []}], "parameters": [{"id": "p-h", "name": "height", "value": 40.0, "dimension": {"length": 1, "angle": 0}}], "bodies": [{"id": "b-1", "name": "Block", "volume": 24000.0, "area": 5200.0, "center_of_mass": [0.0, 0.0, 20.0], "bounding_box": {"min": [-10.0, -15.0, 0.0], "max": [10.0, 15.0, 40.0]}, "faces": 6, "edges": 12, "vertices": 8}, {"id": "b-2", "name": "Spare", "empty": true}]}` reduces to the JSON of 2.3, written as the text of 2.3 byte for byte.
5. An error in the base. Expected results whose `f-fil1` has status `error` give `{path}.ptsm: feature f-fil1 has status error. A corpus model builds without an error (docs/spec/corpus.md section 2.2).` Status `warning` gives none.
6. The corpus uses delivery.md's comparison. A rebuild whose `/bodies/0/volume` is `24000.06` against an expected `24000.0` gives `{path}.ptsm: /bodies/0/volume is 24000.06, expected 24000.0`. `24000.02` matches.
7. Feature types. With the registered types `box` and `fillet`, and models that hold only `box` records, check 5 gives exactly `Feature type fillet has no model in corpus/. Add one with its scripted edits (docs/spec/corpus.md section 2.6).`
8. Reference-holding types. With a literal schema whose `/$defs/feature.a` reaches `/$defs/Reference` through two `$ref`s and whose `/$defs/feature.b` reaches none, check 9 treats `a` as a type that can hold a reference and not `b` (2.7 check 9). With the `.ptsm` schema that `partsmith schema` prints for M1's 20 registered types, the types that can hold a reference are exactly `box`, `chamfer`, `circular_pattern`, `cone`, `cylinder`, `datum_plane`, `fillet`, `linear_pattern`, `mirror`, `move_body`, `revolve`, `shell`, `sketch`, `sphere`, `split_body` and `torus`. `boolean`, `extrude`, `hole` and `import` cannot. Each later feature spec that registers a type, or lets a type hold a reference, moves or adds that type in these two lists in its own change.
9. Measured types. With the reference-holding types `chamfer`, `fillet` and `sketch`, and corpus outcomes files that list references of `f-fil1` and `f-sk1` of `BOX` only, check 9 gives exactly `Feature type chamfer can hold a reference, but no script in corpus/ measures one. Add a script whose edit comes before a chamfer feature that stores one (docs/spec/corpus.md section 2.6).` An outcomes file under `examples/` that lists `f-cha1` changes nothing.
10. Assets. `corpus/imported/assets/<hash>.step` that no model of `corpus/imported/` imports gives `corpus/imported/assets/<hash>.step: no model in corpus/imported/ imports it. Import it there or delete it.` The same file imported by a model of `corpus/plates/` gives it too.
11. Edits file shape. Each of these files gives exactly the problems shown: `{}` and `{"scripts": []}`: `{file}: has no scripts.`; `{"scripts": [], "notes": "x"}`: `{file}: has no scripts.` and `{file}: /notes is not a member of an edits file (docs/spec/corpus.md section 2.4).`, in this order; a script without `description`: `{file}: /scripts/0 has no description.`; a script named `Taller Box`: `{file}: /scripts/0/name is not lowercase words and digits joined by hyphens.`; a script with `"description": ""`: `{file}: script taller-box has no description.`; a script with `"edits": []`: `{file}: script taller-box has no edits.`; a script with `"expect": []`: `{file}: script taller-box: expect is not an object of feature ids to objects of fields.`
12. A name twice. Two scripts named `taller-box` give `{file}: script taller-box is written more than once.`, once.
13. Edit types. A script whose second edit is `{"type": "set_units", "units": "in"}` gives `{file}: script taller-box: edit 2 is a set_units edit, which a scripted edit cannot use (docs/spec/corpus.md section 2.4).` Edits of each of the seven types of 2.4 rule 5 give none. An edit `{"type": "update_feature"}` gives `{file}: script taller-box: edit 1 is not an edit: {reason}`.
14. Expectation forms. `"lost"`, `"edge(f-box1/front|f-box1/top)"` and `["f-box1/top#1", "f-box1/top#2"]` give no problem. `["f-box1/top"]`, `["f-box1/top", "f-box1/top"]`, `"Face7"`, `"lost "` and `3`, as the expectation of `f-fil1` `/edges/0`, each give `{file}: script taller-box: f-fil1 /edges/0 should be "lost", an element name or a list of two or more different element names.`
15. Examples that carry an edits file. `examples/fillet/front.ptsm`, whose `f-fil1` stores a reference, with no `front.edits.json`, gives `examples/fillet/front.ptsm: stores references but has no edits file. Write its scripted edits (docs/spec/corpus.md section 2.4).` `examples/box/plain.ptsm`, with no stored reference and no edits file, gives none. `examples/bench/fifty-features/model.ptsm`, which stores references, gives none.
16. A corpus model's scripts. Its edits file with two scripts gives `{file}: has fewer than 3 scripts. A corpus model has at least 3 (docs/spec/corpus.md section 2.4).` Three scripts that each hold an `add_feature` give `{file}: has no script that only updates parameters or features (docs/spec/corpus.md section 2.4).` Three scripts of `update_parameter` and `update_feature` edits alone give `{file}: has no script that adds a feature (docs/spec/corpus.md section 2.4).` An example's edits file with one script gives none of the three.
17. Downstream after an update of a feature. On `BOX`, the script `[update_feature f-box1 with size ["40", "20", "15"]]` has the downstream references `f-fil1` `/edges/0`, `f-sk1` `/plane/face` and `f-cha1` `/edges/0`, in that order.
18. An updated feature is not downstream of itself. On `BOX`, `[update_feature f-fil1 with radius "3"]` has the downstream references `f-sk1` `/plane/face` and `f-cha1` `/edges/0`, and not `f-fil1` `/edges/0`.
19. An added feature. On `BOX`, `[add_feature at index 1 of f-cha2, a chamfer of b-1 with edges [{"q": "edge(f-box1/left|f-box1/top)", "fp": "line|n=0,1,0|c=-20,0,10|l=20|adj=2,2"}] and distance {"kind": "equal", "expr": "1"}]` has the downstream references `f-fil1` `/edges/0`, `f-sk1` `/plane/face` and `f-cha1` `/edges/0`, and not `f-cha2` `/edges/0`.
20. A removed feature. On `BOX`, `[remove_feature f-fil1]` has the downstream references `f-sk1` `/plane/face` and `f-cha1` `/edges/0`.
21. A feature moved up and down. On `BOX`, `[move_feature f-cha1 to index 1]` has the downstream references `f-cha1` `/edges/0`, `f-fil1` `/edges/0` and `f-sk1` `/plane/face`, in the edited timeline's order. `[move_feature f-fil1 to index 4]` has `f-sk1` `/plane/face`, `f-cha1` `/edges/0` and `f-fil1` `/edges/0`.
22. A changed parameter. On `BOX`, `[update_parameter p-height with expr "15 mm"]` has the downstream references `f-fil1` `/edges/0`, `f-sk1` `/plane/face` and `f-cha1` `/edges/0`: `f-box1` uses `height`.
23. A parameter changed through another. On `BOX`, `[update_parameter p-base with expr "1.5 mm"]` changes `base` and `radius`. Its downstream references are `f-sk1` `/plane/face` and `f-cha1` `/edges/0`: `f-fil1`, the first record that uses `radius`, is not downstream of itself.
24. A label alone. On `BOX`, the script `rename-base`, `[update_feature f-box1 with "name": "Base" and its params unchanged]`, has no downstream reference. Check 8 gives `{file}: script rename-base changes no downstream reference (docs/spec/corpus.md section 3.1).`
25. A suppressed feature. On `BOX`, `[update_feature f-box1 with size ["40", "20", "15"], update_feature f-fil1 with "suppressed": true]` has the downstream references `f-sk1` `/plane/face` and `f-cha1` `/edges/0`.
26. The body of a reference. In the record `{"id": "f-lp1", "type": "linear_pattern", "body": "b-1", "direction": {"axis": {"kind": "origin", "axis": "X"}, "count": "3", "spacing": "20"}, "second_direction": {"axis": {"kind": "edge", "body": "b-2", "edge": {"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,10|l=40|adj=2,2"}, "direction": [1, 0, 0]}, "count": "2", "spacing": "15"}}`, the stored reference `/second_direction/axis/edge` has the body `b-2`, not the record's `b-1`. In `BOX`, `f-fil1` `/edges/0` has the body `b-1`, and so does `f-sk1` `/plane/face`.
27. The oracle covers the downstream references exactly. On `BOX`, the script `radius-three` of test 18 with `expect` `{"f-fil1": {"/edges/0": "lost"}, "f-sk1": {"/plane/face": "f-box1/top"}}` gives, in this order, `{file}: script radius-three: f-cha1 /edges/0 is a downstream reference with no expectation.` and `{file}: script radius-three: f-fil1 /edges/0 is not a downstream reference of this script.`
28. A followed reference, through the engine. `BOX` as an example, with the script of test 17 and the expectations `edge(f-box1/front|f-box1/top)` for `f-fil1` `/edges/0`, `f-box1/top` for `f-sk1` `/plane/face` and `edge(f-box1/back|f-box1/top)` for `f-cha1` `/edges/0`. Check 8 gives three outcomes `resolved` with those elements, and each matches.
29. A lost reference, through the engine. `BOX` with the script `[add_feature at index 1 of f-box2, a box of b-1 of size ["50", "4", "4"] at position ["0", "-10", "6"] with op cut]`, which cuts the front top edge away. The outcome of `f-fil1` `/edges/0` is `missing` with `[]`. It matches the expectation `"lost"`. With the expectation `edge(f-box1/front|f-box1/top)` it does not match.
30. A split reference, through the engine. `BOX` with the script `[add_feature at index 1 of f-box3, a box of b-1 of size ["10", "30", "4"] at position ["0", "0", "8"] with op cut]`, a slot across the top. The outcome of `f-sk1` `/plane/face` is `resolved` with `["f-box1/top#1", "f-box1/top#2"]`. It matches the expectation `["f-box1/top#2", "f-box1/top#1"]`. It does not match `"f-box1/top#1"`.
31. The match rule, on literal outcomes. `"lost"` matches `missing` with `[]` and `ambiguous` with `["a/x#1", "a/x#2"]`, and not `resolved` with `["a/x"]`. `"a/x"` matches `resolved` with `["a/x"]`, and not `resolved` with `["a/x", "a/y"]`, `ambiguous` with `["a/x", "a/y"]` or `missing`. `["a/y", "a/x"]` matches `resolved` with `["a/x", "a/y"]`, and not `resolved` with `["a/x"]`.
32. Names from the element list. In test 28, the name of each outcome is the `name` of the descriptor at the match's index among the descriptors of its kind in `list_elements` of `b-1` at the focused state, and `elements` is in byte order.
33. The edited text. The texts `a\nb\nc\n` saved and `a\nx\nc\n` expected give `{file}: script {name}: a rebuild changes the saved text of the edited document at line 2.` Equal texts give none. In tests 28 to 30, every script keeps its text.
34. The outcomes file. For tests 28 and 30 run as one example `examples/fillet/box-test.ptsm` with both scripts (`taller` and `slot-across-top`), the bless writes `examples/fillet/box-test.outcomes.json` exactly as 2.5 lays it out: `scripts` in the edits file's order; in each, the references in the order of test 17; each reference with `feature`, `field`, `status` and `elements` in that order; two-space indentation and one trailing newline.
35. Drift. With that file stored and its `/scripts/0/references/0/status` changed to `"missing"` and its `elements` to `[]`, a run without `PARTSMITH_BLESS` gives, in this order, `examples/fillet/box-test.outcomes.json: /scripts/0/references/0/status is "resolved", expected "missing"` and `examples/fillet/box-test.outcomes.json: /scripts/0/references/0/elements/0 is unexpected`, and fails.
36. A missing outcomes file. Without `PARTSMITH_BLESS`, check 8 gives `examples/fillet/box-test.outcomes.json is missing. Run: PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test integration examples::`. With `PARTSMITH_BLESS=1` and no `CI`, it writes the file, prints `blessed examples/fillet/box-test.outcomes.json`, and passes.
37. A bless writes no edits file. A bless run over an edits file that gives a problem of check 7 leaves the edits file byte for byte as it was, and check 7 still fails.
38. Bless in CI. With `CI` and `PARTSMITH_BLESS` both set, checks 4 and 8 each fail with `PARTSMITH_BLESS is set in CI. Bless expected results locally and commit them.` and write nothing.
39. The metric line. 1238 matches of 1250, 400 of 402 in examples and 838 of 848 in the corpus, give `Naming metric: 1238 of 1250 downstream references resolve as written, 99.04 percent (400 of 402 in examples/, 838 of 848 in corpus/). Above the M2 target of 99 percent.` 99 of 100 give `99.00 percent` and `Not above the M2 target of 99 percent.` 199999 of 200000 give `99.99 percent`. 0 of 0 gives `Naming metric: no script scores a downstream reference.`
40. Mismatch lines. In `corpus/brackets/l-bracket.edits.json`, script `slot`: an expectation `edge(f-box1/front|f-box1/top)` with the outcome `missing` gives `corpus/brackets/l-bracket.edits.json: script slot: f-fil1 /edges/0 should be edge(f-box1/front|f-box1/top), and is missing.`; `"lost"` with `resolved` `["f-box1/top"]` gives `... f-sk1 /plane/face should be lost, and is resolved to f-box1/top.`; `["f-box1/top#2", "f-box1/top#1"]` with `ambiguous` `["f-box1/top#1", "f-box1/top#3"]` gives `... should be f-box1/top#1, f-box1/top#2, and is ambiguous between f-box1/top#1, f-box1/top#3.` Mismatches print and do not fail check 8.
41. The longest name. When the longest name `list_elements` gives in a run is 214 bytes long, in the edited document of the script `slot` of `corpus/brackets/l-bracket.edits.json`, the last line is `Longest element name: 214 bytes, in corpus/brackets/l-bracket.edits.json, script slot.`
42. Check 8 does not fail on the metric. A run whose only differences from the oracle are mismatches, with every outcome as stored, passes and prints the `Not above` verdict when it applies.
43. The first CI run, by CI. The first CI run of the change that adds the modules passes `rust` and both rows of `rust-platforms`. The log of `rust` holds the metric line. The time of checks 1 to 9 in `rust` replaces the estimate of section 5.
44. At the gate, by CI and review. On the M2 gate's commit, the corpus holds 100 models, each category with the count of 2.6's table as the last cut ADR leaves it. The four success parts carry their edits files. The CI run of that commit passes the nine checks. The log of its `rust` job holds a metric line that ends with `Above the M2 target of 99 percent.` (m2-plan.md gate 6). The gate's record quotes the line.
45. Wave 1, by review. Wave 1 ends with the 46 models of 2.6's wave 1 column, each with its edits, expected results and outcomes, and with an edits file for every example of 2.4. Check 5 finds every M1 type and check 9 every reference-holding type of test 8. The run's metric line is recorded in open issue 6.
46. Scored references. `BOX` as the example `examples/fillet/box-test.ptsm` with one script, `taller` of test 34, whose `expect` names `f-fil1` `/edges/0` and `f-sk1` `/plane/face` as test 28 does and leaves out `f-cha1` `/edges/0`. With its outcomes file blessed, check 8 gives exactly the problem `examples/fillet/box-test.edits.json: script taller: f-cha1 /edges/0 is a downstream reference with no expectation.` and prints `Naming metric: 2 of 2 downstream references resolve as written, 100.00 percent (2 of 2 in examples/, 0 of 0 in corpus/). Above the M2 target of 99 percent.`
47. Incomplete outcomes. `BOX` as the example of test 34, with a third script `bad-move`, `[move_feature f-cha1 to index 9]`, which does not apply. A bless run writes no `examples/fillet/box-test.outcomes.json`, prints no `blessed` line for it, and fails with `examples/fillet/box-test.edits.json: script bad-move does not apply: {message}`. With test 34's file stored and no `PARTSMITH_BLESS`, the run gives that problem alone: the stored file is not compared.
48. An edits file where none belongs. `examples/box/plain.edits.json` next to `examples/box/plain.ptsm`, which stores no reference, gives `examples/box/plain.edits.json: the example stores no reference. Delete the edits file (docs/spec/corpus.md section 2.4).`
49. An outcomes file without its edits file. `examples/fillet/front.outcomes.json` next to `examples/fillet/front.ptsm`, with no `front.edits.json`, gives `examples/fillet/front.outcomes.json: has no edits file next to it.`, as well as test 15's problem for the missing edits file.
50. The edit changes nothing else. On `BOX`, with the script `[update_parameter p-height with expr "15 mm"]`, an edited text that equals `BOX` but for `p-height` gives no problem of 3.5. The same text with `f-sk1`'s `fallback` `origin` written `[0, 0, 15]` gives `{file}: script {name}: applying the edits changes feature f-sk1, which no edit of the script names.` The texts are literal, as in test 33.
51. The longest name, in order. With names of 214 bytes in `b-2` of the base model `corpus/brackets/l-bracket.ptsm` and in `b-1` of the edited document of its script `slot`, and no longer name from steps 2 and 6 of 3.2, the last line is `Longest element name: 214 bytes, in corpus/brackets/l-bracket.ptsm.` A name of 230 bytes that only step 9 lists, at a focused state, changes nothing.
52. A pick the script writes. On `BOX`, `[update_feature f-box1 with size ["40", "20", "15"], update_feature f-fil1 with edges [{"q": "edge(f-box1/left|f-box1/top)", "fp": "line|n=0,1,0|c=-20,0,15|l=20|adj=2,2"}]]` has the downstream references `f-sk1` `/plane/face` and `f-cha1` `/edges/0`, and not `f-fil1` `/edges/0`, whose value the script wrote.
53. A model that does not open. An edits file with three scripts next to a model whose text is `{` gives `{file}: the model does not open: {message}` once, and no problem of its scripts.

## 7. Open issues

1. **Region references.** The metric scores element references alone. An extrude or a revolve also stores its profile regions (`key` and `fp`, sketch.md and ADR 0022). The sketch's region matching resolves them. No engine command resolves a stored region against a sketch. Section 3.2 cannot ask for one through the protocol. Scoring them needs such a command, a protocol schema change for review at a gate. A later spec adds it, with region expectations in the edits files. Until then the reference stability corpus covers regions (R01 to R07, R50 and R66).
2. **A guess counts by its elements.** The protocol's `Resolution` carries a status and matches, not the resolver's `MatchMethod`. A reference that the resolver guesses scores as a match when the guess is the oracle's element. Features report such a reference with `reference_guessed`. Picks store anchors (naming.md section 3.11). A guess comes only from a reference written without them. If the protocol carries the method later, a guess can be scored apart.
3. **The time of the checks.** Section 5's budget is an estimate until the first CI run (test 43).
4. **A crash inside OCCT.** Check 8 builds in the test process, in its workers' sessions. A crash of the kernel there ends the test binary without naming the model, as runner test 5's imports do (delivery.md 2.8).
5. **Fuzzing seeds.** The fuzz job reads `examples/` as seeds for every target (delivery.md 2.15). It does not read `corpus/` yet. Adding it is a change of the fuzz job's run line (ADR 0067 choice 4).
6. **The first measurement.** Wave 1's run of check 8 on M1's naming records here its metric line and its mismatches (test 45). The gate judges the target (test 44). The run of 2026-11-08, over the 46 models of wave 1 and the edits files of the 97 examples, printed `Naming metric: 779 of 787 downstream references resolve as written, 98.98 percent (169 of 171 in examples/, 610 of 616 in corpus/). Not above the M2 target of 99 percent.` and `Longest element name: 336 bytes, in examples/shell/moved-body.ptsm.` (naming.md open issue 6: far below the limit of 16384 bytes). Every script applied, every edited document saved the same after a rebuild, and every downstream reference had a body and an expectation. The track's review changed three expectations before that run, by section 3.6. In `examples/extrude/side-line-split.edits.json` and `examples/revolve/side-line-split.edits.json`, `f-sh1` `/remove_faces/0` picked the whole side, which the base model already follows to both pieces of its split line: the expectation lists both pieces (rules 3 and 4). In `corpus/plates/hole-grid-panel.edits.json`, script `chamfer-first` moves the chamfer of the front top edge before `f-lp1`, which reads that edge: its expectation is `"lost"` (rule 2). The run's 8 mismatches:
   - `corpus/brackets/split-l-bracket.edits.json: script upright-chamfer-first: f-cha1 /edges/0 should be edge(f-ext1/lateral/e-1#2|f-ext1/lateral/e-6#2), and is missing.`
   - `corpus/imported/drilled-bracket.edits.json: script notch-first: f-cha1 /edges/0 should be edge(f-imp1/face#13|f-imp1/face#20#1), edge(f-imp1/face#13|f-imp1/face#20#2), and is missing.`
   - `corpus/imported/flatted-bushing.edits.json: script deeper-flat: f-fil1 /edges/0 should be edge(f-imp1/face#4|f-imp1/face#5), and is ambiguous between edge(f-imp1/face#3|f-imp1/face#4), edge(f-imp1/face#4|f-imp1/face#5).`
   - `corpus/imported/flatted-bushing.edits.json: script no-flat: f-fil1 /edges/0 should be edge(f-imp1/face#4|f-imp1/face#5), and is ambiguous between edge(f-imp1/face#3|f-imp1/face#4), edge(f-imp1/face#3|f-imp1/face#6), edge(f-imp1/face#4|f-imp1/face#5).`
   - `corpus/imported/keyed-cam.edits.json: script bigger-lightening-hole: f-fil1 /edges/0 should be edge(f-imp1/face#4|f-imp1/face#8), and is ambiguous between edge(f-imp1/face#4|f-imp1/face#5), edge(f-imp1/face#4|f-imp1/face#8).`
   - `corpus/imported/keyed-cam.edits.json: script no-lightening-hole: f-fil1 /edges/0 should be edge(f-imp1/face#4|f-imp1/face#8), and is ambiguous between edge(f-imp1/face#4|f-imp1/face#5), edge(f-imp1/face#4|f-imp1/face#8), edge(f-imp1/face#5|f-imp1/face#6).`
   - `examples/references/notch-replaces-line.edits.json: script taller-block: f-fil1 /edges/0 should be edge(f-ext1/end-cap|f-ext1/lateral/e-13), edge(f-ext1/end-cap|f-ext1/lateral/e-9), and is resolved to edge(f-ext1/end-cap|f-ext1/lateral/e-10), edge(f-ext1/end-cap|f-ext1/lateral/e-11), edge(f-ext1/end-cap|f-ext1/lateral/e-12), edge(f-ext1/end-cap|f-ext1/lateral/e-13), edge(f-ext1/end-cap|f-ext1/lateral/e-9).`
   - `examples/references/slot-inserted.edits.json: script deeper-second-slot: f-fil1 /edges/0 should be edge(f-ext1/end-cap#1|f-ext1/lateral/e-1), and is resolved to edge(f-ext1/end-cap#1|f-ext1/lateral/e-1), edge(f-ext1/end-cap#2#1|f-ext1/lateral/e-1).`
