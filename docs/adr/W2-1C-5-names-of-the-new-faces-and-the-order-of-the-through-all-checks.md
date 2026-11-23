# W2-1C-5: Names of the new faces, and the order of the through-all checks

Status: Accepted (owner's rule of 2026-11-05); needs human review at the M2 gate: it adds naming rules to naming.md sections 3.6.2 and 3.6.3 (decisions 1 to 5), and it changes an M1 acceptance test by spec first (decision 6, features/extrude.md test 61). It settles features/extrude.md open issue 6.

## Context

- **M1's rule.** An extrude's or revolve's faces are named by their kernel role: `<id>/start-cap`, `<id>/end-cap` and `<id>/lateral/<entity id>`, with `#n` by naming.md section 3.4's geometric order when a role gives several faces. A boolean that merges faces records a merge alias (naming.md section 3.3 step 2).
- **What the new extents make.** The split prism's end face is the image of the cutting face up to a face, and is also `Modified` of the body's face up to a body, where the end can be several faces (kernel.md section 8.5). `DraftAngle`'s `Modified` names each drafted face from its prism face. A two-sided revolve has caps at two angles that are not 0.
- **Open issue 6.** Through all with op new on a body with a solid gives `body_not_empty`, whose one fix sets Join. In test 61's geometry the sweep points away from the body, and the fix leads to `through_all_misses`, whose Turn on Reverse then works: a fix that does not work at its step.

## Decision

1. **The up-to end is the end cap.** Every face the kernel gives the `EndCap` role, the image of the cutting face up to a face, and every face it gives the role `Target { face }`, a piece of a face of the body up to a body (track 1B's ADR W2-1B-2 decision 4), is named `<id>/end-cap`. The face index of `Target` never enters a name. Several are numbered `<id>/end-cap#n` by naming.md section 3.4. A start face or a start offset leaves the start cap `<id>/start-cap`.
2. **Merges with the target.** When a join or cut makes the end cap one face with a face of the body the extrude writes, the merge rule of naming.md section 3.3 step 2 applies unchanged: the merged face takes the smaller name, and the other becomes its alias.
3. **Tapered faces keep their names.** A tapered lateral face is `<id>/lateral/<entity id>`, as the untapered one, through the draft's `Modified`. A taper of 0 gives the same faces and names as no taper.
4. **Through all both ways** names its caps as through all does: the start cap at the far side against the direction, the end cap at the far side along it. A cut through all both ways records both caps absent with `Through` when neither is in the result.
5. **Revolve two sides and symmetric.** The start cap is the profile turned to the start angle (the opposite angle against the sense, or half the angle), and the end cap the profile turned to the end angle. A total within 1e-9 rad of a full turn has no caps and records both absent with `FullTurn`, as M1's full revolve does.
6. **Open issue 6: misses first.** Through all with op new on a body with a solid reports `through_all_misses` before `body_not_empty` when its sweep does not reach the body. Turn on Reverse is offered when the reversed sweep reaches it, as for every `through_all_misses`. Then `body_not_empty` offers Set Operation to Join, which builds. features/extrude.md test 61 changes by spec first.

## Alternatives considered

Every option is open source and adds no dependency. The first criterion of the rule picks none of them. The rule then takes the safest, then the industry standard, then this work's recommendation. Each choice is Accepted (owner's rule of 2026-11-05).

1. *Names of several up-to-body ends.* (a) `end-cap#n` in naming.md's geometric order; (b) each end named after the body face it lies on, a new generator rule. The rule takes (a), the safest: it reuses M1's machinery and adds no generator. Its names depend on order, as M1's separate caps of disjoint regions do. The corpus measures them (gate 6).
2. *Open issue 6.* (a) Offer Join only when the sweep reaches the body; (b) report `through_all_misses` before `body_not_empty`. The rule takes (b), the safest: each fix it offers works at its own step. (a) leaves the user with no fix when the sweep points away.

## Consequences

- naming.md sections 3.6.2 and 3.6.3 state these rules, with tests 201 to 214. Two-sided tapers wait ([ADR W2-1C-3](W2-1C-3-taper.md)), and no rule names the halves of a lateral face that one would split. features/extrude.md section 3.4 and features/revolve.md section 3.4 list the names.
- features/extrude.md test 61 changes by spec first, and open issue 6 is resolved. The critic checks that the test is not weakened: it keeps `body_not_empty`'s message and its single fix, now reached after Turn on Reverse.
- Order-dependent `end-cap#n` names for up-to-body ends are measured by the naming metric (gate 6) over corpus.md's brackets models, and naming.md open issue 8 records the alternative.
