# W2-1A-4: The conic and spline tools reach the core in wave 2, with previews from the core

Status: Accepted (owner's rule of 2026-11-05): the M2 plan's open choices "How item 14's drawing tools reach the core in wave 2" and "Live previews of conic and spline tools without curve evaluation in the UI", decided as the wave 2 decisions of 2026-11-19 accept them. It amends [ADR 0066](0066-sketch-entities-of-format-version-3-operations-and-display-polylines.md) decision 11. It changes the protocol schema earlier than ADR 0066 planned: command 39 `sketch_recipe`, the `SketchRecipe` type and the error code `sketch_operation_failed` land with item 14 in wave 2 instead of item 24 in wave 3. That change is additive (`PROTOCOL_VERSION` stays 1, ADR 0024) and goes to human review at the M2 gate with ADR 0066 (m2-plan.md gate 19). It changes no file format.

## Context

- **ADR 0066 decision 11** gives `apply_recipe`, `SketchChange` and command 39 to item 24 (wave 3), and protocol.md command 39 says "item 24". sketch.md section 2.13 gives the variants of the conic and spline tools to item 14 (wave 2).
- **The M2 plan's choice 4** builds every drawing tool as a recipe of partsmith-sketch. M1's tools are TypeScript in `ui/src/sketch/recipes.ts`, which item 24 moves.
- **D15 as ADR 0066 amends it**: the UI draws conics, splines and text from the display polylines the core sends, and never evaluates them (ADR 0066 decision 8, ui.md section 3.16.2).
- **Gate 2** (m2-plan.md) wants an E-test for every new entity kind and tool in wave 2.

## Decision

1. **Item 14 lands the route.** `apply_recipe`, `SketchChange`, `SketchRecipe` with the seven variants of the conic and spline tools (sketch.md sections 2.13 and 3.21.1), command 39 and `sketch_operation_failed` land with item 14. Item 24 adds the variants of M1's tools and moves `recipes.ts` and Python's `Rectangle` and `Polygon` onto them in wave 3, as ADR 0066 planned. Commands 40 and 41 keep their items (25 and 26).
2. **Recipes add geometry only.** As in M1, the host adds the constraints of snaps and inference to the change before it stores it (ui.md section 3.16.5). The point entities a spline tool adds are construction points.
3. **Previews come from the core.** While a conic or spline tool is active, each pointer move sends `sketch_recipe` with the clicks so far and the cursor as the next click, on a `candidate` that holds the sketch as stored, then `solve_sketch` with the returned params as its `candidate`, both through the client's `LatestOnly` queue (ui.md, `createLatestOnly`). The UI draws the scaffold at once from the clicks (points, the control polygon, the axes) and the curve when the solve's polylines arrive. Nothing new reaches the protocol, and D15 holds.

## Alternatives considered

Each choice is Accepted (owner's rule of 2026-11-05): the most open-source approach, then the safest, then the industry standard, then this work's recommendation. Every option is in-house code under Apache-2.0: the first criterion ties.

1. *How the tools reach the core.* (i) Item 14 lands `apply_recipe`, command 39 and `sketch_operation_failed` with its own variants, and item 24 adds M1's in wave 3; (ii) the conic and spline tools wait for wave 3; (iii) the UI builds the records in TypeScript, as `recipes.ts` does. The rule takes (i), the safest. (iii) goes against choice 4 and would need a second B-spline and conic implementation in TypeScript, which ADR 0066 decision 8 rules out. (ii) moves gate 2's conic and spline E-tests out of wave 2. (i) keeps one implementation for the UI and Python.
2. *Previews.* (i) `sketch_recipe` and then `solve_sketch` on a candidate through `LatestOnly` on each pointer move; (ii) a preview option on command 39 that returns polylines with the change; (iii) the scaffold only, with the curve drawn after the click; (iv) UI evaluation of conics and splines for previews only, amending D15. The rule takes (i), the safest: no protocol change and no change to D15. (ii) adds a protocol member for display data. (iv) puts a second evaluator in the UI. (iii) is what (i) shows while an answer is on its way. The frame cost of (i) is measured as a sketch.md section 5.1 row.
3. *Whether a recipe adds snap constraints.* (i) The host adds them, as in M1; (ii) the recipe takes snap targets and adds them. The rule takes (i), the industry standard of this code base and the recommendation: ui.md section 3.16.5 already owns snaps and inference, and item 24 keeps that split for M1's tools.

## Consequences

- Specs: sketch.md sections 2.13, 3.21 and 3.21.1, section 4.1's `InvalidToolInput` and tests 332 to 334 and 341 (relabeled to item 14); protocol.md command 39, section 4's `sketch_operation_failed`, tests 34, 37 and 118 to 120 (relabeled to item 14) and its examples. engine.md section 3.27 and tests 228, 229 and 236 move to item 14 in their owner's stage (track 2A). ui.md gets the tools and previews from track 2C.
- Gate 19 reviews command 39 and the error code with ADR 0066, now in wave 2.
- ADR 0066 decision 11 reads, after this ADR: item 14 builds the curve kinds, their roles, profiles, chains, display polylines, the `curve_*` buffers, `apply_recipe` with its own variants, command 39 and `sketch_operation_failed`; item 24 adds the recipes of M1's tools.
