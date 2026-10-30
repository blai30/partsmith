# 0051: The examples runner checks what the corpus covers, and a Vitest replays the drawing tools against their examples

Status: Accepted. No capability, schema, license or dependency changes: the runner reads the engine's registry, schemas and printer profiles, which the CLI's tests already reach through `partsmith-engine`, and the Vitest reads files through the UI's existing `src/test/project-file.ts`.

## Context

- **Technical design section 16** asks the example suite to cover "every feature and option: each sketch tool and constraint, each feature mode, each import and export path, each printability check, FDM and resin", and asks CI to fail when a registered feature has no example. Runner test 3 checked the feature types, runner test 5 the export paths (ADR 0046); nothing checked the rest, so a gap stayed open until a person noticed it (gap list EX-9).
- **Measured on 2026-10-30** with scratch scripts over the corpus of 256 examples and with the current debug CLI and Python bindings:
  - every sketch constraint type and entity type was in an example outside `examples/bench/`, but four record forms of the `.ptsm` schema were in none: the points forms of `horizontal` and `vertical` and the line forms of `horizontal_distance` and `vertical_distance`;
  - every option value of every manifest was written by a record or was the default a record leaves out (`op` `join` of the copies, `hole_type` `simple`, a primitive's `plane` on the origin), and every plane or axis reference kind was in some record of its type, but `second_direction.axis` of `linear_pattern` was only ever `origin`;
  - on `generic-fdm` every check had a finding outside the benchmark; on `generic-resin`, `mesh_validity`, `first_layer_contact`, `islands` and `small_features` had none;
  - both committed assets were imported by an example of their folder;
  - a scratch replay of features/sketch.md tests 44 to 50 through `drawClick` (run with the UI's `jiti`) matched all seven `examples/sketch/tool-*.ptsm`, constraints exactly and coordinates within 1e-9 mm.

## Decision

1. **Four runner tests** in `crates/partsmith-cli/tests/examples.rs` (delivery.md section 2.8, "Corpus coverage"), reading files only: 6, every record form of a sketch constraint and entity in the `.ptsm` schema has a record; 7, every option value and plane or axis reference kind of every manifest field has a record of that type in an example under `examples/<type>/`, where runner test 3 looks for the type; 8, every check of the protocol schema has a finding, of any severity, on each technology of the protocol schema; 9, every file in an `assets/` folder is imported by an example of that folder. Their lists come from the engine (the registry, `ptsm_schema()`, `protocol_schema()`, `printer_profiles()`), so a new form, option, kind, check or technology is checked as soon as the code has it.
2. **Per field and per form.** A reference kind counts for the field that holds it, and a constraint counts for its record form, not only its type: the second direction resolves through the same code as the first, but it is its own field of the file, the inspector and the Python form, and EX-3 itself asked for the distance forms by name.
3. **Defaults count where a record leaves them out.** Canonical form never writes a default, so a record that leaves out a field holds its default. The manifests do not carry defaults, so the runner lists the nine (type, field) pairs that have one (delivery.md 2.8), and test 76 holds the list to the registry's canonical form and to the `.ptsm` schema: the option, plane and axis hints whose field is optional and not nullable there are exactly those nine, so a new such field with a default fails test 76 until it is listed.
4. **The benchmark counts for nothing** in tests 6 to 8: it is a performance document, not documentation, and EX-3 found constraints that only it had.
5. **An info finding counts** for a check: an enclosed hollow on FDM is only ever an info (print.md section 3.9.8).
6. **Gaps are closed with examples, not exceptions.** Seven examples, each with its twin, blessed expected results and a test of its spec: `sketch/point-and-line-forms` (features/sketch.md test 51), `linear_pattern/second-sketch-line-direction`, `second-edge-direction` and `second-face-direction` (features/linear_pattern.md tests 22 to 24), and `boolean/island-resin`, `extrude/wedge-on-edge-resin` and `extrude/corners-touching-resin` (print.md tests 125 to 127). Every face of the resin examples is planar.
7. **The drawing tools are replayed** by ui.md's U-247, a Vitest that clicks through `drawClick` with each click's solution taken from the entities drawn so far, and compares what the replay adds (each commit's change and, in test 46, the Fix tool's constraint, which `constraintFor` makes outside a commit) with each `examples/sketch/tool-*.ptsm`, constraints exactly and numbers within 1e-9 mm. The inferred verticals that the core's probe leaves out of tests 48 and 49 are named by the test, which checks that they are inferred. The examples were kept in step by hand before (ui.md 3.16.4).

## Options considered

- **Reference kinds per type instead of per field** (not taken): no new example, but a second direction from an edge, a round face or a sketch line would stay untested, and the rule would need its own explanation.
- **Constraint types only** (not taken): what the gap list's line says, but it lets the points and line forms the corpus lacked stay out; the schema's alternatives give the forms with the same reading.
- **Defaults from the schema** (not taken): it would need a `default` in the params schemas of `op`, `hole_type` and `plane`, a change to the file-format and protocol schemas that the UI's starting values read.
- **An exception list** (not taken): the measured gaps were small enough to close with examples; delivery.md 2.8 says an exception would be listed with its reason.
- **Point roles, reserved references, finding codes, the benchmark's options and the boolean mode flags** (not taken, delivery.md open issue 20): each would be a further runner test; the hole examples already use all four point roles, and every flag is `true` in an example of its type.
- **Records of a type in any folder for runner test 7** (not taken): it found the same gaps on 2026-10-30, but runner test 3 and the message of test 7 point at `examples/<type>/`, where a reader of the feature looks.
- **The replay with a solver** (not taken): the Vitest has no core, and each example comes back from the solver as stored, so the drawn positions are the solved ones within the solver's tolerance; a probe that leaves out a redundant inferred constraint is the core's, which the test names instead of running.

## Consequences

- delivery.md section 2.8 (runner tests 6 to 10, "Corpus coverage", "Coverage examples", the bless note), sections 3.7 and 4, tests 74 to 78, and open issues 4 and 20; features/sketch.md test 51 and the intro of tests 34 to 51; features/linear_pattern.md tests 22 to 24; print.md section 6.18 and tests 125 to 127; ui.md section 3.16.4 and U-247.
- The code is in `crates/partsmith-cli/tests/examples.rs`, `crates/partsmith-cli/tests/print_examples.rs`, the feature tests of `partsmith-recompute` and `ui/src/sketch/tool-examples.test.ts`, with the doc comment of `readProjectFile` in `ui/src/test/project-file.ts` saying that a path starting with `/..` reaches the repository; no production code changes unless a check finds a defect.
- A change to a drawing tool's output, a new constraint form, option, reference kind, check or technology now fails CI until an example shows it.
