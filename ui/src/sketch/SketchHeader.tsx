import { clsx } from 'clsx/lite'
import { ChevronDown } from 'lucide-react'
import { useId, useRef, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { useApp, useAppState } from '../app/AppProvider'
import {
  createSketchOn,
  deleteSketchConstraints,
  selectInSketch,
  showSketchFinding,
} from '../app/actions/sketch'
import { Button } from '../components/Button'
import { Popover } from '../components/Popover'
import { originPlaneName, originPlaneView, type OriginPlane } from '../core/labels'
import type { ProfileFinding, SketchConstraint, SketchSolution } from '../generated/protocol'
import { originItem } from '../inspector/selection-values'
import { MESSAGES } from '../messages'
import { selectFeatureLabel, selectSketchDof, type SketchDof } from '../state/selectors'
import type { AppState } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { constraintTitle } from './constraints'
import { entitySelectionItem } from './hit'
import { outlineProblems, profileFindingMessage } from './issues'
import { sketchStepHint, type SketchOverlayModel } from './session'

// The sketch header and hint (ui.md 3.16.1, 3.16.3, 3.16.8): the sketch's name and constraint
// status in the viewport's top-center slot (3.2), opening the list of the sketch's problems, and
// under it the step prompt. While a plane is picked for a new sketch, it offers the three origin
// planes. The sketch overlay renders it with the sketch controller's hint.

function plural(count: number, singular: string, several: string): string {
  return `${String(count)} ${count === 1 ? singular : several}`
}

/** The status text of a header state, and whether it reads as an error. */
function statusOf(dof: SketchDof): { readonly text: string; readonly error: boolean } {
  switch (dof.state) {
    case 'picking':
      return { text: 'Pick a plane or a flat face to draw on', error: false }
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
      return { text: `Not fully constrained · ${String(dof.dof)} DOF`, error: false }
  }
}

/** The edited sketch's id: undefined outside sketch mode, null while picking a plane. */
function selectEditedSketch(state: AppState): string | null | undefined {
  return state.context === 'sketch' && state.sketch !== null ? state.sketch.sketchId : undefined
}

/** The three origin planes a new sketch can start on in one click (3.16.1). */
const ORIGIN_PLANES: readonly OriginPlane[] = ['XY', 'XZ', 'YZ']

export type SketchHeaderProps = {
  /** The active tool's hint from the sketch controller (3.16.3), or null without one. */
  readonly hint?: SketchOverlayModel['hint']
  /** Whether inference glyphs show, so the hint names the key that places freely (3.16.5). */
  readonly freePlacement?: boolean
}

/** The sketch header chip and the sketch hint in the top-center slot; nothing outside sketch mode. */
export function SketchHeader({ hint = null, freePlacement = false }: SketchHeaderProps) {
  const sketchId = useAppState(selectEditedSketch)
  if (sketchId === undefined) {
    return null
  }
  return (
    <div className="pointer-events-none absolute inset-x-32 top-4 z-10 flex flex-col items-center gap-2">
      {sketchId === null ? <PlanePicking /> : <EditingHeader sketchId={sketchId} />}
      <SketchHint sketchId={sketchId} hint={hint} freePlacement={freePlacement} />
    </div>
  )
}

/** While a plane is picked for a new sketch: the prompt and the three origin planes. */
function PlanePicking() {
  const app = useApp()
  return (
    <fieldset
      data-testid="sketch-header"
      data-state="picking"
      data-dof="0"
      data-outline="0"
      className={clsx(
        'pointer-events-auto flex h-10 items-center gap-2 rounded-md border pr-1 pl-3 text-sm whitespace-nowrap shadow-sm',
        COLOR_CLASSES.surfaceRaised.background,
        COLOR_CLASSES.border.border,
        COLOR_CLASSES.text.foreground,
      )}
    >
      <legend className="float-left font-medium">Pick a plane or a flat face to draw on</legend>
      {ORIGIN_PLANES.map((plane) => (
        <Button
          key={plane}
          aria-label={`Sketch on ${originPlaneName(plane)}`}
          onClick={() => {
            createSketchOn(app, originItem(plane)).catch((error: unknown) => {
              console.error('Creating the sketch failed.', error)
            })
          }}
        >
          {originPlaneView(plane)}
        </Button>
      ))}
    </fieldset>
  )
}

/** The outline problems of a solution, for the header's `data-outline` (3.16.8). */
function selectOutlineCount(sketchId: string): (state: AppState) => number {
  return (state) => state.document?.sketches[sketchId]?.solution.profile_findings.length ?? 0
}

function EditingHeader({ sketchId }: { readonly sketchId: string }) {
  // Compared by value: a drag replaces the solution every frame without changing these counts.
  const dof = useAppState(useShallow(selectSketchDof))
  const label = useAppState((state) => selectFeatureLabel(state, sketchId))
  const outline = useAppState((state) => {
    const solution = state.document?.sketches[sketchId]?.solution
    return solution === undefined ? null : outlineProblems(solution)
  })
  const outlineCount = useAppState(selectOutlineCount(sketchId))
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const popoverId = useId()
  const status = dof === null ? null : statusOf(dof)

  return (
    <>
      <button
        ref={anchor}
        type="button"
        data-testid="sketch-header"
        data-state={dof?.state}
        data-dof={dof === null ? undefined : String(dof.dof)}
        data-outline={String(outlineCount)}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={popoverId}
        onClick={() => {
          setOpen((wasOpen) => !wasOpen)
        }}
        className={clsx(
          'pointer-events-auto flex h-8 max-w-full items-center gap-3 rounded-md border pr-2 pl-3 text-sm whitespace-nowrap shadow-sm transition-colors duration-150 motion-reduce:transition-none',
          COLOR_CLASSES.surfaceRaised.background,
          COLOR_CLASSES.border.border,
          COLOR_CLASSES.text.foreground,
          COLOR_CLASSES.hover.background,
          COLOR_CLASSES.focusRing.outline,
          COLOR_CLASSES.accent.outline,
        )}
      >
        <span className="truncate font-medium">{label}</span>
        {status === null || dof === null ? null : (
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
              {outline === null ? null : (
                <span className={COLOR_CLASSES.warningText.foreground}>{` · ${outline}`}</span>
              )}
              {dof.redundant > 0 ? (
                <span className={COLOR_CLASSES.warningText.foreground}>
                  {` · ${plural(dof.redundant, 'extra constraint', 'extra constraints')}`}
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
        <div
          id={popoverId}
          data-testid="sketch-problems"
          className="flex w-80 max-w-full flex-col gap-4"
        >
          {/* Rendered only while open: a drag changes the solution every frame. */}
          {open ? <SketchProblems sketchId={sketchId} /> : null}
        </div>
      </Popover>
    </>
  )
}

type SketchHintProps = {
  readonly sketchId: string | null
  readonly hint: SketchOverlayModel['hint']
  readonly freePlacement: boolean
}

/** The sketch hint under the header (3.16.3): the step prompt, or what replaces it for now. */
function SketchHint({ sketchId, hint, freePlacement }: SketchHintProps) {
  const app = useApp()
  const pickHint = useAppState((state) =>
    state.pickHint !== null && state.pickHint.field === null ? state.pickHint.message : null,
  )
  const idle = useAppState((state) => state.sketch?.tool === null)
  const empty = useAppState(
    (state) =>
      sketchId !== null &&
      (state.document?.sketches[sketchId]?.solution.entities.length ?? 0) === 0,
  )
  const idleStep = idle ? sketchStepHint({ kind: 'idle', empty }, app.platform) : null
  const text =
    pickHint ?? (sketchId === null ? MESSAGES.plane_pick_hint() : (hint?.text ?? idleStep))
  const warning = pickHint !== null || hint?.warning === true
  return (
    <output
      data-testid="sketch-hint"
      className={clsx(
        text === null
          ? 'sr-only'
          : 'max-w-full rounded-md border px-3 py-1 text-center text-xs text-pretty shadow-sm',
        text !== null && COLOR_CLASSES.surfaceRaised.background,
        text !== null && COLOR_CLASSES.border.border,
        text !== null && warning && COLOR_CLASSES.text.foreground,
        text !== null && !warning && COLOR_CLASSES.textMuted.foreground,
      )}
    >
      {text}
      {text !== null && sketchId !== null && freePlacement ? (
        <span aria-live="off">{` ${MESSAGES.sketch_free_placement(app.platform)}`}</span>
      ) : null}
    </output>
  )
}

type ConstraintProblem = { readonly constraint: SketchConstraint; readonly conflicting: boolean }

const ENTITY_NOUNS = {
  point: ['point', 'points'],
  line: ['line', 'lines'],
  circle: ['circle', 'circles'],
  arc: ['arc', 'arcs'],
} as const

/** "3 lines, 1 circle": the under-constrained entities counted by type, in type order (3.16.8). */
function freeGeometry(solution: SketchSolution): string {
  const counts = new Map<keyof typeof ENTITY_NOUNS, number>()
  for (const entity of solution.entities) {
    if (entity.constrained === 'under_constrained') {
      counts.set(entity.type, (counts.get(entity.type) ?? 0) + 1)
    }
  }
  return (['line', 'circle', 'arc', 'point'] as const)
    .flatMap((type) => {
      const count = counts.get(type) ?? 0
      const [singular, several] = ENTITY_NOUNS[type]
      return count === 0 ? [] : [plural(count, singular, several)]
    })
    .join(', ')
}

/** The conflicting and extra constraints, the outline problems and the free geometry of the sketch. */
function SketchProblems({ sketchId }: { readonly sketchId: string }) {
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
  const free = solution !== null && solution.dof > 0 ? freeGeometry(solution) : ''

  return (
    <>
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
                      {conflicting ? 'Conflicting' : 'Extra'}
                    </span>
                  </span>
                  <Button
                    aria-label={`Delete ${title}`}
                    onClick={() => {
                      deleteSketchConstraints(app, [constraint.id]).catch((error: unknown) => {
                        console.error('Deleting the constraint failed.', error)
                      })
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
        <section aria-label="Outline" className="flex flex-col gap-2">
          <h2 className={clsx('text-xs font-medium', COLOR_CLASSES.textMuted.foreground)}>
            Outline
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
      {free === '' || solution === null ? null : (
        <section aria-label="Free geometry" className="flex items-center justify-between gap-3">
          <p className="min-w-0 text-sm">{`Free geometry: ${free}`}</p>
          <Button
            onClick={() => {
              selectInSketch(
                app,
                solution.entities
                  .filter((entity) => entity.constrained === 'under_constrained')
                  .map((entity) => entitySelectionItem(sketchId, entity)),
                'replace',
              )
            }}
          >
            Select them
          </Button>
        </section>
      )}
      <p className={clsx('text-xs', COLOR_CLASSES.textMuted.foreground)}>
        {MESSAGES.sketch_dof_help(app.platform)}
      </p>
    </>
  )
}
