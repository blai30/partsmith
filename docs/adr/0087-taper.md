# 0087: Taper

Status: Accepted (owner's rule of 2026-11-05); needs human review at the M2 gate through the `taper` key of format version 3 ([ADR 0085](0085-extent-records-of-extrude-and-revolve.md) decision 2). It settles the taper's sign, range, neutral plane, the extents that take one, and the steep inward taper of kernel.md open issue 21 (d).

## Context

- **The construction.** kernel.md section 8.5 picked `BRepOffsetAPI_DraftAngle` on the lateral faces of a prism, with the neutral plane at the profile and a positive angle tapering outward in the spike's facade. It met the angle within 1.5e-16 rad on `RECT`, a profile with a hole and the 100-edge profile, tapered 5 degrees outward and inward over 10 mm, and up to a tilted face. It took 51 to 57 ms on the 100-edge profile against 60 ms, with 4 of its 20 single-set medians over 60 ms, and it never polls inside its `Build`.
- **Not measured.** A taper on a profile of several regions (the shim refused them), on conic and B-spline curves, two-sided and symmetric tapers, and an inward taper long enough to close the profile. Spike 8.10 measures the steep taper, several regions and the curved profiles.
- **Fusion.** Each side of an extrude has a taper angle. A negative angle narrows the extrude and a positive one widens it (the Fusion API's `ExtrudeFeatureInput.taperAngle`, and Inventor's extrude, where a negative taper decreases the section along the extrusion).

## Decision

1. **Sign.** A positive taper angle widens the extrude along its direction, and a negative one narrows it: each lateral face turns by the angle away from the extrude direction, outward for a positive angle. This is Fusion's convention and the spike's.
2. **Range.** The angle is greater than -90 degrees and less than 90 degrees, checked as `OutOfRange` on `extent.taper`. An angle within 1e-9 rad of 0 builds the plain prism: the same solid and names as no taper.
3. **Neutral plane.** The taper's neutral plane is the start cap's plane, the sketch plane or the start the `start` key sets. The start cap keeps the profile's size, and the end cap grows or shrinks.
4. **Extents.** `distance`, `through_all`, `up_to_vertex` and `up_to_face` on a flat face take a taper. Up to a flat face, the drafted prism is split as the plain one is, the case kernel.md section 8.5 measured on a tilted face. A taper up to a body or a cylindrical face, which the spike did not measure and the kernel refuses ([ADR 0083](0083-extrusions-that-end-on-a-face-or-a-shape-and-revolutions-between-two-angles.md) decision 2), fails with `taper_target`, whose fix clears the taper. The `up_to_body` record keeps its `taper` key for track 4A to widen from spike 8.10. `symmetric`, `two_sides` and `through_all_both` take none; their schema refuses the key.
5. **Profiles.** A taper takes a profile whose union is one face of lines, arcs and circles, holes allowed. Several faces, or a conic or B-spline curve, are refused with `taper_profile`, whose fix clears the taper.
6. **A taper that closes the profile.** With a taper, a kernel error of kind `Failed`, `InvalidResult` or `EmptyResult` makes the extrude fail with `taper_too_steep`: `A taper of {angle} closes part of the profile before the end of {this}. Use a smaller taper, or a shorter extrude.` The kernel error goes to the detail. There is no precheck. The kernel reports no kind of its own for a failed draft. Up to a flat face, a failed split takes the same message. Track 4A narrows that mapping once spike 8.10 or a later kernel round gives the draft's failure a kind of its own. The outcome is provisional until spike 8.10's Results: kernel.md section 3.4.3 leaves the detection of a closing taper to that spike, and track 4A confirms or changes features/extrude.md test 107 by spec first. An outward taper closes a hole of the profile the same way, and the message covers both.
7. **Names.** A tapered lateral face keeps `<id>/lateral/<entity id>`: the draft's `Modified` carries each prism face to its drafted face. Caps keep `start-cap` and `end-cap` ([ADR 0089](0089-names-of-the-new-faces-and-the-order-of-the-through-all-checks.md)).

## Alternatives considered

Every option is open source, built on OCCT. The first criterion of the rule picks none of them. The rule then takes the safest, then the industry standard, then this work's recommendation. Each choice is Accepted (owner's rule of 2026-11-05).

1. *Sign.* (a) Positive widens, Fusion's and the spike's convention; (b) positive narrows, as a mold draft reads. The two are as safe as each other: each builds the same solids, and the field's help says which way a sign turns the sides. The rule takes (a), the industry standard. A user who comes from Fusion types the angle they know.
2. *Which extents.* (a) Every one-sided extent; (b) distance only; (c) every extent, with two sides as one drafted face through the neutral plane or as mirrored halves. The rule takes (a), the safest that covers item 17: kernel.md section 8.5 measured a one-sided taper and a taper up to a face. (c) was not measured, and its mirrored halves split each lateral face in two, which needs a naming rule of its own.
3. *A steep inward taper.* (a) A precheck of the length times the tangent of the angle against the profile's inset; (b) the kernel's failure, or an invalid or empty result, mapped to a message of its own; (c) both. The rule takes (b), the safest: it holds for any profile and needs no inset geometry code. (a) needs an exact inset of every profile, and a precheck that disagrees with the kernel would refuse a valid taper.
4. *Neutral plane.* (a) The start cap; (b) the end cap; (c) a plane the user picks. The rule takes (a), the safest: it is the construction kernel.md section 8.5 measured. It is also the industry standard (Fusion's taper starts at the profile).

## Consequences

- features/extrude.md specifies the taper in sections 2 to 6 and tests 102 to 113. features.md section 4.1 gains `taper_profile`, `taper_target` and `taper_too_steep`.
- The budget row of the tapered 100-edge extrude is set in wave 4 from its measured median plus 20 percent ([ADR 0057](0057-budgets-set-from-the-measured-median-plus-20-percent.md)). Its margin was thin in the spike. A row over its value needs its own ADR on gate 19's list.
- `DraftAngle` never polls inside its `Build` (kernel.md section 8.5). A cancel during a taper waits for the draft to end. kernel.md section 3.13 lists it.
- Two-sided and symmetric tapers go to M3 (m2-plan.md section 5.1, note of 2026-11-23). Track 4A widens the taper to curved and multi-region profiles from spike 8.10's results.
