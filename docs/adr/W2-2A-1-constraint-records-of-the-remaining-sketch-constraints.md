# W2-2A-1: Constraint records of the remaining sketch constraints

Status: Accepted (owner's rule of 2026-11-05): the M2 plan's open choice "Item 15's records: new types, or new forms of M1 types", decided as the wave 2 decisions of 2026-11-19 accept it ([m2-wave2-decisions.md](../design/m2-wave2-decisions.md) section 3). It changes the file format: eight constraint record types join format version 3 ([ADR 0061](0061-format-version-3-for-the-records-of-m2.md)). It changes the protocol schema: the types reach it through `SketchParams` (protocol.md section 1). Both go to human review at the M2 gate with format version 3 (m2-plan.md gate 19). `PROTOCOL_VERSION` stays 1 ([ADR 0024](0024-diagnostics-with-actions-causes-and-ghosts.md)'s additive rule).

## Context

- **Item 15's list.** m2-plan.md item 15 and technical design section 7's constraint table: smooth (G2), B-spline knot and weight, arc length, angle of a line to an axis, block, symmetric curves, the midpoint of an arc, Snell's law, and the point-circle, circle-circle and line-line distances. The explicit tangent-at-endpoint mode and fix with stored coordinates wait (M3, sketch.md section 1.2).
- **M1's records.** `ConstraintKind` in `crates/partsmith-sketch/src/constraint.rs` has 19 variants, each tagged `type`. Five of them have several untagged forms: `distance` through `DistanceTarget`, and `horizontal`, `vertical`, `horizontal_distance` and `vertical_distance` through `Alignment`. sketch.md test 15 pins the 19 names in the schema.
- **The format's growth rule.** file-format.md section 3.9 rule 2 allows three kinds of additions to version 3: an optional key that is omitted when absent, a new record type, and a new value of a tagged enum. A new untagged form with new required keys on an existing type is none of them.
- **Message words.** A constraint's `{Type}` is its `type_name` with underscores replaced by spaces (sketch.md section 4). `snells_law` would read "Snells law".
- **The cut order.** m2-plan.md risk 11 cuts Snell's law, block, and knot and weight fourth. Their records, tests and examples must leave with a cut ADR without weakening a test.
- **What M1 already has.** `angle` takes `x_axis` and `y_axis` (sketch.md section 3.16), and a whole-entity `fix` is what FreeCAD calls Block. [ADR W2-2A-4](W2-2A-4-block-angle-to-an-axis-and-line-distances-need-no-new-type.md) covers the constraints that need no record.

## Decision

1. **Eight new types**, after `angle` in `ConstraintKind` and in this order (sketch.md section 2.5):
   - `smooth` with `curves: [K1, K2]`: two curves meet at an end of each with one tangent and one curvature;
   - `arc_length` with `curve` and a dimension: the length of a circular arc;
   - `circle_distance` with a dimension, in two forms: `point` and `curve` (a point to a circle or arc), and `curves` (between two circles or arcs);
   - `symmetric_curves` with `curves: [K1, K2]` and `line`: two lines, two circles or two arcs are mirror images about a line;
   - `arc_midpoint` with `point` and `curve`: a point is the midpoint of an arc;
   - `refraction` with `points: [P1, P2]`, `curve` and a dimension: Snell's law at a boundary, FreeCAD's form (the end of the incoming ray, the start of the outgoing one, the boundary, and the value n2 / n1);
   - `knot_point` with `point`, `curve` and `knot`: a point lies on a spline at one of its knot values;
   - `weight` with `pole`, `curve` and a dimension: the weight of one pole of a `bspline`.
2. **Names are words.** Each `type_name` reads as the user's word in messages: `Smooth`, `Arc length`, `Circle distance`, `Symmetric curves`, `Arc midpoint`, `Refraction`, `Knot point`, `Weight`. Snell's law is `refraction`.
3. **Field names follow M1.** `curve` and `curves` hold curve ids, `point` and `points` point references, `line` a line, as M1's records do. `pole` holds a point entity id. `knot` is a number, not an index. `expr` and `driving` close every dimension.
4. **One UI tool may write several types.** The dimension tool writes `arc_length` and `circle_distance`, and the constraint tools the others (ui.md, track 3B). A record's type never depends on which tool wrote it.
5. **The cut group.** `refraction`, `knot_point` and `weight` have their own numbered tests (sketch.md tests 450 to 469), their own example (features/sketch.md test 66) and their own property case. A cut ADR of m2-plan.md risk 11 moves those and removes the three types from the schema. It also takes the places sketch.md section 6.27 lists outside the group, as section 6.26 lists those of item 14's cut: test 15's three names, `ValueKind::Scalar` with its words and arms, and protocol.md test 132's description of a measured plain number.

## Alternatives considered

Each choice is Accepted (owner's rule of 2026-11-05): the most open-source approach, then the safest, then the industry standard, then this work's recommendation. Every option is plain JSON in a format under Apache-2.0: the first criterion ties.

1. *Types or forms.* (a) New untagged forms on M1 types: `distance` with `point` and `curve` or with `curves`, `symmetric` with `curves`, `midpoint` with an arc in a `curve` field; (b) new types; (c) wider kinds in M1's fields, such as an arc in `midpoint.line`. The rule takes (b), the safest. Only (b) is within the letter of file-format.md section 3.9 rule 2. (a) adds a form with new required keys to a type a version 3 build already writes, which rule 2 leaves for version 4. (c) gives a field a name that misleads (an arc in a field called `line`) and changes what an M1 record may hold. Safety settles the choice before the industry standard, which would not decide it: FreeCAD's Sketcher applies one Distance constraint to two points, to a point and a line, and to a line's length, as M1's `distance` does, a pattern closer to (a).
2. *The name of Snell's law.* (i) `refraction`; (ii) `snells_law`. The rule takes (i), this work's recommendation: it renders "Refraction" in messages, where (ii) renders "Snells law". FreeCAD calls it "Refraction (Snell's law)" in its menu.
3. *The two circle distances.* (i) One type with two forms, as `distance` has; (ii) two types. Both are as safe. No industry standard decides. The rule takes (i), the recommendation: one dimension tool path, one value kind and one message word, as M1's `distance` does for its three forms.
4. *How a knot is named.* (i) By its value, a number of the record; (ii) by its position among the knots; (iii) by the poles next to it. The rule takes (i), the safest: AGENTS.md forbids index-based references in documents, file-format.md test 73 checks it, and a knot inserted before (ii)'s position would silently move the constraint. (iii) names a knot that a pole insertion can also move. [ADR W2-2A-2](W2-2A-2-knot-points-and-pole-weights.md) gives the rows.

## Consequences

- Specs: sketch.md sections 1.2, 1.3, 2.5, 2.8, 3.3 to 3.5, 3.10, 3.16, 3.17, 3.22 and 4, tests 410 to 469, open issues 30 and 31; features/sketch.md tests 61 to 66; protocol.md section 2.6 and tests 132 to 135; file-format.md tests 77 to 79. sketch.md tests 8 and 15 change spec first: test 8's unknown type `smooth` with two curves deserializes from item 15 on. Its case now has one curve, and an unknown `block` joins it. Test 15 counts 39 type names.
- file-format.md section 3.9's version 3 row lists the eight types and the forms of `circle_distance` (the stage 2 close writes it). `docs/format/ptsm.schema.json` and `docs/format/protocol.schema.json` regenerate when item 15's code lands. `knot` adds a `number` property, not an `integer`. The list of integer properties of file-format.md test 73 is unchanged.
- The engine's `export_python` match and the Python classes gain one arm and one class per type (python-api.md, track 3B, item 33). The UI's tools follow in track 3B. Runner test 6 (delivery.md section 2.8) asks for an example of each record form: features/sketch.md tests 61 to 64 and 66 hold them.
- Needs human review at the M2 gate: the eight record types, with format version 3 and the protocol additions (m2-plan.md gate 19).
