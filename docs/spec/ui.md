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
      shortcuts.ts           parseShortcut, chordToString, formatShortcut, chordFromEvent
      dispatcher.ts          installShortcutDispatcher
      fuzzy.ts               fuzzyScore, rankPaletteItems
      palette.ts             paletteItems
      builtin.ts             built-in UI commands
      features.ts            commands generated from the feature catalog
      toolbars.ts            TOOLBARS
      icons.ts               FEATURE_ICONS, constraint glyph icons
    inspector/
      manifest.ts            FeatureManifest (mirror of features.md 2.3), normalizeManifest
      model.ts               buildInspectorModel, defaultParams, setParam, getParam
      selection-values.ts    selectionValue, acceptsTarget, itemsFromValue
      placement.ts           placeInspector
      Inspector.tsx, fields/*.tsx
    expression/
      evaluate.ts            convertForField
      format.ts              formatNumber, formatQuantity, formatSize, formatPoint
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
      geometry.ts            circleThroughThreePoints, tessellateArc, distance helpers
      recipes.ts             tool outputs
      inference.ts           infer
      constraints.ts         applicableConstraints, constraintFor
      dimensions.ts          dimensionTypeFor, measuredValue, defaultLabelPosition
      session.ts             SketchToolId, SketchChange, SketchCommitResult, the internal sketch controller
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

1. `start()` creates the worker and resolves with `{ protocolVersion, engineVersion }` from the worker's `ready` message. A `fatal` message, a worker `error` event, or no `ready` within `startTimeoutMs` (default 30,000) rejects with `CoreFatalError`; the timeout message is "The geometry core did not start within {startTimeoutMs / 1000} seconds." ("... within 30 seconds." by default).
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
  onEvent(listener: (event: CoreEvent) => void): () => void
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

Every document-mutating `CoreApi` call (apply, transaction, focus, undo, redo, open, new, close, end of drag, save) runs through one `MutationQueue` per app, so the UI's order of edits is the core's order on both transports. Inspector previews and live rollback-marker drags (unlabeled `apply_edits` inside a transaction) go through a `LatestOnly` whose `send` enqueues on that queue, so they stay ordered with other mutations and coalesce while waiting. Sketch drags, sketch probes, evaluations and print analysis change nothing in the document; they use `LatestOnly` outside the queue.

### 2.6 Core API

`src/core/api.ts` wraps every command the M1 UI uses. Method names map one to one to protocol.md section 2.5 commands; argument and result shapes are the generated types.

```ts
import type {
  CatalogEntry, DocHandle, Edit, ElementDescriptor, ExportFormat, Diagnostic,
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
  solveSketch(doc: DocHandle, feature: string, candidate?: SketchParams): Promise<SolvedSketch | null>
  dragSketch(doc: DocHandle, feature: string, point: PointRef, to: readonly [number, number]): Promise<SolvedSketch | null>
  endSketchDrag(doc: DocHandle, feature: string, commit: boolean): Promise<number>
  evaluate(doc: DocHandle, expression: string): Promise<{ value: number; dimension: Dimension }>
  pickReference(doc: DocHandle, pick: Pick): Promise<ElementDescriptor>
  resolveReferences(doc: DocHandle, references: readonly Reference[]): Promise<readonly Resolution[]>
  analyzePrint(doc: DocHandle, options: { profile?: string; orientation: Orientation }): Promise<Health | null>
  exportBodies(doc: DocHandle, format: ExportFormat, bodies?: readonly string[]): Promise<{ files: readonly ExportedFile[]; warnings: readonly Diagnostic[] }>
}

/** A sketch solution with its region fill buffers (2.7). */
export interface SolvedSketch { readonly solution: SketchSolution; readonly display: SketchDisplayData | null }

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
  /** The core's dirty flag from document_changed. */
  readonly dirty: boolean
  /** Set when the UI knows the latest text is not on disk although the core considers it saved (3.19). */
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

export type OriginName = 'XY' | 'XZ' | 'YZ' | 'X' | 'Y' | 'Z' | 'O'

export type SketchItemRef =
  | { readonly kind: 'entity'; readonly sketch: string; readonly entity: string; readonly entityType: 'point' | 'line' | 'circle' | 'arc' }
  | { readonly kind: 'point'; readonly sketch: string; readonly point: PointRef }
  | { readonly kind: 'constraint'; readonly sketch: string; readonly constraint: string }
  | { readonly kind: 'region'; readonly sketch: string; readonly region: RegionInfo }
  | { readonly kind: 'origin'; readonly sketch: string; readonly which: 'point' | 'x_axis' | 'y_axis' }

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

`INITIAL_STATE`: core loading; no catalog, printers or document; context `model`; empty selection; no hover, command or sketch; print report null, overhang map on; view perspective, named `iso`, no section, filter `{ body: false, face: true, edge: true, vertex: true }`; browser open (closed when `window.innerWidth < 1024` at startup), timeline expanded, palette closed, no side panel; settings from `loadSettings()` (2.15); no notifications.

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

`formatShortcut` renders macOS chords as modifier symbols in Apple's order `⌃` (Ctrl), `⌥` (Alt), `⇧` (Shift), `⌘` (Mod), followed by the key, with no separators (`Mod+Shift+S` is `⇧⌘S`), and other platforms as words joined by `+` (`Ctrl+Shift+S`, with `Mod` written `Ctrl`). Keys: letters, digits and `F1` to `F12` as is; `Enter` is `↩` on macOS and `Enter` elsewhere; `Delete` is `⌦` and `Del`; `Backspace` is `⌫` and `Backspace`; `Escape` is `Esc`; `Space` is `Space`; arrows are `↑`, `↓`, `←`, `→` everywhere; `=` and `-` as is.

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
/** Components x, y, z, w. */
export type Quat = readonly [number, number, number, number]
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

/** 30 degrees. */
export const DEFAULT_FOV_Y: number
/** The iso view, target [0, 0, 0], distance 200, perspective, orthoHeight 2 × 200 × tan(15°). */
export const DEFAULT_CAMERA: Camera
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
/** Linear RGB, each 0 to 1. */
export type Rgb = readonly [number, number, number]

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
  /** Whether this draw colors its scalar field with FrameSpec.analysis. */
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

`src/viewport/navigation.ts`:

```ts
export type NavigationPreset = 'fusion' | 'freecad'
export type Gesture = 'orbit' | 'pan' | 'none'
/** The camera gesture for the pressed buttons (PointerEvent.buttons: 1 left, 2 right, 4 middle) and modifiers (section 3.13). */
export function gestureFor(preset: NavigationPreset, buttons: number, modifiers: { readonly shift: boolean }): Gesture
```

`gestureFor`: Fusion: middle alone is `pan`, middle with Shift is `orbit`; FreeCAD: middle alone is `pan`, middle with left or middle with right is `orbit`; every other combination is `none` (left alone selects or box-selects).

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
  | { readonly state: 'valid'; readonly value: number; readonly dimension: Dimension }
  | { readonly state: 'invalid'; readonly message: string }

export interface ExpressionInputProps {
  readonly id: string
  readonly label: string
  readonly value: string
  /** A field unit, or 'any' for parameter expressions (every dimension accepted). */
  readonly unit: FieldUnit | 'any'
  readonly optional?: boolean
  readonly autoFocus?: boolean
  readonly selectOnFocus?: boolean
  readonly onChange: (value: string, state: ExpressionState) => void
  readonly onCommit?: (value: string, state: ExpressionState) => void
  readonly onCancel?: () => void
}
export function ExpressionInput(props: ExpressionInputProps): React.JSX.Element
export function useExpression(text: string, unit: FieldUnit | 'any', label: string): ExpressionState
```

`useExpression` evaluates through `app.core.evaluate` with `createLatestOnly`, converts the result with `convertForField` (for `any`, the value and dimension are kept as returned) (results for an older text never show), re-evaluates when the document revision changes, and returns `pending` while waiting. Empty text is `empty`: valid for optional fields, otherwise `invalid` with the message `{label} needs a value.`

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
export type SaveTarget =
  | { readonly kind: 'handle'; readonly name: string; readonly handle: FileSystemFileHandle }
  | { readonly kind: 'download'; readonly name: string }
export class FileServiceError extends Error { readonly reason: 'permission' | 'not_found' | 'io' }
export interface FileService {
  readonly fileSystemAccess: boolean
  /** Null when the user cancels. */
  pickToOpen(): Promise<OpenedFile | null>
  readHandle(handle: FileSystemFileHandle): Promise<OpenedFile>
  /** Asks where to save. Null when the user cancels. Without the File System Access API it returns a download target at once. */
  chooseSaveTarget(suggestedName: string, kind: FileKind): Promise<SaveTarget | null>
  /** Writes the bytes to the target (a download for download targets). */
  write(target: SaveTarget, kind: FileKind, bytes: Uint8Array): Promise<void>
}
export function createWebFileService(environment: Window & typeof globalThis): FileService
export const FILE_TYPES: Readonly<Record<FileKind, { readonly description: string; readonly mediaType: string; readonly extensions: readonly string[] }>>
```

`FILE_TYPES`: `ptsm` "Partsmith model", `application/json`, `.ptsm`; `3mf` "3MF model", `model/3mf`, `.3mf`; `stl` "STL mesh", `model/stl`, `.stl`; `obj` "OBJ mesh", `model/obj`, `.obj`; `step` "STEP model", `model/step`, `.step` and `.stp`.

- `fileSystemAccess` is true when `showOpenFilePicker` and `showSaveFilePicker` are functions.
- `pickToOpen` uses `showOpenFilePicker({ types: [ptsm], excludeAcceptAllOption: false, multiple: false })`; without the API it clicks a hidden `<input type="file" accept=".ptsm">` and resolves on `change` (null when the input reports no file). An `AbortError` resolves null.
- `readHandle` checks `queryPermission({ mode: 'read' })`, calls `requestPermission` when it is `prompt`, and throws `FileServiceError('permission')` when not granted, then reads `getFile()`. A `NotFoundError` throws `FileServiceError('not_found')`.
- `chooseSaveTarget` uses `showSaveFilePicker({ suggestedName, types: [FILE_TYPES[kind]] })` and returns `{ kind: 'handle', name: handle.name, handle }`; an `AbortError` returns null. Without the API it returns `{ kind: 'download', name: suggestedName }`.
- `write` to a handle checks `queryPermission({ mode: 'readwrite' })`, calls `requestPermission` when it is `prompt`, and throws `FileServiceError('permission')` when not granted; then `createWritable()`, `write(bytes)`, `close()`. A `NotFoundError` throws `FileServiceError('not_found')`; anything else `FileServiceError('io')` with the original message. `write` to a download target creates a `Blob` of the bytes with the kind's media type, an object URL and an `<a download="{name}">`, clicks it, and revokes the URL after the click.

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

`--color-*: initial` removes Tailwind's default palette, so components can only use the tokens below (color is reserved for meaning). Tokens and values (every value is exactly this in the file). Contrast claims are WCAG 2.2 ratios; "on every surface" and an unqualified claim mean on `surface`, `surface-muted` and `surface-raised`; unit test U-70 checks each claim in both themes:

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
| `warning` | `#b45309` | `#fbbf24` | Warning icons (≥ 3:1) |
| `warning-fg` | `#b45309` | `#fcd34d` | Warning text (≥ 4.5:1 on surfaces and warning-subtle) |
| `warning-subtle` | `#fef3c7` | `#451a03` | Warning banner background |
| `success` | `#15803d` | `#4ade80` | Success icons (≥ 3:1) |
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

## 3. Behavior

### 3.1 Startup

1. `main.tsx` checks capabilities before anything else: `typeof WebAssembly === 'object'` and a non-null `document.createElement('canvas').getContext('webgl2')`. If either fails it renders only the unsupported page (`data-testid="unsupported"`) with the message from 4.3 and starts nothing else.
2. It loads settings, applies the resolved theme and UI scale to `<html>`, creates the transport with `chooseTransport(window)`, the stores and services, calls `createApp`, installs the shortcut dispatcher, and renders `App`.
3. The shell renders at once. The viewport shows the loading overlay (`data-testid="core-overlay"`, text "Loading the geometry core") while `app.store.getState().core.status` is `loading`.
4. `startCore` runs `transport.start()`, then `featureCatalog()` and `listPrinterProfiles()` in parallel, registers the feature commands, and sets the core status to `ready`. A failure sets it to `failed` and shows the fatal state (4.4).
5. It reads the recovery store. An entry produces the recovery notification (3.19); the app does not open it automatically.
6. It opens a new document (`newDocument('mm')`) named "Untitled". The `app` root element gets `data-core-status="ready"` only after this document's first `document_changed` is applied.
7. When the first viewport frame renders with that document, `ps:usable` is marked.

### 3.2 Layout regions

The window is a fixed layout with no page scroll:

| Region | Element | Size | Content |
| --- | --- | --- | --- |
| Top bar | `<header>`, `data-testid="top-bar"` | full width, 48 px | Browser toggle, document name and save state, context switcher, context toolbar, palette button, Export button |
| Browser | `<nav aria-label="Browser">`, `data-testid="browser"` | 264 px wide, left | Tree (3.10) |
| Viewport | `<main aria-label="3D view">`, `data-testid="viewport"` | remaining space | Canvas, DOM overlay, view cube, viewport controls, sketch header, legends, inspector, notifications |
| Side panel | `<aside>`, `data-testid="side-panel"` | 360 px wide, right, only while open | Parameters, Print health or Problems (one at a time) |
| Timeline | `<section aria-label="Timeline">`, `data-testid="timeline"` | full width, 56 px (28 px collapsed) | Timeline (3.9) |
| Status bar | `<footer>`, `data-testid="status-bar"` | full width, 28 px | Status items (3.21) |

Top bar, left to right:

- Browser toggle: icon button "Toggle browser" (`panel-left`), `aria-pressed` reflecting the browser state.
- Document name: a button (`data-testid="document-name"`, text the document name, `data-dirty="true|false"`) that opens the Document menu. Next to it, `data-testid="save-state"` shows "Edited" in `fg-muted` while the document is dirty (core `dirty` or `localUnsaved`) and is empty otherwise.
- Context switcher: `role="radiogroup"` with `aria-label="Context"` and three radios "Model", "Sketch", "Print" (segmented control, one tab stop, arrow keys move).
- Context toolbar: `role="toolbar"` with `aria-label` "Model tools", "Sketch tools" or "Print tools" (3.3).
- Palette button: "Search" with the formatted `Mod+K` hint (`data-testid="palette-button"`).
- Export button: "Export" (`download` icon) opening the export popover (3.18).

Document menu (`role="menu"`, `data-testid="document-menu"`), in order: New (desktop shortcut shown), Open..., Open recent (submenu, 3.19), Save, Save as..., Export..., separator, Theme (submenu of radio items System, Light, Dark), Navigation (submenu: Fusion, FreeCAD), UI scale (submenu: 100%, 125%, 150%, 200%, 300%).

Responsive rules:

- When the window is narrower than 1024 CSS px at startup, the browser starts closed. Below 1024 px the browser and side panel open as overlays above the viewport (they do not resize it) and close on Esc or on a pointer press outside them.
- Toolbar buttons show icon and label at window widths of 1280 px and above, and icon only below (the label stays the accessible name and appears in the tooltip).
- The minimum supported window is 800 by 600 CSS px at 100 percent scale.

Floating elements: the inspector (3.7, 3.8) inside the viewport; the notifications stack at the viewport's bottom-right (3.22); the palette centered horizontally, 15 percent from the top of the window, 560 px wide; menus and popovers anchored to their buttons (native Popover API, light dismiss).

### 3.3 Contexts and toolbars

`TOOLBARS` (labels are command titles; feature titles are manifest labels):

| Context | Entries in order |
| --- | --- |
| Model | Sketch (`feature.sketch`), Extrude, Revolve, Hole, Fillet, Chamfer, Shell, group "Primitive" (`cuboid` icon: Box, Cylinder, Sphere, Cone, Torus), group "Pattern" (`grid-3x3` icon: Linear pattern, Circular pattern), Mirror, Boolean |
| Sketch | Line, group "Rectangle" (`rectangle-horizontal`: 2-point rectangle, Center rectangle), Circle, Arc, Point, Construction, Dimension, group "Constrain" (`magnet`: the 13 constraint commands in the order of 3.16.6), separator, Finish sketch (primary style) |
| Print | Printer (menu button showing "Printer: {name}"), Overhangs (toggle), Checks (popover), Health (opens the health panel; shows the finding count), separator, Export 3MF (primary style), group "Export" (`file-output`: STL, OBJ, STEP) |

Datum plane, Move body and Split body are in the palette only. A group button opens a menu (`role="menu"`) of its commands with icon, title and shortcut. Disabled toolbar commands render with `aria-disabled="true"` and show the reason in their tooltip.

Context rules:

- The app starts in Model. `data-context` on the `app` root mirrors the context.
- Model to Print, or Print to Model: an active feature command is cancelled first (`cancel_transaction`). The selection is kept.
- Choosing the Sketch radio outside sketch mode runs `startNewSketch` (3.16.1). In sketch mode, choosing Model or Print finishes the sketch (3.16.11) and then switches.
- A feature command run from Sketch finishes the sketch, switches to Model and starts the command. If the finished sketch has exactly one closed region and the command has a `sketch_region` field, that region is preselected. Run from Print, it switches to Model first.
- Print commands are listed and active in Print only. Sketch tools are listed and active in Sketch only, and only while a sketch is being edited (not during plane picking).

### 3.4 Command palette

- Opens with `Mod+K` (from anywhere, including text fields) or `S` (when focus is not editable), or the palette button. `Mod+K` while open closes it.
- Layout: a container `role="dialog"` with `aria-label="Command palette"` and `data-testid="palette"`, holding a `role="combobox"` input (`data-testid="palette-input"`, placeholder "Search commands, bodies and parameters") with `aria-controls` pointing at a `role="listbox"`. Options (`role="option"`, `data-testid="palette-option"`, `data-item-id`, `data-kind`) show icon, title, detail (muted), a kind tag for non-commands ("Feature", "Body", "Parameter", "Recent file"), and the formatted shortcut (`data-testid="palette-shortcut"`).
- Items (`paletteItems`): every registered command whose availability is not `hidden` in the current context; every feature in the document ("{label}", kind feature, runs: select it in the timeline and scroll it into view); every body ("{name}", kind body, runs: select it and frame it); every parameter ("{name} = {expr}", kind parameter, runs: open the Parameters panel with that row's expression input focused); recent files ("Open recent: {name}", kind recent). Settings are ordinary commands of category Settings: "Theme: System", "Theme: Light", "Theme: Dark", "Navigation: Fusion", "Navigation: FreeCAD", "UI scale: 100%" to "UI scale: 300%", each with `checked` showing the current value.
- Ranking: `rankPaletteItems` with `settings.paletteRecent` (2.10). The first option is active when the list changes.
- Keys: ArrowDown and ArrowUp move the active option and wrap; PageDown and PageUp move by 8; Enter runs the active option and closes the palette; Esc closes it. Focus stays in the input. Tab is ignored.
- Mouse: hovering an option makes it active; clicking runs it.
- A disabled option is dimmed with its reason as the detail; Enter on it shows the reason in the palette footer (`role="status"`) and keeps the palette open.
- Running any item records its id at the front of `settings.paletteRecent`.
- Closing returns focus to the element that had it before the palette opened. A pointer press outside closes it.
- Each keystroke re-ranks synchronously; `ps:palette` measures it.

### 3.5 Keyboard shortcuts

Registry shortcuts (displayed with `formatShortcut`; `Mod` is Cmd on macOS and Ctrl elsewhere):

| Command id | Title | Shortcut | Contexts |
| --- | --- | --- | --- |
| `app.palette` | Command palette | Mod+K, S | all |
| `edit.undo` | Undo | Mod+Z | all |
| `edit.redo` | Redo | Mod+Shift+Z, Mod+Y | all |
| `file.new` | New | Mod+N (desktop only) | all |
| `file.open` | Open | Mod+O | all |
| `file.save` | Save | Mod+S | all |
| `file.save_as` | Save as | Mod+Shift+S | all |
| `file.export` | Export | Mod+Shift+E | all |
| `edit.delete` | Delete | Delete, Backspace | all |
| `edit.rename` | Rename | F2 | all |
| `view.browser` | Toggle browser | Mod+B | all |
| `view.fit` | Fit to view | F6, Shift+F | all |
| `view.home` | Home view | Shift+H | all |
| `view.look_at` | Look at | Shift+L | all |
| `view.projection` | Orthographic | Shift+P | all |
| `view.section` | Section view | Shift+X | all |
| `view.visibility` | Show or hide selection | V | model, print |
| `feature.extrude` | Extrude | E | all |
| `feature.hole` | Hole | H | model, print |
| `feature.fillet` | Fillet | F | model, print |
| `feature.move_body` | Move body | M | model, print |
| `sketch.line` | Line | L | sketch |
| `sketch.rectangle_2point` | 2-point rectangle | R | sketch |
| `sketch.circle` | Circle | C | sketch |
| `sketch.dimension` | Dimension | D | sketch |
| `sketch.construction` | Construction | X | sketch |
| `sketch.constrain_coincident` | Coincident | I | sketch |
| `sketch.constrain_horizontal` | Horizontal | H | sketch |
| `sketch.constrain_vertical` | Vertical | V | sketch |
| `sketch.select_all` | Select all | Mod+A | sketch |
| `sketch.finish` | Finish sketch | Mod+Enter | sketch |

Commands without a default shortcut (ids and titles; all listed in the palette in the contexts given):

| Command id | Title | Contexts |
| --- | --- | --- |
| `feature.<type>` for every other catalog type | Manifest label (Sketch, Revolve, Chamfer, Shell, Box, Cylinder, Sphere, Cone, Torus, Boolean, Mirror, Linear pattern, Circular pattern, Datum plane, Split body) | all (Sketch switches to plane picking) |
| `context.model`, `context.sketch`, `context.print` | Model, Sketch, Print | all |
| `edit.edit_feature` | Edit feature | model, print (one feature selected) |
| `edit.suppress`, `edit.unsuppress` | Suppress, Unsuppress | model, print (features selected) |
| `edit.clear_selection` | Clear selection | all |
| `view.parameters`, `view.problems`, `view.health` | Parameters, Problems, Print health | all |
| `view.timeline` | Toggle timeline | all |
| `view.standard.<view>` for the seven standard views | View: Top, View: Bottom, View: Front, View: Back, View: Left, View: Right, View: Home | all |
| `view.filter.body`, `view.filter.face`, `view.filter.edge`, `view.filter.vertex` | Select bodies, Select faces, Select edges, Select vertices (toggles) | model, print |
| `view.lock`, `view.unlock` | Lock selection, Unlock selection | model, print |
| `sketch.rectangle_center`, `sketch.arc_3point`, `sketch.point` | Center rectangle, Arc, Point | sketch |
| `sketch.constrain_<type>` for the constraints without a key | Point on object, Parallel, Perpendicular, Tangent, Collinear, Concentric, Equal, Symmetric, Midpoint, Fix | sketch |
| `sketch.show_constraints` | Show constraints (toggle) | sketch |
| `print.printer`, `print.overhangs`, `print.checks`, `print.check_now` | Choose printer, Overhangs (toggle), Checks, Check print | print (`print.check_now` all) |
| `print.export_3mf`, `print.export_stl`, `print.export_obj`, `print.export_step` | Export 3MF, Export STL, Export OBJ, Export STEP | all |
| `document.units.mm`, `document.units.cm`, `document.units.m`, `document.units.in` | Units: mm, Units: cm, Units: m, Units: in | all |
| `settings.theme.system`, `settings.theme.light`, `settings.theme.dark` | Theme: System, Theme: Light, Theme: Dark | all |
| `settings.navigation.fusion`, `settings.navigation.freecad` | Navigation: Fusion, Navigation: FreeCAD | all |
| `settings.scale.100`, `.125`, `.150`, `.200`, `.300` | UI scale: 100%, 125%, 150%, 200%, 300% | all |

`FEATURE_SHORTCUTS` holds the four feature entries above; other commands have no shortcut. Letters follow Fusion where Fusion has a default (E, H, F, M, L, R, C, D, X, S); `Mod+N` is desktop only because browsers reserve Ctrl+N. Unit test U-22 asserts `conflicts()` is empty for the built-in registry with the test catalog on every platform.

Keys handled by components, not the registry:

| Key | Where | Effect |
| --- | --- | --- |
| Esc | anywhere | The Esc ladder below |
| Enter | inspector fields, inline editors, line tool | The Enter ladder below |
| Arrow keys | focused viewport | Orbit 15 degrees per press (Left and Right about world Z, Up and Down about the camera right axis) |
| Shift+arrow keys | focused viewport | Pan by 10 percent of the viewport height |
| `=` or `+`, `-` | focused viewport | Zoom in or out by 1.25 at the viewport center |
| Arrow keys, Home, End | toolbars, timeline, tree, menus, segmented controls | Move focus (roving tab index) |
| Shift+F10, ContextMenu | timeline item, browser row | Open the context menu |
| Space | timeline item, browser row | Select (with Mod: toggle) |
| Tab | sketch draw inputs | Cycle between the inputs |
| Hold Ctrl (Windows, Linux) or Cmd (macOS) | sketch drawing | Suppress inference (3.16.5) |
| Shift or Mod with click | viewport, timeline, browser | Toggle the item in the selection |

Esc ladder: each press does the first step that applies: (1) close the innermost open menu, popover, palette, inline rename or inline editor; (2) in sketch mode, cancel the in-progress drawing, drag or box selection (the tool stays active); (3) exit the active sketch tool or constraint pick mode; (4) cancel the active feature command; (5) cancel the plane pick of a new sketch (back to Model); (6) clear the selection. Esc never leaves sketch mode once a sketch is being edited.

Enter ladder: (1) commit the focused inline editor (rename, dimension editor, draw inputs); (2) in the line tool with a chain in progress and no draw input focused, end the chain; (3) when focus is in the inspector (not on a button) or on the viewport and a feature command is active, confirm it if it is complete. Enter on a focused button activates that button only.

### 3.6 Selection, hover and picking

Pick classes and priority (`choosePick`): `point` (vertex, sketch point, origin point) candidates within the radius win over `curve` (edge, sketch curve, origin axis) candidates within the radius, which win over `area` (face, sketch region, origin plane, datum plane) candidates at distance 0. Within a class the smallest distance wins; ties go to the smallest pick id. The radius is 4 CSS px times the UI scale. In the ID pass, edges and sketch curves are drawn 7 px wide, vertices as 9 px squares and sketch points as 11 px squares (times the UI scale), all depth-tested; faces use a polygon offset so coplanar edges win.

Hover:

- On `pointermove` with no button pressed and no camera gesture, at most once per animation frame, the controller calls `backend.pick` and `choosePick` with the accepted kinds, sets the hover target, and updates highlights. The viewport element mirrors it in `data-hover-kind` (`none` or a `SelectionKind`) and `data-hover-key`.
- Mesh hover keys are `mesh:{meshId}:{face|edge|vertex}:{index}` and need no core call. The cursor is `pointer` over a pickable target.
- Hovered faces are tinted with `accent` at 20 percent, hovered edges drawn 2.5 px in `accent` at 70 percent, hovered vertices as 7 px `accent` dots, hovered sketch items in `accent`.

Accepted kinds:

- Model and Print contexts, no command: the selection filter (viewport control) has Bodies, Faces, Edges and Vertices; default Faces, Edges and Vertices on and Bodies off. With Faces off and Bodies on, a face candidate becomes its body. Sketch curves, points and regions of visible sketches, visible datum planes and visible origin items are always accepted. Locked bodies are not drawn into the ID pass.
- With a feature command: only what the active selection field accepts (2.12 table); with no active selection field, nothing (camera only).
- Sketch mode: sketch items of the active sketch (curves, points, constraint glyphs and dimension labels through the DOM) and the origin point and axes.

Click (pointer down and up within 4 px and 400 ms, primary button):

- Mesh picks call `pickReference(doc, { mesh_id, kind, index })` to get the `ElementDescriptor`; the resulting item has key `element:{body}:{name}`. A `stale_pick` or `preview_not_pickable` error ignores the click silently.
- No modifier: the selection becomes that item, or empty when nothing was hit. Shift or Mod: the item is toggled in the selection.
- During a feature command, clicks go to `capturePick` instead (3.7).

Double-click:

- Face: edits the feature named by the first path segment of the face's element name (the text before the first `/`, features.md 3.9) when that is a feature id in the document; otherwise nothing.
- Sketch curve, point or region in Model context: `enterSketch` for its sketch. Datum plane: `editFeature`. Anything else: nothing.

Selection keys: faces, edges and vertices `element:{body id}:{element name}`; bodies `body:{id}`; features (timeline, sketches, datums in the browser) `feature:{id}`; datum planes picked in the viewport `feature:{id}` with kind `datum_plane`; sketch curves `sketch:{sketch id}:{entity id}`; sketch points `point:{sketch id}:{point ref}`; regions `region:{sketch id}:{key joined by "|" and ","}`; constraints `constraint:{sketch id}:{constraint id}`; origin items `origin:XY`, `origin:XZ`, `origin:YZ`, `origin:X`, `origin:Y`, `origin:Z`, `origin:O`; parameters `parameter:{id}`.

Selection after rebuilds: when model-layer meshes change, the UI calls `resolveReferences` for the references of selected faces, edges and vertices. Resolved items keep their selection and are highlighted at the matched display ids; missing or ambiguous items leave the selection silently. Features, bodies and sketch items leave the selection when they no longer exist in the recipe or solution.

Selected faces are tinted with `accent` at 35 percent; selected edges drawn 2.5 px in `accent`; selected vertices as 7 px `accent` dots. The viewport element mirrors the selection size in `data-selection-count`. A polite live region announces "{label} selected", "{n} items selected" or "Selection cleared", debounced by 500 ms.

### 3.7 Feature commands, inspector and live preview

Starting a new feature (`startFeatureCommand(type)`):

1. Cancel an active command; finish sketch mode (remembering the finished sketch); switch to Model.
2. `beginTransaction(doc, manifest.label)`.
3. Generate the feature id (`f-`), set `params = defaultParams(feature, context)` where `lastTargetBody` is the `body` param of the latest active feature in timeline order that has a string `body` param naming a body with model-layer geometry, and `bodiesWithGeometry` comes from the model-layer meshes.
4. Preselection: walk the selection fields in manifest order; each takes the app selection items it accepts (in selection order; one item when not `multiple`) that no earlier field took. The finished-sketch rule of 3.3 applies when the app selection is empty. The app selection is cleared.
5. The active field is the first selection field that is required and empty, skipping a `body` field whose parent object also has a `face` or `edge` field (the first pick fills it, 2.12); if none, focus goes to the first essential expression field with its text selected.
6. The inspector opens (placement below). If the params are complete, the first preview is applied at once.

Editing an existing feature (`editFeature(id)`): a `sketch` feature enters sketch mode instead. Otherwise: cancel an active command, `beginTransaction(doc, 'Edit {label}')`, `setFocus(doc, id)`, take `params` from the record, mark the command `applied`, and open the inspector titled with the feature label. Selection fields show `itemsFromValue` and their references are highlighted through `resolveReferences` on the model layer (which shows the state before the feature). If `setFocus` fails with `focus_inactive`, the transaction is cancelled and the core's message is shown as an error notification with the action "Unsuppress" (suppressed feature: unsuppresses it and retries the edit) or "Move marker to the end" (rolled-back feature: sets the rollback to the end and retries).

Preview cycle: every field change recomputes local field errors (expression states, selection minimums). When the params are complete (every required field set, no field error, no expression pending), the UI submits a preview through a `LatestOnly` over `applyEdits(doc, null, edits)`:

- First application: `add_body` (when op is `new` and the body field is empty: a new body with a `b-` id and `nextBodyName`; the `body` param is set to it), then `add_feature` with the record `{ id, type, ...params }` (no name) at index `features.length` when `rollback` is null, else at the rollback feature's index plus 1 followed by `set_rollback` to the new id; then `setFocus(doc, id)`.
- Later applications: `update_feature` with the full record. Switching op away from `new` removes the body this command created (`remove_body`) and sets `body` to `lastTargetBody`; switching back to `new` adds a fresh body.
- The response's events update the store: `recomputed` gives the feature's status; `body_mesh` events with layer `preview` carry the preview geometry, shown as normal shaded bodies; the model layer shows the state before the feature and stays pickable.
- Incomplete or invalid params after an application hide the preview layer (nothing applied); the next complete state applies again.
- `ps:preview` measures from the change to the drawn preview.

The inspector shows the focused feature's status messages in a banner (`data-testid="inspector-error"`): errors with `role="alert"` and `danger` styling, warnings with `warning` styling. Each message is followed by the fix actions in 4.2.

Confirming (Enter ladder, or the OK button): allowed only when complete. A waiting preview is applied first (awaited). Then `commitTransaction(doc)`, the inspector closes, the command clears and focus returns to where it was before the command. A sketch referenced by the committed params through a `sketch` key is hidden for the session (3.10). If the committed feature's status is `error`, the failure notification of 4.2 appears.

Cancelling (Esc ladder, the Cancel button or the close button): `cancelTransaction(doc)`, the inspector closes, nothing is recorded.

`Mod+Z` during a command cancels the command and does nothing else.

Inspector panel (`role="dialog"`, `aria-modal="false"`, `aria-labelledby` its title, `data-testid="inspector"`, `data-mode="command"`, `data-preview` mirroring `ActiveCommand.preview`: `none` before anything is applied or while the params are incomplete, `pending` while a preview request is in flight, `shown` when the preview layer for the latest params is drawn, `failed` when the focused feature's status is `error`), 320 px wide, maximum height the viewport height minus 32 px with internal scrolling:

- Header: feature icon, title (manifest label for a new feature, feature label when editing), close button "Cancel". The header is the drag handle.
- Status banner as above.
- Essential fields, then a disclosure button "More options" / "Fewer options" (`data-testid="inspector-more"`, `aria-expanded`) when there are more fields. Its state is remembered per feature type in `settings.moreExpanded`. `forceMore` opens it.
- Footer: "Cancel" and "OK" (`data-testid="inspector-ok"`, primary). OK is `aria-disabled` while incomplete, with the tooltip "Fill in the highlighted fields first."; required empty selection fields get a dashed `accent` border and invalid fields a `danger` border.
- Each field is a container with `data-testid="inspector-field"` and `data-path`.

Field controls:

- Expression: `ExpressionInput` (2.13) labeled with the hint label. To its right, `data-testid="expression-result"` shows the unit symbol when `isPlainNumber(text)` holds and the text has no unit word (`mm`, `°`), otherwise `= {formatQuantity(value)}` (`= 10 mm`), or nothing while empty or pending. Below it, `data-testid="field-error"` (`aria-live="polite"`, referenced by `aria-describedby`) shows the message of an invalid state. ArrowUp and ArrowDown step plain numbers (`stepExpression`; Shift coarse, Alt fine) and do nothing otherwise.
- Selection: a box (`data-testid="selection-field"`, `role="button"`, `data-active`, `data-count`) showing the placeholder `Select {noun}` (`Select edges`, `Select a plane`, `Select regions`, `Select a body`, `Select bodies`, `Select a sketch`, `Select points`, `Select faces`, `Select an axis`), one item's label, or `{n} {plural noun}` (`3 edges`). Clicking it makes it the active field (`accent` border); a clear button "Clear {label}" empties it. For `body`, `sketch`, `plane` and `axis` fields and for `sketch_region` fields, a "Choose" menu button lists the enumerable candidates (bodies with geometry; sketches; origin planes and datum planes; origin axes; regions of the current profile's sketch, or of every visible sketch when empty, as "Region {n} ({area})") so these fields work without a pointer. Hovering an item's entry in the box highlights it in the viewport.
- Choice: a segmented control (`role="radiogroup"`) for up to 4 options, else a native `<select>`, with the hint's option labels.
- Switch: `role="switch"`. Text and number: text inputs; number fields reject non-numeric text with "{label} must be a number."
- Unsupported: read-only compact JSON with the help "Edit this value in the .ptsm file."

Selection capture (`capturePick(item)`): when the active field accepts the item (`acceptsTarget`; geometry checks such as a flat face use the item's `ElementDescriptor`), the item is toggled in the field: added (replacing the single value when not `multiple`) or removed when already present. A refused item shows a hint under the field ("Pick a flat face, a datum plane or an origin plane." for `plane`; "Pick a straight edge, a sketch line, a round face or an origin axis." for `axis`; "Pick {plural noun}." otherwise) for 3 s. When a non-multiple field receives its item, the active field advances to the next required empty selection field, if any. The sibling `body` rule of 2.12 applies.

Placement: when the inspector opens, it is placed with `placeInspector` where `anchor` is the screen rectangle of the preselected items (or null) and `bounds` the viewport rectangle; it then stays put while open. Dragging the header moves it within the viewport, and the dragged position (`panels.inspectorPosition`) is used for every inspector until reload.

### 3.8 Selection inspector

With no feature command active and a non-empty app selection (Model and Print contexts), or a non-empty sketch selection with no tool active (sketch mode), the inspector shows the selection (`data-mode="selection"`, accessible name "Selection"). It uses the same panel, placement and dragging rules, and closes when the selection empties.

| Selection | Title (`data-testid="selection-title"`) | Rows and actions |
| --- | --- | --- |
| One body | Body name | Size (`data-testid="body-size"`, `formatSize` of the model-layer bounding box, or "Empty"), Center (`data-testid="body-center"`, `formatPoint` of the box center), Color (swatch button), Material (combobox with suggestions PLA, PETG, ABS, ASA, TPU, Resin; Enter commits "Change material"; empty clears), Visible (switch) |
| One feature, sketch or datum | Feature label | Type (manifest label), status messages with fix actions (4.2), buttons "Edit" and "Suppress" or "Unsuppress" |
| Faces, edges and vertices | "1 face", "3 edges", "2 vertices", or "{n} items" when kinds mix | Body name(s); for one face its surface ("Flat face", "Cylindrical face", "Conical face", "Spherical face", "Toroidal face", "Freeform face", "Face") and area (`formatNumber` in the document unit followed by `{unit}²`); for one edge its curve ("Straight edge", "Circular edge", ...) and length; for one vertex its position. Suggested commands: edges: Fillet, Chamfer; one flat face: Sketch, Hole, Shell; other faces: Shell |
| Regions | "1 region", "{n} regions" | Suggested commands: Extrude, Revolve |
| One origin plane or datum plane | "XY plane" or the datum label | Suggested command: Sketch |
| Anything else mixed | "{n} items" | none |
| Sketch entities and points | "1 line", "2 lines", "1 circle", "1 arc", "1 point", "{n} items" | Constraint suggestions (`applicableConstraints`, each a button with glyph, title and shortcut), Construction (switch, for entities), Delete |
| One sketch constraint | Constraint title ("Horizontal", "Distance") | For dimensions: an expression field with the stored expression (Enter commits "Change dimension") and a Reference switch; Delete |

Suggested commands run the command with the selection preselected.

### 3.9 Timeline

Layout, left to right: collapse button ("Collapse timeline" / "Expand timeline"); the items in timeline order with the rollback marker in the gap after the last active feature; at the right end, the problems button (`data-testid="timeline-problems"`, text "1 problem" or "{n} problems", hidden when there are none). The item strip scrolls horizontally; a newly added item and the marker scroll into view.

Items (`data-testid="timeline-item"`, `data-feature-id`, `data-status` `ok|warning|error|suppressed|rolled_back|pending`, `data-suppressed`, `data-rolled-back`):

- A 40 by 40 px button with the feature icon (20 px). Its accessible name is the feature label; `aria-describedby` points at a hidden text with the status ("Error: {first message}", "Warning: {first message}", "Suppressed", "Rolled back").
- A status badge at the bottom-right: `triangle-alert` in `warning` for warnings, `circle-x` in `danger` for errors. The tooltip shows the label and every message.
- Suppressed items draw their icon at 40 percent opacity with a diagonal strike. Items after the marker draw at 40 percent opacity. While a feature is being edited, it gets an `accent` outline and items after it draw at 40 percent; the sketch being edited gets an `accent` outline.
- Selected items have an `accent-subtle` background.

Interactions:

- Click selects the feature (Shift or Mod toggles). Double-click or Enter edits it (`editFeature`). F2 renames inline: a text field over the item; Enter commits `update_feature` with the new `name` (label "Rename"); an empty name removes `name` (the default label returns); Esc cancels.
- Context menu (right-click, Shift+F10, ContextMenu key; `role="menu"`, `data-testid="context-menu"`): Edit, Rename, Suppress or Unsuppress, Roll back to here, Delete.
- Suppress: `update_feature` with `suppressed: true` (label "Suppress {label}"); unsuppress likewise ("Unsuppress {label}"). Several selected features change in one edit.
- Delete (Delete key or menu): one `apply_edits` labeled "Delete {label}" or "Delete {n} features" with `remove_feature` edits; when the rollback feature is among them, a `set_rollback` to the nearest earlier active feature that is not deleted (or the end, when none) comes first. An info notification "Deleted {label}." offers "Undo".
- Reorder: pressing an item and moving more than 4 px horizontally starts a drag: a ghost follows the pointer and an insertion bar marks the nearest gap. Dropping at gap g (0 to N) moves the feature to index g when g is less than its current index, else g minus 1; the same position does nothing. The edit is `move_feature` labeled "Reorder". Esc cancels the drag. The core does not check dependencies when moving (model.md 3.9); a feature that now comes before something it uses fails and triggers the failure notification of 4.2, whose "Undo" restores the order.
- Rollback marker (`data-testid="timeline-marker"`): an 8 px handle with a 2 px `accent` line, `role="slider"`, `aria-label="Rollback marker"`, `aria-valuemin="1"`, `aria-valuemax` the feature count, `aria-valuenow` the number of active features, `aria-valuetext` "After {label}" or "At the end". Position p (1 to N) means features 1 to p are active; p = N writes `set_rollback` with no id (the end of the timeline), p < N writes the id of feature p.
  - Dragging: `beginTransaction(doc, 'Roll back')`, then on every gap change a `LatestOnly` `apply_edits` with the `set_rollback`, so the model updates live; dropping commits; Esc cancels the transaction. Dropping at the starting gap cancels.
  - Keys: ArrowLeft and ArrowRight move by one gap, Home to position 1, End to the end; each key press applies its own edit labeled "Roll back".
- Collapsed (28 px): only the expand button, the text "{n} features" and the problems button remain.
- Keyboard: the strip is a toolbar with one tab stop; Left and Right move focus between items and the marker; Space selects; Mod+Space toggles.

Problems panel (side panel `problems`, `data-testid="problems-panel"`), opened by the problems button or the palette command "Problems": every feature whose status is warning or error, in timeline order, then document diagnostics. Each row shows the status icon, the feature label (a button that selects it), its messages, and an "Edit" button (`editFeature`). Empty: "No problems."

### 3.10 Browser

A tree (`role="tree"`, `aria-label="Browser"`) with these groups (`role="treeitem"` with children; expansion kept for the session):

| Group | Rows (`data-testid="browser-row"`, `data-kind`, `data-id`) | Default |
| --- | --- | --- |
| Origin | XY, XZ, YZ (kind `origin_plane`); X, Y, Z (kind `origin_axis`); Origin (kind `origin_point`) | collapsed; items hidden |
| Bodies | One per `Document.bodies`, in order (kind `body`) | expanded |
| Sketches | One per `sketch` feature, in timeline order (kind `sketch`) | expanded |
| Construction | One per `datum_plane` feature (kind `datum`) | expanded |
| Parameters | One per parameter, in order (kind `parameter`) | expanded |

Row content:

- Body: color swatch button ("Color of {name}"), name, "(empty)" in `fg-muted` when the body has no model-layer geometry, lock button ("Lock {name}" / "Unlock {name}", `aria-pressed`), eye button ("Hide {name}" / "Show {name}").
- Sketch and datum: icon, feature label, status badge as in the timeline, eye button ("Hide {label}" / "Show {label}").
- Origin item: icon, name ("XY plane", "X axis", "Origin"), eye button.
- Parameter: "{name} = {expr}" and the evaluated value in `fg-muted` when the expression is not a plain number; the group header has an icon button "Edit parameters" (`sliders-horizontal`).

Behavior:

- Click selects (Shift or Mod toggles). While a feature command is active, clicking a row whose kind the active field accepts captures it instead (origin planes and datums for `plane`, origin axes for `axis`, bodies for `body`, sketches for `sketch`).
- Double-click or Enter: sketch: `enterSketch`; datum: `editFeature`; body: inline rename; parameter: opens the Parameters panel focused on it.
- F2 renames bodies (`update_body`, label "Rename") and features (as in the timeline). Delete removes selected sketches and datums (`deleteFeatures`) and selected bodies (`remove_body`, label "Delete {name}"; the core does not check use, so dependent features fail and the failure notification offers Undo). V toggles visibility of selected rows.
- Body eye: `update_body` with `visible` flipped (labels "Hide {name}" / "Show {name}"; undoable). Hidden bodies are not drawn or picked.
- Sketch, datum and origin eyes change `view.sessionVisible` only (not saved, not undoable). On opening a document, a sketch starts hidden when another feature's params contain an object whose `sketch` key equals its id (the `ProfileRef`, `AxisRef` and hole shapes of features.md 2.5), and visible otherwise; datum planes start visible; origin items start hidden. Origin planes are always shown while a plane is being picked.
- Lock: `view.locked` only (session); locked bodies are drawn but not pickable.
- Color: a popover (`data-testid="color-popover"`) with swatch buttons named Default (removes `color`), White `#f4f4f5`, Black `#27272a`, Gray `#9ca3af`, Red `#dc2626`, Orange `#ea580c`, Yellow `#eab308`, Green `#16a34a`, Teal `#0d9488`, Blue `#2563eb`, Purple `#7c3aed`, Pink `#db2777`, Brown `#92400e`, and a hex field accepting `#rrggbb`. Choosing commits `update_body` with the color (label "Change color").
- Keyboard: WAI-ARIA tree pattern (Up, Down, Right expands or enters, Left collapses or goes to the parent, Home, End), no type-ahead (letters stay global shortcuts). Row buttons are not tab stops; their actions are V (visibility), the selection inspector (color, material, visibility) and the palette commands "Lock selection" and "Unlock selection".

### 3.11 Parameters panel

The side panel `parameters` (`data-testid="parameters-panel"`, title "Parameters", close button) opens from the browser's "Edit parameters" button, a parameter row, or the palette command "Parameters".

- One row per parameter (`data-testid="parameter-row"`, `data-id`): name field ("Name of {name}"), expression field (`ExpressionInput` with unit `any`, "Expression of {name}"), value (`data-testid="parameter-value"`: the `recomputed` value formatted by its dimension, or the error message in `danger-fg` with a `circle-x` icon), comment field ("Comment of {name}"), delete button ("Delete {name}").
- `ExpressionInput` with unit `any` accepts every dimension and shows `= {value}` formatted by the returned dimension: length and angle as in 2.13, dimensionless as a scalar, any other dimension as `{value} ({dimension})` in internal units.
- Name: Enter or blur commits `update_parameter` (label "Rename parameter {old} to {new}") when changed. Locally, a name that does not match `^[A-Za-z_][A-Za-z0-9_]*$` or is longer than 64 characters shows "Names start with a letter or _ and use only letters, digits and _." and is not sent; other rules (reserved names, uniqueness) come back as the core's error message under the field. Esc reverts.
- Expression: Enter or blur commits `update_parameter` (label "Change {name}") when valid and changed; Esc reverts.
- Comment: Enter or blur commits (label "Change comment of {name}"); empty removes the comment.
- "Add parameter" appends a draft row whose name field is focused; Enter in the name moves to the expression (default text empty); Enter in a valid expression commits `add_parameter` at the end (label "Add parameter {name}"); Esc discards the draft.
- Delete commits `remove_parameter` (label "Delete parameter {name}"). Expressions that used it then fail; the failure notification offers Undo.

### 3.12 Viewport rendering

- The canvas fills the viewport region; its drawing buffer is the CSS size times `min(devicePixelRatio, 2)`. Rendering is on demand: a frame is drawn only after a change to the camera, scene data, highlights, theme, size, section or analysis, batched with `requestAnimationFrame`. Nothing renders while idle.
- Shading is computed in linear space and written as sRGB.

Passes, in order:

1. Background: vertical gradient from `viewport-top` to `viewport-bottom`.
2. Opaque bodies (model layer, and preview layer during a command): matcap-style shading from the view-space normal `n`: `k = 0.32 + 0.50 × max(dot(n, L), 0) + 0.18 × max(n.z, 0)` with `L = normalize(0.3, 0.6, 0.74)`, color `base × k + 0.06 × (1 - max(n.z, 0))^3` (rim). `base` is `Body.color` or `body`. Element states mix the result toward `accent` (hover 0.20, selected 0.35) or `danger` (problem 0.5). Faces use a polygon offset (factor 1, units 1). With a section, back faces are drawn flat in `base × 0.55` (the cut cap). Bodies whose role is not `part` draw at 40 percent opacity in the translucent pass.
3. Overhang map (Print context, 3.17): for a vertex value `v` (radians from vertical) above the limit `a`, `t = clamp((v - a) / (π/2 - a), 0, 1)` and the color becomes `mix(shaded, mix(analysis-low, analysis-high, t) × k, 0.85)`.
4. Edges: B-rep edges from `edge_positions` as screen-space quads (instanced per segment), 1.25 CSS px wide (times the UI scale), anti-aliased with a 1 px smoothstep falloff, depth-tested against the faces, in `#18181b` or `#e4e4e7` per body (2.16). Selected and hovered edges as in 3.6.
5. Vertices: only hovered and selected vertices are drawn.
6. Translucent: datum and origin planes as squares with `datum` fill at 15 percent and outline at 60 percent; non-part bodies.
7. Overlay (no depth test): sketch curves, points and region fills (3.16.2); in Model context visible sketches draw in `sketch-reference` at 1.25 px with no region fill, except hovered or selected regions (`accent` at 20 or 35 percent).
8. Print bed outline (depth-tested, 3.17).

- Datum and origin planes are drawn as squares of side `max(50 mm, 1.2 × the diagonal of the visible bounds)` centered at the projection of the visible bounds center onto the plane. Origin axes are drawn as dashed lines of the same length through the origin in `sketch-reference`, with DOM labels "X", "Y", "Z" at their positive ends.
- The DOM overlay (absolutely positioned over the canvas, `pointer-events: none` except on interactive children) holds sketch dimension labels, constraint glyphs, draw inputs, inference glyphs, axis labels, finding markers and the box selection rectangle. Overlay positions update in the same animation frame as the canvas.
- WebGL context loss: `preventDefault` on `webglcontextlost`, a status overlay "Restoring the 3D view" (`role="status"`), and on restore `scene.restore()` re-uploads every drawable and renders.

### 3.13 Navigation

| Action | Fusion preset (default) | FreeCAD preset |
| --- | --- | --- |
| Select | Left click | Left click |
| Pan | Middle drag | Middle drag |
| Orbit | Shift + middle drag | Middle + left drag, or middle + right drag (press middle first) |
| Zoom | Wheel at the cursor | Wheel at the cursor |
| Zoom (trackpad pinch: wheel with `ctrlKey`) | At the cursor | At the cursor |
| Fit to view | Middle double-click | none |
| Orbit with the view cube | Left drag on the cube | Left drag on the cube |
| Box selection | Left drag on empty space, sketch mode only | same |

- A gesture starts after 3 px of motion; below that, a press and release is a click. Orbit rate 0.4 degrees per px about the pivot: the world point under the cursor at the gesture start (from the ID pass depth) or the target when nothing is there or the backend cannot read depth.
- Wheel uses `wheelZoomFactor`; one notch of 100 px zooms out by 1.0015^100 ≈ 1.162 (wheel back) or in by the inverse (wheel forward). Wheel listeners are non-passive and call `preventDefault`.
- Middle-button `pointerdown` and `mousedown` call `preventDefault` (no browser autoscroll). The canvas `contextmenu` event calls `preventDefault` and opens nothing in M1.
- During a gesture hover picking pauses; the ID pass is re-rendered on the next pick after the camera has been still for 100 ms.
- The viewport is focusable (`tabindex="0"`, `role="application"`, `aria-roledescription="3D view"`) and takes the keys of 3.5.
- The navigation preset is a setting (`settings.navigation`), mirrored on the viewport as `data-nav-preset` (`fusion` or `freecad`).
- The viewport mirrors the camera in `data-view` (`classifyView`), `data-projection`, and `data-camera-target` (`"{x},{y},{z}"` with each coordinate rounded to 2 decimals, trailing zeros trimmed, `-0` as `0`).

### 3.14 View cube and viewport controls

- The view cube (`data-testid="view-cube"`, `role="group"`, `aria-label="View cube"`) is a DOM element with CSS 3D transforms, 96 px (times the UI scale), 16 px from the viewport's top-right corner, rotated every frame to match the camera. Its 6 faces are buttons labeled "Top", "Bottom", "Front", "Back", "Left", "Right" with accessible names "{Face} view"; its 12 edges are buttons named "{Face} {face} edge view" (pairs in the order Front, Back, Left, Right, Top, Bottom: "Front top edge view", "Front left edge view", "Right bottom edge view", ...); its 8 corners are buttons named "{Front or Back} {Left or Right} {Top or Bottom} corner view".
- Clicking a face sets that standard view; an edge sets the view looking from the normalized sum of its two face directions; a corner from the sum of three (Front right top is `iso`). Up is the projection of +Z onto the view plane, except views looking straight down or up (top and bottom faces) which use the up vectors of 2.11. The target, distance and projection are kept; the change animates.
- Dragging on the cube orbits about the target at the same rate as the navigation orbit.
- A "Home view" button (`house` icon) at the cube's top-left corner sets `iso` and fits the visible bounds.
- Viewport controls (`data-testid="viewport-controls"`), a vertical stack of 32 px icon buttons under the cube: "Fit to view" (`maximize`), "Orthographic" (`cuboid`, `aria-pressed` true in orthographic), "Section view" (`scan-line`, `aria-pressed`; turning it on opens the section popover), "Selection filter" (`mouse-pointer-2`, a menu of `menuitemcheckbox` items Bodies, Faces, Edges, Vertices; hidden in sketch mode).
- Fit to view frames `scene.visibleBounds` with `fitBounds` (margin 0.1), animated; with nothing visible it frames a 100 mm cube at the origin.
- Look at (`view.look_at`): with one flat face (its `placement`), origin plane or datum plane selected, or in sketch mode (the sketch placement), the camera looks along the negated normal from the front side with the plane's y-axis up, target the plane point (face centroid, plane origin), distance and projection kept. Otherwise the command is disabled with "Select a flat face or a plane first."
- Orthographic toggle uses `withProjection`.

### 3.15 Section view

- Toggled by the "Section view" control or Shift+X. When turned on, the section popover (`data-testid="section-popover"`) opens anchored to the control: Plane (segmented XY, XZ, YZ; default XY), Offset (`ExpressionInput`, unit length, default the center of the visible bounds along the plane normal formatted with `formatNumber`), Flip (switch).
- The section removes the half-space where `dot(n, p) > offset`, with `n` the plane normal (XY +Z, XZ -Y, YZ +X; Flip negates both `n` and `offset`). It clips bodies, edges, vertices and the ID pass (removed parts cannot be picked). Sketches, datum and origin planes and the bed are not clipped.
- The section is a view setting: not saved, not undoable, kept across context changes until turned off. `data-section` on the viewport is `on` or `off`.

### 3.16 Sketch mode

#### 3.16.1 Entering

- `startNewSketch`: when the selection is exactly one plane-like item (origin plane, datum plane, or a face whose `surface` is `plane`), `createSketchOn(item)`. Otherwise the app enters plane picking: context Sketch with `sketchId` null, the sketch header reading "Pick a plane or a flat face", origin planes shown, and only origin planes, datum planes and faces accepted. A face that is not flat shows the hint "Pick a flat face, a datum plane or an origin plane." Clicking a browser origin plane or datum row works too. Esc returns to Model.
- `createSketchOn(item)`: commits one `apply_edits` labeled "New sketch" with `add_feature` of a `sketch` record whose `plane` is the `PlaneRef` for the item (`selectionValue('plane', ...)`), plus the fallback frame from the face's `ElementDescriptor.placement` for faces, in sketch.md's shape, with empty entities and constraints; inserted by the marker rule of 3.7 (with `set_rollback` when the marker is set). Then `enterSketch(new id)`.
- `enterSketch(id)`: cancels an active command, sets context Sketch, shows the sketch for the session, gets its solution (`sketch_solved` events or `solveSketch(doc, id)`), and moves the camera: `lookAlong` the placement normal with the placement y-axis up, then, when the sketch has entities, frames their bounds in the sketch plane (`fitBounds`, margin 0.1); an empty sketch keeps the visible height and targets the placement origin. The projection is kept. The camera before entering is pushed as a sketch view entry (3.20). Downstream features stay visible and recompute after each sketch edit.

#### 3.16.2 Display

- Active sketch curves: overlay lines 1.5 px (times the UI scale) colored by `SolvedEntity.constrained`: `under` `sketch-under`, `full` `sketch-full`, `conflicting` `sketch-conflict`. Construction entities are dashed (6 px dash, 4 px gap). Hovered and selected curves are 2.5 px in `accent`. Circles and arcs are tessellated by the UI from entity data with a chord tolerance of 0.25 CSS px, re-tessellated when the scale changes by more than a factor 1.5.
- Points: line ends and arc ends as 5 px squares and centers as 5 px crosses in their entity's color; point entities as 6 px filled circles; points shared by a coincident constraint as filled squares; hovered and selected points 7 px in `accent`.
- The origin point (9 px cross) and the X and Y axes (dashed lines across the view) in `sketch-reference`, pickable for snapping and constraints.
- Closed regions filled with `sketch-region` at 12 percent from the solution's display buffers; hovered regions at 20 percent.
- Profile issues: each issue with `at` gets a 12 px ring in `danger` (DOM, `data-testid="profile-issue"`, title "Open end", "Overlapping curves" or "Curves cross" for `open_loop`, `overlap`, `self_intersection`); the entities of every issue get a 4 px `danger` halo at 30 percent.
- Other visible sketches draw in `sketch-reference` and are not pickable in sketch mode.
- Constraint glyphs (when "Show constraints" is on, the default; palette command "Show constraints" toggles it): one DOM button per geometric constraint except `coincident` and `point_on_object` (which have no glyph; the shared point is drawn filled), 18 px (times the UI scale), `data-testid="constraint-glyph"`, `data-constraint-id`, `data-type`, `data-state` (`satisfied`, `redundant`, `conflicting`, `unsatisfied`), accessible name the constraint title. Placement: anchored at the midpoint of the first constrained curve (lines), the point at 45 degrees on circles and arcs, or the point itself, offset 14 px perpendicular to the curve on screen; further glyphs on the same anchor stack 20 px apart along the curve. Style: icon in `fg-muted` on a `surface-raised` pill at 90 percent opacity; `warning` when redundant, `danger` when conflicting. Hovering highlights the constrained items; clicking selects the constraint.
- Dimensions: dimension geometry (extension lines, dimension line with 8 px arrowheads, radius or diameter leader, angle arc) in `sketch-reference` at 1 px, plus a DOM label button (`data-testid="sketch-dimension"`, `data-constraint-id`, `data-type`, `data-driving`) at the label position (stored by sketch.md if it stores one, else `defaultLabelPosition`). Label text: `formatNumber` of the solved value without unit; prefixed `⌀` for diameter and `R` for radius; suffixed `°` for angles; a non-plain-number expression is prefixed `fx ` (`fx 40`); reference dimensions are wrapped in parentheses (`(40)`). Pill `surface-raised` at 90 percent with `fg` text; `danger-fg` when conflicting, `warning-fg` when redundant. Double-click or Enter opens the dimension editor.

#### 3.16.3 Tools and commits

- Tools are activated by the toolbar, shortcuts or the palette. Activating another tool cancels the current operation. The active tool's toolbar button has `aria-pressed="true"`. The cursor is a crosshair while a drawing tool is active.
- The pointer maps to sketch coordinates by intersecting the camera ray with the sketch plane; when the ray is within 1e-6 of parallel to the plane, pointer input is ignored.
- `commitSketchChange(label, change)`:
  1. Builds candidate params: current entities and constraints plus the change.
  2. Calls `solveSketch(doc, sketch, candidate)`. When a `requested` constraint id is in `conflicting` or `redundant`, returns `over_constrained` and applies nothing. When `inferred` ids are, removes them and solves once more.
  3. Calls `applyEdits(doc, label, [update_feature])` with the candidate params whose entities carry the solved geometry (each `SolvedEntity` without `constrained`).
  4. A `failed` status, or a `sketch_failed` error, returns `failed` with the solver's message; nothing is applied.
- Labels: "Line", "Rectangle", "Circle", "Arc", "Point", the constraint title, "Dimension", "Change dimension", "Construction", "Delete". Drags are committed by `end_sketch_drag` (the core labels them).

#### 3.16.4 Drawing tools

Each tool's output is fixed (unit tests U-50 to U-54 check `recipes.ts`); "inferred" constraints come from 3.16.5; everything else is requested.

- **Line / polyline** (`line`, L). Click 1 sets the start S. A rubber band follows the cursor. Click 2 sets the end E and commits a line S to E; segments shorter than 2 CSS px are ignored. The chain continues from E: the next line starts at E with an inferred `coincident` between the previous line's end and the new line's start. The chain ends on Esc (the rubber band is discarded, the tool stays), Enter with no draw input focused, a double-click (which commits the segment to the clicked point first), or a click on the chain's first point (committing a closing segment with `coincident` to it).
- **2-point rectangle** (`rectangle_2point`, R). Click 1 sets corner C1, click 2 the opposite corner C2; a width or height under 2 CSS px is ignored. Output: lines L1 C1 to (C2.x, C1.y), L2 (C2.x, C1.y) to C2, L3 C2 to (C1.x, C2.y), L4 (C1.x, C2.y) to C1; `coincident` L1.end-L2.start, L2.end-L3.start, L3.end-L4.start, L4.end-L1.start; `horizontal` L1 and L3; `vertical` L2 and L4. Snaps at C1 apply to L1.start and at C2 to L2.end (inferred).
- **Center rectangle** (`rectangle_center`). Click 1 sets the center P, click 2 a corner K. Output: the 2-point rectangle with C1 = 2P - K and C2 = K, plus a construction line D from L1.start to L2.end with `coincident` D.start-L1.start and D.end-L2.end, and a construction point Q at P with `midpoint` (Q, D). Snaps at P apply to Q and at K to L2.end (inferred).
- **Circle** (`circle`, C). Click 1 sets the center, click 2 a point on the circle (radius under 2 CSS px is ignored). Output: one circle. A snap at the center applies to its center point; a point snap at click 2 adds `point_on_object` (that point, the circle) (inferred).
- **3-point arc** (`arc_3point`). Click 1 sets A, click 2 sets B, then the arc follows the cursor T through A, B and T; click 3 commits. When `circleThroughThreePoints` returns null or a radius above 1e6 mm, the rubber band is a straight segment, click 3 is ignored, and the sketch hint (`data-testid="sketch-hint"`, `role="status"`) says "These three points are in a line. Move the last point off the line to make an arc." Arcs are stored counterclockwise: when T lies on the clockwise path from A to B, the arc runs from B to A. Snaps at A and B apply to the arc's ends; a tangent inference at A applies.
- **Point** (`point`). Each click adds a point entity at the (snapped) location.
- **Construction** (`construction`, X). With entities selected and no drawing tool active, toggles their `construction` flag in one commit labeled "Construction". With a drawing tool active, toggles construction mode for new entities (`aria-pressed` on the button). The mode resets when sketch mode ends.

Dimension as you draw (draw inputs: DOM text inputs next to the rubber band's midpoint, `data-testid="draw-input"`, accessible names as listed):

- Line: "Length" (unit length) and "Angle" (unit angle, counterclockwise from the sketch +X axis). Rectangles: "Width" (along X) and "Height" (along Y). Circle: "Diameter". Arc, after click 2: "Radius".
- While a shape is rubber-banding, typing a digit, `.`, `-` or `(` focuses the first draw input and inserts the character; Tab focuses the first input, then cycles between the inputs. A valid typed value locks that quantity (the rubber band follows it; the other quantity still follows the cursor). Esc in a draw input clears the inputs and returns focus to the canvas.
- Enter commits the shape with the typed values: the end point is computed from them (line: `S + L(cos a, sin a)`; rectangles: the corner sign from the cursor's quadrant; circle: diameter; arc: radius on the cursor's side), and each typed value adds a driving dimension with the typed expression: Length a `distance` on the line; Angle: `horizontal` when the angle is a multiple of 180 degrees, `vertical` for 90 plus a multiple of 180, otherwise an `angle` between the line and the sketch X axis (`X_AXIS`); Width a `distance` on L1; Height a `distance` on L4; Diameter a `diameter`; Radius a `radius`. Without a typed angle, direction inference applies as usual.

#### 3.16.5 Inference while drawing

- Snap targets within 8 CSS px (times the UI scale) of the cursor: every solved point of the active sketch and the origin point (`point`), the midpoints of lines (`midpoint`), and, within 6 px, curves including the origin axes (`curve`, at the closest point). Priority point, then midpoint, then curve; nearest first within a class.
- Direction inference for line segments with a start S: `horizontal` when the segment's angle to the sketch X axis is within 3 degrees modulo 180, `vertical` likewise to the Y axis; `tangent` when S is the end of an arc (or, for a 3-point arc, A is the end of a line or arc) and the directions differ by at most 5 degrees. When tangent applies, horizontal and vertical do not. Without a snap at the end, a horizontal or vertical inference moves the end exactly onto the horizontal or vertical through S.
- Constraints from snaps (all inferred): point: `coincident` (new point, target point; the origin point is `ORIGIN_POINT`); midpoint: `midpoint` (new point, line); curve: `point_on_object` (new point, curve; the axes are `X_AXIS` and `Y_AXIS`). Direction: `horizontal`, `vertical` or `tangent` on the new curve.
- While inferring, an inference glyph (`data-testid="inference-glyph"`, `data-type` the inferred constraint type) shows 16 px to the right and below the cursor and the snap target is highlighted in `accent`.
- Suppression: while Ctrl (Windows and Linux) or Cmd (macOS) is held, there are no snaps, no direction inference and no glyphs; the raw cursor is used. The key state is read from each pointer event, including the click.

#### 3.16.6 Constraint tools

Commands `sketch.constrain_<type>` in this order: Coincident, Point on object, Horizontal, Vertical, Parallel, Perpendicular, Tangent, Collinear, Concentric, Equal, Symmetric, Midpoint, Fix.

- If the sketch selection is a valid operand set for the constraint, it is applied at once (`commitSketchChange` with the new id requested) and the selection is cleared. Otherwise the tool enters pick mode: the sketch hint shows `constraintRequirement(type)`, clicks add accepted items to the tool's pick list (refused items flash the hint), and when the list forms a valid set the constraint is applied and the list clears. The tool stays active until Esc or another tool.
- Operands (`applicableConstraints` returns every constraint whose operand rule the selection satisfies exactly; axes count as lines; the origin counts as a point):

| Constraint | Operands | Requirement text |
| --- | --- | --- |
| Coincident | two points; a point and a curve gives Point on object | "Select two points, or a point and a curve." |
| Point on object | a point and a curve | "Select a point and a curve." |
| Horizontal | one line, or two points | "Select a line or two points." |
| Vertical | one line, or two points | "Select a line or two points." |
| Parallel | two lines | "Select two lines." |
| Perpendicular | two lines | "Select two lines." |
| Tangent | a line and a circle or arc, or two circles or arcs | "Select a line and a circle or arc, or two circles or arcs." |
| Collinear | two lines | "Select two lines." |
| Concentric | two circles or arcs | "Select two circles or arcs." |
| Equal | two lines, or two circles or arcs | "Select two lines, or two circles or arcs." |
| Symmetric | two points and a line (the axis) | "Select two points and a line." |
| Midpoint | a point and a line | "Select a point and a line." |
| Fix | one or more points or curves | "Select points or curves." |

- Over-constrained (requested id conflicting): a warning notification "{Title} would over-constrain the sketch: it conflicts with {list}." where the list names the other conflicting constraints by title (dimensions as "Distance 40"), joined with ", " and " and "; actions "Replace them" (one commit labeled with the title that removes those constraints and adds the new one) and "Cancel". Redundant: an info notification "{Title} is already implied by other constraints, so it was not added."

#### 3.16.7 Dimension tool

- `sketch.dimension` (D). Click a curve or point; for a circle or arc the label follows the cursor and the next click places it. For a line, the next click on empty space places its length dimension; a click on a point or line instead adds the second operand, then the next click places the label. For a point, the next click must be a point or a line.
- `dimensionTypeFor`: two points (a line's ends count) not axis-aligned (both coordinate differences above 1e-9 mm): label x within the points' x range and y outside the y range gives `horizontal_distance`; y within and x outside gives `vertical_distance`; otherwise `distance`. Axis-aligned pairs give `distance`. A point and a line: `distance`. Two parallel lines (within 1e-6 rad): `distance`. Two other lines: `angle`, measuring the sector between them that contains the label. A circle: `diameter`. An arc: `radius`. Anything else: unsupported, with "Partsmith cannot dimension {a} and {b}." (`a point and a circle`).
- While placing, the dimension draws with its measured value. The placing click opens the dimension editor (`data-testid="dimension-editor"`): a text input "Dimension value" prefilled with `formatNumber(measuredValue)` (selected), a toggle button "Reference" (`aria-pressed`), and for circles and arcs a segmented "Diameter | Radius". Enter commits a dimension with the text as its expression and `driving` true unless Reference is pressed; the text is validated live as an expression of the dimension's unit (length or angle) and an invalid text keeps the editor open with the message.
- When the probe finds the new driving dimension redundant or conflicting, the editor stays open and shows, with `role="alert"`, "This dimension would over-constrain the sketch. Add it as a reference dimension that only measures?" and the buttons "Add reference" (commits it with `driving` false) and "Cancel".
- The tool stays active after a commit. Esc closes the editor without adding anything.
- Editing an existing dimension (double-click its label, Enter on the focused label, or the selection inspector) opens the editor prefilled with the stored expression text; Enter commits a replacement labeled "Change dimension". When the probe reports status `over_constrained` or `failed`, the editor keeps the text and shows "The sketch cannot be solved with this value. Check for dimensions that cannot all be true at once."

#### 3.16.8 DOF display

- Entity colors come from `SolvedEntity.constrained` (3.16.2).
- The sketch header (`data-testid="sketch-header"`, a button at the viewport's top center, `data-state`, `data-dof`) shows the sketch label and a status: `empty` "No geometry yet"; `under` "{n} DOF"; `full` "Fully constrained"; `conflict` "{n} conflicting constraints" in `danger-fg`; `failed` "Cannot solve" in `danger-fg`. When the solution has redundant constraints, " · {n} redundant" follows in `warning-fg`. During plane picking it shows "Pick a plane or a flat face" with `data-state="picking"`.
- Clicking the header opens a popover (`data-testid="sketch-problems"`) listing conflicting and redundant constraints (title and a "Delete" button each) and profile issues (text and a "Show" button that frames the issue).
- The status bar shows `status-dof` (3.21).

#### 3.16.9 Drag and box selection

- With no tool active, pressing on a point of the active sketch and moving more than 3 px drags it. Points only in M1; pressing on a curve starts nothing.
- Each pointer move maps the cursor to sketch coordinates (no inference) and submits `dragSketch(doc, sketch, pointRef, to)` through a `LatestOnly`. Each non-null solution redraws the sketch in the next frame (`ps:sketch-drag`).
- Release: when at least one drag request was sent, `endSketchDrag(doc, sketch, true)` through the mutation queue (one undo entry); otherwise nothing. Esc during the drag: `endSketchDrag(doc, sketch, false)` and the stored geometry is redrawn.
- Over a point whose entity is `full`, the cursor is `not-allowed`.
- Pressing on empty space and moving more than 3 px draws a selection rectangle (`data-testid="box-selection"`). Dragging right (end x at or above start x) is a window selection: entities whose every point and tessellation vertex lie inside. Dragging left is a crossing selection (dashed border): entities with any part inside. Shift or Mod toggles the result into the sketch selection; otherwise it replaces it.

#### 3.16.10 Sketch selection and deletion

- Clicks select one entity, point, glyph or dimension label (Shift or Mod toggles; a click on empty space clears). `Mod+A` selects every entity.
- Delete removes the selected entities and constraints, plus every constraint that references a removed entity or one of its points, in one commit labeled "Delete".

#### 3.16.11 Finishing

- Finish sketch (toolbar, `Mod+Enter`), the Model or Print radio, or a feature command ends sketch mode: the context becomes Model (or the chosen one), the sketch session clears, the camera stays, and the sketch stays visible for the session.
- When the solution has profile issues, an info notification "{label} has {n} profile problems, so some regions may not extrude." offers "Back to sketch".

### 3.17 Print context

- Printer: the toolbar menu button "Printer: {profile name}" ("Printer: none" without one) opens `data-testid="printer-menu"` (`role="menu"`): radio items for every profile from `listPrinterProfiles`, under the headings "FDM" and "Resin" by technology, then "No printer". Choosing one commits `set_printer` (label "Change printer"). The status bar printer button opens the same menu.
- Overhangs: a toggle (`aria-pressed`, default on, session state) that shows the overhang color map and legend.
- Checks: a popover (`data-testid="checks-popover"`) with one switch per check in this order and wording: Overhangs (`overhangs`), Bed fit (`bed_fit`), First layer contact (`first_layer_contact`), Thin walls (`thin_walls`), Small features (`small_features`), Islands (`islands`), Trapped volumes (`trapped_volumes`), Mesh validity (`mesh_validity`, always on and disabled, help "Always checked before export"). The core runs every check (protocol.md `analyze_print`); a switched-off check only hides its findings from the badge counts, the list and, for Overhangs, the color map. The choice persists in `settings.hiddenChecks`.
- Health: the toolbar button "Health" (with the shown finding count) opens the side panel `health`.

Analysis runs:

- In Print context, 300 ms after the document revision last changed with no transaction open, and on entering Print when the report is stale: `analyzePrint(doc, { orientation: { kind: 'as_modeled' } })` through a `LatestOnly` (a `null` superseded result is ignored). `print.running` is true meanwhile.
- Outside Print, analysis never runs on its own. The badge shows the last report as stale; clicking the badge runs the analysis and opens the health panel.

Health badge (`data-testid="health-badge"`, a button in the status bar, `data-state`, accessible name as listed):

| State | When | Icon and text | Accessible name |
| --- | --- | --- | --- |
| `none` | No report yet | `circle-dashed`, "Check print" | "Print health: not checked" |
| `running` | Analysis in flight | `loader-circle` (spinning unless reduced motion), "Checking" | "Print health: checking" |
| `ok` | No shown findings | `circle-check` in `success`, "Ready to print" | "Print health: no problems" |
| `warning` | Shown warnings, no shown errors | `triangle-alert` in `warning`, "1 warning" or "{n} warnings" | "Print health: {n} warning(s)" |
| `error` | Shown errors | `circle-x` in `danger`, "1 error" or "{n} errors" | "Print health: {e} error(s), {w} warning(s)" |

A report whose revision differs from the document revision is stale: the badge keeps its state, draws at 50 percent opacity, sets `data-stale="true"`, appends " (out of date)" to its text and ", out of date" to its name. Clicking the badge in any state runs the analysis when stale or absent, and opens the health panel.

Health panel (side panel `health`, `data-testid="health-list"`):

- Header: the profile name and a "Check again" button.
- Rows (`data-testid="health-item"`, `data-check`, `data-severity`): severity icon, the finding's message verbatim, the body name in `fg-muted`. Sorted by severity (errors first), then by the check order above, then report order. Hidden checks are left out.
- Clicking a row or pressing Enter on it focuses the finding: with a `location`, the camera frames a cube of side `max(20 mm, 0.25 × the body's bounding-box diagonal)` centered there (`zoomToBounds`, margin 0.3) and a 16 px `danger` ring marker (`data-testid="finding-marker"`) is drawn at the location; without one, the body's bounding box is framed. The marker stays until another row is chosen, the panel closes or the document changes.
- Empty: "No problems found with {profile name}." With no body that has geometry: "Add a body to check it for printing."

Overhang map and legend:

- Shown in Print context while Overhangs is on and the overhangs check is not hidden. For each `BodyPrint` whose `mesh_id` equals the body's current model-layer mesh id, its overhang buffer is set as the body's scalar field (3.12 pass 3); a body whose mesh changed since the report shows no map until the next report.
- The limit `a` is the `limit` of the report's `overhangs` findings when present, else the profile's maximum overhang angle (print.md).
- Legend (`data-testid="overhang-legend"`, bottom-left of the viewport): title "Overhangs", a gradient from `analysis-low` to `analysis-high`, labels "{a}°" and "90°" (degrees with `formatNumber`), and the caption "Faces steeper than {a}° from vertical need support."

Bed outline: in Print context, after a report, the bed rectangle of the report's profile (print.md bed size) at z = 0 in bed coordinates, mapped into model coordinates by the inverse of the first `BodyPrint.transform`, drawn 1.5 px in `bed`, depth-tested.

### 3.18 Export

- The top-bar Export button, `Mod+Shift+E` and the Document menu open the export popover (`data-testid="export-popover"`): Format (radio group 3MF, STL, OBJ, STEP; default 3MF), Bodies (radio group "All visible bodies", "Selected bodies"; the second is disabled unless bodies are selected), and an "Export" button.
- The Print toolbar's "Export 3MF" exports 3MF of all visible bodies directly; its Export group exports STL, OBJ or STEP of all visible bodies directly.
- `exportModel(request)`: the bodies are the visible bodies with model-layer geometry, or the selected bodies. With none, the error notification "There is no visible body with geometry to export." appears and the core is not called. Otherwise `exportBodies(doc, format, bodies)` with `{"format": "3mf"}`, `{"format": "stl", "binary": true, "per_body": false}`, `{"format": "obj", "per_body": false}` or `{"format": "step"}`.
- The single returned file is saved with `files.chooseSaveTarget('{document name}.{3mf|stl|obj|step}', kind)` and `files.write`. A cancelled picker does nothing. Success shows the info notification "Exported {file name}." Each warning diagnostic shows a warning notification with its message. A `CoreError` shows its message as an error notification.

### 3.19 Files

- Document name: the file name without the `.ptsm` extension (case-insensitive); a new document is "Untitled".
- Replacing a dirty document (New, Open, Open recent, a recovery Open): before closing it, the UI writes a recovery entry (`reason: 'replaced'`, text `JSON.stringify(recipe, null, 2)`, its file handle) and, after the new document is open, shows the info notification "{name} had unsaved changes. They are kept for you." with the action "Reopen" (opens the entry as in the recovery flow below).
- New (`file.new`, Document menu): closes the current document (`closeDocument`), opens `newDocument('mm')` named "Untitled", clears the selection, command and sketch session, switches to Model and resets the camera to `DEFAULT_CAMERA`.
- Open (`Mod+O`, Document menu, palette): `files.pickToOpen()`, then `openFile`. Dropping a file on the window shows the overlay `data-testid="drop-overlay"` ("Drop a .ptsm file to open it") while dragging and opens the first file on drop.
- `openFile(file)`:
  1. A name not ending in `.ptsm` (case-insensitive) shows the error "{file} is not a Partsmith model. Partsmith opens .ptsm files." and stops.
  2. The bytes decode as UTF-8 with `fatal: true`; failure shows "{file} could not be read as text." and stops.
  3. `openDocument(text)`. A `CoreError` shows "Could not open {file}. {core message}" and the current document stays.
  4. On success: stash the old document if dirty (above), close it, adopt the new handle and name, remember the file in recent files (handle, or text when there is no handle), switch to Model, set the camera to `iso` and fit the visible bounds, mark `ps:open`. With `migratedFrom`, the info notification "{file} was updated from format version {n}. Saving writes the current format."
- Save (`Mod+S`): with a file handle, `saveDocument(doc)` and `files.write({ kind: 'handle', ... }, 'ptsm', utf8)`; without one, Save as. Success sets `localUnsaved` false and clears the recovery store. A `FileServiceError` sets `localUnsaved` true and shows: permission: "Could not save {file} because Partsmith no longer has permission to write it." (action "Save as"); not_found: "{file} was moved or deleted. Save it again to choose a location." (action "Save as"); io: "Could not save {file}: {reason}" (action "Save as").
- Save as (`Mod+Shift+S`): `files.chooseSaveTarget('{name}.ptsm', 'ptsm')` first (a cancel stops with nothing changed), then `saveDocument(doc)` and `files.write`. A handle target becomes the document's file (name from the handle) and is remembered in recent files; a download target clears the handle, and the first download in a session shows the info notification "This browser saves by downloading. {file name} is in your downloads folder."
- Open recent (Document menu submenu, palette items): up to 10 entries, newest first, each showing the name and its open time (`toLocaleString('en-US', { dateStyle: 'medium', timeStyle: 'short' })`). An entry with a handle reads it (`readHandle`, which may prompt for permission on this click): permission denied shows "Partsmith needs permission to open {file}. Choose it again to grant access." (action "Open..."); not found shows "{file} has moved or been deleted, so it was removed from recent files." and forgets it. An entry with text opens that text (no handle; Save goes to Save as).
- Recovery copy: while the document is dirty, 2 s after the last `document_changed` (debounced), the UI writes a recovery entry (`reason: 'autosave'`, `JSON.stringify(recipe, null, 2)`, the handle). A successful save, a New without changes, or Discard clears it. `beforeunload` calls `preventDefault()` while the document is dirty so the browser asks before leaving.
- At startup a recovery entry shows the info notification (persistent until acted on) "Partsmith kept unsaved changes to {name} from {time}." with actions "Open" (opens the text as a document named {name}, attaches the stored handle, sets `localUnsaved` true) and "Discard" (clears the store).
- The desktop host replaces `FileService` with native dialogs (open issue 9); every behavior above stays.

### 3.20 Undo and redo

- `edit.undo` and `edit.redo` call the core's `undo` and `redo` through the mutation queue. Their palette details are "Undo {undoLabel}" and "Redo {redoLabel}"; with nothing to undo or redo they are disabled with "Nothing to undo." or "Nothing to redo." After either, a polite live region announces "Undid {label}" or "Redid {label}".
- With a feature command active, `Mod+Z` cancels the command and nothing else; `Mod+Shift+Z` is disabled with "Finish or cancel {title} first."
- In sketch mode with an operation in progress (rubber band, drag, box selection, open editor), `Mod+Z` cancels that operation only.
- Sketch view history (technical design 13, principle 6): during a sketch session the UI keeps a stack of entries, each either `view` (a camera) or `document` (a marker for a committed sketch edit). Entering the sketch pushes the camera from before entering; each completed orbit or pan gesture, each wheel burst (ended by 300 ms without wheel events), view cube click, fit, home and look at pushes the camera from before it; each committed sketch edit pushes a `document` marker. In sketch mode, `Mod+Z` pops the top entry: a `view` entry restores its camera (animated) and moves to a redo stack with the camera it replaced; a `document` entry calls the core's `undo`. With the stack empty, `Mod+Z` calls the core's `undo` directly. `Mod+Shift+Z` mirrors this with the redo stack. A new view change or edit clears the redo stack. Leaving sketch mode discards both stacks.

### 3.21 Status bar

Left to right (`data-testid` in parentheses):

- Core status (`status-core`), shown only when not idle: "Loading core" with a spinner, "Rebuilding" with a spinner when a mutating request has been pending for more than 300 ms, or "Core stopped" in `danger-fg`.
- Sketch DOF (`status-dof`, sketch mode with a sketch only): "{n} DOF", with `data-dof` and `data-state` as the sketch header.
- Rebuild time (`status-recompute`): "Rebuilt in {n} ms" (integer) or "Rebuilt in {s} s" (one decimal) for durations of 1000 ms or more, measured from sending the last mutating request to receiving its response; tooltip "Time the last change took to rebuild".
- Units (`status-units`): a button with the unit ("mm") opening a menu of radio items mm, cm, m, in. Choosing one commits `set_units` (label "Change units to {unit}") and shows the info notification "Units are now {unit}. Numbers typed without a unit now mean {unit}." (model.md 3.12).
- Printer (`status-printer`): a button with the profile name or "No printer" opening the printer menu.
- Print health badge (`health-badge`, 3.17).

### 3.22 Notifications

- Stack at the viewport's bottom-right, 16 px from its edges, 360 px wide, newest at the bottom, at most 3 shown. A fourth removes the oldest non-error notification (or the oldest when all are errors).
- Each (`data-testid="notification"`, `data-severity`) has a severity icon (`info`, `circle-check`, `triangle-alert`, `circle-x`), the message, action buttons, and a "Dismiss" button. Running an action dismisses the notification.
- Info and success dismiss after 5 s, warnings after 8 s; errors stay until dismissed. Timers pause while the pointer is over the notification or focus is inside it.
- Errors are announced through a `role="alert"` region; the others through a `role="status"` region.
- A notification with the same message as a visible one does not appear again; the visible one restarts its timer.

### 3.23 Visual design

- Neutral gray chrome from the zinc-based tokens, one accent (violet), generous spacing. Color appears only with meaning: `accent` for focus, selection, active tools and the primary action; `danger`, `warning` and `success` for status; sketch DOF colors; the analysis map.
- Type: the Tailwind sans stack (system UI fonts, no web fonts). Body text 0.875rem (14 px), secondary text and captions 0.75rem (12 px, the minimum anywhere), panel titles 1rem semibold. Numbers in inputs, labels and the status bar use tabular figures.
- Spacing on a 4 px grid: panel padding 16 px, rows and controls 32 px high, toolbar buttons 48 by 40 px with labels and 36 by 36 px icon-only, gaps of 8, 12 and 16 px.
- Corner radius 6 px for controls and 10 px for panels and popovers. Floating panels cast `--ps-shadow`: `0 8px 24px rgb(0 0 0 / 0.12)` (light) and `0 8px 24px rgb(0 0 0 / 0.5)` (dark).
- Icons: lucide-react line icons, stroke width 1.75, 20 px in toolbars and 16 px in lists and fields, in `currentColor` (`fg` or `fg-muted`).
- Motion: popovers and panels fade and slide 4 px over 150 ms with ease-out; camera moves 250 ms; none under `prefers-reduced-motion`.
- Focus: a 2 px `accent` outline with a 2 px offset on `:focus-visible`, on every interactive element.

### 3.24 Accessibility

- Contrast: the tokens in 2.16 meet WCAG 2.2 AA (text 4.5:1, large text and non-text 3:1) in both themes; unit test U-70 enforces the listed pairs. Text drawn over the viewport always sits on a `surface-raised` pill.
- Keyboard: every function is reachable without a pointer except picking faces, edges and vertices in the viewport, drawing sketch geometry, dragging sketch points and dragging the inspector (later). Selection fields for bodies, sketches, planes, axes and regions offer the "Choose" menu.
- Tab order: top bar (document name, context radios, toolbar, palette button, Export), browser (one stop), viewport (one stop), the inspector when open, timeline (one stop), side panel, status bar buttons, notifications. Opening the inspector moves focus into it; closing it returns focus.
- Roving tab index with arrow keys in toolbars, segmented controls, the timeline, the tree and menus. Menus close on Esc and return focus to their button.
- Tooltips appear after 500 ms of hover and immediately on keyboard focus, close on Esc, are referenced by `aria-describedby`, and show the command title and formatted shortcut.
- Every interactive target is at least 24 by 24 CSS px. Every icon-only button has an accessible name.
- Color is never the only signal: status has icons and text, DOF has the header text, selection has thicker strokes, the analysis map has a legend.
- At 200 percent UI scale on a 1280 by 720 CSS px window every region stays usable (toolbars go icon-only, panels scroll).
- Live regions: selection, undo and redo announcements, notifications, sketch hints.

## 4. Errors

### 4.1 Message rules

Every message the user sees follows the architecture's error rule:

- Plain sentences a maker understands, starting with what happened, naming the cause in the user's terms (feature labels, body names, file names, values with units), and naming the fix when one exists.
- No "Error:" prefix, no exclamation marks, no blame, no stack traces, no error codes in the visible text (codes go to `console` and `data-code` on the notification).
- At most two sentences plus action buttons. Action labels start with a verb ("Save as", "Undo", "Edit", "Replace them").
- Core messages (`CoreError.message`, `Diagnostic.message`, finding messages) are shown verbatim; the core already follows these rules (features.md section 4, protocol.md section 4). The UI only wraps them in the templates of 4.3.
- File names are quoted as given, without quotation marks; feature and body names appear in single quotes only where the core puts them.

### 4.2 Where errors appear and the fixes they offer

| Error | Where | Fix actions |
| --- | --- | --- |
| Invalid expression in a field | Under the field (`field-error`), field border `danger`; OK disabled | Edit the field |
| Invalid sketch dimension value | In the dimension editor | Edit the value, "Add reference" when over-constraining |
| Feature status during a command (preview) | Inspector banner (`inspector-error`) | Errors and warnings about a field the message names: the field is highlighted and focused by clicking the message |
| Feature status after a user change: a feature goes from not-error to error | One notification: for one feature "{label} failed. {message}" with "Edit" and "Undo"; for several "{n} features failed after this change." with "Show problems" and "Undo" | As listed |
| Feature warnings and errors at rest | Timeline and browser badges, tooltips, Problems panel, selection inspector | "Edit" opens the inspector, where repairs re-pick the reference (the field named by `Repair.path` becomes the active selection field) |
| Sketch over-constraint while adding | Notification (3.16.6) or dimension editor (3.16.7) | "Replace them", "Add reference", "Cancel" |
| Sketch conflicts and redundancy at rest | Sketch header, glyph and label colors, sketch problems popover | "Delete" per constraint |
| Profile issues | Rings and halos in sketch mode; notification on finishing | "Back to sketch" |
| Command failed in the core (`CoreError`) | Error notification with the core message | "Undo" when the error followed an edit that was applied; otherwise none |
| `focus_inactive` when editing | Error notification with the core message | "Unsuppress" or "Move marker to the end" |
| File, export and permission errors | Error notification (4.3) | "Save as", "Open..." |
| Core fatal | Fatal state (4.4) | "Restart core", "Reload" |
| Unsupported browser | Unsupported page | none |

`Diagnostic` has no structured fix value (protocol.md `Diagnostic` is severity, code and message), so M1 fixes are navigational ("Edit", "Undo") rather than one-click value changes; see open issue 4.

### 4.3 UI-owned messages

`src/messages.ts` exports each of these as a function of its placeholders; tests compare exact text.

| Id | Text | Severity | Actions |
| --- | --- | --- | --- |
| `unsupported_webgl2` | Partsmith needs WebGL2 to draw the 3D view, and this browser does not provide it. Turn on hardware acceleration in your browser settings, or use a current version of Chrome, Edge, Firefox or Safari. | page | none |
| `unsupported_wasm` | Partsmith needs WebAssembly, and this browser does not provide it. Use a current version of Chrome, Edge, Firefox or Safari. | page | none |
| `core_load_failed` | The geometry core did not load: {reason}. Check your connection and reload the page. | fatal | Reload |
| `core_stopped` | The geometry core stopped: {reason}. Restart it to keep working; your latest changes are kept. | fatal | Restart core |
| `not_ptsm` | {file} is not a Partsmith model. Partsmith opens .ptsm files. | error | none |
| `not_text` | {file} could not be read as text. | error | none |
| `open_failed` | Could not open {file}. {core message} | error | none |
| `migrated` | {file} was updated from format version {n}. Saving writes the current format. | info | none |
| `save_permission` | Could not save {file} because Partsmith no longer has permission to write it. | error | Save as |
| `save_not_found` | {file} was moved or deleted. Save it again to choose a location. | error | Save as |
| `save_io` | Could not save {file}: {reason} | error | Save as |
| `download_hint` | This browser saves by downloading. {file} is in your downloads folder. | info | none |
| `recent_permission` | Partsmith needs permission to open {file}. Choose it again to grant access. | error | Open... |
| `recent_missing` | {file} has moved or been deleted, so it was removed from recent files. | warning | none |
| `stashed` | {name} had unsaved changes. They are kept for you. | info | Reopen |
| `recovered` | Partsmith kept unsaved changes to {name} from {time}. | info (persistent) | Open, Discard |
| `nothing_to_export` | There is no visible body with geometry to export. | error | none |
| `exported` | Exported {file}. | success | none |
| `deleted` | Deleted {label}. | info | Undo |
| `feature_failed` | {label} failed. {message} | error | Edit, Undo |
| `features_failed` | {n} features failed after this change. | error | Show problems, Undo |
| `over_constrained` | {title} would over-constrain the sketch: it conflicts with {list}. | warning | Replace them, Cancel |
| `redundant_constraint` | {title} is already implied by other constraints, so it was not added. | info | none |
| `dimension_over_constrained` | This dimension would over-constrain the sketch. Add it as a reference dimension that only measures? | inline | Add reference, Cancel |
| `dimension_unsolvable` | The sketch cannot be solved with this value. Check for dimensions that cannot all be true at once. | inline | none |
| `cannot_dimension` | Partsmith cannot dimension {a} and {b}. | hint | none |
| `arc_collinear` | These three points are in a line. Move the last point off the line to make an arc. | hint | none |
| `profile_problems` | {label} has {n} profile problems, so some regions may not extrude. | info | Back to sketch |
| `units_changed` | Units are now {unit}. Numbers typed without a unit now mean {unit}. | info | none |
| `pick_plane` | Pick a flat face, a datum plane or an origin plane. | hint | none |
| `pick_axis` | Pick a straight edge, a sketch line, a round face or an origin axis. | hint | none |
| `pick_kind` | Pick {plural noun}. | hint | none |
| `pick_same_body` | Pick {kind}s of '{body}', or clear Body first. | hint | none |
| `parameter_name` | Names start with a letter or _ and use only letters, digits and _. | inline | none |
| `field_required` | {label} needs a value. | inline | none |
| `field_not_number` | {label} must be a number. | inline | none |
| `look_at_disabled` | Select a flat face or a plane first. | disabled reason | none |
| `nothing_to_undo` | Nothing to undo. | disabled reason | none |
| `nothing_to_redo` | Nothing to redo. | disabled reason | none |
| `finish_command_first` | Finish or cancel {title} first. | disabled reason | none |
| `fill_fields` | Fill in the highlighted fields first. | disabled reason | none |

### 4.4 Fatal states

- Core load failure (`transport.start()` rejects, including a protocol version mismatch reported by the worker host): the viewport shows a panel (`data-testid="core-fatal"`, `role="alert"`) with `core_load_failed` and a "Reload" button (`location.reload()`); the rest of the shell stays but every core-dependent command is disabled with "The geometry core is not running."
- Core stopped later (fatal listener): the same panel with `core_stopped` and "Restart core". `app.restartCore()` creates a new transport and client, starts it, reloads the catalog and printers, and reopens the last recipe with `openDocument(JSON.stringify(recipe, null, 2))`, keeping the document name and handle and setting `localUnsaved` true. Undo history is lost; selection, command and sketch session are cleared.
- A `CoreError` with code `internal_error` or `document_poisoned` is shown verbatim as an error notification with no other change; the core's message already tells the user to save and reopen.
- Uncaught exceptions in UI code are caught by a React error boundary around each region (top bar, browser, viewport, side panel, timeline, status bar). A crashed region shows "This part of the window hit a problem. Reload to restore it." with "Reload"; the other regions keep working and `console.error` gets the details.

## 5. Performance budgets

Release gates from technical design section 14, measured on the reference machine (a 2022 mid-range laptop) in Chrome for web and in the desktop app, with the User Timing names of 2.18. Performance tests (6.7) run on the fixed hardware runner and are tagged `@perf`.

| Interaction | Desktop | Web | Measured as |
| --- | --- | --- | --- |
| Sketch drag, 500 constraints | 16 ms per frame | 16 ms per frame | 95th percentile of `ps:sketch-drag` over a 2 s drag |
| Feature preview after a value change | 100 ms typical | 250 ms typical | Median of `ps:preview` over 20 changes |
| Full rebuild, 50-feature part | 1 s | 3 s | `ps:rebuild` of a change to the first feature's driving parameter |
| Open a 50-feature file, warm cache | 300 ms | 1 s | `ps:open`, second open of the same file in the session |
| Viewport orbit, 2 M triangles | 60 fps | 60 fps | 95th percentile of `ps:frame` at most 16.7 ms over a 3 s orbit |
| Cold start to usable | 2 s | 4 s | `ps:usable` relative to `performance.timeOrigin`, on a reload with the core module in the HTTP cache |

UI budgets derived from them (same runner):

| Interaction | Budget | Measured as |
| --- | --- | --- |
| Pick query with a current ID pass | 1 ms | 95th percentile of `ps:pick` |
| Hover highlight | next animation frame | `ps:pick` plus one frame |
| Palette re-rank per keystroke, 2,000 items | 8 ms | 95th percentile of `ps:palette` |
| Expression validation round trip (web) | 50 ms | from keystroke to `expression-result` update, median |
| Main-thread long tasks during orbit, drag or preview typing | none over 50 ms | `PerformanceObserver` of `longtask` |
| Applying a `document_changed` plus `recomputed` for 50 features | 8 ms | profiler mark around the store update |
| Initial JavaScript (excluding the core module) | 250 KB gzipped | build output check |

Rules that make these reachable: hot paths bypass React; meshes are never copied on the main thread (frame views go straight to `upload`); picks read a small block of the ID pass; previews and drags are latest-only; analysis never runs automatically outside Print.

## 6. Acceptance tests

Vitest unit tests live next to the code as `src/**/*.test.ts(x)` (jsdom; no WebGL; fakes from `src/test/fakes`). Playwright end-to-end tests live in `ui/e2e/*.spec.ts` and run against the production build with the real WebAssembly core in `ui/public/core/` (`pnpm -C ui e2e`; the project is Desktop Chrome, viewport 1280 by 720, device scale factor 1, Linux, so `Mod` is Control; use Playwright's `ControlOrMeta`). Protocol-level codec, bridge and worker host tests are protocol.md tests 53 to 66 and are not repeated here.

### 6.1 Selectors

Tests use these stable selectors only (plus accessible roles and names given in section 3).

| `data-testid` | Element | Attributes |
| --- | --- | --- |
| `app` | Root | `data-core-status` (`loading`, `ready`, `failed`), `data-context` (`model`, `sketch`, `print`) |
| `unsupported` | Unsupported page | |
| `core-overlay`, `core-fatal` | Loading overlay, fatal panel | |
| `top-bar`, `document-name`, `save-state`, `document-menu`, `palette-button` | Top bar parts | `document-name`: `data-dirty` |
| `palette`, `palette-input`, `palette-option`, `palette-shortcut` | Palette | option: `data-item-id`, `data-kind` |
| `browser`, `browser-row` | Browser | row: `data-kind`, `data-id` |
| `color-popover` | Body color popover | |
| `viewport` | Viewport | `data-hover-kind`, `data-hover-key`, `data-selection-count`, `data-view`, `data-projection`, `data-nav-preset`, `data-section`, `data-camera-target` |
| `view-cube`, `viewport-controls`, `section-popover` | Viewport chrome | |
| `inspector` | Inspector | `data-mode` (`command`, `selection`), `data-preview` (`none`, `pending`, `shown`, `failed`) |
| `inspector-field`, `inspector-more`, `inspector-ok`, `inspector-error` | Inspector parts | field: `data-path` |
| `selection-field` | Selection field box | `data-active`, `data-count` |
| `expression-result`, `field-error` | Expression field parts | |
| `selection-title`, `body-size`, `body-center` | Selection inspector parts | |
| `timeline`, `timeline-item`, `timeline-marker`, `timeline-problems`, `context-menu` | Timeline | item: `data-feature-id`, `data-status`, `data-suppressed`, `data-rolled-back` |
| `side-panel`, `parameters-panel`, `parameter-row`, `parameter-value`, `problems-panel`, `health-list`, `health-item` | Side panels | row: `data-id`; health item: `data-check`, `data-severity` |
| `sketch-header`, `sketch-problems`, `sketch-hint` | Sketch chrome | header: `data-state`, `data-dof` |
| `sketch-dimension`, `dimension-editor`, `constraint-glyph`, `inference-glyph`, `draw-input`, `profile-issue`, `box-selection` | Sketch overlay | dimension: `data-constraint-id`, `data-type`, `data-driving`; glyph: `data-constraint-id`, `data-type`, `data-state`; inference: `data-type` |
| `status-bar`, `status-core`, `status-dof`, `status-recompute`, `status-units`, `status-printer`, `health-badge` | Status bar | dof: `data-dof`, `data-state`; badge: `data-state`, `data-stale` |
| `printer-menu`, `checks-popover`, `overhang-legend`, `finding-marker`, `export-popover`, `drop-overlay` | Print and export | |
| `notification` | Notification | `data-severity`, `data-code` |

### 6.2 Fixtures

`ui/e2e/fixtures/` holds `.ptsm` files written by the test author from model.md, sketch.md and the feature specs. Every fixture is canonical (`partsmith fmt` leaves it unchanged), valid (`partsmith validate` is clean), unit `mm`, with unnamed features (labels come from features.md 3.12).

| File | Content | Result |
| --- | --- | --- |
| `cube.ptsm` | Body `b-cube` "Cube". `f-sk1`: sketch on XY with a fully constrained 20 by 20 rectangle centered on the origin. `f-ext1`: extrude of its region, distance `20 mm`, op new, body `b-cube`. | Cube x, y in [-10, 10], z in [0, 20]; timeline "Sketch 1", "Extrude 1" |
| `rect-sketch.ptsm` | `f-sk1`: sketch on XY: rectangle with a corner coincident with the origin, horizontal and vertical sides, driving `distance` `40 mm` on the bottom line and `20 mm` on the left line; no bodies. | Fully constrained, one region of 40 by 20 at x in [0, 40], y in [0, 20] |
| `underconstrained.ptsm` | `f-sk1`: sketch on XY with line `e-1` from (0, 0) to (30, 0) and circle `e-2` centered (50, 0) radius 5; no constraints; no bodies. | DOF 7 |
| `parametric.ptsm` | Parameter `p-size` `size` = `20 mm`; otherwise `cube.ptsm` with every dimension and the extrude distance set to `size`. | Cube of side `size` |
| `cube-features.ptsm` | `cube.ptsm` plus `f-fil1` fillet `2 mm` on the 4 vertical edges and `f-sh1` shell `2 mm` removing the top face. | 4 features, all ok |
| `two-bodies.ptsm` | `cube.ptsm` plus body `b-peg` "Peg" and `f-cyl1` cylinder radius `4 mm`, height `20 mm`, base center (0, 0, 10), axis +Z, op new, body `b-peg`. | Timeline "Sketch 1", "Extrude 1", "Cylinder 1" |
| `fillet-error.ptsm` | `cube.ptsm` plus `f-fil1` fillet `15 mm` on the 4 top edges. | "Fillet 1" fails with `fillet_too_large` |
| `revolve-sketch.ptsm` | `f-sk1`: sketch on XZ with a fully constrained rectangle at sketch x in [10, 20], y in [0, 10]; no bodies. | One region; sketch visible |
| `offset-cube.ptsm` | As `cube.ptsm` with the rectangle at x in [10, 30], y in [-10, 10]. | Cube x in [10, 30] |
| `overhang.ptsm` | Body "Mushroom": a 10 by 10 by 20 post centered on the origin (z 0 to 20) joined with a 30 by 30 by 5 cap (z 20 to 25); `printer` set to the default FDM profile id (print.md). | At least one `overhangs` finding |
| `oversize.ptsm` | Body "Slab": a 400 by 400 by 10 box; `printer` the default FDM profile. | A `bed_fit` error |
| `notes.txt` | The text `hello`. | Not a model |
| `sketch-500.ptsm` | One sketch with at least 500 constraints in one connected component and DOF at least 2, where dragging point `e-drag.end` moves at least 100 other entities. | Perf |
| `part-50.ptsm` | 50 features (sketches, extrudes, holes, fillets, patterns) whose first sketch is driven by parameter `base`. | Perf |
| `dense.ptsm` | At least 2,000,000 display triangles at standard quality (for example a 20 by 20 linear pattern of spheres). | Perf |
| `palette-2000.ptsm` | 1,980 parameters `p0` to `p1979` and 20 features. | Perf |

### 6.3 Helpers (`ui/e2e/helpers.ts`)

- `openApp(page, { fileSystemAccess = true } = {})`: when false, an init script deletes `window.showOpenFilePicker` and `window.showSaveFilePicker`; then `page.goto('/')` and waits for `[data-testid=app][data-core-status=ready]`.
- `dropFile(page, name)`: reads the fixture, builds a `DataTransfer` with a `File` in the page, and dispatches `dragenter`, `dragover` and `drop` on the app root; waits for `document-name` to equal the name without extension (or for a notification).
- `canvas(page)`: the canvas bounding box `{ x, y, width, height }`, its center `(cx, cy)` and `H = height`.
- `fitView(page, view)`: clicks "{View} view" on the cube, turns "Orthographic" on, clicks "Fit to view", and waits for `data-view` and `data-projection`.
- `toScreen(box, bounds, view)`: maps model millimeters to page pixels using the fit rule of 2.11 (content extents scaled to 80 percent of the limiting dimension, bounds center at the canvas center). For the cube in the front view: `s = 0.04 × H` px per mm, `x_px = cx + x × s`, `y_px = cy - (z - 10) × s`; the top edge of the front face is at `top = cy - 0.4 H`.
- `sketchToScreen(box, contentMin, contentMax)`: the same mapping for a sketch on XY just entered (top view, content framed with margin 0.1).
- `expectHover(page, x, y, kind)`: moves the mouse and polls `data-hover-kind`.
- `download(page, action)`: waits for the `download` event during `action` and returns the suggested name and bytes.
- `measures(page, name)`: `performance.getEntriesByName(name)` durations.

### 6.4 Vitest unit tests

Core and transports:

1. U-1. `createWorkerTransport` with a fake worker that posts `{type: 'ready', protocolVersion: 1, engineVersion: '0.1.0'}`: `start()` resolves `{protocolVersion: 1, engineVersion: '0.1.0'}`.
2. U-2. A fake worker posting `{type: 'fatal', message: 'HTTP 404'}` before ready: `start()` rejects with `CoreFatalError` whose message contains `HTTP 404`.
3. U-3. With fake timers and no ready message, `start()` rejects after exactly 30,000 ms with "The geometry core did not start within 30 seconds."
4. U-4. Two `send` calls post `{type: 'call', frame}` messages whose transfer lists contain each frame; two `{type: 'frame'}` replies resolve them in order.
5. U-5. A worker `error` event after start rejects both pending sends with `CoreFatalError`, calls a fatal listener exactly once, and a later `send` rejects immediately.
6. U-6. `createTauriTransport` with a fake `invoke`: `start()` calls `invoke('call', frame)` with an `engine_info` request frame of id 0 and resolves the versions; a reply with `protocol_version` 2 rejects with "This app and its core speak different protocol versions. Reinstall Partsmith."
7. U-7. Tauri transport: three `drag_sketch` frames for doc 1 and `f-sk1` sent while the first `invoke` is pending: `invoke` runs for the first and third only; the second resolves with a `superseded` error frame for its id.
8. U-8. Tauri transport: an `invoke` rejection with `Error('boom')` resolves the send with an error response frame of code `internal_error` for that request id.
9. U-9. `chooseTransport({ __TAURI__: { core: { invoke } } })` has kind `tauri`; `chooseTransport({})` has kind `worker`.
10. U-10. `createCoreClient`: three requests carry ids 1, 2 and 3, and the first frame's header equals `{protocol_version: 1, messages: [{kind: 'request', id: 1, command}], buffers: []}`.
11. U-11. A response frame `[document_changed, recomputed, response ok]`: listeners see both events, in order, before the request promise resolves.
12. U-12. An error outcome rejects with `CoreError` carrying the code, the message verbatim and `editIndex`.
13. U-13. A response with another id rejects with `CoreFatalError` and notifies fatal listeners.
14. U-14. An event of unknown type is ignored; a listener that throws does not stop the next listener or the request.
15. U-15. `createMutationQueue`: the second task starts only after the first settles; a rejected task does not block the next; `pending` counts queued and running tasks.
16. U-16. `createLatestOnly`: submitting a, b and c while a is in flight sends a then c (never b); `onResult` runs once, for c; `cancel()` drops a waiting input and suppresses the in-flight result.
17. U-17. `decodeEvent` of a `body_mesh` returns typed arrays whose `buffer` is the frame's `ArrayBuffer`; an out-of-range buffer index throws `CoreFatalError`; an unknown event type returns null.
18. U-18. The store ignores a `document_changed` whose revision is lower than the stored one and applies one with an equal revision.
19. U-19. `generateId('f', () => false, () => 0x3fa91c0e72b4)` is `f-3fa91c0e72b4`; `generateId('b', () => false, () => 1)` is `b-000000000001`; a predicate that rejects the first candidate gets the second; 64 rejections throw "Could not generate a free id after 64 attempts."
20. U-20. `featureLabel`: the second unnamed extrude is "Extrude 2"; a named record shows its name; suppressed records count; an unknown type uses the type name. `nextBodyName` with bodies "Body 1" and "Body 3" is "Body 2".

Commands, shortcuts and palette:

21. U-21. `formatShortcut`: `Mod+Shift+S` is `⇧⌘S` on macOS and `Ctrl+Shift+S` on Linux; `Mod+Enter` is `⌘↩` and `Ctrl+Enter`; `Delete` is `⌦` and `Del`; `F6` is `F6`. `parseShortcut` throws `Invalid shortcut "Shift+".`, and likewise for `Shift+Mod+S`, `Mod+Mod+S` and `Mod+Ctrl+S`.
22. U-22. The built-in registry plus the test catalog has no `conflicts()` on macOS, Windows or Linux.
23. U-23. `chordFromEvent`: `{key: 's', metaKey: true, shiftKey: true}` on macOS gives `Mod+Shift+S`; the same with `ctrlKey` on Linux; `{key: 'д', code: 'KeyL'}` gives `L`; a lone Shift gives null; `{key: ' '}` gives `Space`.
24. U-24. Dispatcher: `e` with an `<input>` target runs nothing; `e` with `document.body` runs `feature.extrude`; `Mod+S` in an input runs `file.save`; `Mod+Z` in an input runs nothing; `defaultPrevented` and `repeat` events run nothing; with the palette open only `Mod+K` runs.
25. U-25. Registering a duplicate id throws "Command x is already registered."; running a disabled command calls no `run` and shows an info notification with its reason.
26. U-26. `fuzzyScore('ext', 'Extrude')` is 18.93, `('ext', 'Export 3MF')` 14.9, `('ext', 'Text')` 10.96 (within 1e-9), `('xyz', 'Extrude')` null; `rankPaletteItems('ext', ...)` orders Extrude, Export 3MF, Text; query `pad` matches Extrude through its keyword with score 16.97.
27. U-27. Equal scores order by `recentIds`, then title; an empty query lists up to 5 recent items first, then category order; results never exceed 50.
28. U-28. `paletteItems` in Model context with `cube.ptsm` state: contains "Extrude 1" (feature), "Cube" (body), "Theme: Dark" (setting, `checked` false under the light theme) and no sketch tools; in sketch mode it contains "Line".
29. U-29. The catalog commands: one `feature.<type>` per entry, title the manifest label, `extrude` with shortcut `E`, keywords including `pad`, and an unknown icon name falling back to `Shapes`.

Inspector:

30. U-30. `normalizeManifest` accepts the extrude manifest JSON of features.md 2.3; missing `essential` becomes false and missing `options` empty; `category: 'other'` throws `ManifestError` naming `category`.
31. U-31. `buildInspectorModel` with the extrude test catalog entry (schema and manifest of features/extrude.md) and params `{extent: {kind: 'distance', expr: '30 mm'}, op: 'new'}`: essential paths `profile`, `extent.kind`, `extent.expr`, `op`; more paths `reverse`, `body`; `extent.opposite_expr` absent; controls: `profile` selection `sketch_region` multiple, `extent.kind` segmented choice with 4 options, `extent.expr` expression length, `op` segmented choice, `reverse` switch, `body` selection `body` not multiple. With `kind: 'two_sides'` the `extent.opposite_expr` field appears; with `through_all`, `extent.expr` disappears.
32. U-32. `complete` is false while `profile` is missing and true once set; `forceMore` is true with `errors = {body: '...'}`.
33. U-33. `defaultParams` for extrude: `extent` `{kind: 'distance', expr: '10 mm'}`; `op` `new` without bodies with geometry; `op` `join` and `body` equal to `lastTargetBody` with them.
34. U-34. `setParam(..., 'extent.kind', 'two_sides')` keeps `expr` and adds `opposite_expr` `10 mm`; an unknown key `taper_angle` in params is preserved by every `setParam`.
35. U-35. `selectionValue`: origin XY for `plane` gives `{kind: 'origin', plane: 'XY'}`; a flat face gives `{kind: 'face', body, face: reference}`; origin Z for `axis` gives `{kind: 'origin', axis: 'Z'}`; a sketch line gives `{kind: 'sketch_line', sketch, line}`; two regions of `f-sk1` give a `ProfileRef` with both regions in pick order; edges with `multiple` give an array of references.
36. U-36. `acceptsTarget('plane', face)` is true for `surface: 'plane'` and false for `cylinder`; `acceptsTarget('axis', edge)` is true for `curve: 'line'` and false for `circle`; a cylindrical face is accepted by `axis`.
37. U-37. `placeInspector` with bounds (0, 0, 1000, 600), panel 320 by 400: anchor (100, 100, 50, 50) gives (166, 100); anchor (700, 100, 200, 50) gives (364, 100); anchor (100, 100, 800, 50) gives (16, 16); anchor (100, 500, 50, 50) gives y 184; null gives (16, 16).

Expression input:

38. U-38. `convertForField`: `{value: 2, dimension: 'dimensionless'}` for a length in inches is 50.8; 90 dimensionless for an angle is π/2; 3.0000000001 for a count is 3; 2.5 for a count with label Count gives "Count must be a whole number, but it is 2.5. Use 2 or 3."; an angle for a length labeled Distance with source `10 deg` gives "Distance must be a length, but '10 deg' is an angle. Use a value such as 10 mm."
39. U-39. Formatting: 12.5 mm "12.5 mm"; 2.4 mm in an inch document "0.0945 in"; 1/3 mm "0.333 mm"; -0 "0 mm"; 1234.5 "1234.5 mm"; 25.4 mm in cm "2.54 cm"; 1 mm in m "0.001 m"; π/4 rad "45°"; `formatSize` of (0, 0, 0) to (40, 20, 10) "40 × 20 × 10 mm"; `formatPoint` (10, 0, 10) "10, 0, 10 mm".
40. U-40. `stepExpression`: `10` up gives `11`; `10 mm` up gives `11 mm`; `10mm` up gives `11mm`; `2.5` up coarse gives `12.5`; `2.5` up fine gives `2.6`; `-0.1` up fine gives `0`; `wall` gives null; a count `0` down gives `0`; an angle `45 deg` up gives `46 deg`.
41. U-41. `isPlainNumber` is true for `10`, `-2.5`, `10mm`, `10 mm`, `45 deg` and false for `2*5`, `wall`, `10 mm + 1` and the empty string.
42. U-42. `ExpressionInput` with a fake `evaluate`: typing `2 * 5` shows `= 10 mm`; typing `10` shows `mm`; a rejected evaluation shows the core message in `field-error`; when the result for `1` arrives after the result for `12`, the input shows the result for `12`.
43. U-43. Enter calls `onCommit` only in the valid state; Esc calls `onCancel`; ArrowUp on `10` changes the value to `11`.

Viewport math and picking:

44. U-44. `standardViewRotation` forward and up vectors match 2.11 for all seven views; `classifyView` returns each view for its own rotation and `custom` after a 1e-3 rad perturbation.
45. U-45. `fitBounds` orthographic, front view, bounds (-10, -10, 0) to (10, 10, 20): aspect 2 gives `orthoHeight` 25 and target (0, 0, 10); aspect 0.5 gives 50.
46. U-46. `fitBounds` perspective, top view, bounds (0, 0, 0) to (40, 20, 0), aspect 1.6: `distance` equals `31.25 / (2 tan 15°)` within 1e-9.
47. U-47. `zoomAt` keeps the world point under the cursor at the same NDC position within 1e-9 for both projections; distance and orthoHeight clamp to [0.01, 1e7].
48. U-48. `withProjection` round trip keeps `visibleHeightAtTarget`; `orbit(c, 100, 0, target)` rotates about world Z by -40°; `pan` moves the target by exactly the pixel delta times mm per pixel; `wheelZoomFactor({deltaY: 100, deltaMode: 0, ctrlKey: false}, 600)` is 1.0015^100, with `deltaMode: 1, deltaY: 3` it is 1.0015^48, and with `ctrlKey` 1.01^100.
49. U-49. `encodePickId` and `decodePickId` round-trip 1, 255, 256, 65536, 16777216 and 4294967295; `choosePick` prefers a vertex at 3 px over an edge at 1 px over a face at 0 px; with vertices not accepted it picks the edge; an edge at 5 px with radius 4 loses to the face; a face at 1 px is ignored.

Sketch:

50. U-50. `rectangleTwoPointRecipe` from (0, 0) to (40, 20) returns 4 lines in the order and directions of 3.16.4 and exactly 4 `coincident`, 2 `horizontal` and 2 `vertical` constraints with the listed operands; a zero width returns null.
51. U-51. `rectangleCenterRecipe` with center (0, 0) and corner (20, 10) returns 4 lines from (-20, -10), one construction line from (-20, -10) to (20, 10), one construction point at (0, 0), and 11 constraints (4 + 2 + 2 coincident and orientation, 2 coincident for the diagonal, 1 midpoint).
52. U-52. `circleRecipe` returns one circle; radius 0 returns null. `pointRecipe` returns one point entity with the construction flag as given.
53. U-53. `arcThreePointRecipe` A (10, 0), B (-10, 0), T (0, 10) gives center (0, 0), radius 10, counterclockwise from (10, 0) to (-10, 0); with T (0, -10) it runs from (-10, 0) to (10, 0); collinear points return null.
54. U-54. `lineRecipe` with equal ends returns null; the construction flag is copied.
55. U-55. `circleThroughThreePoints` of (0, 0), (2, 0), (0, 2) is center (1, 1), radius √2; `tessellateArc` of a full circle radius 10 with tolerance 0.01 has `ceil(2π / (2 acos(1 - 0.001)))` segments.
56. U-56. `infer` with 0.1 mm per pixel: a point 0.7 mm away snaps (`point`), one 0.9 mm away does not; a line midpoint 0.5 mm away snaps (`midpoint`); a curve 0.5 mm away snaps (`curve`); a point and a midpoint both in range pick the point; a segment at 2.9° is `horizontal` and its end is moved onto the horizontal, at 3.1° nothing; 88° is `vertical`; a segment from an arc end within 5° of its tangent is `tangent` and not horizontal; `suppressed` returns the raw point and no inference.
57. U-57. `applicableConstraints`: two lines give Parallel, Perpendicular, Collinear, Equal, Fix; one line gives Horizontal, Vertical, Fix; two points give Coincident, Horizontal, Vertical, Fix; a point and a line give Point on object, Midpoint, Fix; two points and a line give Symmetric, Fix; a line and a circle give Tangent, Fix; two circles give Tangent, Concentric, Equal, Fix.
58. U-58. `constraintFor('symmetric', ...)` uses the line as the axis whatever the selection order; `constraintFor('coincident', [point, line])` returns a `point_on_object` constraint.
59. U-59. `dimensionTypeFor` with points (0, 0) and (10, 5): label (5, 10) `horizontal_distance`, (15, 2) `vertical_distance`, (12, 8) `distance`; points (0, 0) and (10, 0) with label (5, 5) `distance`; a circle `diameter`; an arc `radius`; lines at 90° `angle`; parallel lines `distance`; a point and a circle unsupported with "Partsmith cannot dimension a point and a circle."
60. U-60. `measuredValue('angle', ...)` for a line along +X and a line at 60° through the origin is π/3 with the label at (cos 30°, sin 30°) and 2π/3 with the label at (-1, 0.5).
61. U-61. `commitSketchChange` with a fake core: a requested id reported conflicting returns `over_constrained` and calls no `applyEdits`; an inferred id reported redundant is removed, the sketch is solved again, and `applyEdits` gets the params without it and with solved geometry; a `failed` status returns `failed` with the solver message.
62. U-62. Sketch view history: enter (view entry), orbit (view entry), commit an edit (document marker); three `Mod+Z` presses restore the orbit camera, call core `undo` once, and restore the pre-entry camera, in that order; `Mod+Shift+Z` replays them in reverse.
63. U-63. Sketch header mapping: no entities `empty`; `under_constrained` with dof 3 `under` and "3 DOF"; `fully_constrained` `full`; `over_constrained` with 2 conflicting `conflict` and "2 conflicting constraints"; `failed` "Cannot solve"; one redundant appends " · 1 redundant".

Store and actions:

64. U-64. `startFeatureCommand('extrude')` with a preselected region and no bodies: `beginTransaction(doc, 'Extrude')`, then one `applyEdits(doc, null, [add_body "Body 1", add_feature])` with the body id in the record, then `setFocus(doc, featureId)`.
65. U-65. With `rollback` `f-sk1` (index 0 of 3 features), the first preview's `add_feature` has index 1 and is followed by `set_rollback` to the new id.
66. U-66. `confirmCommand` calls `commitTransaction`; `cancelCommand` calls `cancelTransaction`; `Mod+Z` with a command active cancels it and calls no `undo`.
67. U-67. `editFeature('f-ext1')` calls `beginTransaction(doc, 'Edit Extrude 1')` and `setFocus(doc, 'f-ext1')`; a `focus_inactive` error on a suppressed feature calls `cancelTransaction` and shows an error notification with the action "Unsuppress".
68. U-68. Timeline edits: deleting the rollback feature sends `set_rollback` to the nearest earlier remaining active feature before `remove_feature`; dropping feature index 3 at gap 1 sends `move_feature` index 1, at gap 4 index 3 (no edit); `setRollback(N)` sends `set_rollback` without an id and `setRollback(2)` the id of feature 2.
69. U-69. Esc ladder: with an open popover, a sketch drawing in progress, an active tool, a command and a selection, successive Esc presses close the popover, cancel the drawing, exit the tool, cancel the command, then clear the selection.
70. U-70. Theme tokens: `theme.css` (imported `?raw`) defines every token of 2.16 with exactly the listed value in both the light and the dark block, and no other `--ps-` color tokens; `contrastRatio` of every pair stated in the 2.16 table meets its threshold in both themes.
71. U-71. `resolveTheme('system', true)` is `dark`; `applyTheme` sets `data-theme`; `loadSettings` of malformed JSON returns `DEFAULT_SETTINGS`, and of `{"theme": "dark", "uiScale": 7}` returns theme dark with scale 1.
72. U-72. Notifications with fake timers: info disappears after 5 s, warnings after 8 s, errors stay; a fourth notification removes the oldest non-error; a duplicate message restarts the existing timer instead of adding one.
73. U-73. Every message id of 4.3 renders exactly its text for sample placeholders (table-driven).
74. U-74. Web file service with mocked File System Access: `pickToOpen` returns name, bytes and handle; an `AbortError` returns null; `chooseSaveTarget` returns a handle target; `write` calls `requestPermission` when `queryPermission` is `prompt` and throws `FileServiceError` `permission` when it is denied; a `NotFoundError` becomes `not_found`.
75. U-75. Without the API: `chooseSaveTarget` returns `{kind: 'download', name}`; `write` clicks an anchor whose `download` is the name and revokes its object URL; `pickToOpen` resolves from the hidden input's `change` event.
76. U-76. `createMemoryRecentFiles`: at most 10 entries, newest first; a text entry with an existing name replaces it; a handle entry whose handle `isSameEntry` an existing one replaces it.
77. U-77. Recovery: a dirty `document_changed` writes an entry after 2 s (fake timers); another change within 2 s postpones it; a successful save clears the store.
78. U-78. Save as cancelled at the picker never calls `saveDocument`; Save to a handle whose write fails with `permission` sets `localUnsaved` and shows `save_permission` with the action "Save as".
79. U-79. `openFile`: `model.stl` shows `not_ptsm`; invalid UTF-8 shows `not_text`; a `parse_error` shows `open_failed` with the core message and keeps the current document; with a dirty current document the stash is written before `closeDocument`.
80. U-80. `exportModel` with no visible bodies shows `nothing_to_export` and calls no `exportBodies`; formats map to `{"format": "3mf"}`, `{"format": "stl", "binary": true, "per_body": false}`, `{"format": "obj", "per_body": false}` and `{"format": "step"}`; the suggested name is `{document name}.3mf`.
81. U-81. Print: badge states derive as in 3.17, hidden checks are excluded from counts, a report for an older revision is stale; in Print context a revision change schedules `analyzePrint` after 300 ms (fake timers) and a `null` result changes nothing; in Model context no analysis is scheduled.
82. U-82. After a mesh change, a selected face whose reference resolves `missing` leaves the selection and one that resolves is kept.
83. U-83. Rendering `App` with fakes produces the landmarks banner, navigation "Browser", main "3D view", region "Timeline" and contentinfo, and every `button` has a non-empty accessible name.
84. U-84. `app.restartCore()` with fake transports: the new core receives `open_document` with `JSON.stringify(recipe, null, 2)`, the document keeps its name and handle, and `localUnsaved` is true.

### 6.5 Playwright end-to-end tests

Each test starts with `openApp` unless it says otherwise. "Click X" means clicking the element with accessible name X in the region named. Positions are relative to the canvas center `(cx, cy)` in CSS pixels, with `H` the canvas height.

Startup and files:

1. E-1. Cold start: `document-name` is "Untitled" with `data-dirty="false"`; the radio "Model" is checked; the toolbar "Model tools" has, in order, buttons named Sketch, Extrude, Revolve, Hole, Fillet, Chamfer, Shell, Primitive, Pattern, Mirror, Boolean; `status-units` reads "mm"; `health-badge` has `data-state="none"`.
2. E-2. With an init script making `getContext('webgl2')` return null: `unsupported` is visible with the `unsupported_webgl2` text and the page has no worker.
3. E-3. Drop `cube.ptsm`: `document-name` "cube"; two `timeline-item`s named "Sketch 1" and "Extrude 1" with `data-status="ok"`; a `browser-row[data-kind=body]` with text "Cube"; `status-recompute` matches `^Rebuilt in \d+(\.\d)? (ms|s)$`; `data-dirty="false"`.
4. E-4. `openApp(page, {fileSystemAccess: false})`, press ControlOrMeta+O, answer the file chooser with `cube.ptsm`: same expectations as E-3.
5. E-5. Drop `notes.txt`: a `notification[data-severity=error]` reads "notes.txt is not a Partsmith model. Partsmith opens .ptsm files."; `document-name` stays "Untitled".
6. E-6. Without File System Access, drop `cube.ptsm`, press ControlOrMeta+S: a download named `cube.ptsm` whose bytes equal the fixture's bytes; an info notification with `download_hint` text.
7. E-7. Drop `cube.ptsm`; click the body row "Cube", press F2, type "Box", Enter: `save-state` reads "Edited" and `data-dirty="true"`. Press ControlOrMeta+S (no File System Access): the download contains `"name": "Box"`; `data-dirty="false"` and `save-state` is empty.
8. E-8. Without File System Access, drop `cube.ptsm`, reload, open the Document menu, open "Open recent", click "cube": `document-name` "cube" and two timeline items.
9. E-9. Drop `cube.ptsm`, rename the body to "Box", wait 3 s, reload: a notification starting "Partsmith kept unsaved changes to cube from"; click "Open": a body row "Box" and `data-dirty="true"`.

Palette and shortcuts:

10. E-10. Press ControlOrMeta+K: `palette` visible and `palette-input` focused. Type "ext": the first `palette-option` contains "Extrude" and its `palette-shortcut` reads "E". Enter: `inspector[data-mode=command]` named "Extrude"; Escape closes it. ControlOrMeta+K, type "pad": an option "Extrude" is listed. With `cube.ptsm` dropped, ControlOrMeta+K, type "Cube", Enter: `data-selection-count="1"` and `selection-title` "Cube".
11. E-11. Open the palette, run "Parameters", click "Add parameter", type "e": the name field contains "e" and no command inspector opened. Escape, click the canvas at `(cx, cy)`, press "e": the inspector "Extrude" opens.

Sketching:

12. E-12. Click "Sketch" in the toolbar: `data-context="sketch"`, `sketch-header` `data-state="picking"`. Expand "Origin" in the browser and click "XY plane": a timeline item "Sketch 1", `data-view="top"`, header `data-state="empty"`. Press R, move to `(cx, cy)`: an `inference-glyph[data-type=coincident]` is visible. Click there, click `(cx + 200, cy - 100)`: header `data-state="under"`, `data-dof="2"`, `status-dof` `data-dof="2"`. Press D, click `(cx + 100, cy)`, click `(cx + 100, cy + 40)`: `dimension-editor` visible; type "40", Enter: a `sketch-dimension` with text "40" and `data-dof="1"`. Click `(cx, cy - 50)`, click `(cx - 40, cy - 50)`, type "20", Enter: `data-dof="0"`, `data-state="full"`, header text contains "Fully constrained".
13. E-13. Continue from E-12 (or open `rect-sketch.ptsm` and double-click "Sketch 1"). Press E: `data-context="model"`; the inspector "Extrude"; the `selection-field` in `inspector-field[data-path=profile]` has `data-count="1"`; the Distance input is focused. Type "10": `expression-result` reads "mm". Enter: timeline items "Sketch 1" and "Extrude 1", both `data-status="ok"`; a body row "Body 1"; clicking it shows `body-size` "40 × 20 × 10 mm" and `body-center` "20, 10, 5 mm".
14. E-14. Drop `rect-sketch.ptsm`, `fitView(page, 'Top')`, press E, click `(cx, cy)`: the profile field has `data-count="1"`. Fill Distance with "10 deg": `field-error` visible containing "must be a length", `inspector-ok` `aria-disabled="true"`. Fill "2 * 5": `expression-result` "= 10 mm", no `field-error`, OK enabled.
15. E-15. New sketch on XY (as in E-12). Press L, click `(cx, cy)`, move to `(cx + 200, cy - 3)`: `inference-glyph[data-type=horizontal]` visible. Click, Escape, Escape: `data-dof="1"`. Press L, hold Control, click `(cx, cy - 100)`, move to `(cx + 200, cy - 103)`: no `inference-glyph`; click, release Control, Escape, Escape: `data-dof="5"`.
16. E-16. New sketch on XY. Press L, click `(cx, cy)`, move to `(cx + 150, cy)`, type "25": `draw-input` named "Length" has value "25". Enter: a `sketch-dimension` with text "25". Escape, Escape: `data-dof="0"`, `data-state="full"`.
17. E-17. Draw the horizontal line of E-15 and leave the tool. Click `(cx + 100, cy)`: `selection-title` "1 line" and a button "Vertical". Press V: a `notification[data-severity=warning]` reads "Vertical would over-constrain the sketch: it conflicts with Horizontal."; `data-dof="1"`. Click "Replace them": one `constraint-glyph[data-type=vertical]`, none of type horizontal, `data-dof="1"`.
18. E-18. Drop `rect-sketch.ptsm`, double-click the timeline item "Sketch 1": `data-state="full"`. With `s = sketchToScreen` for content (0, 0) to (40, 20): press D, click the top line at `(cx, cy - 10 s)`, click `(cx, cy - 10 s - 30)`, type "40", Enter: the editor shows the `dimension_over_constrained` text in a `role=alert`. Click "Add reference": a new `sketch-dimension` with `data-driving="false"` and text "(40)".
19. E-19. New sketch on XY. Holding Control, draw line A from `(cx - 150, cy + 60)` to `(cx + 150, cy + 60)` and line B from `(cx - 100, cy - 60)` to `(cx + 100, cy - 140)`, leaving the tool after each: `data-dof="8"`. Click A, Shift+click B: the selection inspector has buttons Parallel, Perpendicular, Collinear, Equal. Click "Perpendicular": `data-dof="7"` and one `constraint-glyph[data-type=perpendicular]`.
20. E-20. Drop `underconstrained.ptsm`, double-click "Sketch 1": `data-dof="7"`. With the mapping for content (0, -5) to (55, 5), the line end (30, 0) is a `sketch_point` on hover. Drag it in 10 steps to (30, 10) mm and release: `data-dof="7"`; hovering the new position gives `sketch_point`, the old one does not. ControlOrMeta+Z: hovering the old position gives `sketch_point` again.
21. E-21. Drop `rect-sketch.ptsm`, enter the sketch. Drag on empty space from 30 px above and left of the rectangle's top-left corner to 30 px below and right of its bottom-right corner: `selection-title` "4 lines". Drag from right to left across only the bottom line's middle: `selection-title` "1 line".
22. E-22. In sketch mode press ControlOrMeta+Enter: `data-context="model"`; the browser shows a button "Hide Sketch 1" (the sketch stays visible).

Inspector, timeline and browser:

23. E-23. Drop `parametric.ptsm`; click the body row "Cube": `body-size` "20 × 20 × 20 mm". Run "Parameters"; fill "Expression of size" with "30 mm", Enter: `parameter-value` "30 mm" and `body-size` "30 × 30 × 30 mm". ControlOrMeta+Z: "20 × 20 × 20 mm".
24. E-24. Drop `cube-features.ptsm`; drag `timeline-marker` to the gap between the first and second items: items 2 to 4 have `data-rolled-back="true"`, the marker's `aria-valuenow` is 1, the body row reads "Cube (empty)", and hovering `(cx, cy)` does not give `face`. ControlOrMeta+Z: `aria-valuenow` 4 and no rolled-back items.
25. E-25. Drop `cube-features.ptsm`; focus the marker; ArrowLeft: `aria-valuenow` 3; End: 4 with `aria-valuetext` "At the end"; Home: 1.
26. E-26. Drop `two-bodies.ptsm`; drag "Cylinder 1" to the first gap: item names in order "Cylinder 1", "Sketch 1", "Extrude 1", all ok. Drag "Extrude 1" to the gap between "Cylinder 1" and "Sketch 1": "Extrude 1" has `data-status="error"` and an error notification starts with "Extrude 1 failed."; click its "Undo": the order is "Cylinder 1", "Sketch 1", "Extrude 1" and "Extrude 1" is ok.
27. E-27. Drop `cube-features.ptsm`; right-click "Fillet 1", choose "Suppress": `data-suppressed="true"`, `data-status="suppressed"`. ControlOrMeta+Z: `data-suppressed="false"`.
28. E-28. Drop `fillet-error.ptsm`: "Fillet 1" has `data-status="error"`; `timeline-problems` reads "1 problem"; clicking it opens `problems-panel` listing "Fillet 1" with a message containing "Fillet radius"; clicking "Edit" opens the inspector "Fillet 1" whose `inspector-error` has `role=alert`.
29. E-29. Drop `cube.ptsm`; double-click "Extrude 1": the inspector "Extrude 1" with the Distance input value "20 mm". Fill "30" and wait for `data-preview="shown"`; Escape: the inspector closes and `body-size` (select the body) is "20 × 20 × 20 mm". Double-click "Extrude 1", fill "30", Enter: "20 × 20 × 30 mm". ControlOrMeta+Z: "20 × 20 × 20 mm".
30. E-30. Drop `cube.ptsm`; press E: visible `inspector-field`s have paths `profile`, `extent.kind`, `extent.expr`, `op`; `inspector-more` has `aria-expanded="false"`. Click it: `reverse` and `body` appear. Escape, press E: `aria-expanded="true"`.
31. E-31. Drop `cube.ptsm`, `fitView(page, 'Front')`, press F: the first `selection-field` has `data-active="true"`. Click `(cx, cy - 0.4 H)` (the top front edge): `data-count="1"`; click again: 0; click again: 1. Fill the radius field (features/fillet.md manifest) with "2", Enter: "Fillet 1" ok.
32. E-32. Drop `cube.ptsm`, front fit, click `(cx, cy - 0.4 H)`: `selection-title` "1 edge". Press F: the edges field has `data-count="1"`.
33. E-33. Drop `cube.ptsm`, front fit: hover `(cx, cy)` gives `face`, `(cx, cy - 0.4 H)` gives `edge`, `(cx + 0.4 H, cy - 0.4 H)` gives `vertex`. In "Selection filter" uncheck Edges and Vertices: `(cx, cy - 0.4 H + 2)` gives `face` and `(cx + 0.4 H - 2, cy - 0.4 H + 2)` gives `face`.
34. E-34. Drop `cube.ptsm`, front fit: click `(cx, cy)`: `selection-title` "1 face". Shift+click `(cx, cy - 0.4 H)`: "2 items". Click `(cx, cy + 0.45 H)`: `data-selection-count="0"` and no inspector.
35. E-35. Drop `cube.ptsm`, front fit, double-click `(cx, cy)`: the inspector "Extrude 1".
36. E-36. Drop `cube.ptsm`, front fit. Click "Hide Cube": hover `(cx, cy)` gives `none`. Click "Show Cube": `face`. Click "Lock Cube": `none`. Click "Unlock Cube": `face`.
37. E-37. Without File System Access, drop `cube.ptsm`; click "Color of Cube", click "Red" in `color-popover`; ControlOrMeta+S: the download contains `"color": "#dc2626"`.
38. E-38. Drop `cube.ptsm`; press Tab until a `timeline-item` has focus (at most 30 presses): it is named "Sketch 1". ArrowRight: "Extrude 1" has focus. Enter: the inspector "Extrude 1" opens with focus inside it. Escape: it closes and "Extrude 1" has focus again.
39. E-39. Drop `cube.ptsm`; rename the body to "Box"; ControlOrMeta+Z: the body row reads "Cube"; ControlOrMeta+Shift+Z: "Box". Run "Parameters", click "Add parameter", type "abc", press ControlOrMeta+Z: the body row still reads "Box".
40. E-40. Drop `cube.ptsm`: every visible `button` and `[role=button]` has a non-empty accessible name (`aria-label`, `aria-labelledby` text, or text content).

Viewport:

41. E-41. Drop `cube.ptsm`. Click "Top view": `data-view="top"`; "Front view": `front`; "Front right top corner view": `iso`; "Home view": `iso`. Shift+middle-drag 100 px on the canvas: `custom`.
42. E-42. Click "Orthographic": `aria-pressed="true"` and `data-projection="orthographic"`. Press Shift+P: `perspective`.
43. E-43. Drop `cube.ptsm`, front fit. Middle-drag from `(cx, cy)` to `(cx + 0.6 H, cy)`: `data-camera-target` changed, hover `(cx, cy)` gives `none` and `(cx + 0.6 H, cy)` gives `face`. Press F6, then five wheel events of `deltaY: -100` at `(cx, cy)`: hover `(cx, cy + 0.45 H)` gives `face`.
44. E-44. Run "Navigation: FreeCAD": `data-nav-preset="freecad"`. Front fit; press the middle button, then the left button, move 100 px, release both: `data-view="custom"`. Click "Front view"; middle-drag 100 px: `data-view` stays `front` and `data-camera-target` changes.
45. E-45. Drop `cube.ptsm`, front fit, press Shift+X: `section-popover` visible; fill Offset with "10", Enter: `data-section="on"`; hover `(cx, cy - 0.2 H)` gives `none` and `(cx, cy + 0.2 H)` gives `face`. Shift+X: `data-section="off"` and `(cx, cy - 0.2 H)` gives `face`.
46. E-46. Drop `cube.ptsm`, front fit; ten wheel events of `deltaY: 100`: hover `(cx, cy - 0.39 H)` gives `none`. Press F6: `(cx, cy - 0.38 H)` gives `face` and `(cx, cy - 0.42 H)` gives `none`.

Print and export:

47. E-47. Drop `overhang.ptsm`; click the radio "Print": `data-context="print"`; the toolbar "Print tools" has buttons starting "Printer:", "Overhangs" (`aria-pressed="true"`), "Checks", "Health", "Export 3MF", "Export". `health-badge` reaches `data-state` `warning` or `error` with `data-stale="false"`; `overhang-legend` is visible and contains "°". Click the badge: `health-list` has at least one `health-item[data-check=overhangs]`. Clicking it changes `data-camera-target` and shows `finding-marker`.
48. E-48. Drop `cube.ptsm`, go to Print, click the "Printer:" button: `printer-menu` has at least two `menuitemradio`s. Choose one that is not checked: `status-printer` shows its name. ControlOrMeta+Z: the previous name.
49. E-49. Drop `oversize.ptsm`, go to Print: `health-badge` `data-state="error"` and a `health-item[data-check=bed_fit][data-severity=error]`.
50. E-50. Drop `overhang.ptsm`, go to Print, open "Checks", switch "Overhangs" off: no `health-item[data-check=overhangs]` and no `overhang-legend`. Reload, drop the file, go to Print: the switch is still off.
51. E-51. Drop `overhang.ptsm`, go to Print and wait for a report; click "Model", rename the body: `health-badge` `data-stale="true"`. Click it: `data-stale="false"` and `health-list` visible.
52. E-52. Without File System Access, drop `cube.ptsm`, go to Print, click "Export 3MF": a download `cube.3mf` starting with bytes `50 4b 03 04`, and a notification "Exported cube.3mf."
53. E-53. Open the export popover, choose STL, click "Export": a download `cube.stl` whose length is 84 plus 50 times the little-endian `u32` at offset 80, which is at least 12.
54. E-54. STEP: `cube.step` starting with `ISO-10303-21;`. OBJ: `cube.obj` containing a line starting `v ` and a line starting `f `.
55. E-55. Drop `cube.ptsm`, click "Hide Cube", go to Print, click "Export 3MF": the notification reads "There is no visible body with geometry to export." and no download starts within 2 s.

Themes:

56. E-56. Run "Theme: Dark": `html[data-theme=dark]`; reload: still dark. Run "Theme: Light": light. With `page.emulateMedia({ colorScheme: 'dark' })`, run "Theme: System": dark.

Feature coverage (E-57, one test per row, parametrized). Fields are found by the labels in each feature's manifest table (`docs/spec/features/<type>.md`); selection fields are filled by the clicks listed. Each test ends with Enter and expects a new timeline item with the feature's default label and `data-status="ok"`, plus the listed result.

| Type | Fixture and view | Steps | Result |
| --- | --- | --- | --- |
| `revolve` | `revolve-sketch.ptsm`, front fit | Run Revolve; click `(cx, cy)` (the region); activate the axis field; expand Origin and click "Z axis"; angle `360 deg` | Body 1 `body-size` "40 × 40 × 10 mm" |
| `fillet` | `cube.ptsm`, front fit | E-31 | Cube "20 × 20 × 20 mm" |
| `chamfer` | `cube.ptsm`, front fit | Run Chamfer; click `(cx, cy - 0.4 H)`; distance `2` | Cube "20 × 20 × 20 mm" |
| `shell` | `cube.ptsm`, top fit | Run Shell; click `(cx, cy)` (top face); thickness `2` | Cube "20 × 20 × 20 mm" |
| `hole` | `cube.ptsm`, top fit | Run Hole; fill the essential fields (features/hole.md) for a 5 mm through hole at the top face center, clicking `(cx, cy)` for the placement | Cube "20 × 20 × 20 mm" |
| `box` | new document | Primitive, Box; sizes `10`, `10`, `10`; op New body | Body 1 "10 × 10 × 10 mm" |
| `cylinder` | new document | Primitive, Cylinder; radius `5`, height `10` | Body 1 "10 × 10 × 10 mm" |
| `sphere` | new document | Primitive, Sphere; radius `5` | Body 1 "10 × 10 × 10 mm" |
| `cone` | new document | Primitive, Cone; bottom radius `5`, top radius `0`, height `10` | Body 1 "10 × 10 × 10 mm" |
| `torus` | new document | Primitive, Torus; major radius `10`, minor radius `2` | Body 1 "24 × 24 × 4 mm" |
| `boolean` | `two-bodies.ptsm` | Run Boolean; target: click the row "Cube"; tools: click the row "Peg"; operation Cut | Cube "20 × 20 × 20 mm"; the Peg row reads "Peg (empty)" |
| `move_body` | `cube.ptsm` | Palette "Move body"; body: row "Cube"; translation X `10` | Cube `body-center` "10, 0, 10 mm" |
| `split_body` | `cube.ptsm` | Palette "Split body"; body: row "Cube"; plane: row "XZ plane" | Two body rows with geometry, each "20 × 10 × 20 mm" |
| `mirror` | `offset-cube.ptsm` | Run Mirror; bodies: row "Cube"; plane: row "YZ plane"; op Join | Cube "60 × 20 × 20 mm", center "0, 0, 10 mm" |
| `linear_pattern` | `offset-cube.ptsm` | Pattern, Linear pattern; bodies: "Cube"; direction: row "X axis"; count `3`; spacing `30`; op Join | Cube "80 × 20 × 20 mm" |
| `circular_pattern` | `offset-cube.ptsm` | Pattern, Circular pattern; bodies: "Cube"; axis: row "Z axis"; count `4`; angle `360 deg`; op Join | Cube "60 × 60 × 20 mm" |
| `datum_plane` | `cube.ptsm` | Palette "Datum plane"; plane: row "XY plane"; offset `10` | A Construction row "Datum plane 1" |

`sketch` and `extrude` are covered by E-12 and E-13.

### 6.6 Visual tests

Tagged `@visual`, using `expect(page).toHaveScreenshot` with `animations: 'disabled'` and `maxDiffPixelRatio: 0.002`. Baselines are reviewed and approved by a human at the gate (technical design 16 and 17); a changed baseline needs reviewer approval.

1. V-1. Empty app, light theme. V-2. Empty app, dark theme.
2. V-3. `cube.ptsm` with "Home view", light. V-4. The same, dark.
3. V-5. `rect-sketch.ptsm` in sketch mode (fully constrained colors, dimensions), light. V-6. `underconstrained.ptsm` in sketch mode, dark.
4. V-7. `overhang.ptsm` in Print with the overhang map and legend, light.
5. V-8. `cube.ptsm` with the Extrude inspector open in edit mode, light. V-9. `cube.ptsm` with the front face selected and the top edge hovered, light.

### 6.7 Performance tests

Tagged `@perf`, skipped unless `PARTSMITH_PERF=1`, run on the fixed hardware runner (technical design 16; a regression over 10 percent blocks the merge). Web budgets from section 5; the desktop budgets run in the desktop harness when the host lands.

1. P-1. Load the app once, reload: `ps:usable` is at most 4,000 ms after `performance.timeOrigin`.
2. P-2. `sketch-500.ptsm`: drag `e-drag.end` along a 2 s path with 120 pointer moves: 95th percentile of `ps:sketch-drag` at most 16 ms; no long task over 50 ms.
3. P-3. `part-50.ptsm`: edit the first extrude and type 20 distances, one every 300 ms: median `ps:preview` at most 250 ms.
4. P-4. `part-50.ptsm`: change parameter `base`: `ps:rebuild` at most 3,000 ms.
5. P-5. `part-50.ptsm` opened, New, opened again: the second `ps:open` at most 1,000 ms.
6. P-6. `dense.ptsm`, home view: a 3 s Shift+middle-drag orbit: 95th percentile of `ps:frame` at most 16.7 ms.
7. P-7. `dense.ptsm`: 100 hovers with a still camera: 95th percentile of `ps:pick` at most 1 ms.
8. P-8. `palette-2000.ptsm`: open the palette and type 10 characters: 95th percentile of `ps:palette` at most 8 ms.

## 7. Open issues

1. **Region fingerprint text.** `RegionInfo` in protocol.md's `SketchSolution` has `key`, `area` and `centroid` but no `fp`, which a `RegionRef` (features.md 2.5) needs. This spec has `selection-values.ts` format `region|a={area}|c={x},{y}` with the model's canonical number formatting (features.md open issue 4) until `RegionInfo` carries `fp`; the protocol should add it so the UI never builds fingerprints.
2. **Sketch origin and axis references.** Constraints to the sketch origin and axes (inference snaps, the dimension-as-you-draw angle, symmetric about an axis) need reserved references in sketch.md. `src/sketch/refs.ts` assumes `ORIGIN_POINT`, `X_AXIS` and `Y_AXIS` exist there.
3. **Dimension label positions.** Nothing in the drafts stores a dimension's label position. The UI computes `defaultLabelPosition` and uses a stored position if sketch.md adds one; dragging labels is later.
4. **Structured fixes and field paths in diagnostics.** `Diagnostic` is severity, code and message. Without a field path the inspector cannot attach a feature error to its field, and without a suggested value it cannot offer one-click fixes such as "Use 2.4 mm". M1 offers navigational fixes ("Edit", "Undo"). Suggested protocol addition: `field: Option<String>` and `suggestion: Option<{ path, value, label }>`.
5. **Feature that made a face.** Double-click on a face derives the feature from the first segment of the face's element name (features.md 3.9). An explicit `feature` field in `ElementDescriptor` would be more robust.
6. **Rolling back before the first feature.** `Document.rollback` is the last active feature, with `None` meaning the end, so "no feature active" is not representable. The marker's minimum position is therefore 1.
7. **Renaming a parameter.** model.md says the engine composes a rename with rewritten expressions, but the protocol only offers `apply_edits` with `update_parameter`, which does not rewrite them. This spec sends `update_parameter`; expressions using the old name then fail (with Undo). engine.md or protocol.md should define rename semantics.
8. **Saving marks the document clean before the file is written.** `save_document` clears `dirty` before the UI writes the bytes. The UI chooses the target first and tracks `localUnsaved` after a failed write; a separate "mark saved at revision" step after a successful write would be cleaner.
9. **Desktop host pieces.** The Tauri transport reads `window.__TAURI__` (requires `app.withGlobalTauri: true`) to avoid adding `@tauri-apps/api`; the desktop `FileService` needs the Tauri dialog and fs plugins; and the desktop transport reuses the worker host's queue and superseding logic, which protocol.md keeps inside `createWorkerHost`, so that logic should be factored into its own function. These belong in the desktop spec and need human review as dependencies.
10. **Session-only view state.** Sketch, datum and origin visibility and body locks are not saved, because the document has no fields for them. Persisting them is a format change (ADR and human review).
11. **When print analysis runs.** The technical design runs checks after every recompute. With one web worker, `analyze_print` would delay edits, so this spec runs it automatically only in the Print context and shows a stale badge elsewhere. Revisit with desktop background analysis (protocol.md "later").
12. **Check toggles.** `analyze_print` always runs every check, so the UI's switches only hide findings. An optional check list on the command would save work.
13. **Print profile fields.** The overhang limit, bed size and technology come from `PrinterProfile` in print.md, which was not available; this spec uses the overhang findings' `limit` first and names the profile fields generically. E-48 assumes print.md ships at least two built-in profiles.
14. **Manifest types.** The protocol carries `FeatureManifest` as untyped JSON (protocol.md open issue 4), so the UI declares a mirror type with runtime validation in `inspector/manifest.ts`, against the module rule that the UI uses generated types. A typed schema definition would remove the mirror.
15. **Hole placement value.** The `sketch_point` selection value shape is defined by features/hole.md, which was not available.
16. **Feature insertion at the marker.** model.md says the engine commits `add_feature` after the rollback feature followed by `set_rollback`. This spec has the UI send both edits inside the transaction. If the engine does it on its own, the UI must stop sending `set_rollback`.
17. **Design tokens need sign-off.** The accent (violet, chosen so selection stays distinct from the blue of under-constrained sketch geometry), the gray scale and the body swatches are design-system decisions, which technical design 17 reserves for human review.
18. **Inference hold key on macOS.** This spec suppresses inference with Cmd on macOS (Fusion's convention) because Ctrl-click is a secondary click there.
19. **Dragging sketch curves.** `drag_sketch` pins one point, so M1 drags points only; translating lines and resizing circles by drag needs multi-point drags.
20. **Untitled document units.** New documents use `mm`; a default-units setting is later.
21. **Fixtures depend on unfinished JSON.** The e2e fixtures need sketch.md, print.md and the per-feature specs; they are written with `partsmith fmt` once those exist, and E-57's field labels come from those manifests.
