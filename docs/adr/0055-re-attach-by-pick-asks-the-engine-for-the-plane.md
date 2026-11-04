# 0055: A re-attach by pick asks the engine for the plane

Status: Proposed, needs human review. It changes the protocol schema: a new command, `attach_plane`, and its result (ADR 0016's entry of 2026-11-04). The rule it builds is the owner's decision of 2026-11-04 on ADR 0023: a host's re-attach by pick follows engine.md section 3.22's rule. This ADR does not reopen that decision. The options the owner can still pick are under "Options considered"; (a) is the one wave F's reattach track specifies and builds.

## Context

- **The rule.** The faces built from a sketch inherit its motion: the motion of its stored face just before it when its fallback is in body coordinates, the identity for a fallback in world coordinates or an origin plane, and the datum's motion for a datum plane (features/sketch.md section 3.3). Later features store fallbacks and axis directions in the body coordinates of those faces. A stored plane that changes the sketch's motion moves those later features with no message. engine.md section 3.22 stores `Attach`, `TurnDrawingBack` and drag commits in the coordinates that keep the motion, and does not offer an `Attach` that no coordinates keep it for (ADR 0023, amendment 1).
- **The pick did not follow it.** The UI's re-attach by pick (ui.md section 3.16.1) kept a sketch without motion in world coordinates. For every other sketch it stored a picked face in body coordinates, as a new sketch does, and an origin or datum plane as picked. A sketch on a face that a Move body moved, re-attached to a face that moved otherwise or to an origin plane, then got another motion, and the features built on its faces moved (engine.md open issue 16, ui.md open issue 25).
- **The owner's decision.** On 2026-11-04 the owner decided that a re-attach by pick follows section 3.22's rule: a re-attach that no stored frame keeps the sketch's motion for is not offered. The other option, re-basing the fallbacks and directions of later features in the same batch, was declined.
- **What the UI can see.** Descriptors are at the display state, and a lost face has none. The UI cannot read the motion of the sketch's stored face just before the sketch. A datum's motion is computed from its frames (features/datum_plane.md section 3.2 step 4), and no message carries it. The UI can tell only that a sketch on an origin plane or with a world fallback has no motion. That leaves out every face sketch saved since format version 2 and every sketch on a datum plane. The rule needs the engine.

## Decision

1. The protocol gains command 35, `attach_plane {doc, feature, plane}`, in the Sketch group. Its result is `{plane: Option<SketchPlane>}`, with `plane` absent for none, and it records nothing and emits nothing (protocol.md section 2.5, engine.md section 3.22). A new command does not bump `PROTOCOL_VERSION` (protocol.md section 2.9). Its result carries `SketchPlane`, a closed file-format type that no result carried before (protocol.md section 3.6, test 87).
2. The engine reads the latest recompute as `solve_sketch` does. It compares the motion the sketch's stored plane gives it with the motion the given plane would give it: unchanged for an origin or datum plane with the same motion, and for a face in its own coordinates when those keep the motion, else in the other coordinates when they keep it, else none. A sketch or datum with no motion in the run (its datum failed or did not run) gets the plane unchanged. A face in world coordinates is where the face stood just before the sketch, the fallback a host sent moved by the face's motion there.
3. The UI builds the plane as for a new sketch, asks with `attachPlane` and commits the plane it returns, both in one task of its mutation queue, as one undo entry labeled "Attach '{label}'". A pick the engine gives no plane for commits nothing and shows the hint `attach_moved` in the sketch hint, and the pick stays open (ui.md sections 3.16.1 and 4.3). The `attach_after` refusal stays in the UI and comes first.
4. Re-basing later features stays declined, as the owner decided.

## Options considered

- **(a) A query command, `attach_plane` (proposed; wave F builds it).** One command and one result, no new error code. The UI keeps building the edit and its undo label, and the rule stays in one place in the engine with `Attach`.
- **(b) An applying command, `attach_sketch {doc, feature, plane}`.** The engine stores the plane itself as one undo entry and fails with a new error code when no plane keeps the motion. That saves one round trip, but the undo label, the refusal message and the `attach_after` check move into the engine, and the protocol gains an error code as well as a command.
- **(c) A field on `pick_reference`.** `Pick` gains an optional `sketch`, and the descriptor of a flat face the plane that keeps that sketch's motion. Origin and datum planes are not picked through `pick_reference`. They would need a second addition (a flag on the sketch's solution, and a motion for each datum), two schema changes for one rule.
- **No schema change.** The UI would have to refuse every pick of a sketch whose motion it cannot see (almost every re-attach of a sketch made since format version 2), or keep its old rule. The owner's decision replaced that rule.

## Consequences

- `docs/format/protocol.schema.json` gains the command and its result, and the UI's generated types gain them; `crates/partsmith-protocol` lists 35 commands. Python and the CLI have no re-attach by pick and do not send the command.
- Tests: engine.md 166 and 167, protocol.md 88 (with 34 and 87 updated), ui.md U-146, U-250 and E-165 with the fixture `reattach-moved.ptsm`. engine.md open issue 16 and ui.md open issue 25 are resolved.
- A sketch without motion re-attached to a face that a Move body after the sketch moved now gets that face's place just before the sketch. Before, the UI stored the descriptor's `placement`, the face's place at the end of the timeline.
- Motions are compared exactly, as section 3.22 compares them for `Attach`. A datum whose motion equals a face's only to rounding is refused for a sketch on that face, with the hint, and the user picks the face or the datum the sketch moves with.
- If the owner picks (b) or (c), the UI's re-attach and the tests above change with it; the engine's rule does not.
