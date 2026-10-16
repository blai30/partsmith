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
pnpm test:scripts        # node:test tests of scripts/ and of the workflow files
pnpm check:licenses      # the npm packages that ship against the allow list of deny.toml (docs/spec/delivery.md 2.10)
pnpm notices             # rewrite both third-party notices files after changing a lock file, a Cargo.toml, rust-toolchain.toml or ../scripts/*-version.sh (delivery.md 2.9)
pnpm check:notices       # both notices files against what the lock files give, as CI's schema job runs it
pnpm build && pnpm check:dist   # the build in dist/ against the Pages file limit, the headers, the notices and the initial JavaScript limit (delivery.md 2.6)
pnpm e2e                 # Playwright against the production build and the core in public/core (e2e/README.md); locally set PLAYWRIGHT_CHROMIUM_EXECUTABLE=/opt/pw-browsers/chromium
```

## Rules

- `type`, never `interface`. Strict TypeScript, no `any`.
- Styling is Tailwind utility classes only. Conditional classes use `clsx/lite`. No inline `style` attributes, no `px` values, arbitrary values only when unavoidable.
- `src/index.css` holds only the Tailwind import, the two `@font-face` rules of the self-hosted fonts, the font theme variables and the `dark` custom variant (`docs/spec/ui.md` 2.16.1, ADR 0031). Do not add CSS or CSS variables there without human approval. Interface text and numbers inherit Geist from the root; use `tabular-nums` where digits line up and `font-mono` for code only (ui.md 2.16.7), never a font family or `font-feature-settings`.
- Light and dark themes come from the role-to-color mapping documented in `docs/spec/ui.md`; the `dark` class on the root element switches themes.
- Icons from `lucide-react`.
- Tests build on the shared fakes in `src/test/fakes` (core, app, state, selection items, viewport, deferred promises, a `ResizeObserver` stand-in); `src/test/type-roles.ts` tells tabular figures and code apart in jsdom, which applies no stylesheet (ui.md 2.16.7). `src/test/setup.ts` widens jsdom's window to 1280 CSS pixels (wide enough for docked panels, ui.md 3.2), gives elements the no-op `scrollIntoView` that jsdom lacks (tests spy on it) and unmounts Testing Library renders after each test, after the file's own `afterEach` hooks, so a file calls `cleanup()` itself only when its teardown must follow the unmount; `beforeEach(installPopoverPolyfill)` installs and removes the popover stand-in.
