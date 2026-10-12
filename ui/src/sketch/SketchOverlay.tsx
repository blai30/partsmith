import { clsx } from 'clsx/lite'
import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type KeyboardEvent,
  type MouseEvent,
  type RefObject,
} from 'react'
import { sketchParamsOf } from '../app/actions/sketch'
import { useApp, useAppState } from '../app/AppProvider'
import { CONSTRAINT_ICONS } from '../commands/icons'
import type { SketchConstraint, SketchSolution, Units } from '../generated/protocol'
import { resolvedTheme } from '../layout/color-scheme'
import { COLOR_CLASSES, FONT_CLASSES, VIEWPORT_COLORS } from '../theme/roles'
import { positionElement } from '../viewport/overlay'
import { dimensionLabelText, hasGlyph, solvedValue } from './annotations'
import { constraintTitle, isDimension, type ConstraintType } from './constraints'
import { DimensionEditor } from './DimensionEditor'
import { DrawInputs } from './DrawInputs'
import type { Vector2 } from './geometry'
import { issueRings } from './issues'
import {
  createSketchController,
  EMPTY_SKETCH_MODEL,
  type OverlayPaint,
  type SketchController,
} from './session'

// The sketch overlay (ui.md 3.12, 3.16): constraint glyphs, dimensions, profile issue rings, the
// rubber band, inference glyphs, draw inputs, the dimension editor and the selection box over the
// canvas. React renders the elements; the sketch controller writes their positions each frame.

type GlyphConstraint = Exclude<SketchConstraint, { expr: string }>
type DimensionConstraint = Extract<SketchConstraint, { expr: string }>

/** The elements the painter writes positions and path data into. */
type OverlayElements = {
  readonly glyphs: ReadonlyMap<string, HTMLElement>
  readonly labels: ReadonlyMap<string, HTMLElement>
  readonly issues: ReadonlyMap<number, SVGCircleElement>
  readonly dimensionLines: SVGPathElement | null
  readonly dimensionArrows: SVGPathElement | null
  readonly highlight: SVGPathElement | null
  readonly rubberBand: SVGPathElement | null
  readonly markers: SVGPathElement | null
  readonly box: SVGRectElement | null
  readonly inference: HTMLDivElement | null
  readonly drawInputs: HTMLFieldSetElement | null
  readonly editor: HTMLFieldSetElement | null
  readonly placing: HTMLSpanElement | null
}

/** Places an element with its top left at a canvas pixel, or hides it. */
function placeAt(element: HTMLElement | null, at: Vector2 | null): void {
  if (element === null) {
    return
  }
  if (at === null) {
    element.hidden = true
    return
  }
  element.hidden = false
  positionElement(element, at[0], at[1])
}

/** Places an element centered on a canvas pixel, or hides it. */
function placeCentered(element: HTMLElement | null, at: Vector2 | null): void {
  if (element === null) {
    return
  }
  if (at === null) {
    element.hidden = true
    return
  }
  // Shown first, so its size can be measured.
  element.hidden = false
  positionElement(element, at[0] - element.offsetWidth / 2, at[1] - element.offsetHeight / 2)
}

function setAttribute(element: Element | null, name: string, value: string): void {
  if (element !== null && element.getAttribute(name) !== value) {
    element.setAttribute(name, value)
  }
}

/** Writes a frame's positions and path data into the overlay (3.12 runtime positions). */
function paintOverlay(paint: OverlayPaint, elements: OverlayElements): void {
  for (const [id, element] of elements.glyphs) {
    placeCentered(element, paint.glyphs.get(id) ?? null)
  }
  for (const [id, element] of elements.labels) {
    placeCentered(element, paint.labels.get(id) ?? null)
  }
  for (const [index, ring] of elements.issues) {
    const at = paint.issues[index] ?? null
    setAttribute(ring, 'visibility', at === null ? 'hidden' : 'visible')
    if (at !== null) {
      setAttribute(ring, 'cx', String(at[0]))
      setAttribute(ring, 'cy', String(at[1]))
    }
  }
  setAttribute(elements.dimensionLines, 'd', paint.dimensionLines)
  setAttribute(elements.dimensionArrows, 'd', paint.dimensionArrows)
  setAttribute(elements.highlight, 'd', paint.highlight)
  setAttribute(elements.rubberBand, 'd', paint.rubberBand)
  setAttribute(elements.markers, 'd', paint.markers)
  const box = elements.box
  if (box !== null && paint.box !== null) {
    setAttribute(box, 'x', String(paint.box.x))
    setAttribute(box, 'y', String(paint.box.y))
    setAttribute(box, 'width', String(paint.box.width))
    setAttribute(box, 'height', String(paint.box.height))
  }
  placeAt(elements.inference, paint.inference)
  placeAt(elements.drawInputs, paint.drawInputs)
  placeCentered(elements.editor, paint.editor)
  placeCentered(elements.placing, paint.placing)
  // Empty draw inputs show the rubber band's measured values, written here and never by React.
  for (const input of elements.drawInputs?.querySelectorAll('input') ?? []) {
    const name = input.dataset['field']
    const measure = Object.entries(paint.measures).find(([field]) => field === name)?.[1]
    setAttribute(input, 'placeholder', measure ?? '')
  }
}

function subscribeNothing(): () => void {
  return () => undefined
}

function emptyModel() {
  return EMPTY_SKETCH_MODEL
}

/** Shift or Mod with a click toggles the item in the sketch selection (3.16.10). */
function toggles(event: MouseEvent | KeyboardEvent): boolean {
  return event.shiftKey || event.ctrlKey || event.metaKey
}

/** The overlay of the sketch being edited; nothing outside sketch mode and while picking a plane. */
export function SketchOverlay() {
  const sketchId = useAppState((state) =>
    state.context === 'sketch' ? (state.sketch?.sketchId ?? null) : null,
  )
  return sketchId === null ? null : <SketchLayer key={sketchId} sketchId={sketchId} />
}

function SketchLayer({ sketchId }: { readonly sketchId: string }) {
  const app = useApp()
  const layer = useRef<HTMLDivElement>(null)
  const [controller, setController] = useState<SketchController | null>(null)
  const model = useSyncExternalStore(
    controller?.subscribe ?? subscribeNothing,
    controller?.getModel ?? emptyModel,
  )
  const solution = useAppState((state) => state.document?.sketches[sketchId]?.solution ?? null)
  const record = useAppState(
    (state) => state.document?.recipe.features?.find((feature) => feature.id === sketchId) ?? null,
  )
  const units = useAppState((state) => state.document?.recipe.units ?? 'mm')
  const showConstraints = useAppState((state) => state.sketch?.showConstraints ?? true)
  const selection = useAppState((state) => state.sketch?.selection ?? null)
  const theme = useAppState((state) => resolvedTheme(state.settings))
  const uiScale = useAppState((state) => state.settings.uiScale)
  const colors = VIEWPORT_COLORS[theme]

  const constraints = useMemo(
    () => (record?.type === 'sketch' ? (sketchParamsOf(record)?.constraints ?? []) : []),
    [record],
  )
  const glyphConstraints = useMemo(
    () =>
      constraints.filter(
        (constraint): constraint is GlyphConstraint =>
          !isDimension(constraint) && hasGlyph(constraint),
      ),
    [constraints],
  )
  const dimensions = useMemo(
    () =>
      constraints.filter((constraint): constraint is DimensionConstraint =>
        isDimension(constraint),
      ),
    [constraints],
  )
  const rings = useMemo(() => (solution === null ? [] : issueRings(solution)), [solution])
  const selected = useMemo(() => new Set((selection ?? []).map((item) => item.key)), [selection])

  const glyphElements = useRef(new Map<string, HTMLElement>())
  const labelElements = useRef(new Map<string, HTMLElement>())
  const issueElements = useRef(new Map<number, SVGCircleElement>())
  const dimensionLines = useRef<SVGPathElement>(null)
  const dimensionArrows = useRef<SVGPathElement>(null)
  const highlight = useRef<SVGPathElement>(null)
  const rubberBand = useRef<SVGPathElement>(null)
  const markers = useRef<SVGPathElement>(null)
  const box = useRef<SVGRectElement>(null)
  const inference = useRef<HTMLDivElement>(null)
  const drawInputs = useRef<HTMLFieldSetElement>(null)
  const editor = useRef<HTMLFieldSetElement>(null)
  const placing = useRef<HTMLSpanElement>(null)

  // The controller reads pointer input on the viewport's canvas, beside the viewport controller.
  useEffect(() => {
    const canvas =
      layer.current?.closest('[data-testid="viewport"]')?.querySelector('canvas') ?? null
    if (canvas === null) {
      return undefined
    }
    const created = createSketchController({ app, canvas })
    setController(created)
    return () => {
      created.dispose()
      setController(null)
    }
  }, [app])

  useLayoutEffect(() => {
    if (controller === null) {
      return undefined
    }
    controller.setPainter((paint) => {
      paintOverlay(paint, {
        glyphs: glyphElements.current,
        labels: labelElements.current,
        issues: issueElements.current,
        dimensionLines: dimensionLines.current,
        dimensionArrows: dimensionArrows.current,
        highlight: highlight.current,
        rubberBand: rubberBand.current,
        markers: markers.current,
        box: box.current,
        inference: inference.current,
        drawInputs: drawInputs.current,
        editor: editor.current,
        placing: placing.current,
      })
    })
    return () => {
      controller.setPainter(null)
    }
  }, [controller])

  // Elements React just rendered get their positions before the browser paints.
  useLayoutEffect(() => {
    controller?.repaint()
  })

  return (
    <div
      ref={layer}
      data-testid="sketch-overlay"
      className="pointer-events-none absolute inset-0 overflow-hidden"
    >
      <svg className="absolute inset-0 size-full overflow-visible">
        <path
          ref={dimensionLines}
          aria-hidden="true"
          fill="none"
          stroke={colors.sketchReference}
          strokeWidth={1}
        />
        <path ref={dimensionArrows} aria-hidden="true" fill={colors.sketchReference} />
        <path
          ref={highlight}
          aria-hidden="true"
          fill="none"
          stroke={colors.highlight}
          strokeWidth={2.5 * uiScale}
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <path
          ref={rubberBand}
          aria-hidden="true"
          data-testid="rubber-band"
          fill="none"
          stroke={colors.sketchUnder}
          strokeWidth={1.5 * uiScale}
          strokeDasharray={model.construction ? '6 4' : undefined}
          strokeLinejoin="round"
        />
        <path
          ref={markers}
          aria-hidden="true"
          fill="none"
          stroke={colors.highlight}
          strokeWidth={1.5 * uiScale}
        />
        {rings.map((ring, index) => (
          <circle
            // Rings follow the solution's findings, which have no ids (sketch.md 3.13.7).
            // oxlint-disable-next-line react/no-array-index-key
            key={index}
            ref={(element) => {
              if (element === null) {
                return undefined
              }
              issueElements.current.set(index, element)
              return () => {
                issueElements.current.delete(index)
              }
            }}
            data-testid="profile-issue"
            r={6}
            fill="none"
            strokeWidth={1.5}
            visibility="hidden"
            className={COLOR_CLASSES.errorMark.stroke}
          >
            <title>{ring.title}</title>
          </circle>
        ))}
        {model.box === null ? null : (
          <rect
            ref={box}
            data-testid="box-selection"
            data-mode={model.box}
            fill="none"
            strokeWidth={1}
            strokeDasharray={model.box === 'crossing' ? '4 3' : undefined}
            className={COLOR_CLASSES.accent.stroke}
          />
        )}
      </svg>
      {showConstraints && solution !== null
        ? glyphConstraints.map((constraint) => (
            <ConstraintGlyph
              key={constraint.id}
              constraint={constraint}
              solution={solution}
              selected={selected.has(`constraint:${sketchId}:${constraint.id}`)}
              controller={controller}
              elements={glyphElements}
            />
          ))
        : null}
      {solution === null
        ? null
        : dimensions.map((constraint) => (
            <DimensionLabel
              key={constraint.id}
              constraint={constraint}
              solution={solution}
              units={units}
              selected={selected.has(`constraint:${sketchId}:${constraint.id}`)}
              controller={controller}
              elements={labelElements}
            />
          ))}
      {model.placing === null ? null : (
        <span
          ref={placing}
          className={clsx(
            'absolute top-0 left-0 rounded-sm px-1 text-xs whitespace-nowrap opacity-90',
            FONT_CLASSES.values,
            COLOR_CLASSES.surfaceRaised.background,
            COLOR_CLASSES.text.foreground,
          )}
        >
          {model.placing}
        </span>
      )}
      {model.inference.length === 0 ? null : (
        <div ref={inference} className="absolute top-0 left-0 flex gap-1">
          {model.inference.map((type, index) => (
            // Glyphs are listed snap first, then direction; a type can appear once per role.
            // oxlint-disable-next-line react/no-array-index-key
            <InferenceGlyph key={index} type={type} />
          ))}
        </div>
      )}
      {controller === null || model.drawInputs.length === 0 ? null : (
        <DrawInputs
          ref={drawInputs}
          controller={controller}
          inputs={model.drawInputs}
          focusRequest={model.focusRequest}
        />
      )}
      {controller === null || model.editor === null ? null : (
        <DimensionEditor ref={editor} controller={controller} editor={model.editor} />
      )}
    </div>
  )
}

function InferenceGlyph({ type }: { readonly type: ConstraintType }) {
  const Icon = CONSTRAINT_ICONS[type]
  return (
    <span
      data-testid="inference-glyph"
      data-type={type}
      className={clsx(
        'flex size-4.5 items-center justify-center rounded-sm border opacity-90',
        COLOR_CLASSES.surfaceRaised.background,
        COLOR_CLASSES.border.border,
        COLOR_CLASSES.text.foreground,
      )}
    >
      <Icon aria-hidden="true" strokeWidth={1.75} className="size-3.5" />
    </span>
  )
}

/** Registers an element under an id for the painter while it is mounted. */
function register(
  elements: RefObject<Map<string, HTMLElement>>,
  id: string,
): (element: HTMLElement | null) => (() => void) | undefined {
  return (element) => {
    if (element === null) {
      return undefined
    }
    elements.current.set(id, element)
    return () => {
      elements.current.delete(id)
    }
  }
}

type GlyphProps = {
  readonly constraint: GlyphConstraint
  readonly solution: SketchSolution
  readonly selected: boolean
  readonly controller: SketchController | null
  readonly elements: RefObject<Map<string, HTMLElement>>
}

/** A constraint glyph (3.16.2): a chip with the constraint's icon in its solve state. */
function ConstraintGlyph({ constraint, solution, selected, controller, elements }: GlyphProps) {
  const Icon = CONSTRAINT_ICONS[constraint.type]
  const state =
    solution.constraints.find((entry) => entry.id === constraint.id)?.state ?? 'satisfied'
  const title = constraintTitle(constraint, solution, 'mm')
  return (
    <button
      ref={register(elements, constraint.id)}
      type="button"
      data-testid="constraint-glyph"
      data-constraint-id={constraint.id}
      data-type={constraint.type}
      data-state={state}
      aria-label={title}
      aria-pressed={selected}
      onClick={(event) => {
        controller?.selectConstraint(constraint.id, toggles(event))
      }}
      onPointerEnter={() => {
        controller?.hoverConstraint(constraint.id)
      }}
      onPointerLeave={() => {
        controller?.hoverConstraint(null)
      }}
      onFocus={() => {
        controller?.hoverConstraint(constraint.id)
      }}
      onBlur={() => {
        controller?.hoverConstraint(null)
      }}
      className={clsx(
        'pointer-events-auto absolute top-0 left-0 flex size-4.5 items-center justify-center rounded-sm border',
        COLOR_CLASSES.focusRing.outline,
        selected ? COLOR_CLASSES.accent.border : COLOR_CLASSES.border.border,
        (state === 'satisfied' || state === 'unsatisfied') && 'opacity-90',
        (state === 'satisfied' || state === 'unsatisfied') &&
          COLOR_CLASSES.surfaceRaised.background,
        (state === 'satisfied' || state === 'unsatisfied') && COLOR_CLASSES.textMuted.foreground,
        state === 'redundant' && COLOR_CLASSES.warningFill.background,
        state === 'redundant' && COLOR_CLASSES.warningFill.foreground,
        state === 'conflicting' && COLOR_CLASSES.errorFill.background,
        state === 'conflicting' && COLOR_CLASSES.errorFill.foreground,
      )}
    >
      <Icon aria-hidden="true" strokeWidth={1.75} className="size-3.5" />
    </button>
  )
}

type LabelProps = {
  readonly constraint: DimensionConstraint
  readonly solution: SketchSolution
  readonly units: Units
  readonly selected: boolean
  readonly controller: SketchController | null
  readonly elements: RefObject<Map<string, HTMLElement>>
}

/** A dimension's label (3.16.2): its solved value; double-click or Enter edits it. */
function DimensionLabel({
  constraint,
  solution,
  units,
  selected,
  controller,
  elements,
}: LabelProps) {
  const state = solution.constraints.find((entry) => entry.id === constraint.id)?.state
  const text = dimensionLabelText(constraint, solvedValue(constraint, solution), units)
  const plain = state !== 'conflicting' && state !== 'redundant'
  return (
    <button
      ref={register(elements, constraint.id)}
      type="button"
      data-testid="sketch-dimension"
      data-constraint-id={constraint.id}
      data-type={constraint.type}
      data-driving={String(constraint.driving !== false)}
      aria-label={constraintTitle(constraint, solution, units)}
      aria-pressed={selected}
      onClick={(event) => {
        controller?.selectConstraint(constraint.id, toggles(event))
      }}
      onDoubleClick={() => {
        controller?.editDimension(constraint.id)
      }}
      onKeyDown={(event) => {
        if (event.key === 'Enter') {
          event.preventDefault()
          controller?.editDimension(constraint.id)
        }
      }}
      onPointerEnter={() => {
        controller?.hoverConstraint(constraint.id)
      }}
      onPointerLeave={() => {
        controller?.hoverConstraint(null)
      }}
      className={clsx(
        'pointer-events-auto absolute top-0 left-0 rounded-sm border px-1 text-xs whitespace-nowrap',
        FONT_CLASSES.values,
        COLOR_CLASSES.focusRing.outline,
        selected ? COLOR_CLASSES.accent.border : 'border-transparent',
        plain && 'opacity-90',
        plain && COLOR_CLASSES.surfaceRaised.background,
        plain && COLOR_CLASSES.text.foreground,
        state === 'redundant' && COLOR_CLASSES.warningFill.background,
        state === 'redundant' && COLOR_CLASSES.warningFill.foreground,
        state === 'conflicting' && COLOR_CLASSES.errorFill.background,
        state === 'conflicting' && COLOR_CLASSES.errorFill.foreground,
      )}
    >
      {text}
    </button>
  )
}
