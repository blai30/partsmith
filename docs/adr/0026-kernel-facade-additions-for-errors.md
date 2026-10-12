# 0026: Kernel facade additions for ghosts, limits, void checks and true hints

Status: Accepted, needs human review

## Context

The kernel facade (`partsmith-occt-sys`, kernel.md) is the only place that sees OCCT. [The reference stability and errors study](../design/reference-stability-and-errors.md) found 35 of 123 corpus tests passing today, and 38 that silently use the wrong element; its message inventory found that the 12 kernel failure texts that reach users through features all fail to say why, and 5 also fail to say what to do ("The box could not be built."). Some are false: the extrude text blames the profile ("Check that the profile is a closed region that does not cross itself") for a failure caused by a 1e200 mm distance, although built-in profiles are always closed, and "Moving the body failed." appears under Mirror and patterns, where nobody moved anything. Raw OCCT exception text ("StdFail_NotDone: BRep_API: command not done") reaches the sentence.

Several fixes of the study need facts only the kernel has:

- **Closed voids and separate solids.** A join that closes a hollow inside the part (R55) or a join whose tool does not touch the body (newcomer corpus NC04) should warn, but result measurements carry no solid or shell counts.
- **Limits that name their cause.** The error guidelines allow blaming a small face only when it was measured below the requested value. The fillet of newcomer scenario NC05 fails because the face next to its edge is only 2.4 mm across, which nothing measures today.
- **Ghosts.** Repair mode draws the lost element where it was in its last build (ADR 0024), which needs the display geometry of one element of the previous body state; the facade only tessellates whole shapes.

Changing the facade adds C++ shim code and safe Rust API at the one C++ boundary (CLAUDE.md invariant: no C++ types, exceptions or `unsafe` outside `partsmith-occt-sys`).

## Decision

The kernel.md changes K1 to K4 of the study (section 7.5):

- **K1, solid and shell counts.** Results of solid operations report `solids` and `shells` in their measurements (kernel.md section 3.6). Features compare inner shells (shells minus solids) before and after a join or cut for `closed_void`, and lumps for `separate_solid`.
- **K2, `Shape::width_across(face, edge) -> Result<f64, KernelError>`.** The largest distance from the edge's curve to a point of the face, for the "too large" messages. Used on failure paths only; budget 5 ms per face on the acceptance shapes.
- **K3, `Shape::element_display(kind, index, tolerance) -> Result<ElementDisplay, KernelError>`.** The display geometry of one element (an edge's polyline, a face's triangles, a vertex's position) at a tessellation tolerance, for ghosts. Budget 5 ms for one face of a 1,000-face body.
- **K4, true hints.** kernel.md section 4.3 gives one hint per operation, used after the features' sentence "Partsmith's geometry kernel could not build {this}."; each hint is true whenever the kernel fails in that operation and never blames input that was checked before the kernel. Fillet, chamfer and shell use the "too large" search instead. `Internal` reads "Partsmith's geometry kernel hit an unexpected problem.", and the kernel's own text goes to the message's `detail`.

Input that can be checked is checked before the kernel by the features (the modeling limit of 1,000,000 mm, features.md section 3.4), so kernel hints never stand in for a range check.

## Alternatives considered

- **Compute widths and ghosts outside the kernel.** Fingerprints carry type, centroid, area or length and adjacency, not the width of a face across an edge; drawing one ghost by tessellating the whole previous shape costs far more than one element and needs a kernel call anyway.
- **Infer voids from volumes alone.** A join around a void can change the volume like any other join; only shell and solid counts tell a sealed cavity apart, and they are read while measurements already traverse the result.
- **Keep OCCT's exception text in the message with a friendlier prefix.** The guidelines forbid kernel text in the sentence, and the text rarely says anything a user can act on; it stays available in `detail` for reports.

## Consequences

- Three additions to the facade's safe API and C++ shim, using OCCT algorithms already linked (`BRepExtrema_DistShapeShape` for widths, the existing meshing for display geometry); no new dependency, and the WebAssembly build changes only by the added code. The dependency log (ADR 0020) is unchanged.
- kernel.md gains acceptance tests (solid and shell counts of a join around a void, `width_across` on the NC05 step, `element_display` of a face and an edge, every hint text) and two budget rows.
- Features can state measured facts ("which is only 2.4 mm across", "its rounded faces have radius 1 mm, smaller than the wall") and warn about sealed cavities and floating joins.
- This changes the kernel facade, so it waits for human review at the next gate. It gates the phase 3 kernel code of the study's plan; K3 is used by the engine's ghosts in phase 4.
