import {
  addSketchDimension,
  addSketchShape,
  applySketchConstraints,
  changeSketchDimension,
  createSketchIds,
  refusedDimensionMessage,
  selectInSketch,
  solutionOf,
  storedSketchParams,
} from '../app/actions/sketch'
import { startSketchDrag, type SketchDrag } from '../app/actions/sketch-drag'
import type { App, Platform } from '../app/createApp'
import { reportFailure } from '../app/report'
import { CoreError } from '../core/client'
import { createLatestOnly, type LatestOnly } from '../core/queue'
import { convertForField } from '../expression/evaluate'
import { formatNumber } from '../expression/format'
import type {
  Dimension,
  DocHandle,
  Placement,
  PointRef,
  SketchConstraint,
  SketchSolution,
  Units,
} from '../generated/protocol'
import { hasOpenPopover, isEditable } from '../layout/focus'
import { MESSAGES } from '../messages'
import {
  editingSession,
  selectDocumentUnits,
  selectSketchRecord,
  type EditingSession,
} from '../state/selectors'
import type { AppState, SelectionItem, SketchNotice } from '../state/store'
import { usedEntities } from '../timeline/uses'
import { framedArea, type Camera, type ViewWindow } from '../viewport/camera'
import type { ViewportController, ViewportHandle } from '../viewport/controller'
import { requestFrame } from '../viewport/frames'
import { sketchWorldAxes } from '../viewport/planes'
import { sketchAxisSpans } from '../viewport/sketch-drawables'
import { dimensionLabelText } from './annotations'
import {
  constraintRequirement,
  constraintsFor,
  constraintTitle,
  dimensionUnit,
  isDimension,
  pickVerdict,
  type ConstraintType,
  type DimensionConstraint,
  type QuantityUnit,
} from './constraints'
import {
  DIMENSION_START,
  dimensionClick,
  dimensionForDraft,
  draftPreview,
  dimensionPreview,
  pickedItems,
  type DimensionDraft,
  type DimensionPicking,
  type DimensionPreview,
} from './dimension-tool'
import { defaultLabelPosition } from './dimensions'
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
import { ownerOf, solvedEntity } from './operands'
import { EMPTY_PAINT, overlayPaint, type OverlayPaint, type PaintSnapshot } from './overlay-paint'
import { millimetersPerPixelAt, pixelToSketch, sketchToPixel } from './placement'
import type { SketchAddition } from './recipes'
import { entitiesInBox, freeAreaBox, screenBox, type Projector } from './screen'
import { constraintTool, isDrawingTool } from './tools'

// Sketch mode's session (ui.md 2.14, 3.16): its tools and the changes they commit, the step
// prompts, the profile problems drawn on the sketch, and the internal sketch controller that turns
// pointer and key input on the canvas into drawing, constraining, dimensioning, dragging and box
// selection, bypassing React (ADR 0002).

/** A sketch tool: a drawing tool, the dimension tool, or a constraint tool in pick mode. */
export type SketchToolId = DrawingTool | 'dimension' | `constrain_${ConstraintType}`

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
      /** The probe solution's first `sketch_conflict` or `redundant_constraints` message (sketch.md 4.2), null without one. */
      readonly message: string | null
    }
  | { readonly outcome: 'failed'; readonly message: string }

/** A commit the probe or the core refused. */
export type RefusedSketchCommit = Exclude<SketchCommitResult, { readonly outcome: 'committed' }>

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

// The overlay the controller drives.

/** A draw input (3.16.4): its quantity, the typed text and why the text is not a value, if it is not. */
export type DrawInputModel = {
  readonly name: DrawFieldName
  readonly unit: QuantityUnit
  readonly text: string
  readonly error: string | null
}

/**
 * Why the dimension editor stays open after Enter with Add reference and Cancel (3.16.7): a new
 * driving dimension the other constraints already set (`dimension_over_constrained`), or an edited
 * value the sketch cannot take, with its sentence (`refusedDimensionMessage`).
 */
export type DimensionProblem =
  | { readonly kind: 'over_constrained' }
  | { readonly kind: 'cannot_take'; readonly message: string }

/** The open dimension editor (3.16.7). */
export type DimensionEditorModel = {
  /** Changes each time an editor opens, so its input starts fresh. */
  readonly key: number
  readonly mode: 'new' | 'edit'
  readonly text: string
  readonly unit: QuantityUnit
  /** The Diameter | Radius choice of a new dimension on a circle or arc, else null. */
  readonly round: 'diameter' | 'radius' | null
  readonly reference: boolean
  /** Why the editor stays open with Add reference and Cancel, or null. */
  readonly problem: DimensionProblem | null
  /** Why the core refused a new dimension, shown under the input, or null. */
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
/** How long a refused pick's message stays in the sketch hint. */
const FLASH_MILLISECONDS = 3_000
/** Characters that start typing a value into the first draw input (3.16.4). */
const VALUE_START = /^[\d.\-(]$/

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

/** What a change of cancels the operation in progress: the edited sketch and its tool. */
function sessionKey(state: AppState): string {
  const session = editingSession(state)
  return session === null ? '' : `${session.sketchId} ${session.tool ?? ''}`
}

/** The stored constraints of a sketch, in stored order. */
function storedConstraints(state: AppState, sketchId: string): readonly SketchConstraint[] {
  return storedSketchParams(state, sketchId)?.constraints ?? []
}

/** A viewport that reports its rendered frames, so the overlay follows the camera in the same frame. */
type FrameSource = ViewportHandle & Pick<ViewportController, 'onRender'>

function hasFrames(handle: ViewportHandle): handle is FrameSource {
  return 'onRender' in handle && typeof handle.onRender === 'function'
}

/** A changed dimension that drives again: `driving: true` is the default sketch.md leaves out. */
function withoutDriving(constraint: DimensionConstraint): SketchConstraint {
  const { driving: _driving, ...rest } = constraint
  return rest
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

/**
 * The sketch hint (3.16.3): a warning (a refused pick, collinear arc points), else a constraint
 * tool's requirement, else the step prompt.
 */
function hintOf(
  warning: string | null,
  constraint: ConstraintType | null,
  prompt: string | null,
): SketchOverlayModel['hint'] {
  if (warning !== null) {
    return { text: warning, warning: true }
  }
  if (constraint !== null) {
    return { text: constraintRequirement(constraint), warning: false }
  }
  return prompt === null ? null : { text: prompt, warning: false }
}

/** Shift or Mod with a click toggles the item in the sketch selection (3.16.10). */
export function togglesSelection(event: {
  readonly shiftKey: boolean
  readonly ctrlKey: boolean
  readonly metaKey: boolean
}): boolean {
  return event.shiftKey || event.ctrlKey || event.metaKey
}

/** A selection box dragged rightward selects what it holds; leftward, what it touches (3.16.9). */
function boxMode(selecting: {
  readonly start: Vector2
  readonly end: Vector2
}): 'window' | 'crossing' {
  return selecting.end[0] >= selecting.start[0] ? 'window' : 'crossing'
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
  // The session's notice as last shown, with the step prompt it replaced (3.16.10).
  let notice: {
    readonly shown: SketchNotice
    readonly prompt: string | null
    ended: boolean
  } | null = null
  let last = sessionKey(store.getState())

  type Evaluation = {
    readonly documentHandle: DocHandle
    readonly name: DrawFieldName
    readonly unit: QuantityUnit
    readonly text: string
  }

  /** What an editor opens on: a new dimension's draft or a stored dimension, with its first value. */
  type EditorOpening = (
    | { readonly mode: 'new'; readonly draft: DimensionDraft }
    | { readonly mode: 'edit'; readonly constraint: DimensionConstraint }
  ) & {
    readonly text: string
    readonly reference: boolean
    readonly labelAt: Vector2
  }

  type Editor = EditorOpening & {
    readonly key: number
    readonly problem: DimensionProblem | null
    readonly message: string | null
    readonly busy: boolean
  }

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

  function canvasPixel(event: MouseEvent): Vector2 {
    const rectangle = canvas.getBoundingClientRect()
    return [event.clientX - rectangle.left, event.clientY - rectangle.top]
  }

  /**
   * The camera, view window and sketch placement that map between the canvas and the sketch, in
   * the free area's device coordinates (2.11).
   */
  function view(): {
    readonly camera: Camera
    readonly viewWindow: ViewWindow
    readonly placement: Placement
  } | null {
    const handle = app.viewport()
    const solved = solution()
    return handle === null || solved === null
      ? null
      : {
          camera: handle.getCamera(),
          viewWindow: handle.viewWindow(),
          placement: solved.placement,
        }
  }

  /** The sketch point under a canvas pixel, or null when the view cannot reach the sketch plane. */
  function toSketch(pixel: Vector2): Vector2 | null {
    const mapping = view()
    return mapping === null
      ? null
      : pixelToSketch(mapping.camera, mapping.viewWindow, mapping.placement, pixel)
  }

  function millimetersPerPixel(pixel: Vector2): number | null {
    const mapping = view()
    return mapping === null
      ? null
      : millimetersPerPixelAt(mapping.camera, mapping.viewWindow, mapping.placement, pixel)
  }

  /** Sketch millimeters per CSS pixel at the free area's center, for tessellation and label offsets. */
  function centerPixelSize(): number {
    const mapping = view()
    if (mapping === null) {
      return 1
    }
    const { camera, viewWindow, placement } = mapping
    const area = framedArea(viewWindow)
    const center: Vector2 = [area.x + area.width / 2, area.y + area.height / 2]
    return millimetersPerPixelAt(camera, viewWindow, placement, center) ?? 1
  }

  function projector(): Projector | null {
    const mapping = view()
    if (mapping === null || !(mapping.viewWindow.width > 0) || !(mapping.viewWindow.height > 0)) {
      return null
    }
    const { camera, viewWindow, placement } = mapping
    return (point) => sketchToPixel(camera, viewWindow, placement, point)
  }

  function documentUnits(): Units {
    return selectDocumentUnits(store.getState())
  }

  /** The stored constraint with this id of the edited sketch. */
  function storedConstraint(id: string): SketchConstraint | undefined {
    const session = current()
    return session === null
      ? undefined
      : storedConstraints(store.getState(), session.sketchId).find((stored) => stored.id === id)
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

  /**
   * Whether an event holds the primary modifier (Ctrl, or Cmd on macOS): it suppresses inference
   * (3.16.5) and makes the undo chord.
   */
  function primaryModifier(event: MouseEvent | KeyboardEvent): boolean {
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

  /** The drawing tool's rubber band at the pointer, or null with another tool. */
  function preview(session: EditingSession): DrawPreview | null {
    const tool = session.tool
    if (!isDrawingTool(tool)) {
      return null
    }
    const input = drawInputAt(pointer)
    return input === null ? null : drawPreview(tool, draw, input)
  }

  /** The dimension the dimension tool places with its label at the pointer, or null. */
  function placedDimension(
    session: EditingSession,
    solved: SketchSolution,
  ): (DimensionPreview & { readonly labelAt: Vector2 }) | null {
    const cursor = pointer === null ? null : toSketch(pointer)
    if (session.tool !== 'dimension' || editor !== null || cursor === null) {
      return null
    }
    const placed = dimensionPreview(picking, cursor, solved)
    return placed === null ? null : { ...placed, labelAt: cursor }
  }

  /** The text of the label of the dimension being placed, or null. */
  function placingText(placed: DimensionPreview | null): string | null {
    return placed === null || !isDimension(placed.constraint)
      ? null
      : dimensionLabelText(placed.constraint, placed.value, documentUnits())
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

  /**
   * The session's notice (3.16.10) while it shows: from when it is set until the step prompt it
   * replaced changes, even when that prompt comes back later.
   */
  function noticeText(session: EditingSession, prompt: string | null): string | null {
    const stored = session.notice ?? null
    if (stored === null) {
      notice = null
      return null
    }
    if (notice?.shown !== stored) {
      notice = { shown: stored, prompt, ended: false }
    } else if (notice.prompt !== prompt) {
      notice.ended = true
    }
    return notice.ended ? null : stored.message
  }

  function computeModel(): SketchOverlayModel {
    const session = current()
    if (session === null) {
      return EMPTY_SKETCH_MODEL
    }
    const solved = solution()
    const drawn = preview(session)
    const placed = solved === null ? null : placedDimension(session, solved)
    const tool = session.tool
    const step = toolStep(tool)
    const prompt = step === null ? null : sketchStepHint(step, app.platform)
    const fields = isDrawingTool(tool) ? drawFields(tool, draw) : []
    const inference = drawn?.glyphs ?? []
    return {
      hint: hintOf(
        flash?.text ?? drawn?.hint ?? noticeText(session, prompt),
        constraintTool(tool),
        prompt,
      ),
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
      placing: placingText(placed),
      editor: editorModel(),
      box: box === null ? null : boxMode(box),
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

  /** What this frame paints from; null when the sketch or the view is not there. */
  function paintSnapshot(): PaintSnapshot | null {
    const state = store.getState()
    const session = editingSession(state)
    const solved = solution()
    const project = projector()
    const mapping = view()
    if (session === null || solved === null || project === null || mapping === null) {
      return null
    }
    const pixelSize = centerPixelSize()
    const record = selectSketchRecord(state, session.sketchId)
    return {
      solution: solved,
      project,
      uiScale: uiScale(),
      units: documentUnits(),
      pixelSize,
      tolerance: TESSELLATION_PIXELS * pixelSize,
      constraints: storedConstraints(state, session.sketchId),
      usedEntities: Object.keys(usedEntities(state, session.sketchId)),
      showConstraints: session.showConstraints,
      labelPositions,
      pointer,
      drawn: preview(session),
      placing: placedDimension(session, solved),
      editor: editorSnapshot(solved),
      hovered,
      box,
      worldAxes: record === null ? null : sketchWorldAxes(record),
      axisSpans: sketchAxisSpans(mapping.placement, mapping.camera, mapping.viewWindow),
      freeArea: freeAreaBox(mapping.viewWindow),
    }
  }

  /** The open editor as the paint reads it: where it is, and the new dimension it drafts. */
  function editorSnapshot(solved: SketchSolution): PaintSnapshot['editor'] {
    if (editor === null) {
      return null
    }
    const draft =
      editor.mode === 'new'
        ? {
            constraint: draftPreview(editor.draft, solved).constraint,
            labelAt: editor.draft.labelAt,
          }
        : null
    return { at: editor.labelAt, draft }
  }

  function computePaint(): OverlayPaint {
    const snapshot = paintSnapshot()
    return snapshot === null ? EMPTY_PAINT : overlayPaint(snapshot)
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

  /** Cancels the point drag in progress; true when there was one. */
  function cancelDrag(): boolean {
    if (drag === null) {
      return false
    }
    const ending = drag
    drag = null
    ending.cancel().catch(reportFailure('A sketch action'))
    return true
  }

  /** Cancels the operation in progress; true when there was one (the Esc ladder's step 2). */
  function cancelOperation(): boolean {
    let cancelled = cancelDrag()
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
    cancelDrag()
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
      .catch(reportFailure('A sketch action'))
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
        applySketchConstraints(app, type, constraints).catch(reportFailure('A sketch action'))
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

  function openEditor(opening: EditorOpening): void {
    editorKey += 1
    editor = { ...opening, key: editorKey, problem: null, message: null, busy: false }
  }

  function openNewEditor(draft: DimensionDraft, solved: SketchSolution): void {
    const { value } = draftPreview(draft, solved)
    openEditor({
      mode: 'new',
      draft,
      labelAt: draft.labelAt,
      text: formatNumber(value, dimensionUnit(draft.type), documentUnits()),
      reference: false,
    })
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
      const dimension = dimensionForDraft(
        draft,
        opened.text,
        !reference,
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
        problem: result.outcome === 'over_constrained' ? { kind: 'over_constrained' } : null,
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
    // Nothing was applied: the editor keeps the text and says why, as the selection inspector
    // does for the same refusal (3.16.7).
    editor = {
      ...editor,
      busy: false,
      problem: { kind: 'cannot_take', message: refusedDimensionMessage(result) },
      message: null,
    }
    refresh()
  }

  function editDimension(id: string): void {
    const solved = solution()
    const constraint = storedConstraint(id)
    if (solved === null || constraint === undefined || !isDimension(constraint)) {
      return
    }
    const pixelSize = centerPixelSize()
    draw = IDLE
    clearTyped()
    picking = DIMENSION_START
    openEditor({
      mode: 'edit',
      constraint,
      labelAt: labelPositions.get(id) ?? defaultLabelPosition(constraint, solved, pixelSize),
      text: constraint.expr,
      reference: constraint.driving === false,
    })
    // The editor is where the value changes now, so the selection inspector, which edits the same
    // dimension, does not open over it (3.16.7); closing the editor leaves the selection empty too.
    selectInSketch(app, [], 'replace')
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
          const converted = convertForField(
            result,
            request.unit,
            documentUnits(),
            request.name,
            request.text,
          )
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
    // The user waits on the typed values (4.4).
    await app.waitOn(
      Promise.all(Array.from(evaluations.values(), (evaluation) => evaluation.flush())),
    )
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
    suppressed = primaryModifier(event)
    press = {
      pointerId: event.pointerId,
      start: pixel,
      intent: pressIntent(pixel),
      toggle: togglesSelection(event),
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
    suppressed = primaryModifier(event)
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
    const taken = entitiesInBox(
      solved,
      project,
      screenBox(ended.start, ended.end),
      boxMode(ended),
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
    suppressed = primaryModifier(event)
    if (drag !== null) {
      const finishing = drag
      drag = null
      finishing.finish().catch(reportFailure('A sketch action'))
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
    cancelDrag()
    box = null
    refresh()
  }

  /** Ends the line chain in progress; false when there is none (3.16.4). */
  function endLineChain(): boolean {
    if (current()?.tool !== 'line' || draw.stage !== 'line') {
      return false
    }
    draw = IDLE
    clearTyped()
    refresh()
    return true
  }

  function onDoubleClick(): void {
    // A double-click ends the line chain after its clicks committed the segment (3.16.4).
    endLineChain()
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
    if (event.key === 'Enter' && onView(event.target) && endLineChain()) {
      event.preventDefault()
    }
  }

  function isUndoChord(event: KeyboardEvent): boolean {
    return (
      primaryModifier(event) && !event.shiftKey && !event.altKey && event.key.toLowerCase() === 'z'
    )
  }

  function updateSuppressed(event: KeyboardEvent): void {
    const next = primaryModifier(event)
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
      const stored = storedConstraint(constraint)
      if (session === null || stored === undefined) {
        return
      }
      const title = constraintTitle(stored, solution(), documentUnits())
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
        editor = { ...editor, problem: null, reference }
        refresh()
      }
    },
    setEditorRound(round) {
      const solved = solution()
      if (editor?.mode !== 'new' || solved === null || editor.draft.type === round) {
        return
      }
      const units = documentUnits()
      const before = draftPreview(editor.draft, solved)
      const draft: DimensionDraft = { ...editor.draft, type: round }
      const after = draftPreview(draft, solved)
      // A value the user has not changed follows the choice.
      const untouched = editor.text === formatNumber(before.value, 'length', units)
      editor = {
        ...editor,
        problem: null,
        draft,
        text: untouched ? formatNumber(after.value, 'length', units) : editor.text,
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
