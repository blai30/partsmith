# Tailwind CSS Standards

## Rules

1. **Tailwind utility classes only.** No custom CSS, no inline `style` attribute.
2. **Leave CSS files alone.** Don't edit the global stylesheet (e.g. `src/index.css`) or add CSS variables unless the user asks; if a change seems to need one, ask first.
3. **No `px` values.** Use Tailwind's predefined `rem`-based scale.
4. **Arbitrary values (`[...]`) only when unavoidable, never `px`.**

Post-change checklist: the diff touches no CSS file; no `style=` or `px]`/`[#` in `src/`.

## Component authoring

Inline classes directly on elements; don't assign static class strings to constants. Use `clsx` for conditional classNames (`clsx(base, cond && 'x', className)`). Import from **`clsx/lite`** (string args only); use full `clsx` only if you need object/array syntax. Don't hand-build classNames with template literals or `+`.

## Repetition is expected, not a smell

Repeated class sets across components is correct: keeping classes inline and local is the goal of utility-first styling. Do NOT extract shared constants, helpers, or token-like abstractions to deduplicate ordinary repetition. A project's pre-existing sanctioned fragments (e.g. focus-ring in `lib/styles.ts`) are fine; don't hunt for more.

**Exception: documented design-system mappings.** When repeated classes encode a documented role-to-value rule (a AGENTS.md, design doc, tokens table mapping semantic roles to specific scale steps), single-sourcing keeps the spec authoritative. This covers the **mapping** (role -> hue/step as named facets like `fill`/`fg`/`border`), NOT surrounding layout. The mapping must be documented; undocumented recipes are still ordinary repetition: leave inline.

## Tailwind v4 Token Shorthand

Prefer token shorthand: `bg-(--accent)`, `text-(--text-muted)`. Avoid bracket form: `bg-[color:var(--accent)]`.

## Quality Bar

Ensure mobile layouts aren't broken. Improve accessibility when reasonable. Outside design-led work, use animation only when requested or clearly justified. Visual direction, motion included, follows [frontend-design.md](frontend-design.md).
