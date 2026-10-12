import {
  ArrowDown,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  Box,
  CheckCheck,
  Circle,
  CircleCheck,
  Cuboid,
  Dot,
  Download,
  Eye,
  FilePlus,
  FileOutput,
  FolderOpen,
  Funnel,
  House,
  Lock,
  LockOpen,
  Maximize,
  Monitor,
  Moon,
  MousePointer2,
  PanelBottom,
  PanelLeft,
  Pencil,
  Printer,
  Redo2,
  Ruler,
  RulerDimensionLine,
  Save,
  SaveAll,
  ScanEye,
  ScanLine,
  Search,
  SlidersHorizontal,
  Slash,
  Spline,
  SquareDashed,
  RectangleHorizontal,
  Sun,
  TextCursorInput,
  Trash2,
  TriangleAlert,
  Undo2,
  Wrench,
  CircleX,
  ZoomIn,
  HeartPulse,
  ListChecks,
  Layers,
  Type,
  type LucideIcon,
} from 'lucide-react'
import { setBodyVisible, setLocked, setSessionVisible } from '../app/actions/bodies'
import { editFeature } from '../app/actions/command'
import { setContext } from '../app/actions/context'
import {
  newDocument,
  openWithPicker,
  redo,
  save,
  saveAs,
  setUnits,
  undo,
} from '../app/actions/document'
import { commitEdits } from '../app/actions/edits'
import { exportModel, type ExportRequest } from '../app/actions/export'
import { deleteParameter } from '../app/actions/parameters'
import { openPrintHealth, runPrintAnalysis } from '../app/actions/print'
import { clearSelection } from '../app/actions/selection'
import { updateSettings } from '../app/actions/settings'
import {
  commitSketchChange,
  deleteSketchItems,
  finishSketch,
  setSketchTool,
} from '../app/actions/sketch'
import { deleteFeatures, setSuppressed } from '../app/actions/timeline'
import type { App, Platform } from '../app/createApp'
import type { Edit, Placement, SketchSolution, Units } from '../generated/protocol'
import { resolvedTheme } from '../layout/color-scheme'
import { requestUi } from '../layout/requests'
import { MESSAGES } from '../messages'
import type { Settings } from '../settings/settings'
import type { SketchToolId } from '../sketch/session'
import type { AppContext, AppState, OriginName, SelectionItem, ViewState } from '../state/store'
import { toggledTheme } from '../theme/theme'
import type { StandardView, Vector3 } from '../viewport/camera'
import { cross } from '../viewport/math'
import { isSessionVisible } from '../viewport/scene'
import { CONSTRAINT_ICONS } from './icons'
import type { Availability, CommandCategory, CommandDefinition } from './registry'

// The built-in UI commands of ui.md 3.5: everything the registry offers besides the feature
// commands of the catalog (features.ts).

const ALL: readonly AppContext[] = ['model', 'sketch', 'print']
const MODEL_PRINT: readonly AppContext[] = ['model', 'print']
const SKETCH: readonly AppContext[] = ['sketch']
const PRINT: readonly AppContext[] = ['print']

const ENABLED: Availability = { state: 'enabled' }
const HIDDEN: Availability = { state: 'hidden' }

type CommandInput = {
  readonly id: string
  readonly title: string
  readonly category: CommandCategory
  readonly icon?: LucideIcon | null
  readonly keywords?: readonly string[]
  readonly shortcuts?: readonly string[]
  readonly contexts: readonly AppContext[]
  readonly availability?: (app: App) => Availability
  readonly checked?: (app: App) => boolean
  readonly detail?: (app: App) => string | null
  readonly run: CommandDefinition['run']
}

/** A command with the defaults: no icon, keywords or shortcuts, always enabled. */
function command(input: CommandInput): CommandDefinition {
  const definition: CommandDefinition = {
    id: input.id,
    title: input.title,
    category: input.category,
    icon: input.icon ?? null,
    keywords: input.keywords ?? [],
    shortcuts: input.shortcuts ?? [],
    contexts: input.contexts,
    availability: input.availability ?? (() => ENABLED),
    run: input.run,
  }
  return {
    ...definition,
    ...(input.checked === undefined ? {} : { checked: input.checked }),
    ...(input.detail === undefined ? {} : { detail: input.detail }),
  }
}

function stateOf(app: App): AppState {
  return app.store.getState()
}

// Availability rules.

/** Enabled while the core runs with a document open (4.4). */
function needsDocument(app: App): Availability {
  const state = stateOf(app)
  return state.core.status === 'ready' && state.document !== null
    ? ENABLED
    : { state: 'disabled', reason: MESSAGES.core_not_running() }
}

/** Enabled while the core runs (4.4). */
function needsCore(app: App): Availability {
  return stateOf(app).core.status === 'ready'
    ? ENABLED
    : { state: 'disabled', reason: MESSAGES.core_not_running() }
}

/** needsDocument, and disabled while a feature command is active (export uses the model state, 3.18). */
function needsDocumentOutsideCommand(app: App): Availability {
  const active = stateOf(app).command
  if (active !== null) {
    return { state: 'disabled', reason: MESSAGES.finish_command_first(active.title) }
  }
  return needsDocument(app)
}

/** Whether a sketch is being edited (not while a plane is picked for a new one). */
function editingSketch(state: AppState): boolean {
  return state.context === 'sketch' && state.sketch !== null && state.sketch.sketchId !== null
}

/** Sketch tools are listed and active only while a sketch is being edited (3.3). */
function whileEditingSketch(app: App): Availability {
  return editingSketch(stateOf(app)) ? ENABLED : HIDDEN
}

// Selection helpers.

const FEATURE_KINDS: ReadonlySet<string> = new Set(['feature', 'datum_plane'])

function selectedFeatureIds(state: AppState): string[] {
  return state.selection.flatMap((item) =>
    FEATURE_KINDS.has(item.kind) && item.feature !== null ? [item.feature] : [],
  )
}

function selectedBodyIds(state: AppState): string[] {
  return state.selection.flatMap((item) =>
    item.kind === 'body' && item.body !== null ? [item.body] : [],
  )
}

function selectedParameterIds(state: AppState): string[] {
  return state.selection.flatMap((item) =>
    item.kind === 'parameter' ? [item.key.slice('parameter:'.length)] : [],
  )
}

function featureRecord(state: AppState, id: string) {
  return state.document?.recipe.features?.find((feature) => feature.id === id)
}

// Edit commands.

function undoAvailability(app: App): Availability {
  const state = stateOf(app)
  const core = needsDocument(app)
  if (core.state !== 'enabled') {
    return core
  }
  // Mod+Z cancels an active command (3.20); in sketch mode it also steps back through views.
  if (state.command !== null || editingSketch(state) || state.document?.undoLabel !== null) {
    return ENABLED
  }
  return { state: 'disabled', reason: MESSAGES.nothing_to_undo() }
}

function redoAvailability(app: App): Availability {
  const state = stateOf(app)
  const core = needsDocument(app)
  if (core.state !== 'enabled') {
    return core
  }
  if (state.command !== null) {
    return { state: 'disabled', reason: MESSAGES.finish_command_first(state.command.title) }
  }
  if (editingSketch(state) || state.document?.redoLabel !== null) {
    return ENABLED
  }
  return { state: 'disabled', reason: MESSAGES.nothing_to_redo() }
}

function deleteAvailability(app: App): Availability {
  const state = stateOf(app)
  if (state.context === 'sketch') {
    return editingSketch(state) && (state.sketch?.selection.length ?? 0) > 0
      ? needsDocument(app)
      : HIDDEN
  }
  const any =
    selectedFeatureIds(state).length > 0 ||
    selectedBodyIds(state).length > 0 ||
    selectedParameterIds(state).length > 0
  return any ? needsDocumentOutsideCommand(app) : HIDDEN
}

/** Deletes the selection: sketch items in sketch mode, else features, bodies and parameters. */
async function deleteSelection(app: App): Promise<void> {
  const state = stateOf(app)
  if (state.context === 'sketch') {
    await deleteSketchItems(app, state.sketch?.selection ?? [])
    return
  }
  const features = selectedFeatureIds(state)
  if (features.length > 0) {
    await deleteFeatures(app, features)
  }
  const bodies = (state.document?.recipe.bodies ?? []).filter((body) =>
    selectedBodyIds(state).includes(body.id),
  )
  const [firstBody] = bodies
  if (firstBody !== undefined) {
    // The core does not check use, so dependent features fail and the failure notification offers Undo (3.10).
    const label =
      bodies.length === 1 ? `Delete ${firstBody.name}` : `Delete ${String(bodies.length)} bodies`
    await commitEdits(
      app,
      label,
      bodies.map((body): Edit => ({ type: 'remove_body', id: body.id })),
    )
  }
  // The mutation queue applies the deletions in order.
  await Promise.all(selectedParameterIds(state).map((parameter) => deleteParameter(app, parameter)))
  app.store.setState((current) => ({
    selection: current.selection.filter(
      (item) => item.kind !== 'body' && item.kind !== 'parameter' && !FEATURE_KINDS.has(item.kind),
    ),
  }))
}

/** The one selected feature or body that F2 renames, or null. */
function renameTarget(state: AppState): SelectionItem | null {
  const [only] = state.selection
  if (state.selection.length !== 1 || only === undefined) {
    return null
  }
  return only.kind === 'body' || FEATURE_KINDS.has(only.kind) ? only : null
}

/** Whether keyboard focus is inside the browser, so a rename there edits the browser row. */
function focusInBrowser(): boolean {
  const active = typeof document === 'undefined' ? null : document.activeElement
  return active?.closest('[data-testid="browser"]') !== null && active !== null
}

function editFeatureAvailability(app: App): Availability {
  const state = stateOf(app)
  const features = selectedFeatureIds(state)
  const [only] = features
  if (features.length !== 1 || only === undefined) {
    return HIDDEN
  }
  if (featureRecord(state, only)?.type === 'import') {
    return { state: 'disabled', reason: MESSAGES.import_not_editable() }
  }
  return needsDocument(app)
}

function suppressAvailability(suppressed: boolean): (app: App) => Availability {
  return (app) => {
    const state = stateOf(app)
    const applies = selectedFeatureIds(state).some(
      (id) => (featureRecord(state, id)?.suppressed === true) !== suppressed,
    )
    return applies ? needsDocumentOutsideCommand(app) : HIDDEN
  }
}

function suppressSelection(suppressed: boolean): (app: App) => Promise<void> {
  return async (app) => {
    const state = stateOf(app)
    const ids = selectedFeatureIds(state).filter(
      (id) => (featureRecord(state, id)?.suppressed === true) !== suppressed,
    )
    await setSuppressed(app, ids, suppressed)
  }
}

// View commands.

const ORIGIN_PLANE_FRAMES: Readonly<Record<'XY' | 'XZ' | 'YZ', Placement>> = {
  XY: { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, 0, 1] },
  XZ: { origin: [0, 0, 0], x_axis: [1, 0, 0], normal: [0, -1, 0] },
  YZ: { origin: [0, 0, 0], x_axis: [0, 1, 0], normal: [1, 0, 0] },
}

/** The plane Look at looks at: target point, normal and in-plane y axis (3.14). */
type LookTarget = { readonly target: Vector3; readonly normal: Vector3; readonly yAxis: Vector3 }

function lookTargetOf(placement: Placement, target: Vector3 = placement.origin): LookTarget {
  return {
    target,
    normal: placement.normal,
    yAxis: cross(placement.normal, placement.x_axis),
  }
}

function isOriginPlane(origin: OriginName | null): origin is 'XY' | 'XZ' | 'YZ' {
  return origin === 'XY' || origin === 'XZ' || origin === 'YZ'
}

/** What Look at frames for the state: the sketch plane, or one selected flat face or plane. */
function lookTarget(state: AppState): LookTarget | null {
  if (editingSketch(state)) {
    const sketchId = state.sketch?.sketchId ?? null
    const placement =
      sketchId === null ? undefined : state.document?.sketches[sketchId]?.solution.placement
    return placement === undefined ? null : lookTargetOf(placement)
  }
  const [only] = state.selection
  if (state.selection.length !== 1 || only === undefined) {
    return null
  }
  if (only.kind === 'origin_plane' && isOriginPlane(only.origin)) {
    return lookTargetOf(ORIGIN_PLANE_FRAMES[only.origin])
  }
  if (only.kind === 'datum_plane' && only.feature !== null) {
    const placement = state.document?.datums[only.feature] ?? null
    return placement === null ? null : lookTargetOf(placement)
  }
  const element = only.element
  if (only.kind === 'face' && element?.surface === 'plane' && element.placement != null) {
    return lookTargetOf(element.placement, element.centroid)
  }
  return null
}

function viewChange(app: App, change: Partial<ViewState>): void {
  app.store.setState((state) => ({ view: { ...state.view, ...change } }))
}

type FilterKind = keyof ViewState['filter']

const FILTERS: readonly { readonly kind: FilterKind; readonly title: string }[] = [
  { kind: 'body', title: 'Select bodies' },
  { kind: 'face', title: 'Select faces' },
  { kind: 'edge', title: 'Select edges' },
  { kind: 'vertex', title: 'Select vertices' },
]

const STANDARD_VIEWS: readonly {
  readonly view: StandardView
  readonly title: string
  readonly icon: LucideIcon
}[] = [
  { view: 'top', title: 'View: Top', icon: ArrowDown },
  { view: 'bottom', title: 'View: Bottom', icon: ArrowUp },
  { view: 'front', title: 'View: Front', icon: Box },
  { view: 'back', title: 'View: Back', icon: Box },
  { view: 'left', title: 'View: Left', icon: ArrowRight },
  { view: 'right', title: 'View: Right', icon: ArrowLeft },
  { view: 'iso', title: 'View: Home', icon: House },
]

/** The selection keys whose visibility V toggles: bodies, sketches, datum planes and origin items. */
function visibilityTargets(state: AppState): readonly SelectionItem[] {
  return state.selection.filter(
    (item) =>
      item.kind === 'body' ||
      item.kind === 'datum_plane' ||
      item.kind === 'origin_plane' ||
      item.kind === 'origin_axis' ||
      item.kind === 'origin_point' ||
      (item.kind === 'feature' &&
        item.feature !== null &&
        featureRecord(state, item.feature)?.type === 'sketch'),
  )
}

async function toggleVisibility(app: App): Promise<void> {
  const state = stateOf(app)
  const edits: Promise<void>[] = []
  for (const item of visibilityTargets(state)) {
    if (item.kind === 'body' && item.body !== null) {
      const body = state.document?.recipe.bodies?.find((candidate) => candidate.id === item.body)
      if (body !== undefined) {
        edits.push(setBodyVisible(app, body.id, body.visible === false))
      }
    } else {
      setSessionVisible(app, item.key, !isSessionVisible(state, item.key))
    }
  }
  // The mutation queue applies the body edits in order.
  await Promise.all(edits)
}

// Sketch commands.

const DRAWING_TOOLS: readonly {
  readonly tool: SketchToolId
  readonly title: string
  readonly icon: LucideIcon
  readonly shortcuts: readonly string[]
}[] = [
  { tool: 'line', title: 'Line', icon: Slash, shortcuts: ['L'] },
  {
    tool: 'rectangle_2point',
    title: '2-point rectangle',
    icon: RectangleHorizontal,
    shortcuts: ['R'],
  },
  { tool: 'rectangle_center', title: 'Center rectangle', icon: SquareDashed, shortcuts: [] },
  { tool: 'circle', title: 'Circle', icon: Circle, shortcuts: ['C'] },
  { tool: 'arc_3point', title: 'Arc', icon: Spline, shortcuts: [] },
  { tool: 'point', title: 'Point', icon: Dot, shortcuts: [] },
  { tool: 'dimension', title: 'Dimension', icon: RulerDimensionLine, shortcuts: ['D'] },
]

const DRAWING_TOOL_IDS: ReadonlySet<SketchToolId | null> = new Set(
  DRAWING_TOOLS.filter((entry) => entry.tool !== 'dimension').map((entry) => entry.tool),
)

/** Constraint tools in the order of 3.16.6. */
const CONSTRAINT_TOOLS: readonly {
  readonly type: keyof typeof CONSTRAINT_ICONS
  readonly title: string
  readonly shortcuts: readonly string[]
}[] = [
  { type: 'coincident', title: 'Coincident', shortcuts: ['I'] },
  { type: 'point_on_object', title: 'Point on object', shortcuts: [] },
  { type: 'horizontal', title: 'Horizontal', shortcuts: ['H'] },
  { type: 'vertical', title: 'Vertical', shortcuts: ['V'] },
  { type: 'parallel', title: 'Parallel', shortcuts: [] },
  { type: 'perpendicular', title: 'Perpendicular', shortcuts: [] },
  { type: 'tangent', title: 'Tangent', shortcuts: [] },
  { type: 'collinear', title: 'Collinear', shortcuts: [] },
  { type: 'concentric', title: 'Concentric', shortcuts: [] },
  { type: 'equal', title: 'Equal', shortcuts: [] },
  { type: 'symmetric', title: 'Symmetric', shortcuts: [] },
  { type: 'midpoint', title: 'Midpoint', shortcuts: [] },
  { type: 'fix', title: 'Fix', shortcuts: [] },
]

const ENTITY_NOUNS = { point: 'Point', line: 'Line', circle: 'Circle', arc: 'Arc' } as const

/** Every entity of the edited sketch as an in-sketch selection item (Mod+A, 3.16.10). */
function everySketchEntity(state: AppState): SelectionItem[] {
  const sketchId = state.sketch?.sketchId ?? null
  const solution = sketchId === null ? undefined : state.document?.sketches[sketchId]?.solution
  if (sketchId === null || solution === undefined) {
    return []
  }
  return solution.entities.map((entity) => sketchEntityItem(sketchId, entity))
}

/** The in-sketch selection item of a solved entity: a point by its reference, a curve by its id. */
function sketchEntityItem(
  sketchId: string,
  entity: SketchSolution['entities'][number],
): SelectionItem {
  const point = entity.type === 'point'
  return {
    key: point ? `point:${sketchId}:${entity.id}` : `sketch:${sketchId}:${entity.id}`,
    kind: point ? 'sketch_point' : 'sketch_curve',
    label: `${ENTITY_NOUNS[entity.type]} ${entity.id}`,
    body: null,
    feature: sketchId,
    element: null,
    sketchItem: point
      ? { kind: 'point', sketch: sketchId, point: entity.id }
      : { kind: 'entity', sketch: sketchId, entity: entity.id, entityType: entity.type },
    origin: null,
  }
}

function setSketchSession(app: App, change: Partial<NonNullable<AppState['sketch']>>): void {
  app.store.setState((state) =>
    state.sketch === null ? {} : { sketch: { ...state.sketch, ...change } },
  )
}

/** X: toggles construction on the selected entities, or the mode for new entities (3.16.4). */
async function toggleConstruction(app: App): Promise<void> {
  const sketch = stateOf(app).sketch
  if (sketch === null) {
    return
  }
  const entities = sketch.selection.flatMap((item) => {
    const reference = item.sketchItem
    if (reference?.kind === 'entity') {
      return [reference.entity]
    }
    // A point entity is selected by its point reference, which is its id.
    if (reference?.kind === 'point' && !reference.point.includes('.')) {
      return [reference.point]
    }
    return []
  })
  if (DRAWING_TOOL_IDS.has(sketch.tool) || entities.length === 0) {
    setSketchSession(app, { construction: !sketch.construction })
    return
  }
  await commitSketchChange(app, 'Construction', {
    toggleConstruction: entities,
    requested: [],
    inferred: [],
  })
}

// Settings commands.

const THEMES: readonly {
  readonly theme: Settings['theme']
  readonly title: string
  readonly icon: LucideIcon
}[] = [
  { theme: 'system', title: 'Theme: System', icon: Monitor },
  { theme: 'light', title: 'Theme: Light', icon: Sun },
  { theme: 'dark', title: 'Theme: Dark', icon: Moon },
]

const NAVIGATIONS: readonly {
  readonly navigation: Settings['navigation']
  readonly title: string
}[] = [
  { navigation: 'fusion', title: 'Navigation: Fusion' },
  { navigation: 'freecad', title: 'Navigation: FreeCAD' },
]

const SCALES: readonly Settings['uiScale'][] = [1, 1.25, 1.5, 2, 3]

const UNITS: readonly Units[] = ['mm', 'cm', 'm', 'in']

const EXPORTS: readonly {
  readonly format: ExportRequest['format']
  readonly title: string
}[] = [
  { format: '3mf', title: 'Export 3MF' },
  { format: 'stl', title: 'Export STL' },
  { format: 'obj', title: 'Export OBJ' },
  { format: 'step', title: 'Export STEP' },
]

function setSide(app: App, side: AppState['panels']['side']): void {
  app.store.setState((state) => ({ panels: { ...state.panels, side } }))
}

/** The built-in commands for a platform (the platform decides desktop-only shortcuts). */
export function builtinCommands(platform: Platform): CommandDefinition[] {
  return [
    command({
      id: 'app.palette',
      title: 'Command palette',
      category: 'View',
      icon: Search,
      keywords: ['search', 'commands'],
      shortcuts: ['Mod+K', 'S'],
      contexts: ALL,
      run: (app) => {
        app.store.setState((state) => ({
          panels: { ...state.panels, paletteOpen: !state.panels.paletteOpen },
        }))
      },
    }),

    // File.
    command({
      id: 'file.new',
      title: 'New',
      category: 'File',
      icon: FilePlus,
      keywords: ['new document'],
      // Browsers reserve Ctrl+N (3.5).
      shortcuts: platform.host === 'desktop' ? ['Mod+N'] : [],
      contexts: ALL,
      availability: needsCore,
      run: (app) => newDocument(app),
    }),
    command({
      id: 'file.open',
      title: 'Open',
      category: 'File',
      icon: FolderOpen,
      keywords: ['load', 'file'],
      shortcuts: ['Mod+O'],
      contexts: ALL,
      availability: needsCore,
      run: (app) => openWithPicker(app),
    }),
    command({
      id: 'file.save',
      title: 'Save',
      category: 'File',
      icon: Save,
      shortcuts: ['Mod+S'],
      contexts: ALL,
      availability: needsDocument,
      run: (app) => save(app),
    }),
    command({
      id: 'file.save_as',
      title: 'Save as',
      category: 'File',
      icon: SaveAll,
      shortcuts: ['Mod+Shift+S'],
      contexts: ALL,
      availability: needsDocument,
      run: (app) => saveAs(app),
    }),
    command({
      id: 'file.export',
      title: 'Export',
      category: 'File',
      icon: Download,
      keywords: ['3mf', 'stl', 'obj', 'step', 'download'],
      shortcuts: ['Mod+Shift+E'],
      contexts: ALL,
      availability: needsDocumentOutsideCommand,
      run: (app) => {
        requestUi(app, { kind: 'export' })
      },
    }),

    // Edit.
    command({
      id: 'edit.undo',
      title: 'Undo',
      category: 'Edit',
      icon: Undo2,
      shortcuts: ['Mod+Z'],
      contexts: ALL,
      availability: undoAvailability,
      detail: (app) => {
        const label = stateOf(app).document?.undoLabel ?? null
        return label === null ? null : `Undo ${label}`
      },
      run: (app) => undo(app),
    }),
    command({
      id: 'edit.redo',
      title: 'Redo',
      category: 'Edit',
      icon: Redo2,
      shortcuts: ['Mod+Shift+Z', 'Mod+Y'],
      contexts: ALL,
      availability: redoAvailability,
      detail: (app) => {
        const label = stateOf(app).document?.redoLabel ?? null
        return label === null ? null : `Redo ${label}`
      },
      run: (app) => redo(app),
    }),
    command({
      id: 'edit.delete',
      title: 'Delete',
      category: 'Edit',
      icon: Trash2,
      keywords: ['remove'],
      shortcuts: ['Delete', 'Backspace'],
      contexts: ALL,
      availability: deleteAvailability,
      run: (app) => deleteSelection(app),
    }),
    command({
      id: 'edit.rename',
      title: 'Rename',
      category: 'Edit',
      icon: TextCursorInput,
      shortcuts: ['F2'],
      contexts: ALL,
      availability: (app) =>
        renameTarget(stateOf(app)) === null ? HIDDEN : needsDocumentOutsideCommand(app),
      run: (app) => {
        const target = renameTarget(stateOf(app))
        if (target === null) {
          return
        }
        // The browser shows bodies, sketches and datum planes; every feature has a timeline item.
        const type =
          target.feature === null ? null : featureRecord(stateOf(app), target.feature)?.type
        const inBrowser = type === 'sketch' || type === 'datum_plane'
        const where =
          target.kind === 'body' || (inBrowser && focusInBrowser()) ? 'browser' : 'timeline'
        if (where === 'browser') {
          app.store.setState((state) => ({ panels: { ...state.panels, browserOpen: true } }))
        }
        requestUi(app, { kind: 'rename', key: target.key, where })
      },
    }),
    command({
      id: 'edit.edit_feature',
      title: 'Edit feature',
      category: 'Edit',
      icon: Pencil,
      contexts: MODEL_PRINT,
      availability: editFeatureAvailability,
      run: (app) => {
        const [only] = selectedFeatureIds(stateOf(app))
        return only === undefined ? undefined : editFeature(app, only)
      },
    }),
    command({
      id: 'edit.suppress',
      title: 'Suppress',
      category: 'Edit',
      icon: CircleX,
      keywords: ['disable'],
      contexts: MODEL_PRINT,
      availability: suppressAvailability(true),
      run: suppressSelection(true),
    }),
    command({
      id: 'edit.unsuppress',
      title: 'Unsuppress',
      category: 'Edit',
      icon: CircleCheck,
      keywords: ['enable'],
      contexts: MODEL_PRINT,
      availability: suppressAvailability(false),
      run: suppressSelection(false),
    }),
    command({
      id: 'edit.clear_selection',
      title: 'Clear selection',
      category: 'Edit',
      icon: MousePointer2,
      keywords: ['deselect'],
      contexts: ALL,
      run: (app) => {
        clearSelection(app)
        setSketchSession(app, { selection: [] })
      },
    }),

    // Contexts.
    ...(['model', 'sketch', 'print'] as const).map((context) =>
      command({
        id: `context.${context}`,
        title: context === 'model' ? 'Model' : context === 'sketch' ? 'Sketch' : 'Print',
        category: 'View',
        icon: context === 'model' ? Cuboid : context === 'sketch' ? Pencil : Printer,
        contexts: ALL,
        availability: needsDocument,
        checked: (app) => stateOf(app).context === context,
        run: (app) => setContext(app, context),
      }),
    ),

    // View.
    command({
      id: 'view.browser',
      title: 'Toggle browser',
      category: 'View',
      icon: PanelLeft,
      keywords: ['tree', 'panel'],
      shortcuts: ['Mod+B'],
      contexts: ALL,
      checked: (app) => stateOf(app).panels.browserOpen,
      run: (app) => {
        app.store.setState((state) => ({
          panels: { ...state.panels, browserOpen: !state.panels.browserOpen },
        }))
      },
    }),
    command({
      id: 'view.timeline',
      title: 'Toggle timeline',
      category: 'View',
      icon: PanelBottom,
      keywords: ['history'],
      contexts: ALL,
      checked: (app) => !stateOf(app).panels.timelineCollapsed,
      run: (app) => {
        app.store.setState((state) => ({
          panels: { ...state.panels, timelineCollapsed: !state.panels.timelineCollapsed },
        }))
      },
    }),
    command({
      id: 'view.parameters',
      title: 'Parameters',
      category: 'View',
      icon: SlidersHorizontal,
      keywords: ['variables', 'expressions'],
      contexts: ALL,
      run: (app) => {
        setSide(app, 'parameters')
      },
    }),
    command({
      id: 'view.problems',
      title: 'Problems',
      category: 'View',
      icon: TriangleAlert,
      keywords: ['errors', 'warnings'],
      contexts: ALL,
      run: (app) => {
        setSide(app, 'problems')
      },
    }),
    command({
      id: 'view.health',
      title: 'Print check',
      category: 'View',
      icon: HeartPulse,
      keywords: ['findings', 'checks'],
      contexts: ALL,
      // As the badge does (3.17): the panel opens, and a missing or stale report is checked again.
      run: (app) => openPrintHealth(app),
    }),
    command({
      id: 'view.fit',
      title: 'Fit to view',
      category: 'View',
      icon: Maximize,
      keywords: ['zoom', 'frame'],
      shortcuts: ['F6', 'Shift+F'],
      contexts: ALL,
      run: (app) => {
        app.viewport()?.fit()
      },
    }),
    command({
      id: 'view.home',
      title: 'Home view',
      category: 'View',
      icon: House,
      keywords: ['iso', 'isometric', 'reset'],
      shortcuts: ['Shift+H'],
      contexts: ALL,
      run: (app) => {
        const handle = app.viewport()
        handle?.setStandardView('iso')
        handle?.fit()
      },
    }),
    command({
      id: 'view.look_at',
      title: 'Look at',
      category: 'View',
      icon: ScanEye,
      keywords: ['normal to', 'face'],
      shortcuts: ['Shift+L'],
      contexts: ALL,
      availability: (app) =>
        lookTarget(stateOf(app)) === null
          ? { state: 'disabled', reason: MESSAGES.look_at_disabled() }
          : ENABLED,
      run: (app) => {
        const look = lookTarget(stateOf(app))
        if (look !== null) {
          app.viewport()?.lookAlong(look.normal, look.yAxis, look.target)
        }
      },
    }),
    command({
      id: 'view.projection',
      title: 'Orthographic',
      category: 'View',
      icon: Cuboid,
      keywords: ['perspective', 'projection', 'parallel'],
      shortcuts: ['Shift+P'],
      contexts: ALL,
      checked: (app) => stateOf(app).view.projection === 'orthographic',
      run: (app) => {
        app.viewport()?.toggleProjection()
      },
    }),
    command({
      id: 'view.section',
      title: 'Section view',
      category: 'View',
      icon: ScanLine,
      keywords: ['cut', 'clip', 'cross section'],
      shortcuts: ['Shift+X'],
      contexts: ALL,
      checked: (app) => stateOf(app).view.section !== null,
      run: (app) => {
        const section = stateOf(app).view.section
        viewChange(app, {
          section: section === null ? { plane: 'XY', offset: '', flipped: false } : null,
        })
      },
    }),
    command({
      id: 'view.visibility',
      title: 'Show or hide selection',
      category: 'View',
      icon: Eye,
      keywords: ['hide', 'show', 'visibility'],
      shortcuts: ['V'],
      contexts: MODEL_PRINT,
      availability: (app) =>
        visibilityTargets(stateOf(app)).length === 0 ? HIDDEN : needsDocument(app),
      run: (app) => toggleVisibility(app),
    }),
    ...STANDARD_VIEWS.map((entry) =>
      command({
        id: `view.standard.${entry.view}`,
        title: entry.title,
        category: 'View',
        icon: entry.icon,
        keywords: ['view', 'camera'],
        contexts: ALL,
        checked: (app) => stateOf(app).view.named === entry.view,
        run: (app) => {
          app.viewport()?.setStandardView(entry.view)
        },
      }),
    ),
    ...FILTERS.map((entry) =>
      command({
        id: `view.filter.${entry.kind}`,
        title: entry.title,
        category: 'View',
        icon: Funnel,
        keywords: ['selection filter', 'pick'],
        contexts: MODEL_PRINT,
        checked: (app) => stateOf(app).view.filter[entry.kind],
        run: (app) => {
          const filter = stateOf(app).view.filter
          viewChange(app, { filter: { ...filter, [entry.kind]: !filter[entry.kind] } })
        },
      }),
    ),
    command({
      id: 'view.lock',
      title: 'Lock selection',
      category: 'View',
      icon: Lock,
      keywords: ['freeze', 'not pickable'],
      contexts: MODEL_PRINT,
      availability: (app) => {
        const state = stateOf(app)
        return selectedBodyIds(state).some((id) => !state.view.locked.includes(id))
          ? ENABLED
          : HIDDEN
      },
      run: (app) => {
        for (const id of selectedBodyIds(stateOf(app))) {
          setLocked(app, id, true)
        }
      },
    }),
    command({
      id: 'view.unlock',
      title: 'Unlock selection',
      category: 'View',
      icon: LockOpen,
      contexts: MODEL_PRINT,
      availability: (app) => {
        const state = stateOf(app)
        return selectedBodyIds(state).some((id) => state.view.locked.includes(id))
          ? ENABLED
          : HIDDEN
      },
      run: (app) => {
        for (const id of selectedBodyIds(stateOf(app))) {
          setLocked(app, id, false)
        }
      },
    }),

    // Sketch.
    ...DRAWING_TOOLS.map((entry) =>
      command({
        id: `sketch.${entry.tool}`,
        title: entry.title,
        category: 'Sketch',
        icon: entry.icon,
        shortcuts: entry.shortcuts,
        contexts: SKETCH,
        availability: whileEditingSketch,
        checked: (app) => stateOf(app).sketch?.tool === entry.tool,
        run: (app) => {
          setSketchTool(app, entry.tool)
        },
      }),
    ),
    command({
      id: 'sketch.construction',
      title: 'Construction',
      category: 'Sketch',
      icon: SquareDashed,
      keywords: ['reference geometry', 'helper'],
      shortcuts: ['X'],
      contexts: SKETCH,
      availability: whileEditingSketch,
      checked: (app) => stateOf(app).sketch?.construction === true,
      run: (app) => toggleConstruction(app),
    }),
    ...CONSTRAINT_TOOLS.map((entry) =>
      command({
        id: `sketch.constrain_${entry.type}`,
        title: entry.title,
        category: 'Constrain',
        icon: CONSTRAINT_ICONS[entry.type],
        keywords: ['constraint'],
        shortcuts: entry.shortcuts,
        contexts: SKETCH,
        availability: whileEditingSketch,
        checked: (app) => stateOf(app).sketch?.tool === `constrain_${entry.type}`,
        run: (app) => {
          // The sketch controller applies the constraint at once when the selection fits (3.16.6).
          setSketchTool(app, `constrain_${entry.type}`)
        },
      }),
    ),
    command({
      id: 'sketch.select_all',
      title: 'Select all',
      category: 'Sketch',
      icon: CheckCheck,
      shortcuts: ['Mod+A'],
      contexts: SKETCH,
      availability: whileEditingSketch,
      run: (app) => {
        setSketchSession(app, { selection: everySketchEntity(stateOf(app)) })
      },
    }),
    command({
      id: 'sketch.show_constraints',
      title: 'Show constraints',
      category: 'Sketch',
      icon: Layers,
      keywords: ['glyphs'],
      contexts: SKETCH,
      availability: whileEditingSketch,
      checked: (app) => stateOf(app).sketch?.showConstraints === true,
      run: (app) => {
        setSketchSession(app, { showConstraints: stateOf(app).sketch?.showConstraints !== true })
      },
    }),
    command({
      id: 'sketch.finish',
      title: 'Finish sketch',
      category: 'Sketch',
      icon: CircleCheck,
      keywords: ['done', 'exit sketch'],
      shortcuts: ['Mod+Enter'],
      contexts: SKETCH,
      availability: whileEditingSketch,
      run: (app) => finishSketch(app),
    }),

    // Print.
    command({
      id: 'print.printer',
      title: 'Choose printer',
      category: 'Print',
      icon: Printer,
      keywords: ['profile', 'machine'],
      contexts: PRINT,
      // Changing the printer is an edit, which waits for the active command (3.7).
      availability: needsDocumentOutsideCommand,
      run: (app) => {
        requestUi(app, { kind: 'printer' })
      },
    }),
    command({
      id: 'print.overhangs',
      title: 'Overhangs',
      category: 'Print',
      icon: Ruler,
      keywords: ['overhang map', 'supports'],
      contexts: PRINT,
      checked: (app) => stateOf(app).print.overhangMap,
      run: (app) => {
        app.store.setState((state) => ({
          print: { ...state.print, overhangMap: !state.print.overhangMap },
        }))
      },
    }),
    command({
      id: 'print.checks',
      title: 'Checks',
      category: 'Print',
      icon: ListChecks,
      contexts: PRINT,
      run: (app) => {
        requestUi(app, { kind: 'checks' })
      },
    }),
    command({
      id: 'print.check_now',
      title: 'Check print',
      category: 'Print',
      icon: Wrench,
      keywords: ['analyze', 'health'],
      contexts: ALL,
      availability: needsDocument,
      run: (app) => runPrintAnalysis(app),
    }),
    ...EXPORTS.map((entry) =>
      command({
        id: `print.export_${entry.format}`,
        title: entry.title,
        category: 'Print',
        icon: FileOutput,
        keywords: ['export', 'download'],
        contexts: ALL,
        availability: needsDocumentOutsideCommand,
        run: (app) => exportModel(app, { format: entry.format, scope: 'visible' }),
      }),
    ),

    // Document units.
    ...UNITS.map((unit) =>
      command({
        id: `document.units.${unit}`,
        title: `Units: ${unit}`,
        category: 'Edit',
        icon: Type,
        keywords: ['units', 'millimeters', 'inches'],
        contexts: ALL,
        availability: needsDocumentOutsideCommand,
        checked: (app) => stateOf(app).document?.recipe.units === unit,
        run: (app) => setUnits(app, unit),
      }),
    ),

    // Settings.
    ...THEMES.map((entry) =>
      command({
        id: `settings.theme.${entry.theme}`,
        title: entry.title,
        category: 'Settings',
        icon: entry.icon,
        keywords: ['appearance', 'color scheme'],
        contexts: ALL,
        checked: (app) => stateOf(app).settings.theme === entry.theme,
        run: (app) => {
          updateSettings(app, { theme: entry.theme })
        },
      }),
    ),
    command({
      id: 'settings.theme.toggle',
      title: 'Toggle dark theme',
      category: 'Settings',
      icon: Moon,
      keywords: ['appearance', 'dark mode', 'light mode'],
      contexts: ALL,
      checked: (app) => resolvedTheme(stateOf(app).settings) === 'dark',
      run: (app) => {
        updateSettings(app, { theme: toggledTheme(resolvedTheme(stateOf(app).settings)) })
      },
    }),
    ...NAVIGATIONS.map((entry) =>
      command({
        id: `settings.navigation.${entry.navigation}`,
        title: entry.title,
        category: 'Settings',
        icon: MousePointer2,
        keywords: ['mouse', 'orbit', 'pan'],
        contexts: ALL,
        checked: (app) => stateOf(app).settings.navigation === entry.navigation,
        run: (app) => {
          updateSettings(app, { navigation: entry.navigation })
        },
      }),
    ),
    ...SCALES.map((scale) =>
      command({
        id: `settings.scale.${String(Math.round(scale * 100))}`,
        title: `UI scale: ${String(Math.round(scale * 100))}%`,
        category: 'Settings',
        icon: ZoomIn,
        keywords: ['zoom', 'size', 'text size'],
        contexts: ALL,
        checked: (app) => stateOf(app).settings.uiScale === scale,
        run: (app) => {
          updateSettings(app, { uiScale: scale })
        },
      }),
    ),
  ]
}
