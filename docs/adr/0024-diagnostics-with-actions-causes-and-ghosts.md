# 0024: Diagnostics carry actions, causes, details, candidates and ghosts

Status: Accepted, needs human review

## Context

The owner's second concern is errors that explain what happened, why, and what to do, with the fix one click away (the "Errors that help" direction of the guidelines). A protocol `Diagnostic` today has a severity, a code, a message, a field and one optional suggestion value; a `Repair` carries the stored reference and a centroid.

[The reference stability and errors study](../design/reference-stability-and-errors.md) measured both sides. On references, 35 of 123 corpus tests pass today and 38 silently use the wrong element; 20 more get the right outcome with a message that names no element, no cause and no candidates (study section 2.4, class C9). Of 260 user-facing messages in the inventory, 117 (45%) do not say why, 95 (37%) do not say what to do, 123 (47%) use jargon, ids or raw internal text, and only 14 (5%) offer a one-click fix. Of the 17 messages about lost references and placements, 14 give no cause and none offers a fix; every one sends the user to a "repair panel" that does not exist. The detail a bug report needs (the kernel text, the stored reference, the resolver's scores) has nowhere to go, so it either leaks into the sentence or is lost, and candidates are dropped twice: by features when a whole list fails and by the engine's projection.

"Why" has two answers. What in the model did it ("'Extrude 1', which comes before this fillet, cuts it away") follows from the document alone. Which edit started it ("This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm") only the session knows.

## Decision

The protocol additions of the study's section 5.5 (PR1 to PR6), with the engine behavior of section 5.4 (E1 to E10):

- **`Diagnostic`** gains `actions` (one-click fixes computed from the document alone, in the order to offer them), `cause` (the edit that started the problem, with its own actions), `detail` (technical text for reports, never shown as the message), `root` (recompute.md `Message::root`: the first failing feature of an `upstream_failed` chain, of a loss whose reason is `upstream_failed`, or of the feature a `body_empty` or `tool_empty` names as failed; or the sketch that lost its face, or whose face turned over, under a `no_effect`, `hole_misses_body` or `separate_solid`) and `reason` (why the reference the message names was lost, or which feature a followed or widened reference followed: the wire form of recompute's `LossReason`, with `Removal`, `Absence` and `InstanceIndex`, which the CLI prints in `--json`). Feature messages may have severity `info`; a feature with only info notices has status ok.
- **Actions name fixes, not edits.** A `FixAction` is a verb label plus a `FixKind`: `Apply` with a `FixRef` (the feature, the message's code and field, whether it belongs to the cause line, the action's index and the document revision it was computed at), `Pick`, `Show`, `Undo`, or `FinishTransaction`, the action of a `transaction_open` error, which commits the open transaction and then does again what the failed request did. The new command `apply_fix` builds the edits from the current records and applies them as one undo entry labeled with the action; it fails with the new error `stale_fix` when the document changed after the fix was offered.
- **`Cause`** holds the sentence, the changed feature, its own actions (reverts, Undo, "Keep the sketch where it was", Show) and, for a sketch edit, the changed entity's ghost. It is session data: it never enters documents, cache keys, statuses, messages or the message's actions.
- **Repairs describe and offer.** `Repair::Element` gains the lost element's description in words, its candidates (each with the reference a Use action stores, a description, a relation such as "on the top of 'Box 1', 5 mm from where it was", `meaning`, distance, a score for reports only, and the candidate's mesh element) and a `ghost` of the element from its last build; `Repair::Region` gains a description, candidate region sets and a ghost of the old outline. Ghost positions travel as binary buffers of the existing framing (ADR 0006). Hosts never show a score as a match percentage.
- **References in element data.** `list_elements` sends short references (query and fingerprint), `pick_reference` the full reference with anchors and merged names (ADR 0021); `ElementDescriptor` gains `fallback` and `axis_direction` in body coordinates (ADR 0023). `Pick` gains an optional `body`, which headless picks set, since they name the element by its position among the elements of one body (engine.md section 3.9).
- **Sketch messages.** `SketchSolution` gains `messages`: the sketch's solve issues and profile findings rendered by the sketch crate, each a `Diagnostic` with its actions ("Connect the ends (0.3 mm apart)", whose `FixRef` names the finding by its position, `/profile_findings/0`), so hosts show the core's sentences and fixes for sketches as they do for features, and the dimension editor can show the sentence of a value it probes.
- **Prevention and units.** New commands `list_uses` (what each feature uses, from the document alone, for the timeline drop warning, "Used by" and used marks in sketches) and `set_document_units` with `keep_sizes`; `DocumentInfo` gains `bare_numbers`; `ErrorInfo` gains `detail` and `actions`.
- **Messages.** Every text follows [the error guidelines](../guidelines/errors.md), and a message lint runs over every rendered template.
- All additions are optional fields, new types or new commands, so `PROTOCOL_VERSION` stays 1.

## Alternatives considered

- **Send each fix's edits inline in the diagnostic.** An edit computed before another change would overwrite it when applied, and full references would inflate every status. With a `FixRef` the engine builds the edits from the current records, and a stale fix is refused instead of applied.
- **Put the session's history into the message and its actions.** The same document would then read differently in the UI, in `rebuild --json` and in Python, and cached feature errors would depend on the session. A separate `cause` keeps the message and its actions a function of the document.
- **Keep one `suggestion` value per message.** It covers 14 of 260 messages; candidates, timeline fixes (Unsuppress, Move below), sketch fixes (Connect the ends, Delete a constraint) and reverts need labels and kinds.
- **Show fingerprint scores next to candidates.** A 100% lookalike is exactly what made FreeCAD's breakage silent (R21b's cove scores 1.0). Candidates are ordered and described by what they keep and how far they are; scores stay in the data for reports.
- **Let the UI compute candidates and ghosts itself.** The UI depends only on the protocol (AGENTS.md invariant), and the CLI and Python need the same data.

## Consequences

- `docs/format/protocol.schema.json` changes. It is regenerated with `partsmith schema --write` when the code lands and each check-in is logged in ADR 0016; the UI regenerates its TypeScript types.
- The review of the spec text added four protocol items beyond the study's PR1 to PR6, listed in the Decision above so this review covers them: the action kind `finish_transaction`, `Diagnostic::reason`, `SketchSolution::messages` and `Pick::body` (protocol.md open issues 3 to 5, engine.md open issue 10).
- Every host can show the same message with the same actions: the UI as buttons, repair mode and a notification for every new loss; the CLI as `suggested:` and `candidate:` lines and in `--json`; Python as exception attributes.
- Ghosts need the kernel's per-element display geometry (ADR 0026). The engine captures them once, when a reference goes from resolved to lost, and keeps them in session memory; headless sessions and documents opened fresh have none, and hosts draw a marker at the centroid instead.
- New engine budgets: `apply_fix` as `apply_edits`, `list_uses`, ghost capture, and a parameter change that loses 5 references with diagnosis and causes included.
- This changes the protocol schema, so it waits for human review at the next gate. With ADRs 0021 and 0025 it gates all code of the study's plan; `actions`, `detail` and `root` ship in phase 1, `cause` and ghosts are filled in phase 4.
