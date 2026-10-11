# UI specification

Status: M1 spec. Package: `ui/` (React 19, TypeScript strict, Vite, Tailwind CSS 4, zustand, lucide-react). Read with [architecture.md](../design/architecture.md) (section "UI"), [technical design](../design/technical-design.md) sections 7, 8, 11, 13 and 14, ADRs [0002](../adr/0002-react-for-the-ui.md), [0006](../adr/0006-protocol-framing-and-schema.md), [0007](../adr/0007-webgl2-renderer-first.md) and [0009](../adr/0009-emscripten-c-abi-bridge.md), and the specs this one consumes: [protocol.md](protocol.md) (wire, commands, events, worker host, frame codec), [features.md](features.md) (manifests, shared param types, labels, messages), [model.md](model.md) (edits, ids, labels) and sketch.md and print.md (sketch and printer JSON).

This spec is the definition of done for the UI in M1. Test authors write the tests in section 6 from it without seeing the implementation; implementers make them pass without weakening them. Where this spec quotes another spec's JSON or names, the other spec is authoritative and a mismatch is a bug in this spec.

## 1. Purpose and scope

The UI is the single front end for web and desktop. It renders the model, edits it with the mouse and keyboard, and talks to the core only through protocol frames (architecture.md: "The UI depends only on the protocol"). It never evaluates geometry, never parses `.ptsm` text and never constructs a face, edge or vertex reference itself: references are copied from protocol data (`pick_reference` results, sketch solutions), and the typed wrappers around them (`PlaneRef`, `AxisRef`, `ProfileRef`) are assembled exactly as features.md section 2.5 defines.

### 1.1 In M1 (the "M1 slice")

- App shell: top bar, collapsible browser, viewport, timeline, floating inspector, right side panel, status bar, notifications, light and dark themes from one token set, UI scale 100 to 300 percent.
- Three contexts (Model, Sketch, Print), each with one short toolbar, and one command registry shared by toolbars, the command palette and keyboard shortcuts.
- Command palette (Ctrl/Cmd+K) with fuzzy search over commands, features, bodies, parameters, recent files and settings, showing each command's shortcut.
- Timeline: rollback marker drag, drag to reorder, suppress, status icons, double-click to edit, rename, delete, basic Problems list.
- Browser: origin, bodies, sketches, construction (datum planes) and parameters, with eye, lock and color controls.
- Schema-driven inspector for every catalog feature: essential fields and a "More" toggle from manifest hints, expression inputs with live validation and unit display, selection fields that capture viewport and browser picks, live preview through protocol transactions, Enter confirms, Esc cancels.
- Parameters panel.
- WebGL2 viewport: matcap-style shaded faces, screen-space anti-aliased edges, ID-buffer picking of faces, edges and vertices with hover highlight, selection filter, section plane, analysis color overlay, view cube, orbit, pan and zoom with Fusion and FreeCAD navigation presets, fit to view, perspective and orthographic projection.
- Sketch mode for points, lines, circles and arcs: tools line/polyline, rectangle (2-point and center), circle (center, radius), arc (3-point), point, construction toggle; constraint tools coincident, point on object, horizontal, vertical, parallel, perpendicular, tangent, collinear, concentric, equal, symmetric, midpoint, fix; dimension tool for distance, horizontal and vertical distance, radius, diameter and angle with expression entry; auto-constraint inference while drawing (coincident, horizontal, vertical, tangent, midpoint) with a hold key to suppress it; dimension as you draw; DOF colors and DOF count; constraint glyphs; point drag solved by the core at up to 60 fps; profile check display.
- Print context: printer profile picker, check toggles, overhang color map with legend, print health badge and list with zoom to problem, bed outline.
- Export to 3MF, STL (binary), OBJ and STEP.
- Files: new, open, save, save as with the File System Access API and a download fallback, drag-and-drop open, recent files and a recovery copy in IndexedDB.
- Feature types covered through the generic inspector: `sketch`, `extrude`, `revolve`, `hole`, `box`, `cylinder`, `sphere`, `cone`, `torus`, `fillet`, `chamfer`, `shell`, `boolean`, `move_body`, `split_body`, `mirror`, `linear_pattern`, `circular_pattern`, `datum_plane` (features.md section 3.13).
- Accessibility: WCAG 2.2 AA text and non-text contrast, full keyboard operation of every panel, landmarks, live regions.
- Worker and Tauri transports, the core client, and the generated protocol types.

### 1.2 Later (not in M1)

- Onboarding tutorial, looping tool-tip animations and the "Coming from Fusion / FreeCAD" panel.
- Right-click radial marking menu in the viewport.
- 3Dconnexion SpaceMouse.
- Code panel (CodeMirror 6), script features, Python export UI and the Customizer panel.
- Repair panel beyond the basic Problems list (ghosted lost element, one-click re-pick on the canvas).
- WebGPU render backend.
- Multi-document tabs.
- Navigation presets SolidWorks, Blender and Onshape; trackpad gestures beyond pinch zoom; touch-optimized layout.
- FreeCAD shortcut preset, user shortcut rebinding and user-pinned toolbar tools.
- Remaining sketch tools (3-point and rounded rectangle, 2-point, 3-point and tangent circle, center and tangent arc, tangent-arc toggle in the line tool, conics, splines, polygon, slots, text, image, DXF and SVG import), modify tools (trim, extend, split, sketch fillet and chamfer, offset, mirror, patterns, move and copy) and reference geometry (project, intersect, carbon copy).
- Remaining constraints (smooth, block, arc length, angle to an axis as its own type, Snell's law, B-spline constraints).
- Dragging sketch curves (translating a line, resizing a circle); M1 drags points only.
- Window and crossing box selection in the Model context (M1 has it in sketch mode only).
- Keyboard picking of faces, edges and vertices.
- Features outside the slice (sweep, loft, helix, rib, emboss, thread, draft, scale, direct edits, derive, surfaces, mesh bodies, configurations), components, joints and assemblies.
- Measure tool and mass properties.
- Import UI (STEP, meshes, `.scad`, `.FCStd`, DXF, SVG) and `.ptsz` bundles.
- Print-aware feature presets, orientation tools (auto-orient, lay face on bed), slicer handoff and watch mode, resin tools, heat maps for checks other than overhangs, multi-plate arrangement.
- Display quality setting (`set_tessellation`).
- Cloud sharing, AI assist and plugin panel slots.

### 1.3 Terms

- **Core**: the engine behind the protocol, in a Web Worker (web) or in the Tauri process (desktop).
- **Recipe**: the `Document` (model.md) as carried by `document_changed`.
- **Display state**: what the core currently shows: the model layer and, during a focused transaction, the preview layer (protocol.md `Layer`).
- **Pick target**: anything the pointer can hit: a mesh face, edge or vertex, a sketch region, curve or point, an origin plane, axis or point, a datum plane, a sketch constraint glyph or dimension.
- **Selection item**: a pick target, timeline feature, browser row or parameter held in the app selection or in an inspector selection field.
- **Command**: an entry in the command registry, run from a toolbar, the palette, a shortcut or a menu.
- **Feature command**: a command that creates or edits one feature record through the inspector.
- **Tool**: a sketch-mode command that stays active across several clicks (drawing, constraining, dimensioning).
- **Transaction**: protocol.md's `begin_transaction` ... `commit_transaction` sequence; the UI uses it for every inspector command so the preview is live and the result is one undo entry.
- **Context**: Model, Sketch or Print.
- **DOF**: remaining degrees of freedom of a sketch.

## 2. Public API and module structure

### 2.1 Layout of `ui/`

```text
ui/
  scripts/generate-protocol-types.mjs   protocol types generator (2.2)
  src/
    main.tsx                 capability check, theme bootstrap, createApp, render
    App.tsx                  layout shell (regions of section 3.2)
    app/
      createApp.ts           createApp, App
      AppProvider.tsx        React context; useApp, useAppState
      actions/               document.ts, selection.ts, command.ts, context.ts, sketch.ts,
                             timeline.ts, bodies.ts, parameters.ts, print.ts, export.ts,
                             files.ts, notifications.ts
    core/
      frame.ts               encodeFrame, decodeFrame (protocol.md 2.11)
      bridge.ts              CoreModule, callCore, wrapEmscriptenModule (protocol.md 2.10)
      core.worker.ts         worker entry using createWorkerHost (protocol.md 2.11)
      worker-host.ts         createWorkerHost (protocol.md 2.11)
      transport.ts           Transport, createWorkerTransport, chooseTransport
      tauri-transport.ts     createTauriTransport
      client.ts              createCoreClient
      api.ts                 CoreApi, createCoreApi (the only module that names commands)
      events.ts              CoreEvent, decodeEvent (the only module that names events)
      queue.ts               createMutationQueue, createLatestOnly
      ids.ts                 generateId
      labels.ts              featureLabel, nextBodyName
    generated/
      protocol.ts            generated, checked in, never edited
    state/
      store.ts               AppState, AppStore, createAppStore
      selectors.ts
    commands/
      registry.ts            CommandRegistry, createCommandRegistry
      shortcuts.ts           parseShortcut, formatShortcut, matchChord
      dispatcher.ts          installShortcutDispatcher
      fuzzy.ts               fuzzyScore, rankPaletteItems
      palette.ts             paletteItems
      builtin.ts             built-in UI commands
      features.ts            commands generated from the feature catalog
      toolbars.ts            TOOLBARS
      icons.ts               FEATURE_ICONS, constraint glyph icons
    inspector/
      manifest.ts            FeatureManifest (mirror of features.md 2.3), normalizeManifest
      model.ts               buildInspectorModel, defaultParams, setParam, isComplete
      selection-values.ts    selectionValue, acceptsTarget
      placement.ts           placeInspector
      Inspector.tsx, fields/*.tsx
    expression/
      evaluate.ts            convertForField, dimensionMessage
      format.ts              formatQuantity, formatNumber, formatSize
      step.ts                stepExpression, isPlainNumber
      ExpressionInput.tsx, useExpression.ts
    viewport/
      math.ts                Vec3, Mat4, Quat helpers
      camera.ts              Camera and camera functions
      navigation.ts          NAVIGATION_PRESETS, gestureFor
      backend.ts             RenderBackend and its types
      webgl2/                createWebGl2Backend and shaders
      picking.ts             encodePickId, decodePickId, choosePick
      scene.ts               Scene, createScene
      controller.ts          ViewportController, ViewportHandle
      Viewport.tsx, ViewCube.tsx, ViewportControls.tsx, SectionPopover.tsx
    sketch/
      refs.ts                sketch origin and axis references (sketch.md)
      geometry.ts            circleThroughThreePoints, tessellateCurve, distances
      recipes.ts             tool outputs
      inference.ts           infer
      constraints.ts         applicableConstraints, constraintFor
      dimensions.ts          dimensionTypeFor, measuredValue, defaultLabelPosition
      session.ts             SketchController
      SketchOverlay.tsx, SketchHeader.tsx, DrawInputs.tsx, DimensionEditor.tsx
    timeline/ Timeline.tsx
    browser/ Browser.tsx
    parameters/ ParametersPanel.tsx
    print/ PrinterMenu.tsx, ChecksPopover.tsx, HealthList.tsx, OverhangLegend.tsx
    export/ ExportPopover.tsx
    problems/ ProblemsPanel.tsx
    files/
      file-service.ts        FileService, createWebFileService
      recent.ts              RecentFilesStore, createIndexedDbRecentFiles, createMemoryRecentFiles
      recovery.ts            RecoveryStore, createIndexedDbRecovery, createMemoryRecovery
    settings/ settings.ts    Settings, loadSettings, saveSettings
    theme/
      theme.css              tokens and Tailwind theme variables (2.16)
      theme.ts               resolveTheme, applyTheme, readViewportTheme
    layout/ TopBar.tsx, StatusBar.tsx, SidePanel.tsx, Notifications.tsx, DocumentMenu.tsx
    components/              Button, IconButton, Menu, Popover, Tooltip, Switch, SegmentedControl, TextField
    perf/ marks.ts           PERF_NAMES, perfMark, perfMeasure
    messages.ts              UI-owned message catalog (section 4.3)
    test/ setup.ts, fakes/   FakeTransport, FakeCoreApi, FakeBackend, catalog and documents fixtures
  e2e/
    fixtures/                .ptsm fixtures (6.2)
    helpers.ts               shared Playwright helpers (6.3)
    *.spec.ts
```

Module rules:

- Only `src/core/api.ts` names protocol commands and only `src/core/events.ts` names protocol events. Every other module uses `CoreApi` and `CoreEvent`.
- React components never call `CoreApi`; they call actions (2.9). Actions are plain functions taking the `App`, so they are unit-testable with fakes.
- The viewport and sketch drag bypass React (ADR 0002): `ViewportController` subscribes to the store with `store.subscribe` and draws on demand. React renders only DOM chrome and DOM overlays.
- Runtime dependencies are exactly those in `ui/package.json` (`react`, `react-dom`, `zustand`, `lucide-react`). Menus, popovers, tooltips and the fuzzy matcher are written in-house; popovers use the native Popover API. Adding a dependency needs human review (CLAUDE.md).
- Hand-written code never redeclares a type that exists in `src/generated/protocol.ts`. The only mirror types are for protocol values the schema carries as untyped JSON (feature manifests and params schemas, features.md 2.3), and they are validated at runtime (2.12).

### 2.2 Generated protocol types

`ui/scripts/generate-protocol-types.mjs`, run by `pnpm -C ui generate:protocol`:

1. Reads `../docs/format/protocol.schema.json` relative to `ui/`. A missing or unparsable file exits with status 1 and prints `Cannot read docs/format/protocol.schema.json: <reason>`.
2. Compiles it with `json-schema-to-typescript` `compile(schema, 'Protocol', options)` where `options` are `{ bannerComment: '/* Generated from docs/format/protocol.schema.json by ui/scripts/generate-protocol-types.mjs. Do not edit. */', unreachableDefinitions: true, additionalProperties: false, strictIndexSignatures: true, declareExternallyReferenced: true, enableConstEnums: false, format: true, style: <ui/.prettierrc.json> }`. Every `$defs` entry becomes an exported type with its definition name (`Command`, `CommandResult`, `Event`, `Message`, `MeshBuffers`, `SketchSolution`, `PrintReport`, ...).
3. Appends `export const PROTOCOL_VERSION = <n> as const` where `n` is the schema's top-level `x-protocol-version` (protocol.md 2.9). A schema without it exits with status 1.
4. Writes `ui/src/generated/protocol.ts` with LF line endings and one trailing newline. The output is a pure function of the schema file: running it twice gives byte-identical files.

The generated file is checked in. CI runs `pnpm -C ui generate:protocol && git diff --exit-code ui/src/generated` and fails on drift. `src/generated` is excluded from ESLint and Prettier (already configured).

### 2.3 Frame codec, bridge and worker host

`src/core/frame.ts`, `src/core/bridge.ts`, `src/core/worker-host.ts` and `src/core/core.worker.ts` are specified in protocol.md sections 2.10 and 2.11 (signatures, superseding rules, ready and fatal messages, tests 53 to 66). This spec adds only:

- `core.worker.ts` constructs the host with `load = () => import(/* @vite-ignore */ `${import.meta.env.BASE_URL}core/partsmith_core.mjs`).then((module) => module.default()).then(wrapEmscriptenModule)`, `post = (message, transfer) => self.postMessage(message, transfer)` and `schedule = (task) => setTimeout(task, 0)`, and forwards every `message` event to `host.receive`.
- The worker is created with `new Worker(new URL('./core.worker.ts', import.meta.url), { type: 'module', name: 'partsmith-core' })`.

### 2.4 Transports

`src/core/transport.ts`:

```ts
/** What the core reported when it became ready. */
export interface CoreReady {
  readonly protocolVersion: number
  readonly engineVersion: string
}

/** A byte pipe to the core: one request frame in, its response frame out. */
export interface Transport {
  readonly kind: 'worker' | 'tauri'
  /** Starts the core. Resolves once it accepts calls; rejects with CoreFatalError. */
  start(): Promise<CoreReady>
  /** Sends one request frame and resolves with the response frame for it. */
  send(request: ArrayBuffer): Promise<ArrayBuffer>
  /** Stops the core; pending and later sends reject with CoreFatalError. */
  stop(): void
  /** Called at most once, when the core dies outside the caller's control. */
  onFatal(listener: (error: CoreFatalError) => void): () => void
}

/** The core is gone; the app must restart it. */
export class CoreFatalError extends Error {}

/** Creates the web transport. `createWorker` is injectable for tests. */
export function createWorkerTransport(options?: { createWorker?: () => Worker; startTimeoutMs?: number }): Transport

/** Picks the Tauri transport when the Tauri global is present, else the worker transport. */
export function chooseTransport(environment: { readonly __TAURI__?: TauriGlobal }): Transport
```

Worker transport behavior:

1. `start()` creates the worker and resolves with `{ protocolVersion, engineVersion }` from the worker's `ready` message. A `fatal` message, a worker `error` event, or no `ready` within `startTimeoutMs` (default 30,000) rejects with `CoreFatalError`; the timeout message is "The geometry core did not start within 30 seconds."
2. `send(request)` posts `{ type: 'call', frame: request }` with `[request]` as the transfer list and appends a resolver to a FIFO list. Each `{ type: 'frame', frame }` message resolves the oldest resolver (the worker answers in request order, protocol.md 2.11 rule 5).
3. A `fatal` message, `error` event or `messageerror` event after start rejects every pending resolver with `CoreFatalError(message)`, notifies the fatal listeners once, and makes later sends reject immediately.
4. `stop()` terminates the worker and rejects pending sends with `CoreFatalError('The geometry core was stopped.')`.

`src/core/tauri-transport.ts`:

```ts
/** The part of `window.__TAURI__` the UI uses (requires `app.withGlobalTauri` in the desktop config). */
export interface TauriGlobal {
  readonly core: { invoke(command: string, payload: ArrayBuffer): Promise<ArrayBuffer> }
}
export function createTauriTransport(tauri: TauriGlobal): Transport
```

Tauri transport behavior (protocol.md 2.12):

1. `start()` sends an `engine_info` request frame (request id 0) through `invoke('call', frame)`, checks that `protocol_version` equals the generated `PROTOCOL_VERSION`, and resolves with the versions. A mismatch rejects with `CoreFatalError('This app and its core speak different protocol versions. Reinstall Partsmith.')`.
2. `send(request)` queues the frame and applies the same FIFO, one-at-a-time and superseding rules as the worker host (protocol.md 2.11 rules 3 and 4), reusing the worker host's queue logic; a superseded request resolves with the synthesized `superseded` error frame. Each step calls `invoke('call', frame)` with the frame as the raw body and resolves with the returned `ArrayBuffer`.
3. An `invoke` rejection resolves the pending send with an error response frame for that request (`internal_error`, the rejection message), built with the same encoder; the desktop core lives in the app process, so `onFatal` is never called.

`chooseTransport` returns `createTauriTransport(environment.__TAURI__)` when `environment.__TAURI__?.core.invoke` is a function, else `createWorkerTransport()`.

### 2.5 Core client

`src/core/client.ts`:

```ts
import type { Command, CommandResult, ErrorCode } from '../generated/protocol'

/** A successful response with the frame's buffers (views alias the frame). */
export interface Reply {
  readonly result: CommandResult
  readonly buffers: readonly (Uint8Array | Uint32Array | Float32Array)[]
}

/** A protocol error response. `message` is the core's text, shown verbatim. */
export class CoreError extends Error {
  readonly code: ErrorCode
  readonly editIndex: number | null
}

export interface CoreClient {
  /** Sends one command. Events in the response frame are dispatched before the promise settles. */
  request(command: Command): Promise<Reply>
  onEvent(listener: (event: CoreEvent, buffers: readonly (Uint8Array | Uint32Array | Float32Array)[]) => void): () => void
  onFatal(listener: (error: CoreFatalError) => void): () => void
  close(): void
}

export function createCoreClient(transport: Transport): CoreClient
```

Behavior:

1. Request ids start at 1 and increase by 1 per request; they never exceed `Number.MAX_SAFE_INTEGER`. Id 0 is never sent by the client (protocol.md 3.5).
2. `request` encodes a request frame with `encodeFrame({ protocol_version: PROTOCOL_VERSION, messages: [{ kind: 'request', id, command }] }, [])`, sends it, and decodes the response frame with `decodeFrame`.
3. It decodes every `event` message with `decodeEvent` (2.7) and calls each event listener synchronously in frame order, then settles the request: an `ok` outcome resolves with the result and the frame's buffers; an `error` outcome rejects with `CoreError`. Events with an unknown `type` are ignored (protocol.md 3.6).
4. A response whose id differs from the request id, or a frame that fails to decode, rejects the request with `CoreFatalError('The geometry core sent a response the app cannot read: <detail>')` and is reported to the fatal listeners: the client cannot trust the stream any more.
5. A listener that throws is reported with `console.error`; other listeners and the request are unaffected.

`src/core/queue.ts`:

```ts
/** Runs tasks strictly one after another, in submission order. */
export interface MutationQueue {
  run<T>(task: () => Promise<T>): Promise<T>
  readonly pending: number
}
export function createMutationQueue(): MutationQueue

/** Sends at most one input at a time; while one is in flight, newer inputs replace the waiting one. */
export interface LatestOnly<I> {
  submit(input: I): void
  /** Drops the waiting input and ignores the result of the one in flight. */
  cancel(): void
  readonly busy: boolean
}
export function createLatestOnly<I, O>(
  send: (input: I) => Promise<O>,
  onResult: (output: O, input: I) => void,
  onError: (error: unknown, input: I) => void,
): LatestOnly<I>
```

`createLatestOnly` semantics: when an in-flight send settles and a newer input is waiting, the settled result is discarded (neither callback runs) and the waiting input is sent; otherwise the matching callback runs. A task that fails in `MutationQueue` rejects its own promise and does not block later tasks.

Every document-mutating `CoreApi` call (apply, transaction, undo, redo, open, new, close, end of drag, save) runs through one `MutationQueue` per app, so the UI's order of edits is the core's order on both transports. Previews, drags, sketch probes and evaluations use `LatestOnly` and are not queued behind each other.

### 2.6 Core API

`src/core/api.ts` wraps every command the M1 UI uses. Method names map one to one to protocol.md section 2.5 commands; argument and result shapes are the generated types.

```ts
import type {
  CatalogEntry, DocHandle, Edit, ElementDescriptor, ExportFile, ExportFormat, Diagnostic,
  Health, Pick, PointRef, PrinterProfile, Reference, Resolution, SketchParams, SketchSolution,
  Units, Dimension, Orientation,
} from '../generated/protocol'

export interface CoreApi {
  engineInfo(): Promise<{ protocolVersion: number; engineVersion: string; formatVersion: number; kernel: string }>
  featureCatalog(): Promise<readonly CatalogEntry[]>
  listPrinterProfiles(): Promise<readonly PrinterProfile[]>
  newDocument(units?: Units): Promise<DocHandle>
  openDocument(text: string): Promise<{ doc: DocHandle; migratedFrom: number | null }>
  saveDocument(doc: DocHandle): Promise<string>
  closeDocument(doc: DocHandle): Promise<void>
  applyEdits(doc: DocHandle, label: string | null, edits: readonly Edit[]): Promise<number>
  beginTransaction(doc: DocHandle, label: string): Promise<void>
  setFocus(doc: DocHandle, feature: string | null): Promise<void>
  commitTransaction(doc: DocHandle): Promise<{ revision: number; recorded: boolean }>
  cancelTransaction(doc: DocHandle): Promise<number>
  undo(doc: DocHandle): Promise<{ revision: number; label: string }>
  redo(doc: DocHandle): Promise<{ revision: number; label: string }>
  solveSketch(doc: DocHandle, feature: string, candidate?: SketchParams): Promise<SketchSolution | null>
  dragSketch(doc: DocHandle, feature: string, point: PointRef, to: readonly [number, number]): Promise<SketchSolution | null>
  endSketchDrag(doc: DocHandle, feature: string, commit: boolean): Promise<number>
  evaluate(doc: DocHandle, expression: string): Promise<{ value: number; dimension: Dimension }>
  pickReference(doc: DocHandle, pick: Pick): Promise<ElementDescriptor>
  resolveReferences(doc: DocHandle, references: readonly Reference[]): Promise<readonly Resolution[]>
  analyzePrint(doc: DocHandle, options: { profile?: string; orientation: Orientation }): Promise<Health | null>
  exportBodies(doc: DocHandle, format: ExportFormat, bodies?: readonly string[]): Promise<{ files: readonly ExportedFile[]; warnings: readonly Diagnostic[] }>
}

/** An export file with its bytes resolved from the frame buffer. */
export interface ExportedFile { readonly name: string; readonly mediaType: string; readonly bytes: Uint8Array }

export function createCoreApi(client: CoreClient): CoreApi
```

Rules:

- `solveSketch`, `dragSketch` and `analyzePrint` resolve with `null` when the core answers `superseded`; every other method rejects with that `CoreError`.
- `exportBodies` copies each file's `u8` buffer into a new `Uint8Array` (the frame is not retained).
- The UI uses no other command in M1. `get_document`, `list_elements`, `measure`, `set_tessellation`, `put_printer_profile`, `import_step` and `export_python` are unused.

### 2.7 Core events

`src/core/events.ts` converts protocol events into UI events. Buffer indices are replaced by typed array views (no copies).

```ts
export type CoreEvent =
  | { readonly type: 'document_changed'; readonly doc: DocHandle; readonly revision: number; readonly document: Document; readonly dirty: boolean; readonly undo: string | null; readonly redo: string | null; readonly transactionOpen: boolean; readonly transaction: string | null; readonly focus: string | null }
  | { readonly type: 'recomputed'; readonly doc: DocHandle; readonly revision: number; readonly features: readonly FeatureStatus[]; readonly parameters: readonly ParameterValue[]; readonly diagnostics: readonly Diagnostic[] }
  | { readonly type: 'sketch_solved'; readonly doc: DocHandle; readonly revision: number; readonly solution: SketchSolution; readonly display: SketchDisplayData | null }
  | { readonly type: 'datum_plane'; readonly doc: DocHandle; readonly revision: number; readonly feature: string; readonly placement: Placement | null }
  | { readonly type: 'body_mesh'; readonly doc: DocHandle; readonly revision: number; readonly body: string; readonly layer: 'model' | 'preview'; readonly meshId: number; readonly mesh: BodyMeshData | null }
  | { readonly type: 'print_health'; readonly doc: DocHandle; readonly revision: number; readonly report: PrintReport; readonly overhang: Readonly<Record<string, Float32Array>> }

/** Mesh buffers of one body in one layer (protocol.md MeshBuffers, 3.4 invariants). */
export interface BodyMeshData {
  readonly positions: Float32Array
  readonly normals: Float32Array
  readonly indices: Uint32Array
  readonly faceIds: Uint32Array
  readonly edgePositions: Float32Array
  readonly edgeIds: Uint32Array
  readonly vertexPositions: Float32Array
  readonly faceCount: number
  readonly edgeCount: number
  readonly vertexCount: number
  readonly boundingBox: BoundingBox
}

/** Region fill buffers of a sketch solution (protocol.md SketchDisplay). */
export interface SketchDisplayData {
  readonly regionPositions: Float32Array
  readonly regionIndices: Uint32Array
  readonly regionIds: Uint32Array
}

export function decodeEvent(event: Event, buffers: readonly (Uint8Array | Uint32Array | Float32Array)[]): CoreEvent | null
```

`print_health.overhang` maps body id to the `f32` overhang buffer of that body (`BodyPrint.overhang`). `decodeEvent` returns `null` for an unknown event type and throws `CoreFatalError` for a buffer index that is out of range or of the wrong element type.

Revision rule: the store keeps the highest `revision` seen per document. A `document_changed`, `recomputed`, `sketch_solved`, `datum_plane`, `body_mesh` or `print_health` event whose revision is lower than the stored revision for that document is ignored. Equal revisions are applied (one request emits several events with the same revision).

### 2.8 Store

`src/state/store.ts` uses `zustand/vanilla` so tests create isolated stores without React.

```ts
export type AppContext = 'model' | 'sketch' | 'print'

export interface AppState {
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

export interface CoreState {
  readonly status: 'loading' | 'ready' | 'failed'
  readonly engineVersion: string | null
  readonly fatal: string | null
  /** Mutating requests sent and not yet answered. */
  readonly pendingMutations: number
}

export interface Catalog {
  /** Entries in protocol order (sorted by type). */
  readonly features: readonly CatalogFeature[]
  readonly byType: Readonly<Record<string, CatalogFeature>>
}
export interface CatalogFeature {
  readonly type: string
  readonly manifest: FeatureManifest
  readonly paramsSchema: JsonSchema
}

export interface DocumentState {
  readonly doc: DocHandle
  readonly name: string
  readonly file: FileRef | null
  readonly revision: number
  readonly recipe: Document
  readonly dirty: boolean
  readonly undoLabel: string | null
  readonly redoLabel: string | null
  readonly transactionOpen: boolean
  readonly focus: string | null
  readonly featureStatus: Readonly<Record<string, FeatureStatus>>
  readonly parameterValues: Readonly<Record<string, ParameterValue>>
  readonly diagnostics: readonly Diagnostic[]
  readonly sketches: Readonly<Record<string, SketchSolution>>
  readonly datums: Readonly<Record<string, Placement | null>>
  /** Model-layer mesh metadata by body id; buffers live in the scene, not the store. */
  readonly meshes: Readonly<Record<string, MeshInfo>>
  readonly lastRebuildMs: number | null
}
export interface MeshInfo { readonly meshId: number; readonly boundingBox: BoundingBox; readonly faceCount: number; readonly edgeCount: number; readonly vertexCount: number }
export interface FileRef { readonly name: string; readonly handle: FileSystemFileHandle | null }

export type SelectionKind =
  | 'face' | 'edge' | 'vertex' | 'body' | 'feature' | 'parameter'
  | 'sketch_region' | 'sketch_curve' | 'sketch_point' | 'sketch_constraint'
  | 'origin_plane' | 'origin_axis' | 'origin_point' | 'datum_plane'

export interface SelectionItem {
  /** Stable key (3.6), equal for the same thing across rebuilds. */
  readonly key: string
  readonly kind: SelectionKind
  /** Display label, for example "Face of Cube" or "Line e-1a2b". */
  readonly label: string
  readonly body: string | null
  /** Feature that owns the item: the sketch for sketch items, the datum, or the feature itself. */
  readonly feature: string | null
  /** Element data for faces, edges and vertices (from pick_reference). */
  readonly element: ElementDescriptor | null
  /** Sketch data: the entity id, point ref, constraint id or region info. */
  readonly sketchItem: SketchItemRef | null
  /** Origin item name: XY, XZ, YZ, X, Y, Z or O. */
  readonly origin: OriginName | null
}

export interface HoverTarget { readonly key: string; readonly kind: SelectionKind }

export interface ActiveCommand {
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

export interface SketchSession {
  /** Null while picking the plane for a new sketch. */
  readonly sketchId: string | null
  readonly tool: SketchToolId | null
  readonly construction: boolean
  readonly showConstraints: boolean
  /** In-sketch selection (entities, points, constraints), separate from the app selection. */
  readonly selection: readonly SelectionItem[]
  readonly dragging: boolean
}

export interface PrintState {
  readonly report: PrintReport | null
  /** Revision the report describes; the badge is stale when it differs from the document revision. */
  readonly reportRevision: number | null
  readonly running: boolean
  readonly hiddenChecks: readonly string[]
  readonly overhangMap: boolean
  readonly focusedFinding: number | null
}

export interface ViewState {
  readonly projection: 'perspective' | 'orthographic'
  readonly named: StandardView | 'custom'
  readonly section: SectionState | null
  readonly filter: { readonly body: boolean; readonly face: boolean; readonly edge: boolean; readonly vertex: boolean }
  /** Session-only visibility of sketches, datum planes and origin items, by selection key. */
  readonly sessionVisible: Readonly<Record<string, boolean>>
  /** Session-only locked bodies (not pickable). */
  readonly locked: readonly string[]
}
export interface SectionState { readonly plane: 'XY' | 'XZ' | 'YZ'; readonly offset: string; readonly flipped: boolean }

export interface PanelsState {
  readonly browserOpen: boolean
  readonly timelineCollapsed: boolean
  readonly paletteOpen: boolean
  readonly side: 'parameters' | 'health' | 'problems' | null
  readonly inspectorPosition: { readonly x: number; readonly y: number } | null
  readonly moreExpanded: Readonly<Record<string, boolean>>
}

export interface Notification {
  readonly id: string
  readonly severity: 'info' | 'success' | 'warning' | 'error'
  readonly message: string
  readonly actions: readonly { readonly label: string; readonly run: () => void }[]
  readonly createdAt: number
}

export type AppStore = StoreApi<AppState>
export const INITIAL_STATE: AppState
export function createAppStore(overrides?: Partial<AppState>): AppStore
```

`INITIAL_STATE`: core loading; no catalog, printers or document; context `model`; empty selection; no hover, command or sketch; print report null, overhang map on, no hidden checks; view perspective, named `iso`, no section, filter `{ body: false, face: true, edge: true, vertex: true }`; browser open (closed when `window.innerWidth < 1024` at startup), timeline expanded, palette closed, no side panel; settings from `loadSettings()` (2.15); no notifications.

`src/state/selectors.ts` exports at least `selectFeatureLabel(state, id)`, `selectActiveFeatureIds(state)`, `selectSketchDof(state)`, `selectHealthBadge(state)` and `selectProblems(state)`; components read state only through selectors or `useAppState(selector)` so each panel re-renders only for the slice it shows.

### 2.9 App and actions

`src/app/createApp.ts`:

```ts
export interface Platform {
  readonly os: 'mac' | 'windows' | 'linux' | 'other'
  readonly host: 'web' | 'desktop'
}

export interface App {
  readonly store: AppStore
  readonly core: CoreApi
  readonly mutations: MutationQueue
  readonly commands: CommandRegistry
  readonly files: FileService
  readonly recent: RecentFilesStore
  readonly recovery: RecoveryStore
  readonly platform: Platform
  /** The mounted viewport, or null before the viewport mounts. */
  viewport(): ViewportHandle | null
  attachViewport(handle: ViewportHandle | null): void
  /** Restarts the core after a fatal error and reopens the last recipe (4.4). */
  restartCore(): Promise<void>
}

export interface CreateAppOptions {
  readonly transport: Transport
  readonly files: FileService
  readonly recent: RecentFilesStore
  readonly recovery: RecoveryStore
  readonly platform: Platform
  readonly store?: AppStore
  readonly now?: () => number
}

export function createApp(options: CreateAppOptions): App
export function detectPlatform(navigator: Navigator, environment: { readonly __TAURI__?: unknown }): Platform
```

`detectPlatform` reads `navigator.userAgentData?.platform` when present, else `navigator.platform`, and maps strings containing `Mac` to `mac`, `Win` to `windows` and `Linux` to `linux`. `host` is `desktop` when the Tauri global exists.

Actions (`src/app/actions/*.ts`) are the only code that changes the store in response to the user. Exact signatures:

```ts
// document.ts
export function startCore(app: App): Promise<void>
export function newDocument(app: App): Promise<void>
export function openFile(app: App, file: OpenedFile): Promise<void>
export function openWithPicker(app: App): Promise<void>
export function openRecent(app: App, entry: RecentFile): Promise<void>
export function save(app: App): Promise<void>
export function saveAs(app: App): Promise<void>
export function applyEdits(app: App, label: string, edits: readonly Edit[]): Promise<boolean>
export function undo(app: App): Promise<void>
export function redo(app: App): Promise<void>
export function setUnits(app: App, units: Units): Promise<void>

// selection.ts
export function select(app: App, items: readonly SelectionItem[], mode: 'replace' | 'toggle'): void
export function clearSelection(app: App): void
export function setHover(app: App, target: HoverTarget | null): void
export function pickAt(app: App, sample: PickResult, modifiers: { toggle: boolean }): Promise<void>

// command.ts
export function startFeatureCommand(app: App, featureType: string): Promise<void>
export function editFeature(app: App, featureId: string): Promise<void>
export function setCommandField(app: App, path: string, value: unknown): void
export function setActiveField(app: App, path: string | null): void
export function capturePick(app: App, item: SelectionItem): Promise<void>
export function confirmCommand(app: App): Promise<void>
export function cancelCommand(app: App): Promise<void>

// context.ts
export function setContext(app: App, context: AppContext): Promise<void>

// sketch.ts
export function startNewSketch(app: App): Promise<void>
export function createSketchOn(app: App, plane: SelectionItem): Promise<void>
export function enterSketch(app: App, sketchId: string): Promise<void>
export function finishSketch(app: App): Promise<void>
export function setSketchTool(app: App, tool: SketchToolId | null): void
export function commitSketchChange(app: App, label: string, change: SketchChange): Promise<SketchCommitResult>
export function deleteSketchItems(app: App, items: readonly SelectionItem[]): Promise<void>

// timeline.ts
export function setRollback(app: App, position: number): Promise<void>
export function moveFeature(app: App, featureId: string, toIndex: number): Promise<void>
export function setSuppressed(app: App, featureIds: readonly string[], suppressed: boolean): Promise<void>
export function deleteFeatures(app: App, featureIds: readonly string[]): Promise<void>
export function renameFeature(app: App, featureId: string, name: string): Promise<void>

// bodies.ts
export function setBodyVisible(app: App, bodyId: string, visible: boolean): Promise<void>
export function setBodyColor(app: App, bodyId: string, color: string | null): Promise<void>
export function setBodyMaterial(app: App, bodyId: string, material: string | null): Promise<void>
export function renameBody(app: App, bodyId: string, name: string): Promise<void>
export function setLocked(app: App, bodyId: string, locked: boolean): void
export function setSessionVisible(app: App, key: string, visible: boolean): void

// parameters.ts
export function addParameter(app: App, name: string, expr: string): Promise<boolean>
export function updateParameter(app: App, id: string, change: { name?: string; expr?: string; comment?: string | null }): Promise<boolean>
export function deleteParameter(app: App, id: string): Promise<boolean>

// print.ts
export function setPrinter(app: App, profileId: string | null): Promise<void>
export function setCheckHidden(app: App, check: string, hidden: boolean): void
export function runPrintAnalysis(app: App): Promise<void>
export function focusFinding(app: App, index: number): void

// export.ts
export function exportModel(app: App, request: ExportRequest): Promise<void>

// notifications.ts
export function notify(app: App, notification: Omit<Notification, 'id' | 'createdAt'>): string
export function dismiss(app: App, id: string): void
```

`ExportRequest` is `{ format: '3mf' | 'stl' | 'obj' | 'step'; scope: 'visible' | 'selected' }`. `SketchChange`, `SketchCommitResult` and `PickResult` are defined in 2.14 and 2.11. Every action that changes the document goes through `app.mutations`.

### 2.10 Commands, shortcuts, toolbars and palette search

`src/commands/registry.ts`:

```ts
export type CommandCategory = 'File' | 'Edit' | 'View' | 'Create' | 'Modify' | 'Pattern' | 'Construct' | 'Sketch' | 'Constrain' | 'Print' | 'Settings'

export type Availability =
  | { readonly state: 'enabled' }
  | { readonly state: 'disabled'; readonly reason: string }
  | { readonly state: 'hidden' }

export type InvocationSource = 'toolbar' | 'palette' | 'shortcut' | 'menu' | 'inspector'

export interface CommandDefinition {
  /** Unique id, for example 'file.save' or 'feature.extrude'. */
  readonly id: string
  readonly title: string
  readonly category: CommandCategory
  /** lucide-react icon component, or null. */
  readonly icon: LucideIcon | null
  /** Extra search words, for example ['pad', 'pocket']. */
  readonly keywords: readonly string[]
  /** Shortcut specs (2.10, grammar below); the first is the one displayed. */
  readonly shortcuts: readonly string[]
  /** Contexts in which the command is listed in the palette and its shortcuts are active. */
  readonly contexts: readonly AppContext[]
  /** Only on this host; absent means every host. */
  readonly host?: 'web' | 'desktop'
  availability(app: App): Availability
  /** For toggle commands: the current state, shown as aria-pressed and a check mark. */
  checked?(app: App): boolean
  /** Secondary text in the palette, for example the undo label. */
  detail?(app: App): string | null
  run(app: App, source: InvocationSource): void | Promise<void>
}

export interface ShortcutConflict { readonly chord: string; readonly context: AppContext; readonly commands: readonly string[] }

export interface CommandRegistry {
  /** Throws Error(`Command ${id} is already registered.`) on a duplicate id. */
  register(definition: CommandDefinition): () => void
  get(id: string): CommandDefinition | undefined
  list(): readonly CommandDefinition[]
  /** Runs the command if enabled; a disabled command shows its reason as an info notification. */
  run(app: App, id: string, source: InvocationSource): Promise<void>
  /** The command bound to a chord in a context, considering host and availability 'hidden'. */
  forChord(app: App, chord: string, context: AppContext): CommandDefinition | undefined
  conflicts(platform: Platform): readonly ShortcutConflict[]
  subscribe(listener: () => void): () => void
}
export function createCommandRegistry(): CommandRegistry
```

`src/commands/shortcuts.ts`:

```ts
/** A parsed chord. `key` is normalized (2.10). */
export interface Chord { readonly mod: boolean; readonly ctrl: boolean; readonly alt: boolean; readonly shift: boolean; readonly key: string }
export function parseShortcut(spec: string): Chord
export function chordToString(chord: Chord): string
export function formatShortcut(spec: string, platform: Platform): string
export function chordFromEvent(event: KeyboardEvent, platform: Platform): Chord | null
```

Shortcut grammar: `spec = (modifier "+")* key`, modifiers in the order `Mod`, `Ctrl`, `Alt`, `Shift`, each at most once. `Mod` is Cmd (`metaKey`) on macOS and Ctrl elsewhere; `Ctrl` is the Control key on every platform and is not used together with `Mod`. `key` is an uppercase letter `A` to `Z`, a digit, `F1` to `F12`, or one of `Enter`, `Escape`, `Delete`, `Backspace`, `Tab`, `Space`, `ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`, `Home`, `End`, `=`, `-`. `parseShortcut` throws `Error('Invalid shortcut "<spec>".')` for anything else, including an empty key, a repeated modifier or a wrong modifier order.

`chordFromEvent` normalizes the key: a single letter `event.key` is uppercased; when `event.key` is not an ASCII letter but `event.code` is `KeyA` to `KeyZ` (non-Latin layouts), the letter from `event.code` is used; `' '` becomes `Space`; other keys use `event.key` as is. It returns `null` for a lone modifier press. On macOS `mod` is `metaKey` and `ctrl` is `ctrlKey`; elsewhere `mod` is `ctrlKey` and `ctrl` is always false.

`formatShortcut` renders macOS chords with symbols and no separators (`⌘⇧S`, `⌃` for Ctrl, `⌥` for Alt, `⇧` for Shift, in that order: `⌃⌥⇧⌘` per Apple convention) and other platforms with words joined by `+` (`Ctrl+Shift+S`). Keys render as `Esc`, `Del`, `⌫`/`Backspace`, `Enter`, `Space`, arrows as `↑ ↓ ← →`, letters uppercase.

`src/commands/dispatcher.ts`:

```ts
export function installShortcutDispatcher(app: App, target: Window): () => void
```

Dispatch rules, applied to every `keydown` on `window` in the bubbling phase:

1. Ignore the event when `event.defaultPrevented`, `event.isComposing` or `event.repeat` is true, or `chordFromEvent` returns null.
2. When the event target is editable (an `input` of a text-like type, a `textarea`, a `select`, or an element with `isContentEditable`), only chords with `mod` are considered, and never `Mod+Z`, `Mod+Shift+Z`, `Mod+Y`, `Mod+A`, `Mod+C`, `Mod+X` or `Mod+V` (native text editing keeps them).
3. When the palette, a menu or a popover is open, only `Mod+K` is considered (it closes the palette).
4. Find `app.commands.forChord(app, chord, context)`. If found: `preventDefault()`, then run it with source `shortcut`. A disabled command shows its reason (info notification, 3 s) instead of running.

Components that handle keys themselves (inspector Enter and Esc, sketch dimension entry, tree and timeline arrow keys, viewport navigation keys) call `preventDefault()`, which rule 1 respects.

`src/commands/fuzzy.ts`:

```ts
/** Best score of query against text, or null when query is not a subsequence of text. */
export function fuzzyScore(query: string, text: string): number | null

export interface PaletteItem {
  readonly id: string
  readonly kind: 'command' | 'feature' | 'body' | 'parameter' | 'recent' | 'setting'
  readonly title: string
  readonly keywords: readonly string[]
  readonly detail: string | null
  readonly shortcut: string | null
  readonly disabledReason: string | null
  readonly run: () => void
}

export function rankPaletteItems(query: string, items: readonly PaletteItem[], recentIds: readonly string[]): readonly PaletteItem[]
```

Scoring, exactly:

- Comparison is case-insensitive (`toLocaleLowerCase('en')`) on both strings; leading and trailing whitespace of the query is removed and inner runs of whitespace are kept as single spaces, and a space in the query matches a space in the text.
- A match assigns each query character to a strictly increasing text position with an equal character. Its score is the sum over matched characters of: 1, plus 8 if the position is 0, else plus 6 if the position is a word start (the previous text character is a space, `-`, `_`, `/`, `.`, `:` or `(`, or the original text has a lowercase letter followed by an uppercase letter there), plus 4 if the previous query character matched the immediately preceding text position. `fuzzyScore` is the maximum over all matches (dynamic programming, O(query length × text length)) minus `0.01 × text length`.
- An item's score is the maximum of its title score and each keyword score minus 2. Items with no score are excluded.
- `rankPaletteItems` sorts by score descending, then by position in `recentIds` (most recent first; items not in it after), then by title (`localeCompare` with locale `en`). With an empty query, it returns items in `recentIds` order (at most 5) followed by the remaining items in category order (File, Edit, View, Create, Modify, Pattern, Construct, Sketch, Constrain, Print, Settings, then features, bodies, parameters, recent files) and title order. It returns at most 50 items.

Example scores (tested): `fuzzyScore('ext', 'Extrude')` is 9 + 5 + 5 - 0.07 = 18.93; `fuzzyScore('ext', 'Export 3MF')` is 9 + 5 + 1 - 0.10 = 14.9; `fuzzyScore('ext', 'Text')` is 1 + 5 + 5 - 0.04 = 10.96; `fuzzyScore('xyz', 'Extrude')` is null.

`src/commands/toolbars.ts`:

```ts
export type ToolbarEntry =
  | { readonly kind: 'command'; readonly command: string }
  | { readonly kind: 'group'; readonly id: string; readonly title: string; readonly icon: LucideIcon; readonly commands: readonly string[] }
  | { readonly kind: 'separator' }
export const TOOLBARS: Readonly<Record<AppContext, readonly ToolbarEntry[]>>
```

The exact entries are in section 3.3. A toolbar entry whose command is not registered (a catalog without that feature type) is not rendered.

`src/commands/features.ts` registers one command per catalog entry: id `feature.<type>`, title the manifest label, category from the manifest category (`create` Create, `modify` Modify, `pattern` Pattern, `construct` Construct, `sketch` Create), icon from `FEATURE_ICONS[manifest.icon]` (fallback `Shapes`), keywords from `FEATURE_KEYWORDS[type]`, shortcuts from `FEATURE_SHORTCUTS[type]` (3.5), contexts `model`, `sketch` and `print` (running it from Sketch or Print switches to Model first, 3.3). The `sketch` type's command runs `startNewSketch` instead of the inspector.

`src/commands/icons.ts` maps manifest icon names (features.md 3.13) to lucide icons: `sketch` PencilRuler, `extrude` ArrowUpFromLine, `revolve` Rotate3d, `hole` CircleDot, `box` Box, `cylinder` Cylinder, `sphere` Globe, `cone` Cone, `torus` Torus, `fillet` SquareRoundCorner, `chamfer` Octagon, `shell` PackageOpen, `boolean` Combine, `move-body` Move3d, `split-body` SquareSplitVertical, `mirror` FlipHorizontal2, `linear-pattern` Columns3, `circular-pattern` Orbit, `datum-plane` Layers2. Constraint glyphs are an in-house set of 24 by 24 line icons drawn in the lucide style (2 px stroke, round caps and joins), one per constraint type, because lucide has no parallel, perpendicular or coincident symbols.

`FEATURE_KEYWORDS`: `extrude` pad, pocket, extrusion; `revolve` revolution, groove, lathe; `hole` drill, counterbore, countersink; `fillet` round, radius; `chamfer` bevel; `shell` hollow, thickness; `boolean` combine, union, subtract, intersect, join, cut; `mirror` mirrored, symmetry; `linear_pattern` array, rectangular pattern; `circular_pattern` polar pattern, array; `datum_plane` construction plane, offset plane, work plane; `move_body` placement, translate, rotate; `split_body` slice, cut body; `box` cube, block; `cylinder` rod, pin; `sphere` ball; `cone` taper; `torus` ring, donut; `sketch` drawing, profile.

### 2.11 Viewport: camera, render backend, picking, scene

`src/viewport/camera.ts` (pure functions; lengths in mm, angles in radians):

```ts
export type Vec3 = readonly [number, number, number]
export type Quat = readonly [number, number, number, number]   // x, y, z, w
export type StandardView = 'top' | 'bottom' | 'front' | 'back' | 'left' | 'right' | 'iso'

export interface Camera {
  readonly target: Vec3
  /** Camera-to-world rotation. The camera looks along its local -Z; local +Y is up; local +X is right. */
  readonly rotation: Quat
  /** Eye distance from the target. */
  readonly distance: number
  readonly projection: 'perspective' | 'orthographic'
  /** Vertical field of view, perspective only. */
  readonly fovY: number
  /** Visible height at the target, orthographic only. */
  readonly orthoHeight: number
}

export const DEFAULT_FOV_Y: number           // 30 degrees
export const DEFAULT_CAMERA: Camera          // iso, target [0, 0, 0], distance 200, perspective, orthoHeight 2 * 200 * tan(15 deg)
export function eye(camera: Camera): Vec3
export function forward(camera: Camera): Vec3
export function up(camera: Camera): Vec3
export function viewMatrix(camera: Camera): Float32Array
export function projectionMatrix(camera: Camera, aspect: number, near: number, far: number): Float32Array
export function depthRange(camera: Camera, sceneCenter: Vec3, sceneRadius: number): { near: number; far: number }
export function visibleHeightAtTarget(camera: Camera): number
export function standardViewRotation(view: StandardView): Quat
export function rotationLookingAlong(direction: Vec3, upHint: Vec3): Quat
export function classifyView(camera: Camera): StandardView | 'custom'
export function orbit(camera: Camera, deltaXPx: number, deltaYPx: number, pivot: Vec3): Camera
export function pan(camera: Camera, deltaXPx: number, deltaYPx: number, viewportHeightPx: number): Camera
export function zoomAt(camera: Camera, factor: number, cursorNdc: readonly [number, number], aspect: number): Camera
export function fitBounds(camera: Camera, bounds: BoundingBox, aspect: number, margin?: number): Camera
export function withProjection(camera: Camera, projection: 'perspective' | 'orthographic'): Camera
export function wheelZoomFactor(event: { deltaY: number; deltaMode: number; ctrlKey: boolean }, viewportHeightPx: number): number
```

Definitions:

- Standard views (camera position relative to the target, up vector): `front` from -Y, up +Z; `back` from +Y, up +Z; `right` from +X, up +Z; `left` from -X, up +Z; `top` from +Z, up +Y; `bottom` from -Z, up -Y; `iso` from the direction (1, -1, 1), up the projection of +Z onto the view plane. With these, +X points right in the front, top and bottom views, and the origin plane sketches (architecture.md) are seen from their front: XY gives `top`, XZ gives `front`, YZ gives `right`.
- `classifyView` returns the standard view whose forward and up vectors are both within 1e-6 rad of the camera's, else `custom`.
- `orbit`: horizontal motion rotates about the world +Z axis through `pivot` by `-deltaXPx × 0.4°`; vertical motion then rotates about the camera's right axis through `pivot` by `-deltaYPx × 0.4°`. There is no elevation clamp. Target and rotation both change so `pivot` stays fixed on screen.
- `pan` moves the target so the point at the target depth under the cursor follows the cursor exactly: millimeters per pixel are `visibleHeightAtTarget / viewportHeightPx`.
- `zoomAt` with factor f > 1 zooms out, f < 1 zooms in: perspective multiplies `distance` by f, orthographic multiplies `orthoHeight` by f; the target moves so that the world point at the target depth under `cursorNdc` stays under the cursor. Distance and orthoHeight are clamped to [0.01, 1e7].
- `wheelZoomFactor`: the pixel delta is `deltaY` for `deltaMode` 0, `deltaY × 16` for mode 1, `deltaY × viewportHeightPx` for mode 2; the factor is `1.0015 ^ pixels`, or `1.01 ^ pixels` when `ctrlKey` is true (trackpad pinch).
- `fitBounds`: keeps the rotation; sets the target to the box center; transforms the 8 corners into camera space; with `w` and `h` the camera-space x and y extents, the required visible height is `max(h, w / aspect) / (1 - 2 × margin)` (margin default 0.1, so the content fills 80 percent of the limiting dimension). Orthographic sets `orthoHeight` to it; perspective sets `distance = visibleHeight / (2 tan(fovY / 2)) + d / 2`, where `d` is the camera-space z extent. An empty or degenerate box (all extents below 1e-9) is treated as a 100 mm cube around its center.
- `withProjection` preserves the apparent size at the target: switching to orthographic sets `orthoHeight = 2 × distance × tan(fovY / 2)`; switching to perspective sets `distance = orthoHeight / (2 tan(fovY / 2))`.
- `depthRange` covers the scene's bounding sphere with 10 percent slack: with `c` the distance of the sphere center along the forward axis, perspective uses `near = max(c - 1.1 × r, 0.001 × (c + 1.1 × r), 0.01)` and `far = c + 1.1 × r`; orthographic uses `near = c - 1.1 × r`, `far = c + 1.1 × r` (near may be negative).

`src/viewport/backend.ts` (ADR 0007):

```ts
export type Rgb = readonly [number, number, number]   // linear, 0 to 1

export type Geometry =
  | { readonly kind: 'triangles'; readonly positions: Float32Array; readonly normals: Float32Array; readonly indices: Uint32Array; readonly elementIds: Uint32Array; readonly elementCount: number }
  | { readonly kind: 'lines'; readonly positions: Float32Array; readonly elementIds: Uint32Array; readonly elementCount: number }
  | { readonly kind: 'points'; readonly positions: Float32Array; readonly elementIds: Uint32Array; readonly elementCount: number }
  | { readonly kind: 'fill'; readonly positions: Float32Array; readonly indices: Uint32Array; readonly elementIds: Uint32Array; readonly elementCount: number }

export interface DrawSpec {
  readonly id: string
  readonly layer: 'opaque' | 'translucent' | 'overlay'
  readonly color: Rgb
  /** Optional per-element color, 3 floats per element (sketch DOF colors). */
  readonly elementColors: Float32Array | null
  readonly opacity: number
  /** Lines: width in CSS px; points: size in CSS px. */
  readonly size: number
  /** Lines only: dashed (construction geometry). */
  readonly dashed: boolean
  readonly shading: 'matcap' | 'flat'
  readonly depthTest: boolean
  readonly clipped: boolean
  /** First pick id of this draw, or null when not pickable. Element i has pick id pickBase + i. */
  readonly pickBase: number | null
  /** Pick footprint in CSS px for lines and points in the ID pass. */
  readonly pickSize: number
  /** Index into FrameSpec.analysis per vertex scalar, or false. */
  readonly analysis: boolean
}

export interface FrameSpec {
  readonly view: Float32Array
  readonly projection: Float32Array
  readonly eye: Vec3
  readonly orthographic: boolean
  readonly pixelRatio: number
  readonly uiScale: number
  readonly theme: ViewportTheme
  readonly draws: readonly DrawSpec[]
  readonly section: { readonly normal: Vec3; readonly offset: number } | null
  readonly analysis: { readonly low: Rgb; readonly high: Rgb; readonly threshold: number; readonly max: number } | null
}

export interface ViewportTheme {
  readonly backgroundTop: Rgb
  readonly backgroundBottom: Rgb
  readonly hover: Rgb
  readonly selection: Rgb
  readonly problem: Rgb
  readonly edgeDark: Rgb
  readonly edgeLight: Rgb
}

export interface PickSample {
  readonly pickId: number
  /** Distance from the queried point in CSS px. */
  readonly distance: number
  /** View-space distance in mm when the backend can read it, else null. */
  readonly depth: number | null
}

export interface RenderBackend {
  readonly kind: 'webgl2'
  resize(widthCss: number, heightCss: number, pixelRatio: number): void
  /** Uploads geometry under an id, replacing earlier geometry with that id. */
  upload(id: string, geometry: Geometry): void
  remove(id: string): void
  /** One byte per element: bit 0 hover, bit 1 selected, bit 2 problem. */
  setElementStates(id: string, states: Uint8Array): void
  /** One float per vertex, or null to clear. */
  setScalarField(id: string, values: Float32Array | null): void
  render(frame: FrameSpec): void
  /** Renders the ID pass if stale and reads every pick id within radiusCss of the point. */
  pick(frame: FrameSpec, xCss: number, yCss: number, radiusCss: number): readonly PickSample[]
  onContextLost(listener: () => void): () => void
  onContextRestored(listener: () => void): () => void
  dispose(): void
}

/** Returns null when the canvas cannot create a WebGL2 context. */
export function createWebGl2Backend(canvas: HTMLCanvasElement): RenderBackend | null
```

`src/viewport/picking.ts`:

```ts
/** RGBA8 encoding of a 32-bit pick id: r = bits 0-7, g = 8-15, b = 16-23, a = 24-31. 0 means nothing. */
export function encodePickId(id: number): readonly [number, number, number, number]
export function decodePickId(r: number, g: number, b: number, a: number): number

export type PickClass = 'point' | 'curve' | 'area'
export interface PickCandidate { readonly key: string; readonly kind: SelectionKind; readonly pickClass: PickClass; readonly distance: number }

/** The candidate the pointer means (section 3.6), or null. */
export function choosePick(candidates: readonly PickCandidate[], accepts: (kind: SelectionKind) => boolean, radiusCss: number): PickCandidate | null

/** What a pick resolved to before any core round trip. */
export type PickResult =
  | { readonly kind: 'mesh'; readonly body: string; readonly meshId: number; readonly element: 'face' | 'edge' | 'vertex'; readonly index: number; readonly key: string; readonly world: Vec3 | null }
  | { readonly kind: 'item'; readonly item: SelectionItem; readonly world: Vec3 | null }
```

`src/viewport/scene.ts` keeps every drawable's CPU-side data (so a lost WebGL context can be restored), allocates pick id ranges (consecutive, starting at 1, each draw's range is its element count), maps pick ids back to pick targets, and builds the `FrameSpec` for a camera and the store state:

```ts
export interface Scene {
  applyEvent(event: CoreEvent): void
  /** The pick target for a pick id, or null. */
  targetFor(pickId: number): { readonly result: PickResult; readonly pickClass: PickClass } | null
  /** Bounds of visible model geometry, sketches and datum planes; null when nothing is visible. */
  visibleBounds(state: AppState): BoundingBox | null
  /** Screen rectangle (CSS px) covering selection items, or null. */
  screenRect(items: readonly SelectionItem[], camera: Camera, size: { width: number; height: number }): Rect | null
  frame(state: AppState, camera: Camera, size: { width: number; height: number; pixelRatio: number }): FrameSpec
  restore(): void
}
export function createScene(backend: RenderBackend): Scene
```

`src/viewport/controller.ts`:

```ts
export interface ViewportHandle {
  getCamera(): Camera
  setCamera(camera: Camera, options?: { animate?: boolean; record?: boolean }): void
  fit(bounds?: BoundingBox): void
  setStandardView(view: StandardView): void
  toggleProjection(): void
  lookAlong(normal: Vec3, upHint: Vec3, target: Vec3): void
  zoomToBounds(bounds: BoundingBox, margin: number): void
  screenRect(items: readonly SelectionItem[]): Rect | null
  requestRender(): void
}
export interface ViewportController extends ViewportHandle { dispose(): void }
export function createViewportController(options: { app: App; canvas: HTMLCanvasElement; overlay: HTMLElement; backend: RenderBackend; scene: Scene }): ViewportController
```

Camera animations run 250 ms with a cubic ease-out, and are instant when `matchMedia('(prefers-reduced-motion: reduce)')` matches.

### 2.12 Inspector generation

`src/inspector/manifest.ts` mirrors features.md 2.3 (the protocol carries manifests as untyped JSON, protocol.md open issue 4):

```ts
export type FieldUnit = 'length' | 'angle' | 'scalar' | 'count'
export type ManifestSelection = 'body' | 'sketch' | 'sketch_region' | 'sketch_point' | 'face' | 'edge' | 'plane' | 'axis'
export interface FieldHint {
  readonly path: string
  readonly label: string
  readonly essential: boolean
  readonly unit: FieldUnit | null
  readonly selection: ManifestSelection | null
  readonly options: readonly { readonly value: string; readonly label: string }[]
}
export interface FeatureManifest {
  readonly label: string
  readonly category: 'sketch' | 'create' | 'modify' | 'pattern' | 'construct'
  readonly icon: string
  readonly version: number
  readonly fields: readonly FieldHint[]
}
/** Validates manifest JSON; missing optional members get their defaults; anything malformed throws ManifestError naming the path. */
export function normalizeManifest(json: unknown): FeatureManifest
```

A catalog entry whose manifest or schema fails validation is left out of the catalog and reported once with `console.error`; the rest of the catalog loads.

`src/inspector/model.ts`:

```ts
export type JsonSchema = Readonly<Record<string, unknown>>

export type FieldControl =
  | { readonly type: 'expression'; readonly unit: FieldUnit }
  | { readonly type: 'selection'; readonly accepts: ManifestSelection; readonly multiple: boolean; readonly min: number }
  | { readonly type: 'choice'; readonly options: readonly { value: string; label: string }[]; readonly presentation: 'segmented' | 'select' }
  | { readonly type: 'switch' }
  | { readonly type: 'text' }
  | { readonly type: 'number'; readonly integer: boolean }
  | { readonly type: 'unsupported' }

export interface InspectorField {
  readonly path: string
  readonly label: string
  readonly help: string | null
  readonly essential: boolean
  readonly required: boolean
  readonly control: FieldControl
  readonly value: unknown
  readonly error: string | null
}

export interface InspectorModel {
  readonly essential: readonly InspectorField[]
  readonly more: readonly InspectorField[]
  /** True when a field in `more` has an error, which forces the More section open. */
  readonly forceMore: boolean
  /** True when every required field has a value and no field has an error. */
  readonly complete: boolean
}

export function buildInspectorModel(input: {
  readonly feature: CatalogFeature
  readonly params: Readonly<Record<string, unknown>>
  readonly errors: Readonly<Record<string, string>>
}): InspectorModel
export function defaultParams(feature: CatalogFeature, context: DefaultsContext): Record<string, unknown>
export function setParam(feature: CatalogFeature, params: Readonly<Record<string, unknown>>, path: string, value: unknown): Record<string, unknown>
export function getParam(params: Readonly<Record<string, unknown>>, path: string): unknown
export interface DefaultsContext { readonly bodies: readonly Body[]; readonly bodiesWithGeometry: readonly string[]; readonly lastTargetBody: string | null }
```

Schema subset (draft 2020-12 as schemars 1 emits it). `buildInspectorModel` resolves `$ref` against the schema's `$defs`, then maps each manifest field hint, in manifest order, to a control:

| Schema at the hint's path | Hint | Control |
| --- | --- | --- |
| `$ref` to the `Expr` definition, or a string schema whose title is `Expr` | `unit` set | `expression` with that unit |
| any | `selection` set | `selection`; `multiple` is true when the schema at the path is an array, or for `sketch_region` (a `ProfileRef` holds a list); `min` is 1 when the path is required, else 0 |
| string `enum`, or `oneOf` of `const` strings, or the tag property (`kind`) of a tagged union | `options` set | `choice`, `segmented` for at most 4 options, else `select`, with the hint's labels in the hint's order |
| `boolean` | none | `switch` |
| `string` | none | `text` |
| `integer` or `number` | none | `number` (`integer` for `integer`) |
| anything else | none | `unsupported`: shown read-only as compact JSON with the help text "Edit this value in the .ptsm file." |

- A field is shown only when its path exists in the current value's schema branch: for a tagged union, a hint like `extent.opposite_expr` is shown only while the selected variant (by its `kind` value) declares `opposite_expr`.
- `required` is true when every object on the path lists the property in `required` (for tag-dependent fields, in the selected variant).
- `label` is the hint label; `help` is the schema `description` at the path, else null.
- `essential` fields go to `essential` in manifest order; the others to `more` in manifest order.
- `error` is `errors[path]` when present.
- `Option<T>` (`anyOf` with `{ "type": "null" }`) is the inner control with `required` false; clearing it removes the key from params.

`defaultParams`: every property with a schema `default` gets it; a tagged union gets its first variant (in schema order) with that variant's defaults; an `expression` field without a default gets `"10 mm"` for length, `"90 deg"` for angle, `"1"` for scalar and `"2"` for count; a choice without default gets its first option. Op fields (path `op` with options containing `new`): `new` when `bodiesWithGeometry` is empty, else `join`; a `body` field with selection `body`: `lastTargetBody` when op is not `new`. Selection fields start empty. Defaults are a starting point only; the core validates.

`setParam` writes a value at a path, creating intermediate objects. Changing a tagged union's tag replaces the union object with the new variant's defaults, keeping properties whose names exist in both variants (switching `distance` to `symmetric` keeps `expr`). Unknown keys present in edited params are preserved untouched (features.md 3.2).

`src/inspector/selection-values.ts` builds param values from selection items, exactly in the shapes of features.md 2.5:

```ts
/** Whether a selection field accepts the item. */
export function acceptsTarget(accepts: ManifestSelection, item: SelectionItem): boolean
/** The param value for the items, or null when they cannot form one. */
export function selectionValue(accepts: ManifestSelection, items: readonly SelectionItem[], multiple: boolean): unknown
/** Selection items for an existing param value, used when editing (labels from the document). */
export function itemsFromValue(accepts: ManifestSelection, value: unknown, state: AppState): readonly SelectionItem[]
```

| Accepts | Items accepted | Value |
| --- | --- | --- |
| `body` | body rows; faces, edges and vertices (their body) | body id string; array of ids when `multiple` |
| `sketch` | sketch features (timeline, browser), sketch curves and regions (their sketch) | sketch feature id |
| `sketch_region` | regions of one sketch | `{"sketch": id, "regions": [{"key": <RegionKey>, "fp": <fingerprint>}, ...]}` in pick order; a region of another sketch replaces the list |
| `sketch_point` | sketch points of one sketch | the point list shape defined by features/hole.md, built from `PointRef`s |
| `face` | faces | `Reference` from `ElementDescriptor.reference`; array when `multiple` |
| `edge` | edges | `Reference`; array when `multiple` |
| `plane` | origin planes, datum planes, faces whose `surface` is `plane` | `{"kind": "origin", "plane": "XY"}`, `{"kind": "datum", "feature": id}` or `{"kind": "face", "body": id, "face": <Reference>}` |
| `axis` | origin axes, sketch lines, edges whose `curve` is `line`, faces whose `surface` is `cylinder`, `cone`, `sphere` or `torus` | `{"kind": "origin", "axis": "Z"}`, `{"kind": "sketch_line", "sketch": id, "line": id}`, `{"kind": "edge", "body": id, "edge": <Reference>}` or `{"kind": "face", "body": id, "face": <Reference>}` |

When a `face` or `edge` field's parent object also has a `body` field (selection `body`) that is empty, the first accepted pick fills it with the item's body; later picks from another body are refused with the hint "Pick {kind}s of '{body name}', or clear Body first."

`src/inspector/placement.ts`:

```ts
export interface Rect { readonly x: number; readonly y: number; readonly width: number; readonly height: number }
export function placeInspector(input: { readonly anchor: Rect | null; readonly panel: { width: number; height: number }; readonly bounds: Rect; readonly gap?: number }): { x: number; y: number }
```

With `gap` default 16: if `anchor` is null, return `(bounds.x + gap, bounds.y + gap)`. Otherwise try the right side: `x = anchor.x + anchor.width + gap`; if `x + panel.width + gap <= bounds.x + bounds.width`, use it; else try the left side: `x = anchor.x - gap - panel.width`; if `x >= bounds.x + gap`, use it; else return the null-anchor position. On either side `y = clamp(anchor.y, bounds.y + gap, bounds.y + bounds.height - panel.height - gap)`, and if the panel is taller than the bounds minus two gaps, `y = bounds.y + gap`.

### 2.13 Expression input

`src/expression/evaluate.ts` mirrors features.md 3.4 so the inspector shows what a field's value will be before the feature builds:

```ts
export type FieldValue =
  | { readonly ok: true; readonly value: number }
  | { readonly ok: false; readonly message: string }
/** Converts an evaluate() result for a field of the given unit, or explains why it does not fit. */
export function convertForField(result: { value: number; dimension: Dimension }, unit: FieldUnit, units: Units, label: string, source: string): FieldValue
```

| Unit | Accepted | Value |
| --- | --- | --- |
| length | `length` | value (mm) |
| length | `dimensionless` | value × mm per document unit (mm 1, cm 10, m 1000, in 25.4) |
| angle | `angle` | value (rad) |
| angle | `dimensionless` | value × π / 180 |
| scalar | `dimensionless` | value |
| count | `dimensionless` within 1e-9 of a whole number n with 0 ≤ n ≤ 4294967295 | n |

Messages, using features.md 4.1 wording: wrong dimension: `{label} must be {expected}, but '{source}' is {found}. Use {example}.` with expected `a length`, `an angle`, `a plain number`, `a whole number`; found `a length`, `an angle`, `a plain number`, `an area`, or `a {dimension}`; examples `a value such as 10 mm`, `a value such as 45 deg`, `a plain number such as 2`, `a whole number such as 4`. Not whole: `{label} must be a whole number, but it is {value}. Use {floor} or {ceil}.` Negative count: `{label} must be at least 0, but it is {value}. Use 0 or more.` An `evaluate` error shows the core's message verbatim.

`src/expression/format.ts`:

```ts
export function formatNumber(value: number, unit: FieldUnit, units: Units): string
export function formatQuantity(value: number, unit: FieldUnit, units: Units): string
export function formatSize(bounds: BoundingBox, units: Units): string
export function formatPoint(point: Vec3, units: Units): string
```

- Length: converted from mm to the document unit, rounded to 3 decimals (mm), 4 (cm, in) or 6 (m), trailing zeros and a trailing point removed, `-0` written `0`, no digit grouping, `.` as decimal separator. `formatQuantity` appends a space and the unit (`12.5 mm`, `0.0945 in`).
- Angle: radians to degrees, rounded to 2 decimals, trimmed; `formatQuantity` appends `°` with no space (`45°`).
- Scalar: up to 6 significant digits, trimmed. Count: the integer. No suffix.
- `formatSize` is `{dx} × {dy} × {dz} {unit}` (`40 × 20 × 10 mm`); `formatPoint` is `{x}, {y}, {z} {unit}` (`10, 0, 10 mm`). Values use `formatNumber` for length.

`src/expression/step.ts`:

```ts
/** True for an optional sign, digits, an optional fraction, and an optional unit word (mm, cm, m, in, deg, rad). */
export function isPlainNumber(text: string): boolean
/** The text after an arrow-key step, or null when text is not a plain number. */
export function stepExpression(text: string, direction: 1 | -1, modifier: 'none' | 'coarse' | 'fine', unit: FieldUnit, units: Units): string | null
```

Steps: length 1 (mm), 0.1 (cm), 0.01 (m), 0.1 (in); angle 1 (in degrees); scalar and count 1. `coarse` (Shift) multiplies by 10, `fine` (Alt) by 0.1 (count fields ignore `fine`). The result keeps the unit word and the spacing between number and unit exactly as typed (`10mm` becomes `11mm`, `10 mm` becomes `11 mm`), is rounded to 10 decimals, trimmed, and `-0` is written `0`. Count fields never step below 0.

`src/expression/ExpressionInput.tsx`:

```ts
export type ExpressionState =
  | { readonly state: 'empty' }
  | { readonly state: 'pending' }
  | { readonly state: 'valid'; readonly value: number }
  | { readonly state: 'invalid'; readonly message: string }

export interface ExpressionInputProps {
  readonly id: string
  readonly label: string
  readonly value: string
  readonly unit: FieldUnit
  readonly optional?: boolean
  readonly autoFocus?: boolean
  readonly selectOnFocus?: boolean
  readonly onChange: (value: string, state: ExpressionState) => void
  readonly onCommit?: (value: string, state: ExpressionState) => void
  readonly onCancel?: () => void
}
export function ExpressionInput(props: ExpressionInputProps): React.JSX.Element
export function useExpression(text: string, unit: FieldUnit, label: string): ExpressionState
```

`useExpression` evaluates through `app.core.evaluate` with `createLatestOnly` (results for an older text never show), re-evaluates when the document revision changes, and returns `pending` while waiting. Empty text is `empty`: valid for optional fields, otherwise `invalid` with the message `{label} needs a value.`

### 2.14 Sketch module

```ts
// src/sketch/refs.ts: reserved references to the sketch origin and axes, as defined by sketch.md.
export const ORIGIN_POINT: PointRef
export const X_AXIS: string
export const Y_AXIS: string

// src/sketch/geometry.ts
export type Vec2 = readonly [number, number]
export function circleThroughThreePoints(a: Vec2, b: Vec2, c: Vec2): { center: Vec2; radius: number } | null
export function tessellateArc(center: Vec2, radius: number, start: number, end: number, toleranceMm: number): Float32Array
export function distancePointToSegment(point: Vec2, a: Vec2, b: Vec2): number
export function distancePointToArc(point: Vec2, center: Vec2, radius: number, start: number, end: number): number

// src/sketch/recipes.ts
export interface IdSource { next(prefix: 'e' | 'c'): string }
export interface SketchAddition { readonly entities: readonly SketchEntity[]; readonly constraints: readonly SketchConstraint[] }
export function lineRecipe(input: { start: Vec2; end: Vec2; construction: boolean; ids: IdSource }): SketchAddition | null
export function rectangleTwoPointRecipe(input: { corner: Vec2; opposite: Vec2; construction: boolean; ids: IdSource }): SketchAddition | null
export function rectangleCenterRecipe(input: { center: Vec2; corner: Vec2; construction: boolean; ids: IdSource }): SketchAddition | null
export function circleRecipe(input: { center: Vec2; radius: number; construction: boolean; ids: IdSource }): SketchAddition | null
export function arcThreePointRecipe(input: { start: Vec2; end: Vec2; through: Vec2; construction: boolean; ids: IdSource }): SketchAddition | null
export function pointRecipe(input: { at: Vec2; construction: boolean; ids: IdSource }): SketchAddition

// src/sketch/inference.ts
export type SnapTarget =
  | { readonly kind: 'point'; readonly ref: PointRef; readonly at: Vec2 }
  | { readonly kind: 'midpoint'; readonly line: string; readonly at: Vec2 }
  | { readonly kind: 'curve'; readonly entity: string; readonly at: Vec2 }
export type DirectionInference = { readonly kind: 'horizontal' } | { readonly kind: 'vertical' } | { readonly kind: 'tangent'; readonly entity: string }
export interface InferenceInput {
  readonly cursor: Vec2
  /** Sketch millimeters per CSS pixel at the cursor. */
  readonly mmPerPixel: number
  readonly solution: SketchSolution
  /** Start of the segment being drawn, for direction inference. */
  readonly anchor: { readonly at: Vec2; readonly ref: PointRef | null } | null
  /** Tool being drawn, which decides which direction inferences apply. */
  readonly tool: SketchToolId
  readonly suppressed: boolean
}
export interface InferenceResult { readonly point: Vec2; readonly snap: SnapTarget | null; readonly direction: DirectionInference | null }
export function infer(input: InferenceInput): InferenceResult

// src/sketch/constraints.ts
export type ConstraintType = 'coincident' | 'point_on_object' | 'horizontal' | 'vertical' | 'parallel' | 'perpendicular' | 'tangent' | 'collinear' | 'concentric' | 'equal' | 'symmetric' | 'midpoint' | 'fix'
export type DimensionType = 'distance' | 'horizontal_distance' | 'vertical_distance' | 'radius' | 'diameter' | 'angle'
export function applicableConstraints(selection: readonly SelectionItem[], solution: SketchSolution): readonly ConstraintType[]
export function constraintFor(type: ConstraintType, selection: readonly SelectionItem[], solution: SketchSolution, ids: IdSource): SketchConstraint | null
export function constraintRequirement(type: ConstraintType): string

// src/sketch/dimensions.ts
export function dimensionTypeFor(selection: readonly SelectionItem[], labelAt: Vec2, solution: SketchSolution): DimensionType | { readonly unsupported: string }
export function measuredValue(type: DimensionType, selection: readonly SelectionItem[], labelAt: Vec2, solution: SketchSolution): number
export function dimensionFor(type: DimensionType, selection: readonly SelectionItem[], expr: string, driving: boolean, labelAt: Vec2, ids: IdSource): SketchConstraint
export function defaultLabelPosition(constraint: SketchConstraint, solution: SketchSolution, mmPerPixel: number): Vec2

// src/sketch/session.ts
export type SketchToolId = 'line' | 'rectangle_2point' | 'rectangle_center' | 'circle' | 'arc_3point' | 'point' | 'dimension' | `constrain_${ConstraintType}`
export interface SketchChange {
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
export type SketchCommitResult =
  | { readonly outcome: 'committed'; readonly revision: number }
  | { readonly outcome: 'over_constrained'; readonly constraint: string; readonly conflicting: readonly string[]; readonly redundant: boolean }
  | { readonly outcome: 'failed'; readonly message: string }
```

`SketchEntity`, `SketchConstraint`, `SketchParams` and `PointRef` are the generated types from sketch.md's JSON. The recipes produce entities and constraints exactly as listed in section 3.16; their JSON is whatever sketch.md defines for those entity and constraint types.

### 2.15 Files, recent files, recovery and settings

`src/files/file-service.ts`:

```ts
export type FileKind = 'ptsm' | '3mf' | 'stl' | 'obj' | 'step'
export interface OpenedFile { readonly name: string; readonly bytes: Uint8Array; readonly handle: FileSystemFileHandle | null }
export type SavedFile = { readonly name: string; readonly handle: FileSystemFileHandle | null }
export class FileServiceError extends Error { readonly reason: 'permission' | 'not_found' | 'io' }
export interface FileService {
  readonly fileSystemAccess: boolean
  /** Null when the user cancels. */
  pickToOpen(): Promise<OpenedFile | null>
  readHandle(handle: FileSystemFileHandle): Promise<OpenedFile>
  writeHandle(handle: FileSystemFileHandle, bytes: Uint8Array): Promise<void>
  /** Null when the user cancels. Without the File System Access API it downloads and returns a null handle. */
  saveAs(suggestedName: string, kind: FileKind, bytes: Uint8Array): Promise<SavedFile | null>
}
export function createWebFileService(environment: Window & typeof globalThis): FileService
export const FILE_TYPES: Readonly<Record<FileKind, { readonly description: string; readonly mediaType: string; readonly extensions: readonly string[] }>>
```

`FILE_TYPES`: `ptsm` "Partsmith model", `application/json`, `.ptsm`; `3mf` "3MF model", `model/3mf`, `.3mf`; `stl` "STL mesh", `model/stl`, `.stl`; `obj` "OBJ mesh", `model/obj`, `.obj`; `step` "STEP model", `model/step`, `.step` and `.stp`.

- `fileSystemAccess` is true when `showOpenFilePicker` and `showSaveFilePicker` are functions.
- `pickToOpen` uses `showOpenFilePicker({ types: [ptsm], excludeAcceptAllOption: false, multiple: false })`; without the API it clicks a hidden `<input type="file" accept=".ptsm">` and resolves on `change` (null when the input reports no file). An `AbortError` resolves null.
- `writeHandle` checks `queryPermission({ mode: 'readwrite' })`, calls `requestPermission` when it is `prompt`, and throws `FileServiceError('permission')` when not granted; then `createWritable()`, `write(bytes)`, `close()`. A `NotFoundError` throws `FileServiceError('not_found')`; anything else `FileServiceError('io')` with the original message.
- `saveAs` uses `showSaveFilePicker({ suggestedName, types: [FILE_TYPES[kind]] })` then `writeHandle`; without the API it creates a `Blob` of the bytes with the kind's media type, an object URL, and an `<a download="{suggestedName}">`, clicks it, and revokes the URL after the click.

The desktop file service (Tauri dialogs) implements the same interface and is specified with the desktop host (open issue).

`src/files/recent.ts` and `src/files/recovery.ts`:

```ts
export interface RecentFile { readonly id: string; readonly name: string; readonly openedAt: number; readonly handle: FileSystemFileHandle | null; readonly text: string | null }
export interface RecentFilesStore {
  list(): Promise<readonly RecentFile[]>
  remember(file: { readonly name: string; readonly handle: FileSystemFileHandle | null; readonly text: string }): Promise<void>
  forget(id: string): Promise<void>
}
export function createIndexedDbRecentFiles(factory: IDBFactory, now?: () => number): RecentFilesStore
export function createMemoryRecentFiles(now?: () => number): RecentFilesStore

export interface RecoveryEntry { readonly name: string; readonly text: string; readonly savedAt: number; readonly handle: FileSystemFileHandle | null; readonly reason: 'autosave' | 'replaced' }
export interface RecoveryStore { read(): Promise<RecoveryEntry | null>; write(entry: RecoveryEntry): Promise<void>; clear(): Promise<void> }
export function createIndexedDbRecovery(factory: IDBFactory): RecoveryStore
export function createMemoryRecovery(): RecoveryStore
```

- IndexedDB database `partsmith`, version 1, object stores `recent` (key path `id`) and `recovery` (key path `slot`, one record with slot `current`).
- `remember` keeps at most 10 entries, most recent first. A new entry replaces an existing one for the same file: same handle (`isSameEntry` resolves true) or, for handle-less entries, the same name. Entries with a handle store `text: null`; entries without one store the text (so reopening works without file access). `id` is `crypto.randomUUID()`.
- Every IndexedDB failure (private mode, quota, blocked) is caught: `list` and `read` resolve empty, writes resolve without effect, and a `console.warn` names the store. The app never depends on them working.

`src/settings/settings.ts`:

```ts
export interface Settings {
  readonly theme: 'system' | 'light' | 'dark'
  readonly uiScale: 1 | 1.25 | 1.5 | 2 | 3
  readonly navigation: 'fusion' | 'freecad'
  readonly paletteRecent: readonly string[]
  readonly hiddenChecks: readonly string[]
  readonly moreExpanded: Readonly<Record<string, boolean>>
}
export const DEFAULT_SETTINGS: Settings
export function loadSettings(storage: Storage | null): Settings
export function saveSettings(storage: Storage | null, settings: Settings): void
```

Stored as JSON under `localStorage` key `partsmith.settings.v1`. `DEFAULT_SETTINGS`: theme `system`, scale 1, navigation `fusion`, empty lists and map. Reading wraps every access in `try`/`catch`; malformed or partial JSON yields defaults for the missing or invalid members. `paletteRecent` keeps at most 20 ids.

### 2.16 Theme

`src/theme/theme.css` is the single token set, imported by `src/index.css` in place of the bare Tailwind import:

```css
@import 'tailwindcss';
@custom-variant dark (&:where([data-theme=dark], [data-theme=dark] *));
@theme inline {
  --color-*: initial;
  --color-surface: var(--ps-surface);
  /* one --color-<name>: var(--ps-<name>) line per UI token in the table below */
}
:root, [data-theme=light] { color-scheme: light; --ps-surface: #ffffff; /* ... */ }
[data-theme=dark] { color-scheme: dark; --ps-surface: #18181b; /* ... */ }
```

`--color-*: initial` removes Tailwind's default palette, so components can only use the tokens below (color is reserved for meaning). Tokens and values (every value is exactly this in the file; contrast ratios are WCAG 2.2 ratios against the listed backgrounds and are checked by unit test U-70):

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| `surface` | `#ffffff` | `#18181b` | Top bar, browser, timeline, status bar, panels |
| `surface-muted` | `#f4f4f5` | `#27272a` | Hovered rows, inputs, segmented track |
| `surface-raised` | `#ffffff` | `#27272a` | Inspector, popovers, menus, palette, notifications |
| `border` | `#e4e4e7` | `#3f3f46` | Dividers (decorative) |
| `border-control` | `#8c8c95` | `#7a7a83` | Input and control boundaries (≥ 3:1 on surface, surface-muted, surface-raised) |
| `fg` | `#18181b` | `#f4f4f5` | Primary text and icons (≥ 4.5:1 on every surface) |
| `fg-muted` | `#52525b` | `#a1a1aa` | Secondary text (≥ 4.5:1 on every surface) |
| `fg-disabled` | `#a1a1aa` | `#71717a` | Disabled text (exempt) |
| `accent` | `#7c3aed` | `#a78bfa` | Primary buttons, focus ring, selection, active tool (≥ 3:1 on every surface) |
| `accent-hover` | `#6d28d9` | `#c4b5fd` | Primary button hover |
| `on-accent` | `#ffffff` | `#18181b` | Text on accent fills (≥ 4.5:1) |
| `accent-fg` | `#6d28d9` | `#c4b5fd` | Accent-colored text on surfaces (≥ 4.5:1) |
| `accent-subtle` | `#ede9fe` | `#2e1065` | Selected rows (fg ≥ 4.5:1 on it) |
| `danger` | `#dc2626` | `#f87171` | Error icons and borders (≥ 3:1) |
| `danger-fg` | `#b91c1c` | `#fca5a5` | Error text (≥ 4.5:1 on surfaces and danger-subtle) |
| `danger-subtle` | `#fee2e2` | `#450a0a` | Error banner background |
| `warning` | `#d97706` | `#fbbf24` | Warning icons (≥ 3:1) |
| `warning-fg` | `#b45309` | `#fcd34d` | Warning text (≥ 4.5:1 on surfaces and warning-subtle) |
| `warning-subtle` | `#fef3c7` | `#451a03` | Warning banner background |
| `success` | `#16a34a` | `#4ade80` | Success icons (≥ 3:1) |
| `success-fg` | `#15803d` | `#86efac` | Success text (≥ 4.5:1) |

Viewport tokens (read by `readViewportTheme`, not Tailwind utilities):

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| `viewport-top` | `#fafafa` | `#27272a` | Background gradient top |
| `viewport-bottom` | `#e4e4e7` | `#18181b` | Background gradient bottom |
| `body` | `#b4b4bc` | `#8a8a93` | Default body color (no `Body.color`) |
| `sketch-under` | `#2563eb` | `#60a5fa` | Under-constrained sketch geometry (≥ 3:1 on both viewport backgrounds) |
| `sketch-full` | `#18181b` | `#fafafa` | Fully constrained sketch geometry |
| `sketch-conflict` | `#dc2626` | `#f87171` | Conflicting sketch geometry |
| `sketch-region` | `#3b82f6` | `#60a5fa` | Closed region fill, drawn at 12 percent opacity |
| `sketch-reference` | `#71717a` | `#a1a1aa` | Origin axes, other sketches, dimension lines |
| `datum` | `#a1a1aa` | `#71717a` | Datum and origin plane fill at 15 percent, edges at 60 percent |
| `bed` | `#71717a` | `#a1a1aa` | Printer bed outline |
| `analysis-low` | `#fde68a` | `#fde68a` | Overhang color map at the limit angle |
| `analysis-high` | `#b91c1c` | `#ef4444` | Overhang color map at 90 degrees |

Hover and selection use `accent`; problem highlights use `danger`. Edges are drawn in `#18181b` or `#e4e4e7`, whichever has the higher contrast ratio against the body's base color, so edges stay visible on any body color in either theme.

`src/theme/theme.ts`:

```ts
export function resolveTheme(setting: Settings['theme'], prefersDark: boolean): 'light' | 'dark'
export function applyTheme(root: HTMLElement, theme: 'light' | 'dark'): void
export function readViewportTheme(root: HTMLElement): ViewportTheme
export function contrastRatio(a: string, b: string): number
```

`applyTheme` sets `data-theme` on `<html>`. `main.tsx` applies the resolved theme before the first render, and the app re-applies it when the setting or `prefers-color-scheme` changes. UI scale sets `html { font-size: calc(16px * scale) }`; all chrome sizes are in rem (Tailwind spacing), and viewport pixel sizes (line widths, pick radii, glyph sizes) are multiplied by the scale.

### 2.17 Ids and labels

`src/core/ids.ts`:

```ts
export type IdPrefix = 'p' | 'b' | 'f' | 'e' | 'c'
/** model.md 3.2: prefix, '-', 12 lowercase hex digits from 48 random bits; retries while taken. */
export function generateId(prefix: IdPrefix, isTaken: (id: string) => boolean, random?: () => number): string
/** Every id in use: parameter, body and feature ids plus sketch entity and constraint ids. */
export function usedIds(document: Document): Set<string>
```

`generateId` draws 6 random bytes with `crypto.getRandomValues` (or the injected `random`, which returns integers in [0, 2^48)), formats them as 12 zero-padded lowercase hex digits, and retries up to 64 times while `isTaken` is true, then throws `Error('Could not generate a free id after 64 attempts.')`. Ids generated in one batch are added to the taken set before the next is generated.

`src/core/labels.ts`:

```ts
/** features.md 3.12: the record's name, else "<manifest label> <n>" where n counts records of that type up to and including it. */
export function featureLabel(document: Document, featureId: string, catalog: Catalog | null): string
/** "Body n" with the smallest n ≥ 1 not used by a body name. */
export function nextBodyName(document: Document): string
```

New features are created without a `name`; their label comes from `featureLabel`. New bodies are named with `nextBodyName`.

### 2.18 Performance marks

`src/perf/marks.ts` records User Timing entries that performance tests read; they are cheap and stay in production builds.

| Name | Type | Start | End |
| --- | --- | --- | --- |
| `ps:usable` | mark | | First frame rendered with the startup document open and the core ready |
| `ps:frame` | measure | Previous rendered frame | This rendered frame (only while the camera moves or a drag runs) |
| `ps:pick` | measure | Pick query start | Pick result decoded (ID pass current) |
| `ps:sketch-drag` | measure | Pointer move that started the drag request | Solved geometry drawn |
| `ps:preview` | measure | Inspector value change | Preview meshes drawn for that change |
| `ps:rebuild` | measure | Mutating request sent | Its response frame applied and meshes drawn |
| `ps:open` | measure | File bytes read | First frame with the opened document's meshes |
| `ps:palette` | measure | Palette keystroke | Result list rendered |

Each measure name keeps at most the last 500 entries (`performance.clearMeasures` drops older ones).
