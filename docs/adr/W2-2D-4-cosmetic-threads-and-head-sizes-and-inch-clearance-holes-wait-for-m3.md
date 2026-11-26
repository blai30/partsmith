# W2-2D-4: Cosmetic threads, and head sizes and inch clearance holes wait for M3

Status: Accepted (owner's rule of 2026-11-05): the M2 plan's open choices "What a cosmetic thread is" and "Head sizes for Hole.counterbore(M3), and inch clearance holes", decided as the wave 2 decisions of 2026-11-19 accept them ([m2-wave2-decisions.md](../design/m2-wave2-decisions.md) section 3). Needs human review at the M2 gate for decision 2, a scope deferral that changes m2-plan.md item 33's example and technical design section 10's (m2-plan.md gate 19 and section 7.3 item 16's note of 2026-11-23). Decision 1 changes no protocol schema, file format, naming algorithm, license or dependency.

## Context

- **The cosmetic thread** is named in m2-plan.md item 20 ("modeled by default, or cosmetic"), in [ADR 0074](0074-modeled-threads-lofted-between-helical-wires-and-divided-per-turn.md) decision 4 (a modeled thread's failure message offers it) and in kernel.md section 4.3's hint `A cosmetic thread keeps the face as it is and builds instead.` Nothing says what it is.
- **Fusion and FreeCAD** show a cosmetic thread as a texture or lines on the face, and export it as PMI in STEP AP242. The protocol has no display data for a decoration. Partsmith writes STEP AP214 (io.md).
- **Heads and inch clearance holes.** Technical design section 10 writes `hole(..., kind=Hole.counterbore(M3))`, and m2-plan.md item 33 lists "hole and thread tables (`Hole.counterbore(M3)`)". That needs head sizes per screw (ISO 4762 counterbores, ISO 10642 countersinks). Choice 16 names only ISO 273, a metric standard. None of its tables gives a UNC or UNF screw a clearance hole.

## Decision

1. **A cosmetic thread leaves the geometry as it is.** On a face, the body's shape after the feature is its shape before, byte for byte, with the same element names. It checks the same inputs as a modeled thread and never calls the kernel's thread. It shows in the timeline and the inspector only, with no viewport decoration and no data in exported files. A cosmetic tapped hole bores at the size's basic minor diameter D1, with no clearance (features/thread.md section 7).
2. **Head sizes are explicit in M2, and inch clearance holes wait.** A counterbored or countersunk hole takes the head's sizes as M1's hole does, while its bore can come from a size of the tables (features/hole.md, track 3A). Per-screw head presets (ISO 4762 and ISO 10642) and ASME B18.2.8's inch clearance holes move to M3 with the print presets (m2-plan.md section 5.1). Technical design section 10's example changes from `kind=Hole.counterbore(M3)` to explicit head sizes, and m2-plan.md item 33's example with it (a dated note).

## Alternatives considered

Each choice states its options, the pick and the owner's rule's reasons in order: the most open-source approach, the safest, the industry standard, then this work's recommendation. Every option is open source. The first criterion picks none of them.

1. *What a cosmetic thread is.* (a) Geometry unchanged, with a decoration (helical polylines or a texture) in the body's display data; (b) geometry unchanged, shown only in the timeline and the inspector; (c) the face resized to the tap drill (internal) or the major diameter (external). The rule takes (b), the safest: no protocol change, and the failure message's offer (ADR 0074 decision 4) holds wherever the face exists. (a) is the industry standard but adds display data to the protocol schema. That can come later without changing what is stored. (c) changes geometry a user did not draw, and a thread on an imported part would move its faces.
2. *The bore of a cosmetic tapped hole.* (a) The basic minor diameter D1; (b) ISO 2306's tap drill; (c) D1 with the clearance. The rule takes (a), the safest. D1 is in every row of the tables, and it is the smallest minor diameter of classes 6H and 2B, at which a tap cuts the class's whole thread. (b) adds ISO 2306's table, outside choice 16's list: a change of M2's scope that is the owner's (alternative 3). (c) makes a bore a tap cuts no thread in on small sizes.
3. *Head sizes.* (a) ISO 4762 and ISO 10642 head tables, and ASME B18.3 for inch screws; (b) explicit head sizes as in M1, with the bore from the tables; (c) drop the `Hole.counterbore(M3)` form. The rule takes (b), the safest: both head tables lie outside choice 16's list. Adding them changes M2's scope, and that is the owner's to decide. (b) keeps every M1 hole record as it is.
4. *Inch clearance holes.* (a) ASME B18.2.8's close, normal and loose; (b) none in M2; (c) ISO 273 by the nearest metric size. The rule takes (b), the safest. (c) gives wrong holes for inch screws: the nearest metric size of a 1/4 in screw (6.35 mm) is M6, whose close hole of 6.4 mm leaves 0.025 mm per side. (a) is outside choice 16's list, a change of M2's scope that is the owner's (alternative 3). With (b) a user types an inch clearance hole's diameter, as in M1.

## Consequences

- features/thread.md section 7 holds the cosmetic thread's meaning, and track 3A specifies the feature on it.
- technical-design.md section 10's example writes explicit head sizes (`counterbore=(6.5*mm, 3.3*mm)`) and a clearance size, with track 4B's Python keywords. m2-plan.md item 33 gets a dated note.
- The deferral of per-screw head presets and ASME B18.2.8 clearance holes is on gate 19's list (m2-plan.md section 4 gate 19's note of 2026-11-23 and section 7.3 item 16's note).
- A cosmetic thread adds nothing to STEP, 3MF, OBJ or STL. STEP AP242's thread data waits with AP242 (features/thread.md section 1, Later).
