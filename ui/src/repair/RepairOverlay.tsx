import { clsx } from 'clsx/lite'
import { useEffect, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { useApp, useAppState } from '../app/AppProvider'
import { capitalized } from '../messages'
import { COLOR_CLASSES } from '../theme/roles'
import type { Vector3 } from '../viewport/math'
import { candidateHandlers } from './candidates'
import { candidatePoints } from './frame'
import { GHOST_DASHES, GHOST_STROKE_CLASSES } from './ghost-style'
import {
  ghostBuffer,
  repairTitle,
  selectRegionCandidates,
  selectRepairMode,
  type RepairCandidateView,
  type RepairMode,
} from './repair'

// Repair mode on the model (ui.md 3.7): each repair's ghost as a dashed outline where the element
// was in its last build or where the region's outline lay on its sketch, else a dashed ring at an
// element's stored centroid, and a numbered marker at each candidate of the current repair, at its
// element or at the centroid of its first region.
// `paint.ts` places the ghosts and rings in each frame; the scene fills the candidate regions.

/** The radius in CSS pixels of the ring at a stored centroid: 16 pixels across (3.7). */
const CENTROID_RING_RADIUS = 8

/** The centroids of the current repair's candidates, once listed (3.7); none until then. */
function useCandidatePoints(
  candidates: readonly RepairCandidateView[] | null,
): readonly (Vector3 | null)[] {
  const app = useApp()
  const [found, setFound] = useState<{
    readonly candidates: readonly RepairCandidateView[]
    readonly points: readonly (Vector3 | null)[]
  } | null>(null)
  useEffect(() => {
    if (candidates === null) {
      return undefined
    }
    let current = true
    candidatePoints(app, candidates).then(
      (points) => {
        if (current) {
          setFound({ candidates, points })
        }
      },
      (error: unknown) => {
        console.warn('Partsmith could not place the repair candidates.', error)
      },
    )
    return () => {
      current = false
    }
  }, [app, candidates])
  return found !== null && found.candidates === candidates ? found.points : []
}

/** The ghosts and rings of repair mode, inside the overlay's SVG layer. */
function RepairShapes({ mode }: { readonly mode: RepairMode }) {
  // Compared by value: the selector runs on every store change.
  const ghosts = useAppState(
    useShallow((state) => mode.repairs.map((repair) => ghostBuffer(state, repair) !== undefined)),
  )
  return (
    <svg aria-hidden="true" className="pointer-events-none absolute inset-0 size-full">
      {mode.repairs.map((repair, index) => {
        if (ghosts[index] === true) {
          return (
            <path
              key={repair.path}
              data-testid="repair-ghost"
              data-repair={index}
              strokeDasharray={GHOST_DASHES}
              className={GHOST_STROKE_CLASSES}
            >
              <title>{repairTitle(repair)}</title>
            </path>
          )
        }
        // A region repair without a ghost draws nothing: it has no centroid.
        const centroid = repair.kind === 'element' ? (repair.centroid ?? null) : null
        return centroid === null ? null : (
          <circle
            key={repair.path}
            data-testid="repair-marker"
            data-x={centroid[0]}
            data-y={centroid[1]}
            data-z={centroid[2]}
            r={CENTROID_RING_RADIUS}
            display="none"
            strokeDasharray={GHOST_DASHES}
            className={GHOST_STROKE_CLASSES}
          >
            <title>{repairTitle(repair)}</title>
          </circle>
        )
      })}
    </svg>
  )
}

/** A candidate's numbered marker at its element's centroid, anchored per 3.12. */
function CandidateMarker({
  candidate,
  at,
}: {
  readonly candidate: RepairCandidateView
  readonly at: Vector3
}) {
  const app = useApp()
  return (
    <button
      type="button"
      data-testid="repair-candidate"
      data-number={candidate.number}
      data-anchor={at.join(',')}
      aria-label={`${String(candidate.number)}. ${capitalized(candidate.description)}`}
      // The banner's buttons are the keyboard way to the same candidates (3.24).
      tabIndex={-1}
      hidden
      {...candidateHandlers(app, candidate)}
      // Pressing a marker leaves focus where it is, so the Problems row that shows repair mode
      // keeps it until the click runs the candidate's action (3.7).
      onMouseDown={(event) => {
        event.preventDefault()
      }}
      className={clsx(
        // Centered on its anchor by margins, since the anchor sets the translate (3.12).
        'pointer-events-auto absolute top-0 left-0 -mt-2.5 -ml-2.5 flex size-5 items-center justify-center rounded-sm border text-xs font-semibold tabular-nums shadow-md',
        COLOR_CLASSES.surface.background,
        COLOR_CLASSES.errorMark.border,
        COLOR_CLASSES.text.foreground,
        COLOR_CLASSES.elevation.shadow,
      )}
    >
      {candidate.number}
    </button>
  )
}

/** Repair mode on the model (3.7): ghosts, rings and candidate markers while it is shown. */
export function RepairOverlay() {
  const app = useApp()
  const mode = useAppState(selectRepairMode)
  const regions = useAppState(selectRegionCandidates)
  const candidates = mode?.view.candidates ?? null
  const elementPoints = useCandidatePoints(regions === null ? candidates : null)
  const points = regions?.anchors ?? elementPoints
  // New shapes and markers are placed by the next frame.
  useEffect(() => {
    if (mode !== null) {
      app.viewport()?.requestRender()
    }
  })
  if (mode === null) {
    return null
  }
  return (
    <>
      <RepairShapes mode={mode} />
      {(candidates ?? []).map((candidate, index) => {
        const at = points[index] ?? null
        return at === null ? null : (
          <CandidateMarker key={candidate.number} candidate={candidate} at={at} />
        )
      })}
    </>
  )
}
