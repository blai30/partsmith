# Errors and messages

House rules for every message a user sees: feature errors, warnings and notices, command errors, file, export and import messages, print findings, CLI output, Python exceptions and UI notifications. They apply the "Errors that help" direction of [the guidelines](README.md). Where messages appear and which buttons they get is in [ui.md](../spec/ui.md) section 4; the mechanisms (loss reasons, causes, fixes, details, ghosts) are in [the reference stability and errors design](../design/reference-stability-and-errors.md), whose inventory of 260 messages and newcomer error corpus the examples below come from.

## Prevent first

The best message is the one nobody needs. Before writing a message, check whether the app can make the problem impossible or harmless:

- **Default to what works:** a starting value that builds, the operation the body's state allows, a picker that only accepts what the field can use.
- **Check before the kernel:** ranges, sizes, units and preconditions are checked by the feature, with the field and the limit, so the kernel's generic failures never reach users for input that could be checked.
- **Show it while it is being typed:** live previews, limits under the field, a warning on a timeline drop that would break a reference, what uses a feature before it is suppressed or deleted, which sketch lines later features use.
- **Skip instead of failing:** one missing entry of a list (an edge, a hole point) is a warning, and the rest still builds.
- **Accept what is unambiguous:** a number where an expression is expected, a key out of order, a near-miss name with exactly one close match is offered, not rejected.
- **Never guess silently:** when the app cannot tell what the user meant, it says so and asks with candidates. A wrong result with no message is the worst error there is.
- **Never write behind the user's back:** the app changes a document only through an edit the user made or an action the user clicked, so reverting an edit by hand gives back the same file.

## Structure

Every message answers three questions, in this order:

1. **What happened**, in one sentence, with the affected thing as the subject, named the way the user sees it: "The front top edge of 'Box 1' selected for Edges ...", "Body 'Body 1' is empty here ...", "Fillet radius 3 mm is too large for the selected edges."
2. **Why**, in the same or a second sentence: the cause in the model, naming the feature or value that did it. "... was cut away by 'Extrude 1', which comes before this fillet." When Partsmith cannot tell why, it says so ("Partsmith could not tell why") instead of guessing.
3. **What to do**, as action buttons first. The text names the fix only when no button covers it, or in a host without buttons.

Then, when the host knows it, one **cause line**: "This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm." It names the user's own edit, is shown under the message, and carries its own actions ("Set Distance back to 5 mm", "Undo", "Show the line"), because they depend on the session's history, not on the document.

And one step away, never in the sentence: the **detail** for experts and bug reports (the code, the stored reference, the resolver result, the kernel operation and its text, the panic message), behind "Copy details", in `--json`, or on the exception object.

At most two sentences plus the cause line. If it needs more, it is two problems, or the detail.

**Severity:**

- **Error:** the feature, command or file could not do what was asked; nothing was changed by it.
- **Warning:** it was done, but not entirely as asked: something was skipped, guessed, widened, or is likely wrong. Anything that changes material the user did not pick (a region that now covers a different area, an edge that now runs along several replacement lines) is a warning.
- **Notice:** it was done exactly as asked and followed a change on its own that keeps the user's intent: a line replaced one for one, a line continued on the same line, a region split by new lines without changing its area, a face continued by the rim a shell left. A notice never makes a feature look broken and never raises a notification.

## Name things the way the user sees them

| Thing | Write | Never |
| --- | --- | --- |
| Feature | its label in single quotes: 'Extrude 2', 'Bracket base' | `f-ext2`, `extrude f-000000000003` |
| The feature the message is on | "this fillet" (its type in lowercase) | "this feature" when the type is known |
| A feature that is no longer in the document | "a feature that is not in this document"; the cause line, which knows the history, says "after you deleted 'Extrude 1'" | its id, or "deleted" when the code cannot know it was deleted |
| Body | its name in single quotes: body 'Bracket' | `b-1` |
| Field | its inspector label: Distance, Edges, Operation | `extent.expr`, `size[0]`, `op` |
| Value | value with unit, as typed or in document units: 2.4 mm, 90 deg, 0.094 in | 2.4, 2.39999999 |
| Expression | the expression and its value: "Fillet radius 30 mm (r * 2)" | the value alone when the user typed an expression |
| Face, edge, vertex | words from its feature, position and sketch entity: "the front top edge of 'Box 1'", "the top edge of 'Extrude 1' along the line from (80, 40) to (70, 50) in 'Sketch 1'", "the top rim of the hole at (7, 0)", "the floor of the cut made by 'Extrude 2'" | `edge(f-box1/front\|f-box1/top)`, Edge12, Face7 |
| Sketch entity | kind and position: "the line from (0, 50) to (0, 0) in 'Sketch 1'", "the circle at (40, 25)" | `e-4` |
| Constraint | its title and value: "Distance 10 mm", "Horizontal on the line from (0, 0) to (80, 0)" | `c-3` |
| Copy of a pattern | "the third copy of 'Linear pattern 1'" | `@f-lp1:2` |
| File | the name the user picked: bracket.step | `assets/3fa2...step`, temporary paths |
| The user's edit | "after you changed Count of 'Linear pattern 1' from 3 to 2" | "after an upstream change" |
| A replacement candidate | what it is, what it keeps and how far it is: "the top edge of the cut made by 'Extrude 1', on the top of 'Box 1', 5 mm from where it was" | "91% match", a score, a rank by score |

Ids, element names and hashes may appear in the detail and, for script and file users, next to the words in CLI and Python output ("Fillet 1 (f-fil1)").

## Plain words

| Instead of | Write |
| --- | --- |
| op new, op join, op cut | Operation New body, Join, Cut (the inspector's labels) |
| reference, element, query, fingerprint, resolver | the edge, the face, "picked" ("the edge picked for Edges") |
| can no longer be found (for an ambiguous match) | now matches 2 edges |
| repair panel | Edit 'Fillet 1' and pick it again, or the button |
| tessellation, mesh tolerance | mesh detail |
| canonical form | standard layout |
| non-manifold edges | two parts touch only along an edge |
| degenerate, sliver | thinner than 0.000001 mm, a face only 0.00001 mm across (when measured) |
| region, profile region | the closed shape, the region (in sketch context) |
| invalid type: map, expected a sequence | "bodies" must be a list in [ ], but it is { } |
| geometry kernel failed (StdFail_NotDone ...) | Partsmith's geometry kernel could not build 'Fillet 1' |
| internal error (called `Option::unwrap()` on a `None` value) | Partsmith hit a bug while building 'Shell 1' |
| No such file or directory (os error 2) | the folder "nodir" does not exist |
| no effect | adds nothing to body 'Body 1': it lies entirely inside the body |
| open profile, open wire | 'Sketch 1' has no closed shape: 2 ends are not connected |

## Never in the message text

- Error codes, ids, element names, hashes, JSON Pointers (they go to `data-code`, the detail, or the CLI's location column).
- Kernel, OCCT, serde, OS or panic text, exception names, backticks, stack traces, kernel indices ("Edge 12": the numbered naming the project exists to avoid).
- Temporary file names, byte offsets, internal limits by their variable names.
- Match percentages or fingerprint scores: a 100% lookalike is exactly what made FreeCAD's breakage silent. Scores go to the detail.
- A sentence nested in parentheses inside another sentence.
- Advice that does not apply ("check that the profile is closed" when built-in profiles are always closed; "turn on Reverse" when reversing would miss too; a size limit of 0.0058 mm for a failure that is not about size).
- Anything untrue: "stays at its last position" when it moved, "a newer version wrote this" when it is a typo, "deleted" when the code only knows the feature is not in the document.
- A cause the code did not establish. Blame a small face only when it was measured below the requested value; state facts ("its rounded faces have radius 1 mm, smaller than the wall") rather than conclusions the code cannot check. Otherwise say "Partsmith could not tell why".

## One-click fixes

- Every message whose fix the app can compute carries it as an action: a value ("Use 2.3 mm"), a replacement ("Use the top edge of the cut made by 'Extrude 1'"), a timeline action ("Unsuppress 'Chamfer 1'", "Move 'Fillet 1' below 'Extrude 2'"), a cleanup ("Remove from Edges"), a sketch action ("Connect the ends (0.3 mm apart)", "Delete Distance 30 mm"), or a jump to the root cause ("Show 'Datum plane 1'"). Reverts of the user's own edit ("Set Count back to 3") belong to the cause line.
- Labels start with a verb and say exactly what will change, with the value and the target. Never "Fix", "OK" or "Apply" alone.
- An action never changes more than its label says. When the field holds an expression, the label says so: "Replace r * 2 with 19 mm", or better "Set r to 9.5 mm".
- An action is computed by the core only when it works or is exact: "Turn on Reverse" only when the reversed shape reaches the body, "Remove that edge from Edges" only when the other edges work at the requested radius.
- Every action is one undo entry with the action's label, and is applied to the document it was offered for: when the document changed in between, the host says the fix no longer applies instead of applying it.
- Candidates are offered, never applied by themselves. Each reads as what it is, then what it keeps of the lost element and how far it is from where that was; candidates are listed in that order (those that keep a face of the lost element first, then by distance), the same order the words describe.
- Order: the fix that keeps the user's intent first (a candidate that keeps a face of the lost element, the value that keeps the parameter link), then timeline fixes (Unsuppress, Move), then other candidates, then removals, then "Pick again". The cause line's actions come after the message's.
- No fix for a reference that is still right: when the cause is upstream (a sketch that fails), offer "Show 'Sketch 3'", never "Pick again", which would destroy a correct reference.
- Notifications never offer a candidate, which cannot be judged without seeing it: they offer "Show what 'Fillet 1' lost", the cause line's revert when there is one, and "Undo".

## One root cause, one message

- When one problem breaks several features, the first one carries the real message; the others say which feature they depend on and name the root ("'Sketch 1' cannot be built because 'Datum plane 1' has an error, so this extrude cannot use it either. Fix 'Datum plane 1' first."), and hosts group them under the root.
- A message names the root, not the next link of the chain: the parameter that divides by zero, not the parameter that uses it; the sketch that lost its face, not the extrude that adds nothing because of it.
- Identical findings are merged with a count, never repeated.

## Tone

- A calm, precise colleague: factual, specific, short. No "Error:", no exclamation marks, no "Oops", no apologies, no "please" except when asking for a report.
- No blame: "this fillet is too large for the selected edges", not "you entered an invalid radius". The user's edits are named neutrally as causes ("after you changed ..."), never as mistakes.
- Present tense for the state, past tense for events: "is empty", "was cut away".
- American English, sentences end with a period, numbers with units, no abbreviations a newcomer would not know.
- Bugs are Partsmith's, and say so: "Partsmith hit a bug while building 'Shell 1'. Your document is unchanged. Suppress 'Shell 1' to keep working, and report it with Copy details." A bug is never presented as the user's input error, and a user's input error is never presented as a bug.

## Hosts

The same problem reads the same everywhere; only the way to act differs.

- **UI:** buttons for actions; the cause line and its actions under the message; a notification for every new lost reference, region or sketch face; the lost element's ghost and the numbered candidates in the viewport.
- **CLI:** the message, then the location (`model.ptsm:17: /features/1/edges/0`), then the actions as instructions ("suggested: radius = 2.3 mm", "candidate: edge(f-box1/top|f-ext2/lateral/e-3), on the top of 'Box 1', 5 mm from where it was"); `--json` carries codes, pointers, actions, candidates with their scores, reasons and details. Exit codes, not words, tell scripts what failed.
- **Python:** exceptions and warnings carry `code`, `field`, `suggestion`, `actions`, `candidates`, `cause` and `detail` as attributes; the message starts with the call that failed (`fillet(): ...`) and points at the user's line, not Partsmith's internals; advice uses Python terms ("pass a new edge, for example block.edges().parallel_to(Axis.Z)"). Indexing a selection by number (`edges()[3]`) warns, because a position is the numbered naming the project avoids.

## Examples from the inventory

| Before | After |
| --- | --- |
| The edge selected for Edges can no longer be found. Re-pick it in the repair panel. | The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet. [Use the top edge of the cut made by 'Extrude 1'] [Use the front edge of the cut made by 'Extrude 1'] [Pick an edge again] *This started after you added 'Extrude 1' before this fillet.* [Suppress 'Extrude 1'] [Undo] |
| One of the edges selected for Edges now matches 2 edges, so it was skipped. Pick the one you mean in the repair panel. | The front top edge of 'Box 1' selected for Edges now matches 2 edges after 'Box 1' changed, so it was skipped. Pick the one you mean. [Use the front top edge of 'Box 1'] [Use the back top edge of 'Box 1'] |
| The face this sketch is on can no longer be found, so the sketch stays at its last position. Re-attach it in the repair panel. | The face this sketch is on, the top of 'Box 1', was cut away by 'Extrude 1', which comes before this sketch, so the sketch stays where that face was. [Attach to the floor of the cut made by 'Extrude 1'] [Pick a face] |
| Body 'Body 1' is empty at this point. Use op new to start it. | Body 'Body 1' is empty here because 'Extrude 1', which makes it, is suppressed. [Unsuppress 'Extrude 1'] |
| 'Sketch 1' has an error, so this feature cannot use it. Fix 'Sketch 1' first. | 'Sketch 1' cannot be built because 'Datum plane 1' has an error, so this extrude cannot use it either. Fix 'Datum plane 1' first. [Show 'Datum plane 1'] |
| Fillet radius 2 in is too large for the selected edges. The largest radius that works is about 1.9 in. Try 1.9 in or less. | Fillet radius 2 in is too large for the selected edges: the largest that works is about 1.9 in. '2' has no unit, so it means 2 in in this document. [Use 2 mm] [Use 1.9 in] *This started after you changed the document units from mm to in.* |
| Fillet radius 3 mm does not work on the selected edges, even when smaller. Check for edges next to very small faces, or fillet fewer edges at once. | Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm. The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across. [Use 2.3 mm] |
| Shell thickness 2 mm is too thick for body 'Body 1'. The thickest wall that works is about 0.99 mm. Try 0.99 mm or less. | Shell thickness 2 mm is too thick for body 'Body 1': the largest that works is about 0.99 mm. Its rounded faces made by 'Fillet 1' have radius 1 mm, smaller than the wall. [Set Radius of 'Fillet 1' to 3 mm] [Move 'Shell 1' above 'Fillet 1'] [Use 0.99 mm] |
| This feature does not change body 'Body 1'. Check its position, size and direction. | This extrude adds nothing to body 'Body 1': it lies entirely inside the body. [Attach 'Sketch 2' to the top of 'Extrude 1'] |
| (Extrude disabled) Draw a closed shape in a sketch first. | 'Sketch 1' has no closed shape: 2 ends are not connected. [Show 'Sketch 1'] (in the sketch: [Connect the ends (0.3 mm apart)]) |
| Constraints c-2, c-4 conflict: they cannot all be satisfied at once. Delete one of them or change a dimension. | In 'Sketch 1', Distance 30 mm cannot hold together with Distance 40 mm. [Delete Distance 30 mm] [Make Distance 30 mm a reference] [Delete Distance 40 mm] |
| The geometry kernel failed unexpectedly (StdFail_NotDone: BRep_API: command not done). Please report this with the model file. | Partsmith's geometry kernel could not build this fillet. Fillets fail like this when the edges meet other rounded edges at a corner; try a slightly different radius, or fillet those edges in two steps. [Suppress 'Fillet 1'] [Copy details] |
| The extrusion failed. Check that the profile is a closed region that does not cross itself. | Distance must be at most 1000000 mm, but it is 1e200 mm. [Use 1000000 mm] |
| The box could not be built. | Length must be at most 1000000 mm, but it is 1e200 mm. [Use 1000000 mm] |
| This feature's settings are invalid: 'size[0]' has the wrong form (invalid type: integer `20`, expected a string). Fix the setting, or delete and recreate the feature. | Length of 'Box 1' must be written in quotes, such as "20" or "20 mm" (line 12). (Better still: accept the number.) |
| This feature has settings this version of Partsmith does not know ('remove_face'). They are ignored and kept in the file. | 'remove_face' is not a Shell setting, so it is ignored. Did you mean Faces to remove? [Rename remove_face to remove_faces] |
| Something went wrong inside Partsmith (called `Option::unwrap()` on a `None` value). Save your work and reload. Please report this. | Partsmith hit a bug while rebuilding 'Shell 1'. Your document is unchanged. Suppress 'Shell 1' to keep working, and report it with the details. [Suppress 'Shell 1'] [Copy details] |
| error: cannot write "nodir/a.stl": No such file or directory (os error 2) at path ".../.partsmith-nQaQjQ" | error: cannot write "nodir/a.stl": the folder "nodir" does not exist. Create it first, or choose another --output. |
| The imported file assets/0000...0000.step is missing. Put it next to the document in the assets folder. | The STEP file of this import is not in this document's assets folder. Copy the assets folder that came with the document next to it, or import the file again. [Import again] |
| (no message: the fillet silently rounds another edge) | The top rim of 'Extrude 1' selected for Edges belongs to 'Extrude 1', which is suppressed. [Unsuppress 'Extrude 1'] [Pick an edge again] |

## Checklist

Before a message merges:

- [ ] It says what, why, and what to do, in at most two sentences, with the affected thing named by label.
- [ ] Its severity matches what happened: a notice only when the user's intent is kept exactly.
- [ ] Every computable fix is an action with a verb label, offered only when it works or is exact, and applying it is one undo entry.
- [ ] No ids, codes, element names, scores, kernel, serde or OS text in the sentence; the technical text is in the detail.
- [ ] It is true in every case the code reaches it, including after an edit upstream and in a document opened fresh, without the session's history.
- [ ] Downstream features point at the root cause instead of repeating it.
- [ ] Its exact text is covered by a test, and the message lint passes over every rendered template in the acceptance tests: no core message matches `repair panel`, `\bop (new|join|cut|intersect)\b`, `\b[fbec]-[0-9a-z]+\b` outside the detail, `(Edge|Face|Vertex|Curve) [0-9]`, `[0-9]+% match`, a backtick, or `os error`.
