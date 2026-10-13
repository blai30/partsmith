# Newcomer error corpus

This corpus is the test plan for the "Errors that help" promise of [the guidelines](../guidelines/README.md) on the errors a first-week user hits. [The reference stability corpus](reference-stability-corpus.md) covers references that break after an edit; this one covers the everyday mistakes of someone learning to model: an outline that does not close, a sketch on the wrong plane, a fillet or a shell that is too large, a sketch that cannot hold its constraints, a unit switch, a typo. Each scenario is an exact model, the mistake, and the exact message, actions and result of the fix the user must get.

The scenarios come from [the reference stability and errors study](reference-stability-and-errors.md) section 7 (decision D6, accepted with ADRs 0021 to 0026 and pending human review at the next gate). The messages follow [the error guidelines](../guidelines/errors.md), whose examples are taken from this corpus.

## How to read a scenario

- **Setup** is the timeline before the mistake. Sizes are millimeters, angles degrees. A reference is written `{q: <element name>, fp: <fingerprint>}` and is re-picked at setup time as a pick stores it, as in the reference stability corpus ("Picked references").
- **Edit**, when present, is the change the user makes after the setup. Without an edit, the mistake is in the setup itself.
- **Expect** is the assertion after a full recompute. Codes and messages are exact; actions are given in brackets, in the order offered, with the labels of the protocol's `FixAction` (engine.md section 3.22). "ok" means status ok with no warnings; volumes match within 1e-6 relative and positions within 1e-6 mm.
- **Cause line** is the engine's line under the message ("This started after you ..."), with its own actions. Only the engine and the hosts know it.
- **Then** is the result of applying the first action with `apply_fix`, one undo entry; parentheses name another action when a scenario checks that one instead.
- **Labels** follow features.md section 3.12 in timeline order. Bodies `b-1` and `b-2` are named 'Body 1' and 'Body 2'.
- **Today** is what the current specs give, as of 2026-10-12, from the study's message inventory.

## Shared setups

- **BLOCK** (the reference stability corpus, naming.md test 81): bodies `[b-1]`. `f-sk1` on XY with lines `e-1` (0,0)-(80,0), `e-2` (80,0)-(80,50), `e-3` (80,50)-(0,50), `e-4` (0,50)-(0,0). `f-ext1` extrude of region `{key: e-1,e-2,e-3,e-4, fp: c=40,25|a=4000}`, distance 30, op new: x 0..80, y 0..50, z 0..30, volume 120000. Faces: `f-ext1/lateral/e-1` (front, y=0), `f-ext1/lateral/e-2` (right, x=80), `f-ext1/lateral/e-3` (back, y=50), `f-ext1/lateral/e-4` (left, x=0), `f-ext1/start-cap` (z=0), `f-ext1/end-cap` (z=30).

Setups use BLOCK unless they say otherwise.

## Rules the assertions encode

1. Every message says what happened, why, and what to do, in at most two sentences, naming features, bodies, fields, values, sketch entities and constraints the way the user sees them. No ids, codes, element names, scores or kernel text appear in the sentence, and the message lint of the error guidelines passes.
2. A fix is offered only when it works or is exact. "Turn on Reverse" appears only when the reversed shape reaches the body (NC03 at distance 5 and NC12 offer none), and a limit only when it is a useful value. Applying an offered fix gives the stated result in one undo entry.
3. A cause is stated only when the code established it: the limiting face is named because it was measured (NC05), and round faces smaller than the wall are stated as a fact, not blamed (features/shell.md test 16).
4. One root cause gives one problem: downstream features name the root and offer Show, never Pick (NC07, NC11).
5. The message and its actions are the same in every host and in a document opened fresh. The cause line is the only part that depends on the session.

## Where the assertions run

- **Recompute** (`crates/partsmith-recompute/tests/newcomer_errors.rs`): every scenario's codes, structural messages and fixes from the document alone.
- **Engine** (`crates/partsmith-engine/tests/newcomer_errors.rs`): every scenario through the protocol, with the edit applied by `apply_edits` and the fix by `apply_fix`, checking the action labels, the cause lines of NC08 and NC11, and the result after the fix; NC09 through `set_document_units`.
- **Playwright** against the real core and UI: E-110 to E-121 (NC01 to NC12), including the Extrude disabled reason of NC01 and the units control of NC09.
- **Crate tests** where the study puts them: features document tests for NC02 to NC06 and NC12, fillet and shell spec tests for NC05 and NC06, the sketch crate for the findings, issues and fixes of NC01, NC07 and NC08 (sketch.md tests 237, 243 and 251; a failed sketch is diagnosed where it was given, so NC08 is `sketch_not_solved` whatever the iterations did), the expression evaluator for NC10, and Python for NC09 (`Document.set_units(..., keep_sizes=True)`).

All twelve pass in phase 3 of the study's plan except the cause lines of NC08 and NC11, which pass in phase 4.

## The five errors newcomers hit most

| Error | Scenarios | Today | After |
| --- | --- | --- | --- |
| 1. Open outline, so Extrude has nothing to use | NC01 | Extrude's disabled reason says only "Draw a closed shape in a sketch first."; findings name entities by id; the only way out is back to the sketch | The disabled reason names the sketch and its open ends with Show; findings name entities by kind and coordinates; "Connect the ends (0.3 mm apart)" closes a small gap in one click |
| 2. Sketch on the wrong plane: a join buried inside the body, a cut that misses, a join that floats | NC02, NC03, NC04, NC12 | "This feature does not change body 'Body 1'." or nothing; "turn on Reverse" also when reversing misses too | `no_effect` says where the tool is and by how much; Turn on Reverse only when it works; "Attach 'Sketch 2' to the top of 'Extrude 1'"; a floating join warns that the body now has 2 pieces |
| 3. Fillet or chamfer too large | NC05 | The limit and "Try 2.3 mm or less.", but not which edge or face limits it | The limiting face is named with its measured width; a limit too small to be useful is not suggested; "Remove that edge from Edges" when the other edges work |
| 4. Constraint conflict, or a dimension value the sketch cannot take | NC07, NC08 | Constraints named by id; nothing names the change of a value | The conflicting constraints are named by title and value with Delete and Make a reference actions; a value the sketch cannot take names the change and offers its revert and Undo |
| 5. Shell too thick for the wall the print needs | NC06 | "Try 0.99 mm or less." only | Prevented: the shell builds, the rounds of a fillet thinner than the wall closing into sharp inner corners as in other CAD tools (kernel.md section 3.4.8 item 6). A wall truly too thick names the round faces only when they may be what stops it (features.md section 4.3) |

## Index

| Id | Error | Code | Title | Playwright |
| --- | --- | --- | --- | --- |
| NC01 | 1 | finding, disabled reason | Open outline | E-110 |
| NC02 | 2 | `no_effect` | Join buried in the body | E-111 |
| NC03 | 2 | `no_effect` | Cut that points away | E-112 |
| NC04 | 2 | `separate_solid` | Floating join | E-113 |
| NC05 | 3 | `fillet_too_large` | Fillet too large | E-114 |
| NC06 | 5 | none (ok) | 2 mm walls after a 1 mm fillet | E-115 |
| NC07 | 4 | `sketch_conflict` | Constraint conflict | E-116 |
| NC08 | 4 | `sketch_not_solved` | A dimension value the sketch cannot take | E-117 |
| NC09 | | units control | Units switch with bare numbers | E-118 |
| NC10 | | `expression` | Misspelled parameter | E-119 |
| NC11 | | `profile_not_found`, `body_empty` | A deleted sketch line | E-120 |
| NC12 | 2 | `hole_misses_body` | Hole beside the body | E-121 |

## Scenarios

### NC01 Open outline

- **Setup:** bodies `[b-1]`. `f-sk1` on XY with lines `e-1` (0,0)-(80,0), `e-2` (80,0)-(80,50), `e-3` (80,50)-(0,50), `e-4` (0,50)-(0,0.3): BLOCK's outline with a 0.3 mm gap at the origin. No feature uses the sketch yet.
- **Expect:** `f-sk1` has no region. The UI's Extrude disabled reason reads `'Sketch 1' has no closed shape: 2 ends are not connected.` [Show 'Sketch 1']. `f-sk1` has one profile finding, `The end of the line from (0, 50) to (0, 0.3) at (0, 0.3) is not connected to other geometry.` [Connect the ends (0.3 mm apart)]: the two open ends are one finding, because one fix joins them.
- **Then:** `f-sk1` has one region, and Extrude is enabled.
- **Today:** Extrude's disabled reason says only "Draw a closed shape in a sketch first.", the findings name entities by id, and nothing closes the gap.

### NC02 Join buried in the body (sketch on the wrong plane)

- **Setup:** BLOCK. `f-sk2` on XY with circle `e-5` center (40,25) radius 5; `f-ext2` extrude of it, distance 10, op join (the tool spans z 0..10, inside the block).
- **Expect:** `f-ext2` warning `no_effect`: `This extrude adds nothing to body 'Body 1': it lies entirely inside the body.` [Attach 'Sketch 2' to the top of 'Extrude 1']. Volume 120000.
- **Then:** the boss stands at z 30..40, volume 120785.398163, status ok.
- **Today:** `This feature does not change body 'Body 1'. Check its position, size and direction.`, which says neither where the tool is nor how to fix it.

### NC03 Cut that points away

- **Setup:** BLOCK. `f-dp1` datum XY offset 40. `f-sk2` on `f-dp1` with circle `e-5` center (40,25) radius 5; `f-ext2` extrude of it, distance 15, op cut (the tool spans z 40..55, above the block).
- **Expect:** `f-ext2` warning `no_effect`: `This cut does not touch body 'Body 1': it starts 10 mm from the body and points away from it.` [Turn on Reverse].
- **Then:** a pocket z 25..30, volume 119607.300918. With distance 5 instead, the same message without the Reverse action (reversed, the tool spans z 35..40 and still misses), and the sentence does not change.
- **Today:** the generic `no_effect` text of NC02, and the UI offers Reverse for every `no_effect`, also at distance 5, where reversing misses too.

### NC04 Floating join

- **Setup:** as NC03 with op join (the tool floats at z 40..55).
- **Expect:** `f-ext2` warning `separate_solid`: `This extrude does not touch body 'Body 1', so the body now has 2 separate pieces.` [Put it in a new body] [Attach 'Sketch 2' to the top of 'Extrude 1'].
- **Then (Attach, the second action):** one solid, the boss at z 30..45, volume 121178.097245.
- **Today:** no message: a disjoint join is ok and the body silently has 2 solids (features.md section 6.4, body semantics test 7).

### NC05 Fillet too large

- **Setup:** bodies `[b-1]`. `f-box1` box 40 x 20 x 10 (x -20..20, y -10..10, z 0..10). `f-box2` box 20 x 30 x 5 at position (10, 0, 7.6), op cut: a step 2.4 mm deep over x 0..20, whose riser at x=0 is `f-box2/left`. `f-fil1` fillet radius 3 on `[edge(f-box1/top|f-box2/left)]`, the edge between the top of 'Box 1' and the riser.
- **Expect:** `f-fil1` error `fillet_too_large`: `Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm. The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across.` [Use 2.3 mm].
- **Then:** ok.
- **Today:** `Fillet radius 3 mm is too large for the selected edges. The largest radius that works is about 2.3 mm. Try 2.3 mm or less.`, which does not say which face limits the radius.

### NC06 2 mm walls after a 1 mm fillet

- **Setup:** bodies `[b-1]`. `f-box1` box 40 x 40 x 20. `f-fil1` fillet radius 1 on the four vertical edges `edge(f-box1/front|f-box1/left)`, `edge(f-box1/front|f-box1/right)`, `edge(f-box1/back|f-box1/left)`, `edge(f-box1/back|f-box1/right)`. `f-sh1` shell thickness 2 removing `f-box1/top`.
- **Expect:** every feature ok. The rounds' inner offsets vanish and the 2 mm walls meet in sharp inner corners (kernel.md section 3.4.8 item 6, ui.md open issue 17); volume 8592 + 20π = 8654.831853, 15 faces, no face named `f-sh1/inner/f-fil1/...`.
- **Today:** `Shell thickness 2 mm is too thick for body 'Body 1'. The thickest wall that works is about 0.99 mm. Try 0.99 mm or less.`, whose only way out is a wall too thin to print: OCCT's offset could not take a convex round past its radius, so any wall at least as thick as the fillet failed. The study's first answer named the rounds and offered "Set Radius of 'Fillet 1' to 3 mm" and "Move 'Shell 1' above 'Fillet 1'"; the kernel fix makes the error itself go away (features.md open issue 17).

### NC07 Constraint conflict

- **Setup:** bodies `[b-1]`. `f-sk1` on XY with lines `e-1` (0,0)-(40,0), `e-2` (40,0)-(40,20), `e-3` (40,20)-(0,20), `e-4` (0,20)-(0,0) and constraint `c-1` Distance "40 mm" on `e-1`. `f-ext1` extrude of its region, distance 10, op new.
- **Edit:** add constraint `c-2` Distance "30 mm" on `e-1`.
- **Expect:** `f-sk1` error `sketch_conflict`: `In 'Sketch 1', Distance 30 mm cannot hold together with Distance 40 mm.` [Delete Distance 30 mm] [Make Distance 30 mm a reference] [Delete Distance 40 mm] [Make Distance 40 mm a reference]. `f-ext1` error `upstream_failed` with root 'Sketch 1': `'Sketch 1' has an error, so this extrude cannot use it. Fix 'Sketch 1' first.` [Show 'Sketch 1'].
- **Then:** ok, volume 8000.
- **Today:** `Constraints {ids} conflict: they cannot all be satisfied at once. Delete one of them or change a dimension.` with the ids `c-1` and `c-2`, and no action.

### NC08 A dimension value the sketch cannot take

- **Setup:** bodies `[b-1]`. `f-sk1` on XY with the triangle `e-1` (0,0)-(30,0), `e-2` (30,0)-(30,40), `e-3` (30,40)-(0,0), coincident constraints joining its three corners, and dimensions `c-1` Distance "30 mm" on `e-1`, `c-2` Distance "40 mm" on `e-2` and `c-3` Distance "50 mm" on `e-3`.
- **Edit:** set `c-3` to "80 mm" (no triangle has sides 30, 40 and 80).
- **Expect:** `f-sk1` error `sketch_not_solved`: `'Sketch 1' could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape.` Cause line: `This started after you changed Distance of 'Sketch 1' from 50 mm to 80 mm.` [Set Distance of 'Sketch 1' back to 50 mm] [Undo]. The dimension editor shows the same sentence when 80 mm is typed and offers "Add reference" (ui.md section 3.16.7). Since the editor refuses a value the sketch cannot take before anything is stored, the Playwright test E-117 opens a document that already holds 80 mm (ui.md open issue 24).
- **Then (the revert):** ok.
- **Today:** `The sketch could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape and try again.`, and nothing names the change that caused it.

### NC09 Units switch with bare numbers

- **Setup:** document units mm. Bodies `[b-1]`. `f-box1` box size `["40", "20", "10"]`, op new. `f-fil1` fillet radius `"2"` on `[edge(f-box1/front|f-box1/top)]`.
- **Edit:** switch the document units to in.
- **Expect:** before the switch, `bare_numbers` is 4, and the units control offers `Keep sizes: add mm to the 4 numbers without a unit`, on by default. Switching runs `set_document_units(in, keep_sizes: true)` as one undo entry labeled `Change units to in`, which writes `"40 mm"`, `"20 mm"`, `"10 mm"` and `"2 mm"` and leaves the volume unchanged. Without `keep_sizes`, the four numbers mean inches (the box is 1016 x 508 x 254 mm and the radius 50.8 mm), and a fillet with a bare number that then fails gets the sentence of reference stability scenario R48: `'2' has no unit, so it means 2 in in this document.` [Use 2 mm].
- **Today:** there is no Keep sizes option, and every bare number silently means inches.

### NC10 Misspelled parameter

- **Setup:** BLOCK with parameter `height` = "30 mm" and `f-ext1` distance `"hieght"`.
- **Expect:** `f-ext1` error `expression`: `Distance: 'hieght' is not a parameter. Did you mean 'height'?` [Replace hieght with height]. An unknown name with exactly one parameter within edit distance 2 gets this text and fix (expressions.md); otherwise the text stays `Unknown name 'hieght'. Check the spelling, or add a parameter named hieght.`
- **Then:** ok, volume 120000.
- **Today:** `Unknown name 'hieght'. Check the spelling, or add a parameter named hieght.`, with no fix.

### NC11 A deleted sketch line

- **Setup:** BLOCK. `f-fil1` fillet radius 2 on `[{q: edge(f-ext1/end-cap|f-ext1/lateral/e-3), fp: line|n=1,0,0|c=40,50,30|l=80|adj=2,2}]` (the back top edge).
- **Edit:** delete `e-3` (the back line) from `f-sk1`.
- **Expect:** `f-ext1` error `profile_not_found`: `The region of 'Sketch 1' selected for Profile is gone because part of its outline was deleted.` [Show 'Sketch 1'] [Pick a region again]. `f-fil1` error `body_empty`: `Body 'Body 1' is empty here because 'Extrude 1', which makes it, failed. Fix 'Extrude 1' first.` [Show 'Extrude 1']. One notification names 'Extrude 1', with the cause line `This started after you deleted the line from (80, 50) to (0, 50) in 'Sketch 1'.` [Undo] [Show the line] (engine.md section 3.21: a sketch entity change has no revert, so Show follows Undo).
- **Today:** `None of the selected regions of 'Sketch 1' can be found anymore. Re-pick the profile.`, and the fillet says `Body 'Body 1' is empty at this point, so this feature has nothing to work on. Pick a body that has a solid.`, which names no cause and sends the user to the wrong feature.

### NC12 Hole beside the body

- **Setup:** BLOCK. Face sketch `f-sk2` on `{q: f-ext1/end-cap, fp: plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4}`, fallback origin (0,0,30), x_axis (1,0,0), normal (0,0,1), `fallback_frame: body`, with point `e-p1` (95,25). `f-hole1` hole on `b-1`, sketch `f-sk2`, points `[e-p1]`, diameter 5, through all.
- **Expect:** `f-hole1` warning `hole_misses_body`: `The hole at (95, 25) in 'Sketch 2' does not touch body 'Body 1'. Move the point onto the body.`, and no Reverse action (reversed, the hole misses too). Volume 120000.
- **Today:** `The hole at point 'e-p1' does not touch body 'Body 1'. Move the point, or turn on Reverse.`, which names the point by its id and suggests a Reverse that misses too.
