# W2-2A-4: Block, angle to an axis and the distance between lines need no new type

Status: Accepted (owner's rule of 2026-11-05): the M2 plan's open choices "What block means" and "Angle of a line to an axis", and the line-line part of "Distances to circles and between lines, arc length scope, smooth and symmetric pairing", decided as the wave 2 decisions of 2026-11-19 accept them ([m2-wave2-decisions.md](../design/m2-wave2-decisions.md) section 3). It changes neither the file format nor the protocol schema, and no solver row. Nothing in it needs human review at the M2 gate.

## Context

- **Technical design section 7** lists block ("freeze a sub-sketch rigidly"), the angle of a line to an axis and the line-line distance among the constraints. Section 7's power tools, block among them, sit behind the command palette.
- **Fix.** `fix` with a bare id freezes a whole entity of any kind at its stored geometry (sketch.md sections 3.4 and 3.6.5). FreeCAD's Block constraint does the same: it fixes the entity it is put on. ui.md's keyword list already maps "Block (FreeCAD)" to `sketch.constrain_fix` (`ui/src/commands/keywords.ts`).
- **Angle to an axis.** `angle` takes `x_axis` and `y_axis` as either line (sketch.md section 3.16), measures a line's direction from +u modulo pi, and holds on the mirrored drawing. ui.md section 3.16 stores a typed line angle as `angle` against `x_axis`. python-api.md test 83 writes `Angle(Sketch.X_AXIS, l, 30 * deg)`.
- **Distance between lines.** A distance between two lines is defined only when they are parallel. ui.md's dimension tool already writes a point-line `distance` from the second line's start for two parallel lines (`ui/src/sketch/dimensions.ts`, U-59), and an `angle` for lines that are not.
- **Rigid moves.** Moving or turning a group of entities as one is a modify tool of item 25 (sketch.md section 1.2), not a constraint.

## Decision

1. **Block is a command, not a record.** The palette command Block (track 3B) adds one `fix` with a bare id for each selected entity, in selection order. Its keyword stays on `sketch.constrain_fix`'s list. A block of many entities is many `fix` records, each deleted on its own.
2. **Angle to an axis is `angle` with `x_axis` or `y_axis`**, as M1 has it. The dimension tool gains a path that picks an axis as the second line (track 3B).
3. **The distance between two parallel lines is the point-line `distance`** from the second line's start to the first, as the dimension tool writes it, with the lines held parallel by their own constraint. Lines that are not parallel take an `angle`.
4. **Words and titles.** With no type for block, no title lists more than three entities, and section 4's title rules of sketch.md stay as M1 wrote them.

## Alternatives considered

Each choice is Accepted (owner's rule of 2026-11-05): the most open-source approach, then the safest, then the industry standard, then this work's recommendation. Every option is in-house Rust and plain JSON under Apache-2.0: the first criterion ties.

1. *Block.* (a) M1's whole-entity `fix` over a selection; (b) a rigid group with 3 degrees of freedom left, by distance rows or a rigid-transform substitution; (c) entities taken out of the solve. The rule takes (a), the safest: no solver or substitution change right after ADR 0058's exact step, and every M1 result stays as it is (m2-plan.md gate 5). It is also the industry standard: FreeCAD's Block fixes the geometry it is put on. (b) adds parameters or rows that every component, diagnosis and drag would have to handle. (c) is `fix` under another name.
2. *Angle to an axis.* (a) M1's `angle` with an axis; (b) a new type or a one-line form of `angle`. The rule takes (a), the safest: specified, tested, mirrored and exported to Python since M1. (b) would add a record that says what an existing one says.
3. *Line-line distance.* (a) M1's point-line distance with the lines parallel; (b) a new form that also makes the lines parallel; (c) a new type that needs a separate parallel. The rule takes (a), the safest: the dimension tool already writes it, it adds no row, and a user who removes the parallel sees the dimension stay a point-line distance. (b) puts two relations in one record the user cannot split. (c) is (a) with another name.

## Consequences

- Specs: sketch.md sections 1.2, 1.3 and 3.22.8. ui.md (track 3B) gives the Block palette command, the axis path of the dimension tool and their E-tests, which m2-plan.md gate 2 asks for each constraint of technical design section 7.
- Python (track 3B, item 33): `Fix` and `Angle` already cover block and angle to an axis. Gate 2's twins of these two are those of the examples features/sketch.md tests 49 and 64 name.
- Item 25's move, rotate and copy tools are the way to move a group of entities as one.
