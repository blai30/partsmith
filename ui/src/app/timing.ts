import { measureTiming, type MeasureName } from '../performance/marks'

// Measures that end once the viewport has drawn their result (ui.md 2.18): ps:open, ps:rebuild and
// ps:preview. The viewport draws in an animation frame it requests as soon as a result reaches the
// store or the scene, and animation frame callbacks run in the order they were requested, so a
// frame requested after the result was applied runs right after that drawing.

/** Records the measure `name` from `start` (a `performance.now()` time) to the next animation frame. */
export function measureToNextFrame(name: MeasureName, start: number): void {
  if (typeof requestAnimationFrame !== 'function') {
    // Without animation frames nothing waits to be drawn, so the result is complete now.
    measureTiming(name, start)
    return
  }
  requestAnimationFrame(() => {
    measureTiming(name, start)
  })
}
