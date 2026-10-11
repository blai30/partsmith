# Coding guidelines

Project-wide style guides that every change follows, in addition to the code style section of the root `CLAUDE.md`.

- [TypeScript](typescript.md)
- [Tailwind CSS](tailwind-css.md)
- [Frontend design](frontend-design.md)

## Partsmith design direction

Applying [frontend-design.md](frontend-design.md) to a CAD tool used for hours at a time:

- **Tone:** a precision instrument. Graphite and zinc surfaces, hairline borders, generous spacing, restrained motion. The model in the viewport is the hero; chrome recedes.
- **Accent:** one signal orange for selection, focus and primary actions. It stays distinct from sketch blue (under-constrained geometry) and red (conflicts and errors).
- **Type:** Inter for interface text; JetBrains Mono for numbers, dimensions, expressions and code.
- **Themes:** light and dark from the same role-to-color mapping, toggled with the `dark` class on the root element (`@custom-variant dark` in `ui/src/index.css`). The mapping from roles (surface, text, accent, sketch states, analysis colors) to Tailwind palette steps is documented in `docs/spec/ui.md` and single-sourced in code, which [tailwind-css.md](tailwind-css.md) allows for documented design-system mappings.
- **Stylesheet:** `ui/src/index.css` holds only the font import, the Tailwind import and the dark variant. Everything else is Tailwind utility classes. Positions computed from 3D geometry at runtime (floating inspector, dimension labels) are applied imperatively through refs, never as static styles.
