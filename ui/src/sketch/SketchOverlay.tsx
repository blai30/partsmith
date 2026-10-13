import { clsx } from 'clsx/lite'
import {
  memo,
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
import { useShallow } from 'zustand/react/shallow'
import { sketchParamsOf } from '../app/actions/sketch'
import { useApp, useAppState } from '../app/AppProvider'
import { CONSTRAINT_ICONS } from '../commands/icons'
import type { ConstraintSolveState, SketchConstraint, SketchSolution } from '../generated/protocol'
import { useResolvedTheme } from '../layout/use-resolved-theme'
import { selectEditedSketch } from '../state/selectors'
import { COLOR_CLASSES, FONT_CLASSES, VIEWPORT_COLORS } from '../theme/roles'
import { positionElement } from '../viewport/overlay'
import { hasGlyph } from './annotations'
import {
  CONSTRAINT_TITLES,
  DIMENSION_TITLES,
  isDimension,
  type ConstraintType,
} from './constraints'
import { DimensionEditor } from './DimensionEditor'
import { DrawInputs } from './DrawInputs'
import type { Vector2 } from './geometry'
import { issueRings } from './issues'
import type { DrawInputsPlacement } from './screen'
import {
  createSketchController,
  EMPTY_SKETCH_MODEL,
  type OverlayPaint,
  type SketchController,
} from './session'
import { SketchHeader } from './SketchHeader'

// Sketch mode over the view (ui.md 3.2, 3.12, 3.16): the sketch header and hint in the top-center
// slot and, while a sketch is edited, its overlay: constraint glyphs, dimensions, profile issue
// rings, the rubber band, inference glyphs, draw inputs, the dimension editor and the selection
// box. React renders the elements; the sketch controller writes their positions and the dimension
// values each frame, so a drag re-renders nothing (ADR 0002).

type GlyphConstraint = Exclude<SketchConstraint, { expr: string }>
type DimensionConstraint = Extract<SketchConstraint, { expr: string }>
type ConstraintStates = Readonly<Record<string, ConstraintSolveState>>

/** The elements the painter writes positions, path data and text into. */
type OverlayElements = {
  readonly glyphs: ReadonlyMap<string, HTMLElement>
  readonly labels: ReadonlyMap<string, HTMLElement>
  readonly labelTexts: ReadonlyMap<string, HTMLElement>
  readonly issues: ReadonlyMap<number, SVGCircleElement>
  readonly dimensionLines: SVGPathElement | null
  readonly dimensionArrows: SVGPathElement | null
  readonly highlight: SVGPathElement | null
  readonly rubberBand: SVGPathElement | null
  readonly markers: SVGPathElement | null
  readonly box: SVGRectElement | null
  readonly inference: HTMLDivElement | null
  readonly drawInputs: HTMLFieldSetElement | null
  readonly editor: HTMLDivElement | null
  readonly placing: HTMLDivElement | null
}

/**
 * Places an element at a canvas pixel, or hides it. Elements centered on their anchor translate
 * their content back by half its size with classes, so nothing is measured (3.12).
 */
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

/**
 * Places the draw inputs at their corner; `data-left` and `data-above` say which way they extend,
 * which their content's translate classes follow (3.16.4).
 */
function placeDrawInputs(element: HTMLElement | null, placement: DrawInputsPlacement | null): void {
  placeAt(element, placement?.at ?? null)
  if (element !== null && placement !== null) {
    element.toggleAttribute('data-left', placement.left)
    element.toggleAttribute('data-above', placement.above)
  }
}

function setAttribute(element: Element | null, name: string, value: string): void {
  if (element !== null && element.getAttribute(name) !== value) {
    element.setAttribute(name, value)
  }
}

/** Writes a frame's positions, path data and dimension values into the overlay (3.12). */
function paintOverlay(paint: OverlayPaint, elements: OverlayElements): void {
  for (const [id, element] of elements.glyphs) {
    placeAt(element, paint.glyphs.get(id) ?? null)
  }
  for (const [id, element] of elements.labels) {
    placeAt(element, paint.labels.get(id) ?? null)
  }
  for (const [id, element] of elements.labelTexts) {
    const text = paint.labelTexts.get(id) ?? ''
    if (element.textContent !== text) {
      element.textContent = text
    }
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
  placeDrawInputs(elements.drawInputs, paint.drawInputs)
  placeAt(elements.editor, paint.editor)
  placeAt(elements.placing, paint.placing)
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

/** The solve state of each constraint by id. */
function constraintStatesOf(solution: SketchSolution | undefined): ConstraintStates {
  return Object.fromEntries((solution?.constraints ?? []).map((entry) => [entry.id, entry.state]))
}

/** The titles of the profile issue rings, in `issueRings` order. */
function ringTitlesOf(solution: SketchSolution | undefined): readonly string[] {
  return solution === undefined ? [] : issueRings(solution).map((ring) => ring.title)
}

/**
 * Sketch mode over the view: the sketch header and hint while a plane is picked, and the sketch
 * overlay with its controller while a sketch is edited; nothing outside sketch mode. It is the one
 * sketch element the viewport holds.
 */
export function SketchOverlay() {
  const sketchId = useAppState(selectEditedSketch)
  const [controller, setController] = useState<SketchController | null>(null)
  const model = useSyncExternalStore(
    controller?.subscribe ?? subscribeNothing,
    controller?.getModel ?? emptyModel,
  )
  const editing = sketchId !== undefined && sketchId !== null
  // The header and its hint keep one place in the tree from plane picking to editing and back,
  // so the hint's live region is never replaced and each step's prompt is announced (3.24).
  return (
    <>
      <SketchHeader
        hint={editing ? model.hint : null}
        freePlacement={editing && model.freePlacement}
      />
      {editing ? (
        <SketchLayer key={sketchId} sketchId={sketchId} onController={setController} />
      ) : null}
    </>
  )
}

type SketchLayerProps = {
  readonly sketchId: string
  /** Receives the layer's controller once it exists, and null when the layer goes. */
  readonly onController: (controller: SketchController | null) => void
}

function SketchLayer({ sketchId, onController }: SketchLayerProps) {
  const app = useApp()
  const layer = useRef<HTMLDivElement>(null)
  const [controller, setController] = useState<SketchController | null>(null)
  const model = useSyncExternalStore(
    controller?.subscribe ?? subscribeNothing,
    controller?.getModel ?? emptyModel,
  )
  // Slices compared by value: a drag replaces the solution every frame, and only a changed
  // constraint state or outline problem renders again.
  const solved = useAppState((state) => state.document?.sketches[sketchId] !== undefined)
  const states = useAppState(
    useShallow((state) => constraintStatesOf(state.document?.sketches[sketchId]?.solution)),
  )
  const ringTitles = useAppState(
    useShallow((state) => ringTitlesOf(state.document?.sketches[sketchId]?.solution)),
  )
  const record = useAppState(
    (state) => state.document?.recipe.features?.find((feature) => feature.id === sketchId) ?? null,
  )
  const showConstraints = useAppState((state) => state.sketch?.showConstraints ?? true)
  const selection = useAppState((state) => state.sketch?.selection ?? null)
  const theme = useResolvedTheme()
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
  const selected = useMemo(() => new Set((selection ?? []).map((item) => item.key)), [selection])

  const glyphElements = useRef(new Map<string, HTMLElement>())
  const labelElements = useRef(new Map<string, HTMLElement>())
  const labelTextElements = useRef(new Map<string, HTMLElement>())
  const issueElements = useRef(new Map<number, SVGCircleElement>())
  const dimensionLines = useRef<SVGPathElement>(null)
  const dimensionArrows = useRef<SVGPathElement>(null)
  const highlight = useRef<SVGPathElement>(null)
  const rubberBand = useRef<SVGPathElement>(null)
  const markers = useRef<SVGPathElement>(null)
  const box = useRef<SVGRectElement>(null)
  const inference = useRef<HTMLDivElement>(null)
  const drawInputs = useRef<HTMLFieldSetElement>(null)
  const editor = useRef<HTMLDivElement>(null)
  const placing = useRef<HTMLDivElement>(null)

  // The controller reads pointer input on the viewport's canvas, beside the viewport controller.
  useEffect(() => {
    const canvas =
      layer.current?.closest('[data-testid="viewport"]')?.querySelector('canvas') ?? null
    if (canvas === null) {
      console.error('The sketch overlay needs the viewport canvas: render it inside the Viewport.')
      return undefined
    }
    const created = createSketchController({ app, canvas })
    setController(created)
    onController(created)
    return () => {
      created.dispose()
      setController(null)
      onController(null)
    }
  }, [app, onController])

  useLayoutEffect(() => {
    if (controller === null) {
      return undefined
    }
    controller.setPainter((paint) => {
      paintOverlay(paint, {
        glyphs: glyphElements.current,
        labels: labelElements.current,
        labelTexts: labelTextElements.current,
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
    <>
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
          {ringTitles.map((title, index) => (
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
              <title>{title}</title>
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
        {showConstraints && solved
          ? glyphConstraints.map((constraint) => (
              <ConstraintGlyph
                key={constraint.id}
                constraint={constraint}
                state={states[constraint.id] ?? 'satisfied'}
                selected={selected.has(`constraint:${sketchId}:${constraint.id}`)}
                controller={controller}
                elements={glyphElements}
              />
            ))
          : null}
        {solved
          ? dimensions.map((constraint) => (
              <DimensionLabel
                key={constraint.id}
                constraint={constraint}
                state={states[constraint.id] ?? 'satisfied'}
                selected={selected.has(`constraint:${sketchId}:${constraint.id}`)}
                controller={controller}
                elements={labelElements}
                texts={labelTextElements}
              />
            ))
          : null}
        {model.placing === null ? null : (
          <div ref={placing} hidden className="absolute top-0 left-0">
            <span
              className={clsx(
                'block -translate-x-1/2 -translate-y-1/2 rounded-sm px-1 text-xs whitespace-nowrap opacity-90',
                FONT_CLASSES.values,
                COLOR_CLASSES.surfaceRaised.background,
                COLOR_CLASSES.text.foreground,
              )}
            >
              {model.placing}
            </span>
          </div>
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
          <div ref={editor} hidden className="absolute top-0 left-0">
            <DimensionEditor controller={controller} editor={model.editor} />
          </div>
        )}
      </div>
    </>
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
  readonly state: ConstraintSolveState
  readonly selected: boolean
  readonly controller: SketchController | null
  readonly elements: RefObject<Map<string, HTMLElement>>
}

/**
 * A constraint glyph (3.16.2): a chip with the constraint's icon in its solve state, centered on
 * the position the painter gives its wrapper.
 */
const ConstraintGlyph = memo(function ConstraintGlyph({
  constraint,
  state,
  selected,
  controller,
  elements,
}: GlyphProps) {
  const Icon = CONSTRAINT_ICONS[constraint.type]
  const muted = state === 'satisfied' || state === 'unsatisfied'
  return (
    <div ref={register(elements, constraint.id)} hidden className="absolute top-0 left-0">
      <button
        type="button"
        data-testid="constraint-glyph"
        data-constraint-id={constraint.id}
        data-type={constraint.type}
        data-state={state}
        aria-label={CONSTRAINT_TITLES[constraint.type]}
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
          'pointer-events-auto flex size-4.5 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-sm border',
          COLOR_CLASSES.focusRing.outline,
          selected ? COLOR_CLASSES.accent.border : COLOR_CLASSES.border.border,
          muted && 'opacity-90',
          muted && COLOR_CLASSES.surfaceRaised.background,
          muted && COLOR_CLASSES.textMuted.foreground,
          state === 'redundant' && COLOR_CLASSES.warningFill.background,
          state === 'redundant' && COLOR_CLASSES.warningFill.foreground,
          state === 'conflicting' && COLOR_CLASSES.errorFill.background,
          state === 'conflicting' && COLOR_CLASSES.errorFill.foreground,
        )}
      >
        <Icon aria-hidden="true" strokeWidth={1.75} className="size-3.5" />
      </button>
    </div>
  )
})

type LabelProps = {
  readonly constraint: DimensionConstraint
  readonly state: ConstraintSolveState
  readonly selected: boolean
  readonly controller: SketchController | null
  readonly elements: RefObject<Map<string, HTMLElement>>
  readonly texts: RefObject<Map<string, HTMLElement>>
}

/**
 * A dimension's label (3.16.2): its solved value, which the painter writes because a drag changes
 * it every frame; the accessible name is the dimension's kind followed by that text, so it holds
 * the visible label. Double-click or Enter edits it.
 */
const DimensionLabel = memo(function DimensionLabel({
  constraint,
  state,
  selected,
  controller,
  elements,
  texts,
}: LabelProps) {
  const plain = state !== 'conflicting' && state !== 'redundant'
  return (
    <div ref={register(elements, constraint.id)} hidden className="absolute top-0 left-0">
      <button
        type="button"
        data-testid="sketch-dimension"
        data-constraint-id={constraint.id}
        data-type={constraint.type}
        data-driving={String(constraint.driving !== false)}
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
          'pointer-events-auto block -translate-x-1/2 -translate-y-1/2 rounded-sm border px-1 text-xs whitespace-nowrap',
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
        <span className="sr-only">{DIMENSION_TITLES[constraint.type]}</span>{' '}
        <span ref={register(texts, constraint.id)} />
      </button>
    </div>
  )
})
