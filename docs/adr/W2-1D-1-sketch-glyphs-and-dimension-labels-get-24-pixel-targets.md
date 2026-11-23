# W2-1D-1: Sketch glyphs and dimension labels get 24-pixel targets

Status: Accepted (owner's rule of 2026-11-05): the M2 plan's item 16 and section 7.3 item 22, option (b), with the offsets of the choices below. It amends [ADR 0049](0049-web-content-security-policy-and-accessibility-checks.md) decision 3, a decision the owner made for M1 (option (a), the marks keep their sizes), so it goes to human review at the M2 gate (m2-plan.md gate 19, which names this ADR). No file format, protocol, naming, license, dependency or desktop capability change. It resolves ui.md open issue 42 for the glyphs and the labels. The rollback marker, the view cube's cells and the repair candidate markers keep ADR 0049's exceptions.

## Context

- **The marks of M1.** A constraint glyph is a `size-4.5` button (18 CSS pixels at UI scale 1). Its center sits 14 pixels off its curve (`GLYPH_OFFSET_PIXELS` in `ui/src/sketch/screen.ts`). Further glyphs on the same anchor stack 20 pixels apart along the curve (`GLYPH_STACK_PIXELS`). A glyph at a point sits up and to the right of it, 14 pixels along the diagonal. A dimension label is a `text-xs` chip 18 pixels high (ui.md 3.16.2). The used marks of ui.md 3.16.10 (`size-4`, `role="img"`, not pickable) follow the glyphs on their anchor, one stack step after the last. Read from the tree at 0f578c1.
- **What WCAG 2.2 asks.** Success criterion 2.5.8 (AA) asks 24 by 24 CSS pixels for a pointer target, or a smaller target whose 24-pixel circle meets no other target. Two stacked glyphs 20 pixels apart fail both. ui.md 1.1 claims AA, and 3.24 states the rule. ADR 0049 left the glyphs, the labels, the rollback marker, the view cube's cells and the repair candidate markers out of E-163.
- **The owner's word.** ADR 0049's status: the marks keep their sizes for M1, and this is revisited after M1. m2-plan.md item 16 and section 7.3 item 22 take option (b) for glyphs and labels by a new ADR before the overlay is laid out again. M2 adds about a dozen constraint glyphs (item 15).
- **Where the pointer picks a curve.** In the ID pass, sketch curves are drawn 7 CSS pixels wide and sketch points as 11-pixel squares, and a pick reaches 4 pixels further (the radius of ui.md 3.6; `SKETCH_CURVE_PICK_WIDTH`, `SKETCH_POINT_PICK_SIZE` and `PICK_RADIUS` in `ui/src/viewport/`). So the pointer picks a curve up to 7.5 pixels from its center line, and a point up to 4 pixels around its 11-pixel square. The overlay's buttons sit above the canvas: a 24-pixel button centered 14 pixels off a curve would cover 2 pixels on the far side of the line and take every pick in its 24 pixels.

## Decision

1. **Glyph buttons.** Each constraint glyph is a `size-6` button (24 CSS pixels, times the UI scale through rem) that holds the unchanged `size-4.5` chip, centered (ui.md 3.16.2). The button has no fill and no border of its own. The chip keeps its fill, border, icon and states. Hover, focus ring, click and selection belong to the button.
2. **Label buttons.** Each dimension label is a button of at least `min-h-6 min-w-6` that holds the unchanged `text-xs` chip, centered. A one-character label (`5`) is 24 pixels wide too.
3. **Offsets.** A glyph on a curve is centered 20 pixels (times the UI scale) off the curve, on the side ui.md 3.16.2 gives (`GLYPH_OFFSET_PIXELS` 20). The button's near edge is then 8 pixels from the curve's center line, outside the 7.5 pixels at which the pointer picks it. A glyph at a point is centered 30 pixels from it along the diagonal up and to the right (`POINT_GLYPH_OFFSET_PIXELS` 30, about 21.21 pixels right and 21.21 up). Its corner nearest the point is then about 5.25 pixels from the point's 11-pixel square, outside the 4-pixel pick radius.
4. **Stacking.** Further glyphs on the same anchor stack 24 pixels apart along the curve (`GLYPH_STACK_PIXELS` 24), so stacked buttons touch and never overlap. A used mark follows the last glyph one stack step later, as in M1.
5. **Tests.** The numbers of `screen.test.ts` (14 and 20 pixels) become 20, 30 and 24 by spec first (ui.md U-298). E-163 stops leaving out `constraint-glyph` and `sketch-dimension` and runs a sixteenth state on a new fixture with stacked glyphs (`glyph-stack.ptsm`, ui.md 6.2). It also checks that no two glyph buttons overlap. ui.md 3.24 lists only the rollback marker, the view cube's cells and the repair candidate markers as marks.

## Alternatives considered

For the decision (ADR 0049's labels for decision 3):

- (a) **Keep M1's marks.** No layout change. The glyphs and labels stay under WCAG 2.2's minimum, and stacked glyphs fail its spacing exception. The owner's M1 choice, revisited by m2-plan.md section 7.3 item 22.
- (b) **24-pixel buttons around the unchanged chips, stacked 24 pixels apart** (chosen). The standard is WCAG 2.2 AA, the level ui.md already claims. It is safest to lay the overlay out once, before item 15's glyphs arrive.
- (c) **A `w-6` rollback marker** around its unchanged line. It widens every gap of the timeline strip by 16 pixels. The marker keeps WCAG's equivalent exception ("Roll back to here" in each item's menu and the marker's keys), so the plan does not take it.
- (d) **Axe** as a development dependency (MPL-2.0). A new dependency for review that checks less of the tab order than E-162.

For the offset and the label width (plan choice "Glyph offset and label width"):

- **Keep 14 pixels.** The button would cover the curve's own pick band and 2 pixels past its line. A click on a line under a glyph would select the glyph.
- **At least 20 pixels on a curve and 30 on the diagonal at a point** (chosen). The smallest whole numbers whose buttons clear the pick areas of ui.md 3.6. Open source, safest, and how Fusion and FreeCAD keep constraint icons beside their curve, not on it.
- **Labels `min-h-6` only.** A label of one digit would stay 18 pixels wide and fail the rule.
- **Labels `min-h-6 min-w-6`** (chosen). Every label meets the rule, whatever its text.

## Consequences

- The overlay's layout changes for every sketch: glyphs move 6 pixels further from their curve, stacks are 4 pixels longer per glyph, and point glyphs move about 11 pixels further out. The visual baselines of sketch states (ui.md 6.6) are taken again, in the change that builds this ADR, with the owner's look at them (V-5, V-6 and V-17).
- Every test that reads a glyph's position or a stack step changes by spec first: `screen.test.ts`, `session.test.ts` (the glyph 14 pixels above its line) and U-298. E-tests that click a glyph find it by its test id, not by position, and do not change.
- A glyph can cover another curve that passes within 20 pixels of its anchor. That curve stays pickable outside the button. The M1 overlay had the same property at 14 pixels.
- Reverting is the three constants, two class lists, the E-163 exclusions and the visual baselines.
