# 0030: Floating islands over a full-window viewport, and X, Y, Z in red, green and blue

Status: Accepted (decided by the project owner, 2026-10-13); the visual values need human review at the next gate (ui.md open issue 13)

## Context

The UI shell docked its regions around the 3D view: the top bar, browser, side panel, timeline and status bar took grid tracks with hairline borders, and the canvas filled the cell they left (ui.md 3.2 before this decision). The owner asked for three changes on 2026-10-13 and chose among the options offered:

1. **Floating islands, in the style of shadcn/ui.** The 3D view fills the whole window, and the panels become rounded islands floating over it, with a gap from the window's edges and from each other, soft shadows and a subtle border, as in Figma UI3 or Plasticity. The palette stays: graphite and zinc surfaces, one signal orange, sketch blue, error red, light and dark themes. Fit to view, the view cube, the top-center slot and every overlay work with the area the islands leave free; framing centers the model in that area with an off-center projection, not a smaller canvas; nothing is picked through an island; collapsing a panel gives the model the space back.
2. **X, Y and Z in red, green and blue** wherever an axis or a coordinate is shown, from new roles in `ui/src/theme/roles.ts`, tuned for both themes, distinct from the accent and from error red, and readable with red-green color blindness through the letters. Coordinates stay right-handed with Z up (ADR 0029, which already names these colors).
3. **New interface and display fonts**, chosen by a separate study. Out of scope here: JetBrains Mono stays, the font loading of ADR 0013 is unchanged, and the island and type rules name the two font roles (`FONT_CLASSES.interfaceText` and `FONT_CLASSES.values`), never a family, so a later font change needs no layout edit.

## Decision

Islands (ui.md 3.2, 3.14, 3.23):

- The canvas and its overlay layer fill the window, over an app root in the new `stage` role (the viewport's top step), so the gutter never reads as one surface with the islands. The top bar, browser, side panel, timeline and status bar are frame islands: `rounded-xl` (the slim status bar and the collapsed timeline `rounded-lg`), a `border` outline, `shadow-md`, the `surface` role, 0.5rem from the window's edges and from the free area. Multi-line cards (inspector, notifications, viewport controls, legend, empty-state card, core panels) are `rounded-xl` and one-line chips `rounded-lg`, all `surfaceRaised`; menus, popovers and tooltips are `rounded-lg`; controls are `rounded-md`.
- New roles carry the depth: `border` becomes the outline of islands, cards and popups, a step stronger than before (`zinc-300` light, `zinc-700` dark), because the old `zinc-800` hairline equals the dark viewport's top and the light bottom islands sit on the same `zinc-100` as the viewport's bottom; the old hairline lives on as `divider`, inside islands only; `elevation` colors every shadow, black at 50 percent in the dark theme, where Tailwind's 10 percent cannot be seen.
- The free area is the grid cell the islands leave, and it is the `viewport` element. The camera's window is the free area: fit, the standard views, zoom, pan and the grid scale are computed for it, and the projection is shifted and scaled so the rest of the canvas shows the same view around it. The model therefore shows through the gaps between islands and slides under them when panned. Opening or closing a docked island moves the model to the new free area's center, and a camera still as the last fit left it frames the same bounds again, so a fitted model stays whole.
- The inspector is an island too, but a floating one: it does not change the free area, while Fit to view, Home, Look at and the Show buttons frame into the part of the free area beside it, and opening, moving or closing it moves no camera.
- Islands sit above the canvas, so no pointer event over an island reaches it, and overlay elements anchored outside the free area are hidden.
- The view cube stays in the free area's top-right corner, moved to `top-11 right-9` so its turned corners keep 0.7 to 1.2rem from the free area's edges at every UI scale, and the viewport controls move to its top-left corner: under the cube they do not fit a free area 11.75rem tall (200 percent UI scale on a 1280 by 720 window). The top-center slot keeps clear of both at every width, which resolves ui.md open issue 16.
- The browser and side panel dock while the app root's content box is at least 64rem wide, a container query in rem, so the UI scale counts; the root's 0.5rem gutter makes that a window of at least 65rem (1040 CSS pixels at 100 percent), and the code that closes overlays and the startup rule use the same 65rem. Below it they float over the free area as overlays, as narrow windows did before.

Axes (ui.md 2.16, 3.7, 3.8, 3.10, 3.12, 3.14, 3.16.2):

- New interface and viewport roles `axisX` (`rose-600` light, `rose-400` dark), `axisY` (`emerald-700`, `lime-400`) and `axisZ` (`sky-700`, `sky-300`). Their hues are their own: none of them is the accent, error red, success green or sketch blue. Their text contrast is at least 4.52:1 on fields and raised surfaces, and their non-text contrast at least 3.56:1 on every surface they mark.
- They color the view cube's edges by the axis each runs along, the origin axes and their labels, the origin planes (each in its normal's color), the sketch's own axes on origin planes and their labels, the browser's axis icons, the X, Y and Z letters of coordinate fields (Move body, the primitives' Position) and the coordinate readouts of the selection inspector (Size, Center, a vertex's position).
- An axis color always comes with its letter, except on the origin planes (named by their plane labels) and the view cube's edges (named by its face labels), so color is never the only signal and red-green color blindness loses nothing; hovered and selected axes and plane outlines are also drawn thicker.
- There is no separate axis triad: the cube's colored edges serve as one.

Options not chosen:

- A canvas shrunk to the free area: the model would be cut off at the islands' edges, and the islands would float over empty chrome rather than over the model.
- Translucent islands with a backdrop blur: blurring the WebGL canvas behind every island costs a full-screen pass per frame, and text contrast would depend on the model behind it.
- The plain `red-600`, `green-600` and `blue-600` steps: `red-600` lies 0.079 (OKLab) from error red, `blue-600` is sketch blue itself, and `green-600` reaches only 3.21:1 on white, too little for letters.
- `lime-700` for the light Y and `sky-400` for the dark Z, the first choice: under protanopia `lime-700` lies 0.014 from the accent and under deuteranopia 0.010 from error red, so a hovered Y axis or a Y letter beside an error would not stand out; `sky-400` lies 0.073 from the dark sketch blue, close to dashed under-constrained construction lines on a vertical sketch.
- Coloring the browser's plane icons by their normal: the name beside the icon ("Top plane (XY)") carries the letters of the plane's own axes, so a Z-blue icon would contradict its letters.
- A separate border role for the frame islands only: one stronger `border` for every outline, with the old hairline kept as `divider`, needs one role less and lifts cards over the dark viewport as well.
- Viewport controls at the bottom center, as in Fusion: the fit rule puts the bottom edge of a framed model exactly there, so the controls would cover it.
- Colored grid lines through the origin: not asked for. The origin axes carry the colors whenever they are shown.

## Consequences

- Every visual baseline changes (V-1 to V-12, none approved yet), and V-13 to V-20 are new; all need a reviewer's approval at the gate.
- The canvas draws the whole window: at 1280 by 720 with the browser open that is about 1.5 times the pixels of the docked canvas, the extra mostly under opaque islands (ui.md open issue 31). The frame budget P-6 still applies.
- End-to-end positions are measured in the free area (`canvas(page)` returns the `viewport` box). Tests that state a size or a center compare the readout's `data-value`, since the visible text now carries the axis letters.
- Red, green and blue gain a second, documented meaning in the interface: the axis hues mark axes only, with a letter except on the origin planes and the view cube (ui.md 2.16.3).
- Every pixel mapping between the canvas and the scene (the scene's frame, screen rectangles, the sketch session's pixel mappings) goes through the free area's view window, and canvas-pixel layers (the overlay, the sketch layer, the viewport menu's anchor) sit outside the free-area layer.
