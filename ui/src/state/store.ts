import { createStore, type StoreApi } from 'zustand/vanilla'
import type { SolvedSketch } from '../core/api'
import type { OpenedFile } from '../files/file-service'
import type {
  BoundingBox,
  Diagnostic,
  DocHandle,
  Document,
  ElementDescriptor,
  FeatureManifest,
  FeatureStatus,
  ParameterValue,
  Placement,
  PointRef,
  PrinterProfile,
  PrintReport,
  RegionInfo,
} from '../generated/protocol'
import type { JsonSchema } from '../inspector/model'
import { loadSettings, type Settings } from '../settings/settings'
import type { SketchToolId } from '../sketch/session'
import type { StandardView } from '../viewport/camera'
import type { SelectionItemKind } from '../viewport/picking'

// The app state (ui.md 2.8): one zustand vanilla store per app, so tests create isolated stores
// without React. Actions are the only code that changes it in response to the user.

export type AppContext = 'model' | 'sketch' | 'print'

export type AppState = {
  readonly core: CoreState
  readonly catalog: Catalog | null
  readonly printers: readonly PrinterProfile[]
  readonly document: DocumentState | null
  readonly context: AppContext
  readonly selection: readonly SelectionItem[]
  readonly hover: HoverTarget | null
  readonly command: ActiveCommand | null
  readonly sketch: SketchSession | null
  readonly print: PrintState
  readonly view: ViewState
  readonly panels: PanelsState
  readonly settings: Settings
  readonly notifications: readonly Notification[]
}

export type CoreState = {
  readonly status: 'loading' | 'ready' | 'failed'
  readonly engineVersion: string | null
  readonly fatal: string | null
  /** Mutating requests sent and not yet answered. */
  readonly pendingMutations: number
}

export type Catalog = {
  /** Entries in protocol order (sorted by type). */
  readonly features: readonly CatalogFeature[]
  readonly byType: Readonly<Record<string, CatalogFeature>>
}

export type CatalogFeature = {
  readonly type: string
  readonly manifest: FeatureManifest
  readonly paramsSchema: JsonSchema
}

export type DocumentState = {
  readonly documentHandle: DocHandle
  readonly name: string
  readonly file: FileReference | null
  readonly revision: number
  readonly recipe: Document
  /** The core's dirty flag from document_changed. */
  readonly dirty: boolean
  /** Set when the recipe was opened from a recovery copy or reopened after a core restart, so the core's baseline is not the file on disk; cleared by a successful save (3.19). */
  readonly localUnsaved: boolean
  readonly undoLabel: string | null
  readonly redoLabel: string | null
  readonly transactionOpen: boolean
  readonly focus: string | null
  readonly featureStatus: Readonly<Record<string, FeatureStatus>>
  readonly parameterValues: Readonly<Record<string, ParameterValue>>
  readonly diagnostics: readonly Diagnostic[]
  readonly sketches: Readonly<Record<string, SolvedSketch>>
  readonly datums: Readonly<Record<string, Placement | null>>
  /** Model-layer mesh metadata by body id; buffers live in the scene, not the store. */
  readonly meshes: Readonly<Record<string, MeshSummary>>
  readonly lastRebuildMilliseconds: number | null
}

export type MeshSummary = {
  readonly meshId: number
  readonly boundingBox: BoundingBox
  readonly faceCount: number
  readonly edgeCount: number
  readonly vertexCount: number
}

/** The file a document was opened from or saved to: a web file handle or a desktop grant. */
export type FileReference = { readonly name: string; readonly handle: OpenedFile['handle'] }

/** The kinds of selection items, declared once with the picking code that produces them. */
export type { SelectionItemKind }

export type SelectionItem = {
  /** Stable key (3.6), equal for the same thing across rebuilds. */
  readonly key: string
  readonly kind: SelectionItemKind
  /** Display label, for example "Face of Cube" or "Line e-1a2b". */
  readonly label: string
  readonly body: string | null
  /** Feature that owns the item: the sketch for sketch items, the datum, the feature itself, or, for faces, edges and vertices, `element.feature`. */
  readonly feature: string | null
  /** Element data for faces, edges and vertices (from pick_reference). */
  readonly element: ElementDescriptor | null
  /** Sketch data: the entity id, point ref, constraint id or region info. */
  readonly sketchItem: SketchItemReference | null
  /** Origin item name: XY, XZ, YZ, X, Y, Z or O. */
  readonly origin: OriginName | null
}

export type OriginName = 'XY' | 'XZ' | 'YZ' | 'X' | 'Y' | 'Z' | 'O'

export type SketchItemReference =
  | {
      readonly kind: 'entity'
      readonly sketch: string
      readonly entity: string
      readonly entityType: 'point' | 'line' | 'circle' | 'arc'
    }
  | { readonly kind: 'point'; readonly sketch: string; readonly point: PointRef }
  | { readonly kind: 'constraint'; readonly sketch: string; readonly constraint: string }
  | { readonly kind: 'region'; readonly sketch: string; readonly region: RegionInfo }
  | {
      readonly kind: 'origin'
      readonly sketch: string
      readonly which: 'point' | 'x_axis' | 'y_axis'
    }

export type HoverTarget = { readonly key: string; readonly kind: SelectionItemKind }

export type ActiveCommand = {
  readonly featureType: string
  /** Feature id: the new record's id, or the edited record's id. */
  readonly featureId: string
  readonly editing: boolean
  readonly title: string
  readonly params: Readonly<Record<string, unknown>>
  /** Path of the selection field that receives picks, or null. */
  readonly activeField: string | null
  readonly fieldErrors: Readonly<Record<string, string>>
  /** Whether the feature record has been applied inside the transaction yet. */
  readonly applied: boolean
  /** Body this command creates for op new, if any. */
  readonly newBody: { readonly id: string; readonly name: string } | null
  readonly preview: 'none' | 'pending' | 'shown' | 'failed'
}

export type SketchSession = {
  /** Null while picking the plane for a new sketch. */
  readonly sketchId: string | null
  readonly tool: SketchToolId | null
  readonly construction: boolean
  readonly showConstraints: boolean
  /** In-sketch selection (entities, points, constraints), separate from the app selection. */
  readonly selection: readonly SelectionItem[]
  readonly dragging: boolean
}

export type PrintState = {
  readonly report: PrintReport | null
  /** Revision the report describes; the badge is stale when it differs from the document revision. */
  readonly reportRevision: number | null
  readonly running: boolean
  readonly overhangMap: boolean
  readonly focusedFinding: FindingKey | null
}

/** A finding of the print report: the `BodyPrint.body` id, the check, and the finding's index in that check's `findings`. */
export type FindingKey = {
  readonly body: string
  readonly check: string
  readonly index: number
}

export type ViewState = {
  readonly projection: 'perspective' | 'orthographic'
  readonly named: StandardView | 'custom'
  readonly section: SectionState | null
  readonly filter: {
    readonly body: boolean
    readonly face: boolean
    readonly edge: boolean
    readonly vertex: boolean
  }
  /** Session-only visibility of sketches, datum planes and origin items, by selection key. */
  readonly sessionVisible: Readonly<Record<string, boolean>>
  /** Session-only locked bodies (not pickable). */
  readonly locked: readonly string[]
}

export type SectionState = {
  readonly plane: 'XY' | 'XZ' | 'YZ'
  readonly offset: string
  readonly flipped: boolean
}

export type PanelsState = {
  readonly browserOpen: boolean
  readonly timelineCollapsed: boolean
  readonly paletteOpen: boolean
  readonly side: 'parameters' | 'health' | 'problems' | null
  readonly inspectorPosition: { readonly x: number; readonly y: number } | null
}

export type Notification = {
  readonly id: string
  readonly severity: 'info' | 'success' | 'warning' | 'error'
  readonly message: string
  readonly actions: readonly { readonly label: string; readonly run: () => void }[]
  readonly createdAt: number
}

export type AppStore = StoreApi<AppState>

/** Below this window width (CSS pixels) at startup the browser starts closed (3.2). */
const NARROW_WINDOW_WIDTH = 1024

/** The page's localStorage, or null where reading it is not allowed. */
export function browserStorage(): Storage | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage
  } catch {
    // Opaque origins and blocked site data throw on access.
    return null
  }
}

function startsWithBrowserOpen(): boolean {
  return typeof window === 'undefined' || window.innerWidth >= NARROW_WINDOW_WIDTH
}

export const INITIAL_STATE: AppState = {
  core: { status: 'loading', engineVersion: null, fatal: null, pendingMutations: 0 },
  catalog: null,
  printers: [],
  document: null,
  context: 'model',
  selection: [],
  hover: null,
  command: null,
  sketch: null,
  print: {
    report: null,
    reportRevision: null,
    running: false,
    overhangMap: true,
    focusedFinding: null,
  },
  view: {
    projection: 'perspective',
    named: 'iso',
    section: null,
    filter: { body: false, face: true, edge: true, vertex: true },
    sessionVisible: {},
    locked: [],
  },
  panels: {
    browserOpen: startsWithBrowserOpen(),
    timelineCollapsed: false,
    paletteOpen: false,
    side: null,
    inspectorPosition: null,
  },
  settings: loadSettings(browserStorage()),
  notifications: [],
}

export function createAppStore(overrides: Partial<AppState> = {}): AppStore {
  return createStore<AppState>()(() => ({ ...INITIAL_STATE, ...overrides }))
}
