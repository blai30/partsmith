import type { SolvedSketch } from '../../core/api'
import { CoreError } from '../../core/client'
import { createLatestOnly } from '../../core/queue'
import { measureTiming, TIMING_NAMES } from '../../performance/marks'
import type { Vector2 } from '../../sketch/geometry'
import type { App } from '../createApp'
import { documentOf, showCoreError } from './edits'
import { recordSketchEdit, setSketchDragging, showSketchSolution } from './sketch'

// Dragging a point of the edited sketch (ui.md 3.16.9).

/** A point drag: moves solved through the core one at a time, then one commit or a cancel. */
export type SketchDrag = {
  /** Asks the core to solve with the point pinned at `to`; `startedAt` is the pointer move's time. */
  move(to: Vector2, startedAt: number): void
  /** Ends the drag, committing the last solved frame as one undo entry when a move was sent. */
  finish(): Promise<void>
  /** Ends the drag without committing and shows the stored geometry again. */
  cancel(): Promise<void>
}

type DragRequest = { readonly to: Vector2; readonly startedAt: number }

/**
 * Starts dragging a point of the edited sketch. Each move goes through a LatestOnly outside the
 * mutation queue, since drag frames change nothing in the document; each solution is shown at
 * once. The end goes through the mutation queue.
 */
export function startSketchDrag(app: App, sketchId: string, point: string): SketchDrag | null {
  const document = documentOf(app)
  if (document === null) {
    return null
  }
  const { documentHandle } = document
  const before = document.sketches[sketchId] ?? null
  let sent = false
  // Closing: finish or cancel was called. Ended: solved frames are no longer shown.
  let closing = false
  let ended = false
  const frames = createLatestOnly<DragRequest, SolvedSketch | null>(
    (request) =>
      app.core.dragSketch(documentHandle, sketchId, point, [request.to[0], request.to[1]]),
    (solved, request) => {
      if (solved === null || ended) {
        return
      }
      showSketchSolution(app, sketchId, solved)
      // The viewport draws in the next animation frame, which runs before this one.
      requestAnimationFrame(() => {
        measureTiming(TIMING_NAMES.sketchDrag, request.startedAt)
      })
    },
    (error) => {
      showCoreError(app, error)
    },
  )
  setSketchDragging(app, true)

  /**
   * Ends the drag in the core as one task of the mutation queue, queued when the drag closes: a
   * commit or undo asked for right after the release runs after it and sees the dragged geometry.
   * A committing end first waits for the last drag frame.
   */
  async function end(commit: boolean): Promise<void> {
    try {
      await app.mutations.run(async () => {
        if (commit) {
          await frames.flush()
        }
        ended = true
        await app.core.endSketchDrag(documentHandle, sketchId, commit)
      })
      if (commit) {
        recordSketchEdit(app, sketchId)
      }
    } catch (error) {
      if (!(error instanceof CoreError && error.code === 'no_drag')) {
        showCoreError(app, error)
      }
      if (before !== null) {
        showSketchSolution(app, sketchId, before)
      }
    }
  }

  return {
    move(to, startedAt) {
      if (closing) {
        return
      }
      sent = true
      frames.submit({ to, startedAt })
    },
    async finish() {
      if (closing) {
        return
      }
      closing = true
      if (sent) {
        await end(true)
      }
      ended = true
      setSketchDragging(app, false)
    },
    async cancel() {
      if (closing) {
        return
      }
      closing = true
      ended = true
      frames.cancel()
      if (before !== null) {
        showSketchSolution(app, sketchId, before)
      }
      if (sent) {
        await end(false)
      }
      setSketchDragging(app, false)
    },
  }
}
