import {
  ArrowDown,
  ArrowDownToLine,
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
  Grid2x2,
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
  Repeat,
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
  Keyboard,
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
import { openSidePanel, setBrowserOpen, togglePanel } from '../app/actions/panels'
import {
  cancelLayOnFace,
  openPrintHealth,
  runPrintAnalysis,
  setPrintOrientation,
  startLayOnFace,
} from '../app/actions/print'
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
import { DOCUMENT_UNITS } from '../expression/format'
import type { Edit, Placement, SketchSolution } from '../generated/protocol'
import { parseSelectionKey } from '../inspector/selection-values'
import { resolvedTheme } from '../layout/color-scheme'
import { BROWSER_PANEL_ID } from '../layout/panel-ids'
import { requestUi } from '../layout/requests'
import { MESSAGES } from '../messages'
import {
  NAVIGATIONS,
  scalePercent,
  SETTING_LABELS,
  THEMES,
  UI_SCALES,
  type Settings,
} from '../settings/settings'
import { CONSTRAINT_TITLES, CONSTRAINT_TYPES, type ConstraintType } from '../sketch/constraints'
import type { SketchToolId } from '../sketch/session'
import { selectSketchRadioTooltip } from '../state/selectors'
import type { AppContext, AppState, OriginName, SelectionItem, ViewState } from '../state/store'
import { isSessionVisible } from '../state/visibility'
import { toggledTheme } from '../theme/theme'
import type { StandardView, Vector3 } from '../viewport/camera'
import { cross } from '../viewport/math'
import { ORIGIN_PLANE_PLACEMENTS } from '../viewport/planes'
import { CONSTRAINT_ICONS } from './icons'
import { keywordsFor } from './keywords'
import {
  disabledWith,
  type Availability,
  type CommandCategory,
  type CommandDefinition,
} from './registry'

// The built-in UI commands of ui.md 3.5: everything the registry offers besides the feature
// commands of the catalog (features.ts).

const ALL: readonly AppContext[] = ['model', 'sketch', 'print']
const MODEL_PRINT: readonly AppContext[] = ['model', 'print']
const MODEL_SKETCH: readonly AppContext[] = ['model', 'sketch']
const SKETCH: readonly AppContext[] = ['sketch']
const PRINT: readonly AppContext[] = ['print']

const ENABLED: Availability = { state: 'enabled' }
const HIDDEN: Availability = { state: 'hidden' }

type CommandInput = {
  readonly id: string
  readonly title: string
  readonly label?: (app: App) => string
  readonly category: CommandCategory
  readonly icon?: LucideIcon | null
  /** What the command does (2.10); required for sketch tools, Finish sketch and constraints. */
  readonly description?: string
  readonly shortcuts?: readonly string[]
  readonly contexts: readonly AppContext[]
  readonly availability?: (app: App) => Availability
  readonly checked?: (app: App) => boolean
  readonly detail?: (app: App) => string | null
  readonly run: CommandDefinition['run']
}

/** A command with the defaults: no icon or shortcuts, always enabled; keywords from COMMAND_KEYWORDS. */
function command(input: CommandInput): CommandDefinition {
  const definition: CommandDefinition = {
    id: input.id,
    title: input.title,
    category: input.category,
    icon: input.icon ?? null,
    keywords: keywordsFor(input.id),
    shortcuts: input.shortcuts ?? [],
    contexts: input.contexts,
    availability: input.availability ?? (() => ENABLED),
    run: input.run,
  }
  return {
    ...definition,
    ...(input.label === undefined ? {} : { label: input.label }),
    ...(input.description === undefined ? {} : { description: input.description }),
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
    : disabledWith('core_not_running', MESSAGES.core_not_running())
}

/** Enabled while the core runs (4.4). */
function needsCore(app: App): Availability {
  return stateOf(app).core.status === 'ready'
    ? ENABLED
    : disabledWith('core_not_running', MESSAGES.core_not_running())
}

/** needsDocument, and disabled while a feature command is active (export uses the model state, 3.18). */
function needsDocumentOutsideCommand(app: App): Availability {
  const active = stateOf(app).command
  if (active !== null) {
    return disabledWith('finish_command_first', MESSAGES.finish_command_first(active.title))
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
  return state.selection.flatMap((item) => {
    const parsed = parseSelectionKey(item.key)
    return parsed?.kind === 'parameter' ? [parsed.id] : []
  })
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
  // Mod+Z steps back through an active command, then cancels it (3.7, 3.20); in sketch mode it
  // also steps back through views.
  if (state.command !== null || editingSketch(state) || state.document?.undoLabel !== null) {
    return ENABLED
  }
  return disabledWith('nothing_to_undo', MESSAGES.nothing_to_undo())
}

function redoAvailability(app: App): Availability {
  const state = stateOf(app)
  const core = needsDocument(app)
  if (core.state !== 'enabled') {
    return core
  }
  // In a command, redo steps forward through its history (3.7, 3.20).
  if (state.command !== null) {
    return state.command.future.length > 0
      ? ENABLED
      : disabledWith('finish_command_first', MESSAGES.finish_command_first(state.command.title))
  }
  if (editingSketch(state) || state.document?.redoLabel !== null) {
    return ENABLED
  }
  return disabledWith('nothing_to_redo', MESSAGES.nothing_to_redo())
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
  return active?.closest(`#${BROWSER_PANEL_ID}`) !== null && active !== null
}

function editFeatureAvailability(app: App): Availability {
  const state = stateOf(app)
  const features = selectedFeatureIds(state)
  const [only] = features
  if (features.length !== 1 || only === undefined) {
    return HIDDEN
  }
  if (featureRecord(state, only)?.type === 'import') {
    return disabledWith('import_not_editable', MESSAGES.import_not_editable())
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

/** The feature command Repeat starts again: the last one started in this session (3.5). */
function repeatedCommand(app: App): CommandDefinition | undefined {
  const type = stateOf(app).lastFeatureType
  return type === null ? undefined : app.commands.get(`feature.${type}`)
}

function repeatAvailability(app: App): Availability {
  const repeated = repeatedCommand(app)
  return repeated === undefined
    ? disabledWith('nothing_to_repeat', MESSAGES.nothing_to_repeat())
    : repeated.availability(app)
}

// View commands.

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
    return lookTargetOf(ORIGIN_PLANE_PLACEMENTS[only.origin])
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
  readonly description: string
}[] = [
  {
    tool: 'line',
    title: 'Line',
    icon: Slash,
    shortcuts: ['L'],
    description: 'Draws connected straight lines. Enter or a double-click ends the line.',
  },
  {
    tool: 'rectangle_2point',
    title: '2-point rectangle',
    icon: RectangleHorizontal,
    shortcuts: ['R'],
    description: 'Draws a rectangle from two opposite corners.',
  },
  {
    tool: 'rectangle_center',
    title: 'Center rectangle',
    icon: SquareDashed,
    shortcuts: [],
    description: 'Draws a rectangle from its center and a corner.',
  },
  {
    tool: 'circle',
    title: 'Circle',
    icon: Circle,
    shortcuts: ['C'],
    description: 'Draws a circle from its center and a point on it.',
  },
  {
    tool: 'arc_3point',
    title: 'Arc',
    icon: Spline,
    shortcuts: [],
    description: 'Draws an arc through its start, its end and a point on it.',
  },
  {
    tool: 'point',
    title: 'Point',
    icon: Dot,
    shortcuts: [],
    description: 'Places a point, for example to mark a hole center.',
  },
  {
    tool: 'dimension',
    title: 'Dimension',
    icon: RulerDimensionLine,
    shortcuts: ['D'],
    description:
      'Sets an exact length, distance, diameter, radius or angle. Type a value or a parameter name.',
  },
]

const DRAWING_TOOL_IDS: ReadonlySet<SketchToolId | null> = new Set(
  DRAWING_TOOLS.filter((entry) => entry.tool !== 'dimension').map((entry) => entry.tool),
)

/** What each constraint tool adds to its constraint type (3.16.6): shortcuts and a description. */
const CONSTRAINT_TOOLS: Readonly<
  Record<ConstraintType, { readonly shortcuts: readonly string[]; readonly description: string }>
> = {
  coincident: {
    shortcuts: ['I'],
    description: 'Makes two points meet, or puts a point on a curve.',
  },
  point_on_object: { shortcuts: [], description: 'Keeps a point on a line, circle or arc.' },
  horizontal: {
    shortcuts: ['H'],
    description: 'Makes a line horizontal, or lines up two points horizontally.',
  },
  vertical: {
    shortcuts: ['V'],
    description: 'Makes a line vertical, or lines up two points vertically.',
  },
  parallel: { shortcuts: [], description: 'Makes two lines parallel.' },
  perpendicular: { shortcuts: [], description: 'Makes two lines meet at a right angle.' },
  tangent: {
    shortcuts: [],
    description: 'Makes a line and a curve, or two curves, touch smoothly.',
  },
  collinear: { shortcuts: [], description: 'Puts two lines on the same straight line.' },
  concentric: { shortcuts: [], description: 'Gives two circles or arcs the same center.' },
  equal: {
    shortcuts: [],
    description: 'Makes two lines the same length, or two circles or arcs the same size.',
  },
  symmetric: { shortcuts: [], description: 'Places two points symmetrically about a line.' },
  midpoint: { shortcuts: [], description: 'Puts a point at the middle of a line.' },
  fix: { shortcuts: [], description: 'Locks points or curves where they are.' },
}

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

const THEME_ICONS: Readonly<Record<Settings['theme'], LucideIcon>> = {
  system: Monitor,
  light: Sun,
  dark: Moon,
}

const EXPORTS: readonly {
  readonly format: ExportRequest['format']
  readonly title: string
}[] = [
  { format: '3mf', title: 'Export 3MF' },
  { format: 'stl', title: 'Export STL' },
  { format: 'obj', title: 'Export OBJ' },
  { format: 'step', title: 'Export STEP' },
]

/** The built-in commands for a platform (the platform decides desktop-only shortcuts). */
export function builtinCommands(platform: Platform): CommandDefinition[] {
  return [
    command({
      id: 'app.palette',
      title: 'Command palette',
      category: 'View',
      icon: Search,
      shortcuts: ['Mod+K', 'S'],
      contexts: ALL,
      run: (app) => {
        togglePanel(app, 'paletteOpen')
      },
    }),

    command({
      id: 'app.keys',
      title: 'Keyboard and mouse',
      category: 'View',
      icon: Keyboard,
      shortcuts: ['F1'],
      contexts: ALL,
      checked: (app) => stateOf(app).panels.side === 'keys',
      run: (app) => {
        openSidePanel(app, 'keys')
      },
    }),

    // File.
    command({
      id: 'file.new',
      title: 'New',
      category: 'File',
      icon: FilePlus,
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
      availability: needsDocumentOutsideCommand,
      run: (app) => save(app),
    }),
    command({
      id: 'file.save_as',
      title: 'Save as',
      category: 'File',
      icon: SaveAll,
      shortcuts: ['Mod+Shift+S'],
      contexts: ALL,
      availability: needsDocumentOutsideCommand,
      run: (app) => saveAs(app),
    }),
    command({
      id: 'file.export',
      title: 'Export',
      category: 'File',
      icon: Download,
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
        const active = stateOf(app).command
        if (active !== null) {
          const step = active.history.at(-1)
          return step === undefined ? `Cancel ${active.title}` : `Undo change to ${step.field}`
        }
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
        const active = stateOf(app).command
        if (active !== null) {
          const step = active.future.at(-1)
          return step === undefined ? null : `Redo change to ${step.field}`
        }
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
          setBrowserOpen(app, true)
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
      contexts: MODEL_PRINT,
      availability: suppressAvailability(true),
      run: suppressSelection(true),
    }),
    command({
      id: 'edit.unsuppress',
      title: 'Unsuppress',
      category: 'Edit',
      icon: CircleCheck,
      contexts: MODEL_PRINT,
      availability: suppressAvailability(false),
      run: suppressSelection(false),
    }),
    command({
      id: 'edit.repeat',
      title: 'Repeat',
      label: (app) => {
        const repeated = repeatedCommand(app)
        return repeated === undefined ? 'Repeat' : `Repeat ${repeated.title}`
      },
      category: 'Edit',
      icon: Repeat,
      contexts: MODEL_PRINT,
      availability: repeatAvailability,
      // The feature command starts with the current selection preselected (3.7 step 4).
      run: async (app, source) => {
        const repeated = repeatedCommand(app)
        if (repeated !== undefined) {
          await app.commands.run(app, repeated.id, source)
        }
      },
    }),
    command({
      id: 'edit.clear_selection',
      title: 'Clear selection',
      category: 'Edit',
      icon: MousePointer2,
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
        // The Sketch radio names what it does: "Edit {label}" or "New sketch" (3.3).
        ...(context === 'sketch'
          ? { detail: (app: App) => selectSketchRadioTooltip(stateOf(app)) }
          : {}),
        run: (app) => setContext(app, context),
      }),
    ),

    // View.
    command({
      id: 'view.browser',
      title: 'Toggle browser',
      category: 'View',
      icon: PanelLeft,
      shortcuts: ['Mod+B'],
      contexts: ALL,
      checked: (app) => stateOf(app).panels.browserOpen,
      run: (app) => {
        togglePanel(app, 'browserOpen')
      },
    }),
    command({
      id: 'view.timeline',
      title: 'Toggle timeline',
      category: 'View',
      icon: PanelBottom,
      contexts: ALL,
      checked: (app) => !stateOf(app).panels.timelineCollapsed,
      run: (app) => {
        togglePanel(app, 'timelineCollapsed')
      },
    }),
    command({
      id: 'view.parameters',
      title: 'Parameters',
      category: 'View',
      icon: SlidersHorizontal,
      contexts: ALL,
      run: (app) => {
        openSidePanel(app, 'parameters')
      },
    }),
    command({
      id: 'view.problems',
      title: 'Problems',
      category: 'View',
      icon: TriangleAlert,
      contexts: ALL,
      run: (app) => {
        openSidePanel(app, 'problems')
      },
    }),
    command({
      id: 'view.health',
      title: 'Print check',
      category: 'View',
      icon: HeartPulse,
      contexts: ALL,
      // As the badge does (3.17): the panel opens, and a missing or stale report is checked again.
      run: (app) => openPrintHealth(app),
    }),
    command({
      id: 'view.fit',
      title: 'Fit to view',
      category: 'View',
      icon: Maximize,
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
      shortcuts: ['Shift+L'],
      contexts: ALL,
      availability: (app) =>
        lookTarget(stateOf(app)) === null
          ? disabledWith('look_at_disabled', MESSAGES.look_at_disabled())
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
      id: 'view.grid',
      title: 'Show grid',
      category: 'View',
      icon: Grid2x2,
      contexts: MODEL_SKETCH,
      checked: (app) => stateOf(app).view.grid,
      // Session state, like the other view toggles (3.12).
      run: (app) => {
        viewChange(app, { grid: !stateOf(app).view.grid })
      },
    }),
    command({
      id: 'view.visibility',
      title: 'Show or hide selection',
      category: 'View',
      icon: Eye,
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
        description: entry.description,
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
      description:
        'Turns lines and curves into construction geometry, which guides the shape but is not part of it.',
      shortcuts: ['X'],
      contexts: SKETCH,
      availability: whileEditingSketch,
      checked: (app) => stateOf(app).sketch?.construction === true,
      run: (app) => toggleConstruction(app),
    }),
    ...CONSTRAINT_TYPES.map((type) =>
      command({
        id: `sketch.constrain_${type}`,
        title: CONSTRAINT_TITLES[type],
        category: 'Constrain',
        icon: CONSTRAINT_ICONS[type],
        description: CONSTRAINT_TOOLS[type].description,
        shortcuts: CONSTRAINT_TOOLS[type].shortcuts,
        contexts: SKETCH,
        availability: whileEditingSketch,
        checked: (app) => stateOf(app).sketch?.tool === `constrain_${type}`,
        run: (app) => {
          // The sketch controller applies the constraint at once when the selection fits (3.16.6).
          setSketchTool(app, `constrain_${type}`)
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
      description: 'Leaves the sketch and returns to the 3D model.',
      shortcuts: ['Mod+Enter'],
      contexts: SKETCH,
      availability: whileEditingSketch,
      // Finish sketch selects what the sketch made, so the next step shows (3.16.11).
      run: (app) => finishSketch(app, { selectResult: true }),
    }),

    // Print.
    command({
      id: 'print.printer',
      title: 'Choose printer',
      category: 'Print',
      icon: Printer,
      contexts: PRINT,
      // Changing the printer is an edit, which waits for the active command (3.7).
      availability: needsDocumentOutsideCommand,
      run: (app) => {
        requestUi(app, { kind: 'printer' })
      },
    }),
    command({
      id: 'print.overhangs',
      title: 'Overhang map',
      category: 'Print',
      icon: Ruler,
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
      id: 'print.lay_on_face',
      title: 'Lay on face',
      category: 'Print',
      icon: ArrowDownToLine,
      description: 'Pick the face that should rest on the bed. Click again to print as modeled.',
      contexts: PRINT,
      availability: needsDocument,
      checked: (app) => stateOf(app).print.orientation !== null,
      // A toggle (3.17): with an orientation set it prints as modeled again; a second click
      // while picking ends the pick.
      run: (app) => {
        const print = stateOf(app).print
        if (print.orientation !== null) {
          setPrintOrientation(app, null)
        } else if (print.pickingFace) {
          cancelLayOnFace(app)
        } else {
          startLayOnFace(app)
        }
      },
    }),
    command({
      id: 'print.check_now',
      title: 'Check print',
      category: 'Print',
      icon: Wrench,
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
        contexts: ALL,
        availability: needsDocumentOutsideCommand,
        run: (app) => exportModel(app, { format: entry.format, scope: 'visible' }),
      }),
    ),

    // Document units.
    ...DOCUMENT_UNITS.map((unit) =>
      command({
        id: `document.units.${unit}`,
        title: `Units: ${unit}`,
        category: 'Edit',
        icon: Type,
        contexts: ALL,
        availability: needsDocumentOutsideCommand,
        checked: (app) => stateOf(app).document?.recipe.units === unit,
        run: (app) => setUnits(app, unit),
      }),
    ),

    // Settings.
    ...THEMES.map((theme) =>
      command({
        id: `settings.theme.${theme}`,
        title: `Theme: ${SETTING_LABELS[theme]}`,
        category: 'Settings',
        icon: THEME_ICONS[theme],
        contexts: ALL,
        checked: (app) => stateOf(app).settings.theme === theme,
        run: (app) => {
          updateSettings(app, { theme })
        },
      }),
    ),
    command({
      id: 'settings.theme.toggle',
      title: 'Toggle dark theme',
      category: 'Settings',
      icon: Moon,
      contexts: ALL,
      checked: (app) => resolvedTheme(stateOf(app).settings) === 'dark',
      run: (app) => {
        updateSettings(app, { theme: toggledTheme(resolvedTheme(stateOf(app).settings)) })
      },
    }),
    ...NAVIGATIONS.map((navigation) =>
      command({
        id: `settings.navigation.${navigation}`,
        title: `Navigation: ${SETTING_LABELS[navigation]}`,
        category: 'Settings',
        icon: MousePointer2,
        contexts: ALL,
        checked: (app) => stateOf(app).settings.navigation === navigation,
        run: (app) => {
          updateSettings(app, { navigation })
        },
      }),
    ),
    ...UI_SCALES.map((scale) =>
      command({
        id: `settings.scale.${scalePercent(scale)}`,
        title: `UI scale: ${scalePercent(scale)}%`,
        category: 'Settings',
        icon: ZoomIn,
        contexts: ALL,
        checked: (app) => stateOf(app).settings.uiScale === scale,
        run: (app) => {
          updateSettings(app, { uiScale: scale })
        },
      }),
    ),
  ]
}
