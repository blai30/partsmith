# Reference stability corpus

This corpus is the test plan for two promises in [the guidelines](../guidelines/README.md): references that survive edits, and errors that help. In FreeCAD a fillet or a sketch stores a number such as Edge7 or Face6, and the slightest change to an earlier step can make that number point at another element, often with no message at all. Partsmith stores a semantic name plus a geometric fingerprint ([naming.md](../spec/naming.md)), but a name scheme is only as good as the edits it has been tested against. Each scenario below is one such edit: an exact model, one reference in it, one change made earlier in the timeline, and the exact result and message the user must get.

The scenarios were merged from 113 candidates written against [technical design](technical-design.md) section 5, naming.md, [features.md](../spec/features.md) sections 3.6 to 3.8 and 4, and the feature specs. Ids are in priority order: R01 to R48 are high, R49 to R64 medium, R65 low. Within a priority they follow the category order of this document.

## How to read a scenario

- **Setup** is the timeline before the edit. Sizes are millimeters, angles degrees. A reference is written `{q: <element name>, fp: <fingerprint>}` as stored in the document.
- **Reference** names the field that holds the reference under test and what the user picked, in words.
- **Edit** is the change the user makes. "Insert before X" puts the new features directly above X in the timeline. A scenario with variants (a), (b) is one test per variant.
- **Expect** is the assertion after a full recompute. "ok" means status ok with no warnings; volumes match within 1e-6 relative, positions within 1e-6 mm, and "about" means within 0.05 mm. A fillet is located by its axis (a radius r fillet on an edge between two perpendicular faces has its axis r in from each face).
- **Messages.** Labels follow features.md section 3.12 in timeline order, so an inserted feature can get a lower number than its id suggests; the scenarios give the rendered labels. Bodies `b-1` and `b-2` are named 'Body 1' and 'Body 2'. "The message names X" means the rendered text contains X in words; it never contains an element name such as `edge(f-box1/front|f-box1/top)`, an id or raw kernel text. A "one-click fix" is an action the host applies in one step: a suggestion, a candidate in the repair data, or a timeline action such as Unsuppress or Move below. A "notice" is a non-blocking message on a feature that built correctly.
- **Today** is what the current specs and resolver arithmetic (naming.md section 3.9, `CONFIDENCE_THRESHOLD` 0.85, `AMBIGUITY_MARGIN` 0.05) give, as of 2026-10-12. "Guard" marks a scenario that passes today and must keep passing.
- Messages, notices, causes and actions that M1 does not have yet are specified in naming.md, features.md or the feature spec before code is written (the project's spec-first process). The corpus fixes the outcome, not the mechanism.

## Shared setups

- **BLOCK** (naming.md test 81): bodies `[b-1]`. `f-sk1` on XY with lines `e-1` (0,0)-(80,0), `e-2` (80,0)-(80,50), `e-3` (80,50)-(0,50), `e-4` (0,50)-(0,0). `f-ext1` extrude of region `{key: e-1,e-2,e-3,e-4, fp: c=40,25|a=4000}`, distance 30, op new: x 0..80, y 0..50, z 0..30, volume 120000. Faces: `f-ext1/lateral/e-1` (front, y=0), `f-ext1/lateral/e-2` (right, x=80), `f-ext1/lateral/e-3` (back, y=50), `f-ext1/lateral/e-4` (left, x=0), `f-ext1/start-cap` (z=0), `f-ext1/end-cap` (z=30, fingerprint `plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4`).
- **SLOT(a,b)** (naming.md test 84), on BLOCK: `f-dp1` datum XY offset 30; `f-sk2` on `f-dp1` with a rectangle x a..b, y -1..51; `f-ext2` extrude of it, distance 5, reverse, op cut. SLOT(30,50) splits the top into `f-ext1/end-cap#1` (x 0..30) and `f-ext1/end-cap#2` (x 50..80).
- **CUBE20**: bodies `[b-1]`. `f-box1` box 20 x 20 x 20, op new: x -10..10, y -10..10, z 0..20.
- **CUBE10**: bodies `[b-1]`. `f-box1` box 10 x 10 x 10, op new: x -5..5, y -5..5, z 0..10.
- **BOX-BELOW**: bodies `[b-1]`. `f-box1` box 40 x 40 x 10 at position (0,0,-10), op new: x -20..20, y -20..20, z -10..0 (top at z=0).
- **Face sketch**: a sketch whose plane is `{kind: face, body: b-1, face: <reference>, fallback: {origin, x_axis, normal}}`; the fallback is written after the reference.

## Rules the assertions encode

1. A name that is not order dependent wins over a stale fingerprint. Fingerprints are recorded when the user picks and go stale with every later edit.
2. An order-dependent name (one with `#n`, including imported `face#n`) is confirmed by meaning: which piece it is, what it touches, which generator made it. Distance to a fingerprint recorded before the edit is not enough, and neither is a rigid move.
3. A split element resolves to all its pieces, a merged one through its aliases, and a history descendant (a shell rim made from the face it replaced) counts as history. Names nested inside generated names follow the same rules.
4. When history says the element is gone, no other element is substituted silently. The feature reports the loss, or, where a scenario says "follows with a notice", applies the replacement and says so with keep and re-pick actions.
5. Every loss names what was lost in words, which change removed it (the feature and the edit), and what to do, with candidates as one-click replacements. One root cause gives one message; features downstream point at it.
6. The outcome never depends on a typed number crossing a hidden threshold. Scenarios list the sizes that must behave alike.
7. Reverting an edit restores the original result and a byte-identical document. References are never rewritten without the user seeing it.

## Index

| Id | Priority | Today | Title |
| --- | --- | --- | --- |
| R01 | high | wrong result | Corner chamfer line added to the base outline |
| R02 | high | wrong result | Notch added to the base outline |
| R03 | high | wrong result | Corner bevel removed from the outline |
| R04 | high | wrong result | Circle drawn across the outline edge |
| R05 | high | wrong result | Two profile rectangles overlap after a dimension change |
| R06 | high | wrong result | Side line deleted and redrawn |
| R07 | high | wrong result | A follow by shape must not decay on the next edit |
| R08 | high | wrong result | Widening the part must not move a fillet to the other piece of a split top |
| R09 | high | wrong result | Moving a slot so the pieces swap sizes must not move the shell opening |
| R10 | high | wrong result | A split top becomes one face again |
| R11 | high | wrong result | A notch moved further than its width keeps the chamfer on its side |
| R12 | high | wrong result | A pocket that becomes a through hole must not move the floor fillet |
| R13 | high | unclear message | Extruded block made flush with the base |
| R14 | high | guard | The second fillet keeps the first fillet's arc when the first radius changes |
| R15 | high | guard | Dress-ups inserted upstream keep the names they modify |
| R16 | high | unclear message | A cut that removes the filleted edge |
| R17 | high | wrong result | A second slot inserted upstream splits references by meaning |
| R18 | high | unclear message | A dress-up inserted before another dress-up on the same edge |
| R19 | high | wrong result | A shell inserted upstream keeps a fillet on the outer top edge |
| R20 | high | wrong result | A slot inserted before a pattern splits references on the copy |
| R21 | high | wrong result | A box joined against a filleted side |
| R22 | high | wrong result | A two-distance chamfer keeps its sides when a join renames the top |
| R23 | high | wrong result | Suppressing one peg must not move its fillet to the other peg |
| R24 | high | wrong result | A slot-rim fillet when the slot is suppressed or the fillet dragged above it |
| R25 | high | unclear message | Suppressing a chamfer that carries a datum |
| R26 | high | wrong result | Deleting and recreating the base extrude |
| R27 | high | wrong result | Removing an upstream move must not hand a reference to the sibling piece |
| R28 | high | unclear message | A fillet dragged above the feature that makes its edge |
| R29 | high | wrong result | Swapping two independent slot cuts must not widen a reference |
| R30 | high | wrong result | A join changed to a cut must not re-target its rim fillet |
| R31 | high | wrong result | A split that moves the referenced edge to the other body |
| R32 | high | wrong result | A sketch keeps its place on a face that moves or turns over |
| R33 | high | wrong result | A sketch whose face is cut away must not jump to its attach-time position |
| R34 | high | wrong result | A shell inserted under a sketch must not drop it onto the floor |
| R35 | high | guard | A slot splits the face a sketch is on |
| R36 | high | guard | A sketch on the inside floor follows a shell thickness change |
| R37 | high | wrong result | A datum on a chamfer face survives an upstream merge |
| R38 | high | wrong result | A pattern direction from an edge must not flip after a 1 mm tweak |
| R39 | high | guard | Axes on edges and faces that an upstream edit splits or shortens |
| R40 | high | guard | Pattern count or directions change: references stay on their copy |
| R41 | high | unclear message | Lowering a pattern count says which copy is gone |
| R42 | high | guard | Hole points added, reordered or moved |
| R43 | high | wrong result | A hole point removed: the chamfer must not jump to the neighbor |
| R44 | high | guard | Fillets inserted before a shell |
| R45 | high | guard | An imported file revised with an extra hole |
| R46 | high | wrong result | An imported file revised without the referenced hole |
| R47 | high | wrong result | An upstream sketch conflict must not send a fillet to a lookalike |
| R48 | high | unclear message | Switching document units with bare numbers |
| R49 | medium | wrong result | Splitting a side line must not halve the shell opening |
| R50 | medium | decision | A circle drawn inside the selected region |
| R51 | medium | wrong result | The picked arc of a tangent chain is deleted |
| R52 | medium | wrong result | A merged element comes apart |
| R53 | medium | wrong result | A join covers the referenced face |
| R54 | medium | unclear message | Deleting a boss under a datum gives one root cause |
| R55 | medium | wrong result | A sketch on a boss top moved above the boss |
| R56 | medium | unclear message | An edge that now lives in another body |
| R57 | medium | unclear message | A revolve becomes a full turn |
| R58 | medium | wrong result | A datum on a face that an inserted shell removes |
| R59 | medium | unclear message | The edge used as an axis is rounded upstream |
| R60 | medium | guard | A mirror plane on a face that a groove splits |
| R61 | medium | unclear message | Separate solids fuse, and come apart again |
| R62 | medium | guard | A channel splits a hole's bore |
| R63 | medium | wrong result | Changing the hole type |
| R64 | medium | wrong result | An inside corner already rounded upstream |
| R65 | low | guard | Reordering sketch entities and renaming features change nothing |

## A. Sketch edits

### R01 Corner chamfer line added to the base outline

- **Setup:** BLOCK. Face sketch `f-sk2` on `{q: f-ext1/end-cap, fp: plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4}`, fallback origin (0,0,30), x_axis (1,0,0), normal (0,0,1), with circle `e-5` center (40,25) radius 5; `f-ext2` extrude of it, distance 10, op join (boss z 30..40). `f-fil1` fillet radius 2 on `[{q: edge(f-ext1/end-cap|f-ext1/lateral/e-3), fp: line|n=1,0,0|c=40,50,30|l=80|adj=2,2}]`.
- **Reference:** `f-ext1` profile.regions[0] (the rectangle), and through it `f-sk2` plane.face (the top) and `f-fil1` edges[0] (the back top edge).
- **Edit:** in `f-sk1` end `e-1` at (75,0), start `e-2` at (80,5) and add line `e-6` (75,0)-(80,5) (a 5 mm corner chamfer). Variants: 0.5 mm and 10 mm (`e-6` (70,0)-(80,10)).
- **Expect:** `f-ext1` builds the new outline, at most with a notice that the outline changed: after `f-ext1` the volume is 119625 (10 mm variant 118500) with 7 faces, `f-ext1/lateral/e-6` among them and `end-cap`, `lateral/e-1` to `e-4` keeping their names. `f-sk2` stays on the top (origin (0,0,30)), the boss stands at (40,25) from z 30 to 40, `f-fil1` rounds the back top edge (axis y=48, z=28, x 0..80). All three sizes behave alike.
- **Today:** the key, outer-boundary and fingerprint steps of sketch.md section 3.14 all fail (the area changes by 12.5 mm2, over the 4 mm2 tolerance): `region_missing`, `profile_not_found`, and every later feature fails. The 0.5 mm chamfer passes, so success depends on the size typed.

### R02 Notch added to the base outline

- **Setup:** BLOCK. `f-fil1` fillet radius 2 on `[{q: edge(f-ext1/end-cap|f-ext1/lateral/e-1), fp: line|n=1,0,0|c=40,0,30|l=80|adj=2,2}]`. `f-cha1` chamfer equal 1 on `[{q: edge(f-ext1/lateral/e-1|f-ext1/lateral/e-2), fp: line|n=0,0,1|c=80,0,15|l=30|adj=2,2}]`.
- **Reference:** `f-ext1` profile.regions[0]; `f-fil1` edges[0] (front top edge); `f-cha1` edges[0] (front right vertical edge).
- **Edit:** `e-3` becomes (80,50)-(50,50); add `e-5` (50,50)-(50,40), `e-6` (50,40)-(30,40), `e-7` (30,40)-(30,50), `e-8` (30,50)-(0,50): a 20 x 10 notch in the back (region key `e-1` to `e-8`, area 3800, centroid (40,23.947368)). Variants: a 2 x 1 notch; instead of a notch, an arc of radius 3, then of radius 5, rounding the back right corner.
- **Expect:** `f-ext1` ok apart from at most an outline notice; after `f-ext1` the notched variant has 10 faces and volume 114000, with `f-ext1/lateral/e-5` to `e-8`. `f-fil1` and `f-cha1` resolve exactly and build on the front top edge (fillet axis y=2, z=28) and the front right vertical edge. No reference warnings, and every variant behaves alike.
- **Today:** the area changes 5 percent, over the 1e-3 relative tolerance, so `profile_not_found` takes the whole model down. The 2 x 1 notch (0.05 percent) passes with `region_remapped`; the radius 3 corner (0.048 percent) passes and the radius 5 corner (0.134 percent) fails.

### R03 Corner bevel removed from the outline

- **Setup:** bodies `[b-1]`. `f-sk1` on XY: `e-1` (0,0)-(80,0), `e-2` (80,0)-(80,40), `e-5` (80,40)-(70,50), `e-3` (70,50)-(0,50), `e-4` (0,50)-(0,0). `f-ext1` extrude of region `{key: e-1,e-2,e-3,e-4,e-5, fp: c=39.535865,24.725738|a=3950}`, distance 30, op new. `f-fil1` fillet radius 2 on `[{q: edge(f-ext1/end-cap|f-ext1/lateral/e-5), fp: line|n=0.707107,-0.707107,0|c=75,45,30|l=14.142136|adj=2,2}, {q: edge(f-ext1/end-cap|f-ext1/lateral/e-1), fp: line|n=1,0,0|c=40,0,30|l=80|adj=2,2}]`.
- **Reference:** `f-ext1` profile.regions[0]; `f-fil1` edges[0] (top edge of the bevel) and edges[1] (front top edge).
- **Edit:** delete `e-5`; end `e-2` and start `e-3` at (80,50) (a square corner).
- **Expect:** `f-ext1` ok on the full rectangle (6 faces, volume 120000), at most an outline notice. `f-fil1` status warning: edges[1] is rounded (volume 119931.327412); edges[0] is reported lost with repair pointer `/edges/0`, and the message names the element (the top edge of the bevel face) and the cause (its line was deleted from 'Sketch 1'), with actions to remove it from Edges or re-pick. The name `f-ext1/lateral/e-5` makes this cause knowable.
- **Today:** the key changes and the area changes 1.27 percent, so `f-ext1` fails with `profile_not_found`; with the profile fixed, the warning would say only "can no longer be found".

### R04 Circle drawn across the outline edge

- **Setup:** BLOCK. `f-fil1` fillet radius 2 on `[{q: edge(f-ext1/end-cap|f-ext1/lateral/e-3), fp: line|n=1,0,0|c=40,50,30|l=80|adj=2,2}]`.
- **Reference:** `f-ext1` profile.regions[0]; `f-fil1` edges[0] (back top edge).
- **Edit:** add circle `e-5` center (40,50) radius 10 to `f-sk1` (a guide for a later notch). The old region splits into the rectangle minus the lower half disc (area 3842.920367) and the lower half disc (area 157.079633); the upper half disc is a new region outside.
- **Expect:** the profile follows every piece of the old region, as a split face resolves to all its pieces. Their union is the rectangle, so after `f-ext1` the body has volume 120000 and exactly the six original face names (no `f-ext1/lateral/e-5`); `f-fil1` resolves exactly (volume 119931.327412). At most a notice that the region was split.
- **Today:** key, outer boundary and fingerprint (area minus 3.9 percent) all fail: `profile_not_found`, and every later feature fails.

### R05 Two profile rectangles overlap after a dimension change

- **Setup:** bodies `[b-1]`. `f-sk1` on XY: rectangle A `e-1` (0,0)-(30,0), `e-2` (30,0)-(30,50), `e-3` (30,50)-(0,50), `e-4` (0,50)-(0,0); rectangle B `e-5` (50,0)-(80,0), `e-6` (80,0)-(80,50), `e-7` (80,50)-(50,50), `e-8` (50,50)-(50,0). `f-ext1` extrude of regions `[{key: e-1,e-2,e-3,e-4, fp: c=15,25|a=1500}, {key: e-5,e-6,e-7,e-8, fp: c=65,25|a=1500}]`, distance 10, op new (two solids, caps `f-ext1/end-cap#1` and `#2`). `f-fil1` fillet radius 1 on `[{q: edge(f-ext1/end-cap#2|f-ext1/lateral/e-6), fp: line|n=0,1,0|c=80,25,10|l=50|adj=2,2}]`.
- **Reference:** both profile regions; `f-fil1` edges[0] (top edge of B's right side).
- **Edit:** move rectangle B to x 20..70 (the gap dimension between A and B goes from 20 to -10).
- **Expect:** `f-ext1` extrudes the area the two selected regions now cover: one solid x 0..70, y 0..50, z 0..10, volume 35000, one end cap `f-ext1/end-cap`. `f-fil1` resolves by history to `edge(f-ext1/end-cap|f-ext1/lateral/e-6)` and builds (axis x=69, z=9).
- **Today:** the arrangement gives three regions with new keys (A only, the overlap, B only); no key, outer boundary or fingerprint matches, so both regions are missing and `f-ext1` fails with `profile_not_found`.

### R06 Side line deleted and redrawn

- **Setup:** BLOCK. `f-fil1` fillet radius 2 on `[{q: edge(f-ext1/end-cap|f-ext1/lateral/e-2), fp: line|n=0,1,0|c=80,25,30|l=50|adj=2,2}]`. Face sketch `f-sk2` on `{q: f-ext1/lateral/e-2, fp: plane|n=1,0,0|c=80,25,15|a=1500|adj=4,4}`, fallback origin (80,0,0), x_axis (0,1,0), normal (1,0,0), with circle `e-5` center (25,15) radius 4; `f-ext2` extrude of it, distance 5, reverse, op cut (a pocket in the right side at (80,25,15)).
- **Reference:** `f-fil1` edges[0] (right top edge) and `f-sk2` plane.face (right side), both named after line `e-2`; `f-ext1` profile.regions[0].
- **Edit:** (a) delete `e-2` and draw `e-9` from (80,0) to (80,50) in the same place. (b) draw `e-9` at x=90 instead, extending `e-1` to (90,0) and starting `e-3` at (90,50).
- **Expect:** nothing moves. The profile follows the region (3 of its 4 lines remain), the right side is now `f-ext1/lateral/e-9`, `f-fil1` rounds the right top edge (axis x=78 in (a), x=88 in (b), z=28) and the pocket stays centered on the right side at y=25, z=15. In (a) the volume is 119705.752221. Each reference that followed by shape instead of name shows a notice ("matched by position after a line of 'Sketch 1' was replaced") with a one-click "keep this match" that stores the new name.
- **Today:** (a) the region is remapped by fingerprint with a permanent `region_remapped` warning (the stored key is never refreshed), and the fillet and sketch follow by fingerprint with no message. (b) the area is 4500 against 4000: `profile_not_found`.

### R07 A follow by shape must not decay on the next edit

- **Setup:** BLOCK. `f-fil1` fillet radius 2 on `[{q: edge(f-ext1/end-cap|f-ext1/lateral/e-3), fp: line|n=1,0,0|c=40,50,30|l=80|adj=2,2}]` (back top edge).
- **Reference:** `f-fil1` edges[0].
- **Edit:** (1) delete `e-3` and draw `e-9` (80,50)-(0,50) in its place (as R06). (2) then (a) change `f-ext1` distance from 30 to 80, or (b) widen the block to 160 (`e-2` moves to x=160, `e-1` ends and `e-9` starts at x=160).
- **Expect:** after (1), the fillet is on the back top edge at z=30. After (2), it is still on the back top edge: (a) axis y=48, z=78, x 0..80, volume 319931.327412, bottom back edge sharp; (b) axis y=48, z=28, x 0..160. Once a match by shape is accepted, later unrelated edits keep following that element (for example by storing the new name as an explicit, undoable change, or by resolving through the sketch's record that `e-9` replaced `e-3`). The extrude shows no lingering warning.
- **Today:** (a) the stored fingerprint still says z=30, so the bottom back edge `edge(f-ext1/lateral/e-9|f-ext1/start-cap)` scores 0.8784 against 0.8193 for the true edge and the fillet silently moves to the bottom. (b) the stored region key was never refreshed, so the wider outline (area 8000 against 4000) loses the profile and the whole model fails; with the profile fixed, the true edge scores 0.736, below the threshold, and the fillet fails. Either way an unrelated edit breaks it.

### R49 Splitting a side line must not halve the shell opening

- **Setup:** BLOCK. `f-sh1` shell thickness 2, remove_faces `[{q: f-ext1/lateral/e-2, fp: plane|n=1,0,0|c=80,25,15|a=1500|adj=4,4}]` (open right side).
- **Reference:** `f-sh1` remove_faces[0] (the whole right side).
- **Edit:** in `f-sk1` end `e-2` at (80,25) and add a collinear line `e-6` (80,25)-(80,50). The right side becomes `f-ext1/lateral/e-2` (y 0..25) and `f-ext1/lateral/e-6` (y 25..50).
- **Expect:** the whole right side stays open: one opening in x=80 spanning y 2..48, z 2..28. If only the `e-2` part is used, a notice says the face was split by the sketch edit and offers `f-ext1/lateral/e-6` in one click; the silent half opening is the failure.
- **Today:** the exact name matches the lower half only, so the shell silently leaves a wall over y 25..50 (and the profile shows `region_remapped`).

### R50 A circle drawn inside the selected region

- **Setup:** BLOCK.
- **Reference:** `f-ext1` profile.regions[0].
- **Edit:** add circle `e-5` center (40,25) radius 10 to `f-sk1` (for example as a guide for a later hole). The regions become the ring (`e-1`,`e-2`,`e-3`,`e-4` with hole `e-5`) and the disc `e-5`.
- **Expect (proposed, consistent with R04):** the selection keeps covering the same area, ring plus disc: volume 120000, no face `f-ext1/lateral/e-5`, and a notice that the region was split and both parts were kept, so a user who wants a hole removes the disc from the profile. This is a product decision recorded in sketch.md section 3.14 first; the test pins the rule chosen.
- **Today:** the outer-boundary step matches the ring only, so a through hole appears (volume 110575.222039) with only a `region_remapped` warning.

### R51 The picked arc of a tangent chain is deleted

- **Setup:** bodies `[b-1]`. `f-sk1` on XY, a rounded rectangle: lines `e-1` (5,0)-(75,0), `e-2` (80,5)-(80,45), `e-3` (75,50)-(5,50), `e-4` (0,45)-(0,5); arcs of radius 5 `e-5` center (75,5), `e-6` center (75,45), `e-7` center (5,45), `e-8` center (5,5). `f-ext1` extrude distance 10, op new. `f-fil1` fillet radius 1 on `[{q: edge(f-ext1/end-cap|f-ext1/lateral/e-6), fp: circle|n=0,0,1|c=78.183099,48.183099,10|l=7.853982|r=5|adj=2,2}]`; the tangent chain carries it round the whole top loop (8 fillet faces).
- **Reference:** `f-fil1` edges[0] (top edge of the back right corner arc).
- **Edit:** delete arc `e-6`; extend `e-2` to (80,50) and start `e-3` at (80,50) (a sharp corner).
- **Expect:** `f-ext1` builds (profile followed, at most a notice). `f-fil1` error: the fillet was attached to the top edge of an arc deleted from 'Sketch 1', through which it rounded the whole top loop; the one-click fix offers the top edge of the right side or of the back side (same loop) as the replacement. No other corner's arc is picked.
- **Today:** the area changes 0.135 percent, so `f-ext1` fails with `profile_not_found` as in R01; with the profile fixed, the other arcs score far below the threshold and the message says only "can no longer be found".

### R65 Reordering sketch entities and renaming features change nothing

- **Setup:** BLOCK with `f-sk1` entities stored in order `[e-1, e-2, e-3, e-4]`. `f-fil1` fillet radius 2 on `[{q: edge(f-ext1/end-cap|f-ext1/lateral/e-1), fp: line|n=1,0,0|c=40,0,30|l=80|adj=2,2}]`.
- **Reference:** `f-ext1` profile.regions[0]; `f-fil1` edges[0].
- **Edit:** (a) store the entities in order `[e-3, e-1, e-4, e-2]` with the same coordinates. (b) rename `f-ext1` to "Base" and body `b-1` to "Bracket".
- **Expect:** the same region key and fingerprint, the same face, edge and vertex names and fingerprints, volume 119931.327412, no warnings; the naming map JSON of `b-1` is identical to the original. In (b) messages use the new names.
- **Today:** guard.

## B. Dimension and parameter changes

### R08 Widening the part must not move a fillet to the other piece of a split top

- **Setup:** (a) BLOCK and SLOT(30,50). `f-fil1` fillet radius 2 on `[{q: edge(f-ext1/end-cap#2|f-ext1/lateral/e-1), fp: line|n=1,0,0|c=65,0,30|l=30|adj=2,2}]`. (b) Parameter `w` = "80 mm". `f-box1` box size `["w","50","30"]` at position `["w/2","25","0"]` (x 0..w, y 0..50, z 0..30), op new; `f-box2` box size `["20","60","5"]` at position `["w/2","25","25"]`, op cut (a centered slot); top pieces `f-box1/top#1` (x 0..30) and `#2` (x 50..80). `f-fil1` fillet radius 2 on `[{q: edge(f-box1/front|f-box1/top#2), fp: line|n=1,0,0|c=65,0,30|l=30|adj=2,2}]`.
- **Reference:** `f-fil1` edges[0]: the front top edge of the right-hand piece.
- **Edit:** (a) move the right side of `f-sk1` from x=80 to x=200 (`e-1` ends at (200,0), `e-2` runs (200,0)-(200,50), `e-3` starts at (200,50)). (b) set `w` to "200 mm" (one parameter drives two features).
- **Expect:** `f-fil1` ok with exactly one fillet face, axis y=2, z=28, spanning (a) x 50..200, (b) x 110..200; the left piece's front top edge stays sharp. The same for widths 120, 200 and 400.
- **Today:** the order-dependent name is confirmed against the stale fingerprint: the named edge scores (a) 0.486 against 0.685 for the left piece's edge, (b) 0.482 against 0.663, so step 1 hands the reference to the left piece and the fillet moves silently. At width 120 it stays, so the outcome depends on the number typed.

### R09 Moving a slot so the pieces swap sizes must not move the shell opening

- **Setup:** BLOCK and SLOT(20,30): `f-ext1/end-cap#1` x 0..20 (area 1000), `#2` x 30..80 (area 2500). `f-sh1` shell thickness 2, remove_faces `[{q: f-ext1/end-cap#2, fp: plane|n=0,0,1|c=55,25,30|a=2500|adj=4,4}]`.
- **Reference:** `f-sh1` remove_faces[0]: the top of the right-hand block, the piece that touches the right side `f-ext1/lateral/e-2`.
- **Edit:** move the slot to x 55..65 (pieces `#1` x 0..55, area 2750; `#2` x 65..80, area 750).
- **Expect:** the shell still opens the right-hand block: a rim face `f-sh1/rim/f-ext1/end-cap#2` with centroid x 72.5 exists, `f-ext1/end-cap#1` (x 0..55) stays a closed top at z=30, no warning.
- **Today:** step 1 scores `#2` 0.7296 against 0.8153 for `#1` (the old area now matches `#1`) and silently opens the left piece. Comparing what each piece touches would keep it.

### R10 A split top becomes one face again

- **Setup:** (a) BLOCK and SLOT(30,50). `f-fil1` fillet radius 2 on `[{q: edge(f-ext1/end-cap#2|f-ext1/lateral/e-1), fp: line|n=1,0,0|c=65,0,30|l=30|adj=2,2}]`. (b) BLOCK; `f-dp1` datum XY offset 30; slot A: `f-sk2` on `f-dp1` rectangle x 10..20, y -1..51, `f-ext2` distance 5, reverse, op cut; slot B: `f-sk3` on `f-dp1` rectangle x 50..60, y -1..51, `f-ext3` the same (pieces `#1` x 0..10, `#2#1` x 20..50, `#2#2` x 60..80). `f-fil1` fillet radius 1 on `[{q: edge(f-ext1/end-cap#2#1|f-ext1/lateral/e-1), fp: line|n=1,0,0|c=35,0,30|l=30|adj=2,2}]`.
- **Reference:** `f-fil1` edges[0]: (a) the front top edge of the right-hand piece; (b) the front top edge of the middle piece.
- **Edit:** (a) move the slot to x 50..81: the right piece is cut away and the top is one face `f-ext1/end-cap` (x 0..50). (b) delete `f-sk2` and `f-ext2`, and separately suppress them: pieces become `#1` (x 0..50) and `#2` (x 60..80).
- **Expect:** (a) `f-fil1` error, no fillet face, volume 112500; the message says the front top edge of the right part of the top was removed when 'Sketch 2' moved the slot of 'Extrude 2', and the repair data offers the slot floor's front edge (y=0, z=25, x 50..80) as a one-click replacement. The left part's edge is not rounded. (b) `f-fil1` ok on the merged segment x 0..50 that contains the old middle piece (axis y=1, z=29); x 60..80 stays sharp; a notice that the edge grew is allowed.
- **Today:** (a) step 3 finds a family of one and resolves it with confidence 0.625 to the left part's edge, rounding it silently. (b) passes (0.801 against 0.658). A fix must keep both: a fingerprint threshold on family matches breaks (b); checking that the old piece lies within the candidate does not.

### R11 A notch moved further than its width keeps the chamfer on its side

- **Setup:** BLOCK. `f-sk2` on XY: circle `e-5` center (80,25) radius 5; `f-ext2` extrude through all, op cut (a half-round notch in the right side, which splits `f-ext1/lateral/e-2` into `#1` y 0..20 and `#2` y 30..50). `f-cha1` chamfer equal 1 on `[{q: edge(f-ext1/lateral/e-2#2|f-ext2/lateral/e-5), fp: line|n=0,0,1|c=80,30,15|l=30|adj=2,2}]`.
- **Reference:** `f-cha1` edges[0]: the vertical edge where the notch meets the right side on its +y side.
- **Edit:** move the center of `e-5` to (80,40) (pieces `#1` y 0..35 and `#2` y 45..50; the notch edges are at y=35 and y=45).
- **Expect:** `f-cha1` ok, no warning; its chamfer face has centroid y between 44 and 46, not 34 to 36. Rank within a family whose member count did not change survives moves larger than half the notch's width.
- **Today:** step 1 scores the named edge (y=45) 0.847 against 0.940 for the y=35 edge and resolves to the wrong side.

### R12 A pocket that becomes a through hole must not move the floor fillet

- **Setup:** BLOCK. `f-dp1` datum XY offset 30. `f-sk2` on `f-dp1`: circle `e-5` center (40,25) radius 10. `f-ext2` extrude of it, distance 5, reverse, op cut (pocket z 25..30, floor `f-ext2/end-cap`). `f-fil1` fillet radius 2 on `[{q: edge(f-ext2/end-cap|f-ext2/lateral/e-5), fp: circle|n=0,0,1|c=40,25,25|l=62.831853|r=10|adj=2,1}]`. Variant: a square pocket, `f-sk2` lines `e-11` (30,15)-(50,15), `e-12` (50,15)-(50,35), `e-13` (50,35)-(30,35), `e-14` (30,35)-(30,15), and `f-fil1` radius 1 on `[{q: edge(f-ext2/end-cap|f-ext2/lateral/e-11), fp: line|n=1,0,0|c=40,15,25|l=20|adj=2,2}]`.
- **Reference:** `f-fil1` edges[0]: the concave edge where the pocket floor meets its wall.
- **Edit:** change `f-ext2` distance from 5 to 30 (exactly through), and separately to 40.
- **Expect:** `f-fil1` error, no face `f-fil1/fillet/*`; volume 110575.222039 with 7 faces (square variant 108000). The message says the bottom edge of the pocket cut by 'Extrude 2' no longer exists because 'Extrude 2' now goes through the body (Distance changed from 5 mm to 30 mm), with repair data for `/edges/0`. The rims may be listed as candidates but are never applied.
- **Today:** the fingerprint step scores the top rim 0.9732 (square variant 0.914) and rounds the hole's entrance, status ok, no warning.

### R13 Extruded block made flush with the base

- **Setup:** BLOCK. `f-sk2` on XY: `e-5` (80,0)-(120,0), `e-6` (120,0)-(120,50), `e-7` (120,50)-(80,50), `e-8` (80,50)-(80,0); `f-ext2` extrude of region `{key: e-5,e-6,e-7,e-8, fp: c=100,25|a=2000}`, distance 20, op join (a lower step; the riser `f-ext1/lateral/e-2` at x=80, z 20..30). `f-fil1` fillet radius 2 on `[{q: edge(f-ext1/end-cap|f-ext1/lateral/e-2), fp: line|n=0,1,0|c=80,25,30|l=50|adj=2,2}]`. Face sketch `f-sk3` on `{q: f-ext2/end-cap, fp: plane|n=0,0,1|c=100,25,20|a=2000|adj=4,4}`, fallback origin (0,0,20), x_axis (1,0,0), normal (0,0,1), with circle `e-9` center (100,25) radius 5; `f-ext3` extrude distance 5, op join. `f-fil2` fillet radius 1 on `[{q: edge(f-ext2/end-cap|f-ext2/lateral/e-6), fp: line|n=0,1,0|c=120,25,20|l=50|adj=2,2}]`.
- **Reference:** `f-fil1` edges[0] (the step edge); `f-sk3` plane.face (top of the lower block); `f-fil2` edges[0] (its right top edge).
- **Edit:** change `f-ext2` distance from 20 to 30: the block top is flush with the base top and merges into `f-ext1/end-cap` (alias `f-ext2/end-cap`), and the riser disappears.
- **Expect:** `f-sk3` resolves through the alias to the merged top and sits at z=30 (boss z 30..35 at (100,25)); `f-fil2` resolves to `edge(f-ext1/end-cap|f-ext2/lateral/e-6)` and builds (axis x=119, z=29). `f-fil1` error: the step edge between the top and the right side of 'Extrude 1' no longer exists because 'Extrude 2' now reaches the same height (Distance changed from 20 mm to 30 mm); repair data `/edges/0`; no substitution (the nearest candidate scores 0.786).
- **Today:** `f-sk3` and `f-fil2` pass; `f-fil1` says only "can no longer be found".

### R14 The second fillet keeps the first fillet's arc when the first radius changes

- **Setup:** bodies `[b-1]`. `f-box1` box 30 x 20 x 10 (x -15..15, y -10..10, z 0..10). `f-fil1` fillet radius 3 on the four vertical edges `edge(f-box1/front|f-box1/left)`, `edge(f-box1/front|f-box1/right)`, `edge(f-box1/back|f-box1/left)`, `edge(f-box1/back|f-box1/right)`. `f-fil2` fillet radius 1 on `[{q: edge(f-box1/top|f-fil1/fillet/edge(f-box1/front|f-box1/left)), fp: circle|n=0,0,1|c=-13.909859,-8.909859,10|l=4.712389|r=3|adj=2,2}]`; the tangent chain carries it round the top loop.
- **Reference:** `f-fil2` edges[0]: the top arc of the front-left corner rounded by `f-fil1`.
- **Edit:** change `f-fil1` radius from 3 to 5, and separately to 2.
- **Expect:** the arc keeps its name and `f-fil2` resolves exactly despite the stale radius: it rounds the whole top loop with 8 faces named `f-fil2/fillet/...` (as fillet.md test 11), no warnings.
- **Today:** guard.

### R52 A merged element comes apart

- **Setup:** (a) CUBE10. `f-lp1` linear_pattern of `b-1` along origin X, count 3, spacing 10, op join: the copies touch and fuse into one bar x -5..25, whose merged faces keep the names `f-box1/top` and `f-box1/front`. `f-fil1` fillet radius 1 on `[{q: edge(f-box1/front|f-box1/top), fp: line|n=1,0,0|c=10,-5,10|l=30|adj=2,2}]` (the bar's 30 mm front top edge). (b) bodies `[b-1]`. `f-box1` box 20 x 20 x 10 (x -10..10, y -10..10, z 0..10); `f-box2` box 20 x 20 x 10 at position (20,0,0), op join (one top `f-box1/top`, x -10..30, area 800, alias `f-box2/top`). `f-sh1` shell thickness 2, remove_faces `[{q: f-box1/top, fp: plane|n=0,0,1|c=10,0,10|a=800|adj=4,4}]` (an open 40 x 20 tray).
- **Reference:** (a) `f-fil1` edges[0]; (b) `f-sh1` remove_faces[0], the whole merged top.
- **Edit:** (a) spacing 10 to 15 (three blocks at x -5..5, 10..20, 25..35). (b) `f-box2` height 10 to 15 (the tops separate: `f-box1/top` at z=10, `f-box2/top` at z=15).
- **Expect:** every former part is used: (a) three fillet faces, axis y=-4, z=9, along x -5..5, 10..20 and 25..35; (b) faces `f-sh1/rim/f-box1/top` and `f-sh1/rim/f-box2/top` exist and no face named `f-box2/top` remains. Alternatively only the part that kept the name is used and a notice says the picked element came apart, offering the other parts in one click. The silent partial result is the failure.
- **Today:** the exact name resolves with confidence 1 to the original block's part only ((a) the copies stay sharp, (b) Box 2's top stays closed), with no message.

## C. Features inserted upstream

### R15 Dress-ups inserted upstream keep the names they modify

- **Setup:** (a) bodies `[b-1]`. `f-box1` box 40 x 40 x 10 (x -20..20, y -20..20, z 0..10). Face sketch `f-sk2` on `{q: f-box1/top, fp: plane|n=0,0,1|c=0,0,10|a=1600|adj=4,4}`, fallback origin (0,0,10), x_axis (1,0,0), normal (0,0,1), with circle `e-5` center (10,10) radius 4; `f-ext2` extrude distance 10, op join. (b) bodies `[b-1, b-2]`. CUBE20. `f-dp1` datum_plane on face `{q: f-box1/top, fp: plane|n=0,0,1|c=0,0,20|a=400|adj=4,4}`, offset -5 (z=15). `f-split1` split_body of `b-1` by `f-dp1`, target `b-2`. (c) CUBE20. `f-fil1` fillet radius 2 on `[{q: edge(f-box1/front|f-box1/top), fp: line|n=1,0,0|c=0,-10,20|l=20|adj=2,2}]`.
- **Reference:** (a) `f-sk2` plane.face; (b) `f-dp1` base.face; (c) `f-fil1` edges[0].
- **Edit:** (a) and (b) insert after `f-box1` a fillet radius 2 on the four top edges (front, back, left and right with top). (c) insert before `f-fil1` a chamfer `f-cha1`, equal 1, on `edge(f-box1/left|f-box1/top)` and `edge(f-box1/right|f-box1/top)`.
- **Expect:** exact matches and no warnings anywhere. (a) sketch origin (0,0,10), boss at (10,10) from z 10 to 20. (b) datum origin (0,0,15), normal +Z; `b-1` bounding box [-10,-10,15]-[10,10,20]; `b-2` bounding box [-10,-10,0]-[10,10,15], volume 6000. (c) `f-fil1/fillet/edge(f-box1/front|f-box1/top)` is a radius 2 cylinder with axis along x at y=-8, z=18; both chamfer faces remain.
- **Today:** guard. The names are not order dependent, so the stale fingerprints (area 1600 against about 1296 in (a)) are not consulted.

### R16 A cut that removes the filleted edge

- **Setup:** CUBE20. `f-fil1` fillet radius 2 on `[{q: edge(f-box1/front|f-box1/top), fp: line|n=1,0,0|c=0,-10,20|l=20|adj=2,2}]`.
- **Reference:** `f-fil1` edges[0]: the front top edge.
- **Edit:** insert before `f-fil1`: `f-sk2` on YZ with a rectangle u (y) -11..-5, v (z) 15..21, and `f-ext2` extrude symmetric 30, op cut (a 5 x 5 rabbet along the whole front top edge). Their labels are 'Sketch 1' and 'Extrude 1'.
- **Expect:** `f-fil1` error, no fillet, volume 7500. The message says what (the front top edge of 'Box 1'), why ('Extrude 1', which comes before this fillet, cut it away) and what to do (pick one of the new edges along the cut). The repair panel offers the rabbet's two outer edges (y=-5, z=20 and y=-10, z=15) as one-click replacements and draws the old edge ghosted.
- **Today:** the three rabbet edges score 0.884 to 0.914, the reference is ambiguous, the candidates are dropped, and `references_not_found` says only "can no longer be found".

### R17 A second slot inserted upstream splits references by meaning

- **Setup:** (a, b) BLOCK. `f-dp1` datum XY offset 30. `f-sk3` on `f-dp1` rectangle x 50..60, y -1..51; `f-ext3` distance 5, reverse, op cut (pieces `#1` x 0..50, `#2` x 60..80). `f-fil1` fillet radius 1 on `[{q: edge(f-ext1/end-cap#1|f-ext1/lateral/e-1), fp: line|n=1,0,0|c=25,0,30|l=50|adj=2,2}]`. `f-cha1` chamfer equal 1 on `[{q: edge(f-ext1/end-cap#2|f-ext1/lateral/e-3), fp: line|n=1,0,0|c=70,50,30|l=20|adj=2,2}]`. (c) BLOCK and SLOT(30,50); `f-fil1` fillet radius 1 on `[{q: edge(f-ext1/end-cap#2|f-ext1/lateral/e-1), fp: line|n=1,0,0|c=65,0,30|l=30|adj=2,2}]`; `f-cha1` chamfer equal 1 on `[{q: edge(f-ext1/end-cap#1|f-ext1/lateral/e-3), fp: line|n=1,0,0|c=15,50,30|l=30|adj=2,2}]`.
- **Reference:** `f-fil1` edges[0] and `f-cha1` edges[0], each one segment of the top's front or back edge.
- **Edit:** (a) insert `f-sk2` (on `f-dp1`, rectangle x 10..20, y -1..51) and `f-ext2` (distance 5, reverse, cut) between `f-dp1` and `f-sk3`: pieces `#1` x 0..10, `#2#1` x 20..50, `#2#2` x 60..80. (b) insert the same slot after `f-ext3` instead: pieces `#1#1` x 0..10, `#1#2` x 20..50, `#2` x 60..80. (c) insert a slot at x 60..70 between `f-dp1` and `f-sk2`: pieces `#1#1` x 0..30, `#1#2` x 50..60, `#2` x 70..80.
- **Expect:** all ok, no warnings. (a) and (b): `f-fil1` rounds both pieces of the old left segment, x 0..10 and x 20..50 (two fillet faces, axis y=1, z=29); `f-cha1` chamfers only x 60..80. (c) `f-fil1` rounds x 50..60 and x 70..80; `f-cha1` chamfers only x 0..30.
- **Today:** (a) `f-fil1` resolves to x 20..50 only (family scores 0.694, 0.841, 0.636) and `f-cha1` to x 20..50 and 60..80 (both taken as split descendants of `#2`). (b) passes. (c) `f-fil1` resolves exactly to x 70..80 only and `f-cha1` to x 0..30 and 50..60. The suffix a piece gets depends on the order of cuts, not on what it is.

### R18 A dress-up inserted before another dress-up on the same edge

- **Setup:** (a) BLOCK. `f-cha1` chamfer equal 2 on `[{q: edge(f-ext1/lateral/e-1|f-ext1/lateral/e-2), fp: line|n=0,0,1|c=80,0,15|l=30|adj=2,2}]`. (b) CUBE20. `f-fil1` fillet radius 2 on `[{q: edge(f-box1/front|f-box1/top), fp: line|n=1,0,0|c=0,-10,20|l=20|adj=2,2}]`.
- **Reference:** (a) `f-cha1` edges[0], the front right vertical edge; (b) `f-fil1` edges[0], the front top edge.
- **Edit:** (a) insert a fillet `f-fil1` radius 5 on the same edge between `f-ext1` and `f-cha1`. (b) insert a chamfer `f-cha1` equal 2 on the same edge before `f-fil1`.
- **Expect:** neither tangent edge of the inserted feature is used. (a) `f-cha1` error: the edge was rounded by 'Fillet 1', which now comes before this chamfer; suggestions: remove one of them, move 'Fillet 1' after the chamfer, or re-pick; repair data `/edges/0` lists the two tangent edges. (b) `f-fil1` error: 'Chamfer 1' cut the edge away; repair data lists both chamfer edges; volume 7960, no face `f-fil1/fillet/*`.
- **Today:** both tangent edges tie ((a) 0.9403, (b) 0.963), the reference is ambiguous, the candidates are dropped when the list resolves to nothing, and the message says only "can no longer be found".

### R19 A shell inserted upstream keeps a fillet on the outer top edge

- **Setup:** BLOCK. `f-fil1` fillet radius 1 on `[{q: edge(f-ext1/end-cap|f-ext1/lateral/e-1), fp: line|n=1,0,0|c=40,0,30|l=80|adj=2,2}]`.
- **Reference:** `f-fil1` edges[0]: the front top edge.
- **Edit:** insert between `f-ext1` and `f-fil1` a shell `f-sh1` thickness 3, remove_faces `[{q: f-ext1/end-cap, fp: plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4}]`. The end cap becomes the rim `f-sh1/rim/f-ext1/end-cap`.
- **Expect:** `f-fil1` ok on `edge(f-ext1/lateral/e-1|f-sh1/rim/f-ext1/end-cap)` (the outer edge, axis y=1, z=29, x 0..80), producing `f-fil1/fillet/edge(f-ext1/lateral/e-1|f-sh1/rim/f-ext1/end-cap)`; the inner edge at y=3 stays sharp. The rim is the end cap's history descendant (naming.md section 3.6.7 rule 1), so this is a history match; at most a notice that the edge now borders the rim.
- **Today:** the outer edge scores 1.0 and the inner edge 0.969, inside the 0.05 margin, so the reference is ambiguous and `f-fil1` fails although one candidate matches exactly.

### R20 A slot inserted before a pattern splits references on the copy

- **Setup:** BLOCK. `f-lp1` linear_pattern of `b-1` along origin Y, count 2, spacing 60, op join (a copy at y 60..110, a separate solid). `f-fil1` fillet radius 1 on `[{q: edge(f-ext1/end-cap@f-lp1:1|f-ext1/lateral/e-1@f-lp1:1), fp: line|n=1,0,0|c=40,60,30|l=80|adj=2,2}]`.
- **Reference:** `f-fil1` edges[0]: the front top edge of the copy.
- **Edit:** (a) insert SLOT(30,50) (`f-dp1`, `f-sk2`, `f-ext2`) between `f-ext1` and `f-lp1`; the copy's top pieces are `f-ext1/end-cap#1@f-lp1:1` and `f-ext1/end-cap#2@f-lp1:1`. (b) on the model after (a), with the reference stored as `{q: edge(f-ext1/end-cap#2@f-lp1:1|f-ext1/lateral/e-1@f-lp1:1), fp: line|n=1,0,0|c=65,60,30|l=30|adj=2,2}`, suppress `f-ext2`.
- **Expect:** the copy behaves like the original (naming.md test 84). (a) `f-fil1` resolves by history to both front top segments of the copy, x 0..30 and x 50..80 at y=60, two fillet faces. (b) it resolves by history to the copy's unsplit front top edge (x 0..80, y=60); unsuppressing restores the result of (a).
- **Today:** the instance suffix follows the split suffix, so neither the split step nor the family step sees the pieces: (a) both segments score 0.757, below 0.85, and `f-fil1` fails while the same reference on the source body works; (b) the best candidate scores 0.641 and `f-fil1` fails.

### R21 A box joined against a filleted side

- **Setup:** bodies `[b-1]`. `f-box1` box 20 x 20 x 10 (x -10..10, y -10..10, z 0..10). `f-fil1` fillet radius 2 on `[{q: edge(f-box1/right|f-box1/top), fp: line|n=0,1,0|c=10,0,10|l=20|adj=2,2}]`.
- **Reference:** `f-fil1` edges[0]: the top edge of Box 1's right side (convex).
- **Edit:** insert before `f-fil1` a box `f-box2` 20 x 20 x h at position (20,0,0), op join: (a) h = 10 (the tops merge; `f-box1/right` becomes interior); (b) h = 15 (the inside corner `edge(f-box1/top|f-box2/left)` appears exactly where the edge was).
- **Expect:** no fillet and no cove: `f-fil1` error, volume (a) 8000, (b) 10000. The message says the top edge of the right side of 'Box 1' is now inside the part (a) or has become an inside corner (b) because 'Box 2' was joined against that side, and suggests deleting the fillet or picking another edge; (b) offers the inside corner as a one-click replacement without applying it. The new right end at x=30 is never rounded.
- **Today:** (a) the correct failure (nearest 0.754) with only "can no longer be found". (b) the inside corner has an identical fingerprint (score 1.0) and the fillet silently adds a cove.

### R22 A two-distance chamfer keeps its sides when a join renames the top

- **Setup:** CUBE20. `f-cha1` chamfer two_distances, expr 1, second_expr 3, on `[{q: edge(f-box1/front|f-box1/top), fp: line|n=1,0,0|c=0,-10,20|l=20|adj=2,2}]`: 1 mm on `f-box1/front` (first in byte order), 3 mm on `f-box1/top` (chamfer.md test 2).
- **Reference:** `f-cha1` edges[0] and the side each distance is measured on.
- **Edit:** (a) insert before `f-cha1` a box `f-base1` 20 x 40 x 20 at position (20,0,0), op join (x 10..30, y -20..20): the tops merge as `f-base1/top`, the smaller name, with alias `f-box1/top`; the fronts are not coplanar. (b) insert instead a box `f-a1` 20 x 20 x 20 at position (0,20,0), op join (behind; the tops merge as `f-a1/top`).
- **Expect:** the edge resolves through the alias and the chamfer keeps its sides: 1 mm down the front (the chamfer's edge on the front at z=19) and 3 mm into the top (its edge on the top at y=-7). In (b) `f-box1/front` has area 380 and the merged top 740. The side never depends on the inserted feature's id.
- **Today:** the merged top sorts first, so 1 mm goes on the top and 3 mm on the front, silently ((b): front 340, top 780). With interactive ids (`f-` plus 12 random hex digits) about half of such joins flip the chamfer.

### R53 A join covers the referenced face

- **Setup:** (a) bodies `[b-1]`. `f-box1` box 80 x 50 x 30 (x -40..40, y -25..25, z 0..30). `f-sh1` shell thickness 2, remove_faces `[{q: f-box1/top, fp: plane|n=0,0,1|c=0,0,30|a=4000|adj=4,4}]`. (b) CUBE20. `f-dp1` datum_plane on face `{q: f-box1/top, fp: plane|n=0,0,1|c=0,0,20|a=400|adj=4,4}`, offset 5 (z=25). `f-sk1` on `f-dp1` with circle `e-1` center (0,0) radius 3; `f-ext1` extrude distance 8, reverse, op cut.
- **Reference:** (a) `f-sh1` remove_faces[0]; (b) `f-dp1` base.face; both the top of Box 1.
- **Edit:** insert before the referencing feature a box `f-box2` covering the top exactly, op join: (a) 80 x 50 x t at position (0,0,30); (b) 20 x 20 x t at position (0,0,20); each with t = 5 and t = 10. Box 1's top becomes interior and is deleted; the new top is `f-box2/top`.
- **Expect:** the same outcome for t = 5 and t = 10, and no silent move: the feature reports that its face (the top of 'Box 1') is now covered by 'Box 2', inserted before it, and offers "Use the top of Box 2" as a one-click fix (after which (a) opens z=30+t and (b) puts the datum at z=25+t). In (b), `f-sk1` and `f-ext1` name the datum as the cause, not their own references.
- **Today:** (a) the new top scores 0.97 (t=5) and 0.943 (t=10) and the shell silently opens it. (b) at t=5 the datum silently moves 5 mm up (0.914); at t=10 it fails (0.847) and `f-sk1` and `f-ext1` fail with generic messages.

## D. Suppress and delete

### R23 Suppressing one peg must not move its fillet to the other peg

- **Setup:** bodies `[b-1]`. `f-box1` box 60 x 30 x 10 (x -30..30, y -15..15, z 0..10). `f-dp1` datum XY offset 10. `f-sk1` on `f-dp1`: circle `e-5` center (-5,0) radius 4; `f-ext1` extrude distance 20, op join (peg A, z 10..30). `f-sk2` on `f-dp1`: circle `e-7` center (5,0) radius 4; `f-ext2` extrude distance 20, op join (peg B). `f-fil1` fillet radius 1 on `[{q: edge(f-ext1/end-cap|f-ext1/lateral/e-5), fp: circle|n=0,0,1|c=-5,0,30|l=25.132741|r=4|adj=2,1}]`.
- **Reference:** `f-fil1` edges[0]: the top rim of peg A.
- **Edit:** suppress `f-ext1`; separately, delete `f-sk1` and `f-ext1`.
- **Expect:** `f-fil1` error, no face `f-fil1/fillet/*`, peg B's top rim sharp. The message names the element (the top rim of 'Extrude 1') and the cause ('Extrude 1' is suppressed, or a deleted feature made it), with Unsuppress as the one-click fix when suppressed. Unsuppressing gives back the original result and a byte-identical document.
- **Today:** step 4 scores peg B's top rim 0.885 (its base rim 0.794) and silently rounds peg B.

### R24 A slot-rim fillet when the slot is suppressed or the fillet dragged above it

- **Setup:** CUBE20. `f-box2` box 4 x 30 x 5 at position (-6,0,15), op cut (a slot x -8..-4 through Y at z 15..20; the top splits into `f-box1/top#1` x -10..-8 and `#2` x -4..10). `f-fil1` fillet radius 0.5 on `[{q: edge(f-box1/top#2|f-box2/right), fp: line|n=0,1,0|c=-4,0,20|l=20|adj=2,2}]`.
- **Reference:** `f-fil1` edges[0]: the slot's right rim.
- **Edit:** (a) suppress `f-box2`. (b) drag `f-fil1` above `f-box2` (order `f-box1`, `f-fil1`, `f-box2`).
- **Expect:** no other edge is rounded. (a) `f-fil1` error naming 'Box 2' as suppressed, with Unsuppress or re-pick; the plain cube remains (volume 8000, 6 faces). (b) `f-fil1` error of the forward kind: the edge is made by 'Box 2', which now comes after this fillet; one click moves the fillet below 'Box 2'; the slotted cube without a fillet remains (volume 7600).
- **Today:** in both, step 4 picks the cube's outer left top edge `edge(f-box1/left|f-box1/top)` (0.899, next 0.804) and rounds it silently.

### R25 Suppressing a chamfer that carries a datum

- **Setup:** CUBE20. `f-cha1` chamfer equal 2 on `[{q: edge(f-box1/front|f-box1/top), fp: line|n=1,0,0|c=0,-10,20|l=20|adj=2,2}]`. `f-dp1` datum_plane on face `{q: f-cha1/chamfer/edge(f-box1/front|f-box1/top), fp: plane|n=0,-0.707107,0.707107|c=0,-9,19|a=56.568542|adj=4,4}`, offset 0 (datum_plane.md test 9). `f-sk1` on `f-dp1` with circle `e-1` center (0,7.071068) radius 0.5 (the chamfer face's center); `f-ext1` extrude distance 1, op join.
- **Reference:** `f-dp1` base.face: the chamfer face.
- **Edit:** suppress `f-cha1`, then unsuppress it.
- **Expect:** while suppressed, `f-dp1` error: the face was made by 'Chamfer 1', which is suppressed; one-click "Unsuppress Chamfer 1", or re-pick. `f-sk1` and `f-ext1` report `upstream_failed` pointing at 'Datum plane 1' and its root cause. No substitution onto the top or front (about 0.46). After unsuppressing, every feature is ok, the body equals the original, and the saved document is byte-identical to the one before the suppress.
- **Today:** "The face selected for Base plane can no longer be found. Re-pick it in the repair panel." with no cause.

### R26 Deleting and recreating the base extrude

- **Setup:** BLOCK. Face sketch `f-sk2` on `{q: f-ext1/end-cap, fp: plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4}`, fallback origin (0,0,30), x_axis (1,0,0), normal (0,0,1), with circle `e-5` center (40,25) radius 10; `f-ext2` extrude distance 10, op join (boss on top).
- **Reference:** `f-sk2` plane.face: the top of the block.
- **Edit:** (1) change `f-ext1` distance from 30 to 80 (the sketch follows by name to z=80). (2) delete `f-ext1` and add `f-ext3` with the same profile, distance 80, op new, at the same timeline position.
- **Expect:** the boss stays on top at z 80..90: one solid, volume 323141.592654, bounding box max z 90. If the face is not matched, the sketch keeps its last resolved placement (z=80), and the warning says the feature that made its face was deleted and offers a one-click re-attach to the top of the new extrude.
- **Today:** the stored fingerprint still says z=30, the new end cap scores 0.7875, and the sketch uses the fallback stored at attach time (z=30) while the message claims it "stays at its last position"; the boss lands inside the body and `f-ext2` reports `no_effect`.

### R27 Removing an upstream move must not hand a reference to the sibling piece

- **Setup:** CUBE20. `f-box2` box 4 x 30 x 5 at position (0,0,15), op cut (a centered slot; the top splits into `f-box1/top#1` x -10..-2 and `#2` x 2..10). `f-mv1` move_body of `b-1`, translation (12,0,0). `f-fil1` fillet radius 0.5 on `[{q: edge(f-box1/front|f-box1/top#1), fp: line|n=1,0,0|c=6,-10,20|l=8|adj=2,2}]`, picked after the move.
- **Reference:** `f-fil1` edges[0]: the front top edge of the left piece.
- **Edit:** suppress `f-mv1` (the same as setting its translation to (0,0,0)).
- **Expect:** the fillet stays on the left piece, now x -10..-2 (axis y=-9.5, z=19.5), no warning. The move never renumbered the pieces (naming.md section 3.6.12), so the stored name is right.
- **Today:** the world-space fingerprint now matches `#2`'s edge exactly (1.0, against 0.698 for `#1`), step 1 hands the reference to `#2` (method history, no warning) and the fillet jumps to the right piece. FreeCAD keeps indices under a placement change, so this hazard is specific to Partsmith.

### R54 Deleting a boss under a datum gives one root cause

- **Setup:** BLOCK. Face sketch `f-sk2` on `{q: f-ext1/end-cap, fp: plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4}`, fallback origin (0,0,30), x_axis (1,0,0), normal (0,0,1), with lines `e-11` (20,10)-(60,10), `e-12` (60,10)-(60,30), `e-13` (60,30)-(20,30), `e-14` (20,30)-(20,10); `f-ext2` extrude distance 10, op join (boss z 30..40). `f-dp1` datum_plane on face `{q: f-ext2/end-cap, fp: plane|n=0,0,1|c=40,20,40|a=800|adj=4,4}`, offset 0. `f-sk3` on `f-dp1` with circle `e-21` center (40,20) radius 5; `f-ext3` extrude distance 5, op join.
- **Reference:** `f-dp1` base.face: the boss top.
- **Edit:** delete `f-sk2` and `f-ext2`.
- **Expect:** `f-dp1` error: its face belonged to a feature that was deleted; re-pick a face, with the block top offered. `f-sk3` and `f-ext3` report `upstream_failed` pointing at 'Datum plane 1' as the single root cause. Nothing re-attaches by guess (the block top scores 0.695); volume 120000.
- **Today:** the message says only "can no longer be found".

## E. Reorder

### R28 A fillet dragged above the feature that makes its edge

- **Setup:** BLOCK. `f-dp1` datum XY offset 30. `f-sk2` on `f-dp1`: circle `e-5` center (40,25) radius 10. `f-ext2` extrude distance 10, op join (boss z 30..40). `f-fil1` fillet radius 2 on `[{q: edge(f-ext2/end-cap|f-ext2/lateral/e-5), fp: circle|n=0,0,1|c=40,25,40|l=62.831853|r=10|adj=2,1}]`. Variant: the reference is the concave ring at the boss base, `{q: edge(f-ext1/end-cap|f-ext2/lateral/e-5), fp: circle|n=0,0,1|c=40,25,30|l=62.831853|r=10|adj=2,1}`.
- **Reference:** `f-fil1` edges[0]: the boss's top rim (variant: its base ring).
- **Edit:** drag `f-fil1` to directly after `f-ext1`; then drag it back.
- **Expect:** while above, `f-fil1` fails with an order message, not a lost one: the edge comes from 'Extrude 2', which now comes after this fillet, with a one-click "Move Fillet 1 below Extrude 2" (the name contains `f-ext2`, so the cause is exact). No fingerprint search, no block edge rounded, final volume 123141.592654. Ideally the timeline warns before the drop. After dragging back, `f-fil1` resolves exactly and the document is byte-identical to the original.
- **Today:** `references_not_found` "The edge selected for Edges can no longer be found. Re-pick it in the repair panel.", because element names never create feature dependencies (features.md section 2.4).

### R29 Swapping two independent slot cuts must not widen a reference

- **Setup:** BLOCK. `f-dp1` datum XY offset 30. Slot A: `f-sk2` on `f-dp1` rectangle x 10..20, y -1..51; `f-ext2` distance 5, reverse, op cut. Slot B: `f-sk3` on `f-dp1` rectangle x 50..60, y -1..51; `f-ext3` the same. Pieces `#1` x 0..10, `#2#1` x 20..50, `#2#2` x 60..80. `f-cha1` chamfer equal 1 on `[{q: edge(f-ext1/end-cap#1|f-ext1/lateral/e-1), fp: line|n=1,0,0|c=5,0,30|l=10|adj=2,2}]`. `f-fil1` fillet radius 1 on `[{q: edge(f-ext1/end-cap#2#1|f-ext1/lateral/e-3), fp: line|n=1,0,0|c=35,50,30|l=30|adj=2,2}, {q: edge(f-ext1/end-cap#2#2|f-ext1/lateral/e-3), fp: line|n=1,0,0|c=70,50,30|l=20|adj=2,2}]`.
- **Reference:** `f-cha1` edges[0] (front top edge of the leftmost piece); `f-fil1` edges[0] and [1] (back top edges of the middle and right pieces).
- **Edit:** move `f-sk3` and `f-ext3` above `f-sk2` (B then A). The geometry is identical; the names become `#1#1` x 0..10, `#1#2` x 20..50, `#2` x 60..80.
- **Expect:** `f-cha1` chamfers only x 0..10 (centroid x 5); `f-fil1` rounds only the back edges x 20..50 and 60..80; all ok; volume equal to the volume before the reorder within 1e-9 relative.
- **Today:** step 2 takes both `#1#1` and `#1#2` as split descendants of `#1`, so `f-cha1` also chamfers x 20..50, with confidence 1: a silent change from reordering two independent features. `f-fil1` passes.

### R55 A sketch on a boss top moved above the boss

- **Setup:** bodies `[b-1]`. `f-box1` box 40 x 40 x 10 (x -20..20, y -20..20, z 0..10). `f-dp1` datum XY offset 10. `f-sk1` on `f-dp1`: circle `e-5` center (0,0) radius 8. `f-ext1` extrude distance 10, op join (boss z 10..20). Face sketch `f-sk2` on `{q: f-ext1/end-cap, fp: plane|n=0,0,1|c=0,0,20|a=201.06193|adj=1,1}`, fallback origin (0,0,20), x_axis (1,0,0), normal (0,0,1), with point `e-p1` (0,0). `f-hole1` hole diameter 4, depth 15, points `[e-p1]`.
- **Reference:** `f-sk2` plane.face: the boss top.
- **Edit:** drag `f-ext1` to the end of the timeline.
- **Expect:** `f-sk2` warns that its face (the top of 'Extrude 1') is made by a feature that now comes after it and offers to move the sketch below 'Extrude 1'. The part does not silently gain a closed internal void; if one results, it is reported, also by the print check.
- **Today:** the sketch falls back to z=20 with "can no longer be found", the hole cuts z 5..10 into the box, the boss then caps it, and the part has a hidden void.

## F. Op, body and direction changes

### R30 A join changed to a cut must not re-target its rim fillet

- **Setup:** (a) BLOCK. `f-dp1` datum XY offset 30. `f-sk2` on `f-dp1`: circle `e-5` center (40,25) radius 10. `f-ext2` extrude symmetric 40, op join (boss up to z=50). `f-fil1` fillet radius 2 on `[{q: edge(f-ext2/end-cap|f-ext2/lateral/e-5), fp: circle|n=0,0,1|c=40,25,50|l=62.831853|r=10|adj=2,1}]`. (b) bodies `[b-1, b-2]`. CUBE20 in `b-1`. `f-box2` box 10 x 10 x 10 at position (0,0,14.5) in `b-2`, op new. `f-bool1` boolean target `b-1`, tools `[b-2]`, op join. `f-fil1` fillet on `b-1` radius 1 on `[{q: edge(f-box2/front|f-box2/top), fp: line|n=1,0,0|c=0,-5,24.5|l=10|adj=2,2}]`.
- **Reference:** `f-fil1` edges[0]: the boss's top rim (a), the top front edge of the boss (b).
- **Edit:** (a) change `f-ext2` op from join to cut (a pocket z 10..30). (b) change `f-bool1` op from join to cut (a pocket z 14.5..20).
- **Expect:** `f-fil1` error, nothing rounded, volume (a) 113716.814693, (b) 7450. The message names the element (the top rim of 'Extrude 2'; the top front edge of 'Box 2') and the cause ('Extrude 2' now cuts instead of joining; 'Boolean 1' now cuts 'Body 2' away), with repair data. Also: `edge(f-ext1/end-cap|f-ext2/lateral/e-5)` keeps its name across the op change, so a reference to that rim stays exact.
- **Today:** the fingerprint step rounds the pocket's opening ((a) 0.9046, (b) 0.859), status ok, no warning.

### R31 A split that moves the referenced edge to the other body

- **Setup:** bodies `[b-1, b-2]`, CUBE20 in `b-1`. (a) `f-cha1` chamfer on `b-1` equal 1 on `[{q: edge(f-box1/bottom|f-box1/front), fp: line|n=1,0,0|c=0,-10,0|l=20|adj=2,2}]`. (b) `f-dp1` datum XY offset 10; `f-split1` split_body of `b-1` by `f-dp1`, target `b-2` (`b-1` keeps z 10..20); `f-fil1` fillet on `b-1` radius 2 on `[{q: edge(f-box1/front#2|f-box1/top), fp: line|n=1,0,0|c=0,-10,20|l=20|adj=2,2}]`.
- **Reference:** (a) the bottom front edge; (b) the top front edge of the upper part.
- **Edit:** (a) insert before `f-cha1`: `f-dp1` datum XY offset 5 and `f-split1` split_body of `b-1` by `f-dp1`, target `b-2` (`b-1` keeps z 5..20). (b) set `f-split1` flip to true (`b-1` keeps z 0..10).
- **Expect:** nothing in `b-1` is beveled or rounded. (a) `f-cha1` error: the edge is now in 'Body 2' because 'Split body 1' put that side there; re-pick; `b-1` volume 6000. (b) `f-fil1` error: the edge is now in 'Body 2' because 'Split body 1' keeps the other side; one-click "Use Body 2", where the name resolves exactly; `b-1` volume 4000.
- **Today:** the fingerprint step picks `b-1`'s cut edge ((a) at z=5, (b) at z=10, both 0.914) and bevels or rounds it silently.

### R56 An edge that now lives in another body

- **Setup:** (a, b) bodies `[b-1, b-2]`. CUBE20 in `b-1`. `f-box2` box 10 x 10 x 10 at position (0,0,20) in `b-2`, op new (on top). `f-bool1` boolean target `b-1`, tools `[b-2]`, op join. `f-fil1` fillet on `b-1` radius 1 on `[{q: edge(f-box2/front|f-box2/top), fp: line|n=1,0,0|c=0,-5,30|l=10|adj=2,2}]`. (c) bodies `[b-1, b-2]`. `f-box1` box 20 x 20 x 10 in `b-1`. `f-dp1` datum XY offset 10. `f-sk1` on `f-dp1`: circle `e-5` center (0,0) radius 5. `f-ext1` extrude body `b-1`, distance 10, op join. `f-fil1` fillet on `b-1` radius 1 on `[{q: edge(f-box1/top|f-ext1/lateral/e-5), fp: circle|n=0,0,1|c=0,0,10|l=31.415927|r=5|adj=2,1}]`.
- **Reference:** `f-fil1` edges[0]: (a, b) the small box's top front edge; (c) the ring where the boss meets the box.
- **Edit:** (a) suppress `f-bool1`. (b) swap target and tools: target `b-2`, tools `[b-1]`. (c) change `f-ext1` to body `b-2`, op new.
- **Expect:** no other edge of `b-1` is rounded. (a) `f-fil1` error: the edge belongs to 'Box 2' in 'Body 2' while 'Boolean 1' is suppressed; actions "Unsuppress Boolean 1" or pick an edge of 'Body 1'; volume 8000. (b) `f-fil1` error: 'Body 1' is empty here because 'Boolean 1' now uses it as a tool, and the edge is now in 'Body 2'; one-click "Use Body 2", after which `f-fil1` resolves exactly and `b-2` has volume 8997.853982. (c) `f-fil1` error: 'Extrude 1' now makes the separate body 'Body 2', so the edge is no longer in 'Body 1'; offers to undo the op change or re-pick.
- **Today:** (a) and (c) say only "can no longer be found"; (b) says `body_empty` with no cause.

### R57 A revolve becomes a full turn

- **Setup:** bodies `[b-1]`. `f-sk1` on XZ: `e-1` (10,0)-(20,0), `e-2` (20,0)-(20,10), `e-3` (20,10)-(10,10), `e-4` (10,10)-(10,0). `f-rev1` revolve of that region about the origin Z axis, angle 270, op new. `f-cha1` chamfer equal 1 on `[{q: edge(f-rev1/end-cap|f-rev1/lateral/e-3), fp: line|n=0,1,0|c=0,-15,10|l=10|adj=2,2}]`. Face sketch `f-sk2` on `{q: f-rev1/start-cap, fp: plane|n=0,-1,0|c=15,0,5|a=100|adj=4,4}`, fallback origin (15,0,5), x_axis (1,0,0), normal (0,-1,0), with circle `e-11` center (0,0) radius 2; `f-ext2` extrude distance 3, reverse, op cut.
- **Reference:** `f-cha1` edges[0] (top edge of the end face); `f-sk2` plane.face (the start face).
- **Edit:** set `f-rev1` to a full turn; then back to 270.
- **Expect:** the caps no longer exist. `f-cha1` error naming the top edge of the end face of 'Revolve 1' and the cause ('Revolve 1' now turns all the way round, so it has no end faces), suggesting an angle below 360 or a re-pick; no match onto a seam or circle edge. `f-sk2` keeps its last placement (origin (15,0,5), normal (0,-1,0)) with a `plane_fallback` warning naming the same cause and a one-click re-attach; `f-ext2` still cuts there. Setting 270 again restores the original result.
- **Today:** generic messages; the sketch lands right only because its attach-time fallback happens to be current.

## G. Sketches and datums on faces

### R32 A sketch keeps its place on a face that moves or turns over

- **Setup:** (a) bodies `[b-1]`. `f-box1` box 40 x 40 x 10 (x -20..20, y -20..20, z 0..10). Face sketch `f-sk2` on `{q: f-box1/top, fp: plane|n=0,0,1|c=0,0,10|a=1600|adj=4,4}`, fallback origin (0,0,10), x_axis (1,0,0), normal (0,0,1), with point `e-p1` (15,15). `f-hole1` hole on `b-1`, sketch `f-sk2`, points `[e-p1]`, diameter 4, through all. (b) BLOCK. Face sketch `f-sk2` on `{q: f-ext1/end-cap, fp: plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4}`, fallback origin (0,0,30), x_axis (1,0,0), normal (0,0,1), with point `e-p1` (20,10). `f-hole1` diameter 5, depth 10, points `[e-p1]`.
- **Reference:** `f-sk2` plane.face, and with it where `e-p1` sits relative to the face's edges.
- **Edit:** (a) insert `f-mv1` move_body of `b-1`, translation (100,0,0), between `f-box1` and `f-sk2`; variant: a rotation of 90 about the origin Z axis instead. (b) set `f-ext1` reverse to true (the end cap is now the bottom at z=-30, facing -Z).
- **Expect:** exact face match, no warnings, and the point keeps its place on the face. (a) the hole axis is at world x=115, y=15; volume 15874.336294; rotation variant: the hole is at world (-15,15). (b) the hole axis is at world x=20, y=10, drilled from z=-30 up to z=-20; volume 119803.650459.
- **Today:** the placement takes its origin and x axis from the stored fallback (`Placement::on_plane` projects the hint), so only the plane follows: (a) the hole stays at (15,15), outside the moved box: `hole_misses_body` and `no_effect`; rotation variant: the hole stays at world (15,15), another corner of the face. (b) the flipped normal turns the sketch's y axis to -Y, `e-p1` maps to (20,-10,-30), off the face: `hole_misses_body` and `no_effect`.

### R33 A sketch whose face is cut away must not jump to its attach-time position

- **Setup:** bodies `[b-1]`. `f-box1` box 20 x 20 x 40 (x -10..10, y -10..10, z 0..40). Face sketch `f-sk2` on `{q: f-box1/top, fp: plane|n=0,0,1|c=0,0,10|a=400|adj=4,4}`, fallback origin (0,0,10), x_axis (1,0,0), normal (0,0,1) (attached when the box was 10 tall; the height was then edited to 40 and the sketch followed by name), with circle `e-5` center (0,0) radius 3; `f-ext2` extrude distance 5, op join (boss z 40..45).
- **Reference:** `f-sk2` plane.face: the top of the box.
- **Edit:** insert before `f-sk2`: `f-dp1` datum XY offset 35, `f-sk3` on `f-dp1` with a rectangle x -15..15, y -15..15, and `f-ext3` extrude distance 10, op cut (removes z 35..40; the box top is deleted and the new top is `f-ext3/start-cap` at z=35). After the insert the cut is labeled 'Extrude 1'.
- **Expect:** the sketch does not move to the attach-time fallback (z=10). It keeps the placement of its last successful build (origin (0,0,40)), and its warning names the face (the top of 'Box 1') and the cause ('Extrude 1', inserted before this sketch, cut it away), and offers the new top at z=35 as a one-click re-attach. The boss is never inside the box: bounding box max z 45. How the last placement survives closing and reopening the document is decided in features/sketch.md.
- **Today:** the face is missing (best 0.72 against the stale z=10 fingerprint), the sketch goes to z=10, the boss is joined inside the box with only `no_effect`, and it vanishes.

### R34 A shell inserted under a sketch must not drop it onto the floor

- **Setup:** bodies `[b-1]`. `f-box1` box 200 x 200 x 30 (x -100..100, y -100..100, z 0..30). Face sketch `f-sk2` on `{q: f-box1/top, fp: plane|n=0,0,1|c=0,0,30|a=40000|adj=4,4}`, fallback origin (0,0,30), x_axis (1,0,0), normal (0,0,1), with circle `e-1` center (0,0) radius 10; `f-ext2` extrude of region `{key: e-1, fp: c=0,0|a=314.159265}`, distance 10, op join (boss z 30..40).
- **Reference:** `f-sk2` plane.face: the top.
- **Edit:** insert before `f-sk2` a shell `f-sh1` thickness 2 removing `f-box1/top` (an open tray).
- **Expect:** the sketch stays in the plane z=30 (origin (0,0,30), normal +Z): on the rim `f-sh1/rim/f-box1/top`, the top's history descendant in the same plane, or at its last placement with a `plane_fallback` warning naming 'Shell 1'. The boss is at z 30..40, bounding box max z 40.
- **Today:** step 4 matches the tray floor `f-sh1/inner/f-box1/bottom` at z=2 (0.940; the rim 0.731), the sketch silently drops to z=2, and the boss is built inside the tray (bounding box max z 30).

### R35 A slot splits the face a sketch is on

- **Setup:** BLOCK. Face sketch `f-sk3` on `{q: f-ext1/end-cap, fp: plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4}`, fallback origin (0,0,30), x_axis (1,0,0), normal (0,0,1), with point `e-p1` (20,25) and circle `e-6` center (65,25) radius 5. `f-hole1` hole on `b-1`, sketch `f-sk3`, points `[e-p1]`, diameter 6, depth 10. `f-ext3` extrude of region `e-6`, distance 10, op join.
- **Reference:** `f-sk3` plane.face: the top.
- **Edit:** insert SLOT(30,50) (`f-dp1`, `f-sk2`, `f-ext2`) before `f-sk3`: the top splits into `f-ext1/end-cap#1` (x 0..30) and `#2` (x 50..80).
- **Expect:** the face resolves by history to both pieces, which share one plane: origin (0,0,30), x_axis (1,0,0), normal (0,0,1), no warning. `f-hole1/bore/e-p1` is a radius 3 cylinder about x=20, y=25, z 20..30, with `f-hole1/bottom/e-p1` at z=20; the boss stands on piece `#2` at (65,25), z 30..40; volume 115502.654825.
- **Today:** guard.

### R36 A sketch on the inside floor follows a shell thickness change

- **Setup:** bodies `[b-1]`. `f-box1` box 80 x 50 x 30 (x -40..40, y -25..25, z 0..30). `f-sh1` shell thickness 2, remove_faces `[{q: f-box1/top, fp: plane|n=0,0,1|c=0,0,30|a=4000|adj=4,4}]`. Face sketch `f-sk2` on `{q: f-sh1/inner/f-box1/bottom, fp: plane|n=0,0,1|c=0,0,2|a=3496|adj=4,4}`, fallback origin (0,0,2), x_axis (1,0,0), normal (0,0,1), with circle `e-5` center (20,10) radius 3; `f-ext2` extrude distance 8, op join (a standoff).
- **Reference:** `f-sk2` plane.face: the inside floor.
- **Edit:** change `f-sh1` thickness from 2 to 3; separately, change the removed face from the top to the front.
- **Expect:** exact match, no warnings. Thickness 3: sketch origin (0,0,3), the standoff at (20,10) from z=3 to z=11, joined to the floor (one solid). Front removed: the floor keeps its name and the standoff its place.
- **Today:** guard.

### R37 A datum on a chamfer face survives an upstream merge

- **Setup:** CUBE20. `f-cha1` chamfer equal 2 on `[{q: edge(f-box1/front|f-box1/top), fp: line|n=1,0,0|c=0,-10,20|l=20|adj=2,2}]`. `f-dp1` datum_plane on face `{q: f-cha1/chamfer/edge(f-box1/front|f-box1/top), fp: plane|n=0,-0.707107,0.707107|c=0,-9,19|a=56.568542|adj=4,4}`, offset 0: origin (0,-14,14), normal (0,-0.707107,0.707107), x_axis (1,0,0).
- **Reference:** `f-dp1` base.face: the chamfer face.
- **Edit:** insert before `f-cha1` a box `f-a1` 20 x 20 x 20 at position (20,0,0), op join (the block spans x -10..30). The fronts and tops merge as `f-a1/front` and `f-a1/top`; the chamfer resolves its edge through the aliases to `edge(f-a1/front|f-a1/top)` and names its face `f-cha1/chamfer/edge(f-a1/front|f-a1/top)`.
- **Expect:** `f-dp1` finds the same chamfer face, only longer (area 113.137085, centroid (10,-9,19)); its placement is unchanged; no error. The same holds for any generated name whose generator was merged, for example a fillet face after a boss wall is moved flush with the base.
- **Today:** aliases are chased only for whole face names, not for names nested inside generated names, and the fingerprint scores 0.603: `f-dp1` fails and everything on it reports `upstream_failed`.

### R58 A datum on a face that an inserted shell removes

- **Setup:** bodies `[b-1, b-2]`. `f-box1` box 80 x 50 x 30 in `b-1` (x -40..40, y -25..25, z 0..30). `f-dp1` datum_plane on face `{q: f-box1/top, fp: plane|n=0,0,1|c=0,0,30|a=4000|adj=4,4}`, offset -2 (z=28). `f-split1` split_body of `b-1` by `f-dp1`, target `b-2` (`b-1` keeps the 2 mm lid, z 28..30).
- **Reference:** `f-dp1` base.face: the top, which sets the lid line.
- **Edit:** insert a shell `f-sh1` thickness 2 removing `f-box1/top` between `f-box1` and `f-dp1`.
- **Expect:** the top is removed, but its rim `f-sh1/rim/f-box1/top` lies in the same plane and is its history descendant: the datum follows the rim (still z=28) with a notice naming 'Shell 1', and the split still separates a 2 mm ring of wall tops (z 28..30) from the open box.
- **Today:** the rim scores 0.75, `f-dp1` fails with "can no longer be found", and `f-split1` reports `upstream_failed`.

## H. Axes and directions

### R38 A pattern direction from an edge must not flip after a 1 mm tweak

- **Setup:** bodies `[b-1]`. `f-sk1` on XY: `e-1` (0,0)-(40,0), `e-2` (40,0)-(40,20), `e-3` (40,20)-(0,20), `e-4` (0,20)-(0,0). `f-ext1` extrude distance 10, op new. `f-lp1` linear_pattern of `b-1`, direction `{axis: {kind: edge, body: b-1, edge: {q: edge(f-ext1/lateral/e-4|f-ext1/start-cap), fp: line|n=0,1,0|c=0,10,0|l=20|adj=2,2}}, count: 2, spacing: 30}`, op join (copy at y 30..50).
- **Reference:** `f-lp1` direction.axis: the left wall's bottom edge.
- **Edit:** move the shared point of `e-3`'s end and `e-4`'s start from (0,20) to (-1,20) (the left wall leans 1 mm).
- **Expect:** the edge resolves exactly and the pattern keeps the side of the stored direction (n=0,1,0): direction (-0.049938, 0.998752, 0), copy offset (-1.498129, 29.962570, 0), bounding box [-2.498129,0,0]-[40,49.962570,10]. Leaning the wall to (1,20) also keeps the copy on +Y.
- **Today:** the axis direction is canonicalized so its first nonzero component is positive (features.md section 3.6), giving (0.049938, -0.998752, 0): the copy jumps to the other side (bounding box [-1,-29.962570,0]-[41.498129,20,10]). Revolve, circular pattern and move-body rotation about an edge or a round face use the same rule, so a small edit can reverse a revolve or a rotation.

### R39 Axes on edges and faces that an upstream edit splits or shortens

- **Setup:** (a) bodies `[b-1, b-2]`. `f-box1` box 10 x 10 x 10 at position (0,5,0) in `b-2` (x -5..5, y 0..10). `f-sk1` on XZ: `e-1` (10,0)-(20,0), `e-2` (20,0)-(20,30), `e-3` (20,30)-(10,30), `e-4` (10,30)-(10,0). `f-rev1` revolve into `b-1` of region `{key: e-1,e-2,e-3,e-4, fp: c=15,15|a=300}`, full, op new, about `{kind: edge, body: b-2, edge: {q: edge(f-box1/front|f-box1/left), fp: line|n=0,0,1|c=-5,0,5|l=10|adj=2,2}}` (revolve.md test 16: volume 37699.111843, 4 faces, bounding box [-30,-25,0]-[20,25,30]). (b) bodies `[b-1]`. `f-cyl1` cylinder radius 10, height 50. `f-sk2` on XZ: rectangle x 8..12, z 20..24. `f-rev1` revolve of it about `{kind: face, body: b-1, face: {q: f-cyl1/side, fp: cylinder|n=0,0,1|c=0,0,25|a=3141.592654|r=10|adj=2,3}}`, full, op cut (a groove). (c) CUBE10. `f-mv1` move_body of `b-1`, rotation about `{kind: edge, body: b-1, edge: {q: edge(f-box1/right|f-box1/top), fp: line|n=0,1,0|c=5,0,10|l=10|adj=2,2}}`, angle 180 (move_body.md test 4: bounding box [5,-5,10]-[15,5,20]).
- **Reference:** (a) `f-rev1` axis edge; (b) `f-rev1` axis face; (c) `f-mv1` rotation axis edge.
- **Edit:** (a) insert before `f-rev1` a box `f-box3` 4 x 4 x 2 at position (-5,0,4) in `b-2`, op cut (a notch that splits the corner edge into `#1` z 0..4 and `#2` z 6..10). (b) insert before `f-sk2` a sketch `f-sk3` on XZ with a rectangle x 8..12, z 10..12 and `f-rev2`, a full revolve of it about the origin Z axis, op cut (the side splits into `f-cyl1/side#1` and `#2`). (c) insert before `f-mv1` a fillet radius 1 on `edge(f-box1/front|f-box1/right)` and `edge(f-box1/back|f-box1/right)` (the hinge shortens to y -4..4).
- **Expect:** the same axis line, no warnings: (a) resolves by history to both collinear pieces, axis x=-5, y=0 along +Z; `b-1` unchanged (volume 37699.111843, 4 faces, same bounding box). (b) resolves to both pieces, axis Z through the origin; the groove stays at z 20..24. (c) exact match, axis x=5, z=10 along +Y; bounding box [5,-5,10]-[15,5,20].
- **Today:** guard.

### R59 The edge used as an axis is rounded upstream

- **Setup:** (a) the setup of R39 (a). (b) bodies `[b-1, b-2]`. CUBE20 in `b-1`. `f-dp1` datum on XZ offset 10 (the plane y=-10). `f-sk1` on `f-dp1`: rectangle u -16..-12, v 0..20. `f-rev1` revolve of it into `b-2`, angle 90, op new, about `{kind: edge, body: b-1, edge: {q: edge(f-box1/front|f-box1/left), fp: line|n=0,0,1|c=-10,-10,10|l=20|adj=2,2}}`.
- **Reference:** `f-rev1` axis edge: a vertical box corner.
- **Edit:** insert before `f-rev1` a fillet `f-fil1` on the axis edge, radius 1 in (a), radius 2 in (b).
- **Expect:** no straight edge is left at the corner: `f-rev1` error and its body stays empty. The message says the edge used as the axis was rounded by 'Fillet 1', inserted before this revolve, and suggests a sketch line, an origin axis, or one of the fillet's two straight tangent edges (listed in the repair data); no axis is guessed.
- **Today:** both tangent edges score 0.963, so the error says the edge "now matches 2 edges", which suggests it still exists.

### R60 A mirror plane on a face that a groove splits

- **Setup:** bodies `[b-1]`. `f-box1` box 10 x 10 x 10 at position (10,0,0) (x 5..15). `f-mir1` mirror of `b-1` across `{kind: face, body: b-1, face: {q: f-box1/right, fp: plane|n=1,0,0|c=15,0,5|a=100|adj=4,4}}`, op join (mirror.md test 9: volume 2000, bounding box [5,-5,0]-[25,5,10]).
- **Reference:** `f-mir1` plane: the right face.
- **Edit:** insert before `f-mir1` a box `f-box2` 4 x 20 x 2 at position (15,0,4), op cut (a groove x 13..17 through Y at z 4..6, which splits the right face into `f-box1/right#1` and `#2`).
- **Expect:** the plane resolves by history to both pieces: x=15, normal +X. One solid, volume 1920, bounding box [5,-5,0]-[25,5,10], with a 4 x 2 tunnel along Y at x 13..17, z 4..6; no warning.
- **Today:** guard.

## I. Patterns and mirror

### R40 Pattern count or directions change: references stay on their copy

- **Setup:** (a, b) CUBE10. `f-lp1` linear_pattern of `b-1` along origin X, count 3, spacing 20, op join (copies at x 15..25 and 35..45, separate solids). `f-fil1` fillet radius 1 on `[{q: edge(f-box1/front@f-lp1:2|f-box1/top@f-lp1:2), fp: line|n=1,0,0|c=40,-5,10|l=10|adj=2,2}]`. `f-cha1` chamfer equal 1 on `[{q: edge(f-box1/front@f-lp1:1|f-box1/top@f-lp1:1), fp: line|n=1,0,0|c=20,-5,10|l=10|adj=2,2}]`. (c) CUBE10. `f-lp1` along origin X count 3 spacing 20, second_direction along origin Y count 2 spacing 15, op join (6 cubes). `f-fil1` fillet radius 1 on `[{q: edge(f-box1/front@f-lp1:2.1|f-box1/top@f-lp1:2.1), fp: line|n=1,0,0|c=40,10,10|l=10|adj=2,2}]`. (d) bodies `[b-1]`. `f-box1` box 4 x 4 x 5 at position (20,0,0). `f-cp1` circular_pattern about origin Z, count 6, full turn, op join. `f-cha1` chamfer equal 0.5 on `[{q: edge(f-box1/right@f-cp1:3|f-box1/top@f-cp1:3), fp: line|n=0,1,0|c=-22,0,5|l=4|adj=2,2}]` (outer top edge of copy 3, at 180).
- **Reference:** edges of particular copies.
- **Edit:** (a) count 3 to 5. (b) add second_direction along origin Y, count 2, spacing 15. (c) first count 3 to 4. (d) count 6 to 8.
- **Expect:** exact matches, no warnings. (a) the fillet is on x 35..45 (axis y=-4, z=9), the chamfer on x 15..25; the new copies stay sharp; 5 solids, volume 4992.853982. (b) instances (i,0) keep their one-index names, so the same edges are used; 6 solids, volume 5992.853982. (c) the fillet stays on grid position (2,1): axis y=11, z=9, x 35..45; 8 solids, volume 7997.853982 (a linear instance index would move it to another cube). (d) copies are identified by number (circular_pattern.md section 3.4), so copy 3 is now at 135 and keeps the chamfer (chamfer face centroid about (-15.4,15.4,4.8)); the block now at 180 stays sharp. If the product decides that position should win, the rule goes into naming.md first and this variant pins it.
- **Today:** guard.

### R41 Lowering a pattern count says which copy is gone

- **Setup:** CUBE10. `f-lp1` linear_pattern along origin X, count 3, spacing 20, op join. `f-fil1` fillet radius 1 on `[{q: edge(f-box1/right@f-lp1:1|f-box1/top@f-lp1:1), fp: line|n=0,1,0|c=25,0,10|l=10|adj=2,2}, {q: edge(f-box1/right@f-lp1:2|f-box1/top@f-lp1:2), fp: line|n=0,1,0|c=45,0,10|l=10|adj=2,2}]`. Variant: only the second reference.
- **Reference:** `f-fil1` edges: the top right edges of the second and third cube.
- **Edit:** count 3 to 2.
- **Expect:** edges[0] resolves exactly; edges[1] is lost and is not moved to another copy (nearest 0.664). `f-fil1` warning `reference_missing` at `/edges/1` whose message says the edge belonged to the third copy of 'Linear pattern 1', which no longer exists because Count changed from 3 to 2, with "Set Count back to 3" as a one-click fix and re-pick; one fillet face (axis x=24, z=9); volume 1997.853982. Variant: error with the same message, volume 2000.
- **Today:** the right outcome, but the message says only "can no longer be found".

### R61 Separate solids fuse, and come apart again

- **Setup:** (a) CUBE10. `f-lp1` along origin X, count 3, spacing 20, op join. `f-fil1` fillet radius 1 on `[{q: edge(f-box1/front@f-lp1:2|f-box1/top@f-lp1:2), fp: line|n=1,0,0|c=40,-5,10|l=10|adj=2,2}]`. (b) bodies `[b-1]`. `f-box1` box 20 x 20 x 20 at position (15,0,0) (x 5..25). `f-mir1` mirror of `b-1` across origin YZ, op join (image x -25..-5). `f-fil1` fillet radius 2 on `[{q: edge(f-box1/front@f-mir1:1|f-box1/top@f-mir1:1), fp: line|n=1,0,0|c=-15,-10,20|l=20|adj=2,2}]`. (c) CUBE20. `f-box2` box 20 x 20 x 20 at position (30,0,0), op join (x 20..40). `f-fil1` fillet radius 2 on `[{q: edge(f-box1/front|f-box1/top), fp: line|n=1,0,0|c=0,-10,20|l=20|adj=2,2}, {q: edge(f-box2/front|f-box2/top), fp: line|n=1,0,0|c=30,-10,20|l=20|adj=2,2}]`.
- **Reference:** `f-fil1` edges: (a) the third cube's front top edge; (b) the image's front top edge; (c) both boxes' front top edges.
- **Edit:** (a) spacing 20 to 10 (the cubes fuse, naming.md test 89), then back to 20. (b) `f-box1` position x 15 to 10 (source and image touch at x=0 and fuse), then back. (c) `f-box2` position x 30 to 20 (the boxes fuse).
- **Expect:** (a) after fusing, the reference resolves through the aliases to the merged edge (x -5..25) and builds, volume 2993.561945, with a notice that the picked edge merged with its neighbors; after spacing 20 again only the third cube's edge is rounded (volume 2997.853982) and the stored reference is byte-identical after both edits. A reference to the third cube's outer right top edge follows the same way to `edge(f-box1/right@f-lp1:2|f-box1/top)`. (b) resolves through the aliases to the merged front top edge (x -20..20), with a notice that the picked edge merged with its mirror image; moving back restores the original result. (c) both references resolve to `edge(f-box1/front|f-box1/top)`, which is used once: one fillet face, area 125.663706; one solid, volume 15965.663706; no warnings, because the user picked both parts.
- **Today:** (a) and (b) resolve with no notice; (c) passes.

## J. Holes

### R42 Hole points added, reordered or moved

- **Setup:** BOX-BELOW. `f-sk1` on XY with points `e-p1` (0,0) and `e-p2` (10,10). `f-hole1` hole on `b-1`, sketch `f-sk1`, points `[e-p1]`, diameter 6, through all. `f-cha1` chamfer equal 1 on `[{q: edge(f-box1/top|f-hole1/bore/e-p1), fp: circle|n=0,0,1|c=0,0,0|l=18.849556|r=3|adj=2,1}]`.
- **Reference:** `f-cha1` edges[0]: the top rim of the hole at `e-p1`.
- **Edit:** (a) points `[e-p2, e-p1]` (a second hole listed first). (b) add point `e-p4` (-10,10) and use `[e-p4, e-p1, e-p2]`. (c) after (a), swap the coordinates: `e-p1` to (10,10), `e-p2` to (0,0). (d) after (a), move `e-p1` to (2,-2).
- **Expect:** exact match by point, no warnings; the chamfer is a cone from radius 4 at z=0 to radius 3 at z=-1 about `e-p1`'s axis: (a) and (b) x=0, y=0; (c) x=10, y=10 (the perfect lookalike now at (0,0) is ignored, because the name is not order dependent); (d) x=2, y=-2. Every other hole's rim stays sharp.
- **Today:** guard.

### R43 A hole point removed: the chamfer must not jump to the neighbor

- **Setup:** BOX-BELOW. `f-sk1` on XY with points `e-p1` (0,0) and `e-p2` (7,0). `f-hole1` points `[e-p1, e-p2]`, diameter 6, through all. `f-cha1` chamfer equal 0.5 on `[{q: edge(f-box1/top|f-hole1/bore/e-p2), fp: circle|n=0,0,1|c=7,0,0|l=18.849556|r=3|adj=2,1}]`.
- **Reference:** `f-cha1` edges[0]: the top rim of the hole at (7,0).
- **Edit:** (a) set `f-hole1` points to `[e-p1]`. (b) delete point `e-p2` from `f-sk1` instead.
- **Expect:** no chamfer face; the hole at (0,0) stays; volume 15717.256661. `f-cha1` error: the edge was the top edge of the hole at (7, 0), which 'Hole 1' no longer makes because that point was removed from its Points (a) or deleted from 'Sketch 1' (b), with actions to re-pick or delete the chamfer. In (b) `f-hole1` still cuts the hole at (0,0) and warns that a point it used was deleted from 'Sketch 1' (repair: re-pick or remove it from Points). A fingerprint match never crosses to another generator of the same feature.
- **Today:** (a) step 4 scores `e-p1`'s top rim 0.891 (its bottom rim 0.833) and chamfers the other hole silently. (b) the whole hole fails with `unknown_entity`, so the body has no holes, and `f-cha1` fails too: two errors for one deleted point.

### R62 A channel splits a hole's bore

- **Setup:** BOX-BELOW. `f-sk1` on XY with point `e-p1` (0,0). `f-hole1` diameter 6, through all, points `[e-p1]`. `f-cha1` chamfer equal 1 on `[{q: edge(f-box1/top|f-hole1/bore/e-p1), fp: circle|n=0,0,1|c=0,0,0|l=18.849556|r=3|adj=2,1}]`.
- **Reference:** `f-cha1` edges[0]: the top rim of the hole.
- **Edit:** insert before `f-cha1`: `f-sk2` on XZ with a rectangle u (x) -25..25, v (z) -6..-4, and `f-ext2` extrude symmetric 10, op cut (a channel that cuts the bore into `f-hole1/bore/e-p1#1`, z -10..-6, and `#2`, z -4..0).
- **Expect:** the reference resolves by history to `edge(f-box1/top|f-hole1/bore/e-p1#2)` only: the chamfer is on the top rim (cone from radius 4 at z=0 to radius 3 at z=-1), the channel edges of both bore pieces stay sharp, no warnings.
- **Today:** guard.

### R63 Changing the hole type

- **Setup:** (a) BOX-BELOW. `f-sk1` on XY with points `e-p1` (0,0) and `e-p2` (10,10). `f-hole1` points `[e-p1, e-p2]`, diameter 6, through all. `f-cha1` chamfer equal 0.5 on `[{q: edge(f-box1/top|f-hole1/bore/e-p2), fp: circle|n=0,0,1|c=10,10,0|l=18.849556|r=3|adj=2,1}]`. (b) BOX-BELOW. `f-sk1` with point `e-p1` (0,0). `f-hole1` at `[e-p1]`, diameter 6, counterbore diameter 11 depth 4, through all (hole.md test 4). `f-cha1` chamfer equal 0.5 on `[{q: edge(f-box1/top|f-hole1/counterbore/e-p1), fp: circle|n=0,0,1|c=0,0,0|l=34.557519|r=5.5|adj=2,1}]`. Variant of (b): diameter 3.
- **Reference:** `f-cha1` edges[0]: the hole's top edge (its mouth).
- **Edit:** (a) set hole_type to counterbore, diameter 11, depth 4. (b) set hole_type to simple.
- **Expect:** no silent move. (a) is ambiguous by meaning: `f-cha1` reports that the top edge of the hole at (10, 10) changed because 'Hole 1' is now a counterbore and offers the new mouth `edge(f-box1/top|f-hole1/counterbore/e-p2)` (radius 5.5, z=0) in one click; the counterbore floor's inner edge at z=-4 is never chamfered. (b) follows to the same hole's mouth `edge(f-box1/top|f-hole1/bore/e-p1)` with a notice that the mouth changed: a cone from radius 3.5 to 3 (diameter 3: from 2 to 1.5) between z=0 and z=-0.5, for both diameters, because the match stays within the same hole, point and top face.
- **Today:** (a) the fingerprint step picks the counterbore floor's inner edge at z=-4 (0.933 against 0.864 for the new mouth) and chamfers it, status ok. (b) follows by fingerprint luck (0.864 against 0.776); with diameter 3 the score is 0.782 and the reference breaks.

## K. Shell

### R44 Fillets inserted before a shell

- **Setup:** bodies `[b-1]`. `f-box1` box 80 x 50 x 30 (x -40..40, y -25..25, z 0..30). `f-sh1` shell thickness 2, remove_faces `[{q: f-box1/top, fp: plane|n=0,0,1|c=0,0,30|a=4000|adj=4,4}]`.
- **Reference:** `f-sh1` remove_faces[0]: the top.
- **Edit:** insert before `f-sh1` a fillet `f-fil1` radius 5 on the four vertical edges `edge(f-box1/front|f-box1/left)`, `edge(f-box1/front|f-box1/right)`, `edge(f-box1/back|f-box1/left)`, `edge(f-box1/back|f-box1/right)`.
- **Expect:** the top is modified, not renamed, so the shell resolves it exactly: no face named `f-box1/top`; the rim `f-sh1/rim/f-box1/top` is planar at z=30 with area 490.265482; inner corners such as `f-sh1/inner/f-fil1/fillet/edge(f-box1/front|f-box1/left)` are radius 3 cylinders; bounding box [-40,-25,0]-[40,25,30]; one open solid; no warnings.
- **Today:** guard.

### R64 An inside corner already rounded upstream

- **Setup:** BLOCK. `f-sh1` shell thickness 2 removing `f-ext1/end-cap` (an open box; inner walls x 2..78, y 2..48, floor z=2). `f-fil1` fillet radius 1 on `[{q: edge(f-sh1/inner/f-ext1/lateral/e-1|f-sh1/inner/f-ext1/lateral/e-2), fp: line|n=0,0,1|c=78,2,16|l=28|adj=2,2}]`.
- **Reference:** `f-fil1` edges[0]: the inner vertical corner at the front right.
- **Edit:** in `f-sk1` round the corner between `e-1` and `e-2`: `e-1` becomes (0,0)-(74,0), a new arc `e-5` center (74,6) radius 6 runs from (74,0) to (80,6), `e-2` becomes (80,6)-(80,50). The shell's inner corner becomes an arc of radius 4.
- **Expect:** `f-ext1` and `f-sh1` build (profile followed). `f-fil1` error: the inside corner it rounded is already round because 'Sketch 1' now rounds that corner; suggests deleting 'Fillet 1' or picking another edge; no fillet applied. Not a bare "smooth edge skipped" or "can no longer be found".
- **Today:** the area changes 0.19 percent, so `f-ext1` fails first with `profile_not_found` (as R01); with the profile fixed, the two tangent replacements at (74,2) and (78,6) score 0.948 each and the message is generic.

## L. Imported geometry

### R45 An imported file revised with an extra hole

- **Setup:** asset A, written with `write_step`: a 40 x 20 x 5 plate at x 0..40, y 0..20, z 0..5 with a through hole radius 3 at (30,10). `f-imp1` import of A, op new into `b-1` (faces by geometric order: `#1` left, `#2` bottom, `#3` top, `#4` front, `#5` back, `#6` hole wall, `#7` right). `f-cha1` chamfer equal 0.5 on `[{q: edge(f-imp1/face#3|f-imp1/face#6), fp: circle|n=0,0,1|c=30,10,5|l=18.849556|r=3|adj=2,1}]`.
- **Reference:** `f-cha1` edges[0]: the hole's top rim.
- **Edit:** replace the asset with B: the same plate with a second hole radius 3 at (10,10). The faces renumber (`#2` is the new hole, `#7` the old one); the rim is now `edge(f-imp1/face#5|f-imp1/face#7)`.
- **Expect:** `f-cha1` ok (method history, no warning); the chamfer cone's axis passes through (30,10); the hole at (10,10) has a sharp rim. (The rim scores 1.0, the same hole's bottom rim 0.918, the new hole's top rim 0.771.)
- **Today:** guard.

### R46 An imported file revised without the referenced hole

- **Setup:** (a) as R45. (b) asset A2: the same plate with one through hole radius 3 at (20,10) (faces `#1` left, `#2` front, `#3` bottom, `#4` hole wall, `#5` top, `#6` back, `#7` right). `f-cha1` chamfer equal 0.5 on `[{q: edge(f-imp1/face#4|f-imp1/face#5), fp: circle|n=0,0,1|c=20,10,5|l=18.849556|r=3|adj=2,1}]`.
- **Reference:** `f-cha1` edges[0]: a hole's top rim.
- **Edit:** (a) replace the asset with the plain plate without a hole (faces `#1` left, `#2` front, `#3` bottom, `#4` top, `#5` back, `#6` right, so `edge(f-imp1/face#3|f-imp1/face#6)` now names the straight bottom right edge). (b) replace A2 with a plate with holes radius 3 at (10,10) and (30,10) and none at the center.
- **Expect:** no chamfer anywhere. (a) `f-cha1` error: the round edge it chamfered (the top of a hole at (30, 10, 5)) is not in the new version of the file imported by 'Import 1'; volume 4000. (b) `f-cha1` error saying the edge now matches the top edges of the holes at (10, 10) and (30, 10), with repair data listing those two rims first; volume 3717.256661. A resolution below the confidence threshold never counts as resolved.
- **Today:** (a) step 1 finds the exact name, every family member scores 0 (a circle against lines), and the name is returned as exact with confidence 0: the chamfer cuts a straight edge of the plate. (b) both top rims score 0.856 (bottom rims 0.843), the reference is ambiguous, and the candidates are dropped from the error.

## M. Units and upstream failures

### R47 An upstream sketch conflict must not send a fillet to a lookalike

- **Setup:** BLOCK. `f-sk2` on XY: circle `e-6` center (50,25) radius 10; `f-ext2` extrude through all, op cut (a hole). `f-dp1` datum XY offset 30. `f-sk3` on `f-dp1`: circle `e-5` center (20,25) radius 10 with constraint `c-1` diameter "20 mm"; `f-ext3` extrude distance 10, op join (boss z 30..40). `f-fil1` fillet radius 2 on `[{q: edge(f-ext3/end-cap|f-ext3/lateral/e-5), fp: circle|n=0,0,1|c=20,25,40|l=62.831853|r=10|adj=2,1}]`.
- **Reference:** `f-fil1` edges[0]: the boss's top rim.
- **Edit:** add constraint `c-2` radius "6 mm" on `e-5` (it conflicts with `c-1`).
- **Expect:** `f-sk3` fails with `sketch_conflict` and `f-ext3` with `upstream_failed`. `f-fil1` rounds nothing: its error names 'Sketch 3' (through 'Extrude 3') as the cause and says to fix 'Sketch 3' first, with no re-pick prompt, because re-picking would destroy a reference that is still right. Volume 110575.222039. After removing `c-2`, `f-fil1` resolves exactly and gives the original result.
- **Today:** `f-fil1` runs on the body without the boss, step 4 scores the hole's top rim 0.862 (bottom rim 0.808), and the hole is rounded silently.

### R48 Switching document units with bare numbers

- **Setup:** document units mm. BLOCK with distance "30" and SLOT(30,50) with datum offset "30" and depth "5". `f-fil1` fillet radius "2" on `[{q: edge(f-ext1/end-cap#2|f-ext1/lateral/e-1), fp: line|n=1,0,0|c=65,0,30|l=30|adj=2,2}]`.
- **Reference:** `f-fil1` edges[0]: the front top edge of the right piece.
- **Edit:** set the document units to in. Bare numbers now mean inches (height 762 mm, datum 762 mm, slot depth 127 mm, radius 50.8 mm); sketch coordinates stay millimeters.
- **Expect:** the reference still resolves to the right piece's front top edge (x 50..80, z=762) with no reference warning. `f-fil1` fails with `fillet_too_large`, and the message, besides the limit, says that "2" has no unit and now means 2 in because the document units changed from mm to in, offering "2 mm" (the previous size) as a one-click fix. After applying it, `f-fil1` is ok with a radius 2 mm fillet face `f-fil1/fillet/edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)`.
- **Today:** the message gives only the limit in inches; nothing points at the units change.

## Coverage

Downstream consumers:

- Fillet edges: R01 to R08, R10, R12, R13, R14, R15, R16, R17, R18, R19, R20, R21, R23, R24, R27, R28, R29, R30, R31, R41, R47, R48, R51, R52, R56, R61, R64, R65.
- Chamfer edges, equal distance: R02, R11, R17, R18, R29, R31, R40, R42, R43, R45, R46, R57, R62, R63. Two distances: R22.
- Shell removed faces: R09, R44, R49, R52, R53.
- Sketch on a face: R01, R06, R13, R15, R26, R32, R33, R34, R35, R36, R55, R57.
- Datum plane on a face: R15, R25, R37, R53, R54, R58.
- Extrude profile regions: R01 to R07, R49, R50, R51, R64, R65.
- Revolve axis on an edge: R39, R59; on a round face: R39.
- Mirror plane on a face: R60. Linear pattern direction on an edge: R38. Move body rotation axis on an edge: R39.
- Split body plane through a datum: R15, R31, R58.
- Hole points: R32, R35, R42, R43, R63.
- Boolean target and tools, and the body a feature works on: R30, R31, R56.
- Pattern and mirror copies: R20, R40, R41, R52, R61.
- Imported faces: R45, R46.

Edit types:

- Sketch geometry: lines added (R01, R02, R49), removed (R03, R51), deleted and redrawn (R06, R07), a crossing or inner circle (R04, R50), points moved (R11, R38, R42), overlapping outlines (R05), a corner rounded (R02, R64), entities reordered (R65), a conflicting constraint (R47).
- Dimensions and parameters: R08, R09, R10, R12, R13, R14, R26, R36, R52, R61.
- Features inserted upstream: R15 to R22, R31, R32, R33, R34, R35, R37, R39, R44, R53, R58, R59, R60, R62.
- Suppress and unsuppress: R10, R20, R23, R24, R25, R27, R56. Delete: R10, R23, R26, R54.
- Reorder: R24, R28, R29, R55.
- Op and body changes: R30, R56. Flip and reverse: R31, R32. Extent: R12 (through), R57 (full turn).
- Pattern count, spacing and directions: R40, R41, R52, R61. Hole points and type: R42, R43, R63.
- Move body inserted or removed: R27, R32. Units: R48. Imported file revised: R45, R46. Renaming: R65.
- Reverting an edit (undo trips): R23, R25, R28, R57, R61.
