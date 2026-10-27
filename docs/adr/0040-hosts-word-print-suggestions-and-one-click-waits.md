# 0040: Hosts word print suggestions from their geometry; applying one in one click waits

Status: Accepted, except the Python API of the decision (`Finding.suggestion`, `Suggestion` and `SplitPlane` in `partsmith.reports`), which is proposed, needs human review (a public API change, a human gate of the M1 gap list)

## Context

The print checks already compute suggestions: a bed fit finding that does not fit carries `Suggestion::Split` with its planes, and a resin trapped volume carries `Suggestion::DrainHole` with its point (print.md 2.6, 3.9.2, 3.9.8). The protocol carries them to every host as geometry (protocol.md `Finding.suggestion`), but no host showed them: the UI listed only the message, Python's `Finding` dropped the member, and `partsmith rebuild --checks` printed only the message (gap list item PRINT-1). The technical design (section 11) promises a bed fit check that "suggests a split plane when it doesn't fit" and trapped volumes "with drain hole suggestion".

Two questions had to be answered before code: where the words that describe a suggestion come from, and whether M1 applies a suggestion in one click (a split into bodies with `split_body`, or a drain hole).

The architecture renders every message in the core, so that one problem reads the same in every host, and hosts "depend on the engine for all behavior". A suggestion is not a message, though: it is geometry, and its words are a pure function of that geometry. ADR 0019 already removed one copy of print knowledge from the CLI so that it could not drift from the engine.

## Options for the words

1. **Each host words the suggestion from its geometry.** Three copies (Rust in `partsmith-cli`, Python, TypeScript). The CLI's copy would repeat print knowledge that `partsmith-print` owns, against ADR 0019, and need a second `len` (the CLI's `human_number` keeps 6 decimals), and print.md's examples of the words would have no test in the crate that states them.
2. **`partsmith-print` words it, the CLI prints that, and Python and the UI word it themselves in the same words (chosen).** `Suggestion::text` (print.md 2.6, 3.12) writes the words with the `len` of the finding messages, so print's own tests check the examples, and the CLI calls it on the suggestion it reaches through the protocol's `PrintReport`, with no new dependency. Python and the UI see only the protocol, so they keep their own copies. No schema, no expected result and no engine code changes. The cost is two more small implementations of the same words (Python in `partsmith.reports`, TypeScript in `ui/src/print/suggestion.ts`), kept in step by the shared cases of print.md 3.12, which print test 114, python-api.md test 106 and ui.md U-233 each cover.
3. **As option 2, with `partsmith-py` exposing `Suggestion::text` to Python.** One copy fewer: a `_core` function that takes a protocol suggestion and returns its words, with an engine or protocol re-export of `Suggestion` for `partsmith-py` to read it into. Not taken: it adds a function to the extension module's interface (python-api.md 2.2, `_core.pyi`) and a re-export to the protocol crate for one sentence, makes `print_report` call native code for each finding where it now converts plain protocol values, and leaves the UI's copy in place, so the shared cases are needed either way. Worth taking if Python's copy ever drifts or grows.
4. **The print crate renders a `label` into `Suggestion`.** One implementation, and the words reach every host by construction. It changes the protocol schema (a human gate, ADR 0016's log) and the JSON of `rebuild --checks --json`, and with it the expected results of every example whose report has a suggestion (EX-2 adds the first ones). Worth taking if a fourth host appears or the hosts' words drift.

Every host words a suggestion in millimeters, as the messages are, with the unit after every length and point (`Split at x = 150 mm`, `Add a drain hole near (5, 5, 5 mm)`). The UI could write the document's unit instead, but its print check row then mixes units: a message in millimeters (print.md open issue 7) above a suggestion in inches, and a drain hole's point with no unit would read as either. Millimeters keep a row in one unit, as the printer's numbers already are in the UI (`printerDetail`), and keep the UI's line equal to the CLI's and Python's; showing print text in the document's unit is open issue 7's, for messages and suggestions together.

## Options for one click

1. **No one-click route in M1 (chosen).** Each suggestion is worded, the focused finding frames the body with its split planes drawn through it, and the drain hole is marked at its point; the user splits with Datum plane and Split body, at the coordinates the line names. Planes along a model axis can be built with a datum plane offset from an origin plane (and, after Lay on face, the `too_tall` plane with an offset from the face on the bed). A tilted plane has no M1 datum plane (features/datum_plane.md, Later), so its line and drawing only guide the user.
2. **A UI-built transaction** of datum planes, new bodies and `split_body` features. A datum plane is only an offset from another plane (features/datum_plane.md), so only planes along a model axis could be built, which excludes every tilted plane, such as a footprint split after Lay on face or Python's `rotate`; three or more pieces, or a split along x and y, need a chain of splits into several new bodies. It puts modeling logic in a host, against AGENTS.md, and gives the CLI and Python nothing.
3. **An engine fix on the finding**, offered with `apply_fix` like a feature's one-click fixes (ADR 0024). This is the right shape, but it adds actions to print findings (a protocol schema change), needs a datum plane through a point with a normal for tilted planes, and belongs with the technical design's split with connectors. It is a large item after M1 (ui.md 1.2).

## Decision

- `partsmith-print` gains `Suggestion::text`, the words of print.md 3.12 in millimeters, None for a split without planes, which every host treats as no suggestion. The CLI prints it as a `suggested:` line under the finding; the JSON of `rebuild --checks --json` carries the geometry only, so no expected result changes.
- Python's `Finding` gains `suggestion: Suggestion | None`, a frozen dataclass with `kind`, `text`, `planes` (a tuple of `SplitPlane` with `origin` and `normal`, so a `Finding` stays hashable) and `at`, the protocol's suggestion member that gap list item PRINT-1 names. `Suggestion` and `SplitPlane` live in `partsmith.reports` and are kept out of `partsmith.__all__`, so that python-api.md 2.3, its test 77 and the names code generation reserves (the engine's `export_python`, python-api.md 3.8 rule 7) do not change.
- The UI words a suggestion in millimeters on the row's `health-suggestion` line, frames a split suggestion's finding by its box, draws each plane clipped to that box in `problem` without depth test while the finding is focused, and marks a drain hole at its point (ui.md 3.17). The marker and the planes go when another row is chosen, the panel closes, the document, the print orientation or the report changes, or the finding's check is hidden.
- Applying a suggestion in one click waits for an engine fix after M1 (option 3 of one click).

## For review

The Python bullet of the decision adds public API (python-api.md section 2), which the M1 gap list counts as a human gate. It is implemented and tested as proposed so that the review sees it working; the rest of the decision does not depend on it. `Suggestion::text` is not public API (`partsmith-print` is not published), and the CLI's line uses the `suggested:` line format that cli.md 3.3 already has for feature messages, with no JSON change. The options for Python:

- (a) As proposed (recommended): `suggestion` is the last field of `Finding` and required, like `value` and `limit` (Partsmith makes a `Finding` only in `print_report`), and `Suggestion` and `SplitPlane` stay out of `partsmith.__all__`, so python-api.md 2.3, its test 77 and the names that code generation reserves do not change.
- (b) As (a), with `suggestion: Suggestion | None = None`, so that a script that builds a `Finding` itself with the earlier fields keeps working. No other field of `Finding` has a default.
- (c) As (a), with `Suggestion` and `SplitPlane` in `partsmith.__all__`: python-api.md 2.3 and test 77 change, and the engine's `export_python` reserves two more names (python-api.md 3.8 rule 7).
- (d) No Python change in M1: `Finding` keeps dropping the suggestion, python-api.md tests 104 to 106 wait, and the technical design's promise holds only in the UI and the CLI. Taking it removes `Suggestion`, `SplitPlane` and `Finding.suggestion` from `partsmith.reports` and python-api.md 2.9 and 3.7, with tests 104 to 106.

## Consequences

- No protocol or file-format schema, license, dependency or capability changes, no change to the crate graph, and no example's expected result changes.
- The suggestion's words exist three times: Rust in `partsmith-print`, printed by the CLI; Python in `partsmith.reports`; TypeScript in `ui/src/print/suggestion.ts`. A change to print.md 3.12 changes all three and the shared cases their tests cover.
- A value exactly halfway between two thousandths can round differently in the UI (`formatNumber` rounds the shortest decimal text half away from zero, `len` the binary value half to even); the shared cases avoid such values.
- In a document in another unit than millimeters, a print check row reads in millimeters throughout, its suggestion included (print.md open issue 7).
- Planes along a model axis can be built in M1 with a datum plane offset from an origin plane (after Lay on face, the `too_tall` plane from the face on the bed); a tilted plane cannot, so for it the line and the drawing only guide the user until the engine fix.
- The drain hole's x and y follow the shell's lowest vertex with the smallest welded index (print.md 3.9.8), so for a flat-bottomed hollow they depend on the mesh's vertex order; tests accept any corner of the hollow's floor.
