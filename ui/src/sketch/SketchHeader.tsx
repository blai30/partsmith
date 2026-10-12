import { clsx } from 'clsx/lite'
import { ChevronDown } from 'lucide-react'
import { useId, useRef, useState } from 'react'
import { useApp, useAppState } from '../app/AppProvider'
import { deleteSketchConstraints, showSketchFinding } from '../app/actions/sketch'
import { Button } from '../components/Button'
import { Popover } from '../components/Popover'
import type { ProfileFinding, SketchConstraint } from '../generated/protocol'
import { selectFeatureLabel, selectSketchDof, type SketchDof } from '../state/selectors'
import type { AppState } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { constraintTitle } from './constraints'
import { profileFindingMessage } from './issues'

// The sketch header (ui.md 3.16.8): the sketch's name and constraint status at the top of the
// viewport, opening the list of the sketch's problems.

const PICKING_TEXT = 'Pick a plane or a flat face'

function plural(count: number, singular: string, several: string): string {
  return `${String(count)} ${count === 1 ? singular : several}`
}

/** The status text of a header state, and whether it reads as an error. */
function statusOf(dof: SketchDof): { readonly text: string; readonly error: boolean } {
  switch (dof.state) {
    case 'picking':
      return { text: PICKING_TEXT, error: false }
    case 'empty':
      return { text: 'No geometry yet', error: false }
    case 'conflict':
      return {
        text: plural(dof.conflicting, 'conflicting constraint', 'conflicting constraints'),
        error: true,
      }
    case 'failed':
      return { text: 'Cannot solve', error: true }
    case 'full':
      return { text: 'Fully constrained', error: false }
    case 'under':
      return { text: `${String(dof.dof)} DOF`, error: false }
  }
}

/** The edited sketch's id: undefined outside sketch mode, null while picking a plane. */
function selectEditedSketch(state: AppState): string | null | undefined {
  return state.context === 'sketch' && state.sketch !== null ? state.sketch.sketchId : undefined
}

/** The sketch header chip; nothing outside sketch mode. */
export function SketchHeader() {
  const sketchId = useAppState(selectEditedSketch)
  const dof = useAppState(selectSketchDof)
  const label = useAppState((state) =>
    sketchId === undefined || sketchId === null ? '' : selectFeatureLabel(state, sketchId),
  )
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const popoverId = useId()

  if (sketchId === undefined) {
    return null
  }
  if (sketchId === null) {
    return (
      <output
        data-testid="sketch-header"
        data-state="picking"
        data-dof="0"
        className={clsx(
          'absolute top-3 left-1/2 z-10 flex h-8 -translate-x-1/2 items-center rounded-md border px-3 text-sm font-medium whitespace-nowrap shadow-sm',
          COLOR_CLASSES.surfaceRaised.background,
          COLOR_CLASSES.border.border,
          COLOR_CLASSES.text.foreground,
        )}
      >
        {PICKING_TEXT}
      </output>
    )
  }

  const status = dof === null ? null : statusOf(dof)
  return (
    <>
      <button
        ref={anchor}
        type="button"
        data-testid="sketch-header"
        data-state={dof?.state}
        data-dof={dof === null ? undefined : String(dof.dof)}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={popoverId}
        onClick={() => {
          setOpen((wasOpen) => !wasOpen)
        }}
        className={clsx(
          'absolute top-3 left-1/2 z-10 flex h-8 max-w-[calc(100%-2rem)] -translate-x-1/2 items-center gap-3 rounded-md border pr-2 pl-3 text-sm whitespace-nowrap shadow-sm transition-colors duration-150 motion-reduce:transition-none',
          COLOR_CLASSES.surfaceRaised.background,
          COLOR_CLASSES.border.border,
          COLOR_CLASSES.text.foreground,
          COLOR_CLASSES.hover.background,
          COLOR_CLASSES.focusRing.outline,
          COLOR_CLASSES.accent.outline,
        )}
      >
        <span className="truncate font-medium">{label}</span>
        {status === null ? null : (
          <>
            <span className="sr-only">, </span>
            <span
              className={clsx(
                'flex items-center border-l pl-3',
                COLOR_CLASSES.border.border,
                status.error
                  ? COLOR_CLASSES.errorText.foreground
                  : COLOR_CLASSES.textMuted.foreground,
              )}
            >
              <span>{status.text}</span>
              {dof !== null && dof.redundant > 0 ? (
                <span className={COLOR_CLASSES.warningText.foreground}>
                  {` · ${String(dof.redundant)} redundant`}
                </span>
              ) : null}
            </span>
          </>
        )}
        <ChevronDown
          aria-hidden="true"
          strokeWidth={1.75}
          className={clsx('size-4 shrink-0', COLOR_CLASSES.textMuted.foreground)}
        />
      </button>
      <Popover label="Sketch problems" open={open} anchor={anchor} onOpenChange={setOpen}>
        <SketchProblems id={popoverId} sketchId={sketchId} />
      </Popover>
    </>
  )
}

type ConstraintProblem = { readonly constraint: SketchConstraint; readonly conflicting: boolean }

/** The conflicting and redundant constraints and the profile issues of the edited sketch. */
function SketchProblems({ id, sketchId }: { readonly id: string; readonly sketchId: string }) {
  const app = useApp()
  const document = useAppState((state) => state.document)
  const solution = document?.sketches[sketchId]?.solution ?? null
  const record = document?.recipe.features?.find((feature) => feature.id === sketchId)
  const stored: unknown = record?.['constraints']
  const constraints = Array.isArray(stored) ? (stored as SketchConstraint[]) : []
  const units = document?.recipe.units ?? 'mm'
  const problems = constraints.flatMap((constraint): ConstraintProblem[] => {
    if (solution?.conflicting.includes(constraint.id) === true) {
      return [{ constraint, conflicting: true }]
    }
    return solution?.redundant.includes(constraint.id) === true
      ? [{ constraint, conflicting: false }]
      : []
  })
  const findings: readonly ProfileFinding[] = solution?.profile_findings ?? []

  return (
    <div id={id} data-testid="sketch-problems" className="flex w-80 max-w-full flex-col gap-4">
      {problems.length === 0 && findings.length === 0 ? (
        <p className={COLOR_CLASSES.textMuted.foreground}>
          This sketch has no conflicting or redundant constraints and no profile problems.
        </p>
      ) : null}
      {problems.length > 0 ? (
        <section aria-label="Constraints" className="flex flex-col gap-2">
          <h2 className={clsx('text-xs font-medium', COLOR_CLASSES.textMuted.foreground)}>
            Constraints
          </h2>
          <ul className="flex flex-col gap-1">
            {problems.map(({ constraint, conflicting }) => {
              const title = constraintTitle(constraint, solution, units)
              return (
                <li key={constraint.id} className="flex items-center justify-between gap-3">
                  <span className="min-w-0">
                    <span className="block truncate">{title}</span>
                    <span
                      className={clsx(
                        'block text-xs',
                        conflicting
                          ? COLOR_CLASSES.errorText.foreground
                          : COLOR_CLASSES.warningText.foreground,
                      )}
                    >
                      {conflicting ? 'Conflicting' : 'Redundant'}
                    </span>
                  </span>
                  <Button
                    aria-label={`Delete ${title}`}
                    onClick={() => {
                      void deleteSketchConstraints(app, [constraint.id])
                    }}
                  >
                    Delete
                  </Button>
                </li>
              )
            })}
          </ul>
        </section>
      ) : null}
      {findings.length > 0 ? (
        <section aria-label="Profile" className="flex flex-col gap-2">
          <h2 className={clsx('text-xs font-medium', COLOR_CLASSES.textMuted.foreground)}>
            Profile
          </h2>
          <ul className="flex flex-col gap-2">
            {findings.map((finding, index) => {
              const message = profileFindingMessage(finding)
              return (
                // Findings have no ids; their order is stable for one solution (sketch.md 3.13.7).
                // oxlint-disable-next-line react/no-array-index-key
                <li key={index} className="flex items-start justify-between gap-3">
                  <span className="min-w-0 text-sm">{message}</span>
                  <Button
                    aria-label={`Show: ${message}`}
                    onClick={() => {
                      showSketchFinding(app, finding)
                    }}
                  >
                    Show
                  </Button>
                </li>
              )
            })}
          </ul>
        </section>
      ) : null}
    </div>
  )
}
