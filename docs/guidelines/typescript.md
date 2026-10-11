# TypeScript guidelines

- Use `type` instead of `interface`, except for Astro component props.
- Keep naming intention-revealing, consistent with nearby code.
- No single-letter variables except `i` in loops, `x`/`y` for coordinates. Avoid overly short names, but also unnecessary verbosity (eg. `latitude`/`longitude`, not `lat`/`lon`).
- Use latest stable APIs for dependencies already in the project.
