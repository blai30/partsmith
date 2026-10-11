# 0002: React for the UI

Status: Accepted

## Context

The technical design proposed SolidJS for fine-grained reactivity, then listed React as the leaning choice because agents write React more reliably (section 19, open questions).

## Decision

The UI uses React 19 with TypeScript in strict mode, built with Vite and styled with Tailwind CSS 4. Panels with many live values subscribe to narrow slices of a small external store, which gives the fine-grained updates SolidJS would have provided.

## Consequences

The largest ecosystem and the most reliable agent output. Hot paths (viewport, sketch drag) bypass React rendering entirely and talk to the renderer directly.
