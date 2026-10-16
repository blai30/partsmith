# Reference stability and helpful errors

This document answers two concerns of the project owner. First, FreeCAD tracks faces and edges by numbers (Edge002, Face009) that break when anything earlier in the model changes, and users lose hours finding out why; Partsmith must do much better. Second, when an error does happen, the message must explain what happened, why, and what to do, suggest the fix and its cause, and the app must prevent errors where it can. These are the "References that survive edits" and "Errors that help" bullets of [the design direction](../guidelines/README.md).

It reports two studies and proposes the fixes:

- **Reference stability**: the 65 scenarios of the [reference stability corpus](reference-stability-corpus.md), written as 123 tests (one per variant and size) and run against the current resolver, naming rules and messages (section 2).
- **Error quality**: an inventory of 260 user-facing messages from every crate, host and spec, each checked for what, why, what to do, a one-click fix and jargon (section 4), and the five errors newcomers hit most (section 7).

The fixes are given as spec text for [naming.md](../spec/naming.md), [sketch.md](../spec/sketch.md), [features.md](../spec/features.md), the per-feature specs, [recompute.md](../spec/recompute.md), [file-format.md](../spec/file-format.md), [python-api.md](../spec/python-api.md), [engine.md](../spec/engine.md), [protocol.md](../spec/protocol.md), [kernel.md](../spec/kernel.md) and [ui.md](../spec/ui.md) (sections 3 and 5 to 7). The house rules for every message are in [the error guidelines](../guidelines/errors.md). Changes to the naming algorithm, the file format, the protocol and the kernel facade need human review and ADRs before code (section 9, ADRs 0021 to 0026). Section 10 is the implementation plan: every change in order, spec text first, each with the tests that prove it, so the work can proceed from that section and the spec text it points to. Nothing here is implemented yet.

## 1. Summary

- **Today 35 of 123 corpus tests pass (28%).** 38 tests (31%) end with Partsmith using the wrong element with status ok and no message at all: exactly the FreeCAD failure mode, sometimes worse, because a fingerprint can pick a lookalike anywhere in the body. 25 tests fail loudly where the model should have built (a whole model lost to a 5 mm chamfer line in a sketch), and 25 get the right geometry with a message that says neither what was lost, nor why, nor what to do.
- **Messages today:** of 260, 117 (45%) do not say why, 95 (37%) do not say what to do, 123 (47%) use jargon, ids or raw internal text, and only 14 (5%) offer a one-click fix. Of the 17 messages about lost references and placements, 14 give no cause and none offers a fix. Several are false: a sketch "stays at its last position" while it jumps to its attach-time position, an empty body says "Use op new" when the real cause is a failed feature above it, an extrude failure blames the profile when the distance caused it, and every lost reference sends the user to a "repair panel" that does not exist.
- **Four root causes.** (1) The resolver substitutes lookalikes by fingerprint when history says the element is gone, and confirms order-dependent names (`#n`) against fingerprints that went stale with the very edit being resolved. (2) Profile regions are matched by area within 0.1 percent, so ordinary outline edits lose the profile, and the outcome depends on the size typed. (3) Features drop what the resolver knew (candidates, which part of the name vanished), and nothing computes which feature removed an element or which edit caused it. (4) Some geometric rules are anchored in world space or byte order instead of the model: sketch frames on faces, axis directions, chamfer sides.
- **The fix in one sentence:** resolve by meaning (the name confirmed by what it touched, recorded history, continuations and relatives), never by guess; when the meaning cannot be confirmed, say it is a guess and ask; when a reference is lost, say which element, which feature removed it and which edit caused it, show where it was, and offer the replacements in one click, described by what they keep and how far they are.
- **Expected result** (section 3.9): phase 1 (the whole resolver with anchors and region boundaries, shipped together so that nothing that builds correctly today breaks) takes the corpus from 35 to 112 passing tests and the silent wrong results from 38 to 4; phase 2 (frames that live on the model) reaches 119 with no silent wrong result left; all four phases pass 123 of 123 with decision D3 and the corpus amendments of section 3.10, under which a deleted base feature (R26) and a face that turned over (R32b) end with a true warning and a one-click fix instead of a silent guess.
- **Cause attribution** (section 5): recompute explains every loss from the current run alone ("'Extrude 1', which comes before this fillet, cuts it away"), and the engine adds which edit started it by comparing with the feature's last build without that problem ("This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm"), with its own one-click actions such as "Set Distance back to 5 mm".
- **Repair experience** (section 6): every new loss raises a notification that names the feature, the lost element and the cause; repair mode shows the lost element's ghost captured from its last build, numbered candidates described by meaning ("on the top of 'Box 1', 5 mm from where it was"), one click to use one, and any pick to replace it. This moves the item ui.md section 1.2 lists as later into M1, because the owner's first concern cannot be met without it.
- **Newcomer errors** (section 7): the five errors newcomers hit most (an open outline, a sketch on the wrong plane, a fillet too large, a constraint conflict, a shell too thick for the wall the print needs) get a cause and fixes computed by the core, every feature message is rewritten to the guidelines, and a newcomer error corpus of 12 scenarios pins the exact texts in recompute, engine and Playwright tests.

### 1.1 Changes after review

Two design reviews found that the first version of this design wrote documents behind the user's back, changed what saved files mean, let order and random ids decide in a few places, and left the newcomer errors and the notification path untouched. This version:

- **Writes nothing automatically.** The fallback refresh, the early-cutoff cache keys and the first version's decision D5 are gone. A sketch whose face a later-inserted feature removed is placed where that face was just before the removal, read from the run itself (section 3.5.1); a sketch whose face's feature is not in the document uses its saved position, says so truthfully, and the cause line offers "Keep the sketch where it was". Reverting an edit by hand restores a byte-identical document.
- **Keeps the meaning of saved files.** `fallback` stays in world coordinates; hints in body coordinates carry an explicit marker that only new attaches write; picked axes store their direction explicitly; `FORMAT_VERSION` goes to 2 once for every addition, with a migration from 1 that changes nothing, so older builds stop with a clear message instead of a serde error (section 3.7).
- **Keeps motions per face name**, inherited like names, never pruned, and composed with the instance transform for pattern copies (section 3.2, N3, N7 and N8).
- **Confirms names without anchors** against their siblings and reports a guess when a sibling fits better; anchors ship in phase 1, store only what tells siblings apart (at most six), are matched through canonical names indexed once per body state, and round-trip through Python (sections 3.2 and 3.7).
- **Keeps the loss diagnosis out of cached errors**: it is computed from names only, memoized, and attached to the rendered message (section 3.6). Candidates carry the reference that would store them. Fixes reach hosts as small descriptors that `apply_fix` applies against the current revision; actions that depend on the session belong to the cause line, never to the message (section 5).
- **Never lets order decide**: aliases merge as sorted unions, a face placement needs coplanar faces and an axis collinear edges, region seeds must be connected, and a sketch on a face that turned over keeps a right-handed frame (sections 3.2 to 3.4, decision D3).
- **Describes candidates by meaning and distance**, never by a match percentage; turns replacements by several entities and regions that change area into warnings; notifies the user of every new loss (sections 6.2 and 6.3).
- **Adds the newcomer errors** (section 7) and the engine query behind the prevention measures (section 5.4, E6).

## 2. Reference stability results

### 2.1 How the study ran

The corpus scenarios R01 to R65 are tests in `crates/partsmith-recompute/tests/`: `reference_stability_1.rs` (R01 to R22), `reference_stability_2.rs` (R23 to R44) and `reference_stability_3.rs` (R45 to R65), with helpers in `stability_support_1.rs` to `stability_support_3.rs`, behind the crate feature `stability-study` so they never run in the normal suite. Run them with `cargo test -p partsmith-recompute --features stability-study --test reference_stability_1` (and `_2`, `_3`); each runs in under 2 s. Every failing test lists each unmet expectation, the resolver result, the top five fingerprint scores and the region match. Every setup was checked to build before its edit, and every candidate element a test looks for exists in the body, so the failures come from Partsmith, not from the setups.

Where the corpus was open, the tests read it this way: "at most a notice" is status ok, or a warning without a lost-reference or lost-region code (today's `region_remapped` counts as the outline notice); message checks require the right features named in words, no ids, element names or kernel text, and repair data at the right pointer; R07 keeps references as stored between its two edits; R20b's "unsuppressing restores" means the fillet returns at x 50 to 80 with the same volume.

Corpus "Today" columns that turned out wrong, to correct in the corpus: R20a fails because the copy's bottom front edge (0.878) and the source's back bottom edge (0.873) tie, not because the pieces score below 0.85 (a lead of 0.005 more would have rounded the copy's bottom edge silently); R31b's nearest candidate scores 0.847, not 0.914 (the centroid distance is 10 mm, not 5), so it fails by 0.003 instead of filleting silently; R49 does not leave a wall silently but fails with a misleading "too thick" error; R51 and R64 fail on the profile before the fillet reference is resolved.

### 2.2 Pass rate per category

Categories follow the corpus sections.

| Category | Scenarios | Tests | Pass | Rate |
| --- | --- | --- | --- | --- |
| A. Sketch edits | R01 to R07, R49, R50, R51, R65 | 19 | 5 | 26% |
| B. Dimension and parameter changes | R08 to R14, R52 | 20 | 6 | 30% |
| C. Features inserted upstream | R15 to R22, R53 | 20 | 4 | 20% |
| D. Suppress and delete | R23 to R27, R54 | 8 | 0 | 0% |
| E. Reorder | R28, R29, R55 | 4 | 0 | 0% |
| F. Op, body and direction changes | R30, R31, R56, R57 | 8 | 0 | 0% |
| G. Sketches and datums on faces | R32 to R37, R58 | 10 | 3 | 30% |
| H. Axes and directions | R38, R39, R59, R60 | 8 | 5 | 63% |
| I. Patterns and mirror | R40, R41, R61 | 9 | 5 | 56% |
| J. Holes | R42, R43, R62, R63 | 10 | 5 | 50% |
| K. Shell | R44, R64 | 2 | 1 | 50% |
| L. Imported geometry | R45, R46 | 3 | 1 | 33% |
| M. Units and upstream failures | R47, R48 | 2 | 0 | 0% |
| **All** | **65** | **123** | **35** | **28%** |

Most passing tests are guards (edits the current rules already handle). Several others pass only because a typed size stays under a threshold or a sign happens to come out right: R01 passes at 0.5 mm and fails at 5 mm, R02 passes for a 2 x 1 notch and fails for 20 x 10, R08 passes at width 120 and fails at 200, R38 passes when the wall leans one way and fails the other way. That is corpus rule 6 broken: the outcome depends on a typed number crossing a hidden threshold.

### 2.3 How the failures look

| Kind | R01 to R22 | R23 to R44 | R45 to R65 | All |
| --- | --- | --- | --- | --- |
| Silent wrong result: status ok, no message, wrong element used | 17 | 13 | 8 | 38 |
| Loud wrong result: a feature or the whole model fails, or a misleading message, where it should build | 12 | 6 | 7 | 25 |
| Right geometry, unclear message (no element in words, no cause, no repair data) | 6 | 6 | 13 | 25 |
| **Failing tests** | **35** | **25** | **28** | **88** |

The silent wrong results are R07a, R08 (four), R09, R10a, R11, R12 (four), R17a, R17c, R21b, R22 (two), R23 (two), R24 (two), R27, R29, R30 (two), R31a, R32 (rotation), R34, R38, R43a, R46a, R47, R52 (two), R53a (two), R53b at t=5 and R63a.

### 2.4 Failure classes and root causes

Every failing test belongs to one class below (the counts add up to 88). Steps refer to naming.md section 3.9 as it is today.

**C1. Profile regions matched by area (12 tests: R01 at 5 and 10 mm, R02 with the 20 x 10 notch and the r5 corner, R03, R04, R05, R06b, R07b, R50, R51, R64).** sketch.md section 3.14 tries the exact key, the outer boundary, then the fingerprint, which allows 1e-3 of the area (4 mm² on the 4000 mm² block). Adding a corner chamfer line (R01), a notch (R02), deleting a bevel (R03, R51), drawing a circle across the edge (R04) or inside (R50), moving overlapping rectangles (R05) or redrawing a side further out (R06b, R07b) all miss: `profile_not_found` takes the whole model down, and every later feature repeats the symptom with a misleading message (`body_empty` advising "Use op new"; face sketches reporting `plane_fallback` "stays at its last position" while they move to their attach-time placement; R06b lands at x=80). R50 resolves by outer boundary to the ring only and silently cuts a through hole. Root cause: a region reference has no rule for an edited boundary that keeps most of its entities, no rule for a split region (corpus rule 3: all its pieces), and a size threshold decides (rule 6). Re-picking the profile by hand shows the second layer: R01, R02 and R04 then resolve every downstream reference, R05 resolves by history at 0.8245, R03 reports the bevel edge missing with only generic text, and R07b's true edge scores 0.736 and fails.

**C2. Sketch edits leave no history for names (3 tests: R06a, R07a, R49; also the message part of R03, R51 and R64).** A line deleted and redrawn in place (R06a) gives the face a new name (`f-ext1/lateral/e-9`) with no link to `f-ext1/lateral/e-2`. The fillet and the face sketch follow only by fingerprint (1.0 and 0.975), with no message, and `region_remapped` stays forever because the stored key is never refreshed. On the next unrelated edit the stale fingerprint decides: in R07a the height goes from 30 to 80 and the bottom back edge (0.8784) beats the true edge (0.8193), so the fillet moves to the bottom silently. A side line split in two (R49) leaves the shell opening only the lower half of the side, which the kernel cannot do next to a coplanar kept face, so the shell fails with "too thick" (removing both halves gives exactly the expected opening). Root cause: naming.md section 3.6.2 names lateral faces by entity id and records no succession, and region references keep a sorted key, not the loop order, so a succession cannot be inferred safely.

**C3. Order-dependent names confirmed by stale fingerprints (8 tests: R08 at 200 and 400 in both variants, R09, R11, R27, R46a).** Step 1 finds the exact name, sees `#`, and confirms it by fingerprint distance against its family. The fingerprint was recorded before the edit, so the sibling piece wins: R08a #1 at 0.685 against the named #2 at 0.486, R09 #1 at 0.815 against #2 at 0.730, R11 the y=35 edge at 0.940 against the named y=45 edge at 0.847, R27 #2 at 1.0 against #1 at 0.698 (the fingerprint was recorded after a move that is then removed). Each resolves by history to the wrong piece with status ok. R46a (an imported file revised) finds the exact name `edge(f-imp1/face#3|f-imp1/face#6)`, every family member scores 0, and the name is accepted with confidence 0: a straight edge is chamfered instead of a hole rim. Root cause: corpus rule 2, an order-dependent name must be confirmed by meaning (which piece, what it touches), and a below-threshold result must never count as resolved.

**C4. Split numbers follow cut order (4 tests: R10a, R17a, R17c, R29).** `#n` comes from geometric order within each cut, so a cut inserted upstream (R17) or two cuts swapped (R29) renumber the pieces and `#2` comes to mean other geometry. Step 2 takes every string descendant (`#1#1` and `#1#2`) and step 3 accepts a family of one at any score, so R10a rounds the left part of the top (0.625) after the right part was cut away. Root cause: piece identity is the cut sequence, not what the piece touches.

**C5. Instance suffix hides pieces (2 tests: R20a, R20b).** The copy's pieces are named `f-ext1/end-cap#1@f-lp1:1`: the split suffix precedes the instance suffix, so neither the split step nor the family step sees them (an open issue in the naming crate's AGENTS.md). R20a ties two lookalikes and fails; R20b's best candidate is 0.641 and fails.

**C6. Lookalikes substituted after the element is gone (19 tests: R12 in four variants, R21b, R23 in two, R24 in two, R30 in two, R31a, R34, R43a, R47, R53a in two, R53b at t=5, R63a).** Step 4 scores every element of the body and takes the best above 0.85. A pocket floor ring becomes the hole's top rim when the pocket goes through (R12: 0.9732), an inside corner becomes a cove (R21b: 1.0, fingerprints carry no convexity), peg A's rim becomes peg B's when peg A is suppressed or deleted (R23: 0.8851), a slot rim becomes the cube's outer edge (R24: 0.8992), a boss rim becomes the pocket opening when the join turns into a cut (R30: 0.9046), an edge moved to body 2 is replaced by body 1's new cut edge (R31a: 0.9140), a sketch on the top drops to the tray floor when a shell is inserted (R34: 0.9404), a chamfer jumps to the neighboring hole (R43a: 0.8914), a fillet runs on the body without the boss whose sketch failed (R47: 0.8616), a covered face is replaced by the face covering it (R53: 0.9704, 0.9431, 0.914), a counterbore's floor edge is chamfered (R63a: 0.9331). Each with status ok. Root cause: step 4 ignores provenance, and corpus rule 4 says that when history says the element is gone, nothing is substituted silently. Element names also create no feature dependencies (features.md section 2.4), so suppression, deletion, reordering and upstream failures are invisible to the resolver.

**C7. History descendants, nested names and relatives not followed (5 tests: R19, R37, R58, R63b at both diameters).** A shell rim is named after the face it replaced (`f-sh1/rim/f-ext1/end-cap`, naming.md section 3.6.7), but the resolver never treats it as that face's continuation: R19's fillet sees the outer edge at 1.0 and the inner at 0.969, inside the margin, and fails; R58's datum fails at 0.75. Aliases are chased for whole face names only, not for names nested in generated names: R37's chamfer face becomes `f-cha1/chamfer/edge(f-a1/front|f-a1/top)` and the datum on it fails at 0.603. R63b (a counterbore hole made simple) follows the new mouth only by fingerprint luck (0.8636 with no notice) and breaks at diameter 3 (0.7818), although the hole, its point and the top face are unchanged.

**C8. Merged elements (4 tests: R52 in two variants, R61a, R61b).** When a merged edge or face comes apart (R52), the exact name resolves to the part that kept it, and the other parts are dropped silently. When separate parts fuse (R61), the reference follows through the aliases with no notice.

**C9. Losses without cause or candidates (20 tests: R13, R16, R18 in two, R21a, R25, R28 in two, R31b, R41 in two, R46b, R53b at t=10, R54, R56 in three, R57, R59 in two; and the message part of every C1 and C6 scenario).** The outcome is right but the message is one of "The edge selected for Edges can no longer be found. Re-pick it in the repair panel." and its variants. Root causes, all in features.md and recompute.md: section 3.8 drops the per-reference warnings and their candidates when a whole list fails, and `ReferencesNotFound` carries no repair data (R16, R18, R41 variant, R46b); `Missing.nearest` is computed and dropped; an ambiguous reference is reported as "can no longer be found"; no record says which feature removed the element, or that its feature is suppressed (R25, R56a), deleted (R54), later in the timeline (R28) or making another body (R31b, R56c); `upstream_failed` names the direct input instead of the root cause (R25, R53b, R54); `body_empty` names no cause (R56b); a fillet rounding the axis edge upstream is reported as "now matches 2 edges", which suggests the edge still exists (R59); and the "repair panel" does not exist.

**C10. Sketch frames on faces anchored in world space (5 tests: R26, R32 in three variants, R33).** features.md section 3.6 places a face sketch with `Placement::on_plane(face centroid, normal, fallback)`, which projects the stored fallback onto the face: only the plane follows the face, the in-plane frame stays in world space. A body moved by 100 mm leaves the hole behind (R32a, `hole_misses_body` and `no_effect`); a 90 degree rotation puts it at another corner silently; a reversed extrude flips the y axis and drops the point off the face (R32b). The fallback is written at attach time and never refreshed, so when the face is lost the sketch jumps to its attach-time position (R26: z=30 instead of 80; R33: z=10 instead of 40) and the boss ends up inside the body, while the message claims the sketch "stays at its last position".

**C11. Axis sense canonicalized (1 test: R38).** features.md section 3.6 makes the first nonzero component of an edge or face axis positive, so a wall leaning 1 mm one way flips a pattern direction from +Y to -Y. Revolve, circular pattern and move body rotations about an edge use the same rule.

**C12. Two-distance chamfer side by byte order (2 tests: R22).** chamfer.md section 3.3 step 4 puts the first distance on the face whose current name sorts first. A join that renames the top (`f-base1/top` or `f-a1/top`) puts 1 mm on the top and 3 mm on the front, silently; with random interactive ids about half of such joins flip the chamfer.

**C13. Feature rules that fail too much or say too little (3 tests: R43b, R48, R55).** A point deleted from a hole's sketch fails the whole hole (hole.md `UnknownEntity`), so no hole is cut and the chamfer fails too: two errors for one deleted point. Switching document units makes bare numbers mean inches and the fillet error says only "Try 1.9 in or less" (R48). A sketch on a boss top moved above the boss falls back with a generic message and the part gains a closed void nobody reports (R55).

## 3. Fixes for reference stability

### 3.1 Principles

The fixes follow the corpus rules, restated as the resolver's contract:

1. **Meaning, never guesses.** An element is used only when recorded history connects it to the reference: the same name confirmed by what it touched, the pieces of a split element, aliases of merged elements, a sketch entity's recorded successor, a shell rim continuing a removed face, a face of the same hole or sketch entity under another role. A fingerprint never chooses an element on its own. Fingerprints rank the candidates a user is offered, confirm fallback names (imported faces, which have no other identity), and mark a name without anchors as a guess when a sibling fits better.
2. **Order-dependent names carry what tells them apart.** A reference to a split piece records its anchors: the faces it touched that its siblings did not. A piece is found again by what it touches, so cut order, rigid moves and sizes never change which piece it is.
3. **Regions are found by their boundary.** A profile region follows the entities that bounded it, covers every connected piece when new geometry splits it, and records which entity replaced which, so the faces made from them keep their meaning.
4. **Every loss is explained.** The resolver says which part of the name is gone; recompute's diagnosis says which feature removed it, from the current run; the engine says which edit started it. Candidates, each with the reference that would store it, are kept for every lost reference, single or in a list.
5. **Frames live on the model.** Sketch frames on faces and picked axis directions are kept in the coordinates of the face's own feature and move with it; chamfer sides follow the face the user picked; a sketch whose face was cut away stays where that face was.
6. **Nothing is rewritten without the user.** Stored references, fallbacks and directions change only through an explicit, undoable action (Keep, Use, Pick, Keep the sketch where it was). Recompute and the engine never write the document on their own, so reverting an edit by hand, suppressing then unsuppressing, and dragging a feature back all restore a byte-identical document, as undo does.
7. **Saved files keep their meaning.** A file written before this change rebuilds the same way after it, except where the old rule was the bug (an axis that flipped on a small tilt). New data is additive and marked, and the format version tells older builds to stop instead of misreading.
8. **References stay small and order free.** A reference stores only what resolution needs: anchors that tell siblings apart (at most six), merged names per face with pattern copies written once. Nothing depends on the order of inputs, on random ids or on byte order beyond the final tie break.

### 3.2 naming.md

**N1. Section 1, "Explicitly later": replace the last bullet** ("A repair-history log of deleted elements (the resolver reports `Missing` without the deleting feature).") with:

````markdown
- A persistent log of deleted elements. The resolver reports which parts of a lost name are gone (section 3.9), and recompute's loss diagnosis (recompute.md section 3.10) names the feature that removed them from the run itself, so no log is stored.
````

**N2. Section 2.4: replace the `Reference` definition and its JSON with:**

````markdown
```rust
/// A stored reference to a face, edge or vertex: a name query, the fingerprint it had when picked, what tells its
/// order-dependent parts apart from their siblings, and the former names merged into its faces (section 3.11).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "ReferenceData", deny_unknown_fields)]
pub struct Reference {
    /// The element name the reference was made with.
    #[serde(rename = "q")]
    pub query: ElementName,
    /// The element's fingerprint when the reference was made.
    #[serde(rename = "fp")]
    pub fingerprint: Fingerprint,
    /// For each order-dependent part of the query, the faces it touched that its siblings did not.
    #[serde(rename = "nb", default, skip_serializing_if = "Anchors::is_empty")]
    pub anchors: Anchors,
    /// For each face name in the query (parts and nested terms), the former names merged into that face.
    #[serde(default, skip_serializing_if = "Merged::is_empty")]
    pub merged: Merged,
}

/// The most anchors stored for one part.
pub const ANCHOR_LIMIT: usize = 6;

/// Part name to the sorted, distinct piece-free names of at most `ANCHOR_LIMIT` faces.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Anchors(pub BTreeMap<ElementName, Vec<ElementName>>);

/// Face name occurring in the query to the sorted, distinct former names merged into it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Merged(pub BTreeMap<ElementName, Vec<MergedName>>);

/// A former name merged into a face: one face name, or every copy of a face made by one pattern or mirror.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "String", into = "String")]
pub enum MergedName {
    /// A face name.
    Face(ElementName),
    /// Every instance name of `source` made by `feature`, written `<source>@<feature>:*`.
    Copies { source: ElementName, feature: Id },
}

impl Anchors { pub fn is_empty(&self) -> bool; }
impl Merged { pub fn is_empty(&self) -> bool; }

impl Reference {
    /// Builds a reference without anchors or merged names; fails if the query's element kind differs from the fingerprint's.
    pub fn new(query: ElementName, fingerprint: Fingerprint) -> Result<Reference, NamingError>;
    /// The reference that denotes this element as it is now in `set`, with anchors and merged names (section 3.11).
    /// Every pick and every stored fix uses it.
    pub fn from_element(info: &ElementInfo, set: &ElementSet) -> Reference;
    /// The same reference without anchors and merged names, as element lists send it (engine.md section 3.9).
    pub fn short(&self) -> Reference;
}
```

JSON (canonical key order `q`, `fp`, `nb`, `merged`; `nb` and `merged` are omitted when empty, so a reference without them serializes exactly as in format version 1):

```json
{"q": "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)", "fp": "line|n=1,0,0|c=65,0,30|l=30|adj=2,2", "nb": {"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]}}
```

```json
{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=50,10,10|a=2000|adj=4,4", "merged": {"f-box1/top": ["f-box1/top@f-lp1:*"]}}
```

Deserialization checks that every `nb` key is an order-dependent part of `q`, or `q` itself when it has its own `#n` (section 3.1), that every anchor list is sorted, distinct, at most `ANCHOR_LIMIT` long and holds valid face names, that every `merged` key is a face name occurring in `q`, and that every merged list is sorted and distinct. `Reference::from_info` is replaced by `Reference::from_element`.
````

**N3. Section 2.6: replace the `Aliases` definition and the `NamingMap` methods `build` and `with_instance` with the following, and add motions:**

````markdown
```rust
/// Former face names mapped to the piece-free names that carry them now, sorted and distinct: one face when faces
/// merged, one or more when a feature records that a face continues in others (section 3.6.2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Aliases(pub BTreeMap<ElementName, Vec<ElementName>>);

impl Aliases {
    /// The current names `name` stands for: follows aliases until names that are no alias keys, sorted and distinct;
    /// stops on a cycle. A name that is no key gives itself.
    pub fn chase(&self, name: &ElementName) -> Vec<ElementName>;
    /// True when there are no aliases.
    pub fn is_empty(&self) -> bool;
}

/// An isometry in mm: a 3 x 3 orthonormal matrix (a rotation, or a rotation and a reflection for a mirror copy), then
/// a translation, row major. Negative zeros are stored as zeros, and equality compares bit patterns, so maps stay `Eq`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct Motion([f64; 12]);

impl Motion {
    /// The motion that changes nothing.
    pub const IDENTITY: Motion;
    /// `self` followed by `after`.
    pub fn then(&self, after: &Motion) -> Motion;
    /// The motion that undoes `self`.
    pub fn inverse(&self) -> Motion;
    /// Moves a point.
    pub fn apply_point(&self, point: [f64; 3]) -> [f64; 3];
    /// Turns a direction (no translation).
    pub fn apply_direction(&self, direction: [f64; 3]) -> [f64; 3];
    /// True when it keeps handedness (determinant +1).
    pub fn is_proper(&self) -> bool;
}

impl PartialEq for Motion { /* bitwise */ }
impl Eq for Motion {}

impl NamingMap {
    /// Builds the map for one output shape (algorithm in section 3.3). `inherited` are the maps of every input shape,
    /// the body the feature writes first; `added` are aliases the feature records on purpose (section 3.6.2).
    pub fn build(faces: Vec<FaceSource>, topology: &TopologyInfo, inherited: &[&NamingMap], added: &Aliases) -> Result<NamingMap, NamingError>;
    /// How far Move body features and copy transforms have moved the face since it was built (section 3.3 step 8):
    /// the motion recorded for its piece-free name, or the map's base motion. Defined for every face name, also one
    /// that is no longer in the shape.
    pub fn motion(&self, face: &ElementName) -> Motion;
    /// The same map with `motion` applied after every recorded motion (section 3.6.12).
    pub fn moved(&self, motion: &Motion) -> NamingMap;
    /// The map of a copy made by a pattern or mirror instance whose transform is `transform` (section 3.5.1).
    pub fn with_instance(&self, feature: &Id, instance: Instance, transform: &Motion) -> Result<NamingMap, NamingError>;
}
```

The serialized map (result cache only) gains `"motions": {"base": [12 numbers], "own": {"<piece-free face name>": [12 numbers]}}`, omitted when the base is the identity and `own` is empty, and an alias target becomes a sorted list: `"aliases": {"f-ext2/end-cap": ["f-ext1/end-cap"]}`.
````

**N4. Section 2.7: add the resolver's view of a body state, and `generators` to `ElementName` (section 2.2):**

````markdown
```rust
/// The resolver's view of one body state: its element infos, its aliases, and an index of every face under the
/// canonical forms of its name (section 3.9). Built once per body state and shared (features.md `BodyState::element_set`).
pub struct ElementSet { /* private */ }

impl ElementSet {
    /// Builds the set and its canonical-name index.
    pub fn new(elements: Arc<[ElementInfo]>, aliases: Aliases) -> Result<ElementSet, NamingError>;
    /// The element infos, faces, then edges, then vertices.
    pub fn elements(&self) -> &[ElementInfo];
    /// The aliases.
    pub fn aliases(&self) -> &Aliases;
}

/// A feature, and the sketch generator when the name has one, that a name depends on.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GeneratorUse {
    /// The feature id.
    pub feature: Id,
    /// The sketch entity id or point reference text, for example `e-3` or `e-p1`.
    pub generator: Option<String>,
}

impl ElementName {
    /// Every feature and sketch generator the name depends on: those of each face term, of every nested name and
    /// of every instance group, sorted and distinct. Engine queries use it to say what uses a feature (engine.md section 3.24).
    pub fn generators(&self) -> Vec<GeneratorUse>;
}
```

`resolve(reference, set)` replaces `resolve(reference, elements, aliases)`.
````

**N5. Section 2.8: replace `Resolution`, `Candidate` and `MatchMethod` with:**

````markdown
```rust
/// Outcome of resolving one reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Resolution {
    /// The reference denotes these elements: one, or every piece of a split element.
    Resolved {
        /// Sorted by name.
        elements: Vec<ElementName>,
        method: MatchMethod,
        /// 1, except for a fallback name confirmed by its score and for `Guess`, which carry the score.
        confidence: f64,
        /// How the reference followed a change, when features should tell the user (section 3.9).
        #[serde(skip_serializing_if = "Option::is_none", default)]
        follow: Option<Follow>,
        /// The alias keys the match went through (successions and merges), sorted. Empty for a match by name.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        via: Vec<ElementName>,
        /// For `Guess`: the other elements that could be meant, in candidate order.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        alternatives: Vec<Candidate>,
    },
    /// Several elements fit by meaning, or, for fallback names, by shape. In candidate order.
    Ambiguous { candidates: Vec<Candidate> },
    /// The element is gone. Candidates are possible replacements in candidate order; the resolver never uses them.
    Missing {
        #[serde(skip_serializing_if = "Lost::is_empty", default)]
        lost: Lost,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        candidates: Vec<Candidate>,
    },
}

/// A possible replacement for a lost, ambiguous or guessed reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Candidate {
    /// The candidate's name.
    pub name: ElementName,
    /// True when it touches a face in the family of a present part of the query (section 3.9 step 5).
    pub meaning: bool,
    /// Distance in mm between its fingerprint centroid and the stored one; 0 when either has none.
    pub distance: f64,
    /// Its fingerprint score against the stored fingerprint (section 3.9.1), for reports. Hosts never show it as a match.
    pub score: f64,
}

/// What the resolver found gone.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Lost {
    /// Parts of the query with no face, continuation or relative in the body.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub parts: Vec<ElementName>,
    /// Order-dependent parts whose piece no longer exists (section 3.9 step 2).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub pieces: Vec<ElementName>,
    /// True when every part exists but no element joins them (the faces no longer meet).
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub unmet: bool,
}

/// Which resolver step produced the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MatchMethod {
    /// The name matched, confirmed by its anchors when it is order dependent.
    Exact,
    /// Recorded history matched: pieces of a split, aliases of a merge, a recorded succession, merged names, a piece found by its anchors.
    History,
    /// A face that continues a removed face (a shell rim) matched.
    Continued,
    /// An edge or vertex of the same feature and generator under another role matched.
    Related,
    /// An order-dependent name without anchors: the named piece while a sibling fits the stored fingerprint better,
    /// or the best-fitting piece when the named one is gone. Features report it as a guess.
    Guess,
}

/// How a resolved reference followed a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Follow {
    /// Through a succession: a sketch entity was replaced by one or more others.
    Replaced,
    /// Through a succession: a sketch entity now continues in further entities on the same line or circle.
    Extended,
    /// Through aliases: the element merged with parts the reference did not name.
    Merged,
    /// To the continuation of a removed face.
    Continued,
    /// To a relative under another role.
    Related,
}
```

JSON:

```json
{"kind": "resolved", "elements": ["f-ext1/end-cap#1", "f-ext1/end-cap#2"], "method": "history", "confidence": 1}
{"kind": "resolved", "elements": ["edge(f-ext1/end-cap|f-ext1/lateral/e-9)"], "method": "history", "confidence": 1, "follow": "replaced", "via": ["f-ext1/lateral/e-2"]}
{"kind": "resolved", "elements": ["f-box1/top#1"], "method": "guess", "confidence": 0.809931, "alternatives": [{"name": "f-box1/top#2", "meaning": false, "distance": 0, "score": 1}]}
{"kind": "missing", "lost": {"parts": ["f-ext2/end-cap"]}, "candidates": [{"name": "edge(f-ext1/end-cap|f-ext2/lateral/e-5)", "meaning": true, "distance": 5, "score": 0.973234}]}
{"kind": "missing", "lost": {"unmet": true}}
```

`CONFIDENCE_THRESHOLD` and `AMBIGUITY_MARGIN` keep their values and apply only to fingerprint comparisons (section 3.9 steps 1 and 2).
````

**N6. Section 3.1, definitions: add after "Byte order":**

````markdown
- **Suffix groups** of a face name: the `#n` and `@<feature>:<instance>` groups after its role or generator, in written order. **Piece-free name:** the face name with every `#n` group removed, wherever it stands (`f-ext1/end-cap#1@f-lp1:1#2` gives `f-ext1/end-cap@f-lp1:1`). A face name `d` **extends** `t` when their piece-free names are equal and `d`'s suffix groups are `t`'s with zero or more `#n` groups inserted (`f-ext1/end-cap#1@f-lp1:1` extends `f-ext1/end-cap@f-lp1:1`; `f-a/x#2#1` extends `f-a/x#2`, not `f-a/x#1`). The **pieces** of `t` are the faces that extend it: `t` itself, or `t` split further.
- **Fallback name:** a face name whose role is `face` (section 3.6.14). It has no meaning beyond geometric order.
- **Parts** of a query: its face terms for an edge or vertex name, the name itself for a face name. The parts of an edge or vertex name with its own trailing `#n` also include the name itself. The **face names occurring in** a query are its parts and every face name nested in their generators.
- **Candidate order:** candidates with `meaning` first, then by increasing `distance`, then by name in byte order.
````

**N7. Section 3.3, building a naming map.** The signature becomes `build(faces, topology, inherited, added)`. In step 2, an alias is recorded as `piece_free(s) -> [piece_free(base)]` (piece-free instead of split base). Replace step 6 with:

````markdown
6. **Aliases.** For every key, the target list is the union of its lists in every inherited map, in step 2 and in `added`, sorted and distinct, so the same maps in any order give the same aliases (reordering a boolean's tools changes nothing). Flatten: replace every target by its `chase` over the combined set, keeping the list sorted and distinct. Prune: remove every target that is not the piece-free name of some output face, then every entry left without targets, then every entry whose key is the piece-free name of an output face unless the key is among its own targets (an extension, section 3.6.2).
````

Add step 8:

````markdown
8. **Motions.** A face's motion says how Move body features and copy transforms have moved it since it was built. The map stores a `base` motion and `own` motions per piece-free face name; `motion(name)` is the own motion of the name's piece-free name, else the base. The output's base is the base of the first inherited map (the identity without inherited maps). Every own motion of every inherited map is kept; every piece-free name that an inherited map knows (its own keys and its faces' names) and whose motion there differs from the output's base gets that motion as an own entry; on a conflict the first map in `inherited` order wins. A face named `New` gets an own identity entry when the base is not the identity (it was built where it is); a face named `From` keeps the motion of its source, and a merged face the motion of the name it keeps. Own entries equal to the base are dropped. Entries are never dropped because their name left the shape, so a face that a later feature removed still has its motion, and a name the map never saw (its feature is gone) has the base: the motion of the body as a whole.
````

**N8. Section 3.5.1, instance copies: replace the second paragraph with:** "`NamingMap::with_instance(feature, instance, transform)` renames every face, edge and vertex name this way, and every alias key and target (as face names), and composes `transform` after the base and every own motion. It does not renumber anything: a transform copy has the same element indices as its source (kernel spec), so element `i` of the copy has the instance name of element `i` of the source. A face of a copy therefore has the motion of its source followed by the copy's transform, so a sketch on a copy face moves with the copy when the spacing changes. A mirror's transform is a reflection, so the motions of its copies are improper; a face placement that comes out left-handed follows decision D3 (features.md section 3.6)."

**N9. Section 3.6.2, extrude: add at the end:**

````markdown
**Sketch succession.** When the profile match reports successions (sketch.md section 3.14), the feature passes them to `build` as `added` aliases, one per succession `m -> [n1, ..., nk]`: `f/lateral/<m> -> [f/lateral/<n1>, ..., f/lateral/<nk>]`. A replacement lists only the new entities (`f/lateral/e-3 -> [f/lateral/e-9]`); an extension lists the old entity first (`f/lateral/e-2 -> [f/lateral/e-2, f/lateral/e-6]`). So a reference to the face of a replaced or extended line follows to the faces that carry its place now (section 3.9 step 1). Successions exist only while the stored region reference is older than the sketch edit; keeping the profile stores the current regions, and the engine rewrites the references that went through the successions in the same edit (engine.md section 3.22).
````

Section 3.6.3 (revolve): add "Sketch succession applies as for extrude."

**N10. Section 3.6.12, move body: replace with:**

````markdown
The output map is the input map moved by the feature's transform (`NamingMap::moved`): names, indices and aliases are unchanged, and the transform is composed after the base and every own motion. The feature must not rebuild the map, because rebuilding could renumber `#n` groups whose order key changed under the motion. Motions let a sketch on a moved face keep its place on the face and a picked axis keep its side (features.md section 3.6).
````

**N11. Section 3.9: replace the whole section (keep 3.9.1) with:**

````markdown
### 3.9 Resolver

`resolve(reference, set)` resolves a stored reference against an `ElementSet` (section 2.7): the element infos of one body state, its aliases and its canonical-name index. It considers only candidates whose `kind` equals the query's kind. An element is returned only when the name, recorded history (pieces, aliases, successions, merged names), the anchors of a split piece, a continuation or a relative connects it to the query. Fingerprints rank candidates (step 5), confirm fallback names (step 1), and mark a name without anchors as a guess when a sibling fits the stored fingerprint better (steps 1 and 2); they never choose an element on their own. Let `q` be the query, `fp` its fingerprint, `nb` its anchors and `merged` its merged names, `best` the highest score in a set and `second` the next highest (0 if none). The steps run in order; the first that produces a result wins. Candidates are always listed in candidate order (section 3.1).

**Canonical names.** For a face name `t`, `canon(t)` is a set of piece-free names: the piece-free name of `t`, with every element name nested in its generator replaced by each of its canonical forms (for an edge or vertex generator, the canonical forms of its face terms, the terms sorted again), one name per combination; each such name followed through `aliases.chase`; and, for every merged name recorded for `t` or for a face name nested in it, the canonical forms of that merged name (a `Copies` entry stands for every instance name of its source made by its feature). The `ElementSet` indexes every face under the canonical forms of its own name, computed once per body state, so a query term is matched by lookups and candidates are never parsed during a resolve. The *family* of a part `t` is the set of faces whose canonical forms meet `canon(t)`; `t` is *present* when its family is not empty. The family of an edge or vertex query is the set of candidates whose adjacent faces pair one to one with the query's face terms, each adjacent face in the family of its term (or, in step 2, among the faces selected for its term); each adjacent face is looked up once, so pairing is linear in the number of candidates.

**Anchor test.** For an anchored part `t` with anchors `A = nb[t]`: the *live anchors* `L` are the names `a` of `A` whose family is not empty. A face *touches* `a` when one of its adjacent faces is in the family of `a`, and *covers* `L` when it touches every name of `L`. For the part that is the query itself (an edge or vertex with its own `#n`), "adjacent faces" means the faces containing its vertices other than its own faces. A part whose `L` is empty is treated as a part without anchors: its anchors no longer tell anything apart.

**Step 1: exact.** Let `e` be the candidate named `q`; when there is none and `q` has no `#`, the candidate whose parts' canonical forms equal those of `q` (each part's `canon` holding one name).
- `canon` of some part holds several names (an extension, a replacement by several entities, merged faces that came apart): go to step 2.
- `q` is not order dependent and `e` exists: `Resolved { [e], Exact, 1 }` when `e` is named `q`; otherwise `Resolved { [e], History, 1 }` with `via` the alias keys used, `follow` `Replaced` when a succession alias was used, and `Merged` when every face of `e` is an alias target that also stands for names that are neither parts of `q` nor its merged names.
- `q` is order dependent and every anchored part has live anchors: `e` is accepted when it covers the live anchors of every anchored part: `Resolved { [e], Exact, 1 }`. Otherwise go to step 2.
- `q` is order dependent, has no live anchors, and its order-dependent parts are fallback names (imported faces): score the family of `q` against `fp`. If `best >= CONFIDENCE_THRESHOLD` and `best - second >= AMBIGUITY_MARGIN`: `Resolved { [e], Exact, score(e) }` when `score(e) >= best - AMBIGUITY_MARGIN`, else `Resolved { [best member], History, best }`. If `best >= CONFIDENCE_THRESHOLD` otherwise: `Ambiguous` with the members scoring at least `best - AMBIGUITY_MARGIN`. If `best < CONFIDENCE_THRESHOLD`: go to step 5. A name scoring below the threshold never counts as resolved.
- `q` is order dependent, has no live anchors, and `e` exists (a reference written before anchors existed, by hand or by a script, or whose siblings could not be told apart when it was picked): score the family of `q` against `fp`. When members other than `e` score at least `score(e) + AMBIGUITY_MARGIN`: `Resolved { [e], Guess, score(e), alternatives }` with those members. Otherwise `Resolved { [e], Exact, 1 }`. The name is used, but a sibling that fits the stored shape clearly better makes it a guess that features report (features.md section 3.8), so a number-like name is never confirmed silently against a fingerprint that may be stale.

**Step 2: pieces.** For each part `t`, select faces from its family:
- A part without live anchors selects, when it is not order dependent, its whole family (an element split by a later edit resolves to all of its pieces, and a name with several canonical forms to every face that carries one); when it is order dependent, the members that extend `t` (its pieces). When an order-dependent part has no piece (the named piece no longer exists): score the family against `fp`; when `best >= CONFIDENCE_THRESHOLD` and `best - second >= AMBIGUITY_MARGIN`, select the best member with method `Guess` and the other members as `alternatives`; otherwise `t` is *piece-lost*.
- An anchored part with live anchors: (a) if exactly one member covers `L`, select it: the piece kept its meaning, whatever its size and number; (b) otherwise, if the members that touch at least one name of `L` together cover `L`, select them all: the piece was split; (c) otherwise `t` is piece-lost.

If a part is piece-lost, go to step 5. Otherwise the result is, for a face query, the selected faces, and for an edge or vertex query, the candidates whose adjacent faces pair one to one with the faces selected for every part (and, for a query with its own `#n` and live anchors, those that cover them). Non-empty: `Resolved { sorted, History, 1, follow, via }`, or `Guess` with the score when a part was selected by score; `follow` is `Extended` when a part used an extension and `Replaced` when it used a replacement. Empty: go to step 3.

**Step 3: continuation.** A part that is not present is *continued* by the faces named `<feature>/rim/<s>` with `s` in `canon(part)`: the rim a shell leaves where it removed the face (section 3.6.7), in the same surface. For a face query whose part is continued: `Resolved { continuing faces, Continued, 1, Continued }`. For an edge or vertex query in which every part is present or continued: the candidates pairing with the members or continuations; one or more give `Resolved { them, Continued, 1, Continued }`. A fillet or chamfer face generated from an edge is not a continuation of that edge: it consumed it.

**Step 4: related (edges and vertices only).** Faces of the same feature with the same sketch generator are *relatives* of a part `<feature>/<role>/<generator>` when their role differs (a hole that changes type keeps its point; an extrude keeps its entity). Fallback names, parts without a generator, and parts whose generator is an element name have no relatives. Faces are never resolved to relatives.
- If exactly one part is not present and the others are: the candidates that pair with the present parts and a relative of the missing part. One: `Resolved { [it], Related, 1, Related }`. Several: `Ambiguous` with them, each with `meaning: true`.
- If every part is present but no candidate pairs with all of them: the candidates that keep one part and whose other face is a relative of another part, or of the same feature and generator as a kept part. One or more: `Ambiguous` with them (the change is ambiguous by meaning: a hole whose bore now ends at a counterbore has a new mouth and a new bore edge, and either may be what the user meant).

**Step 5: lost.** `Missing { lost, candidates }`. `lost.parts` lists the parts that are not present, continued or related; `lost.pieces` the piece-lost parts; `lost.unmet` is true when every part is present. `candidates` are at most five elements of the query's kind in candidate order: `meaning` is true for an element that touches a face in the family of a present part (or, when no part is present, a face of a feature named in a part), and `distance` is the distance between its fingerprint centroid and `fp`'s. Each carries its score (section 3.9.1). No candidate is ever returned as resolved.

The resolver is a pure function: the same reference and set give the same `Resolution`, including candidate order, whatever the order of the elements and aliases in the set.
````

**N12. Section 3.9.1, scoring: add at the end:** "Scores confirm fallback names, decide whether a name without anchors is a guess, and are reported with candidates; they never select an element of a built-in feature's named output, and hosts never show them as a match percentage."

**N13. Section 3.10, invariants: add:**

````markdown
5. A resolved element always descends from the query by recorded history (exact, pieces, aliases, successions, merged names, anchors, continuation, relatives). The exceptions are fallback names confirmed by their score and `Guess` results, which features report as guesses.
6. Resolving never depends on a size crossing a threshold, on the order of inputs, or on random ids, except for fallback names and the guess rule of names without anchors.
````

**N14. New section 3.11, anchors and merged names:**

````markdown
### 3.11 Anchors and merged names

`Reference::from_element(info, set)` builds the reference of `info` with `q = info.name` and `fp = info.fingerprint`, plus:

- **Anchors.** For each order-dependent part `t` of `q` that is not a fallback name, and for `q` itself when it has its own `#n`: let `F` be the other members of the family of `t` in `set`, and `adj(x)` the piece-free names of the faces sharing a non-degenerate edge with face `x` (for `q` itself: of the faces containing its vertices other than its own faces), leaving out fallback names and the names in `canon(t)`. The *distinguishing* names of `t` are the names of `adj(t)` that some member of `F` does not touch (every name of `adj(t)` when `F` is empty): a face every sibling also touches tells nothing apart. At most `ANCHOR_LIMIT` are kept, those touched by the fewest members of `F` first, then in byte order, and stored sorted. The part gets no entry when the list is empty, or when a member of `F` has the same list computed the same way: nothing in the body tells them apart, so the resolver confirms the name against the fingerprint instead (section 3.9 step 1).
- **Merged names.** For each face name `n` occurring in `q`, the alias keys whose `chase` includes the piece-free name of `n`, sorted and distinct, under the key `n`. When the list holds the instance names of one source made by one feature for every instance of that feature present in `set` (as a face or as an alias key), they are written once as `<source>@<feature>:*`.

Anchors use piece-free names so that further splits of a neighbor do not change them; the anchor test needs only the names of present faces, so it never depends on sizes or positions; and a reference to a split top surrounded by a hundred holes stores at most six names. Merged names are kept per face name, nested terms included, so a reference picked after a join still resolves when the join is later removed.
````

**N15. Section 5, performance budgets: add rows:**

````markdown
| `ElementSet::new` for 1,000 faces, 3,000 edges, 2,000 vertices, canonical-name index included | 3 ms |
| `resolve` of a reference with 100 merged names per part and a 100-instance `Copies` entry against a 1,000-face set | 0.5 ms |
| `Reference::from_element` for an edge of a face split in 100 pieces, in a 1,000-face set | 0.5 ms |
````

The row "`resolve` of one reference against 1,000 face candidates, worst case (reaches step 5): 0.2 ms" is kept and measured with the index.

**N16. Section 6, acceptance tests.** Change these expectations (scores within 1e-6):

- 61, 62 and 64: `Missing` with `lost.parts == [f-gone/top]` and the first candidate `f-box1/top` with `meaning: false`, distance 0, 5 and 0, score 1, 0.884184 and 1.
- 63: `Missing` with `lost.parts == [f-gone/top]` and first candidate `f-box1/top` at distance 70, score 0.613866.
- 65: `Missing` without candidates.
- 66: `Missing` with both faces as candidates at distance 0, ordered `f-a/x`, `f-b/x`.
- 69: without anchors, the swapped sibling fits better: `Resolved { [f-box1/top#1], Guess, 0.809931, alternatives: [f-box1/top#2] }`.
- 70: `Resolved { [f-box1/top#1], Exact, 1 }` (both pieces score 0.889184, so no sibling fits better).
- 71: without anchors the named piece is gone and the remaining face scores 0.718593, below the threshold: `Missing` with `lost.pieces == [f-box1/top#2]`; with `nb {f-box1/top#2: [f-box1/right]}` it is `Resolved { [f-box1/top], History, 1 }`.
- 72 and 74: alias targets are lists (`{f-ext9/end-cap: [f-box1/top]}`); results unchanged with `via` the alias key.
- 75: `Resolved { [edge(f-a/x|f-b/y)#1], Guess, 0.613731, alternatives: [edge(f-a/x|f-b/y)#2] }`.
- 76: `Missing` with `lost.pieces == [f-hole1/bore/e-5#3]` and both pieces as candidates (score 0.774944, distance 10, by name).
- 77: `Missing` with the three vertices as candidates by distance, the first `vertex(f-a/x)` with score 0.925985.
- 80: `Missing` with no `lost` and no candidates serializes as `{"kind": "missing"}`.
- 89: the aliases of the joined pattern are lists: `{f-box1/top@f-pat1:1: [f-box1/top], f-box1/top@f-pat1:2: [f-box1/top]}`.

Add:

````markdown
91. Anchored piece, stale fingerprint. Scope: faces `f-b/top#1` (adjacency `[f-b/back, f-b/front, f-b/left, f-c/w1]`, centroid x=15) and `f-b/top#2` (`[f-b/back, f-b/front, f-b/right, f-c/w2]`, x=65), with edges `edge(f-b/front|f-b/top#1)` and `edge(f-b/front|f-b/top#2)`. Query `edge(f-b/front|f-b/top#2)` with the fingerprint of the `#1` edge and `nb {f-b/top#2: [f-b/right, f-c/w2]}`: `Resolved { [edge(f-b/front|f-b/top#2)], Exact, 1 }`.
92. Renumbered piece. Scope pieces `f-b/top#1` (`[f-b/back, f-b/front, f-b/left, f-n/w]`), `f-b/top#2#1` (`[f-b/back, f-b/front, f-n/w2, f-c/w1]`), `f-b/top#2#2` (`[f-b/back, f-b/front, f-b/right, f-c/w2]`); query `f-b/top#2` with `nb [f-b/right, f-c/w2]`: `Resolved { [f-b/top#2#2], History, 1 }`.
93. Split anchored piece. Same scope; query `f-b/top#1` with `nb [f-b/left, f-c/w1]`: the face named `f-b/top#1` touches only `f-b/left`, so step 2 (b) selects `Resolved { [f-b/top#1, f-b/top#2#1], History, 1 }`; `f-b/top#2#2` touches neither anchor.
94. Piece gone. Scope `f-b/top` (`[f-b/back, f-b/front, f-b/left, f-c/w1]`) and `f-b/right`; query `f-b/top#2` with `nb [f-b/right, f-c/w2]`: `Missing` with `lost.pieces == [f-b/top#2]`.
95. Piece grown. Scope `f-b/top#1` (`[f-b/back, f-b/front, f-b/left, f-d/w]`) and `f-b/top#2` (`[f-b/back, f-b/front, f-b/right, f-d/w3]`); query `f-b/top#2#1` with `nb [f-c/w1, f-d/w]` (no face `f-c/w1` exists): `Resolved { [f-b/top#1], History, 1 }`.
96. No live anchors. Scope of test 91; query `edge(f-b/front|f-b/top#2)` with the fingerprint of the `#1` edge and `nb {f-b/top#2: [f-z/gone]}`: the anchors are ignored, and the result is `Guess` on the named edge with the `#1` edge as the alternative.
97. Live anchors through an alias. Scope of test 91 with `f-b/right` renamed `f-j/right` and aliases `{f-b/right: [f-j/right]}`: the query of test 91 still resolves `Exact` (the anchor's family is found through `canon`).
98. Instance pieces. Query `edge(f-e/cap@f-p:1|f-e/side@f-p:1)`; scope edges `edge(f-e/cap#1@f-p:1|f-e/side@f-p:1)` and `edge(f-e/cap#2@f-p:1|f-e/side@f-p:1)` with matching adjacency: `Resolved` with both, `History`.
99. No substitution. BOX scope; query `edge(f-gone/x|f-box1/top)` with E2's fingerprint: `Missing` with `lost.parts == [f-gone/x]`; the candidates are E2 (`meaning: true`, distance 0, score 1), then the other three top edges by distance.
100. Continuation. Scope faces `f-ext1/lateral/e-1`, `f-sh1/inner/f-ext1/lateral/e-1`, `f-sh1/rim/f-ext1/end-cap` and edges `edge(f-ext1/lateral/e-1|f-sh1/rim/f-ext1/end-cap)` and `edge(f-sh1/inner/f-ext1/lateral/e-1|f-sh1/rim/f-ext1/end-cap)`; query `edge(f-ext1/end-cap|f-ext1/lateral/e-1)`: `Resolved { [the first edge], Continued, 1, Continued }`. Query `f-ext1/end-cap`: `Resolved { [f-sh1/rim/f-ext1/end-cap], Continued, 1, Continued }`.
101. Related. Scope edges `edge(f-box1/top|f-hole1/bore/e-p1)` and `edge(f-box1/top|f-hole1/bore/e-p2)`; query `edge(f-box1/top|f-hole1/counterbore/e-p1)`: `Resolved { [edge(f-box1/top|f-hole1/bore/e-p1)], Related, 1, Related }`.
102. Relatives never cross generators. Scope edge `edge(f-box1/top|f-hole1/bore/e-p1)` only; query `edge(f-box1/top|f-hole1/bore/e-p2)`: `Missing`, the edge offered as a meaning candidate.
103. Ambiguous by meaning. Scope faces `f-box1/top`, `f-hole1/counterbore/e-p2`, `f-hole1/counterbore-floor/e-p2`, `f-hole1/bore/e-p2` and edges `edge(f-box1/top|f-hole1/counterbore/e-p2)`, `edge(f-hole1/bore/e-p2|f-hole1/counterbore-floor/e-p2)`; query `edge(f-box1/top|f-hole1/bore/e-p2)`: `Ambiguous` with both, `meaning: true`, the mouth first by distance.
104. Nested aliases. Aliases `{f-box1/front: [f-a1/front], f-box1/top: [f-a1/top]}`; scope face `f-cha1/chamfer/edge(f-a1/front|f-a1/top)`; query `f-cha1/chamfer/edge(f-box1/front|f-box1/top)`: `Resolved` with that face, `History`.
105. Nested merged names, reverse direction. No aliases; scope face `f-cha1/chamfer/edge(f-box1/front|f-box1/top)`; query `f-cha1/chamfer/edge(f-a1/front|f-a1/top)` with `merged {f-a1/front: [f-box1/front], f-a1/top: [f-box1/top]}`: `Resolved` with that face, `History`. Without `merged`: `Missing`.
106. Replacement. Aliases `{f-ext1/lateral/e-3: [f-ext1/lateral/e-9]}`; query `edge(f-ext1/end-cap|f-ext1/lateral/e-3)` against a scope with `edge(f-ext1/end-cap|f-ext1/lateral/e-9)`: `Resolved`, `History`, `follow` `Replaced`, `via [f-ext1/lateral/e-3]`.
107. Replacement by several. Aliases `{f-ext1/lateral/e-3: [f-ext1/lateral/e-10, f-ext1/lateral/e-11, f-ext1/lateral/e-9]}`; query `edge(f-ext1/end-cap|f-ext1/lateral/e-3)` against a scope with the three top edges: `Resolved` with all three, `History`, `follow` `Replaced`.
108. Extension. Aliases `{f-ext1/lateral/e-2: [f-ext1/lateral/e-2, f-ext1/lateral/e-6]}`; query `f-ext1/lateral/e-2`: `Resolved { [f-ext1/lateral/e-2, f-ext1/lateral/e-6], History, 1, Extended }`.
109. Merged names. Query `{q: f-box1/top, merged: {f-box1/top: [f-box2/top]}}` against a scope with separate faces `f-box1/top` and `f-box2/top`: `Resolved` with both. With `merged {f-box1/top: [f-box1/top@f-lp1:*]}` against a scope with `f-box1/top`, `f-box1/top@f-lp1:1` and `f-box1/top@f-lp1:2` as separate faces: `Resolved` with all three.
110. Fallback names below the threshold. Scope of line edges whose family all scores 0 against a circle fingerprint; query `edge(f-imp1/face#3|f-imp1/face#6)`: `Missing`, not `Exact` with confidence 0.
111. `Reference::from_element` on the scope of test 91 for `edge(f-b/front|f-b/top#2)` gives `nb {f-b/top#2: [f-b/right, f-c/w2]}` (front and back are touched by the sibling) and no `merged`; on a box-only scope it gives a reference that serializes exactly as `{"q": ..., "fp": ...}`.
112. `from_element` stores no anchors for a piece whose sibling has the same distinguishing names: in a scope where `f-b/top#1` and `f-b/top#2` both have adjacency `[f-b/back, f-b/front, f-c/w]`, the reference of either has no `nb`.
113. `from_element` on a piece touching ten distinguishing faces stores six, those touched by the fewest siblings first, sorted.
114. `from_element` on the fused top of a 4-instance pattern gives `merged {f-box1/top: [f-box1/top@f-lp1:*]}`; when one copy stands apart and is not merged, the three merged instance names are listed.
115. Aliases are order free: `NamingMap::build` of a boolean with its tool maps in either order gives equal maps.
116. Motions: `moved` by a translation of (100, 0, 0) gives `motion(f-box1/top)` with that translation and leaves names, indices and aliases unchanged; a face built after the move has the identity; `with_instance(f-lp1, 1, T)` gives copy faces the source motion followed by `T`; a join with a smaller feature id after a move keeps `motion(f-ext1/end-cap)` equal to the move although the merged face is named after the joined feature; a face removed after a move keeps its motion in every later map; a name the map never saw has the base. Maps built from motions with `-0.0` and `0.0` are equal.
117. `ElementName::generators` of `edge(f-ext1/lateral/e-2@f-lp1:1|f-fil1/fillet/edge(f-box1/front|f-box1/top))` is `[(f-box1, None), (f-ext1, e-2), (f-fil1, None), (f-lp1, None)]`.
118. The canonical-name index gives the same families as the definition of section 3.9 on random scopes with aliases and merged names (property test), and resolving is independent of the order of the scope's elements and of the alias map's insertion order.
````

### 3.3 sketch.md (region matching)

**S1. Section 2.7: replace `RegionRef`, `RegionMatch` and `MatchMethod` with:**

````markdown
```rust
/// A stored reference to a region of a sketch.
pub struct RegionRef {
    /// The region's key.
    pub key: RegionKey,
    /// The region's fingerprint when the reference was made.
    pub fp: RegionFingerprint,
    /// The region's outer loop in traversal order when the reference was made (section 3.13.8); absent in
    /// references written before format version 2.
    pub outline: Option<RegionLoop>,
}

/// An outer loop as a cyclic sequence of key entries, written like "e-1,e-2,~e-5,e-3".
pub struct RegionLoop { /* private: entries */ }

/// Finds the regions a stored reference means today (section 3.14). `entities` are the ids of the sketch's entities.
pub fn match_region(regions: &[Region], entities: &[Id], reference: &RegionRef) -> RegionMatch;

/// Result of region matching.
pub enum RegionMatch {
    /// One or more regions matched; indices into the `regions` slice, ascending.
    Resolved { indices: Vec<usize>, method: MatchMethod, successions: Vec<Succession> },
    /// Several separate sets of regions fit; each candidate is one set, indices ascending, candidates ordered by their first index.
    Ambiguous { candidates: Vec<Vec<usize>> },
    /// No region matches.
    Missing { loss: RegionLoss },
}

/// How regions were matched.
#[serde(rename_all = "snake_case")]
pub enum MatchMethod { Key, Boundary, Coverage, Fingerprint }

/// A boundary entity of the stored region that now continues in other entities.
pub struct Succession {
    /// The stored entity.
    pub entity: Id,
    /// The entities that carry its place now: the new entities for a replacement, the entity itself then the new ones for an extension.
    pub now: Vec<Id>,
}

/// Why no region matched.
pub enum RegionLoss {
    /// These entities of the stored outer boundary are no longer in the sketch.
    Deleted { entities: Vec<Id> },
    /// Every entity of the stored outer boundary is gone, and no region has the stored size and place: the outline was drawn again.
    Redrawn,
    /// The stored entities are still in the sketch, but no closed region lies on their inner side: the outline is open.
    Open,
}
```

A `RegionRef` serializes as `{"key": "<key text>", "fp": "<fingerprint text>", "outline": "<loop text>"}`, with `outline` omitted when absent. `Region::reference()` always writes `outline`.
````

**S2. New section 3.13.8, region loop:**

````markdown
#### 3.13.8 Region loop

A region's loop text lists the entries of its outer loop (`[~]id`, as in the key) in traversal order, with consecutive entries of the same entity written once, as a cyclic sequence rotated so that the text is the smallest of all its rotations in byte order. Examples: the counterclockwise rectangle `e-1,e-2,e-3,e-4`; the rectangle with a corner chamfer line `e-6` between `e-1` and `e-2`, `e-1,e-6,e-2,e-3,e-4`; a rectangle whose top line `e-3` is interrupted by an arc `e-5`, `e-1,e-2,e-3,~e-5,e-3,e-4`. `RegionLoop::from_str` accepts any non-empty comma-separated entries and canonicalizes the rotation; anything else fails with `InvalidRegionKey { text }`. Like keys, loops survive unrelated edits (moving geometry and editing dimensions keep them).
````

**S3. Section 3.14: replace with:**

````markdown
### 3.14 Region matching

`match_region(regions, entities, reference)` finds the regions a stored `RegionRef` means in a rebuilt profile. It follows the entities that bounded the region; the area is used only when none of them is left. With the stored fingerprint `(A0, c0)`, the score of a region `(A, c)` is `norm(c - c0) / sqrt(A0) + abs(A - A0) / A0` (when `A0 = 0`, `norm(c - c0) + abs(A - A0)`). An entity is *added* when it is in neither the stored key nor the stored outline.

1. **Key.** The regions whose key equals the stored key. One: `Resolved { [i], Key, [] }`. Several: the one with the lowest score; if the lowest two scores differ by at most `1e-12`, `Ambiguous` with each region tied at the lowest score as its own candidate.
2. **Boundary.** An entry `(entity, reversed)` of the stored key's outer loop *survives* when it appears with the same `reversed` flag in the outer loop of some region: the region lies on the same side of that entity as the stored one. The *seeds* are the regions whose outer loop has a surviving entry. The set starts with the seeds and grows by *filling*: repeatedly add every region whose outer loop runs, reversed, along a hole of a region in the set, when that hole's entities are all added (new geometry drawn inside the region). Stored holes stay holes. Two regions of the set are *connected* when they share a boundary piece of an added entity, or one fills a hole of the other; the set splits into connected components.
   - One component of one region: `Resolved { [i], Boundary, successions }`, with the successions below.
   - One component of several regions: `Resolved { indices, Coverage, [] }`: the region was split by new geometry, and every piece is used, as a split face resolves to all its pieces (naming.md section 3.9).
   - Several components: `Ambiguous` with each component as a candidate. A region that keeps a stored entity but is not joined to the others through new geometry (a second rectangle drawn on an extended line) is never added silently.
3. **Fingerprint.** If no stored outer entry survives: the regions with `abs(A - A0) <= 1e-3 * A0` and `norm(c - c0) <= 1e-3 * sqrt(A0)`. One: `Resolved { [i], Fingerprint, [] }`. Several: the lowest score with the tie rule of step 1. Features report this match as a guess.
4. Otherwise `Missing` with the loss: `Deleted` with the stored outer entities that are not in `entities` when there are any but not all; `Redrawn` when none of them is in `entities`; `Open` when all are.

**Successions** (step 2 with one region). Let `C` be the matched region's loop (section 3.13.8) and `O` the stored `outline`. An entity of `O` absent from `C` is *removed*, an added entity of `C` is *new*, the others *survive*.
- *Replacement.* For two surviving entities `a` and `b` that follow each other among the surviving entities of `C`, compare the run of `O` strictly between `a` and `b` with the run of `C` strictly between them. When the old run is exactly one removed entity `m` and the new run is not empty and made only of new entities, `m` was replaced: `Succession { entity: m, now: the new run in traversal order }`.
- *Extension.* A new entity `n` next to a surviving entity `s` in `C`, sharing an end point with it, and lying on the same line (both end points of `n` within `PROFILE_TOLERANCE` of the infinite line of `s`) or the same circle (centers and radii within `PROFILE_TOLERANCE`), continues `s`: `Succession { entity: s, now: [s, n, ...] }`, with further new entities chained the same way.
- Without a stored `outline`, the replacement rule applies only when exactly one entity of the stored outer loop is removed and the new entities of `C` form one consecutive run whose two neighbors in `C` survive; extensions apply as above.

Matching uses only the arguments; it never depends on the area except in step 3.
````

**S4. Section 6.12: add tests** (one per rule, in the style of the existing ones): a corner chamfer line added (one region, `Boundary`, no successions, for chamfer sizes 0.5, 5 and 10); a line deleted and redrawn in place (`Boundary`, `e-2 -> [e-9]`); a line redrawn 10 mm further out (`Boundary`, same succession); a side line split in two collinear lines (`Boundary`, `e-2 -> [e-2, e-6]`); a side line replaced by a five-line notch (`Boundary`, `e-3 -> [e-9, e-10, e-11, e-12, e-13]` in traversal order); a circle across a side (`Coverage` with the big region and the inner half disc in one component, never the outer half disc); a circle inside (`Coverage` with the ring and the disc); two overlapping rectangles (each reference `Coverage` with its own part and the overlap); `e-1` extended to x=200 with a second rectangle drawn on its far end (`Ambiguous` with two candidates: the original region, and the new rectangle); a deleted bevel with a chamfer added elsewhere in the same edit (with `outline`: no succession; without it: no succession either, because two entities change); a fully redrawn outline (`Fingerprint`, and `Missing { Redrawn }` when the new outline is 1 percent larger); a deleted side line that opens the outline (`Missing { Deleted [e-2] }`); a stored region whose entities remain but no longer close (`Missing { Open }`).

The message rewrite of sketch.md section 4 is in section 7.4.

### 3.4 features.md

**F1. Section 2.4, rules: replace the second bullet with:** "A face, edge or vertex reference adds the body it is resolved in to `bodies_read` (or leaves it in `bodies_written` when the feature writes that body). Element names inside `q` never create feature dependencies; recompute's loss diagnosis reads them to explain a loss (recompute.md section 3.10), and the engine's `list_uses` reads them to say what uses a feature (engine.md section 3.24)."

**F2. Section 2.6, build context:**

- `BuildInputs` gains `history: &'a BTreeMap<Id, Vec<(Id, BodyState)>>`: for each declared body, its state just before each earlier active writer, oldest first, with the writer's id (recompute.md section 3.5). `BuildContext` gains `pub fn body_history(&self, body: &Id) -> Result<&[(Id, BodyState)], FeatureError>` (`UndeclaredDependency` for an undeclared body).
- `BodyState` gains `pub fn element_set(&self) -> Result<Arc<ElementSet>, FeatureError>` ("the resolver's view of the state, naming.md section 2.7, computed once from `elements` and the map's aliases and shared by clones") and `pub fn motion(&self, face: &ElementName) -> Motion` ("the motion the naming map records for the face, naming.md section 3.3 step 8; the identity for an empty body").
- `ctx.face_placement(body, face, hint, frame)` takes the hint's frame (`HintFrame::World` or `HintFrame::Body`, section 3.6) and returns `Ok(Ok(FacePlacement { placement, turned_over }))` on success. New: `ctx.face_placement_before_removal(body, face, hint, frame) -> Result<Option<(Placement, Id)>, FeatureError>` places on the face as it was just before the writer that removed it, moved by every motion since, and returns that writer (section 3.6).
- `Resolved` gains `notices: Vec<FeatureWarning>` (info notices for references that followed a change); its `warnings` hold losses, guesses and widenings. `ResolvedProfile` gains `successions: Vec<Succession>` and `notices`.
- `FeatureOutput` gains `absent: Vec<AbsentFace>`: faces its naming rule would make that this build did not make, with why (`Through`: a cut or hole whose end lies outside the body; `FullTurn`: a revolve of a full turn has no caps). The loss diagnosis reads it (recompute.md section 3.10).

**F3. Section 2.8: add the loss types and fixes, and change the reference variants:**

````markdown
```rust
/// One reference that did not resolve to what it meant, as a feature reports it.
pub struct Loss {
    /// Position in the reference list (0 for a single reference).
    pub position: usize,
    /// The body the reference is resolved in.
    pub body: Id,
    /// The stored reference.
    pub reference: Reference,
    /// Face, edge or vertex.
    pub kind: ElementKind,
    /// True when several elements fit, false when none does.
    pub ambiguous: bool,
    /// What the resolver found gone (naming.md section 2.8).
    pub lost: Lost,
    /// Possible replacements in candidate order, never applied automatically.
    pub candidates: Vec<LossCandidate>,
}

/// A possible replacement, with the reference that would store it.
pub struct LossCandidate {
    /// `Reference::from_element` of the candidate in the state the reference was resolved in, built when the loss is found.
    pub reference: Reference,
    /// True when it touches a face the lost element touched.
    pub meaning: bool,
    /// Distance in mm between its centroid and the stored one.
    pub distance: f64,
    /// Its fingerprint score, for reports only.
    pub score: f64,
}

/// A one-click fix a host can apply (section 4.4). Built from the feature's own build; hosts receive small descriptors
/// and the engine rebuilds the edit from the current records when the fix is applied (engine.md section 3.22).
pub enum Fix {
    /// Set a params field of a feature (this one when `feature` is None): "Use 2.3 mm", "Turn on Reverse".
    SetField { feature: Option<Id>, field: FieldRef, value: serde_json::Value },
    /// Replace the reference at `pointer` with one or more references: a candidate, or "Keep all".
    UseReferences { pointer: String, references: Vec<Reference> },
    /// Store the reference at `pointer` as it resolves now, anchors included, so a notice or guess goes away.
    KeepMatch { pointer: String },
    /// Remove the list entry at `pointer`.
    RemoveEntry { pointer: String },
    /// Replace the region reference at `pointer` with these regions.
    UseRegions { pointer: String, regions: Vec<RegionRef> },
    /// Store the regions the profile uses now, and re-anchor later references that followed its successions.
    KeepRegions { pointer: String },
    /// Resolve this feature's references in another body.
    UseBody { field: FieldRef, body: Id },
    /// Put this feature's result into a new body.
    NewBody,
    /// Attach a sketch to a face.
    Attach { sketch: Id, body: Id, face: Reference },
    /// Mirror the stored drawing of a face sketch whose face turned over and store its current frame (decision D3).
    TurnDrawingBack { sketch: Id },
    /// Connect two open ends of a sketch with a coincident constraint.
    ConnectEnds { sketch: Id, first: PointRef, second: PointRef },
    /// Delete a constraint of a sketch.
    DeleteConstraint { sketch: Id, constraint: Id },
    /// Make a driving dimension of a sketch a reference dimension.
    MakeReference { sketch: Id, constraint: Id },
    /// Rename an unknown params key to the known key it nearly matches.
    RenameKey { from: String, to: String },
    /// Unsuppress or suppress a feature.
    Unsuppress { feature: Id },
    Suppress { feature: Id },
    /// Move a feature directly below or above another one, named by id so it stays right after other edits.
    Move { feature: Id, place: Place },
    /// Open this feature with the field at `pointer` active, so the next pick replaces the reference.
    Pick { pointer: String },
    /// Select and frame a feature (the root cause), or one of its sketch entities.
    Show { feature: Id, entity: Option<Id> },
}

/// Where a moved feature goes.
pub enum Place { Below(Id), Above(Id) }
```

`FeatureError` and `FeatureWarning` gain `fn fixes(&self, units: Units) -> Vec<Fix>` (in the order a host should offer them; `suggestion` returns the value of the first own `SetField`), `fn severity(&self) -> Severity` (`Error` for errors; `Warning` or `Info` for warnings) and `fn detail(&self) -> Option<String>` (the technical text a report needs: the kernel operation, kind and message, a panic message, the stored reference and the resolver result with scores, the parser text of invalid params; never shown as the message). `Repair::Element` gains `loss: Loss` (replacing `reference`, `ambiguous` and `candidates`, which it contains) and `Repair::Region` gains `candidates: Vec<Vec<usize>>`, `successions: Vec<Succession>` and `loss: Option<RegionLoss>`.
````

Variant changes (the texts are in section 7.2):

- Errors: `ReferencesNotFound { field, kind, losses: Vec<Loss> }` (every loss kept, never dropped); `ReferenceNotFound { field, loss }`; `ProfileNotFound { field, sketch, losses: Vec<RegionLoss> }`; `NotPlanar`, `NotStraight` and `NoAxis` gain the resolved element's name for the description; new `NotCoplanar { field, faces: Vec<ElementName> }` and `NotCollinear { field, edges: Vec<ElementName> }` (section 3.6); `ThroughAllMisses { body, reverse_hits: bool }`; `InvalidParams { field: Option<FieldRef>, detail }`; `FilletTooLarge`, `ChamferTooLarge` and `ShellTooThick` gain `limit: Option<Limit>` (section 7.3); new `ShellRemovesAll { body, count }`; `Internal { message }` keeps its message as detail only; new `NameTooDeep { length }` for the naming limit a user can reach.
- Warnings: `ReferenceMissing` and `ReferenceAmbiguous` become `ReferenceLost { field, loss }` with codes `reference_missing` or `reference_ambiguous` by `loss.ambiguous`; new `ReferenceGuessed { field, position, body, reference, element: ElementName, alternatives: Vec<LossCandidate> }` (code `reference_guessed`, warning); `ReferenceFollowed { field, position, body, reference, elements: Vec<ElementName>, follow: Follow, via: Vec<ElementName> }` (code `reference_followed`, info); `ReferenceWidened { field, position, body, reference, elements, via }` (code `reference_widened`, warning: a replacement by several entities that do not lie on one line or circle); `RegionSplit { field, position, sketch, region, count }` (code `region_split`, info) and `RegionResized { field, position, sketch, region, count, area, stored_area }` (code `region_resized`, warning); `RegionAmbiguous` gains `candidates: Vec<Vec<usize>>` and `RegionMissing` gains `loss: RegionLoss`; `PlaneFallback { field, body, loss: Option<Loss>, cause: FallbackCause, placed: Placed }` with `FallbackCause` one of `Missing`, `Ambiguous`, `NotPlanar`, `NotCoplanar` and `Placed` one of `BeforeRemoval { by: Id }`, `Saved`; new `SketchTurnedOver { field, body }` (code `sketch_turned_over`, warning); `NoEffect { body, how: NoEffectHow, reverse_hits: bool, attach: Option<(Id, Reference)> }` (section 7.3); new `SeparateSolid { body, pieces }` (code `separate_solid`, warning); `HoleMissesBody { sketch, point, position: [f64; 2], body, reverse_hits }`; new `PointMissing { field, position, sketch, point }` (code `point_missing`); new `ClosedVoid { body }` (code `closed_void`); `UnknownParams { keys, suggestions: Vec<(String, String)> }`.
- `UpstreamFailed { feature }` and `BodyEmpty { body, use_new }` keep their fields; recompute adds `root` and `because` to the rendered message (recompute.md section 3.10).

**F4. Section 3.6: replace the `Face` bullet, the `face_placement` paragraph and the canonicalization bullet:**

````markdown
- `Face`: resolve the reference in the body (section 3.8, `resolve`). Every resolved face must be planar (else `NotPlanar`) and they must be coplanar: normals within 1e-9 rad and each centroid within `LINEAR_TOLERANCE` of the first face's plane (else `NotCoplanar` with the faces, which the error offers as candidates); a split flat face keeps its plane, and a replacement that spreads over several planes is never decided by name order. The placement is the **face placement rule**: `Placement::on_plane(face centroid, outward normal, &h)`, with `h` the XY placement for `ctx.plane`. It reproduces the origin plane frames for faces parallel to them and does not depend on kernel parametrization.
````

````markdown
`ctx.face_placement(body, face, hint, frame)` applies the face placement rule with a hint of the caller's own (the sketch feature's fallback, [features/sketch.md](features/sketch.md) section 3.3). A hint in body coordinates (`HintFrame::Body`) is first moved by `m`, the motion of the stored query's face (`BodyState::motion` of `q`, naming.md section 3.3 step 8, which also knows faces that merged into others): `h` has the origin `m.apply_point(origin)` and the axes `m.apply_direction(x_axis)` and `m.apply_direction(normal)` (features' `move_placement` helper). A hint in world coordinates (`HintFrame::World`, every hint in documents written before body coordinates existed) is used as it is, so such documents rebuild exactly as before. When the outward normal points against `h`'s normal (dot product below 0), the face turned over since the hint was recorded (a reversed extrude, or a mirror copy whose motion is a reflection); decision D3 applies. With the recommended rule the placement stays right-handed (`on_plane` with the outward normal and `h`'s x axis, so the drawing lies mirrored on the face) and `turned_over` is true. The helper does not fail on the face: an empty body or a missing reference gives `Ok(Err(Missing))`, an ambiguous reference `Ok(Err(Ambiguous))`, faces in several planes `Ok(Err(NotCoplanar))`, and a face that is not flat `Ok(Err(NotPlanar))`. Its `Err` is for failures no fallback fixes, such as `UndeclaredDependency` for an undeclared body or a `Kernel` error while reading the face.

`ctx.face_placement_before_removal(body, face, hint, frame)` walks `body_history(body)` from the newest state back and finds the latest state `S` in which the reference resolves (exact, history, continued or related) to coplanar planar faces; the writer just after `S` is the one that removed the face. It returns the face placement rule applied in `S`, then moved by `motion_now(q).then(motion_S(q).inverse())`, the motion of the stored query's face since `S` (Move body features after the removal move the sketch with the body), together with that writer. `None` when no state has the face. It is a pure function of the declared body's history, which the body's provider key covers, so the build stays cacheable.
````

````markdown
- For `Edge` and `Face`, every resolved edge must be straight and they must be collinear (directions within 1e-9 rad, each on the first one's line within `LINEAR_TOLERANCE`; else `NotCollinear`, offered as candidates). The axis keeps the side the user picked: when the stored `AxisRef` has `direction` (body coordinates, written by every pick since format version 2), the axis is oriented to a positive dot product with `m.apply_direction(direction)`, `m` the motion of the query's first face term; otherwise with the direction of the stored fingerprint (world coordinates when it was picked), when it has one; when the dot product is within 1e-9 of 0, or there is no direction to compare with, the first component (x, then y, then z) whose absolute value exceeds 1e-9 is made positive. A small edit that tilts the edge never reverses the axis, and a Move body inserted before the feature turns the axis with the part. The origin is the point of the axis line closest to the world origin. Features offer `reverse` to flip a direction.
````

Decision D3 (section 9) chooses the rule for a face that turned over. The alternative to the recommended rule keeps the drawing in place with a mirrored placement (`Placement::mirrored`), and then needs a handedness rule in sketch.md (`Placement`), features.md (`make_profile` input, extrude and revolve directions, hole drilling) and kernel.md (`make_profile` with a left-handed frame), with tests for extrude, revolve and hole on a mirrored placement.

**F5. Section 3.7: replace the table with:**

````markdown
| Match | Effect |
| --- | --- |
| `Resolved { method: Key }` | used, no warning |
| `Resolved { method: Boundary }` | used, no warning; its successions become aliases of the feature's lateral faces (naming.md section 3.6.2) |
| `Resolved { method: Coverage }` | every region used; info `RegionSplit` when the regions' total area equals the stored area within 1e-6 relative (the region was only split by new lines, the material is unchanged), otherwise warning `RegionResized` with the fixes `KeepRegions` and `Pick` |
| `Resolved { method: Fingerprint }` | used; warning `RegionRemapped` (a guess) with the fixes `KeepRegions` and `Pick` |
| `Ambiguous` | skipped, `RegionAmbiguous` with the candidate sets and the fixes `UseRegions` for each and `Pick` |
| `Missing` | skipped, `RegionMissing` with its `RegionLoss` and the fixes `Pick` and `Show` of the sketch |
````

**F6. Section 3.8: replace the list with:**

````markdown
`ctx.resolve_all(field, body, references, kind)` resolves each `Reference` with `partsmith_naming::resolve` against `body(body).element_set()`, the state just before this feature:

- `Resolved` with method `Exact`, `History`, `Continued` or `Related`: the elements are added in resolver order, skipping duplicates. A resolution with `follow` adds:
  - for `Replaced` or `Extended` whose elements lie on one line, one circle, one plane or one cylinder (from the kernel geometry of the resolved elements): a `ReferenceFollowed` notice (severity info) with the fixes `KeepMatch` and `Pick`;
  - for `Replaced` whose elements do not: a `ReferenceWidened` warning with the fixes `KeepMatch` ("Keep all"), `UseReferences` with the first element alone ("Use only"), and `Pick`; the elements are used, so the feature still builds, but the user decides;
  - for `Merged`, `Continued` and `Related`: a `ReferenceFollowed` notice with `KeepMatch` and `Pick`, except `Merged` when every alias source of the merged element is referenced by another entry of the same list (the user picked all its parts).
- `Resolved` with method `Guess`: the elements are used and a `ReferenceGuessed` warning is added with the fixes `KeepMatch` (which stores the reference with anchors), `UseReferences` for each alternative, and `Pick`.
- `Ambiguous` or `Missing`: skipped, with a `ReferenceLost` warning carrying the `Loss`, whose candidates each hold `Reference::from_element` of the candidate in the same element set, so a fix stores exactly what the resolver saw. Fixes: `UseReferences` for each candidate, `RemoveEntry`, `Pick`.
- An empty body gives `BodyEmpty { use_new: false }` (recompute adds `because`).
- A non-empty list that resolves to nothing gives `ReferencesNotFound` with every `Loss`; nothing is dropped.

`ctx.resolve(field, body, reference, kind)` resolves one required reference: `Missing` or `Ambiguous` gives `ReferenceNotFound` with its `Loss`; a guess, a follow or a widening adds the warning or notice above.
````

**F7. Section 3.12, labels and message rendering: add:**

````markdown
- **Element descriptions.** Messages name faces, edges and vertices in words from the stored name, the fingerprint and the current sketch outputs, never by name or id. Coordinates are in document units, rendered with `format_value`.
  - A face of a primitive by its role: "the top of 'Box 1'", "the side of 'Cylinder 1'".
  - A cap or lateral face of an extrude or revolve whose fingerprint normal is within 1e-6 of a world axis, by its direction word (+Z top, -Z bottom, -Y front, +Y back, -X left, +X right): "the right side of 'Extrude 1'", "the top of 'Extrude 2'". Any other lateral face by the entity it was made from: "the side of 'Extrude 1' made from the line from (80, 40) to (70, 50) in 'Sketch 1'" (arcs "the arc from ... to ...", circles "the circle at ..."); when the sketch no longer has the entity, "the side of 'Extrude 1' made from a line of 'Sketch 1'" ("a curve" for a face that is not flat), and the cause line names the deleted entity with its coordinates (engine.md section 3.21). Other caps: "the end face of", "the start face of".
  - Fillet and chamfer faces: "the rounded face of 'Fillet 1'", "the beveled face of 'Chamfer 1'". Shell faces: "the inside of" the described generator ("the inside floor of 'Shell 1'" for the inner face of a bottom), "the rim of 'Shell 1'". Hole faces: "the wall of the hole at (7, 0)", "the counterbore of", "the countersink of", "the bottom of", "the tip of". Split faces: "the cut face of 'Split body 1'". Fallback names: "a face of 'Import 1'".
  - Faces a cut leaves (an extrude, revolve or primitive with Operation Cut, the tool faces of a cut boolean) read as the cut's floor and walls: "the floor of the cut made by 'Extrude 2'", "a wall of the cut made by 'Extrude 2'", or with its entity "the wall of the cut made by 'Extrude 2' along the line from (10, -1) to (10, 51)". Holes use their own words.
  - A piece of a split face: "the {left, right, front, back, lower, upper} part of" the face, by the piece's position among the family along the axis where the pieces differ most; otherwise "a part of".
  - A copy: the description followed by "of the second copy of 'Linear pattern 1'" (ordinal of instance index + 1; two indices: "of the copy in row 2, column 3") or "of the mirror image made by 'Mirror 1'".
  - An edge between two faces of one feature that both have direction words: "the front top edge of 'Box 1'" (front or back, then left or right, then top or bottom). An edge between a face with a direction word and a face described by its entity: "the top edge of 'Extrude 1' along the line from (80, 40) to (70, 50) in 'Sketch 1'" ("along a line of 'Sketch 1'" when the entity is gone). A circle edge between a planar face and a round face: "the top rim of the hole at (7, 0)", "the top rim of 'Extrude 2'". The edge between a cut face and a face of the body: "the top edge of the cut made by 'Extrude 2'"; between two cut faces: "the bottom edge of the cut made by 'Extrude 2'". Otherwise "the edge between {face} and {face}". A vertex: "the front left top corner of 'Box 1'", else "the corner where {face}, {face} and {face} meet".
  - A feature that is not in the document: "a face of a feature that is not in this document" (only the session knows whether it was deleted; the cause line says so).
  - When no words apply, "an edge of 'Box 1' near (40, 0, 30)" from the fingerprint centroid. When two elements in one message or candidate list would read the same, each gets its entity clause ("along the line from (80, 50) to (50, 50) in 'Sketch 1'") or, failing that, its position clause ("near (65, 0, 30)").
- **Message structure** (house rules in docs/guidelines/errors.md): what happened, naming the element and the features by label; why, naming the feature that caused it; at most two sentences. What to do is carried by `fixes`; a sentence says it only when no fix covers it. Technical text goes to `detail`. An acceptance test renders, from these rules and the rendered reasons, every message the reference stability corpus and the newcomer error corpus (section 7.6) expect, and runs the message lint of the guidelines over every rendered template.
````

**F8. Section 3.5, body semantics: add:**

````markdown
- After a join or cut, a result with more inner shells (shells minus solids) than the body had before adds a `ClosedVoid` warning: such a hollow prints as a sealed cavity and usually means features are in the wrong order. Comparing inner shells rather than totals also catches two lumps joined into one solid around a void. Shell's closed hollows are intended and give no warning. The kernel reports shell and solid counts with the result measurements (kernel.md change K1).
- A join by a solid feature (extrude, revolve, primitive, import) whose tool does not touch the body (the result has one more lump than the body had, for each such tool) adds a `SeparateSolid` warning with the fixes `NewBody` and, when the attach rule of section 3.15 finds a face, `Attach`. Copy features are not checked: separate copies are what they make.
- `NoEffect` carries why it changed nothing and which fixes work (section 3.15).
````

Section 6.4 body semantics test 7 becomes: warning `separate_solid`, volume 2000, 12 faces, 2 solids. Tests 5 and 6 change their texts to those of section 7.3 (`it lies beside it`, `it lies entirely inside the body`).

**F9. Sections 4.1 and 4.2:** replaced in full by section 7.2, so every row follows the guidelines and passes the message lint.

**F10. Section 4.4: replace the first paragraph with:** "`fixes(units)` gives the one-click fixes of an error or warning, in the order hosts offer them: the fix that keeps the user's intent first, then timeline fixes, then other candidates, then removals, then `Pick`. `SetField` fixes with a computed limit are those of the table below (`suggestion` returns the first). Lost, guessed, followed and widened references give the fixes listed in section 3.8, regions those of section 3.7, and sections 3.15 and 4.3 list the fixes of `NoEffect`, `SeparateSolid`, `HoleMissesBody` and the "too large" errors. Recompute adds the fixes of the loss reason (recompute.md section 3.10): `Suppressed` gives `Unsuppress`; `ComesLater` gives `Move { feature: this, place: Below(the feature that makes it) }`; `OtherBody` gives `UseBody` when the reference's parts are present there, and `Unsuppress` of the suppressed feature that joined it; `UpstreamFailed` gives `Show { root }` and no `Pick`, because re-picking would destroy a reference that is still right. A fix is offered only when it is exact or was verified to work (a Reverse that reaches the body, a radius at which the other edges work)."

**F11. Section 3.4, expression evaluation:** the modeling limit of section 7.5.

**F12. Section 3.11, Python codegen contract: replace the references bullet with:** "References are `Ref(\"<q>\", \"<fp>\")`, followed by `anchors={...}` and `merged={...}` when the stored reference has them (dicts of lists of strings in stored order), and for edge and face axes `direction=(x, y, z)` when stored. A reference list with one element is written as that element alone, a longer list as a list. Planes and axes use `PythonValue::plane` and `PythonValue::axis`, whose face and edge forms carry `part=<part>`." And the profile bullet: "`regions=[Region(\"<key>\", \"<fp>\", outline=\"<loop>\"), ...]`, with `outline` only when stored."

**F13. Section 2.5, shared param types:** `AxisRef::Edge` and `AxisRef::Face` gain `direction: Option<[f64; 3]>` (JSON `"direction": [x, y, z]`, omitted when absent): the axis direction the user picked, in body coordinates, written by hosts from the pick's `axis_direction` (engine.md section 3.9). A file written before format version 2 has none and keeps the fingerprint rule of section 3.6. The direction is stored rather than read from the fingerprint because a fingerprint is in world coordinates as they were at pick time: applying today's motion to it would count a Move body twice when the edge was picked after that Move body existed.

**F14. Section 2.1:** `partsmith_features::VERSION` is bumped with this change (recompute.md cache key line 2), so a persisted `.ptsm-cache/` written with the old naming maps is never reused.

### 3.5 Per-feature specs

**3.5.1 features/sketch.md.**

Section 2: `SketchPlane::Face` gains `fallback_frame: HintFrame` (JSON `"fallback_frame": "body"`, omitted for `world`, the default). `world` means `fallback` is in world coordinates, as in every document written before format version 2; `body` means it is in the coordinates of the face's own feature before any later Move body or copy transform (features.md section 3.6). Hosts write `body` with every new attach and every `end_sketch_drag` commit (engine.md section 3.9), and the fixes `Attach`, `TurnDrawingBack` and "Keep the sketch where it was" write it too; nothing rewrites a fallback on its own.

Section 3.3 step 1, face: the placement is the first that applies:

1. The face resolves to coplanar planar faces: `ctx.face_placement(body, face, &fallback, fallback_frame)`. When `turned_over`, the warning `SketchTurnedOver` with the fix `TurnDrawingBack` (decision D3, recommended rule).
2. The face reference is `Missing`: `ctx.face_placement_before_removal(...)`. When an earlier state of the body has the face, the sketch is placed on it as it was just before the writer `W` that removed it, moved with the body since, and a `PlaneFallback { placed: BeforeRemoval { by: W } }` warning carries the loss. This is the sketch's last resolved placement, read from the run itself: the sketch of R33 stays at z=40 and that of R34 at z=30 while nothing is written to the document.
3. Otherwise (the body is empty, the reference is ambiguous, the faces are not flat or not coplanar, or no earlier state has the face because its feature is not in the document, is suppressed or comes later): `fallback`, moved by the query's motion when `fallback_frame` is `body`, with `PlaneFallback { placed: Saved }`.

Section 3.5: `PlaneFallback` texts, `{reason}` from recompute's diagnosis:

- `BeforeRemoval`: `The face this sketch is on, {element}, {reason}, so the sketch stays where that face was.`
- `Saved`, missing: `The face this sketch is on, {element}, {reason}, so the sketch uses the position saved with it.`
- Not planar: `The face this sketch is on, {element}, is no longer flat, so the sketch uses the position saved with it.`
- Ambiguous or not coplanar: `The face this sketch is on, {element}, now matches {n} faces that do not lie in one plane, so the sketch uses the position saved with it. Pick the one you mean.`
- Fixes: `Attach` for each candidate face ("Attach to {description}"), `Pick`, and the reason's fixes (`Unsuppress`, `Move`). The engine adds "Keep the sketch where it was" to the cause line when the session saw the sketch elsewhere (engine.md section 3.21).

`SketchTurnedOver` (code `sketch_turned_over`, warning): `The face this sketch is on, {element}, now faces the other way, so the drawing is mirrored on it.` Fix `TurnDrawingBack`, labeled "Turn the drawing back": one undo entry that mirrors every stored entity across the sketch's x axis, stores the current frame as the fallback in body coordinates, and re-anchors the profile regions of the features that use the sketch (engine.md section 3.22).

Acceptance tests: (a) a face sketch on a box top with `fallback_frame: body` and a Move body of (100, 0, 0) inserted before it keeps its point at world (115, 15) (corpus R32a), and with a 90 degree rotation at (-15, 15); (b) the same document with `fallback_frame` absent (as written before format version 2) rebuilds exactly as before this change: the point stays at world (15, 15) and the document text is unchanged after recompute; (c) R33: placement origin (0, 0, 40), warning naming 'Extrude 1', document text unchanged; (d) R26 after the delete: placement equals the saved fallback, `placed: Saved`; (e) R32b: warning `sketch_turned_over`, and after `TurnDrawingBack` the hole axis is at (20, 10) with no warning; (f) a face split into two coplanar pieces keeps its placement; a reference that now resolves to faces in two planes gives `plane_fallback` with both offered; (g) a sketch on a face of a pattern copy follows a spacing change from 60 to 80 (the copy face's motion).

**3.5.2 features/chamfer.md.** Section 3.3 step 4: replace the first sentence with: "Face assignment for two distances: for each edge, the first face is the adjacent face that corresponds to the first face term of the stored reference the edge was resolved from: the face in the family of that term, through canonical names (naming.md section 3.9). When no face corresponds (the edge was reached by continuation or relation), the first face is the one whose name sorts first, and the chamfer adds a `ReferenceFollowed` notice." Test 2 keeps its result; add corpus R22 (a) and (b) as tests 15 and 16, each also with the join's feature id smaller and larger than `f-box1` (the side never depends on ids). Open issue 1 is resolved by this rule.

**3.5.3 features/hole.md.** Section 3.3 step 4: replace "(unknown: `UnknownEntity { field: \"points[i]\", noun: point }`)" with "(a point that the sketch no longer has is skipped with a `PointMissing` warning, with the fixes `RemoveEntry` at `/points/i` and `Pick`; when no point is left, `UnknownEntity` on `points[0]`)". Section 4 row "Unknown point" becomes the warning, plus the error row "No point left". `HoleMissesBody` reports the point's sketch coordinates and offers "Turn on Reverse" only when the reversed hole reaches the body (one kernel check on the warning path, cached with the result). Test 15 keeps its error (its only point is unknown); add a test with points `[e-p1, e-p9]`: warning `point_missing`, one hole at `e-p1` (corpus R43b); tests 7 and 32 change their texts to `The hole at ({u}, {v}) in 'Sketch 1' does not touch body 'Body 1'.`, with "Turn on Reverse" offered only in test 7's reversed variant.

**3.5.4 features/extrude.md and features/revolve.md.** Add to the naming section: "Successions of the profile match become aliases of the lateral faces (naming.md section 3.6.2)", and the document tests: R06a (a side line deleted and redrawn keeps a fillet on its top edge with a `reference_followed` notice), R07a (the next unrelated edit keeps it), R49 (a split side line keeps a shell opening the whole side, with a notice), and R66 (section 3.10: a side line replaced by a notch gives `reference_widened` with "Keep all 5", "Use only ..." and "Pick", and builds the fillet on all five top edges). The extrude records `absent` faces: `end-cap` with `Through` when a cut's end lies outside the body (R12), and the revolve records its caps with `FullTurn` at 360 degrees (R57).

**3.5.5 features/fillet.md, chamfer.md and shell.md.** The limit search and its messages change as section 7.3 states; each spec adds the newcomer corpus scenarios NC05 and NC06 as document tests.

**3.5.6 features/linear_pattern.md, circular_pattern.md and mirror.md.** Each copy's naming map is `with_instance(feature, instance, transform)` with the copy's transform (naming.md section 3.5.1). Add a test each: a face sketch on a copy face follows a spacing or angle change; on a mirror copy face it gives `sketch_turned_over` under decision D3's recommended rule.

**3.5.7 features/datum_plane.md, revolve.md, linear_pattern.md, circular_pattern.md, move_body.md, split_body.md.** The axis and face rules of features.md section 3.6 apply. Add a test each: an edge axis whose wall leans 1 mm either way keeps its direction (corpus R38 for linear_pattern); a 180 degree Move body inserted before the feature turns the axis with the part when `direction` is stored, and keeps today's direction for a file without it; a reference that resolves to two non-collinear edges gives `not_collinear` with both offered.

**3.5.8 features/move_body.md.** The output map is `NamingMap::moved(transform)` (naming.md section 3.6.12); add a test that a face sketch after it keeps its point on the moved face.

### 3.6 recompute.md

**RC1. Section 3.5, running: body histories.** Add after step 5: "The running state also keeps, per body, its writer chain: for each active writer that ran, was reused or failed, the writer's id and the body's state just before it. Step 7 passes each declared body's chain to the build as `BuildInputs::history` (features.md section 2.6)." Add to section 3.4's consequences: "A build that reads a declared body's history is still a function of its key: the body's provider key hashes the provider key of every earlier writer of the body (item 8, recursively), and each state in the chain is the output of one of those writers. No key changes." The chains are `Arc`s of states the cache already holds, and they also give the loss diagnosis (section 3.10) the naming maps it walks. `plan` is unchanged.

**RC2. Section 3.5 step 11: replace with:** "**Status.** `Ok` without warnings or with info notices only, `Warning` with a warning of severity `Warning` (including `UnknownParams`, added here and not cached), `Error` with an error."

**RC3. Section 2: replace `Message` and `Severity` with:**

````markdown
```rust
/// A rendered message.
pub struct Message {
    /// Info, warning or error.
    pub severity: Severity,
    /// Stable snake_case code, for example "reference_missing".
    pub code: &'static str,
    /// JSON Pointer of the field it is about, relative to the feature record, for example "/edges/1".
    pub pointer: Option<String>,
    /// The message text, rendered with current names and units and, for losses, the diagnosed reason.
    pub text: String,
    /// Replacement expression for the field at `pointer`, in display units, when the feature computed a limit.
    pub suggestion: Option<String>,
    /// Repair data for lost references and regions.
    pub repair: Option<Repair>,
    /// One-click fixes in the order to offer them: the error's or warning's `fixes(units)` merged with the fixes of `reason` (section 3.10).
    pub fixes: Vec<Fix>,
    /// Technical text for reports (features.md `detail`), never shown as the message.
    pub detail: Option<String>,
    /// The first failing feature of an `upstream_failed` chain, or the sketch that lost its face under a `no_effect`, so hosts group one root cause as one problem.
    pub root: Option<Id>,
    /// Why a reference was lost, or which feature a followed reference followed (section 3.10).
    pub reason: Option<LossReason>,
}

/// Severity of a message.
pub enum Severity { Info, Warning, Error }
```
````

Section 3.6: "Messages are rendered with `severity()`, `fixes(units)` merged with the reason's fixes in the order of features.md section 4.4, `detail()` and the diagnosis of section 3.10: error first, then warnings, then info notices."

**RC4. New section 3.10, loss diagnosis:**

````markdown
### 3.10 Loss diagnosis

After the run, recompute explains: every `Loss` in the reports (`ReferencesNotFound`, `ReferenceNotFound`, `ReferenceLost`, `PlaneFallback`); every `ReferenceFollowed` and `ReferenceWidened` (`by`); every `BodyEmpty` and `ToolEmpty` (`because`); every `UpstreamFailed` (`root`); and every `NoEffect` or `HoleMissesBody` of a feature whose profile or point sketch reports `PlaneFallback` (`root` is that sketch, and the text says so). The explanation goes on the rendered `Message` (`reason`, `root`, the reason's fixes and the text), never into the feature's error or warning: those are cached under the feature's key, while the explanation reads records outside it.

The diagnosis reads only the document, the graph, the upstream outputs (their `absent` faces, sketch outputs and body updates) and the naming maps of the body histories of section 3.5: names, aliases, and the adjacency that edge names give (`edge(a|b)` makes `a` and `b` adjacent). It never reads fingerprints, element infos, topology or the kernel. Explanations are memoized in the `Recompute` under a key made of the feature's cache key, the stored reference, the provider keys of the writers it walked, and, for each record it consulted, its state, whether it ended with an error, and its timeline order relative to the feature. A run with the same losses reuses every explanation.

For a loss of feature `F` in body `B`, with `t` the first part of `lost` (a lost part, a piece-lost part, or for `unmet` the query itself) and `g` the feature id that starts `t` (for an instance name, the feature of its last `@` group):

1. `g` names no record of the document: `NotInDocument { feature: g }`. Whether it was deleted is session knowledge, which the engine's cause line adds (engine.md section 3.21).
2. `g` is suppressed: `Suppressed`. `g` comes after `F`: `ComesLater`. `g` is rolled back: `RolledBack`. `g` ended with an error: `UpstreamFailed { feature: g, root }`, with `root` followed through `UpstreamFailed` errors to the first feature whose error is its own.
3. The reference's parts are present by name (through aliases and merged names) in the naming map of another body's state just before `F`, and for an edge or vertex that map names an element joining them: `OtherBody { body, by, by_suppressed }`, with `by` the last record before `F` that writes `B` and reads or writes that body (a split or a boolean), suppressed or not, and `by_suppressed` true when it is suppressed. Otherwise, when `g` is active, before `F`, and writes other bodies but not `B` (its operation or target changed): `OtherBody { body: the first body g writes, by: g, by_suppressed: false }`.
4. `t` is an instance name, and the output of `g'`, the feature of its last `@` group, names nothing with that instance group: `CopyGone { feature: g', instance }`.
5. `t`'s generator is a sketch entity of the sketch `g` reads (its first feature link to a sketch) that the sketch's output no longer has: `EntityDeleted`. For a hole point: the sketch no longer has the point, `PointGone { deleted: true }`; it has it but `g`'s output names no face with it, `PointGone { deleted: false }`.
6. `g` is an `import` and `t` is a fallback name: `NotInFile`.
7. Otherwise walk the writers of `B` from `g` to `F` in timeline order, with each writer's input and output naming maps: the *remover* `W` is the first writer whose input has a face matching `t` and whose output has none. A face matches `t` when it is in the family of `t` by canonical names; for a piece-lost part, when it also passes the anchor test of naming.md section 3.9 step 2 against the anchors live in that writer's input; for `unmet`, an edge name joining the parts takes the place of the face. `how`: `Rounded` or `Beveled` when `W`'s output names a face generated from a canonical form of the query (`W/fillet/...`, `W/chamfer/...`); `Hollowed` when `W` is a shell; `PieceGone` for a piece-lost part; `CutAway` when `W`'s update of `B` has change `Cut` or `Intersect`, or `W` is a split; `Covered` when it has change `Join`; `NoLongerMade { absence }` when `W` is `g` itself, with `absence` from `g`'s `absent` faces when one matches `t`; `NoLongerMeet { between }` for `unmet` otherwise, with `between` a face of `W`'s output adjacent to a member of every part.
8. When no remover is found: `Unknown`.

For a `ReferenceFollowed` or `ReferenceWidened`, `by` is the first writer of `B` from `g` on whose output records the alias the resolution went through (`via`), names the continuing rim, or names the relative; for a succession alias (between lateral faces of one extrude or revolve), `by` is the sketch that writer reads, because the sketch edit is what the user did. The reason is `Followed { by }`.

`because` of `BodyEmpty` and `ToolEmpty`: the last record before `F` whose `bodies_written` contains the body and that failed (`Failed`), is suppressed (`Suppressed`), or emptied it with change `Clear` (`UsedAsTool`); when there is none, the first record after `F` that writes it (`ComesLater`).

```rust
/// Why a reference was lost.
pub enum LossReason {
    /// Not diagnosed.
    Unknown,
    /// The feature that made it is not in the document.
    NotInDocument { feature: Id },
    /// The feature that made it is suppressed.
    Suppressed { feature: Id },
    /// The feature that made it comes after this one in the timeline.
    ComesLater { feature: Id },
    /// The feature that made it is at or after the rollback marker.
    RolledBack { feature: Id },
    /// The feature that made it failed; `root` is the first failing feature of the chain.
    UpstreamFailed { feature: Id, root: Id },
    /// It is now in another body, put there by `by` when known.
    OtherBody { body: Id, by: Option<Id>, by_suppressed: bool },
    /// A feature between the one that made it and this one removed it.
    Removed { by: Id, how: Removal },
    /// The sketch entity that made its face was deleted.
    EntityDeleted { feature: Id, sketch: Id, entity: Id },
    /// A hole no longer uses the point that made it.
    PointGone { feature: Id, sketch: Id, point: String, deleted: bool },
    /// A pattern or mirror no longer makes the copy it was on.
    CopyGone { feature: Id, instance: Instance },
    /// An imported file no longer has it.
    NotInFile { feature: Id },
    /// For a followed or widened reference: the feature whose change it followed.
    Followed { by: Id },
}

/// How a feature removed an element.
pub enum Removal { CutAway, Covered, Rounded, Beveled, Hollowed, PieceGone, NoLongerMade { absence: Option<Absence> }, NoLongerMeet { between: Option<ElementName> } }
```

`{reason}` renders, with `{this}` the feature's manifest label in lowercase after "this" (`this fillet`):

| Reason | Text |
| --- | --- |
| `NotInDocument` | `is made by a feature that is not in this document`, or `is gone` when the element description already says so ("a face of a feature that is not in this document") |
| `Suppressed` | `belongs to '{feature}', which is suppressed` |
| `ComesLater` | `is made by '{feature}', which now comes after {this}` |
| `RolledBack` | `is made by '{feature}', which is after the rollback marker` |
| `UpstreamFailed` | `is missing because '{feature}' failed`, and ` ('{root}' has an error)` when the root differs |
| `OtherBody` | `is now in body '{body}'`, and `, put there by '{by}'` when known, or ` while '{by}', which joins it to this body, is suppressed` when `by_suppressed` |
| `Removed`, `CutAway` | `was cut away by '{by}', which comes before {this}` |
| `Removed`, `Covered` | `is now inside the part, covered by '{by}'` |
| `Removed`, `Rounded` | `was rounded by '{by}', which comes before {this}` |
| `Removed`, `Beveled` | `was beveled by '{by}', which comes before {this}` |
| `Removed`, `Hollowed` | `was removed by '{by}', which hollows the part` |
| `Removed`, `PieceGone` | `no longer exists: that part of {face} was removed by '{by}'` |
| `Removed`, `NoLongerMade` | `is no longer made by '{by}'`, and `, which goes all the way through the body` (`Through`) or `, which turns all the way round` (`FullTurn`) |
| `Removed`, `NoLongerMeet` | `no longer exists, because its faces no longer meet`, and `: {between} now lies between them`; when `between` is a round face made from a sketch arc: `is already round: '{sketch}' now rounds that corner` |
| `EntityDeleted` | `is gone because that {line or curve} was deleted from '{sketch}'` |
| `PointGone` | deleted: `is gone because '{feature}' no longer makes a hole at that point, which was deleted from '{sketch}'`; not deleted: `is gone because '{feature}' no longer makes the hole at ({u}, {v}): that point was removed from its Points` |
| `CopyGone` | `is gone because '{feature}' no longer makes its {ordinal} copy` |
| `NotInFile` | `is not in the new version of the file imported by '{feature}'` |
| `Unknown` | `can no longer be found, and Partsmith could not tell why` |

Fixes added by reason: `Suppressed` gives `Unsuppress`; `ComesLater` gives `Move { feature: F, place: Below(feature) }`; `OtherBody` gives `UseBody` when the parts are present there, and `Unsuppress { by }` when `by_suppressed`; `UpstreamFailed` gives `Show { root }` and removes `Pick`.
````

**RC5. Section 5, budgets: add rows:**

````markdown
| Diagnosis of one lost reference on `bench50`, nothing memoized | 1 ms |
| `run` of `bench50` with every result cached and 5 lost references | 5 ms (the same as without losses: explanations are memoized) |
````

**RC6. Section 6, acceptance tests: add** one test per `LossReason` and `Removal` variant on a small document (each corpus scenario that section 3.8 maps to it); a memoization test (a cached run gives messages equal to a cold run, and a change of a consulted record's state, such as suppressing the remover, changes the explanation); a names-only test (a diagnosed run computes no `BodyState::elements` for states nothing else read, observable through `BodyState::measurements`, which gains `elements: bool`); the body-history test (a build that reads `body_history` is reused from the cache after an unrelated edit and rebuilt after an edit to an earlier writer of the body); and the status rule of RC2 (a feature with only a `reference_followed` notice has status `Ok`).

### 3.7 File format, Python and CLI

**FF1. file-format.md section 3.9:** `FORMAT_VERSION` becomes 2 and `OLDEST_FORMAT_VERSION` stays 1. Version 2 adds, all optional: `nb` and `merged` on references (naming.md N2), `outline` on region references (sketch.md S1), `fallback_frame` on face sketch planes (features/sketch.md section 2) and `direction` on edge and face axes (features.md section 2.5). `MIGRATIONS` gets one step from 1 that returns the document unchanged: every version 1 document means the same at version 2, because absent fields keep the old meaning (world-coordinate fallbacks, no anchors, no outline, the fingerprint axis rule). Older builds reject version 2 files with `NewerFormatVersion` instead of a serde "unknown field" error. When `load` migrates, the engine tells the user that saving will write version 2 (existing rule). Tests change: 23 uses `"format_version": 3` (`NewerFormatVersion { found: 3, supported: 2 }`, message "... (up to 2) ..."); 39 becomes "`migrate` of E1 at version 1 returns it with `format_version` 2 and `Some(1)`"; E1 to E3 are written at version 2; new tests: a version 1 document with every new field absent loads, saves at version 2, and differs from its input only in the `format_version` line; a version 2 document with `nb`, `merged`, `outline`, `fallback_frame` and `direction` round-trips byte for byte; `document.schema.json` describes the new fields. A recompute test opens a version 1 document with a Move body before a face sketch and checks that it rebuilds with the same placements, volumes and statuses as before this change.

**PY1. python-api.md section 2.8 and 3.8 rule 9:** `Ref(q, fp, *, part=None, anchors=None, merged=None, direction=None)`, `Region(key, fp, *, outline=None)` and `Sketch(plane, *, fallback=None, fallback_frame=None, ...)`; generated scripts write each keyword only when the stored value has it (`anchors={"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]}`, `merged={"f-box1/top": ["f-box1/top@f-lp1:*"]}`, `outline="e-1,e-2,e-3,e-4"`, `fallback_frame="body"`, `direction=(0, 1, 0)`). The round-trip test of section 3.9 adds a document with each of them, so GUI to Python to `.ptsm` keeps every reference's resolution.

**PY2. python-api.md section 3.6, selectors:** a `Selection` compiles each element to the full reference of `pick_reference` (anchors and merged names), not the short form of `list_elements`. Indexing a `Selection` of more than one element with an integer (`edges()[3]`, `faces()[-1]`) emits a `PartsmithWarning`: `edges()[3] picks an edge by its position in the list, which can change when anything before it changes. Select it by what it is, for example edges().top().parallel_to(Axis.X).` Iteration, slicing and `len` do not warn. Generated scripts always write queries or `Ref(...)`, never an index. New tests: the warning text and its `code` (`selection_index`); iterating does not warn; a generated script never contains `)[`.

**PY3. python-api.md section 4, errors:** `FeatureError` and `PartsmithWarning` gain `actions` (labels), `candidates` (each a `Ref` with `description` and `relation`), `cause` (None in scripts unless an edit in the same session started it) and `detail`; `code` and `suggestion` are always set from the diagnostic. The message starts with the call that failed and names the user's line.

**PY4. python-api.md section 2.4:** `Document.set_units(units, *, keep_sizes=False)` maps onto `set_document_units` (engine.md section 3.25); with `keep_sizes=True` every number without a unit gets the old unit, so no size changes. Test: NC09 in Python.

**CLI1. cli.md:** `rebuild` and `validate` print every feature message with its location, then one line per action (`suggested: {label}`) and per candidate (`candidate: {q}, {relation}`); `--json` carries `code`, `field`, `actions` (label and kind), `candidates` (reference, description, relation, meaning, distance, score), `reason`, `root` and `detail`. `validate` reports lost, ambiguous and guessed references as warnings with exit code 1 (a document that rebuilds with a lost reference is not "ok").

### 3.8 How each failing scenario passes afterwards

"Structural message" is the text recompute renders from the diagnosis; "cause line" is the engine's line of section 5, checked by the engine twins of section 3.10 item 2. Phases are those of section 3.9.

| Id | Today | Fix and result after | Phase |
| --- | --- | --- | --- |
| R01 (5, 10 mm) | profile lost by area | Boundary match on the surviving `e-1` to `e-4`; one region, no succession, no notice; every size alike | 1 |
| R02 (20 x 10, r5) | profile lost by area | Boundary match; names unchanged; fillet and chamfer exact | 1 |
| R03 | profile lost; then a generic message | Boundary match; edges[1] exact; edges[0] lost with `EntityDeleted` ("is gone because that line was deleted from 'Sketch 1'"), repair `/edges/0`, fixes Remove from Edges and Pick; the cause line names the line by its coordinates with Show (phase 4) | 1 |
| R04 | profile lost | Coverage in one component: the big region and the inner half disc (the outer half disc has `e-3` on the other side); same total area, so an info notice; six original faces | 1 |
| R05 | profile lost | Coverage per reference: each part and the overlap, connected through the other rectangle's lines; one solid; the fillet's anchored piece covers its live anchors | 1 |
| R06a | silent follow by fingerprint, lingering warning | Succession `e-2 -> [e-9]`; fillet and sketch follow through the alias with a `reference_followed` notice and Keep this match; no region warning | 1 |
| R06b | profile lost | Boundary match; succession `e-2 -> [e-9]`; fillet at x=88, pocket centered at y=25, z=15 | 1 |
| R07a | silent move to the bottom edge | The succession alias is derived on every build, so the stale fingerprint is never read; back top edge at z=78 | 1 |
| R07b | profile lost | Boundary match regardless of area; the alias keeps the fillet on the back edge, x 0 to 160 | 1 |
| R08 (200, 400; a, b) | silent move to the other piece | Exact name confirmed by anchors (the piece covers `lateral/e-2` and the slot wall); a reference without anchors gets `reference_guessed`, never silence | 1 |
| R09 | shell opens the wrong piece | Exact name `end-cap#2` confirmed by anchors | 1 |
| R10a | silent fillet on the left part | Anchors: the remaining top does not touch `lateral/e-2`, so the piece is lost (`PieceGone` by 'Extrude 2'); the slot floor's front edge is the first candidate | 1 |
| R11 | wrong side of the notch | Exact name with anchors (`lateral/e-3`) | 1 |
| R12 (four) | hole entrance rounded silently | No substitution: `f-ext2/end-cap` has no family, continuation or relative; `NoLongerMade` by 'Extrude 2', "which goes all the way through the body"; rims offered as candidates, never applied; the cause line names Distance 5 mm to 30 mm with Set Distance back to 5 mm (phase 4) | 1 |
| R13 | generic message | `Covered` by 'Extrude 2' ("is now inside the part"); repair `/edges/0` | 1 |
| R16 | ambiguous, candidates dropped | `CutAway` by 'Extrude 1' ("was cut away by 'Extrude 1', which comes before this fillet"); meaning candidates the rabbet's two outer edges; ghost from the last build (phase 4) | 1 |
| R17a, R17c | wrong pieces | Anchors select the pieces of the old piece and leave the others | 1 |
| R18a, R18b | ambiguous, dropped | `Rounded` by 'Fillet 1' or `Beveled` by 'Chamfer 1'; candidates the two tangent edges | 1 |
| R19 | fillet fails | Continuation: `f-sh1/rim/f-ext1/end-cap`; outer edge, notice | 1 |
| R20a | ambiguous lookalikes | Pieces across the instance suffix: both segments of the copy | 1 |
| R20b | fillet fails | The anchored piece covers its live anchors on the unsplit copy top | 1 |
| R21a | generic message | `Covered` by 'Box 2' | 1 |
| R21b | silent cove | No substitution; the inside corner offered as a meaning candidate, never applied | 1 |
| R22 (a, b) | sides swapped silently | Chamfer side by the stored term order through canonical names | 2 |
| R23 (a, b) | peg B rounded silently | `Suppressed` with Unsuppress, or `NotInDocument` (the cause line says it was deleted); unsuppressing gives the byte-identical document | 1 |
| R24a, R24b | cube edge rounded silently | `Suppressed` with Unsuppress; `ComesLater` with Move 'Fillet 1' below 'Box 2' | 1 |
| R25 | generic message | `Suppressed` 'Chamfer 1' with Unsuppress; 'Sketch 1' and 'Extrude 1' report `upstream_failed` with root 'Datum plane 1' | 1 |
| R26 | boss inside the body, false message | After the delete: `NotInDocument`, the sketch at its saved position with a true message and "Attach to the top of 'Extrude 3'" first; the cause line says 'Extrude 1' was deleted and offers Keep the sketch where it was (z=80), amended expectation of section 3.10 | 4 |
| R27 | fillet jumps to the right piece | Exact name; anchors confirm; the move never mattered | 1 |
| R28 (two) | generic message | `ComesLater` 'Extrude 2' with Move 'Fillet 1' below 'Extrude 2'; dragging back gives the byte-identical document | 1 |
| R29 | widened to both pieces | Anchors: only `#1#1` covers the live anchors of the old `#1` | 1 |
| R30 (a, b) | pocket opening rounded silently | `NoLongerMade` by 'Extrude 2'; `CutAway` by 'Boolean 1'; no substitution | 1 |
| R31a | cut edge in body 1 beveled | `OtherBody` 'Body 2' by 'Split body 1'; no substitution | 1 |
| R31b | right by luck (0.003) | `OtherBody` with Use body 'Body 2' (present there by name); no threshold involved | 1 |
| R32a (move, rotate) | hole misses or moves | Body-coordinate fallback moved by the top's motion; hole at (115, 15) and (-15, 15) | 2 |
| R32b (reverse) | hole misses | Decision D3: the recommended rule warns that the drawing is mirrored and Turn the drawing back puts the hole at (20, 10) (amended expectation); the alternative keeps (20, 10) with a mirrored placement | 2 |
| R33 | boss inside, false message | Placed where the top was just before 'Extrude 1' cut it away (z=40); warning names 'Extrude 1' and offers to attach to the new top; bounding box max z 45; document unchanged | 2 |
| R34 | sketch drops to the floor | Continuation to the rim `f-sh1/rim/f-box1/top` in the same plane, notice naming 'Shell 1' | 1 |
| R37 | datum fails | Canonical names through nested aliases: `f-cha1/chamfer/edge(f-a1/front\|f-a1/top)` | 1 |
| R38 | copy jumps to -Y | Axis keeps the side of the stored direction | 2 |
| R41 (two) | generic message, no repair data | `CopyGone` ("no longer makes its third copy"); the cause line offers Set Count back to 3 (phase 4); variant keeps its repair data | 1 |
| R43a | neighbor chamfered | No substitution; relatives never cross generators; `PointGone` | 1 |
| R43b | whole hole fails | Hole skips the deleted point with `point_missing`; chamfer `PointGone { deleted: true }` | 3 |
| R46a | straight edge chamfered | Fallback name below the threshold is lost; `NotInFile` | 1 |
| R46b | candidates dropped | Ambiguous loss keeps both rims as candidates | 1 |
| R47 | hole rim rounded silently | `UpstreamFailed` 'Extrude 3' with root 'Sketch 3'; Show, no Pick | 1 |
| R48 | limit in inches only | Bare-number sentence and the fix Use 2 mm | 3 |
| R49 | shell fails as too thick | Extension `e-2 -> [e-2, e-6]` on one line: both halves removed, one opening, notice | 1 |
| R50 | through hole with a vague warning | Coverage with fill: ring and disc, same area, notice; decision D2 | 1 |
| R51 | profile lost; then generic | Boundary match; fillet lost with `EntityDeleted`; candidates the top edges of the right and back sides, meaning first | 1 |
| R52 (a, b) | other parts dropped | Merged names resolve every former part | 1 |
| R53a, R53b (t=5, 10) | covered face replaced, or generic | `Covered` by 'Box 2' with Use the top of 'Box 2' for every t; root 'Datum plane 1' for (b) | 1 |
| R54 | generic message | `NotInDocument` (the cause line says it was deleted); root 'Datum plane 1'; block top offered, not applied | 1 |
| R55 | void, generic message | `ComesLater` with Move 'Sketch 2' below 'Extrude 1' (phase 1); `closed_void` on the boss (phase 3) | 3 |
| R56 (a, b, c) | generic or no cause | `OtherBody` with Unsuppress 'Boolean 1'; `BodyEmpty` because 'Boolean 1' uses it as a tool, with Use body 'Body 2'; `OtherBody` because 'Extrude 1' makes 'Body 2' | 1 |
| R57 | generic messages | `NoLongerMade` by 'Revolve 1', "which turns all the way round", for the chamfer and the sketch | 1 |
| R58 | datum fails | Continuation to the rim, notice naming 'Shell 1' | 1 |
| R59 (two) | "now matches 2 edges" | `Rounded` by 'Fillet 1'; tangent edges as candidates; no axis guessed | 1 |
| R61a, R61b | no notice | `Merged` follow notice | 1 |
| R63a | floor edge chamfered silently | Ambiguous by meaning (mouth and bore edge), never applied; mouth first | 1 |
| R63b (d=6, d=3) | fingerprint luck, no notice | Related: the same hole's mouth, notice, both diameters | 1 |
| R64 | profile lost; then generic | Boundary match; `NoLongerMeet` with the arc's round face between: "is already round: 'Sketch 1' now rounds that corner" | 1 |

### 3.9 Phases and expected results

"Built correctly before, fails after" counts tests whose geometry was right before the phase (passing, or failing only on the message) and is wrong or missing after it. It must be 0 at every phase exit; the tests at risk are the 35 that pass today, the 25 right-geometry tests with unclear messages, and those right only by luck: R01 at 0.5 mm, R02 with the 2 x 1 notch, R06a, R08 at width 120, R10b, R38 leaning one way, R61a, R61b and R63b at d=6. This is why the resolver, anchors, region boundaries and successions ship together: without anchors R10b and R08 at 120 lose the luck that holds them today, and without successions R06a loses its fillet.

| Phase | Content | Specs | Corpus passing | Silent wrong left | Built correctly before, fails after |
| --- | --- | --- | --- | --- | --- |
| Today | | | 35 | 38 | |
| 1 | Resolve by meaning and explain every loss, delivered to every host: the whole resolver (exact with anchors and the guess rule, pieces, continuation, relatives, merged names, canonical index), anchors and merged names stored by every pick, region boundary and coverage matching with successions, losses with candidates and reasons, the message rewrite of losses, format version 2, the protocol's actions and repairs, `apply_fix`, CLI and Python output, notifications and repair mode in the UI | naming N1 to N16 (motions built, used from phase 2), sketch S1 to S4, features F1 to F3, F5 to F7, F9 (loss rows), F10, F12 to F14 (frames parsed and kept, not yet honored), per-feature 3.5.4, recompute RC1 to RC6, FF1, PY1 to PY3, CLI1, engine E1, E2, E4, E8, protocol PR1 to PR6, ui U1 to U6 | 112 | 4 | 0 |
| 2 | Frames on the model: motions, body-coordinate fallbacks, placement before removal, turned-over faces, axis directions, coplanar and collinear checks, chamfer sides | features F4, per-feature 3.5.1, 3.5.2, 3.5.6 to 3.5.8, engine E2 (descriptor frames), decision D3; the motions of naming N3, N7 step 8, N8 and N10, built in phase 1, are read from here | 119 | 0 | 0 |
| 3 | Newcomer errors and feature rules: hole points, units sentence and Keep sizes, closed voids, separate solids, no-effect causes, limits that name their face, open outlines, sketch messages, kernel texts, the modeling limit, did-you-mean | features F8, F9 (all other rows), F11, per-feature 3.5.3, 3.5.5, sections 7.2 to 7.6, kernel K1 to K4, engine E7, PY4, ui U10 to U12 | 122 | 0 | 0 |
| 4 | Causes, ghosts and prevention: baselines per message, cause lines with their actions, ghosts captured at the loss, `list_uses`, the timeline drop warning, Used by, used marks in sketches, limits under fields | engine E3, E5, E6, E9, E10, ui U7 to U9, U13, U14 | 123 | 0 | 0 |

Phase 1 is the single most valuable change: it removes 34 of the 38 silent wrong results with no frame or kernel change, because it stops guessing and starts explaining. The four left (R22 in two variants, R38, R32a rotated) are frames anchored in world space or byte order, which phase 2 fixes.

### 3.10 Corpus and test amendments (human approval needed)

1. **References as picks store them.** The corpus writes references as `{q, fp}` and regions as `{key, fp}`. After this change a pick stores anchors, merged names and the region outline, and an attach stores `fallback_frame: body`. Test setups re-pick each reference by name at setup time with `Reference::from_element`, `Region::reference()` and the engine's descriptor fallback (what `pick_reference` and the sketch editor store), so the inputs match real documents; expected outcomes do not change except as listed below. References in the old form are tested by naming tests 69 to 77 and 96 (the guess and loss rules) and by the version 1 rebuild test of FF1.
2. **Engine twins.** Cause lines, cause actions and "Keep the sketch where it was" need the session. Each of R03, R10a, R12, R13, R16, R23b, R26, R33, R41, R48, R54 and R57 gets a twin in `crates/partsmith-engine/tests/reference_causes.rs` that opens the setup, applies the corpus edit with `apply_edits`, and checks the cause line and its actions; the recompute tests check the structural message, which says "a feature that is not in this document" where the corpus says "deleted" (R23b, R26, R54), the twin checking "This started after you deleted '...'".
3. **R26 expectation.** After the delete, the sketch uses its saved position (z=30) with the `plane_fallback` message `The face this sketch is on, a face of a feature that is not in this document, is gone, so the sketch uses the position saved with it.` and the fixes Attach to the top of 'Extrude 3' first, then Pick; 'Extrude 2' reports `no_effect` with root 'Sketch 2' (`This extrude adds nothing to body 'Body 1' because 'Sketch 2' lost the face it was on. Fix 'Sketch 2' first.`). The engine twin checks the cause line "This started after you deleted 'Extrude 1'." and its action Keep the sketch where it was, after which the boss is at z 80..90 (volume 323141.592654). Nothing moves without the user, because the run alone cannot tell that 'Extrude 3' replaces 'Extrude 1'.
4. **R32b expectation** follows decision D3: with the recommended rule, `sketch_turned_over` and, after Turn the drawing back, the hole axis at (20, 10) from z=-30 to z=-20, volume 119803.650459; with the alternative, the original expectation.
5. **R66, new scenario (high): a side line replaced by a notch.** Setup: BLOCK; `f-fil1` fillet radius 1 on `[{q: edge(f-ext1/end-cap|f-ext1/lateral/e-3), fp: line|n=1,0,0|c=40,50,30|l=80|adj=2,2}]` (the back top edge). Edit: delete `e-3` and draw in its place `e-9` (80,50)-(50,50), `e-10` (50,50)-(50,40), `e-11` (50,40)-(30,40), `e-12` (30,40)-(30,50), `e-13` (30,50)-(0,50). Expect: the profile matches by boundary with the succession `e-3 -> [e-9, e-10, e-11, e-12, e-13]`; `f-fil1` warning `reference_widened` at `/edges/0`: `The back top edge of 'Extrude 1' selected for Edges now covers 5 edges, because the line it was made from was replaced by 5 lines in 'Sketch 1'. Keep all of them, or use only one.` with the actions Keep all 5, Use only the back top edge of 'Extrude 1' along the line from (80, 50) to (50, 50), and Pick an edge again; the fillet is built on all five top edges until the user chooses. A replacement by one line (R06a) stays a notice.
6. **Corpus text.** Correct the "Today" of R20a, R31b and R49 (section 2.1), record decisions D2 (R50) and D3 (R32b), and note that R10b's optional notice is not emitted (a grown piece resolves without a notice).
7. **Naming tests** 61 to 77, 80 and 89 change as listed in N16.
8. **Newcomer error corpus.** Section 7.6 becomes `docs/design/newcomer-error-corpus.md`, run in recompute, engine and Playwright tests like this corpus.

## 4. Error quality results

### 4.1 Inventory counts

The inventory covers 260 messages: feature errors and warnings, kernel texts that reach users through features, sketch and recompute messages, file loading, engine protocol errors, export and import, print findings, the CLI, the Python API and the desktop host. Each was triggered where possible (otherwise read from code) and judged on five questions.

| Question | Entries failing it | Share |
| --- | --- | --- |
| Says what happened | 12 | 5% |
| Says why (the cause, in the user's terms) | 117 | 45% |
| Says what to do | 95 | 37% |
| Offers a one-click fix | 246 | 95% |
| Free of jargon, ids, element names and raw internal text | 123 | 47% |

- 105 entries (40%) say what, why and what to do; 58 (22%) do that without jargon; 10 (4%) also offer a one-click fix: the fillet, chamfer and shell "too large" searches, Angle above 360 deg, Through all pointing away, the two `format_version` load errors, and the bed-size and enclosed-hollow print findings.
- The 17 messages about lost references, regions and sketch placements (the owner's first concern): 14 give no cause, none offers a fix, and none names the lost element in words. The CLI and Python forms drop the repair data altogether, and the resolver's silent fingerprint match has no message at all.
- The 12 kernel failure texts that reach users through features (cutting, joining, intersecting, extruding, revolving, profiles, primitives, transforms, splits, grouping, measuring, queries) all fail "why"; 5 also fail "what to do" ("The box could not be built.").
- Sketch messages are accurate but almost all use constraint and entity ids (`c-3`, `e-5`) as their subject: 28 of 31 have jargon.
- File loading stacks two or three prefixes ("The file is not a valid Partsmith document: This document has a problem:") over raw serde text with backticks, and never gives a line for structural problems.

### 4.2 Worst offenders

Ranked by harm: a message that misleads is worse than one that says nothing, and silence about a wrong result is worst.

1. **Silence about a wrong result.** A reference resolved by fingerprint only reports status ok (inventory: "naming resolver, all hosts"; corpus: 38 tests). The Python form of the same problem: `fillet(block.edges()[3], 2)` selects by position, the FreeCAD pattern in script form, with no warning.
2. **False statements.** `plane_fallback` "the sketch stays at its last position" while it moves to its attach-time position (R26, R33). `body_empty` "Use op new to start it" when the body is empty because its first feature failed or was suppressed. The extrude kernel text "Check that the profile is a closed region that does not cross itself" for a failure caused by a 1e200 mm distance (built-in profiles are always closed). `references_not_found` says "can no longer be found" for an ambiguous reference. Every lost-reference message points to a "repair panel" that does not exist. `unknown_params` implies a newer Partsmith wrote a setting that is really a typo (`remove_face`), while the shell silently becomes a closed hollow. `shell_too_thick` for a side face split by a sketch edit (R49). `fillet_too_large` "Try 1.9 in or less" after a units change made "2" mean inches (R48).
3. **Lost references without cause or candidates.** "The edge selected for Edges can no longer be found. Re-pick it in the repair panel." and its siblings: no element in words, no cause, no candidates (dropped twice: by `resolve_all` for whole lists and by the engine's projection), no ghost when the list failed.
4. **Raw internals in the sentence.** Kernel exceptions ("(StdFail_NotDone: BRep_API: command not done)"), panic text ("called `Option::unwrap()` on a `None` value"), naming limits ("An element name would be 16385 bytes long"), serde text ("invalid type: integer `20`, expected a string"), kernel indices ("Edge 12 is a seam", the very numbering the owner wants gone), temporary file paths and OS error numbers ("(os error 2) at path .../.partsmith-nQaQjQ").
5. **Fragments.** "The box could not be built.", "Moving the body failed." (shown under Mirror and patterns, where nobody moved anything), "The bodies could not be grouped.", "The selected geometry could not be measured." (under a Box, which has no selection).
6. **Chains that blame the next link.** `upstream_failed` names the direct input ("'Sketch 1' has an error") when the root is a datum plane two steps up; `depends_on_failed` names the next parameter, not the one that divides by zero; the Problems panel shows one row per downstream feature with an Edit button that opens the wrong feature.
7. **Detail thrown away.** `Diagnostic` has no detail field, so the kernel cause of a fillet failure, the stored reference and the resolver scores never reach a bug report; `partsmith validate` prints "ok" for a document whose fillet edge no longer resolves; `rebuild --json` omits repairs; Python's `FeatureError.code` is None and drops the suggestion.

### 4.3 What fixes the inventory

| Class | Fix | Where |
| --- | --- | --- |
| Lost references, regions and placements (17 entries) | Element in words, loss reason, cause line, candidates described by meaning, ghost, fixes, a notification for every new loss | sections 3, 5, 6 |
| Upstream chains and empty bodies | `root` and `because`; one problem per root; downstream `no_effect` points at the sketch that lost its face | recompute RC4, ui U6 |
| Kernel texts through features | "Partsmith's geometry kernel could not build this {type}" with a hint that is true for the operation; kernel text to `detail`; sizes checked before the kernel | sections 7.2 and 7.5 |
| Internal errors and host bugs | "Partsmith hit a bug while ..." with what is safe, a way to keep working, and Copy details | section 7.2, engine E8 |
| Newcomer errors: open outlines, wrong planes, limits, conflicts, thick shells | A cause and fixes computed by the core, and a newcomer error corpus | section 7 |
| Params, file loading and validation | Field labels and lines, did-you-mean, bare numbers accepted in expression fields, no stacked prefixes, parser text to `detail` | section 7.2 (`invalid_params`, `unknown_params`), engine, model and cli specs (follow-up) |
| Sketch (31 entries) | Constraint titles with values ("Distance 10 mm") and entities by kind and coordinates instead of ids, ids in detail, conflicts that name their partners | section 7.4 |
| Ranges and limits | Enough digits that value and bound differ; fixes in both directions; expressions shown with their parameters; a modeling limit | sections 7.2 and 7.5 |
| Export, import, print checks, CLI, Python and desktop | Name the body or file, say the next step per host, drop OS error numbers and temporary paths, merge identical findings, no finding without an action | per-host specs (follow-up), CLI1, PY1 to PY3 |

Every rewrite follows [the error guidelines](../guidelines/errors.md), whose examples are taken from this inventory and the newcomer error corpus.

## 5. Cause attribution

### 5.1 Two kinds of why

A message answers "why" twice:

- **What in the model did it**, from the current run alone: "'Extrude 1', which comes before this fillet, cuts it away." This is recompute's loss diagnosis (section 3.6, RC4). It is a pure function of the document, so the CLI, Python, a reopened file and the UI all say the same thing, and the message's actions are computed the same way everywhere.
- **Which edit started it**, from the session: "This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm." Only the engine knows what the document was before, so this is a separate, optional `cause` on the diagnostic, shown as its own line with its own actions (reverts, Undo, Keep the sketch where it was, Show). It never changes statuses, messages, the message's actions or results, so determinism is kept: the same document gives the same `message` and `actions` in the UI, in `rebuild --json` and in Python.

The first answers the owner's "what caused it"; the second answers "why did this break just now", the question FreeCAD users spend hours on.

### 5.2 The engine's baseline

For each open document the engine records, after every recompute, a snapshot of each feature's inputs: its canonical record, label and timeline position; the display text of each declared expression's value; the document units; the values of the parameters it uses; the snapshots of its feature links and body providers; the asset names it reads; for a sketch, its solved entities; for a face sketch, the placement it was built with. For each feature and each message key it has now (code and field), the engine keeps the *clean snapshot*: the snapshot of the latest recompute in which the feature had no message with that key. Snapshots share structure through `Arc`, so unchanged features cost nothing. A feature that leaves the document keeps a tombstone (label, record, timeline position) until the document is closed. Baselines start empty at `new_document` and `open_document`; undo and redo update them like any recompute. They are session memory only: never saved, never in cache keys.

### 5.3 Attribution

After each recompute, every message of a feature `F` (of any severity) whose key has a clean snapshot that differs from `F`'s current inputs gets a cause:

1. **Changes that can reach `F`** between the clean snapshot and now: `F`'s own params fields whose canonical value changed; each feature `U` upstream of `F` now or in the snapshot (feature links and body providers, transitively) that was added, deleted (a tombstone), suppressed or unsuppressed, moved relative to `F`, or edited; each parameter used by `F` or such a `U` whose value changed; the document units, when `F` or such a `U` has a number without a unit; each asset of such a `U` whose name changed.
2. **Primary cause.** The first that applies: a change of a feature the message names (its reason's `feature` or `by`, its `root`, its `because`); a change of `F`'s own record; a change of a feature `F` links to directly; the change of the feature latest in timeline order. Other changes are counted.
3. **Text.** `This started after you {change}.`, where `{change}` is one of: `changed {label} from {old} to {new}` (a field of `F`), `changed {label} of '{feature}' from {old} to {new}`, `added '{feature}' before this {noun}`, `added '{feature}'`, `deleted '{feature}'`, `suppressed '{feature}'`, `unsuppressed '{feature}'`, `moved '{feature}' above '{other}'` or `below '{other}'`, `changed parameter '{name}' from {old} to {new}`, `changed the document units from {old} to {new}`, `replaced the file of '{feature}'`; for an edited sketch, its first entity or dimension change in stored order: `deleted {entity} in '{sketch}'`, `added {entity} in '{sketch}'`, `moved {entity} in '{sketch}'`, `changed {dimension title} of '{sketch}' from {old} to {new}`, where `{entity}` is `the line from ({u1}, {v1}) to ({u2}, {v2})`, `the circle at ({u}, {v})`, `the arc from ({u1}, {v1}) to ({u2}, {v2})` or `the point at ({u}, {v})` in document units, taken from the snapshot for a deleted entity. Several changes: the primary, then `, and {n} other changes`.
4. **Cause actions**, in this order, each only when it applies:
   - a revert of the primary change alone: `Set {label} back to {old}` (one field of `F`), `Set {label} of '{feature}' back to {old}`, `Set '{name}' back to {old}` (a parameter), `Unsuppress '{feature}'`, `Suppress '{feature}'` (for an added feature), `Move '{feature}' back below '{neighbor}'` (named by its former neighbor's id, so it stays right after other edits), and for a units change `Use {expr} {old unit}` for each field of `F` whose expression is a number without a unit;
   - `Undo` when the primary change is in the newest undo entry;
   - for a `plane_fallback`, `Keep the sketch where it was` when the snapshot's placement differs from the one used now: one edit that stores the snapshot's placement as the fallback in body coordinates (`fallback_frame: body`);
   - `Show`: the changed feature, or for a sketch edit the sketch with the changed entity's ghost (its geometry before and after, from the snapshot and the current solve), so "(80, 40) to (70, 50)" is seen on the screen.

Messages of features without a clean snapshot (broken since the document was opened) get no cause.

Examples from the corpus (actions in brackets, the cause line in italics with its own actions):

- R12: "The bottom edge of the cut made by 'Extrude 2' selected for Edges is no longer made by 'Extrude 2', which goes all the way through the body." [Use the top rim of the cut made by 'Extrude 2'] [Pick an edge again] *This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm.* [Set Distance of 'Extrude 2' back to 5 mm] [Undo]
- R16: "The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet." [Use the top edge of the cut made by 'Extrude 1'] [Use the front edge of the cut made by 'Extrude 1'] [Pick an edge again] *This started after you added 'Extrude 1' before this fillet.* [Suppress 'Extrude 1'] [Undo]
- R41: "The top right edge of the third copy of 'Linear pattern 1' selected for Edges is gone because 'Linear pattern 1' no longer makes its third copy, so it was skipped." [Remove from Edges] [Pick an edge again] *This started after you changed Count of 'Linear pattern 1' from 3 to 2.* [Set Count of 'Linear pattern 1' back to 3]
- R03: "The top edge of 'Extrude 1' along a line of 'Sketch 1' selected for Edges is gone because that line was deleted from 'Sketch 1', so it was skipped." [Remove from Edges] [Pick an edge again] *This started after you deleted the line from (80, 40) to (70, 50) in 'Sketch 1'.* [Undo] [Show the line]
- R48: "Fillet radius 2 in is too large for the selected edges: the largest that works is about 1.9 in. '2' has no unit, so it means 2 in in this document." [Use 2 mm] [Use 1.9 in] *This started after you changed the document units from mm to in.*
- R26: "The face this sketch is on, a face of a feature that is not in this document, is gone, so the sketch uses the position saved with it." [Attach to the top of 'Extrude 3'] [Pick a face] *This started after you deleted 'Extrude 1'.* [Keep the sketch where it was] [Undo]

### 5.4 engine.md changes

**E1. Section 3.4, feature status: replace "`messages` holds ... Candidate names are not sent." with:**

````markdown
`messages` holds recompute's rendered messages (error first, then warnings, then info notices), each with `field` from `Message::pointer`, `suggestion` from `Message::suggestion`, `actions` from `Message::fixes` (section 3.22), `detail` from `Message::detail`, `root` from `Message::root`, and `cause` from section 3.21. `repairs` holds one `Repair` per message with repair data: `element` from `Repair::Element` with its body, the element description, the centroid parsed from the stored fingerprint, the candidates (each with its reference, description, relation, `meaning`, distance, score and display element when the body has a model-layer mesh) and the ghost (section 3.23); `region` from `Repair::Region` with its description, candidate sets and ghost. `resolution` is `ambiguous` when the loss is ambiguous, else `missing`. A candidate's `relation` reads `on {kept face}, {distance} from where it was` for a meaning candidate and `{distance} from where it was` otherwise (`where it was` when the distance is below `LINEAR_TOLERANCE`), with the distance formatted in document units.
````

**E2. Section 3.9, descriptors: change** "Each descriptor holds the element name, its `Reference` (built by partsmith-naming from the name and the fingerprint ...)" to "Each descriptor holds the element name and its `Reference`: in `list_elements` the short form (`Reference::short`: query and fingerprint), in `pick_reference` the full form (`Reference::from_element` against the element set of its body at the display state, with anchors and merged names), so long anchor and merged lists never inflate element lists; `pick_reference` returns the descriptor of `list_elements` with the full reference." Add: "`fallback` (planar faces): the `placement` in body coordinates (the inverse of the face's motion applied), which hosts store as a face sketch's fallback with `fallback_frame: body`. `axis_direction` (line edges and faces with an axis): the direction in body coordinates that hosts store as `direction` when the element is picked for an axis." `end_sketch_drag` with `commit` writes the fallback the same way and sets `fallback_frame: body`.

**E3. New section 3.21, causes:** the baseline and attribution of sections 5.2 and 5.3 as spec text, with: "Causes are session state: they never enter documents, cache keys, statuses, messages or the message's actions. The `cause` of a diagnostic holds the text, the changed feature's id, the cause actions and, for a sketch edit in a display session, the changed entity's ghost."

**E4. New section 3.22, fix actions and `apply_fix`:**

````markdown
### 3.22 Fix actions

The engine projects each `Fix` of a message and each cause action into a protocol `FixAction`: a label and how to run it. Actions that change the document are `apply` actions carrying a `FixRef` (the feature, the message's code and field, whether it is a cause action, the action's index, and the document revision it was computed at), never the edits themselves. `apply_fix { doc, fix }` fails with `stale_fix` when the document's revision differs from `fix.revision`, or the feature no longer has that message or action; otherwise it builds the edits from the current records and applies them as one batch (section 3.3) labeled with the action's label: outside a transaction one undo entry, inside one appended to it, as `apply_edits`.

| Fix | Label | Edits built at apply time |
| --- | --- | --- |
| `SetField` (own field) | `Use {value}`; `Replace {expression} with {value}` when the field holds an expression other than a plain number; `Turn on {label}` or `Turn off {label}` for a toggle | one `update_feature` of the current record with that field replaced |
| `SetField` (another feature) | `Set {label} of '{feature}' to {value}` | one `update_feature` |
| `UseReferences`, one reference | `Use {description}`; for a sketch plane `Attach to {description}` | one `update_feature` replacing the entry at the pointer |
| `UseReferences` from a widening | `Use only {description}` | the same |
| `KeepMatch` | `Keep this match`; from a widening `Keep all {n}` | re-anchoring, below |
| `RemoveEntry` | `Remove from {label}` | one `update_feature` without the entry |
| `UseRegions` | `Use {region description}` | one `update_feature` |
| `KeepRegions` | `Keep these regions` | re-anchoring, below |
| `UseBody` | `Use body '{body}'` | one `update_feature` |
| `NewBody` | `Put it in a new body` | `add_body`, then `update_feature` with that body and Operation New body |
| `Attach` | `Attach '{sketch}' to {description}` | one `update_feature` of the sketch's plane, with the fallback in body coordinates |
| `TurnDrawingBack` | `Turn the drawing back` | `update_feature` of the sketch (mirrored entities, new fallback), then the re-anchored regions of the features that use it |
| `ConnectEnds` | `Connect the ends ({gap} apart)` | `update_feature` of the sketch with one coincident constraint added |
| `DeleteConstraint` | `Delete {constraint title}` | `update_feature` of the sketch |
| `MakeReference` | `Make {constraint title} a reference` | `update_feature` of the sketch |
| `RenameKey` | `Rename {from} to {to}` | one `update_feature` |
| `Unsuppress`, `Suppress` | `Unsuppress '{feature}'`, `Suppress '{feature}'` | one `update_feature` |
| `Move` | `Move '{feature}' below '{other}'` or `above '{other}'` | one `move_feature` to the position relative to `other` at apply time |
| `Pick` | `Pick {noun} again` | host action `pick` |
| `Show` | `Show '{feature}'` | host action `show` |

Cause actions use the same projection: reverts are `update_feature`, `update_parameter` or `move_feature` edits built from the current record and the clean snapshot; `Undo` is the host action `undo`; Keep the sketch where it was is one `update_feature` of the sketch plane.

**Re-anchoring.** `KeepMatch` stores `Reference::from_element` of each element the reference resolves to now (one entry, or one per element for Keep all). `KeepRegions` stores the current regions and, in the same edit, rewrites every later reference whose resolution `via` lists an alias that a succession of the old region reference produced, to `Reference::from_element` of the element it resolves to now, so keeping a profile never breaks what followed it. Both are one undo entry.
````

**E5. New section 3.23, ghosts:**

````markdown
### 3.23 Ghosts

When a reference of feature `F` goes from resolved, in the previous recompute of the document, to lost, the engine resolves it in `F`'s `bodies_before` of the previous report and asks the kernel for that element's display geometry only (kernel.md `element_display`: an edge's polyline, a face's triangles, a vertex's position) at the document's display tolerance. The ghost is kept, keyed by feature and pointer, while the loss lasts, and sent as the repair's `ghost`. A region repair's ghost is the stored outline's entities as the previous `sketch_solved` of that sketch had them, in sketch coordinates. A reference lost since the document was opened has no ghost; hosts draw a marker at `centroid`. Headless sessions capture and send no ghosts.
````

**E6. New section 3.24, uses:**

````markdown
### 3.24 Uses

`list_uses { doc }` returns what every feature uses, from the document alone (no recompute): one `Use { user, field, feature, entity }` for every feature link of `user` (`entity` absent) and for every generator of every stored reference of `user`, of its query and of its merged names (`ElementName::generators`), where `entity` is the sketch entity the generator names in the sketch that `feature` reads (a lateral face's line, a hole's point). Sorted by the user's timeline position, then field, then feature, then entity. The UI derives from it the timeline drop warning, "Used by" and the used marks of sketch entities (ui.md sections 3.9 and 3.16.10).
````

**E7. New section 3.25, document units:** `set_document_units { doc, units, keep_sizes }` changes the document units as one batch labeled `Change units to {units}`; with `keep_sizes`, every expression field and parameter whose expression is a number without a unit gets the old unit appended (`20` becomes `20 mm`) in the same batch, so no size changes. The model provides the edit set (`Document::units_edits(units, keep_sizes, inspector)`, model.md change). `DocumentInfo` (`document_changed`) gains `bare_numbers`: how many expressions are numbers without a unit, so hosts can offer Keep sizes with its count.

**E8. Section 4:** `internal_error` reads `Partsmith hit a bug while running {command}. Your document is unchanged. Save a copy, and report this with the details.` with the panic text in `ErrorInfo.detail`; `invalid_params` and `parse_error` keep the parser text in `detail` only; new `stale_fix`: `This fix no longer applies because the document changed after it was offered. Look at the problem again.`

**E9. Section 5, budgets: add rows:** `apply_fix` (the same budget as `apply_edits`, 100 ms desktop, 250 ms web); `list_uses` of the reference part (5 ms, 15 ms); ghost capture of one lost edge on the reference part (5 ms, 15 ms); `apply_edits` of a parameter change that loses 5 references on the reference part (100 ms, 250 ms, diagnosis and causes included).

**E10. Section 6: add tests.** The corpus twins of section 3.10 item 2 (cause text, cause actions, and applying the first revert restores the clean result); `apply_fix` of each `Fix` kind on a small document, with its undo label; `stale_fix` after an intervening edit and after the message went away; a `Move` fix applied after another feature was inserted above lands below the named neighbor; the actions of a diagnostic are equal in a fresh session that opens the same text (no session data in `actions`); attribution per message key (a warning code absent from the last clean build gets a cause, a message that existed before keeps the cause of its own clean snapshot); Keep the sketch where it was (R26); ghosts captured once at the resolved-to-lost transition and kept, none after reopening; `list_uses` of BLOCK with a fillet and a face sketch; `set_document_units` with and without `keep_sizes`; `pick_reference` returns anchors that `list_elements` leaves out; `end_sketch_drag` writes `fallback_frame: body`.

### 5.5 protocol.md changes (protocol schema change: human review and ADR 0024)

All additions are optional fields, new types or new commands, so `PROTOCOL_VERSION` stays 1 (section 2.9), but every one changes `docs/format/protocol.schema.json` and needs review at the next gate.

**PR1. `Diagnostic` (section 2.4): add fields and types.**

````markdown
```rust
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub field: Option<String>,
    pub suggestion: Option<String>,
    /// One-click fixes computed from the document alone, in the order to offer them (engine.md section 3.22). Omitted when empty.
    pub actions: Vec<FixAction>,
    /// The edit that started the problem, with its own actions (engine.md section 3.21). Session data.
    pub cause: Option<Cause>,
    /// Technical text for reports, never shown as the message.
    pub detail: Option<String>,
    /// The first failing feature of an `upstream_failed` chain, or the sketch a `no_effect` depends on.
    pub root: Option<Id>,
}

/// The edit that started a problem.
pub struct Cause {
    /// Plain sentence, for example "This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm."
    pub text: String,
    /// The feature the edit changed, when it changed one (deleted features included).
    pub feature: Option<Id>,
    /// Actions that depend on the session: reverts, Undo, Keep the sketch where it was, Show.
    pub actions: Vec<FixAction>,
    /// For a sketch edit in a display session: the changed entity before and after.
    pub ghost: Option<Ghost>,
}

/// A fix a host offers as one button.
pub struct FixAction {
    /// Button label starting with a verb, for example "Set Count of 'Linear pattern 1' back to 3".
    pub label: String,
    /// What the button does.
    pub run: FixKind,
}

#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FixKind {
    /// Run `apply_fix` with this reference.
    Apply { fix: FixRef },
    /// Edit the feature with the field at `path` active, so the next pick replaces the reference.
    Pick { feature: Id, path: String },
    /// Select and frame the feature, and highlight one of its sketch entities when `entity` is set.
    Show { feature: Id, entity: Option<Id> },
    /// Undo the newest undo entry.
    Undo,
}

/// Names one fix of one message at one revision; the engine builds the edits when it is applied.
pub struct FixRef {
    pub feature: Id,
    pub code: String,
    pub field: Option<String>,
    /// True for an action of the cause line.
    pub cause: bool,
    pub index: u32,
    pub revision: u64,
}
```

```json
{"severity": "error", "code": "references_not_found", "message": "The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet.", "field": "/edges/0", "actions": [{"label": "Use the top edge of the cut made by 'Extrude 1'", "run": {"kind": "apply", "fix": {"feature": "f-fil1", "code": "references_not_found", "field": "/edges/0", "cause": false, "index": 0, "revision": 7}}}, {"label": "Pick an edge again", "run": {"kind": "pick", "feature": "f-fil1", "path": "/edges/0"}}], "cause": {"text": "This started after you added 'Extrude 1' before this fillet.", "feature": "f-ext2", "actions": [{"label": "Suppress 'Extrude 1'", "run": {"kind": "apply", "fix": {"feature": "f-fil1", "code": "references_not_found", "field": "/edges/0", "cause": true, "index": 0, "revision": 7}}}, {"label": "Undo", "run": {"kind": "undo"}}]}, "detail": "edges[0] q=edge(f-box1/front|f-box1/top) resolution=missing lost.unmet=true"}
```
````

Feature messages may now have severity `info` (notices); a feature's status is `ok` when it has only info messages.

**PR2. `Repair` (section 2.7): add fields.**

````markdown
```rust
Element {
    path: String, body: Id, reference: Reference, resolution: ResolutionStatus, centroid: Option<Vec3>,
    /// The lost element in words, for example "the front top edge of 'Box 1'".
    description: String,
    /// Possible replacements in candidate order; each is also an action of the message.
    candidates: Vec<RepairCandidate>,
    /// Where the element was in its last build, for a dashed outline in the viewport (engine.md section 3.23).
    ghost: Option<Ghost>,
},
Region {
    path: String, sketch: Id, region: RegionRef, resolution: ResolutionStatus,
    /// The lost region in words, for example "the region inside the rectangle from (0, 0) to (80, 50)".
    description: String,
    /// Sets of regions that could be meant; each is also an action of the message.
    candidates: Vec<RegionCandidate>,
    /// The stored outline as it was, in sketch coordinates, for the sketch overlay.
    ghost: Option<Ghost>,
},

pub struct RepairCandidate {
    /// The reference the "Use" action stores.
    pub reference: Reference,
    /// The candidate in words.
    pub description: String,
    /// What it keeps and how far it is: "on the top of 'Box 1', 5 mm from where it was".
    pub relation: String,
    /// True when it keeps a face the lost element touched.
    pub meaning: bool,
    /// Distance in mm from where the lost element was.
    pub distance: f64,
    /// Fingerprint score in [0, 1], for reports; hosts never show it.
    pub score: f64,
    /// The candidate in the body's current model-layer mesh, for highlighting (absent in headless sessions).
    pub element: Option<MeshElement>,
}

pub struct RegionCandidate {
    pub regions: Vec<RegionRef>,
    pub description: String,
}

pub struct Ghost {
    /// `f32` positions: 2 points per segment for lines, 3 per triangle for faces, 1 point for a vertex, in mm.
    pub positions: BufferIndex,
    /// Lines, triangles or point.
    pub kind: GhostKind,
    /// World coordinates, or the sketch's own coordinates for a region or sketch entity ghost.
    pub frame: GhostFrame,
}
```
````

**PR3. Shared types.** `ElementDescriptor` gains `fallback: Option<Placement>` (planar faces, body coordinates) and `axis_direction: Option<Vec3>` (body coordinates); its `reference` is the short form in `list_elements` and the full form in `pick_reference` (engine.md E2). `ErrorInfo` gains `detail: Option<String>` and `actions: Vec<FixAction>` (for example "Finish and save" for `transaction_open`). `DocumentInfo` gains `bare_numbers: u32`.

**PR4. Commands and errors.** `apply_fix { doc, fix: FixRef }` returns `{ revision }`; `list_uses { doc }` returns `{ uses: Vec<Use> }` with `Use { user: Id, field: String, feature: Id, entity: Option<Id> }`; `set_document_units { doc, units, keep_sizes: bool }` returns `{ revision, kept: u32 }`. New error code `stale_fix` (engine.md E8).

**PR5. Shared schemas.** `Reference` gains `nb` and `merged` (naming.md N2), `RegionRef` gains `outline` (sketch.md S1), the face sketch plane gains `fallback_frame` and edge and face axes gain `direction`; all are also file-format version 2 changes (`docs/format/document.schema.json`). If decision D3 takes the alternative, `Placement` gains `mirrored`.

**PR6. Tests:** a `Diagnostic` with every new field round-trips and serializes keys in declaration order; without them the JSON is exactly as before; a `FixAction` of each `FixKind` round-trips; `apply_fix`, `list_uses` and `set_document_units` requests and results round-trip; a `recomputed` event with element and region ghosts passes the frame checks of section 3.4; `stale_fix` is in the `ErrorCode` set; the `Repair` round-trip test 68 gains the new fields.

## 6. Repair experience

### 6.1 What the user sees

When an edit breaks a reference:

1. **A notification for every new loss, one per root cause.** After any edit, and once when a sketch session ends, each new `reference_missing`, `reference_ambiguous`, `reference_guessed`, `reference_widened`, `region_missing`, `region_ambiguous`, `region_resized`, `plane_fallback` or `sketch_turned_over` (one whose feature, code and field were not there before) raises a notification, also when the feature only has a warning: a fillet that silently loses one of its edges is the FreeCAD failure. "'Fillet 1' lost the top edge of 'Extrude 1' along a line of 'Sketch 1'." with the cause line under it ("This started after you deleted the line from (80, 40) to (70, 50) in 'Sketch 1'.") and the actions [Show what 'Fillet 1' lost], the cause line's first revert when there is one, and [Undo]. Several features broken by one change give one notification naming the root ("'Datum plane 1' lost its face, so 2 features after it failed too."). A notification never offers a candidate: a replacement cannot be judged without seeing it.
2. **The Problems panel groups by root cause.** `upstream_failed` rows, and `no_effect` rows whose root is a sketch that lost its face, nest under the feature they name, so one cause is one problem, and their action is "Show '{root}'". Info notices are listed under a collapsed "Notes" heading, never as problems.
3. **Repair mode** opens from "Show what 'Fillet 1' lost", the Problems row, or the feature: the inspector shows the message, its actions, then the cause line and its actions; the viewport shows the lost element's ghost as a dashed outline in the error color where it was in its last build, labeled "{description} (missing)", or a ring at the stored centroid when there is no ghost; each candidate gets a numbered marker on the model, and the inspector lists them in the same order: "1. The top edge of the cut made by 'Extrude 1'" over "on the top of 'Box 1', 5 mm from where it was". Hovering a candidate highlights it; clicking it applies the replacement (one undo entry). Any other pick on the model replaces the reference too, because the repair field is active. For a list field, "Remove from Edges" is offered as well. For a lost profile, the sketch overlay shows the old outline's ghost and highlights the candidate regions.
4. **Notices** ("now follows the line that replaced it") show a small info mark on the timeline item and the "Keep this match" action, which stores the current match so the note goes away. Nothing is rewritten until the user clicks.

### 6.2 Candidates

Candidates come from the resolver (section 3.2, N11 step 5): those that keep a face the lost element touched first (such as the edges along the cut that removed it), then the others, each group by distance from where the element was, at most five. Each is described by what it is, what it keeps and how far it is ("on the top of 'Box 1', 5 mm from where it was"), and listed in that order, so the words explain the order. The fingerprint score is never shown: it ranks nothing the user sees, and a 100% lookalike is exactly what made FreeCAD's breakage silent (R21b's cove scores 1.0). Candidates are never applied automatically.

### 6.3 ui.md changes

**U1. Move the repair tools into M1.** Delete from section 1.2 the bullet "Repair tools beyond the Problems list, the Re-pick action and the repair marker of 3.7 and 4.2 (ghosted shape of the lost element, candidate lists, one-click re-pick on the canvas)." Add to section 1.1:

````markdown
- Repair mode for lost references and regions (3.7): the lost element's ghost drawn where it was in its last build, numbered candidates described by meaning and distance, one-click use of a candidate, re-pick on the canvas, Remove from list, Keep this match for followed references, a notification for every new loss (3.22), and the cause line of every message with its actions (4.1). This is the M1 answer to "references that survive edits": when one cannot, the user is told at once, sees what was lost and why, and fixes it in one click.
````

**U2. Section 4.1, message rules: replace the third bullet with:** "At most two sentences, then the message's action buttons, then the cause line ('This started after you ...', in `textMuted`) with its own action buttons. Action labels start with a verb and are the protocol's `FixAction` labels. A 'Copy details' action copies the code, the message, the cause and `detail` for reports." Replace the fourth bullet's "the core already follows these rules" with "the core follows docs/guidelines/errors.md".

**U3. Section 4.2: replace the "Feature warnings and errors at rest" row's fix column with:** "Every `FixAction` of the message, then of its cause, in order: `apply` runs `apply_fix` with its `FixRef` (a `stale_fix` error shows the core message as a notification with 'Show problems'); `pick` runs `editFeature` with that field active (the former 'Re-pick {field label}'); `show` selects and frames the feature and highlights `entity` with the cause's ghost; `undo` runs undo. A message with repairs also offers 'Show what {label} lost', which opens repair mode (3.7)." In the "Feature status during a command" row, delete the UI's own "Reverse" offer for `no_effect` and `through_all_misses`: the core offers "Turn on Reverse" only when reversing reaches the body (features.md section 3.15); "Reverse and use {text}" for a negative value stays, because it is exact.

**U4. Section 4.2 and 3.22, notifications on losses: add a row** "A feature gains a new lost, guessed or widened reference, a lost or ambiguous or resized region, a sketch face fallback or a turned-over sketch face (codes `reference_missing`, `reference_ambiguous`, `reference_guessed`, `reference_widened`, `region_missing`, `region_ambiguous`, `region_resized`, `plane_fallback`, `sketch_turned_over`) after a user change, warning or error" with "One warning notification per root (`Notification.features` naming the features): '{label} lost {repair description}.' for one feature, '{root label} lost {description}, so {n} features after it failed too.' for a chain, the cause line under the message, and the actions 'Show what {label} lost' (opens repair mode and frames the ghost), the cause's first revert action when it has one, and 'Undo'; never a candidate. A loss is new when its feature, code and field were not in the status before the change. During a sketch session, sketch edits raise none; when the session ends, the new losses since it started raise this notification once. Info notices never notify." The existing not-error-to-error row keeps its text and is merged with this one when both apply to one change.

**U5. Section 3.7: replace the repair marker paragraph with repair mode:**

````markdown
**Repair mode.** While a feature whose status has `repairs` is being edited, or its Problems row is focused, for each element repair: its `ghost` is drawn on the overlay as a dashed `errorMark` outline (line segments for an edge, the triangles' outline for a face, a ring for a vertex; `data-testid="repair-ghost"`, `stroke-dasharray="4 3"`), or the 16 px ring at `centroid` when there is no ghost, with the title "{description} (missing)"; each candidate gets a numbered marker (`data-testid="repair-candidate"`) at its element's centroid, and the inspector banner lists the candidates as buttons "{n}. {description}" with `relation` on a second line in `textMuted`, in the order received. No percentage or score is shown. Hovering a button or a marker highlights the candidate's `element`; clicking runs its action. For a region repair in sketch mode, the ghost is drawn in the sketch overlay and each candidate set is highlighted with its number. The repair field is the active selection field, so any pick replaces the reference. Esc leaves repair mode with the command.
````

**U6. Problems panel (3.9):** "Rows are grouped by `root`: a feature whose messages have a `root` is listed under the root's row, indented, with the action 'Show '{root}''; the panel counts one problem per root. Info notices are not problems: they are listed under a collapsed 'Notes' heading at the end."

**U7. Section 3.9, timeline drop warning:** "While a feature is dragged in the timeline, the insertion bar turns `warningMark` when the drop would put the feature above a feature whose elements or output it uses, or below a feature that uses its elements or output (from `list_uses`, engine.md section 3.24), with the hint '{label} uses {what} made by '{other}'. Drop below '{other}' instead?' Dropping anyway is allowed and undoable; there is no confirmation."

**U8. Section 3.9, used by:** "The timeline item's menu shows 'Used by {labels}' (from `list_uses`, at most three labels and '{n} more') next to Suppress and Delete. After Suppress or Delete, the notification of U4 names what broke, with 'Undo'."

**U9. Section 3.16.10, used marks in sketches:** "In sketch mode, entities that `list_uses` names for later features show a small 'used' mark (`data-testid="entity-used"`, tooltip 'Used by {labels}'). Deleting one shows the sketch hint '{label} uses the edge made from this line. Draw its replacement in the same place and {label} will follow it.'"

**U10. Units, Keep sizes:** "The document units control offers, when `bare_numbers` is above 0, the option 'Keep sizes: add {old unit} to the {n} numbers without a unit', on by default; choosing a unit runs `set_document_units` with `keep_sizes` set from the option. One undo entry."

**U11. Section 3.7, limits under fields:** "While a fillet, chamfer or shell command's preview status has a `fillet_too_large`, `chamfer_too_large` or `shell_too_thick` message with a limit, the field shows 'Largest that works: {limit}' under it in `warningText` and its border turns `warningMark`; the message's actions stay in the banner."

**U12. Section 2.10, open outlines:** "Extrude's and Revolve's disabled reason, when sketches exist but none has a closed region, reads ''{sketch}' has no closed shape: {n} ends are not connected.' (the sketch with the fewest open ends, then the latest) with a 'Show '{sketch}'' action that enters the sketch and frames the open ends. In sketch mode, the outline findings carry the core's actions, such as 'Connect the ends ({gap} apart)'."

**U13. Section 4.3, cause lines:** "A diagnostic's `cause` renders under the message and its actions, in `textMuted`, followed by the cause's own actions; `show` with an `entity` highlights the entity and draws the cause's ghost (old geometry dashed, new geometry solid)."

**U14. Section 6: add tests** U-127 to U-143:

- U-127 repair mode renders one ghost and N candidate markers from a fake status; buttons read "{n}. {description}" with the relation line and no percentage.
- U-128 clicking a candidate calls `apply_fix` with its `FixRef`; a `stale_fix` reply shows the stale notification.
- U-129 a status with `root` nests under the root; notes are not counted.
- U-130 Copy details copies code, message, cause and detail.
- U-131 a cause line renders under the message's actions, with its own actions after it.
- U-132 a new `reference_missing` warning after an edit raises one notification naming the feature, the element and the cause, with "Show what 'Fillet 1' lost", the cause's revert and Undo, and no candidate.
- U-133 a loss that existed before the edit raises nothing; new losses during a sketch session raise one notification when it ends.
- U-134 `reference_guessed`, `reference_widened`, `region_resized`, `plane_fallback` and `sketch_turned_over` notify; `reference_followed` and `region_split` never do.
- U-135 dragging 'Fillet 1' above 'Extrude 2' with fake `list_uses` data shows the warning bar and the hint.
- U-136 the timeline item menu shows "Used by 'Fillet 1', 'Datum plane 1'".
- U-137 sketch entities used later show the used mark; deleting one shows the hint.
- U-138 the units control offers Keep sizes with the count and sends `keep_sizes`.
- U-139 the limit shows under the field from a preview status with a limit.
- U-140 Extrude's disabled reason names the sketch and its open ends, with Show.
- U-141 a `no_effect` without a "Turn on Reverse" action shows no Reverse button.
- U-142 attaching a sketch to a face stores the descriptor's `fallback` with `fallback_frame: body`.
- U-143 picking an edge or a round face for an axis stores the descriptor's `axis_direction` as `direction`.

And E-109 to E-126 with the real core (ui.md had already used E-105 to E-108, so the four tests first numbered so are E-123 to E-126, ui.md open issue 22): E-123 R16 raises one notification naming 'Fillet 1' and the lost edge, repair mode shows two candidates, and applying the first gives status ok; E-124 R41 notifies with "Set Count of 'Linear pattern 1' back to 3", and applying it restores the fillet; E-125 R28 "Move 'Fillet 1' below 'Extrude 2'" restores the document byte for byte; E-126 R06a shows a note and Keep this match removes it; E-109 R03 deleting the line in a sketch session raises one notification at the end of the session naming 'Fillet 1' and the cause line, and Show draws the deleted line's ghost; E-110 to E-121 the newcomer error corpus NC01 to NC12 (section 7.6); E-122 a fix offered before an unrelated edit reports that it no longer applies.

## 7. Newcomer errors

### 7.1 The five errors newcomers hit most

Reviewing the inventory against what a first-week user does gives five errors that matter more than their count: each is common, each blocks the next step, and each is explained badly today.

| Error | Says what happened today | Says why today | Says what to do today | After this design |
| --- | --- | --- | --- | --- |
| 1. Open outline, so Extrude has nothing to use | yes | partly (entity ids) | "Back to sketch" only | The disabled reason names the sketch and the number of open ends with Show; findings name entities by kind and coordinates; "Connect the ends (0.3 mm apart)" closes a small gap in one click (NC01) |
| 2. Sketch on the wrong plane: a cut that misses, a join buried inside the body, or a join that floats | vague, or nothing | no | wrong ("turn on Reverse" when it misses too) or none | `no_effect` says where the tool is and by how much; Turn on Reverse only when it works; "Attach 'Sketch 2' to the top of 'Extrude 1'"; a floating join warns that the body now has 2 pieces (NC02 to NC04) |
| 3. Fillet or chamfer too large | yes | no (which edge or face limits it) | "Try 2.3 mm" | The limiting face is named with its measured width; a limit too small to be useful is not suggested; "Remove that edge from Edges" when the other edges work (NC05) |
| 4. Constraint conflict, or a dimension value the sketch cannot take | yes | no (ids, and nothing when a value is changed) | none when a value is changed | The conflicting constraints are named by title and value with Delete and Make a reference actions; a value the sketch cannot take names the change and offers Undo (NC07, NC08) |
| 5. Shell too thick for the wall the print needs | yes | no | only "make the wall thinner" | The rounded faces smaller than the wall are named with the feature that made them; "Set Radius of 'Fillet 1' to 3 mm" and "Move 'Shell 1' above 'Fillet 1'" come before the thinner wall (NC06) |

### 7.2 features.md sections 4.1 and 4.2, rewritten

These tables replace sections 4.1 and 4.2 in full (F9). `{this}` is "this" followed by the feature's manifest label in lowercase (`this fillet`); `{Element}` is the capitalized element description of section 3.12 followed by ` selected for {label}`; `{reason}` is recompute's diagnosis (recompute.md section 3.10); `{entity}` is a sketch entity by kind and coordinates (section 7.4). Fixes are listed in the order offered; "host" fixes are added by hosts, not by `fixes()`. Every rendered row passes the message lint of the guidelines.

**4.1 `FeatureError`**

| Code | Variant | Message | Fixes |
| --- | --- | --- | --- |
| `unknown_type` | `UnknownType { type_name }` | `This document has a feature of type '{type_name}', which this version of Partsmith does not have. Update Partsmith, or delete the feature.` | |
| `invalid_params` | `InvalidParams { field: Option<FieldRef>, detail }` | with field: `{label} of {this} cannot be read, so it was not built. Set {label} again.` without: `The settings of {this} cannot be read, so it was not built. Edit it to set them again, or delete it.` (`detail` holds the parser text) | `Pick` of the field when it is a reference |
| `empty_selection` | `EmptySelection { field, noun }` | `Select at least one {noun} for {label}.` | `Pick` |
| `target_required` | `TargetRequired { field }` | `Operation New body puts the copies into another body, so {label} needs an empty body. Pick one, or set Operation to Join.` | `SetField` Operation Join, `Pick` |
| `target_not_allowed` | `TargetNotAllowed { field }` | `{label} is used only when Operation is New body. Clear {label}, or set Operation to New body.` | `SetField` Operation New body, `SetField` clearing {label} |
| `same_body` | `SameBody { field, body }` | `Body '{body}' cannot be used twice here. Pick a different body for {label}.` | `Pick` |
| `expression` | `Expression { field, expr, error }` | `{label}: {error}` (the evaluator's message; a misspelled name offers the near match, NC10) | the evaluator's fix |
| `out_of_range` | `OutOfRange { field, kind, value, bound, bound_name }` | the bound table below | `SetField` with the bound for `AtLeast` and `AtMost` |
| `not_whole_number` | `NotWholeNumber { field, value }` | `{label} must be a whole number, but it is {value}.` | `SetField` `{floor}`, `SetField` `{ceil}` |
| `undeclared_dependency` | `UndeclaredDependency { what }` | `Partsmith hit a bug in {this}, so it was not built. Suppress it to keep working, and report it with the details.` (`what` in `detail`) | `Suppress`, host Copy details |
| `unknown_body` | `UnknownBody { field, body }` | `{label} names a body that is not in this document. Pick a body for {label}.` | `Pick` |
| `unknown_feature` | `UnknownFeature { field, feature }` | `{label} names a feature that is not in this document. Pick another one for {label}.` | `Pick` |
| `wrong_feature_type` | `WrongFeatureType { field, feature, expected }` | `'{feature}' is not a {expected}. Pick a {expected} for {label}.` | `Pick` |
| `forward_reference` | `ForwardReference { field, feature }` | `{label} uses '{feature}', which comes after {this}. Move {this} below '{feature}', or pick something earlier.` | `Move` below '{feature}', `Pick` |
| `upstream_failed` | `UpstreamFailed { feature }`, `root` from recompute | root is the feature: `'{feature}' has an error, so {this} cannot use it. Fix '{feature}' first.` otherwise: `'{feature}' cannot be built because '{root}' has an error, so {this} cannot use it either. Fix '{root}' first.` | `Show` root |
| `upstream_suppressed` | `UpstreamSuppressed { feature }` | `'{feature}' is suppressed, so {this} cannot use it. Unsuppress '{feature}', or pick something else.` | `Unsuppress`, `Pick` |
| `asset_missing` | `AssetMissing { field, asset }` | `The STEP file of {this} is not in this document's assets folder. Copy the assets folder that came with the document next to it, or import the file again.` (`asset` in `detail`) | host Import again |
| `body_not_empty` | `BodyNotEmpty { body }` | `Body '{body}' already has a solid here, so Operation New body cannot start it. Set Operation to Join, or put {this} in a new body.` | `SetField` Operation Join, `NewBody` |
| `body_empty` | `BodyEmpty { body, use_new }`, `because` from recompute | failed: `Body '{body}' is empty here because '{feature}', which makes it, failed. Fix '{feature}' first.` suppressed: `Body '{body}' is empty here because '{feature}', which makes it, is suppressed.` comes later: `Body '{body}' is empty here because '{feature}', which makes it, now comes after {this}.` used as tool: `Body '{body}' is empty here because '{feature}' uses it as a tool, which empties it.` none, use_new: `Body '{body}' has no solid yet at this point, so there is nothing to add to. Set Operation to New body to start it here.` none, otherwise: `Body '{body}' has no solid at this point, so {this} has nothing to work on. Pick a body that has a solid.` | failed: `Show`; suppressed: `Unsuppress`; comes later: `Move` below it; used as tool: `UseBody` of the tool's target; use_new: `SetField` Operation New body |
| `tool_empty` | `ToolEmpty { body }`, `because` from recompute | `Body '{body}' is empty here, so it cannot be used as a tool.` followed by the `because` clause of `body_empty` when known | as `body_empty`, then `Pick` |
| `empty_result` | `EmptyResult { body, op }` | cut: `This cut removes all of body '{body}'. Make it smaller, or change its position or direction.` intersect: `The shapes do not overlap body '{body}', so the intersection is empty. Move them so they overlap it.` | |
| `references_not_found` | `ReferencesNotFound { field, kind, losses }` | one loss: `{Element} {reason}.` several: `None of the {count} {kind}s selected for {label} are left: {first element} {first reason}, and {count - 1} more.` | per loss: `UseReferences` for each candidate, `Pick`; reason fixes |
| `reference_not_found` | `ReferenceNotFound { field, loss }` | `{Element} {reason}.` ambiguous: `{Element} now matches {n} {kind}s{after}. Pick the one you mean.` (`{after}` is ` after '{feature}' changed` when the diagnosis names it) | `UseReferences` for each candidate, `Pick`; reason fixes |
| `profile_not_found` | `ProfileNotFound { field, sketch, losses }` | one region: `The region of '{sketch}' selected for {label} is gone because {region reason}.` several: `None of the {n} regions of '{sketch}' selected for {label} are left, because {region reason, plural}.` with `{region reason}` one of `part of its outline was deleted`, `its outline was drawn again`, `its outline is no longer closed` (plural: `parts of their outline were deleted`, `their outline was drawn again`, `their outline is no longer closed`); the cause line names the deleted curve with its coordinates | `Show` the sketch, `Pick` |
| `not_planar` | `NotPlanar { field, element }` | `{Element} is not flat. Pick a flat face, a datum plane or an origin plane.` | `Pick` |
| `not_coplanar` | `NotCoplanar { field, faces }` | `The face selected for {label} now matches {n} faces that do not lie in one plane. Pick the one you mean.` | `UseReferences` for each face, `Pick` |
| `not_straight` | `NotStraight { field, element }` | `{Element} is not straight. Pick a straight edge, a sketch line or an origin axis.` | `Pick` |
| `not_collinear` | `NotCollinear { field, edges }` | `The edge selected for {label} now matches {n} edges that do not lie on one line. Pick the one you mean.` | `UseReferences` for each edge, `Pick` |
| `no_axis` | `NoAxis { field, element }` | `{Element} has no axis. Pick a round face, a straight edge, a sketch line or an origin axis.` | `Pick` |
| `unknown_entity` | `UnknownEntity { field, sketch, entity, noun }` | `'{sketch}' no longer has the {noun} picked for {label}. Pick a {noun} of '{sketch}' again.` (`entity` in `detail`) | `Pick` |
| `not_a_line` | `NotALine { field, sketch, entity }` | `{Entity} in '{sketch}', picked for {label}, is not a line with length. Pick a line.` | `Pick` |
| `sketch_invalid` | `SketchInvalid { error }` | the sketch crate's message (section 7.4) | the sketch crate's fixes |
| `sketch_conflict` | `SketchConflict { constraints }` | the conflict message (section 7.4) | `DeleteConstraint`, `MakeReference` per constraint |
| `sketch_not_solved` | `SketchNotSolved` | the not-converged message (section 7.4) | host Undo |
| `kernel` | `Kernel { error }` | `Partsmith's geometry kernel could not build {this}. {hint}` (kernel.md section 4.3; the kernel's text in `detail`) | `Suppress`, host Copy details |
| `fillet_too_large`, `chamfer_too_large`, `shell_too_thick` | with `max` and `limit` | section 7.3 | section 7.3 |
| `shell_removes_all` | `ShellRemovesAll { body, count }` | `Shell would remove all {count} faces of body '{body}', which leaves nothing to hollow. Remove at most {count - 1} faces, or leave Faces to remove empty for a closed hollow.` | `Pick` |
| `profile_crosses_axis` | `ProfileCrossesAxis` | `The profile crosses the revolve axis. Move the profile to one side of the axis, or pick another axis.` | `Pick` of the axis |
| `axis_not_in_plane` | `AxisNotInPlane` | `The revolve axis does not lie in the sketch plane. Pick a line in the sketch, or an axis that lies in the sketch plane.` | `Pick` of the axis |
| `through_all_misses` | `ThroughAllMisses { body, reverse_hits }` | `Through all points away from body '{body}', so there is nothing to go through.` and, without a working Reverse, ` Point it at the body, or use a distance.` | `SetField` Reverse on (when `reverse_hits`) |
| `through_all_needs_body` | `ThroughAllNeedsBody { body }` | `Through all goes through an existing solid, and body '{body}' has none here. Use a distance instead.` | |
| `both_sides_zero` | `BothSidesZero` | `Both sides of the extrusion are 0. Give at least one side a distance.` | |
| `cone_radii_zero` | `ConeRadiiZero` | `Cone bottom and top radius cannot both be 0. Give one of them a size.` | |
| `plane_misses_body` | `PlaneMissesBody { body }` | `The split plane does not pass through body '{body}'. Move the plane so it cuts through the body.` | `Pick` of the plane |
| `too_many_instances` | `TooManyInstances { count, limit }` | `This pattern would make {count} copies, more than the limit of {limit}. Lower Count.` | |
| `canceled` | `Canceled` | `Recompute was canceled before {this} finished.` | |
| `internal` | `Internal { message }` | `Partsmith hit a bug while building {this}. Suppress it or undo the last change to keep working, and report it with the details.` (`message` in `detail`) | `Suppress`, host Copy details |
| `name_too_deep` | `NameTooDeep { length }` | `{this} builds on a chain of rounded or beveled edges too long for Partsmith to name. Combine some of the fillets or chamfers into one.` (capitalized; `length` in `detail`) | |

`OutOfRange` keeps its bound table, with a strict bound written with enough digits that value and bound differ (`Distance must be greater than 0 mm, but it is 0.0000001 mm.`) and the fixes of section 4.4.

**4.2 `FeatureWarning`**

| Code | Severity | Variant | Message | Fixes |
| --- | --- | --- | --- | --- |
| `reference_missing` | warning | `ReferenceLost` (not ambiguous) | `{Element} {reason}, so it was skipped.` | `UseReferences` per candidate, reason fixes, `RemoveEntry`, `Pick` |
| `reference_ambiguous` | warning | `ReferenceLost` (ambiguous) | `{Element} now matches {n} {kind}s{after}, so it was skipped. Pick the one you mean.` | `UseReferences` per candidate, `RemoveEntry`, `Pick` |
| `reference_guessed` | warning | `ReferenceGuessed` | named piece kept: `{Element} was taken by its position among the parts of {face}, but {alternative} now fits its shape better. Check which one you mean.` named piece gone: `{Element} no longer exists as named, so the {kind} that fits its shape best, {used}, is used. Check it, then keep it or pick another.` | `KeepMatch`, `UseReferences` per alternative, `Pick` |
| `reference_followed` | info | `ReferenceFollowed` | replaced: `{Element} now follows {new}, because its {line or curve} was replaced in '{by}'.` extended: `{Element} now also covers {new}, because its line continues in '{by}'.` merged: `{Element} merged with neighboring parts when '{by}' joined them, so all of it is used.` continued: `{Element} was removed by '{by}', which left its rim in the same place, so the rim is used.` related: `{Element} changed when '{by}' changed, so {new} is used.` | `KeepMatch`, `Pick` |
| `reference_widened` | warning | `ReferenceWidened` | `{Element} now covers {n} {kind}s, because the {line or curve} it was made from was replaced by {n} {lines, or curves when not all are lines} in '{by}'. Keep all of them, or use only one.` | `KeepMatch` (Keep all {n}), `UseReferences` (Use only the first), `Pick` |
| `region_split` | info | `RegionSplit` | `The region of '{sketch}' selected for {label} is now split in {count} parts by new lines, and all of them are used. Remove a part from the profile to leave it out.` | `KeepRegions`, `Pick` |
| `region_resized` | warning | `RegionResized` | `The region of '{sketch}' selected for {label} is now {count} parts that together cover {area} instead of {stored area}. Check the profile, then keep it or pick it again.` | `KeepRegions`, `Pick` |
| `region_remapped` | warning | `RegionRemapped` | `The outline of the region of '{sketch}' selected for {label} was drawn again, so the region with the same size and place is used. Check it, then keep it or pick it again.` | `KeepRegions`, `Pick` |
| `region_ambiguous` | warning | `RegionAmbiguous` | `The region of '{sketch}' selected for {label} now matches {n} separate areas, so it was skipped. Pick the one you mean.` | `UseRegions` per candidate set, `Pick` |
| `region_missing` | warning | `RegionMissing` | `The region of '{sketch}' selected for {label} is gone because {region reason}, so it was skipped.` | `Show` the sketch, `Pick` |
| `plane_fallback` | warning | `PlaneFallback` | features/sketch.md section 3.5 (section 3.5.1 above) | `Attach` per candidate, reason fixes, `Pick` |
| `sketch_turned_over` | warning | `SketchTurnedOver` | `The face this sketch is on, {element}, now faces the other way, so the drawing is mirrored on it.` | `TurnDrawingBack`, `Pick` |
| `no_effect` | warning | `NoEffect` | section 7.3 | section 7.3 |
| `separate_solid` | warning | `SeparateSolid { body, pieces }` | `{this} does not touch body '{body}', so the body now has {pieces} separate pieces.` (capitalized) | `NewBody`, `Attach` when found |
| `hole_misses_body` | warning | `HoleMissesBody` | `The hole at ({u}, {v}) in '{sketch}' does not touch body '{body}'.` and, without a working Reverse, ` Move the point onto the body.` | `SetField` Reverse on (when `reverse_hits`) |
| `point_missing` | warning | `PointMissing` | `{this} no longer makes the hole at a point that was deleted from '{sketch}', so it was skipped.` (capitalized) | `RemoveEntry` (Remove from Points), `Pick` |
| `closed_void` | warning | `ClosedVoid { body }` | `This closes a hollow inside body '{body}', which will print as a sealed cavity. Check the order of the features if that is not intended.` | |
| `ignored` | warning | `Ignored { field, reason }` | `SmoothEdge`: `One of the edges selected for {label} is smooth (its faces meet without a corner), so it was skipped.` `TipAngleThroughAll`: `{label} is ignored because the hole goes through all.` | `RemoveEntry` for a smooth edge |
| `unknown_params` | warning | `UnknownParams { keys, suggestions }` | one key with one near match (edit distance at most 2): `'{key}' is not a {Type} setting, so it is ignored. Did you mean {label}?` otherwise: `{this} has settings Partsmith does not know ({keys}). They are kept in the file and ignored.` (capitalized) | `RenameKey` for each near match |
| `redundant_constraints` | warning | `RedundantConstraints { issues }` | the redundant messages of section 7.4, joined with a space | `DeleteConstraint`, `MakeReference` per constraint |
| `import_warning` | warning | `ImportWarning { message }` | `{message}`, the kernel's warning text for skipped or repaired STEP content (kernel.md section 3.8.1) | |

`repair()` returns `Element` for `ReferenceLost`, `ReferenceGuessed`, `ReferenceWidened`, `PlaneFallback` and `ReferenceNotFound` (one per loss for `ReferencesNotFound`), and `Region` for `RegionRemapped`, `RegionResized`, `RegionAmbiguous`, `RegionMissing` and `ProfileNotFound`, with the JSON Pointer of the list entry (`/edges/1`, `/profile/regions/0`).

### 7.3 Causes and fixes computed by the core

**features.md new section 3.15, "Why nothing changed":**

````markdown
### 3.15 Why nothing changed

`apply_op` reports why a join, cut or intersect changed nothing (`NoEffect.how`), from the kernel bounding boxes of the tool and the body (kernel.md `bounding_box`) and the volumes it already computes. Let `d` be the feature's direction when it has one (an extrude's direction after `reverse`), and `[t0, t1]`, `[b0, b1]` the extents of the tool and the body along `d`:

| `how` | When | Message |
| --- | --- | --- |
| `Inside` | a join whose volume did not change: the tool lies inside the body | `This {type} adds nothing to body '{body}': it lies entirely inside the body.` |
| `Away { gap }` | a cut or intersect whose volume did not change, with `t0 >= b1` along `d` and the boxes overlapping across `d` | `This cut does not touch body '{body}': it starts {gap} from the body and points away from it.` |
| `Short { gap }` | the same with `t1 <= b0` | `This cut does not reach body '{body}': it ends {gap} short of it.` |
| `Beside` | any other cut whose volume did not change, or a feature without a direction | `This cut does not touch body '{body}': it lies beside it.` |
| `Covers` | an intersect whose volume did not change: the shape covers the whole body | `This intersection keeps all of body '{body}': its shape covers the whole body.` |

"Across `d`" uses the two world axes perpendicular to `d` when `d` is within 1e-9 of a world axis; for any other `d` the case is `Beside`, so no distance is claimed that was not measured. When the feature's profile sketch reports `PlaneFallback` in the same run, recompute replaces the message with `This {type} adds nothing to body '{body}' because '{sketch}' lost the face it was on. Fix '{sketch}' first.` (or `changes nothing in`) and sets `root` (recompute.md section 3.10).

Fixes, each only when it holds:
- **Turn on Reverse** (`reverse_hits`): the feature builds the reversed tool and checks with the kernel that it overlaps the body (one `common` on the warning path, cached with the result).
- **Attach** (joins, `Inside` and `SeparateSolid`): the *attach face* is the planar face of the body whose outward normal equals `d` within 1e-9 rad and that lies farthest along `d`, when exactly one face is there; the fix attaches the feature's profile sketch to it ("Attach 'Sketch 2' to the top of 'Extrude 1'"), with the sketch's current placement projected onto the face as the fallback in body coordinates.
- **Put it in a new body** (`SeparateSolid`).

`SeparateSolid` (section 3.5) uses the same attach rule.
````

**features.md section 4.3, the "too large" search: replace with:**

````markdown
### 4.3 The "too large" search

When a fillet, chamfer or shell kernel operation fails (any `KernelError` except `InvalidInput` and `Internal`), the feature searches for the largest working value before reporting `FilletTooLarge`, `ChamferTooLarge` or `ShellTooThick` with the first failure as `cause`:

- Bisection on (0, requested): `lo = 0`, `hi = requested`; for at most 10 attempts, `mid = (lo + hi) / 2`; if the operation succeeds at `mid`, `lo = mid` and `max = mid`, else `hi = mid`. There is no time limit, because a limit would make the result depend on the machine. For a two-distance chamfer both distances are scaled by the same factor and the larger scaled distance is reported.
- `max` is the largest attempted value that succeeded. A `max` below 10 percent of the requested value or below 0.05 mm counts as none: such a limit is not a useful fix and usually means the failure is not about size.
- **Limit** (fillet and chamfer): on the failure path only, the kernel measures each face next to a selected edge across that edge (kernel.md `width_across`: the largest distance from the edge's curve to a point of the face). The narrowest such face is the `limit` when its width is below the requested value: it is named with the edge. When `limit` is set and more than one edge is selected, one more attempt without that edge at the requested value decides whether `RemoveEntry` of that edge is offered.
- **Limit** (shell): the round faces of the body (cylinder, cone, sphere or torus fingerprints) whose radius is smaller than the thickness, from their fingerprints, grouped by the feature that made them (from their names). Only facts are stated: the faces and their radius, not that they caused the failure.
- The attempt sequence is fixed, so the error is deterministic and cached like any result.

| Case | Message | Fixes |
| --- | --- | --- |
| fillet, `max` and `limit` | `Fillet radius {radius} is too large for the selected edges: the largest that works is about {max}. {Edge} lies next to {face}, which is only {width} across.` | `SetField` {max}, `RemoveEntry` of that edge when verified |
| fillet, `max`, no `limit` | `Fillet radius {radius} is too large for the selected edges: the largest that works is about {max}.` | `SetField` {max} |
| fillet, no `max`, `limit` | `Fillet radius {radius} does not work on the selected edges, even when smaller. {Edge} lies next to {face}, which is only {width} across.` | `RemoveEntry` when verified, `Suppress` |
| fillet, neither | `Fillet radius {radius} does not work on the selected edges, even when smaller, and Partsmith could not tell why.` | `Suppress`, host Copy details |
| chamfer | the same four rows with `Chamfer distance {distance}` | the same |
| shell, `max`, round faces | `Shell thickness {thickness} is too thick for body '{body}': the largest that works is about {max}. Its rounded faces made by '{feature}' have radius {r}, smaller than the wall.` | `SetField` of the radius of '{feature}' to 1.5 times the thickness, rounded up to 2 significant digits, when '{feature}' is a fillet whose radius is a plain value ("Set Radius of 'Fillet 1' to 3 mm"); `Move` '{shell}' above '{feature}' when it is a fillet or chamfer of the same body before the shell; `SetField` {max} |
| shell, `max`, no round faces | `Shell thickness {thickness} is too thick for body '{body}': the largest that works is about {max}.` | `SetField` {max} |
| shell, no `max` | `Shell thickness {thickness} does not work on body '{body}', even when thinner.` followed by the round-face sentence, or ` Partsmith could not tell why.` | the radius and move fixes, `Suppress` |

`{Edge}` and `{face}` are element descriptions (section 3.12); `{width}` is formatted with `format_value`. In a document whose units are not mm, an expression that is a number without a unit adds the sentence `'{expr}' has no unit, so it means {expr} {unit} in this document.` and the fix `SetField` with `{expr} mm` before the other fixes (corpus R48).
````

**Open outlines (sketch.md section 3.13 and 4.3).** `build_profile` findings gain the fix `ConnectEnds` for two open ends of different entities within 2 mm of each other (the closest pair first), labeled with the gap ("Connect the ends (0.3 mm apart)"). Extrude's disabled reason is the UI's (U12).

### 7.4 sketch.md section 4, rewritten

Messages name entities and constraints the way the user sees them, never by id; ids go to the detail. Numbers are rendered in the caller's units (`render(&self, units: Units)`, with partsmith-model's quantity display), so the sketch crate and features read the same. Placeholders:

- `{entity}`: `the line from ({u1}, {v1}) to ({u2}, {v2})`, `the circle at ({u}, {v}) with radius {r}`, `the arc from ({u1}, {v1}) to ({u2}, {v2}) around ({u}, {v})`, `the point at ({u}, {v})`, from the solved (or, before solving, the stored) geometry; `the origin`, `the x axis`, `the y axis` for reserved references.
- `{title}` of a constraint: a dimension is its type label and value (`Distance 40 mm`, `Angle 90 deg`, `Radius 5 mm`, `Horizontal distance 10 mm`); a constraint on one entity is `{Type} on {entity}` (`Horizontal on the line from (0, 0) to (80, 0)`); on two, `{Type} between {entity} and {entity}`; a dimension on an entity adds ` on {entity}` when two titles in one message would otherwise read the same.

**4.1 `SketchError`** (rows not listed keep their meaning with these subjects):

| Variant | Message |
| --- | --- |
| `InvalidPointRef { text }` | `"{text}" is not a point of this sketch. Pick a point, or a line's start or end, or a circle's center.` |
| `DuplicateId`, `ReservedId` | `This sketch has two items with the same id, so it cannot be read. Delete and redraw the later one, or report it if you did not edit the file by hand.` (ids in detail) |
| `InvalidPlacement { reason }` | `The position of this sketch cannot be used: {reason}. Attach the sketch to a plane or face again.` (reasons unchanged) |
| `NonFiniteGeometry { entity }` | `{Entity} has a coordinate, radius or angle that is not a number. Delete it and draw it again.` |
| `NegativeRadius { entity, radius }` | `{Entity} has a negative radius ({radius}). Use a radius of 0 or more.` |
| `UnknownEntity { constraint, entity }` | `{Title} refers to geometry that is not in this sketch. Delete the constraint.` |
| `ReservedMisuse { constraint, reference }` | `{Title} cannot use {the origin, the x axis, the y axis}: the origin is a single point, and the axes are endless lines. Pick a point or line of the sketch instead.` |
| `InvalidPointRole { constraint, point, kind }` | `{Title} uses a point that {entity} does not have: a line has a start and an end, a circle a center, an arc a start, an end and a center.` |
| `WrongEntityKind { constraint, entity, found, expected }` | `{Title} cannot use {entity}, which is {article found}. It needs {expected}.` |
| `SameEntity`, `SamePoint`, `PointOnOwnEntity` | `{Title} uses {entity or point} twice. Pick two different ones.` and `{Title} uses a point of {entity} itself. Pick a point of another entity.` |
| `MissingValue`, `NonFiniteValue` | `{Type} has no value it can use. Check its expression.` |
| `ValueNotPositive { value, points }` | `{Type} {value} must be greater than 0.` and, for two points, ` To make two points meet, use Coincident instead.` |
| `ValueNegative { value }` | `Distance {value} from a point to a line must be 0 or more.` |
| `UnknownDragTarget`, `NonFiniteCursor` | `The dragged geometry is no longer in this sketch. Drag it again.` and `The pointer position cannot be used. Move the pointer over the sketch and drag again.` |
| `InvalidRegionKey`, `InvalidFingerprint` | `A region stored in this document cannot be read. Pick the profile again.` (text in detail) |

**4.2 `SolveIssue`**

| Issue | Message | Fixes |
| --- | --- | --- |
| `Conflicting` with two or more constraints | `In '{sketch}', {title 1} cannot hold together with {title 2}` and, for more, `, {title 3}` ... `and {title n}`, ending with a period. | `DeleteConstraint` per constraint, newest first; `MakeReference` for each driving dimension |
| `Conflicting` with one constraint | `In '{sketch}', {title} cannot hold.` | `DeleteConstraint`, `MakeReference` for a dimension |
| `Redundant`, geometric | `In '{sketch}', {title} is not needed: the other constraints already hold it.` | `DeleteConstraint` |
| `Redundant`, dimension | `In '{sketch}', {title} is not needed: the other constraints already set it.` | `MakeReference`, `DeleteConstraint` |
| `NotConverged` | `'{sketch}' could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape.` | host Undo |

When a conflict or a failure follows a change of a dimension's value, the engine's cause line names it ("This started after you changed Distance of 'Sketch 1' from 50 mm to 80 mm.") with "Set Distance of 'Sketch 1' back to 50 mm". The dimension editor (ui.md section 3.16.7) shows the same conflict sentence for a value it cannot apply, from the solver probe's `conflicting`, with "Add as reference" and "Delete {title}".

**4.3 `ProfileFinding`**

| Finding | Message | Fixes |
| --- | --- | --- |
| `OpenEndpoint { entity, point }` | `The end of {entity} at ({u}, {v}) is not connected to other geometry.` | `ConnectEnds` with the nearest other open end within 2 mm |
| `Overlap { entities, start, end }` | `{Entity a} and {entity b} overlap between ({u1}, {v1}) and ({u2}, {v2}). Delete one of them, or move it so they meet only at their ends.` | |
| `SelfIntersection { entities, point }` | `{Entity a} and {entity b} cross at ({u}, {v}), which splits the outline into several regions.` | |
| `ZeroLength { entity }` | `The line at ({u}, {v}) has no length and is ignored for profiles. Delete it.` | |

Two open ends that a single `ConnectEnds` joins are reported as one finding.

### 7.5 kernel.md changes and the modeling limit

**K1. Section 3.6, measurements:** results of solid operations report `solids` and `shells` counts, so features compare inner shells (features.md section 3.5).

**K2. New query `Shape::width_across(face: u32, edge: u32) -> Result<f64, KernelError>`:** the largest distance from the edge's curve to a point of the face (`BRepExtrema_DistShapeShape` against the face's boundary vertices and a sampled interior), for the "too large" messages. Used on failure paths only; budget 5 ms per face on the acceptance shapes.

**K3. New query `Shape::element_display(kind, index, tolerance) -> Result<ElementDisplay, KernelError>`:** the display geometry of one element (an edge's polyline, a face's triangles, a vertex's position) at a tessellation tolerance, for ghosts (engine.md section 3.23). Budget 5 ms for one face of a 1,000-face body.

**K4. Section 4.3, user-facing messages: replace the operation table** with one hint per operation, used after features' `Partsmith's geometry kernel could not build {this}.`; each hint is true whenever the kernel fails in that operation, never blames the input when the input was checked before the kernel, and the kernel's own text goes to `detail`:

| Operation | Hint |
| --- | --- |
| primitives (`make_box`, `make_cylinder`, `make_sphere`, `make_cone`, `make_torus`) | `Its sizes passed every check, so this is likely a bug; report it with the details.` |
| `make_profile` | `The closed shape could not be turned into a face; report it with the details if the sketch shows no problem.` |
| `extrude`, `revolve` | `Report it with the details if the profile and the sizes look right.` |
| `fuse`, `cut`, `common` | `Bodies that touch only along an edge or at a point, or faces that almost coincide, often cause this. Move one of them slightly.` |
| `fillet`, `chamfer`, `shell` | not used: these run the "too large" search (features.md section 4.3) |
| `transform` (move, mirror, patterns) | `Report it with the details.` (the "Moving the body failed." text is gone, since mirror and patterns move nothing the user asked to move) |
| `split` | `Try moving the split plane slightly.` |
| `section`, `compound`, `measure`, `query`, `tessellate`, `write_step`, `read_step`, `write_brep`, `read_brep` | each says what could not be done in the user's terms (`The body could not be meshed for display or export.`), unchanged where the inventory found no fault |

`Internal` becomes `Partsmith's geometry kernel hit an unexpected problem.` with the exception and its message in `detail`.

**Modeling limit (features.md section 3.4):** every length a feature evaluates is at most `MODEL_LIMIT` = 1,000,000 mm in absolute value, and every count at most the feature's own limit; larger values give `OutOfRange { bound: AtMost(MODEL_LIMIT) }` with the fix "Use 1000000 mm", before the kernel runs. Tests: a box of length "1e200", an extrude of distance "1e200" and a fillet of radius "1e7" each give this error and never a kernel error.

### 7.6 Newcomer error corpus

Each scenario is a test in recompute (structural message and fixes), in the engine (cause line and actions, with `apply_edits` and `apply_fix`) and in Playwright (E-110 to E-121, the real core and UI). Messages are exact; "then" checks the result of the first fix. Setups use the corpus's BLOCK unless stated.

- **NC01 Open outline.** Setup: `f-sk1` on XY with lines (0,0)-(80,0), (80,0)-(80,50), (80,50)-(0,50), and (0,50)-(0,0.3). Expect: no region; the UI's Extrude disabled reason `'Sketch 1' has no closed shape: 2 ends are not connected.` with [Show 'Sketch 1']; one finding `The end of the line from (0, 50) to (0, 0.3) at (0, 0.3) is not connected to other geometry.` with [Connect the ends (0.3 mm apart)]. Then: one region, Extrude enabled.
- **NC02 Join buried in the body (sketch on the wrong plane).** BLOCK; `f-sk2` on XY with circle (40,25) r5; `f-ext2` distance 10, op join. Expect: `f-ext2` warning `no_effect`: `This extrude adds nothing to body 'Body 1': it lies entirely inside the body.` [Attach 'Sketch 2' to the top of 'Extrude 1']. Then: the boss stands at z 30..40, volume 120785.398163, status ok.
- **NC03 Cut that points away.** BLOCK; `f-dp1` datum XY offset 40; `f-sk2` on it with circle (40,25) r5; `f-ext2` distance 15, op cut. Expect: `This cut does not touch body 'Body 1': it starts 10 mm from the body and points away from it.` [Turn on Reverse]. Then: a pocket z 25..30, volume 119607.300918. With distance 5 instead, the same message without the Reverse fix (reversed it ends at z=35 and still misses) and the sentence does not change.
- **NC04 Floating join.** As NC03 with op join. Expect: `f-ext2` warning `separate_solid`: `This extrude does not touch body 'Body 1', so the body now has 2 separate pieces.` [Put it in a new body] [Attach 'Sketch 2' to the top of 'Extrude 1']. Then (Attach): one solid, boss at z 30..45.
- **NC05 Fillet too large.** Bodies `[b-1]`; `f-box1` box 40 x 20 x 10 (x -20..20, y -10..10, z 0..10); `f-box2` box 20 x 30 x 5 at position (10, 0, 7.6), op cut (a step 2.4 mm deep over x 0..20); `f-fil1` fillet radius 3 on the edge between `f-box1/top` and the step's riser at x=0. Expect: `Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm. The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across.` [Use 2.3 mm]. Then: ok.
- **NC06 Shell too thick for 2 mm walls.** Bodies `[b-1]`; `f-box1` box 40 x 40 x 20; `f-fil1` fillet radius 1 on the four vertical edges; `f-sh1` shell thickness 2 removing `f-box1/top`. Expect: `Shell thickness 2 mm is too thick for body 'Body 1': the largest that works is about 0.99 mm. Its rounded faces made by 'Fillet 1' have radius 1 mm, smaller than the wall.` [Set Radius of 'Fillet 1' to 3 mm] [Move 'Shell 1' above 'Fillet 1'] [Use 0.99 mm]. Then (the first fix): ok, wall 2 mm.
- **NC07 Constraint conflict.** `f-sk1` with the line (0,0)-(40,0), Distance 40 mm on it, then Distance 30 mm added on the same line. Expect: `f-sk1` error `sketch_conflict`: `In 'Sketch 1', Distance 30 mm cannot hold together with Distance 40 mm.` [Delete Distance 30 mm] [Make Distance 30 mm a reference] [Delete Distance 40 mm] [Make Distance 40 mm a reference]; 'Extrude 1' reports `upstream_failed` with root 'Sketch 1'. Then (the first fix): ok.
- **NC08 A dimension value the sketch cannot take.** `f-sk1` a triangle whose sides have Distance 30 mm, Distance 40 mm and Distance 50 mm; change the third to 80 mm. Expect: `f-sk1` error `sketch_not_solved`: `'Sketch 1' could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape.` with the cause line `This started after you changed Distance of 'Sketch 1' from 50 mm to 80 mm.` [Set Distance of 'Sketch 1' back to 50 mm] [Undo]. The dimension editor shows the same sentence when the value is typed and offers "Add as reference". Then (the revert): ok.
- **NC09 Units switch with bare numbers.** Bodies `[b-1]`; `f-box1` box size `["40", "20", "10"]`; `f-fil1` radius `"2"`; document units mm. Expect: `bare_numbers` is 4; the units control offers `Keep sizes: add mm to the 4 numbers without a unit`; `set_document_units(in, keep_sizes: true)` writes `"40 mm"`, `"20 mm"`, `"10 mm"`, `"2 mm"` and leaves the volume unchanged; without it, R48's message applies to a fillet that fails.
- **NC10 Misspelled parameter.** Parameter `height` = `30 mm`; `f-ext1` distance `hieght`. Expect: `expression` error `Distance: 'hieght' is not a parameter. Did you mean 'height'?` [Replace hieght with height] (expressions.md change: an unknown name with exactly one parameter within edit distance 2 gets this text and fix; otherwise the current `Unknown name 'hieght'. Check the spelling, or add a parameter named hieght.`). Then: ok.
- **NC11 A deleted sketch line.** BLOCK with `f-fil1` on the back top edge; delete `e-3` (the back line). Expect: `f-ext1` error `profile_not_found`: `The region of 'Sketch 1' selected for Profile is gone because part of its outline was deleted.` [Show 'Sketch 1'] [Pick a region again]; `f-fil1` error `body_empty`: `Body 'Body 1' is empty here because 'Extrude 1', which makes it, failed. Fix 'Extrude 1' first.` [Show 'Extrude 1']; one notification naming 'Extrude 1' with the cause line `This started after you deleted the line from (80, 50) to (0, 50) in 'Sketch 1'.` [Undo].
- **NC12 Hole beside the body.** BLOCK; `f-sk2` on the top with point (95, 25); `f-hole1` diameter 5 through all. Expect: `hole_misses_body`: `The hole at (95, 25) in 'Sketch 2' does not touch body 'Body 1'. Move the point onto the body.` and no Reverse action (reversed, it misses too).

## 8. Error prevention, ranked by value

Ranked by how many users the error hits times how much it costs them; the first four come straight from the corpus. Each names the spec text that carries it, so none is left without a home.

1. **Never apply a guess** (section 3, phase 1). Prevents the costliest error of all, the silent wrong result, in 34 of the 38 corpus cases at once, and the remaining 4 in phase 2. Spec: naming N11, features F6.
2. **Tell the user at once when something is lost** (ui U4). A warning that only shows as a timeline badge is the FreeCAD experience; every new loss raises one notification per root cause with the way back.
3. **Warn before a timeline drop that breaks references** (ui U7, engine E6). While dragging a feature in the timeline, the insertion bar turns `warningMark` when the drop would put a feature above an element it uses or below a feature that uses its elements, with the hint "'Fillet 1' uses an edge made by 'Extrude 2'. Drop below 'Extrude 2' instead?" Dropping anyway is allowed and undoable (no confirmation in the path). Prevents R24b, R28 and R55.
4. **Show what uses a feature before it is suppressed or deleted** (ui U8, engine E6). The timeline item's menu shows "Used by 'Fillet 1', 'Datum plane 1'" next to Suppress and Delete, and the notification after the action names what broke with "Undo". Prevents R23, R25, R54 and R56a, and makes the rest one click to undo.
5. **Tell the sketch editor which entities are in use** (ui U9, engine E6). Entities whose faces later references use show a small "used" mark; deleting one shows the hint "Fillet 1 uses the edge made from this line. Draw its replacement in the same place and Fillet 1 will follow it." (the succession rule). Prevents R03, R51 and most C2 cases.
6. **Keep sizes when switching units** (ui U10, engine E7). The units control offers "Keep sizes: add mm to the 14 numbers without a unit", on by default (one undo entry; `set_units(..., keep_sizes=True)` in Python). Prevents R48 and every bare-number surprise (NC09).
7. **Show the limit while typing** (ui U11). The fillet, chamfer and shell inspectors show the largest working value under the field once the preview has searched it. Turns an error into a hint.
8. **Check sizes before the kernel** (section 7.5). The modeling limit (1,000,000 mm), positive length bounds printed with enough digits, and the pre-check for a shell removing every face, so the kernel's fragments ("The box could not be built.") never appear for input that can be checked.
9. **Skip, do not fail, on a missing list entry** (per-feature 3.5.3). Holes skip a deleted point with a warning (R43b), as fillet already skips a lost edge.
10. **Close small gaps for the user** (section 7.3, ui U12). "Connect the ends (0.3 mm apart)" and an Extrude disabled reason that names the sketch and its open ends (NC01).
11. **Pickers that only accept valid picks.** Flat faces for planes, straight edges and round faces for axes, bodies with a solid for booleans, never the target body as a tool, no smooth edges for fillets and chamfers (with a hint why). Spec: ui.md section 3.6 (follow-up).
12. **Did-you-mean everywhere a name is typed.** Parameter names in expressions (NC10), settings keys in files (`unknown_params`), feature types, units, enum values, printer profiles, CLI body names: within edit distance 2, offer the fix in one click. Spec: section 7.2 and expressions.md; the rest are follow-ups in the model and CLI specs.
13. **Accept what is unambiguous.** A JSON number in an expression field, a region key out of order, a UTF-16 file, a rollback marker pointing at a deleted feature (open with the marker at the end and a warning). Spec: model and file-format follow-ups.
14. **Warn about results nobody wants** (features F8). Closed voids from joins, joins that leave a separate solid, cuts that leave nothing: a warning on the feature that made them, so later failures and print findings have a named cause.

## 9. Decisions and reviews needed

These change the naming algorithm, the file format, the protocol, the kernel facade or a product rule, so they need human review at the next gate and an ADR each (AGENTS.md, "Process"). The decisions are accepted with the recommended options and recorded in ADRs 0021 to 0026 (D6 is applied to the corpus documents), each waiting for human review at the next gate.

- **D1. Resolve by meaning (naming algorithm, architecture).** The resolver no longer substitutes by fingerprint; names without anchors are confirmed against their siblings and reported as guesses; references store anchors (`nb`, at most six, only what tells siblings apart) and merged names per face; aliases become sorted one-to-many unions; canonical names indexed per body state; piece and continuation and relative rules. architecture.md's "Topological naming" bullet ("then a fingerprint match above a confidence threshold") and technical-design.md section 5 items 4 and 5 change to "candidates ranked by fingerprint for the user to choose, and a repair mode with the lost element's ghost". [ADR 0021](../adr/0021-resolve-references-by-meaning.md).
- **D2. A region split by new geometry keeps covering the same connected area (product rule, R50).** The alternative is to follow only the outer part (a circle drawn inside becomes a hole); the corpus proposes keeping the area, with a notice when the area is unchanged and a warning when it is not, and `Ambiguous` when the matching regions are not connected through new geometry. Recorded in sketch.md section 3.14 with the region outline. [ADR 0022](../adr/0022-split-regions-keep-their-area.md).
- **D3. Frames on the model (naming motions, sketch placement rule).** Motions per face name; hints in body coordinates behind the explicit `fallback_frame` marker; picked axis directions; placement before removal for a face a later feature removed; and the rule for a face that turned over (R32b, mirror copies). Recommended: placements stay right-handed; the drawing lies mirrored on the turned-over face, the sketch warns `sketch_turned_over`, and "Turn the drawing back" mirrors the stored drawing in one undo entry. The alternative keeps the drawing in place with a mirrored placement (`Placement::mirrored`), which needs a handedness rule in sketch.md, features.md (`make_profile` input, extrude and revolve directions, hole drilling) and kernel.md, with tests for extrude, revolve and hole, because left-handed frames otherwise give inverted profiles and break "every solid has positive volume" (features.md section 3.10). [ADR 0023](../adr/0023-frames-live-on-the-model.md), with the recommended rule.
- **D4. Diagnostics with actions, causes, details, candidates and ghosts (protocol schema).** `FixAction` with `FixRef` and the `apply_fix` command, `Cause` with its own actions, `detail`, `root`, repair candidates with descriptions and relations (no scores shown), ghosts, `list_uses`, `set_document_units`, `stale_fix`, short references in element lists. Section 5.5. [ADR 0024](../adr/0024-diagnostics-with-actions-causes-and-ghosts.md).
- **D5. File format version 2.** `nb`, `merged`, `outline`, `fallback_frame` and `direction`, all optional, with a migration from version 1 that changes nothing; older builds then reject new files with `NewerFormatVersion`. Section 3.7, FF1. [ADR 0025](../adr/0025-file-format-version-2.md).
- **D6. Corpus and test amendments.** Section 3.10, including the amended expectations of R26 and R32b, the new scenario R66, and the newcomer error corpus as a design document. Applied in [the reference stability corpus](reference-stability-corpus.md) and [the newcomer error corpus](newcomer-error-corpus.md).
- **D7. Kernel facade additions** (kernel.md K1 to K4): solid and shell counts in measurements, `width_across`, `element_display`, and the rewritten user-facing hints. [ADR 0026](../adr/0026-kernel-facade-additions-for-errors.md).

No new dependency is needed; the dependency log (ADR 0020) is unchanged.

## 10. Implementation plan

This plan is the order of work for a later pipeline; it can be followed from this section and the spec text items it names (N, S, F, RC, FF, PY, CLI, E, PR, U and K items, sections 3.5, 7.2 to 7.6). Where merged spec text differs from sections 3 to 7 (each spec's open issues list the differences), the spec wins. Each step names the spec text, the crate, and the tests that prove it. A step is done when its tests pass, `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` are clean, the crate's `AGENTS.md` lists the new public interface, the message lint passes over every rendered template the step touches, and its budgets hold. Steps within a phase are ordered by dependency; steps in different crates can run in parallel once the spec text they implement is merged. A phase ships only when its exit criteria hold, and each phase keeps every earlier phase's tests green.

### 10.1 Step 0: decisions before anything else

1. ADRs 0021 to 0026 record D1 to D5 and D7 (section 9) and are logged in `docs/adr/README.md`; take them to human review at the next gate. D1, D4 and D5 gate all code; D3 gates phase 2 code; D7 gates phase 3 kernel code.
2. Get D6 approved: the corpus amendments of section 3.10 and the newcomer error corpus.

### 10.2 Step 1: spec changes, in this order

Each is one change to one spec (or a small group), merged before any code that implements it, so every later spec refers only to merged text.

1. **naming.md:** N1 to N16.
2. **sketch.md:** S1 to S4, and section 4 per section 7.4 (sketch.md's own message tests updated to the new texts).
3. **file-format.md:** FF1. **model.md:** `Document::units_edits(units, keep_sizes, inspector)` and `bare_numbers` (E7). **expressions.md:** the did-you-mean rule for unknown names (NC10).
4. **kernel.md:** K1 to K4, with acceptance tests (solid and shell counts of a join around a void, `width_across` on the NC05 step, `element_display` of a face and an edge, every hint text) and budget rows.
5. **features.md:** F1 to F14, new section 3.15, sections 4.1 to 4.3 per sections 7.2 and 7.3, the modeling limit of section 7.5; the per-feature specs of section 3.5 (sketch, chamfer, hole, extrude, revolve, fillet, shell, linear and circular pattern, mirror, datum plane, move body, split body) with their tests.
6. **recompute.md:** RC1 to RC6.
7. **python-api.md:** PY1 to PY4. **cli.md:** CLI1.
8. **protocol.md:** PR1 to PR6. **engine.md:** E1 to E10.
9. **ui.md:** U1 to U14.
10. **Design and architecture text:** architecture.md (the "Topological naming" bullet per D1; recompute's diagnosis reads names; the protocol additions), technical-design.md section 5 (items 4 and 5, and "Sketch-on-face also records a fallback placement" reworded to the placement-before-removal rule), reference-stability-corpus.md (D6 amendments and R66), and the new `docs/design/newcomer-error-corpus.md` from section 7.6.

**Status: step 1 is written.** Every spec change of this step is merged, after two reviews of the spec text, and the code steps below implement the specs, not sections 3 to 7. Where each lives:

1. naming.md: sections 2.4 to 2.8 (references with anchors and merged names, aliases and successions, motions, `ElementSet` and `NameSet`, resolutions), 3.1, 3.3, 3.5.1, 3.6 (with `new_motion`), 3.9 to 3.11, 5 and 6; [ADR 0021](../adr/0021-resolve-references-by-meaning.md).
2. sketch.md: sections 2.2 (`HintFrame`, `fallback_frame`), 2.7 (`RegionRef::outline`, `RegionMatch`, successions, `RegionLoss`, `Connection`), 3.13.8, 3.14, 3.15, 4 (messages by title and coordinates, fixes) and 6.12; [ADR 0022](../adr/0022-split-regions-keep-their-area.md).
3. file-format.md sections 3.9 and 3.10 and their tests ([ADR 0025](../adr/0025-file-format-version-2.md)); model.md sections 2.7 (`ExprLink::unit`) and 3.12 (`units_edits`, `bare_numbers`); expressions.md section 3.18 (the near match and its fix).
4. kernel.md: K1 to K4 in sections 2.2, 2.13, 3.6, 3.7 and 4.3, with tests 117 to 126 and budget rows ([ADR 0026](../adr/0026-kernel-facade-additions-for-errors.md)).
5. features.md: sections 2.4 to 2.8, 3.4 to 3.15, 4.1 to 4.4 and 6.3 to 6.5; the per-feature specs features/sketch.md, chamfer.md, hole.md, extrude.md, revolve.md, fillet.md, shell.md, linear_pattern.md, circular_pattern.md, mirror.md, datum_plane.md, move_body.md and split_body.md, each with its tests ([ADR 0023](../adr/0023-frames-live-on-the-model.md) for the frames).
6. recompute.md: sections 1, 2 (`Message`), 3.3 to 3.6, 3.10 (the loss diagnosis), 5, and tests 41 to 73.
7. python-api.md: sections 2.4, 2.8, 3.6, 3.8 to 3.10, 4 and 5; cli.md: sections 2.2, 3.1 and 3.3 and tests 55 to 62.
8. protocol.md: sections 2.3 to 2.9 and tests 68 and 72 to 83; engine.md: sections 3.4, 3.6, 3.9, 3.19 and 3.21 to 3.25, 4 and 5, and tests 115 to 152 ([ADR 0024](../adr/0024-diagnostics-with-actions-causes-and-ghosts.md)); the planned schema check-ins of `protocol.schema.json` and `ptsm.schema.json` are logged in [ADR 0016](../adr/0016-generated-schema-check-ins.md).
9. ui.md: sections 1.1, 2.10, 3.7 (repair mode, limits under fields), 3.9 (Problems grouped by root, the drop warning, Used by), 3.16 (used marks, re-attaching), 3.22 (notifications for losses), 4.1 to 4.3 and 5, with U-127 to U-149, E-109 to E-126 and P-9.
10. architecture.md (the "Topological naming" bullet, the loss diagnosis, the protocol additions and the `partsmith-recompute -> partsmith-naming` edge), technical-design.md section 5 and its risk table, [the reference stability corpus](reference-stability-corpus.md) (amendments 1 to 8 and R66) and [the newcomer error corpus](newcomer-error-corpus.md); [the error guidelines](../guidelines/errors.md) quote the texts the specs render.

### 10.3 Phase 1: resolve by meaning and explain every loss

1. **partsmith-naming.**
   - Code: `Reference` with `Anchors`, `Merged`, `MergedName` and `short`; `Aliases` with sorted target lists; `Motion` and the map's motions (build step 8, `moved`, `with_instance` with a transform, `motion`), built now and read from phase 2; `ElementSet` with the canonical-name index; `ElementName::generators`; the new `Resolution`, `Candidate`, `Lost`, `MatchMethod::Guess`, `Follow`, `via` and `alternatives`; `NamingMap::build` steps 2, 6 and 8 (N7); the resolver of N11; `Reference::from_element` of N14.
   - Tests: naming tests 60 to 90 changed per N16; new tests 91 to 118; property tests for order independence of resolving (118) and of building (115); benches for the rows of N15 and the existing resolve row.
2. **partsmith-sketch.**
   - Code: `RegionRef.outline`, `RegionLoop` with canonical rotation, `Region::reference()` writing the outline, `match_region(regions, entities, reference)` with connected components, successions and `RegionLoss` (S1 to S3).
   - Tests: the S4 list; `RegionRef` round trips with and without `outline`; existing section 6.12 tests updated.
   - Also: the mirrored drawing of sketch.md section 3.17 (`SketchParams::mirrored`, `mirrored_point`, `mirrored_region`), which phase 2's `TurnDrawingBack` (features/sketch.md test 24, engine.md test 135) needs; tests 248 to 250, 252 and 253 in `tests/mirrored.rs`. Test 251 (NC08 `NotConverged`) belongs to phase 3 step 3.
3. **partsmith-model.**
   - Code: `FORMAT_VERSION` 2 and the identity migration from 1 (FF1).
   - Tests: file-format tests 23, 39 and E1 to E3 at version 2; a version 1 document saves differing only in its `format_version` line; a version 2 document with every new field round-trips byte for byte.
4. **partsmith-features.**
   - Code: `BodyState::element_set` and `measurements().elements`; `BuildInputs::history` and `body_history`; `FeatureOutput::absent` (extrude `Through`, revolve `FullTurn`); `Loss`, `LossCandidate` (built with `from_element`), `Fix`, `Place`, `fixes`, `severity`, `detail`; `resolve` and `resolve_all` per F6, with the follow classification from the kernel geometry of the resolved elements; `profile` per F5; extrude and revolve succession aliases (N9); element descriptions (F7); the rows of section 7.2 for references, regions, `upstream_failed`, `body_empty`, `tool_empty`, `internal` and `name_too_deep`; `AxisRef.direction` and `fallback_frame` parsed and kept, not yet honored (nothing writes them before phase 2); Python codegen of `anchors`, `merged`, `outline`, `direction` and `fallback_frame` (F12); `VERSION` bumped (F14).
   - Tests: build context tests for `resolve_all` (each follow kind gives its notice or warning; a guess; a loss whose candidates equal `from_element` of the candidates; a fully lost list keeps every loss); one profile test per F5 row; document tests R06a, R07a, R49 and R66 for extrude and revolve; description tests for every rule of F7, including the clause added when two elements read the same; the rendering test of every corpus message from the F7 rules; the message lint over every rendered template; codegen tests for each new keyword.
5. **partsmith-recompute.**
   - Code: writer chains and `BuildInputs::history` (RC1); status (RC2); `Message` fields and `Severity::Info` (RC3); the diagnosis with memoization, reasons, `root`, `because` and the reason's fixes (RC4).
   - Tests: the RC6 list; the RC5 budget rows; the reference stability corpus with the setups re-picked per section 3.10 item 1 and R66 added, still behind the `stability-study` feature.
6. **partsmith-protocol.**
   - Code: PR1 (`actions`, `detail`, `root`; `cause` declared, filled in phase 4), PR2 (description, candidates with relation; `ghost` declared, filled in phase 4), PR3 (short and full references; `fallback` and `axis_direction` declared, filled in phase 2; `ErrorInfo.detail` and `actions`), PR4 (`apply_fix`, `stale_fix`), PR5.
   - Tests: the PR6 list for these types.
7. **partsmith-engine.**
   - Code: E1, E2 (short and full references), E4 (`apply_fix`, labels, re-anchoring for `KeepMatch` and `KeepRegions`), E8; then regenerate the schemas with `cargo run -p partsmith-cli -- schema --write`.
   - Tests: from E10, `apply_fix` of each `Fix` kind with its undo label, `stale_fix`, a `Move` fix after an insertion above, diagnostic actions equal in a fresh session, `pick_reference` anchors that `list_elements` leaves out, and `KeepRegions` after R06a leaving every later feature's status and geometry unchanged and removing the notice.
8. **partsmith-cli, partsmith-py and `python/`.**
   - Code: CLI1; PY1 to PY3 (full references from selections, the index warning, exception attributes).
   - Tests: `rebuild` text and `--json` on the R16 and R03 documents; `validate` exits 1 on a lost reference; the Python round-trip test with every new field; the index warning and its absence when iterating; the exception attributes.
9. **partsmith-wasm:** rebuild; the protocol passes through `ps_call` unchanged; the wasm smoke tests.
10. **ui.**
    - Code: regenerate the protocol types (`pnpm -C ui generate:protocol`); U1 to U6: actions through `apply_fix`, notifications for new losses, repair mode with the centroid ring (ghosts come in phase 4), Problems grouped by root with Notes, Copy details; remove the UI's own Reverse offer.
    - Tests: U-127 to U-130, U-132 to U-134, U-141; E-123, E-125 and E-126, and E-124 and E-109 for their message and actions (their cause lines are checked in phase 4).
11. **examples.** Add `examples/references/line-redrawn` (R06a), `examples/references/slot-inserted` (R17) and `examples/references/notch-replaces-line` (R66), each with its Python twin and expected results; re-bless the examples whose expected messages changed, with the sign-off of technical design section 16.

Exit: corpus 112 of 123 with the amended setups; silent wrong results 4 (R22 a and b, R38, R32a rotated); built correctly before, fails after: 0 over the at-risk list of section 3.9; every budget, old and new, of naming, sketch, recompute and engine; `pnpm -C ui lint && pnpm -C ui typecheck && pnpm -C ui test` and `pnpm -C ui e2e` green.

### 10.4 Phase 2: frames on the model

1. **partsmith-features.**
   - Code: F4 (coplanar and collinear checks with `NotCoplanar` and `NotCollinear`, the face placement rule with the hint frame and the motion, `turned_over`, `face_placement_before_removal`, the axis rule with `direction` and the motion); features/sketch.md per section 3.5.1 (`fallback_frame` honored, the three placement steps, `PlaneFallback { cause, placed }`, `SketchTurnedOver`, `TurnDrawingBack`) under the D3 decision; the chamfer side rule (3.5.2); copy transforms in pattern and mirror naming (3.5.6); `moved` in move body (3.5.8).
   - Tests: features/sketch.md tests (a) to (g) of section 3.5.1; chamfer tests 15 and 16 with both id orders; the pattern and mirror copy-face tests; the axis tests of section 3.5.7, including the 180 degree Move body with and without a stored `direction`.
2. **partsmith-recompute.** The corpus tests R22, R32a (both), R32b (per D3) and R33 pass; the FF1 test that a version 1 document with a Move body before a face sketch rebuilds identically passes.
3. **partsmith-engine and partsmith-protocol.**
   - Code: descriptor `fallback` in body coordinates and `axis_direction` (E2, PR3); `end_sketch_drag` writes `fallback_frame: body`; `apply_fix` of `Attach` and `TurnDrawingBack` with region re-anchoring.
   - Tests: a descriptor's fallback equals the inverse motion applied to its placement; a drag commit writes the marker; Turn the drawing back on R32b leaves no warning and the hole at (20, 10).
4. **python.** `Sketch(face, ...)` writes `fallback_frame="body"` with a body-coordinate fallback; a selection used as an axis writes `direction`. Tests: both, and the round trip.
5. **ui.** Attaching a sketch to a face stores the descriptor's fallback with `fallback_frame: body`; picking an edge or face for an axis stores `direction`. Tests: U-142 and U-143.
6. **examples.** `examples/references/moved-face-sketch` (R32a), `examples/references/sketch-after-cut` (R33) and `examples/linear_pattern/leaning-edge` (R38), with Python twins.

Exit: corpus 119; silent wrong results 0; built correctly before, fails after: 0; the version 1 rebuild test; budgets.

### 10.5 Phase 3: newcomer errors and feature rules

1. **partsmith-occt-sys.** K1 (solid and shell counts), K2 (`width_across`), K3 (`element_display`, used in phase 4), K4 (hint texts). Tests and budget rows of step 1.4.
2. **partsmith-model and the expression evaluator.** `units_edits`, `bare_numbers`, and the did-you-mean of NC10. Tests: model tests for both, the expressions test for one and for several near matches.
3. **partsmith-sketch.** Section 4 per section 7.4 with `render(units)`, and the `ConnectEnds` fix on open-end findings. Tests: every rewritten row, one conflict with three constraints, a gap of 0.3 mm (fix) and of 3 mm (no fix).
4. **partsmith-features.**
   - Code: every remaining row of section 7.2; section 3.15 (`NoEffect.how`, verified Reverse, the attach rule); `SeparateSolid`; `ClosedVoid` by inner shells (F8); hole points skipped and holes described by coordinates (3.5.3); the "too large" search with `limit`, the 10 percent and 0.05 mm floor and the verified Remove fix (section 7.3); shell round faces and their fixes; `ShellRemovesAll`; the modeling limit; `unknown_params` with `RenameKey`; the bare-number sentence (R48).
   - Tests: body semantics tests 5 to 7 changed; hole tests for `point_missing` and the new texts of tests 7 and 32; NC02 to NC06 and NC12 as document tests; the modeling limit tests; the lint over every row; the corpus tests R43b, R48 and R55.
5. **partsmith-engine and partsmith-protocol.** E7 (`set_document_units`, `bare_numbers`), `ErrorInfo.actions`. Tests: E10's units tests; NC09 at engine level.
6. **python.** PY4. Test: NC09 in Python.
7. **ui.** U10 to U12. Tests: U-138 to U-140; E-110 to E-121 for the newcomer corpus (the cause lines of NC08 and NC11 are checked in phase 4).
8. **examples.** `examples/hole/deleted-point` (R43b) with its Python twin.

Exit: corpus 122; the newcomer corpus passes in recompute and engine tests and in Playwright for everything but cause lines; the lint passes over every template of features, sketch, kernel and recompute; budgets, including K2 and K3.

### 10.6 Phase 4: causes, ghosts and prevention

1. **partsmith-engine.** E3 (baselines per feature and message key, attribution, cause actions: reverts, Undo, Keep the sketch where it was, Show with the entity ghost), E5 (ghosts captured at the resolved-to-lost transition with `element_display`), E6 (`list_uses`), E9 budgets. Tests: the remaining E10 list, including the corpus twins of section 3.10 item 2 and R26's Keep the sketch where it was.
2. **partsmith-protocol.** `Cause` and `Ghost` filled, `list_uses`. Tests: the remaining PR6 list.
3. **ui.** U7 to U9 and U13. Tests: U-131, U-135 to U-137; E-124 and E-109 complete with their cause lines; E-122; the cause lines of NC08 and NC11. Also the drop check's performance test P-9 (ui.md 6.7): the `part-300.ptsm` fixture of ui.md 6.2 and the `ps:drop-check` mark of ui.md 2.18.
4. **partsmith-recompute.** Move the reference stability corpus out of the `stability-study` feature into the normal test suite, now that all 123 pass.

Exit: corpus 123 of 123 with the D6 amendments; every engine twin; every budget; the full UI and e2e suites.

### 10.7 Exit criteria at a glance

| Phase | Corpus passing | Silent wrong left | Built correctly before, fails after | Newcomer corpus | Other gates |
| --- | --- | --- | --- | --- | --- |
| 1 | 112 | 4 | 0 | | naming, sketch, recompute and engine budgets; schemas regenerated; examples re-blessed |
| 2 | 119 | 0 | 0 | | version 1 documents rebuild identically |
| 3 | 122 | 0 | 0 | 12 of 12 without cause lines | kernel budgets; lint over every template |
| 4 | 123 | 0 | 0 | 12 of 12 | engine twins; corpus in the normal suite |
