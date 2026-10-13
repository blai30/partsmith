import {
  addSketchDimension,
  addSketchShape,
  applySketchConstraints,
  changeSketchDimension,
  createSketchIds,
  selectInSketch,
  sketchParamsOf,
  solutionOf,
  startSketchDrag,
  type SketchDrag,
} from '../app/actions/sketch'
import type { App, Platform } from '../app/createApp'
import { CoreError } from '../core/client'
import { createLatestOnly, type LatestOnly } from '../core/queue'
import { convertForField } from '../expression/evaluate'
import { formatNumber } from '../expression/format'
import type {
  Dimension,
  DocHandle,
  PointRef,
  SketchConstraint,
  SketchSolution,
} from '../generated/protocol'
import { MESSAGES } from '../messages'
import type { AppState, SelectionItem, SketchSession } from '../state/store'
import type { Camera } from '../viewport/camera'
import type { ViewportController, ViewportHandle } from '../viewport/controller'
import { dimensionGeometry, dimensionLabelText, hasGlyph } from './annotations'
import {
  constraintReferences,
  constraintRequirement,
  constraintsFor,
  constraintTitle,
  isDimension,
  pickVerdict,
  type ConstraintType,
} from './constraints'
import {
  DIMENSION_START,
  dimensionClick,
  draftPreview,
  dimensionPreview,
  pickedItems,
  type DimensionDraft,
  type DimensionPicking,
} from './dimension-tool'
import { defaultLabelPosition, dimensionFor } from './dimensions'
import {
  drawClick,
  drawFields,
  drawPreview,
  IDLE,
  type DrawFieldName,
  type DrawingTool,
  type DrawInput,
  type DrawPreview,
  type DrawState,
  type TypedValue,
} from './drawing'
import { distance, type Vector2 } from './geometry'
import {
  constraintSelectionItem,
  entitySelectionItem,
  hitSelectionItem,
  hitTest,
  type SketchHit,
} from './hit'
import { issueRings } from './issues'
import { ownerOf, pointPosition, solvedEntity } from './operands'
import { millimetersPerPixelAt, pixelToSketch, sketchToPixel, type CanvasSize } from './placement'
import type { SketchAddition } from './recipes'
import { isAxis, ORIGIN_POINT } from './refs'
import {
  circlePath,
  dimensionPaths,
  drawInputsPlacement,
  entitiesInBox,
  entityPoints,
  glyphCenters,
  polylinePath,
  projectPolyline,
  screenBox,
  shapePoints,
  type DrawInputsPlacement,
  type Projector,
} from './screen'
import { constraintTool, isDrawingTool } from './tools'

// Sketch mode's session (ui.md 2.14, 3.16): its tools and the changes they commit, the step
// prompts, the profile problems drawn on the sketch, and the internal sketch controller that turns
// pointer and key input on the canvas into drawing, constraining, dimensioning, dragging and box
// selection, bypassing React (ADR 0002).

/** A sketch tool: a drawing tool, the dimension tool, or a constraint tool in pick mode. */
export type SketchToolId =
  | 'line'
  | 'rectangle_2point'
  | 'rectangle_center'
  | 'circle'
  | 'arc_3point'
  | 'point'
  | 'dimension'
  | `constrain_${ConstraintType}`

/** One edit of the sketch being edited, committed as one undo entry by `commitSketchChange`. */
export type SketchChange = {
  readonly add?: SketchAddition
  readonly removeEntities?: readonly string[]
  readonly removeConstraints?: readonly string[]
  readonly toggleConstruction?: readonly string[]
  readonly replaceConstraint?: SketchConstraint
  /** Constraint ids the user asked for; refused when the probe finds them redundant or conflicting. */
  readonly requested: readonly string[]
  /** Constraint ids added by inference; dropped silently when redundant or conflicting. */
  readonly inferred: readonly string[]
}

/** What became of a sketch change. */
export type SketchCommitResult =
  | { readonly outcome: 'committed'; readonly revision: number }
  | {
      readonly outcome: 'over_constrained'
      readonly constraint: string
      readonly conflicting: readonly string[]
      readonly redundant: boolean
    }
  | { readonly outcome: 'failed'; readonly message: string }

// Step prompts.

/** The step the sketch session waits for, which picks the step prompt (3.16.3). */
export type SketchStep =
  | { readonly kind: 'idle'; readonly empty: boolean }
  | { readonly kind: 'line'; readonly chain: boolean }
  | { readonly kind: 'rectangle_2point' | 'rectangle_center' | 'circle'; readonly clicks: 0 | 1 }
  | { readonly kind: 'arc_3point'; readonly clicks: 0 | 1 | 2 }
  | { readonly kind: 'point' }
  | {
      readonly kind: 'dimension'
      readonly picked: 'nothing' | 'line' | 'point' | 'round' | 'pair'
    }

/** The step prompt for a step, shortcuts rendered with formatShortcut, or null when there is none. */
export function sketchStepHint(step: SketchStep, platform: Platform): string | null {
  switch (step.kind) {
    case 'idle':
      return step.empty ? MESSAGES.sketch_step_start(platform) : null
    case 'line':
      return step.chain
        ? MESSAGES.sketch_step_line_next(platform)
        : MESSAGES.sketch_step_line_start()
    case 'rectangle_2point':
      return step.clicks === 0
        ? MESSAGES.sketch_step_corner()
        : MESSAGES.sketch_step_opposite_corner()
    case 'rectangle_center':
      return step.clicks === 0
        ? MESSAGES.sketch_step_center()
        : MESSAGES.sketch_step_rectangle_corner()
    case 'circle':
      return step.clicks === 0 ? MESSAGES.sketch_step_center() : MESSAGES.sketch_step_circle_size()
    case 'arc_3point':
      return (
        [
          MESSAGES.sketch_step_arc_start(),
          MESSAGES.sketch_step_arc_end(),
          MESSAGES.sketch_step_arc_shape(),
        ][step.clicks] ?? null
      )
    case 'point':
      return MESSAGES.sketch_step_point()
    case 'dimension':
      switch (step.picked) {
        case 'nothing':
          return MESSAGES.sketch_step_dimension()
        case 'line':
          return MESSAGES.sketch_step_dimension_place()
        case 'point':
          return MESSAGES.sketch_step_dimension_second()
        case 'round':
        case 'pair':
          return MESSAGES.sketch_step_dimension_label()
      }
  }
}

// Profile problems.

// The overlay the controller drives.

/** A draw input (3.16.4): its quantity, the typed text and why the text is not a value, if it is not. */
export type DrawInputModel = {
  readonly name: DrawFieldName
  readonly unit: 'length' | 'angle'
  readonly text: string
  readonly error: string | null
}

/** The open dimension editor (3.16.7). */
export type DimensionEditorModel = {
  /** Changes each time an editor opens, so its input starts fresh. */
  readonly key: number
  readonly mode: 'new' | 'edit'
  readonly text: string
  readonly unit: 'length' | 'angle'
  /** The Diameter | Radius choice of a new dimension on a circle or arc, else null. */
  readonly round: 'diameter' | 'radius' | null
  readonly reference: boolean
  /** The over-constrained offer of a new driving dimension, or the unsolvable value of an edit. */
  readonly problem: 'over_constrained' | 'unsolvable' | null
  /** Why the core refused the dimension otherwise, or null. */
  readonly message: string | null
  readonly busy: boolean
}

/** What the overlay renders; it changes only when its structure does, not on every pointer move. */
export type SketchOverlayModel = {
  /**
   * The active tool's sketch hint (3.16.3): its step prompt, a constraint tool's requirement, or a
   * warning (a refused pick, collinear arc points); null with no tool.
   */
  readonly hint: { readonly text: string; readonly warning: boolean } | null
  /** Whether inference glyphs show, so the hint names the key that places freely. */
  readonly freePlacement: boolean
  /** One inference glyph per inferred constraint, snap first (3.16.5). */
  readonly inference: readonly ConstraintType[]
  readonly drawInputs: readonly DrawInputModel[]
  /** Increases each time typing asks for the first draw input to take focus. */
  readonly focusRequest: number
  readonly construction: boolean
  /** The label text of the dimension being placed, or null. */
  readonly placing: string | null
  readonly editor: DimensionEditorModel | null
  readonly box: 'window' | 'crossing' | null
  readonly hovered: string | null
}

/** A rectangle on the canvas in CSS pixels. */
export type PixelRectangle = {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}

/** Where the overlay's elements go this frame, in canvas CSS pixels (3.12 runtime positions). */
export type OverlayPaint = {
  /** Center of each constraint glyph by constraint id; null hides it. */
  readonly glyphs: ReadonlyMap<string, Vector2 | null>
  /** Center of each dimension label by constraint id; null hides it. */
  readonly labels: ReadonlyMap<string, Vector2 | null>
  /** Text of each dimension label by constraint id: its solved value changes during a drag. */
  readonly labelTexts: ReadonlyMap<string, string>
  /** Center of each profile issue ring, in `issueRings` order. */
  readonly issues: readonly (Vector2 | null)[]
  /** SVG path data: dimension lines (stroked) and their arrowheads (filled). */
  readonly dimensionLines: string
  readonly dimensionArrows: string
  /** SVG path data of the rubber band. */
  readonly rubberBand: string
  /** SVG path data stroked in `highlight`: the snap curve and the hovered constraint's curves. */
  readonly highlight: string
  /** SVG path data of point markers in `highlight`: the snap point and constrained points. */
  readonly markers: string
  /** Top left of the inference glyph row and the dimension editor. */
  readonly inference: Vector2 | null
  /** Where the draw inputs go, beside the rubber band and away from the pointer. */
  readonly drawInputs: DrawInputsPlacement | null
  readonly editor: Vector2 | null
  /** Center of the label of the dimension being placed. */
  readonly placing: Vector2 | null
  readonly box: PixelRectangle | null
  /** The rubber band's quantities as text, shown in empty draw inputs. */
  readonly measures: Readonly<Partial<Record<DrawFieldName, string>>>
}

export type SketchController = {
  /** React reads the model through this pair (`useSyncExternalStore`). */
  subscribe(listener: () => void): () => void
  getModel(): SketchOverlayModel
  /** Sets the function that writes each frame's positions into the overlay's elements. */
  setPainter(painter: ((paint: OverlayPaint) => void) | null): void
  /** Paints now, after React rendered new overlay elements. */
  repaint(): void
  setDrawText(name: DrawFieldName, text: string): void
  /** Commits the shape with the typed values (Enter in a draw input). */
  commitDrawInputs(): Promise<void>
  /** Clears the draw inputs and gives focus back to the view (Esc in a draw input). */
  clearDrawInputs(): void
  hoverConstraint(constraint: string | null): void
  selectConstraint(constraint: string, toggle: boolean): void
  /** Opens the editor on a stored dimension (double-click or Enter on its label). */
  editDimension(constraint: string): void
  setEditorText(text: string): void
  setEditorReference(reference: boolean): void
  setEditorRound(round: 'diameter' | 'radius'): void
  /** Adds or changes the dimension with the editor's text; `reference` forces a reference dimension. */
  commitEditor(options?: { readonly reference?: boolean }): Promise<void>
  closeEditor(): void
  dispose(): void
}

export type SketchControllerOptions = {
  readonly app: App
  /** The view's canvas, whose pointer input the controller reads alongside the viewport controller. */
  readonly canvas: HTMLCanvasElement
}

/** A press moves more than this many CSS pixels to drag a point or draw a selection box (3.16.9). */
const DRAG_THRESHOLD_PIXELS = 3
/** A press released within this many CSS pixels of where it started is a click (3.6). */
const CLICK_DISTANCE_PIXELS = 4
/** The chord tolerance of curves the overlay draws, in CSS pixels (3.16.2). */
const TESSELLATION_PIXELS = 0.25
/** Inference glyphs sit this many CSS pixels (times the UI scale) right of and below the cursor. */
const INFERENCE_OFFSET_PIXELS = 16
/** Draw inputs sit this many CSS pixels (times the UI scale) off the rubber band's midpoint, away from the pointer. */
const DRAW_INPUT_OFFSET_PIXELS = 12
/** Dimension arrowheads are this many CSS pixels long (3.16.2). */
const ARROW_PIXELS = 8
/** Radius of point markers in CSS pixels (times the UI scale). */
const MARKER_PIXELS = 5
/** How long a refused pick's message stays in the sketch hint. */
const FLASH_MILLISECONDS = 3_000
/** Characters that start typing a value into the first draw input (3.16.4). */
const VALUE_START = /^[\d.\-(]$/

const EMPTY_PAINT: OverlayPaint = {
  glyphs: new Map(),
  labels: new Map(),
  labelTexts: new Map(),
  issues: [],
  dimensionLines: '',
  dimensionArrows: '',
  rubberBand: '',
  highlight: '',
  markers: '',
  inference: null,
  drawInputs: null,
  editor: null,
  placing: null,
  box: null,
  measures: {},
}

export const EMPTY_SKETCH_MODEL: SketchOverlayModel = {
  hint: null,
  freePlacement: false,
  inference: [],
  drawInputs: [],
  focusRequest: 0,
  construction: false,
  placing: null,
  editor: null,
  box: null,
  hovered: null,
}

type EditingSession = SketchSession & { readonly sketchId: string }

/** The session of the sketch being edited, or null outside sketch mode and while picking a plane. */
function editingSession(state: AppState): EditingSession | null {
  const session = state.sketch
  if (state.context !== 'sketch' || session === null || session.sketchId === null) {
    return null
  }
  return { ...session, sketchId: session.sketchId }
}

type DimensionConstraint = Extract<SketchConstraint, { expr: string }>

/** What a change of cancels the operation in progress: the edited sketch and its tool. */
function sessionKey(state: AppState): string {
  const session = editingSession(state)
  return session === null ? '' : `${session.sketchId} ${session.tool ?? ''}`
}

/** The stored constraints of a sketch, in stored order. */
function storedConstraints(state: AppState, sketchId: string): readonly SketchConstraint[] {
  const record = state.document?.recipe.features?.find((feature) => feature.id === sketchId)
  const params = record?.type === 'sketch' ? sketchParamsOf(record) : null
  return params?.constraints ?? []
}

/** An input of a text-like type, a textarea, a select, or content-editable element. */
function isEditable(target: EventTarget | null): boolean {
  if (target instanceof HTMLInputElement) {
    return !['button', 'checkbox', 'radio', 'range', 'reset', 'submit', 'color', 'file'].includes(
      target.type,
    )
  }
  return (
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  )
}

/** Whether a menu or a light-dismiss popover is open, which takes Esc and Enter first (3.5). */
function hasOpenPopover(document: Document): boolean {
  return document.querySelector('[popover=""]:popover-open, [popover="auto"]:popover-open') !== null
}

/** Animation frames, or a timer where the environment has none. */
function requestFrame(callback: () => void): () => void {
  if (typeof requestAnimationFrame === 'function') {
    const id = requestAnimationFrame(callback)
    return () => {
      cancelAnimationFrame(id)
    }
  }
  const timer = setTimeout(callback, 16)
  return () => {
    clearTimeout(timer)
  }
}

/** A viewport that reports its rendered frames, so the overlay follows the camera in the same frame. */
type FrameSource = ViewportHandle & Pick<ViewportController, 'onRender'>

function hasFrames(handle: ViewportHandle): handle is FrameSource {
  return typeof Reflect.get(handle, 'onRender') === 'function'
}

function report(error: unknown): void {
  console.error('A sketch action failed.', error)
}

/** The unit of the value a dimension type takes. */
function dimensionUnit(type: string): 'length' | 'angle' {
  return type === 'angle' ? 'angle' : 'length'
}

/** The rectangle spanned by two canvas pixels. */
function pixelRectangle(start: Vector2, end: Vector2): PixelRectangle {
  const area = screenBox(start, end)
  return {
    x: area.left,
    y: area.top,
    width: area.right - area.left,
    height: area.bottom - area.top,
  }
}

/** A changed dimension that drives again: `driving: true` is the default sketch.md leaves out. */
function withoutDriving(constraint: DimensionConstraint): SketchConstraint {
  const { driving: _driving, ...rest } = constraint
  return rest
}

/** The point references and curve ids a constraint names, split into the two. */
function constrainedItems(
  constraint: SketchConstraint,
  solution: SketchSolution,
): { readonly points: readonly PointRef[]; readonly curves: readonly string[] } {
  const points: PointRef[] = []
  const curves: string[] = []
  for (const reference of constraintReferences(constraint)) {
    const owner = ownerOf(reference)
    const entity = solvedEntity(solution, owner)
    if (reference !== owner || reference === ORIGIN_POINT || entity?.type === 'point') {
      points.push(reference)
    } else if (!isAxis(reference)) {
      curves.push(reference)
    }
  }
  return { points, curves }
}

/** What the dimension tool has picked, for its step prompt. */
function dimensionPicked(
  picking: DimensionPicking,
): Extract<SketchStep, { kind: 'dimension' }>['picked'] {
  switch (picking.stage) {
    case 'first':
      return 'nothing'
    case 'line':
      return 'line'
    case 'point':
      return 'point'
    case 'place':
      return picking.picks.length === 1 ? 'round' : 'pair'
  }
}

/** Creates the controller of the sketch being edited; it lives as long as the sketch overlay. */
export function createSketchController(options: SketchControllerOptions): SketchController {
  const { app, canvas } = options
  const store = app.store
  const listeners = new Set<() => void>()
  const cleanups: (() => void)[] = []
  let model: SketchOverlayModel = EMPTY_SKETCH_MODEL
  let painter: ((paint: OverlayPaint) => void) | null = null
  let cancelPaint: (() => void) | null = null
  let disposed = false
  let framed: { readonly handle: FrameSource; readonly stop: () => void } | null = null

  // The pointer and the operation in progress.
  let pointer: Vector2 | null = null
  let suppressed = false
  let press: {
    readonly pointerId: number
    readonly start: Vector2
    readonly intent:
      | { readonly kind: 'drag'; readonly point: PointRef }
      | { readonly kind: 'box' }
      | { readonly kind: 'none' }
    readonly toggle: boolean
    moved: boolean
  } | null = null
  let drag: SketchDrag | null = null
  let box: { readonly start: Vector2; end: Vector2; readonly toggle: boolean } | null = null
  let notAllowed = false

  // Drawing tools and their draw inputs.
  let draw: DrawState = IDLE
  let texts: Partial<Record<DrawFieldName, string>> = {}
  let typed: Partial<Record<DrawFieldName, TypedValue>> = {}
  let inputErrors: Partial<Record<DrawFieldName, string>> = {}
  let focusRequest = 0
  const evaluations = new Map<DrawFieldName, LatestOnly<Evaluation>>()

  // The dimension tool and editor.
  let picking: DimensionPicking = DIMENSION_START
  let editorKey = 0
  let editor: Editor | null = null
  /** Labels placed this session; sketch.md stores no label positions. */
  const labelPositions = new Map<string, Vector2>()

  let hovered: string | null = null
  let flash: { readonly text: string; readonly timer: ReturnType<typeof setTimeout> } | null = null
  let last = sessionKey(store.getState())

  type Evaluation = {
    readonly documentHandle: DocHandle
    readonly name: DrawFieldName
    readonly unit: 'length' | 'angle'
    readonly text: string
  }

  type EditorBase = {
    readonly key: number
    readonly text: string
    readonly reference: boolean
    readonly labelAt: Vector2
    readonly problem: 'over_constrained' | 'unsolvable' | null
    readonly message: string | null
    readonly busy: boolean
  }

  type Editor =
    | (EditorBase & { readonly mode: 'new'; readonly draft: DimensionDraft })
    | (EditorBase & { readonly mode: 'edit'; readonly constraint: DimensionConstraint })

  // Reading the state and the view.

  function current(): EditingSession | null {
    return editingSession(store.getState())
  }

  function solution(): SketchSolution | null {
    const session = current()
    return session === null ? null : solutionOf(app, session.sketchId)
  }

  function uiScale(): number {
    return store.getState().settings.uiScale
  }

  function canvasSize(): CanvasSize {
    return { width: canvas.clientWidth, height: canvas.clientHeight }
  }

  function canvasPixel(event: MouseEvent): Vector2 {
    const rectangle = canvas.getBoundingClientRect()
    return [event.clientX - rectangle.left, event.clientY - rectangle.top]
  }

  /** The sketch point under a canvas pixel, or null when the view cannot reach the sketch plane. */
  function toSketch(pixel: Vector2): Vector2 | null {
    const camera = app.viewport()?.getCamera() ?? null
    const solved = solution()
    return camera === null || solved === null
      ? null
      : pixelToSketch(camera, canvasSize(), solved.placement, pixel)
  }

  function millimetersPerPixel(pixel: Vector2): number | null {
    const camera = app.viewport()?.getCamera() ?? null
    const solved = solution()
    return camera === null || solved === null
      ? null
      : millimetersPerPixelAt(camera, canvasSize(), solved.placement, pixel)
  }

  /** Sketch millimeters per CSS pixel at the canvas center, for tessellation and label offsets. */
  function centerPixelSize(solved: SketchSolution): number {
    const camera: Camera | null = app.viewport()?.getCamera() ?? null
    const size = canvasSize()
    const center: Vector2 = [size.width / 2, size.height / 2]
    return (
      (camera === null ? null : millimetersPerPixelAt(camera, size, solved.placement, center)) ?? 1
    )
  }

  function projector(): Projector | null {
    const camera = app.viewport()?.getCamera() ?? null
    const solved = solution()
    const size = canvasSize()
    if (camera === null || solved === null || !(size.width > 0) || !(size.height > 0)) {
      return null
    }
    return (point) => sketchToPixel(camera, size, solved.placement, point)
  }

  /** What the drawing tools read at a canvas pixel. */
  function drawInputAt(pixel: Vector2 | null): DrawInput | null {
    const solved = solution()
    const cursor = pixel === null ? null : toSketch(pixel)
    const scale = pixel === null ? null : millimetersPerPixel(pixel)
    if (solved === null || cursor === null || scale === null) {
      return null
    }
    return {
      cursor,
      millimetersPerPixel: scale,
      uiScale: uiScale(),
      suppressed,
      solution: solved,
      typed,
    }
  }

  /** The suppression key (Ctrl, or Cmd on macOS) of a pointer or key event (3.16.5). */
  function suppressing(event: MouseEvent | KeyboardEvent): boolean {
    return app.platform.operatingSystem === 'mac' ? event.metaKey : event.ctrlKey
  }

  function hitAt(pixel: Vector2, includeOrigin: boolean): SketchHit | null {
    const solved = solution()
    const cursor = toSketch(pixel)
    const scale = millimetersPerPixel(pixel)
    if (solved === null || cursor === null || scale === null) {
      return null
    }
    return hitTest(solved, cursor, {
      millimetersPerPixel: scale,
      uiScale: uiScale(),
      includeOrigin,
    })
  }

  function itemAt(pixel: Vector2): SelectionItem | null {
    const session = current()
    const solved = solution()
    const hit = hitAt(pixel, true)
    return session === null || solved === null || hit === null
      ? null
      : hitSelectionItem(session.sketchId, hit, solved)
  }

  function focusView(): void {
    canvas.closest<HTMLElement>('[tabindex]')?.focus()
  }

  // The model React renders.

  /** The step of the active tool; null with no tool or a constraint tool (3.16.3). */
  function toolStep(tool: SketchToolId | null): SketchStep | null {
    switch (tool) {
      case 'line':
        return { kind: 'line', chain: draw.stage === 'line' }
      case 'rectangle_2point':
      case 'rectangle_center':
      case 'circle':
        return { kind: tool, clicks: draw.stage === 'idle' ? 0 : 1 }
      case 'arc_3point':
        return {
          kind: 'arc_3point',
          clicks: draw.stage === 'idle' ? 0 : draw.stage === 'third' ? 2 : 1,
        }
      case 'point':
        return { kind: 'point' }
      case 'dimension':
        return { kind: 'dimension', picked: dimensionPicked(picking) }
      default:
        return null
    }
  }

  function preview(session: EditingSession): DrawPreview | null {
    const tool = session.tool
    if (!isDrawingTool(tool)) {
      return null
    }
    const input = drawInputAt(pointer)
    return input === null ? null : drawPreview(tool, draw, input)
  }

  function placingText(): string | null {
    const solved = solution()
    const cursor = pointer === null ? null : toSketch(pointer)
    if (current()?.tool !== 'dimension' || editor !== null || solved === null || cursor === null) {
      return null
    }
    const placed = dimensionPreview(picking, cursor, solved)
    if (placed === null || !isDimension(placed.constraint)) {
      return null
    }
    const units = store.getState().document?.recipe.units ?? 'mm'
    return dimensionLabelText(placed.constraint, placed.value, units)
  }

  function editorModel(): DimensionEditorModel | null {
    if (editor === null) {
      return null
    }
    const type = editor.mode === 'new' ? editor.draft.type : editor.constraint.type
    return {
      key: editor.key,
      mode: editor.mode,
      text: editor.text,
      unit: dimensionUnit(type),
      round: editor.mode === 'new' && (type === 'diameter' || type === 'radius') ? type : null,
      reference: editor.reference,
      problem: editor.problem,
      message: editor.message,
      busy: editor.busy,
    }
  }

  function computeModel(): SketchOverlayModel {
    const session = current()
    if (session === null) {
      return EMPTY_SKETCH_MODEL
    }
    const drawn = preview(session)
    const tool = session.tool
    const constraint = constraintTool(tool)
    const step = toolStep(tool)
    const prompt = step === null ? null : sketchStepHint(step, app.platform)
    const warning = flash?.text ?? drawn?.hint ?? null
    const fields = isDrawingTool(tool) ? drawFields(tool, draw) : []
    const inference = drawn?.glyphs ?? []
    return {
      hint:
        warning !== null
          ? { text: warning, warning: true }
          : constraint !== null
            ? { text: constraintRequirement(constraint), warning: false }
            : prompt === null
              ? null
              : { text: prompt, warning: false },
      freePlacement: inference.length > 0,
      inference,
      drawInputs: fields.map((field) => ({
        name: field.name,
        unit: field.unit,
        text: texts[field.name] ?? '',
        error: inputErrors[field.name] ?? null,
      })),
      focusRequest,
      construction: session.construction,
      placing: placingText(),
      editor: editorModel(),
      box: box === null ? null : box.end[0] >= box.start[0] ? 'window' : 'crossing',
      hovered,
    }
  }

  function publish(): void {
    const next = computeModel()
    if (JSON.stringify(next) === JSON.stringify(model)) {
      return
    }
    model = next
    for (const listener of Array.from(listeners)) {
      listener()
    }
  }

  // Painting.

  function computePaint(): OverlayPaint {
    const state = store.getState()
    const session = editingSession(state)
    const solved = solution()
    const project = projector()
    if (session === null || solved === null || project === null) {
      return EMPTY_PAINT
    }
    const scale = uiScale()
    const units = state.document?.recipe.units ?? 'mm'
    const pixelSize = centerPixelSize(solved)
    const tolerance = TESSELLATION_PIXELS * pixelSize
    const constraints = storedConstraints(state, session.sketchId)

    // Glyphs, dimensions and profile issues.
    const glyphs = session.showConstraints
      ? glyphCenters(constraints.filter(hasGlyph), solved, project, scale)
      : new Map<string, Vector2 | null>()
    const labels = new Map<string, Vector2 | null>()
    const labelTexts = new Map<string, string>()
    const values = new Map(solved.constraints.map((entry) => [entry.id, entry.value ?? null]))
    const dimensionLines: string[] = []
    const dimensionArrows: string[] = []
    for (const constraint of constraints) {
      if (!isDimension(constraint)) {
        continue
      }
      const labelAt =
        labelPositions.get(constraint.id) ?? defaultLabelPosition(constraint, solved, pixelSize)
      labels.set(constraint.id, project(labelAt))
      labelTexts.set(
        constraint.id,
        dimensionLabelText(constraint, values.get(constraint.id) ?? null, units),
      )
      const paths = dimensionPaths(
        dimensionGeometry(constraint, solved, labelAt),
        project,
        tolerance,
        ARROW_PIXELS,
      )
      dimensionLines.push(paths.lines)
      dimensionArrows.push(paths.arrows)
    }
    const issues = issueRings(solved).map((ring) => project(ring.at))

    // The dimension being placed.
    const cursor = pointer === null ? null : toSketch(pointer)
    let placing: Vector2 | null = null
    if (session.tool === 'dimension' && editor === null && cursor !== null && pointer !== null) {
      const placed = dimensionPreview(picking, cursor, solved)
      if (placed !== null) {
        const paths = dimensionPaths(
          dimensionGeometry(placed.constraint, solved, cursor),
          project,
          tolerance,
          ARROW_PIXELS,
        )
        dimensionLines.push(paths.lines)
        dimensionArrows.push(paths.arrows)
        placing = pointer
      }
    }
    let editorAt: Vector2 | null = null
    if (editor !== null) {
      editorAt = project(editor.labelAt)
      if (editor.mode === 'new') {
        const drafted = draftPreview(editor.draft, solved)
        const paths = dimensionPaths(
          dimensionGeometry(drafted.constraint, solved, editor.draft.labelAt),
          project,
          tolerance,
          ARROW_PIXELS,
        )
        dimensionLines.push(paths.lines)
        dimensionArrows.push(paths.arrows)
      }
    }

    // The rubber band, its snap and the hovered constraint's items.
    const highlight: Vector2[][] = []
    const markers: Vector2[] = []
    const drawn = preview(session)
    const rubberBand =
      drawn === null
        ? ''
        : polylinePath(
            drawn.shapes.flatMap((shape) =>
              projectPolyline(shapePoints(shape, tolerance), project),
            ),
          )
    if (drawn?.snap !== null && drawn?.snap !== undefined) {
      const marker = project(drawn.snap.at)
      if (marker !== null) {
        markers.push(marker)
      }
      const snapped = drawn.snap.kind === 'curve' ? solvedEntity(solved, drawn.snap.entity) : null
      if (snapped !== null && snapped !== undefined) {
        highlight.push(...projectPolyline(entityPoints(snapped, tolerance), project))
      }
    }
    const hoveredConstraint =
      hovered === null ? undefined : constraints.find((constraint) => constraint.id === hovered)
    if (hoveredConstraint !== undefined) {
      const items = constrainedItems(hoveredConstraint, solved)
      for (const curve of items.curves) {
        const entity = solvedEntity(solved, curve)
        if (entity !== undefined) {
          highlight.push(...projectPolyline(entityPoints(entity, tolerance), project))
        }
      }
      for (const point of items.points) {
        const at = pointPosition(solved, point)
        const pixel = at === null ? null : project(at)
        if (pixel !== null) {
          markers.push(pixel)
        }
      }
    }

    // Positions of the HTML chrome.
    const inference =
      pointer === null || drawn === null || drawn.glyphs.length === 0
        ? null
        : ([
            pointer[0] + INFERENCE_OFFSET_PIXELS * scale,
            pointer[1] + INFERENCE_OFFSET_PIXELS * scale,
          ] as Vector2)
    const inputsAt = drawn?.inputsAt === null || drawn === null ? null : project(drawn.inputsAt)
    const measures: Partial<Record<DrawFieldName, string>> = {}
    for (const [name, value] of Object.entries(drawn?.measures ?? {})) {
      const field = name as DrawFieldName
      measures[field] = formatNumber(value, field === 'Angle' ? 'angle' : 'length', units)
    }

    return {
      glyphs,
      labels,
      labelTexts,
      issues,
      dimensionLines: dimensionLines.filter((path) => path !== '').join(' '),
      dimensionArrows: dimensionArrows.filter((path) => path !== '').join(' '),
      rubberBand,
      highlight: polylinePath(highlight),
      markers: markers.map((marker) => circlePath(marker, MARKER_PIXELS * scale)).join(' '),
      inference,
      drawInputs:
        inputsAt === null
          ? null
          : drawInputsPlacement(inputsAt, pointer, DRAW_INPUT_OFFSET_PIXELS * scale),
      editor: editorAt,
      placing,
      box: box === null ? null : pixelRectangle(box.start, box.end),
      measures,
    }
  }

  function paint(): void {
    if (cancelPaint !== null) {
      cancelPaint()
      cancelPaint = null
    }
    followFrames()
    painter?.(computePaint())
  }

  function schedulePaint(): void {
    if (disposed || cancelPaint !== null) {
      return
    }
    cancelPaint = requestFrame(() => {
      cancelPaint = null
      paint()
    })
  }

  /** Paints in the viewport's frames, so the overlay moves with the camera in the same frame. */
  function followFrames(): void {
    const handle = app.viewport()
    if (framed?.handle === handle) {
      return
    }
    framed?.stop()
    framed = null
    if (handle !== null && hasFrames(handle)) {
      framed = {
        handle,
        stop: handle.onRender(() => {
          paint()
        }),
      }
    }
  }

  function refresh(): void {
    if (disposed) {
      return
    }
    publish()
    updateCursor()
    schedulePaint()
  }

  function updateCursor(): void {
    const tool = current()?.tool ?? null
    canvas.classList.toggle('cursor-crosshair!', isDrawingTool(tool))
    canvas.classList.toggle('cursor-not-allowed!', tool === null && notAllowed)
  }

  function flashHint(text: string): void {
    if (flash !== null) {
      clearTimeout(flash.timer)
    }
    flash = {
      text,
      timer: setTimeout(() => {
        flash = null
        refresh()
      }, FLASH_MILLISECONDS),
    }
    refresh()
  }

  // Operations.

  function clearTyped(): void {
    for (const evaluation of evaluations.values()) {
      evaluation.cancel()
    }
    texts = {}
    typed = {}
    inputErrors = {}
  }

  /** Cancels the operation in progress; true when there was one (the Esc ladder's step 2). */
  function cancelOperation(): boolean {
    let cancelled = false
    if (drag !== null) {
      const ending = drag
      drag = null
      ending.cancel().catch(report)
      cancelled = true
    }
    if (box !== null) {
      box = null
      cancelled = true
    }
    press = null
    if (editor !== null) {
      closeEditor()
      cancelled = true
    }
    if (draw !== IDLE) {
      draw = IDLE
      clearTyped()
      cancelled = true
    }
    if (picking !== DIMENSION_START) {
      picking = DIMENSION_START
      selectInSketch(app, [], 'replace')
      cancelled = true
    }
    if (cancelled) {
      refresh()
    }
    return cancelled
  }

  /** Activating another tool, or leaving the sketch, cancels the operation in progress (3.16.3). */
  function resetOperation(): void {
    if (drag !== null) {
      const ending = drag
      drag = null
      ending.cancel().catch(report)
    }
    box = null
    press = null
    draw = IDLE
    clearTyped()
    picking = DIMENSION_START
    editor = null
    hovered = null
    if (flash !== null) {
      clearTimeout(flash.timer)
      flash = null
    }
  }

  function commitShape(
    tool: DrawingTool,
    next: DrawState,
    label: string,
    change: SketchChange,
  ): void {
    addSketchShape(app, label, change)
      .then((added) => {
        // A refused segment ends the chain that would continue from it.
        if (!added && draw === next && tool === 'line') {
          draw = IDLE
          refresh()
        }
      })
      .catch(report)
  }

  function drawAt(tool: DrawingTool, pixel: Vector2): void {
    const session = current()
    const input = drawInputAt(pixel)
    if (session === null || input === null) {
      return
    }
    const result = drawClick(tool, draw, input, createSketchIds(app), session.construction)
    const changed = result.state !== draw
    draw = result.state
    if (changed || result.commit !== null) {
      clearTyped()
    }
    if (result.commit !== null) {
      commitShape(tool, result.state, result.commit.label, result.commit.change)
    }
    refresh()
  }

  function pickAt(type: ConstraintType, pixel: Vector2): void {
    const session = current()
    const solved = solution()
    const item = itemAt(pixel)
    if (session === null || solved === null || item === null) {
      return
    }
    if (session.selection.some((picked) => picked.key === item.key)) {
      selectInSketch(app, [item], 'toggle')
      return
    }
    const picks = [...session.selection, item]
    switch (pickVerdict(type, picks, solved)) {
      case 'refused':
        flashHint(constraintRequirement(type))
        return
      case 'partial':
        selectInSketch(app, picks, 'replace')
        return
      case 'complete': {
        const constraints = constraintsFor(type, picks, solved, createSketchIds(app))
        selectInSketch(app, [], 'replace')
        applySketchConstraints(app, type, constraints).catch(report)
      }
    }
  }

  function dimensionAt(pixel: Vector2): void {
    const solved = solution()
    const at = toSketch(pixel)
    if (solved === null || at === null || editor !== null) {
      return
    }
    const result = dimensionClick(picking, itemAt(pixel), at, solved)
    picking = result.picking
    if (result.refused !== null) {
      flashHint(result.refused)
    }
    if (result.draft !== null) {
      openNewEditor(result.draft, solved)
    } else {
      selectInSketch(app, pickedItems(picking), 'replace')
    }
    refresh()
  }

  function click(pixel: Vector2): void {
    const tool = current()?.tool ?? null
    if (isDrawingTool(tool)) {
      drawAt(tool, pixel)
      return
    }
    if (tool === 'dimension') {
      dimensionAt(pixel)
      return
    }
    const type = constraintTool(tool)
    if (type !== null) {
      pickAt(type, pixel)
    }
  }

  // The dimension editor.

  function openNewEditor(draft: DimensionDraft, solved: SketchSolution): void {
    const units = store.getState().document?.recipe.units ?? 'mm'
    const { value } = draftPreview(draft, solved)
    editorKey += 1
    editor = {
      key: editorKey,
      mode: 'new',
      draft,
      labelAt: draft.labelAt,
      text: formatNumber(value, dimensionUnit(draft.type), units),
      reference: false,
      problem: null,
      message: null,
      busy: false,
    }
    selectInSketch(app, draft.picks, 'replace')
  }

  function closeEditor(): void {
    if (editor === null) {
      return
    }
    editor = null
    picking = DIMENSION_START
    selectInSketch(app, [], 'replace')
    focusView()
    refresh()
  }

  async function commitEditor(request: { readonly reference?: boolean } = {}): Promise<void> {
    const opened = editor
    const solved = solution()
    if (opened === null || opened.busy || solved === null) {
      return
    }
    const reference = request.reference ?? opened.reference
    editor = { ...opened, reference, busy: true, problem: null, message: null }
    refresh()
    if (opened.mode === 'new') {
      const { draft } = opened
      const dimension = dimensionFor(
        draft.type,
        draft.picks,
        opened.text,
        !reference,
        draft.labelAt,
        createSketchIds(app),
        solved,
      )
      const result = await addSketchDimension(app, dimension)
      if (editor?.key !== opened.key) {
        return
      }
      if (result.outcome === 'committed') {
        labelPositions.set(dimension.id, draft.labelAt)
        closeEditor()
        return
      }
      editor = {
        ...editor,
        busy: false,
        problem: result.outcome === 'over_constrained' ? 'over_constrained' : null,
        message: result.outcome === 'failed' ? result.message : null,
      }
      refresh()
      return
    }
    const changed: SketchConstraint = reference
      ? { ...opened.constraint, expr: opened.text, driving: false }
      : withoutDriving({ ...opened.constraint, expr: opened.text })
    const result = await changeSketchDimension(app, changed)
    if (editor?.key !== opened.key) {
      return
    }
    if (result.outcome === 'committed') {
      closeEditor()
      return
    }
    editor = { ...editor, busy: false, problem: 'unsolvable' }
    refresh()
  }

  function editDimension(id: string): void {
    const state = store.getState()
    const session = editingSession(state)
    const solved = solution()
    const constraint =
      session === null
        ? undefined
        : storedConstraints(state, session.sketchId).find((stored) => stored.id === id)
    if (solved === null || constraint === undefined || !isDimension(constraint)) {
      return
    }
    const pixelSize = centerPixelSize(solved)
    draw = IDLE
    clearTyped()
    picking = DIMENSION_START
    editorKey += 1
    editor = {
      key: editorKey,
      mode: 'edit',
      constraint,
      labelAt: labelPositions.get(id) ?? defaultLabelPosition(constraint, solved, pixelSize),
      text: constraint.expr,
      reference: constraint.driving === false,
      problem: null,
      message: null,
      busy: false,
    }
    refresh()
  }

  // Draw inputs.

  function evaluationOf(name: DrawFieldName): LatestOnly<Evaluation> {
    let evaluation = evaluations.get(name)
    if (evaluation === undefined) {
      evaluation = createLatestOnly<Evaluation, { value: number; dimension: Dimension }>(
        (request) => app.core.evaluate(request.documentHandle, request.text),
        (result, request) => {
          if (texts[request.name] !== request.text) {
            return
          }
          const units = store.getState().document?.recipe.units ?? 'mm'
          const converted = convertForField(result, request.unit, units, request.name, request.text)
          if (converted.ok) {
            typed = { ...typed, [request.name]: { text: request.text, value: converted.value } }
            inputErrors = { ...inputErrors, [request.name]: undefined }
          } else {
            inputErrors = { ...inputErrors, [request.name]: converted.message }
          }
          refresh()
        },
        (error, request) => {
          if (texts[request.name] !== request.text) {
            return
          }
          inputErrors = {
            ...inputErrors,
            [request.name]:
              error instanceof CoreError ? error.message : MESSAGES.core_not_running(),
          }
          refresh()
        },
      )
      evaluations.set(name, evaluation)
    }
    return evaluation
  }

  function setDrawText(name: DrawFieldName, text: string): void {
    const session = current()
    const tool = session?.tool ?? null
    const field = isDrawingTool(tool)
      ? drawFields(tool, draw).find((entry) => entry.name === name)
      : undefined
    const documentHandle = store.getState().document?.documentHandle ?? null
    if (field === undefined || documentHandle === null) {
      return
    }
    texts = { ...texts, [name]: text }
    const { [name]: _dropped, ...rest } = typed
    typed = rest
    inputErrors = { ...inputErrors, [name]: undefined }
    if (text.trim() === '') {
      evaluationOf(name).cancel()
    } else {
      evaluationOf(name).submit({ documentHandle, name, unit: field.unit, text })
    }
    refresh()
  }

  async function commitDrawInputs(): Promise<void> {
    const tool = current()?.tool ?? null
    if (!isDrawingTool(tool) || draw === IDLE) {
      return
    }
    await Promise.all(Array.from(evaluations.values(), (evaluation) => evaluation.flush()))
    const filled = Object.entries(texts).filter(
      ([, text]) => text !== undefined && text.trim() !== '',
    )
    const ready = filled.every(([name]) => typed[name as DrawFieldName] !== undefined)
    if (filled.length === 0 || !ready || current()?.tool !== tool) {
      return
    }
    drawAt(tool, pointer ?? [0, 0])
    focusView()
  }

  function clearDrawInputs(): void {
    clearTyped()
    focusView()
    refresh()
  }

  // Pointer and key input.

  function pressIntent(pixel: Vector2): NonNullable<typeof press>['intent'] {
    if (current()?.tool !== null) {
      return { kind: 'none' }
    }
    const hit = hitAt(pixel, true)
    if (hit === null) {
      return { kind: 'box' }
    }
    if (hit.kind !== 'point' || fullyConstrained(hit.point)) {
      // Points only in M1; pressing on a curve, the origin or a fixed point starts nothing.
      return { kind: 'none' }
    }
    return { kind: 'drag', point: hit.point }
  }

  function fullyConstrained(point: PointRef): boolean {
    const solved = solution()
    const owner = solved === null ? undefined : solvedEntity(solved, ownerOf(point))
    return owner?.constrained === 'fully_constrained'
  }

  function onPointerDown(event: PointerEvent): void {
    if (event.button !== 0 || event.buttons !== 1 || current() === null) {
      return
    }
    const pixel = canvasPixel(event)
    pointer = pixel
    suppressed = suppressing(event)
    press = {
      pointerId: event.pointerId,
      start: pixel,
      intent: pressIntent(pixel),
      toggle: event.shiftKey || event.ctrlKey || event.metaKey,
      moved: false,
    }
  }

  function beginDrag(started: NonNullable<typeof press>, pixel: Vector2): void {
    const session = current()
    if (session === null) {
      return
    }
    if (started.intent.kind === 'drag') {
      drag = startSketchDrag(app, session.sketchId, started.intent.point)
    } else if (started.intent.kind === 'box') {
      box = { start: started.start, end: pixel, toggle: started.toggle }
    }
  }

  function onPointerMove(event: PointerEvent): void {
    if (current() === null) {
      return
    }
    const pixel = canvasPixel(event)
    pointer = pixel
    suppressed = suppressing(event)
    if (press !== null && press.pointerId === event.pointerId) {
      if ((event.buttons & 1) === 0) {
        press = null
      } else if (!press.moved && distance(pixel, press.start) > DRAG_THRESHOLD_PIXELS) {
        press.moved = true
        beginDrag(press, pixel)
      }
    }
    if (drag !== null) {
      const to = toSketch(pixel)
      if (to !== null) {
        drag.move(to, event.timeStamp)
      }
      return
    }
    if (box !== null) {
      box.end = pixel
      refresh()
      return
    }
    if (current()?.tool === null && press === null) {
      const hit = hitAt(pixel, false)
      const over = hit?.kind === 'point' && fullyConstrained(hit.point)
      if (over !== notAllowed) {
        notAllowed = over
        updateCursor()
      }
      return
    }
    refresh()
  }

  function finishBox(ended: NonNullable<typeof box>): void {
    const session = current()
    const solved = solution()
    const project = projector()
    const pixelSize = pointer === null ? null : millimetersPerPixel(pointer)
    if (session === null || solved === null || project === null) {
      return
    }
    const mode = ended.end[0] >= ended.start[0] ? 'window' : 'crossing'
    const taken = entitiesInBox(
      solved,
      project,
      screenBox(ended.start, ended.end),
      mode,
      TESSELLATION_PIXELS * (pixelSize ?? 1),
    )
    selectInSketch(
      app,
      taken.map((entity) => entitySelectionItem(session.sketchId, entity)),
      ended.toggle ? 'toggle' : 'replace',
    )
  }

  function onPointerUp(event: PointerEvent): void {
    if (press === null || press.pointerId !== event.pointerId || event.button !== 0) {
      return
    }
    const ended = press
    press = null
    const pixel = canvasPixel(event)
    pointer = pixel
    suppressed = suppressing(event)
    if (drag !== null) {
      const finishing = drag
      drag = null
      finishing.finish().catch(report)
      refresh()
      return
    }
    if (box !== null) {
      const finished = box
      box = null
      finished.end = pixel
      finishBox(finished)
      refresh()
      return
    }
    if (distance(pixel, ended.start) <= CLICK_DISTANCE_PIXELS) {
      click(pixel)
    }
  }

  function onPointerCancel(event: PointerEvent): void {
    if (press?.pointerId !== event.pointerId) {
      return
    }
    press = null
    if (drag !== null) {
      const ending = drag
      drag = null
      ending.cancel().catch(report)
    }
    box = null
    refresh()
  }

  function onDoubleClick(): void {
    // A double-click ends the line chain after its clicks committed the segment (3.16.4).
    if (current()?.tool === 'line' && draw.stage === 'line') {
      draw = IDLE
      clearTyped()
      refresh()
    }
  }

  function onPointerLeave(): void {
    if (press === null && notAllowed) {
      notAllowed = false
      updateCursor()
    }
  }

  /** Typing a value or Tab while a shape rubber-bands goes to the draw inputs (3.16.4). */
  function onKeyDownCapture(event: KeyboardEvent): void {
    if (
      event.defaultPrevented ||
      event.isComposing ||
      event.ctrlKey ||
      event.metaKey ||
      event.altKey ||
      model.drawInputs.length === 0
    ) {
      return
    }
    if (!onView(event.target)) {
      return
    }
    const [first] = model.drawInputs
    if (event.key === 'Tab' && !event.shiftKey) {
      event.preventDefault()
      focusRequest += 1
      refresh()
      return
    }
    if (first !== undefined && VALUE_START.test(event.key)) {
      event.preventDefault()
      focusRequest += 1
      setDrawText(first.name, `${texts[first.name] ?? ''}${event.key}`)
    }
  }

  /** Whether a key press is meant for the view: the body or an element around the canvas has focus. */
  function onView(target: EventTarget | null): boolean {
    return (
      target === canvas.ownerDocument.body ||
      (target instanceof Node && !isEditable(target) && target.contains(canvas))
    )
  }

  function onKeyDown(event: KeyboardEvent): void {
    if (event.key === 'Control' || event.key === 'Meta') {
      updateSuppressed(event)
    }
    if (event.defaultPrevented || event.isComposing || current() === null) {
      return
    }
    // An open palette, menu or popover takes Esc first and Enter belongs to it (3.5).
    if (store.getState().panels.paletteOpen || hasOpenPopover(canvas.ownerDocument)) {
      return
    }
    if (event.key === 'Escape' && cancelOperation()) {
      event.preventDefault()
      return
    }
    // Mod+Z cancels the operation in progress only (3.20); text fields keep their own undo.
    if (isUndoChord(event) && !isEditable(event.target) && cancelOperation()) {
      event.preventDefault()
      return
    }
    // Enter on the view ends the line chain; on a button it only activates the button (3.5).
    if (
      event.key === 'Enter' &&
      onView(event.target) &&
      current()?.tool === 'line' &&
      draw.stage === 'line'
    ) {
      event.preventDefault()
      draw = IDLE
      clearTyped()
      refresh()
    }
  }

  function isUndoChord(event: KeyboardEvent): boolean {
    const primary = app.platform.operatingSystem === 'mac' ? event.metaKey : event.ctrlKey
    return primary && !event.shiftKey && !event.altKey && event.key.toLowerCase() === 'z'
  }

  function updateSuppressed(event: KeyboardEvent): void {
    const next = suppressing(event)
    if (next !== suppressed) {
      suppressed = next
      if (isDrawingTool(current()?.tool ?? null)) {
        refresh()
      }
    }
  }

  const pointerListeners: readonly [string, EventListener][] = [
    ['pointerdown', onPointerDown as EventListener],
    ['pointermove', onPointerMove as EventListener],
    ['pointerup', onPointerUp as EventListener],
    ['pointercancel', onPointerCancel as EventListener],
    ['pointerleave', onPointerLeave],
    ['dblclick', onDoubleClick],
  ]
  for (const [type, listener] of pointerListeners) {
    canvas.addEventListener(type, listener)
  }
  const keyTarget = canvas.ownerDocument
  keyTarget.addEventListener('keydown', onKeyDownCapture, true)
  keyTarget.addEventListener('keydown', onKeyDown)
  keyTarget.addEventListener('keyup', updateSuppressed)
  cleanups.push(() => {
    for (const [type, listener] of pointerListeners) {
      canvas.removeEventListener(type, listener)
    }
    keyTarget.removeEventListener('keydown', onKeyDownCapture, true)
    keyTarget.removeEventListener('keydown', onKeyDown)
    keyTarget.removeEventListener('keyup', updateSuppressed)
  })

  cleanups.push(
    store.subscribe((state, previous) => {
      const key = sessionKey(state)
      if (key !== last) {
        last = key
        resetOperation()
      }
      if (
        state.sketch !== previous.sketch ||
        state.document !== previous.document ||
        state.settings !== previous.settings ||
        state.context !== previous.context
      ) {
        refresh()
      }
    }),
  )

  model = computeModel()
  updateCursor()
  schedulePaint()

  return {
    subscribe(listener) {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    getModel: () => model,
    setPainter(next) {
      painter = next
      if (next !== null) {
        paint()
      }
    },
    repaint: paint,
    setDrawText,
    commitDrawInputs,
    clearDrawInputs,
    hoverConstraint(constraint) {
      if (hovered !== constraint) {
        hovered = constraint
        refresh()
      }
    },
    selectConstraint(constraint, toggle) {
      const session = current()
      const state = store.getState()
      const stored =
        session === null
          ? undefined
          : storedConstraints(state, session.sketchId).find((entry) => entry.id === constraint)
      if (session === null || stored === undefined) {
        return
      }
      const units = state.document?.recipe.units ?? 'mm'
      const title = constraintTitle(stored, solution(), units)
      selectInSketch(
        app,
        [constraintSelectionItem(session.sketchId, constraint, title)],
        toggle ? 'toggle' : 'replace',
      )
    },
    editDimension,
    setEditorText(text) {
      if (editor !== null && editor.text !== text) {
        editor = { ...editor, text, problem: null, message: null }
        refresh()
      }
    },
    setEditorReference(reference) {
      if (editor !== null && editor.reference !== reference) {
        editor = { ...editor, reference, problem: null }
        refresh()
      }
    },
    setEditorRound(round) {
      const solved = solution()
      if (editor?.mode !== 'new' || solved === null || editor.draft.type === round) {
        return
      }
      const units = store.getState().document?.recipe.units ?? 'mm'
      const before = draftPreview(editor.draft, solved)
      const draft: DimensionDraft = { ...editor.draft, type: round }
      const after = draftPreview(draft, solved)
      // A value the user has not changed follows the choice.
      const untouched = editor.text === formatNumber(before.value, 'length', units)
      editor = {
        ...editor,
        draft,
        text: untouched ? formatNumber(after.value, 'length', units) : editor.text,
        problem: null,
      }
      refresh()
    },
    commitEditor,
    closeEditor,
    dispose() {
      if (disposed) {
        return
      }
      resetOperation()
      disposed = true
      cancelPaint?.()
      cancelPaint = null
      framed?.stop()
      framed = null
      for (const evaluation of evaluations.values()) {
        evaluation.cancel()
      }
      for (const cleanup of cleanups) {
        cleanup()
      }
      canvas.classList.remove('cursor-crosshair!', 'cursor-not-allowed!')
      listeners.clear()
      painter = null
    },
  }
}
