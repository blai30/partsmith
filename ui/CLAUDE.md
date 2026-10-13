# ui

React 19 + TypeScript app for the web and the desktop front end. It talks to the core only through the protocol (`docs/spec/protocol.md`), with types generated into `src/generated/`.

Spec: `docs/spec/ui.md`. Guidelines: `docs/guidelines/` (TypeScript, Tailwind CSS, frontend design, and the Partsmith design direction).

## Commands

```sh
pnpm install
pnpm dev                 # Vite dev server
pnpm lint                # oxlint, then oxfmt --check
pnpm format              # oxfmt
pnpm typecheck
pnpm test                # Vitest
pnpm e2e                 # Playwright against the production build and the core in public/core (e2e/README.md); locally set PLAYWRIGHT_CHROMIUM_EXECUTABLE=/opt/pw-browsers/chromium
```

## Rules

- `type`, never `interface`. Strict TypeScript, no `any`.
- Styling is Tailwind utility classes only. Conditional classes use `clsx/lite`. No inline `style` attributes, no `px` values, arbitrary values only when unavoidable.
- `src/index.css` holds only the font import, the Tailwind import and the `dark` custom variant. Do not add CSS or CSS variables there without human approval.
- Light and dark themes come from the role-to-color mapping documented in `docs/spec/ui.md`; the `dark` class on the root element switches themes.
- Icons from `lucide-react`.
