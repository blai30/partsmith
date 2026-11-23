# W2-1A-5: Axis roles of ellipses and hyperbolas, and which new roles are hole points

Status: Accepted (owner's rule of 2026-11-05): the M2 plan's open choices "Handles and dimensions for the axes of ellipses and hyperbolas" and "Which new roles count as hole points and sketch_point picks", decided as the wave 2 decisions of 2026-11-19 accept them. It amends [ADR 0066](0066-sketch-entities-of-format-version-3-operations-and-display-polylines.md) decision 2 (the point roles). It changes the file format: `PointRole` gains `major` and `minor` in format version 3 ([ADR 0061](0061-format-version-3-for-the-records-of-m2.md)), and a hyperbola takes the existing role `vertex`. It changes the protocol schema: the `PointRef` pattern and the roles in `SketchSolution::points`. Both are additive and go to human review at the M2 gate with format version 3 (m2-plan.md gate 19), with ADR 0066 decision 2. The naming review sees the hole rule (decision 3), because hole face names embed the point reference text.

## Context

- **ADR 0066 decision 2** gives a parabola `vertex` and `focus` and a conic `control`. An ellipse has only `center`, and a hyperbola `start`, `end` and `center` (sketch.md section 3.20.1).
- **Axes.** An ellipse's `major` vector and `minor_radius` and a hyperbola's are solver parameters (sketch.md section 3.20.3). With only a center, a user can neither drag nor dimension an axis. Format version 3 freezes at the gate: a role added after it needs version 4.
- **Mirroring.** "Turn the drawing back" mirrors a sketch, and every constraint must hold on the mirror as it did (sketch.md section 3.17). A mirror turns a right-handed frame into a left-handed one.
- **Hole points.** features/hole.md places holes at sketch points: ends and centers in M1. Hole face names embed the point reference text.

## Decision

1. **`major` and `minor` on ellipses and elliptical arcs.** `.major` is the end of the major semi-axis, `center + major`. `.minor` is the end of the minor semi-axis on the left of `major`, `center + minor_radius * n` with `n` the unit of `major` turned counterclockwise by a quarter turn. Both follow `control` in `PointRole`'s order. Every point constraint and distance takes them. The axes are dimensioned and dragged with M1's constraints. They are derived points, not solver parameter pairs (sketch.md section 3.6.1). The solver takes `.minor` as `center + abs(b) * n` for its minor parameter `b`, the point the output's `minor_radius = abs(b)` gives back. So a constraint on `.minor` holds on the output whatever sign `b` reached, and a drag of `.minor` sets the minor radius to the cursor's distance from the major axis line (sketch.md section 3.20.3, test 343).
2. **A hyperbola's `vertex`.** A hyperbola takes `.vertex`, the vertex of its branch, `center + major`, the role a parabola's vertex has. It takes no `minor` role.
3. **Hole points and `sketch_point` picks.** The centers of ellipses and elliptical arcs and the ends (`start`, `end`) of every open curve kind count, as ends and centers do in M1. A vertex, a focus, a control point, `major` and `minor` do not. features/hole.md and model.md state it (track 3A).

## Alternatives considered

Each choice is Accepted (owner's rule of 2026-11-05): the most open-source approach, then the safest, then the industry standard, then this work's recommendation. Every option is plain JSON under Apache-2.0: the first criterion ties.

1. *Axis handles.* (a) New point roles at the semi-axis ends; (b) construction axis lines tied by new internal-alignment constraints, as FreeCAD does; (c) an axis key on `radius` and `distance`; (d) no axis handles in M2. The rule takes (a), the safest. (d) leaves M2's axes undimensioned and undraggable, and roles added after the gate need format version 4. (a) reuses the `PointRef` pattern that ADR 0066 extended: every point constraint and distance works with no new constraint type. (b) adds constraint types. FreeCAD and Fusion both expose axis geometry to constraints.
2. *The hyperbola's minor role.* (i) `vertex` only; (ii) `major` and `minor` as on an ellipse. The rule takes (i), the safest. A mirror maps the point on the left of the major axis to the right of it. An ellipse keeps both roles on its mirror by swapping its axes (its `minor_radius` may exceed `norm(major)`, sketch.md section 3.20.1), and `.major` and `.minor` trade places. A hyperbola's axes are never swapped (its branch opens along `major`). A constraint on its `minor` could not hold on the mirror. Its vertex lies on the curve and keeps its place. The minor radius is set by typed values, the inspector, or a point on the curve with a distance.
3. *Hole points.* (a) Every new role; (b) M1's list only; (c) the centers of ellipses and elliptical arcs and the ends of every open kind. The rule takes (c), the recommendation. It extends M1's rule, ends and centers, to points a user places on purpose. A focus, a control point or an axis end is construction scaffolding, as a spline's control points are construction points.

## Consequences

- Specs: sketch.md sections 2.4, 3.2, 3.3, 3.4, 3.6.1, 3.17, 3.20.1, 3.20.3 and 3.20.11, tests from 342; protocol.md section 2.6 and test 131. features/hole.md and model.md take decision 3 in stage 3 (track 3A). file-format.md section 3.9's version 3 row lists `major` and `minor` beside `vertex`, `focus` and `control` (the stage-close integrator).
- ADR 0066 decision 2 reads, after this ADR: `vertex` and `focus` (a parabola's, and `vertex` a hyperbola's), `control` (a conic's), then `major` and `minor` (an ellipse's and an elliptical arc's) follow `center` in `PointRole`.
- Gate 19 reviews the roles with format version 3, ADR 0066 decision 2 with them, and decision 3 in the naming review.
