# W2-1C-2: Up-to targets, direction and reach checks

Status: Accepted (owner's rule of 2026-11-05). It changes no file format, protocol schema, naming algorithm, license or dependency beyond [ADR W2-1C-1](W2-1C-1-extent-records-of-extrude-and-revolve.md)'s records. It settles kernel.md open issue 21 (d) for the cases kernel.md section 8.5 measured, and refuses the rest with messages of their own until spike 8.10's results.

## Context

- **What was measured.** Kernel.md section 8.5 picked the split prism: a prism through the body, split by `BRepAlgoAPI_Splitter` at the target face's surface or at the whole body, keeping the pieces that hold an image of the start face. It passed on `RECT` up to a parallel plane, a plane tilted 20 degrees, the side of a cylinder and a body, all with one region of lines. Its tapered prism is `BRepOffsetAPI_DraftAngle`, split the same way.
- **What was not.** Cone, sphere, torus and freeform targets; a face the prism never reaches; inward tapers through deep bodies; profiles of several regions (the spike's shim refused them); profiles with conics or B-splines, which item 14 brings in the same wave. Spike 8.10 measures these in stage 2 (kernel.md section 8.10, track 1B's criteria). Kernel.md section 8.1 item 7 lets a feature spec specify only the construction its spike picked.
- **Silent wrong results.** The splitter cuts at the target's extended surface and reports nothing. When the prism never crosses the surface the whole through-prism comes back, with no end face. When the profile reaches a body only in part, a piece of the prism runs past the body to the far cap (kernel.md section 8.5, open issue 21 (d) and (e)).
- **Gate 7.** "An extrude up to a face it never reaches" is one of M2's newcomer mistakes, tested in recompute, the engine and Playwright.

## Decision

1. **Targets.** `up_to_face` ends on a planar or cylindrical face. Every other surface kind is refused with `up_to_surface`, whose message names the kind (`conical`, `spherical`, `toroidal`, and so on, features/extrude.md section 4). A face reference that resolves to several faces is used when they all lie on one plane or one cylinder, and the end must lie within their union. `up_to_body` ends where the prism first meets the `target` body, any body of the document: it is read through `bodies_read` unless it is the body the extrude writes.
2. **Profiles.** `up_to_face` and `up_to_body` take a profile whose union is one face of lines, arcs and circles, holes allowed. A union of several faces, or one with a conic or B-spline curve, is refused with `up_to_profile`, before the kernel runs. `up_to_vertex` is a computed distance ([ADR W2-1C-4](W2-1C-4-up-to-a-vertex-and-the-vertex-selection-kind.md)) and takes every profile a distance takes.
3. **Direction.** The extrude runs along its direction, the sketch normal turned by `reverse`, as in M1. It never turns toward the target by itself. A target behind the start fails with `up_to_misses`, and Turn on Reverse is offered when the extrude the other way reaches the whole target.
4. **Reach, from the split.** The extrude is refused, never built short or long, when the split shows that the end does not lie wholly on the target: `up_to_misses` when no part of the profile reaches it (the face lies beside the prism's path, or the prism misses the body), `up_to_partly` when only part does (an end sample outside the face, or the prism's far cap among the kept pieces), and `up_to_starts_inside` when the profile starts inside the target body. The far-cap check is the one the rib needs (kernel.md open issue 21 (e)), so item 28 reuses it.
5. **The second split.** When the split along the direction reaches nothing, the feature runs the same split the other way from the same start plane, on that error path only, as `through_all_misses` runs one `common` (features/extrude.md section 3.3). It tells a target behind the start from one beside the path.
6. **Order of the checks.** The record and profile checks, the start, the target, the kernel's split, the reach checks on its result, then `apply_op`. The first that fails is reported.
7. **The kernel's part.** The split, the reach outcomes and the draft are track 1B's calls (ADR W2-1B-2, kernel.md section 2.6): `TargetNotReached` and `TargetPartlyReached` (its decision 3). A start inside the target body is one more outcome the shim tells from the kept piece's start face, which this ADR asks of the kernel spec. The feature maps each outcome to its error, and no message holds a kernel index.
8. **Widening.** Track 4A widens targets, multi-region profiles and conic and B-spline profiles from spike 8.10's recorded Results, in features/extrude.md from test 126 on.

## Alternatives considered

Every option is open source, built on OCCT. The first criterion of the rule picks none of them. The rule then takes the safest, then the industry standard, then the planner's recommendation. Each choice is Accepted (owner's rule of 2026-11-05).

1. *Targets.* (a) Planar and cylindrical faces, measured; (b) also conical faces, which the spike's cutting face accepted without a measurement; (c) any surface, through a cutting face built on the surface's own bounds. The rule takes (a), the safest: kernel.md section 8.1 item 7 lets a spec use only what its spike measured. (b) and (c) wait for spike 8.10.
2. *Direction.* (a) The explicit `reverse` of M1, with a target behind the sketch refused with a Turn on Reverse fix; (b) the direction turned toward the target automatically (Fusion's "To Object"), with `reverse` ignored. The rule takes (a), the safest: a moving target cannot flip the extrude without a message, and `reverse` keeps one meaning in every extent. It matches `through_all_misses`.
3. *Never or partly reached.* (a) Refused, each with its own message; (b) built as the splitter gives it. The rule takes (a), the safest: (b) builds a through-prism with no end face, or a stub past the body, with no word to the user.
4. *Which body up to a body reads.* (a) The extrude's own body only; (b) any body, through `bodies_read`. The rule takes (b), the industry standard: Fusion's "To Object" takes any body. The dependency rules already cover a read body.
5. *A start inside the target body.* (a) Up to the body's exit; (b) refused. The rule takes (b), the safest: (a) runs the other way from what "up to the body" says, through the body the user wanted to reach.
6. *Multi-region and curved profiles.* (a) Refused until spike 8.10; (b) the compound swept and split at once, unmeasured; (c) one sweep per region, then a fuse. The rule takes (a), the safest, as the wave 2 decisions, item 1, require.

## Consequences

- features/extrude.md sections 3 and 4 and tests 71 to 90, 114 to 116 and 120 specify this. features.md section 4.1 gains `up_to_misses`, `up_to_partly`, `up_to_starts_inside`, `up_to_surface` and `up_to_profile`.
- The newcomer error corpus gains NC14, the face never reached, with its recompute, engine and corpus assertions, and its E-test from track 2C.
- Track 1B's kernel calls return the reach outcomes, distinguishable without parsing a message: never reached, partly reached and starts inside.
- A join that ends exactly on a face of its own body makes coincident faces, which kernel.md open issue 22 lists as a risk on the web. features/extrude.md test 74 builds one.
- The plan's deferrals of other surfaces and of multi-region and curved profiles are routed to 4A by the wave 2 decisions (item 1), and in a dated m2-plan.md section 5 note by the stage 1 integrator.
