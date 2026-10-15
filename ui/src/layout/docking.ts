// Docking (ui.md 3.2): the browser and the side panel dock while the window is at least 65rem
// wide, the root's 0.5rem gutter around the `@5xl/app` container query of 64rem, so the UI scale
// counts. Below it they float over the free area as overlays. Media queries evaluate rem against
// the initial font size and would miss the UI scale, so the code measures the root's font size.

/** The browser and side panel dock while the window is at least this many rem wide. */
export const DOCKING_WIDTH_REM = 65

/** The browser's default font size in CSS pixels, where none can be read (jsdom). */
const DEFAULT_FONT_PIXELS = 16

/** The root element's computed font size in CSS pixels. */
function rootFontPixels(): number {
  const size = Number.parseFloat(getComputedStyle(document.documentElement).fontSize)
  return Number.isFinite(size) && size > 0 ? size : DEFAULT_FONT_PIXELS
}

/** Whether the browser and side panel float as overlays: the window is narrower than 65rem now. */
export function isNarrowWindow(): boolean {
  return typeof window !== 'undefined' && window.innerWidth < DOCKING_WIDTH_REM * rootFontPixels()
}

/**
 * Whether the window is narrower than 65rem at `uiScale`, measured with the root's font size
 * before the UI scale class is applied (the store is created first), so the browser starts closed.
 */
export function startsNarrow(uiScale: number): boolean {
  return (
    typeof window !== 'undefined' &&
    window.innerWidth < DOCKING_WIDTH_REM * rootFontPixels() * uiScale
  )
}
