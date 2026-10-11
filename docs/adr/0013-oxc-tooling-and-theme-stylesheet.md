# 0013: oxlint and oxfmt for the UI, and a minimal theme stylesheet

Status: Accepted (human decision)

## Context

The UI scaffold started with ESLint, typescript-eslint and Prettier. The maintainer asked to use the oxc tools instead. The Tailwind guideline (`docs/guidelines/tailwind-css.md`) also asks for approval before any stylesheet or CSS variable change, and the UI needs fonts and a manually toggled dark theme.

## Decision

- `ui/` lints with oxlint (plugins: typescript, unicorn, oxc, react, import, jsx-a11y, vitest; correctness and suspicious as errors, perf as warnings; React hooks rules on; `type` over `interface` enforced) and formats with oxfmt. `pnpm lint` runs `oxlint --deny-warnings` then `oxfmt --check`. ESLint, typescript-eslint and Prettier are removed.
- `ui/src/index.css` contains exactly the maintainer-approved block: the Google Fonts import for Inter and JetBrains Mono, the Tailwind import, and `@custom-variant dark (&:where(.dark, .dark *));`. No other CSS and no CSS variables.
- Visual direction: graphite surfaces with one signal-orange accent, Inter for interface text, JetBrains Mono for values (`docs/guidelines/README.md`).

## Consequences

Linting and formatting run in well under a second. Type-aware lint rules from typescript-eslint are gone; `tsc -b` remains the type gate. Fonts load from Google Fonts, which sends `Cross-Origin-Resource-Policy: cross-origin`, so they work under the app's `Cross-Origin-Embedder-Policy: require-corp`; offline desktop use falls back to system fonts.
