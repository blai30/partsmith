# W2-2A-2: Knot points and pole weights

Status: Accepted (owner's rule of 2026-11-05): the M2 plan's open choice "Knot and weight constraints", decided as the wave 2 decisions of 2026-11-19 accept it ([m2-wave2-decisions.md](../design/m2-wave2-decisions.md) section 3). It settles what [ADR 0066](0066-sketch-entities-of-format-version-3-operations-and-display-polylines.md) alternative 4 and sketch.md section 3.20.3 left to item 15. It changes the file format through the records `knot_point` and `weight` of [ADR W2-2A-1](W2-2A-1-constraint-records-of-the-remaining-sketch-constraints.md), and what degrees of freedom mean on a spline with a driving weight, which the ADR log records. Both go to human review at the M2 gate with format version 3 (m2-plan.md gate 19).

## Context

- **Stored constants.** A `bspline`'s `degree`, `knots` and `weights` are constants of the solve (sketch.md section 3.20.3). ADR 0066 alternative 4 kept them constants because as parameters they would leave every spline under-constrained, and the degrees of freedom the UI shows would mean nothing.
- **Poles are point entities.** A `bspline` names its poles by id (ADR 0066 alternative 2). A constraint on a control point names a point entity, and inserting a knot shifts nothing a constraint names.
- **No index.** AGENTS.md forbids index-based references in documents. file-format.md test 73 finds no `index` key and no integer property but those it lists.
- **FreeCAD's model.** FreeCAD shows a B-spline's knots as points the user can constrain, and its poles as circles whose radius is the weight. A weight is then a dimension like a radius.

## Decision

1. **Knots stay constants.** `knot_point` holds a point on a `bspline` or `fixed_spline` at one knot value: two rows, the point minus the curve's point at that value (sketch.md section 3.22.3). The knot is named by its value, a number that must equal one of the spline's knot values (`UnknownKnot` otherwise). A knot inserted elsewhere leaves it as it was. A knot removed makes the constraint invalid, with its own message and the fix to delete it.
2. **A driving weight makes one weight a parameter.** A driving `weight` names a pole of a `bspline` by its point entity id. That pole's weight becomes a solver parameter of the spline, and the constraint adds one row that sets it to the value. The parameter and the row cancel in the degrees of freedom: a spline's count means what it meant in M1 and item 14. Substitution freezes the parameter at the value (sketch.md section 3.7.3), and the solved weight is the value bitwise. The weight can follow an expression, as every dimension does. Other weights stay constants. A reference `weight` adds no parameter and no row, and reports the pole's weight.
3. **Output.** A spline whose weight parameter changed is returned with `weights` holding every pole's weight: the stored ones, 1 for each when `weights` was absent, and the solved value in each named pole's place. A build never stores them. A drag commit stores them with the rest of the solved geometry (engine.md section 3.6).
4. **Not on every kind.** A `fixed_spline` takes `knot_point` but no `weight`: its poles are coordinates, which no constraint can name. A `fit_spline` takes neither: its knots and poles are derived on every build (sketch.md section 3.20.2).

## Alternatives considered

Each choice is Accepted (owner's rule of 2026-11-05): the most open-source approach, then the safest, then the industry standard, then this work's recommendation. Every option is in-house Rust and plain JSON under Apache-2.0: the first criterion ties.

1. *Weights.* (i) Constants with no constraint, edited as record data; (ii) a parameter only for the pole a driving weight names; (iii) every weight of a spline a parameter. The rule takes (ii), the safest. (iii) leaves every spline under-constrained, which ADR 0066 alternative 4 refused. (i) makes a weight a number the user cannot tie to a parameter and that a conflict cannot report. (ii) is also FreeCAD's model, where a weight is a dimension on one pole.
2. *Knots.* (i) Constants, with a knot point constraint; (ii) parameters with a dimension. The rule takes (i), the safest. A knot parameter would change the spline's basis functions and their derivatives in every row that evaluates it, a dense Jacobian the exact step relies on (the same reason [ADR 0077](0077-solver-rows-of-conics-and-splines.md) alternative 4 gives for fit splines). FreeCAD keeps knots fixed as well and constrains the points at them.
3. *How a knot is named.* (i) By its value; (ii) by its position among the knots; (iii) by the poles next to it. The rule takes (i), the safest, for the reasons [ADR W2-2A-1](W2-2A-1-constraint-records-of-the-remaining-sketch-constraints.md) alternative 4 gives.
4. *How a pole is named.* (i) By its point entity id; (ii) by its position in `poles`. The rule takes (i), the safest: the invariant forbids (ii), and (i) is how every other constraint names a control point.
5. *A reference weight.* (i) No parameter and no row; (ii) a free parameter that other constraints may set. The rule takes (i), the safest: a reference dimension adds no rows and removes no degrees of freedom (sketch.md section 3.5). Under (ii) a reference dimension would add a degree of freedom.

## Consequences

- Specs: sketch.md sections 2.5, 3.4, 3.6.1, 3.7.3, 3.7.6, 3.8, 3.20.3, 3.20.4 and 3.22, and tests 450 to 469 (the cut group of m2-plan.md risk 11).
- A spline's parameters are no longer always none: a `bspline` with a driving weight has the weight parameters of the poles it names (sketch.md section 3.20.3). Its entity state looks at them and at its poles.
- A later knot insertion or removal (item 25's modify tools) keeps a `knot_point` while its knot value is still a knot of the spline. Item 25's spec states what happens to one whose knot goes.
- Needs human review at the M2 gate: the `knot_point` and `weight` records, with format version 3, and the meaning of degrees of freedom on a spline with a driving weight.
