# 0072: A face that breaks Delabella's hull is triangulated by OCCT's Watson algorithm

Status: Accepted (owner's rule of 2026-11-05). It amends [ADR 0039](0039-portable-triangulation-in-the-shim.md) decision 3 item 4, a decision the owner made for M1, and adds item 7 to that list. It goes to human review at the M2 gate with the other ADRs that amend an owner's decision (m2-plan.md gate 19). No schema, naming, license or dependency change. The copy of Delabella stays MIT, under the same notices entry.

## Context

- **What ADR 0039 decided.** The shim triangulates planar faces and the faces OCCT's deflection control meshes with its own copy of Delabella, in double-double arithmetic. The four `assert`s of OCCT's copy became `require`, which throws `std::logic_error`. The base algorithm turned that exception into a `Standard_Failure`. OCCT's mesher then gave the face no triangulation, and `tessellate` failed with `Failed` (ADR 0039 decision 3 item 4, kernel.md 3.7 item 7).
- **A crash the asserts did not catch.** E-171's stop can leave `examples/bench/fifty-features` built only part way. Tessellating such a state for display crashed the process natively and trapped or hung the web core. A rollback to that part's feature 25 (`f-bool2`) crashed it too, with no stop involved. The face is planar and bounded by a square with round corners. Rounding on its cocircular nodes, which sit in mirror-symmetric positions, breaks the hull of the shim's copy. The copy then reads vertices it never set. OCCT's copy, in the 64-bit mantissa of GCC's `long double` on x86_64, happened to pass on the same nodes.
- **Who found it.** The cancel track (ADR 0063) met it on 2026-11-08 and first recorded the change as that ADR's choice 18, with an edit of ADR 0039 in place. A change to a decision needs an ADR of its own (AGENTS.md Process). This ADR holds it.

## Decision

1. **The copy checks its hull** where OCCT's copy reads vertices it never set (ADR 0039 decision 3 item 7, kernel.md 3.9 item 7). `Face::Alloc` requires a free face. Each point added must leave exactly two more faces (the free list then starts at the first face never used). A vertex field `sewn` holds the index of the point whose silhouette last set its `sew` and `next`. The sewing walk requires every vertex it visits to be set by the current point, and the loop to close after exactly the number of edges the point saw. Each check calls `require`, as the four checks of ADR 0039 do.
2. **A failed check falls back to Watson** (kernel.md 3.7 item 2). The base algorithm catches the exception and triangulates the same nodes, the four auxiliary corners included, with OCCT's Watson algorithm (`BRepMesh_Delaun` over every node of the face). OCCT's code then recovers the boundary as before. This replaces ADR 0039 decision 3 item 4's last two sentences: such a face now gets triangles, and `tessellate` no longer fails with `Failed` for it.

## Options considered

- (i) The checks, and Watson for a face whose check fails (chosen).
- (ii) The same checks, with the face left without a triangulation and `tessellate` failing with `Failed`, as ADR 0039 decision 3 item 4 said.
- (iii) Exact predicates in the copy.

The owner's rule takes (i), the safest. The part displays and exports, and every other face keeps its triangles. Option (ii) turns the crash into a body that does not display. Option (iii) changes the triangles of every cocircular face and with them the examples' expected results. Every option is open source, and the first criterion picks none of them.

## Consequences

- Kernel.md test 172 covers the face: a 20 mm square about the origin on `Plane::XY` with corners rounded to 3 mm tessellates at `FINE` and returns `Ok`, with counterclockwise triangles whose areas sum to the face's area within the chord deviation.
- Wherever the copy without these checks succeeds, the checked copy gives the same triangles: 1,287 generated circles and round-cornered rectangles, compared triangle by triangle. The examples runner passes with the expected results unchanged.
- Watson inserts nodes one at a time and is slower on a large face (ADR 0039's 11 to 12 s on the engine's sphere). It runs only on a face whose check failed.
- The checks run in the copy's double-double arithmetic. The same nodes therefore fail them on every platform, and the same faces fall back. Watson's triangles on such a face may differ between platforms, since OCCT computes them in plain doubles. Test 172 checks the face's area and orientation, not its triangles. No example face falls back on Linux, and the Windows and macOS rows of `rust-platforms` run the examples with this change for the first time.
- Kernel.md 3.7 item 2 and 3.9 item 7, ADR 0039 decision 3 items 4 and 7, and the crate's `AGENTS.md` point here.
