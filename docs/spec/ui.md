# UI specification

Status: M1 spec. Package: `ui/` (React 19, TypeScript strict, Vite, Tailwind CSS 4, zustand, lucide-react, clsx; linted with oxlint and formatted with oxfmt). Read with [architecture.md](../design/architecture.md) (section "UI"), [technical design](../design/technical-design.md) sections 7, 8, 11, 13 and 14, ADRs [0002](../adr/0002-react-for-the-ui.md), [0006](../adr/0006-protocol-framing-and-schema.md), [0007](../adr/0007-webgl2-renderer-first.md), [0009](../adr/0009-emscripten-c-abi-bridge.md) and [0013](../adr/0013-oxc-tooling-and-theme-stylesheet.md), the guidelines in [docs/guidelines/](../guidelines/README.md) ([TypeScript](../guidelines/typescript.md), [Tailwind CSS](../guidelines/tailwind-css.md), [frontend design](../guidelines/frontend-design.md) and the Partsmith design direction), and the specs this one consumes: [protocol.md](protocol.md) (wire, commands, events, worker host, frame codec), [features.md](features.md) (manifests, shared param types, labels, messages), [model.md](model.md) (edits, ids, labels) and sketch.md and print.md (sketch and printer JSON).

This spec is the definition of done for the UI in M1. Test authors write the tests in section 6 from it without seeing the implementation; implementers make them pass without weakening them. Where this spec quotes another spec's JSON or names, the other spec is authoritative and a mismatch is a bug in this spec.

## 1. Purpose and scope

The UI is the single front end for web and desktop. It renders the model, edits it with the mouse and keyboard, and talks to the core only through protocol frames (architecture.md: "The UI depends only on the protocol"). It never evaluates geometry, never parses `.ptsm` text and never constructs a face, edge or vertex reference itself: references are copied from protocol data (`pick_reference` results, sketch solutions), and the typed wrappers around them (`PlaneRef`, `AxisRef`, `ProfileRef`) are assembled exactly as features.md section 2.5 defines. Display arithmetic on values the core reports is not geometry evaluation: a diameter from a descriptor's radius (3.8) or the point where the pointer's ray meets a reported face plane (3.7, as sketch mode does for its plane, 3.16.3).

### 1.1 In M1 (the "M1 slice")

- App shell: top bar, collapsible browser, viewport, timeline, floating inspector, right side panel, status bar, notifications, light and dark themes from one role-to-color mapping toggled with the `dark` class on the root element (2.16), UI scale 100 to 300 percent.
- Three contexts (Model, Sketch, Print), each with one short toolbar, and one command registry shared by toolbars, the command palette and keyboard shortcuts.
- Command palette (Ctrl/Cmd+K) with fuzzy search over commands, features, bodies, parameters, recent files and settings, showing each command's shortcut and description, finding commands by their Fusion and FreeCAD names, and browsable by category with an empty query.
- Timeline: rollback marker drag, drag to reorder, suppress, status icons, double-click to edit, rename, delete, basic Problems list.
- Browser: origin, bodies, sketches, datum planes and parameters, with eye, lock and color controls.
- Schema-driven inspector for every catalog feature: essential fields and a "More" toggle from manifest hints (plus the inline Reverse of 2.12), starting values that work on printed parts, expression inputs with live validation, unit display and parameter name completion, selection fields that capture viewport and browser picks, live preview through protocol transactions, undo inside a command, Enter confirms, Esc cancels.
- Parameters panel.
- WebGL2 viewport: matcap-style shaded faces, screen-space anti-aliased edges, ID-buffer picking of faces, edges and vertices with hover highlight, selection filter, section plane, analysis color overlay, ground grid, view cube, orbit, pan and zoom with Fusion and FreeCAD navigation presets (both also orbit with a right drag), a right-click context menu, fit to view, perspective and orthographic projection.
- Sketch mode for points, lines, circles and arcs: tools line/polyline, rectangle (2-point and center), circle (center, radius), arc (3-point), point, construction toggle; constraint tools coincident, point on object, horizontal, vertical, parallel, perpendicular, tangent, collinear, concentric, equal, symmetric, midpoint, fix; dimension tool for distance, horizontal and vertical distance, radius, diameter and angle with expression entry; auto-constraint inference while drawing (coincident, horizontal, vertical, tangent, midpoint) with a hold key to suppress it; dimension as you draw; step prompts; DOF colors and DOF count; constraint glyphs; point drag solved by the core at up to 60 fps; profile check display.
- Print context: printer profile picker, check toggles, overhang color map with legend, print check badge and list with zoom to problem, bed outline, and Lay on face for print checks and 3MF and STL export.
- Guidance inside the one interface (open issue 15): the empty-document first step, descriptions and field help on hover, availability reasons before a click, next steps and measurements in the selection inspector, and the keyboard and mouse reference (F1).
- Export to 3MF, STL (binary), OBJ and STEP.
- Files: new, open, save, save as with the File System Access API and a download fallback, drag-and-drop open, recent files and a recovery copy in IndexedDB.
- Feature types covered through the generic inspector: `sketch`, `extrude`, `revolve`, `hole`, `box`, `cylinder`, `sphere`, `cone`, `torus`, `fillet`, `chamfer`, `shell`, `boolean`, `move_body`, `split_body`, `mirror`, `linear_pattern`, `circular_pattern`, `datum_plane` (features.md section 3.13). The `import` type has no command in M1 (2.10); its timeline items are shown and managed like any other feature.
- Accessibility: WCAG 2.2 AA text and non-text contrast, full keyboard operation of every panel, landmarks, live regions.
- Worker and Tauri transports, the core client, and the generated protocol types.

### 1.2 Later (not in M1)

- Onboarding tutorial and looping tool-tip animations. The "Coming from Fusion / FreeCAD" mapping is part of M1 as the palette's tagged keywords (2.10), not as a separate panel.
- Right-click radial marking menu in the viewport (M1 has a plain context menu with the same items, 3.13).
- 3Dconnexion SpaceMouse.
- Code panel (CodeMirror 6), script features, Python export UI and the Customizer panel.
- Repair tools beyond the Problems list, the Re-pick action and the repair marker of 3.7 and 4.2 (ghosted shape of the lost element, candidate lists, one-click re-pick on the canvas).
- On-canvas handles for feature values (technical design 13, principle 1): an arrow for Extrude distance, Hole depth, Datum plane offset, Shell thickness and Fillet and Chamfer size, an arc for Revolve angle and a triad for Move body, each with a value chip that is the field's `ExpressionInput`. Dragging writes plain numbers in `stepExpression` steps (Shift coarse, Alt fine) through the preview `LatestOnly`, dragging an extrude arrow through zero toggles Reverse, and a field holding an expression that is not a plain number shows a dimmed handle that does not drag. Needs a handle anchor (point and direction per field path) from the core with the preview.
- WebGPU render backend.
- Multi-document tabs.
- Navigation presets SolidWorks, Blender and Onshape; trackpad gestures beyond pinch zoom and the secondary-button drag of 3.13 (two-finger scroll to pan or orbit); touch-optimized layout.
- FreeCAD shortcut preset, user shortcut rebinding and user-pinned toolbar tools.
- Remaining sketch tools (3-point and rounded rectangle, 2-point, 3-point and tangent circle, center and tangent arc, tangent-arc toggle in the line tool, conics, splines, polygon, slots, text, image, DXF and SVG import), modify tools (trim, extend, split, sketch fillet and chamfer, offset, mirror, patterns, move and copy) and reference geometry (project, intersect, carbon copy).
- Remaining constraints (smooth, block, arc length, angle to an axis as its own type, Snell's law, B-spline constraints).
- Dragging sketch curves (translating a line, resizing a circle); M1 drags points only.
- Window and crossing box selection in the Model context (M1 has it in sketch mode only).
- Keyboard picking of faces, edges and vertices.
- Features outside the slice (sweep, loft, helix, rib, emboss, thread, draft, scale, direct edits, derive, surfaces, mesh bodies, configurations), components, joints and assemblies.
- Measure tool, distances and angles between two selected items (the core's two-target `measure`, protocol.md 1.2), and mass properties beyond volume.
- Import UI (STEP, meshes, `.scad`, `.FCStd`, DXF, SVG) and `.ptsz` bundles.
- Print-aware feature presets, orientation tools beyond Lay on face (auto-orient, saving the orientation in the document), custom printer profiles, slicer handoff and watch mode, resin tools, heat maps for checks other than overhangs, multi-plate arrangement.
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
    index.css                font import, Tailwind import, dark variant (ADR 0013; nothing else, 2.16)
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
      superseding.ts         isSuperseded (protocol.md 2.11), shared by both transports
      transport.ts           Transport, createWorkerTransport, chooseTransport
      tauri-transport.ts     createTauriTransport
      client.ts              createCoreClient
      api.ts                 CoreApi, createCoreApi (the only module that names commands)
      events.ts              CoreEvent, decodeEvent (the only module that names events)
      queue.ts               createMutationQueue, createLatestOnly
      ids.ts                 generateId
      labels.ts              featureLabel, nextBodyName, originPlaneName
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
      features.ts            commands generated from the feature catalog, FEATURE_REQUIREMENTS
      keywords.ts            COMMAND_KEYWORDS
      descriptions.ts        FEATURE_DESCRIPTIONS
      component-keys.ts      COMPONENT_KEYS (the component-key table of 3.5 as data)
      toolbars.ts            TOOLBARS
      icons.ts               FEATURE_ICONS, constraint glyph icons
    inspector/
      model.ts               buildInspectorModel, defaultParams, setParam, getParam
      start-values.ts        START_VALUES
      help.ts                FIELD_HELP
      suggestions.ts         suggestedCommands
      measurements.ts        selectionMeasurements
      selection-values.ts    selectionValue, acceptsTarget, itemsFromValue
      placement.ts           placeInspector
      Inspector.tsx, fields/*.tsx
    expression/
      evaluate.ts            convertForField
      format.ts              formatNumber, formatQuantity, formatSize, formatPoint
      step.ts                stepExpression, isPlainNumber
      span.ts                utf16Range
      ExpressionInput.tsx, useExpression.ts
    viewport/
      math.ts                Vector3, Matrix4, Quaternion helpers
      camera.ts              Camera and camera functions
      navigation.ts          NAVIGATION_PRESETS, gestureFor, navigationLegend
      grid.ts                gridSpacing
      menu.ts                viewportMenuItems
      backend.ts             RenderBackend and its types
      webgl2/                createWebGl2Backend and shaders
      picking.ts             encodePickId, decodePickId, choosePick
      scene.ts               Scene, createScene
      controller.ts          ViewportController, ViewportHandle
      overlay.ts             positionElement, orientElement (runtime positions, 3.12)
      Viewport.tsx, ViewCube.tsx, ViewportControls.tsx, SectionPopover.tsx, EmptyState.tsx, ViewportMenu.tsx
    sketch/
      refs.ts                sketch origin and axis references (sketch.md)
      geometry.ts            circleThroughThreePoints, tessellateArc, distance helpers
      recipes.ts             tool outputs
      inference.ts           infer
      constraints.ts         applicableConstraints, constraintFor
      dimensions.ts          dimensionTypeFor, measuredValue, defaultLabelPosition
      session.ts             SketchToolId, SketchChange, SketchCommitResult, SketchStep, sketchStepHint,
                             outlineProblems, the internal sketch controller
      SketchOverlay.tsx, SketchHeader.tsx, DrawInputs.tsx, DimensionEditor.tsx
    timeline/ Timeline.tsx
    browser/ Browser.tsx
    parameters/ ParametersPanel.tsx
    print/ PrinterMenu.tsx, ChecksPopover.tsx, HealthList.tsx, OverhangLegend.tsx,
           printer.ts (printerDetail, printerButtonText)
    export/ ExportPopover.tsx, status.ts (exportStatus)
    problems/ ProblemsPanel.tsx
    keys/ KeysPanel.tsx
    files/
      file-service.ts        FileService, createWebFileService
      recent.ts              RecentFilesStore, createIndexedDbRecentFiles, createMemoryRecentFiles
      recovery.ts            RecoveryStore, createIndexedDbRecovery, createMemoryRecovery
    settings/ settings.ts    Settings, loadSettings, saveSettings
    theme/
      roles.ts               the role-to-color mapping, palette hex values, viewport colors,
                             font and UI scale classes (2.16)
      theme.ts               resolveTheme, applyTheme, applyUiScale, toggledTheme
    layout/ TopBar.tsx, StatusBar.tsx, SidePanel.tsx, Notifications.tsx, DocumentMenu.tsx
    components/              Button, IconButton, Menu, Popover, Tooltip, Switch, SegmentedControl, TextField
    performance/marks.ts     TIMING_NAMES, markTiming, measureTiming
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
- Runtime dependencies are exactly those in `ui/package.json` (`react`, `react-dom`, `zustand`, `lucide-react`, `clsx`). Menus, popovers, tooltips and the fuzzy matcher are written in-house; popovers use the native Popover API. Icons come from `lucide-react` only; there are no charts in M1. Motion uses Tailwind transition utilities only; the `motion` library is not a dependency (adding it, like any dependency, needs human review, CLAUDE.md).
- Hand-written code never redeclares a type that exists in `src/generated/protocol.ts`. The only mirror types are for protocol values the schema carries as untyped JSON (feature params schemas only), and they are validated at runtime (2.12).
- TypeScript follows [typescript.md](../guidelines/typescript.md): `type`, never `interface` (oxlint's `typescript/consistent-type-definitions` enforces it); names are words, not abbreviations (`documentHandle`, `millimetersPerPixel`, `Rectangle`), with single letters only for `i` and the coordinates `x`, `y`, `z`. Names that come from the protocol schema or other specs keep their spelling (`DocHandle`, `PointRef`, `params`, `expr` in JSON). Every signature in this spec follows these rules.
- Styling follows [tailwind-css.md](../guidelines/tailwind-css.md): Tailwind utility classes only, written inline on the element; conditional classes with `clsx` imported from `clsx/lite` (never template literals or `+`); no `style` prop or attribute, with the one documented exception for runtime positions (3.12); no `px` values (Tailwind's rem scale; arbitrary values only where this spec names them, never in px); no CSS beyond `src/index.css` (2.16). Colors, the two font families and the UI scale come from the role mapping in `src/theme/roles.ts` (2.16); components never name a palette step themselves. Layout classes are not abstracted: repeated layout classes stay inline.

### 2.2 Generated protocol types

`ui/scripts/generate-protocol-types.mjs`, run by `pnpm -C ui generate:protocol`:

1. Reads `../docs/format/protocol.schema.json` relative to `ui/`. A missing or unparsable file exits with status 1 and prints `Cannot read docs/format/protocol.schema.json: <reason>`.
2. Compiles it with `json-schema-to-typescript` `compile(schema, 'Protocol', options)` where `options` are `{ bannerComment: '/* Generated from docs/format/protocol.schema.json by ui/scripts/generate-protocol-types.mjs. Do not edit. */', unreachableDefinitions: true, additionalProperties: false, strictIndexSignatures: true, declareExternallyReferenced: true, enableConstEnums: false, format: false }`. The output is not reformatted (oxfmt ignores `src/generated`, and `format: false` keeps the formatter bundled in `json-schema-to-typescript` out of the build). Every `$defs` entry becomes an exported type with its definition name (`Command`, `CommandResult`, `Event`, `Message`, `MeshBuffers`, `SketchSolution`, `PrintReport`, ...).
3. Appends `export const PROTOCOL_VERSION = <n> as const` where `n` is the schema's top-level `x-protocol-version` (protocol.md 2.9). A schema without it exits with status 1.
4. Writes `ui/src/generated/protocol.ts` with LF line endings and one trailing newline. The output is a pure function of the schema file: running it twice gives byte-identical files.

The generated file is checked in. CI runs `pnpm -C ui generate:protocol && git diff --exit-code ui/src/generated` and fails on drift. `src/generated` is excluded from oxlint and oxfmt (`ignorePatterns` in `ui/.oxlintrc.json` and `ui/.oxfmtrc.json`, already configured), so the `interface` declarations `json-schema-to-typescript` emits are machine output outside the `type` rule, which applies to hand-written code. CI runs `pnpm -C ui lint` (`oxlint --deny-warnings`, then `oxfmt --check`), `pnpm -C ui typecheck` and `pnpm -C ui test` on every change; type-aware checks come from `tsc -b` (ADR 0013).

### 2.3 Frame codec, bridge and worker host

`src/core/frame.ts`, `src/core/bridge.ts`, `src/core/worker-host.ts` and `src/core/core.worker.ts` are specified in protocol.md sections 2.10 and 2.11 (signatures, superseding rules, ready and fatal messages, tests 53 to 66). This spec adds only:

- `core.worker.ts` constructs the host with `load = () => import(/* @vite-ignore */ `${import.meta.env.BASE_URL}core/partsmith_core.mjs`).then((module) => module.default()).then(wrapEmscriptenModule)`, `post = (message, transfer) => self.postMessage(message, transfer)` and `schedule = (task) => setTimeout(task, 0)`, and forwards every `message` event to `host.receive`.
- The worker is created with `new Worker(new URL('./core.worker.ts', import.meta.url), { type: 'module', name: 'partsmith-core' })`.

### 2.4 Transports

`src/core/transport.ts`:

```ts
/** What the core reported when it became ready. */
export type CoreReady = {
  readonly protocolVersion: number
  readonly engineVersion: string
}

/** A byte pipe to the core: one request frame in, its response frame out. */
export type Transport = {
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
export function createWorkerTransport(options?: { createWorker?: () => Worker; startTimeoutMilliseconds?: number }): Transport

/** Picks the Tauri transport when the Tauri global is present, else the worker transport. */
export function chooseTransport(environment: { readonly __TAURI__?: TauriGlobal }): Transport
```

Worker transport behavior:

1. `start()` creates the worker and resolves with `{ protocolVersion, engineVersion }` from the worker's `ready` message. A `fatal` message, a worker `error` event, or no `ready` within `startTimeoutMilliseconds` (default 30,000) rejects with `CoreFatalError`; the timeout message is "The geometry core did not start within {startTimeoutMilliseconds / 1000} seconds." ("... within 30 seconds." by default).
2. `send(request)` posts `{ type: 'call', frame: request }` with `[request]` as the transfer list and appends a resolver to a FIFO list. Each `{ type: 'frame', frame }` message resolves the oldest resolver (the worker answers in request order, protocol.md 2.11 rule 5).
3. A `fatal` message, `error` event or `messageerror` event after start rejects every pending resolver with `CoreFatalError(message)`, notifies the fatal listeners once, and makes later sends reject immediately.
4. `stop()` terminates the worker and rejects pending sends with `CoreFatalError('The geometry core was stopped.')`.

`src/core/tauri-transport.ts`:

```ts
/** The part of `window.__TAURI__` the UI uses (requires `app.withGlobalTauri` in the desktop config). */
export type TauriGlobal = {
  readonly core: { invoke(command: string, payload: ArrayBuffer): Promise<ArrayBuffer> }
}
export function createTauriTransport(tauri: TauriGlobal): Transport
```

Tauri transport behavior (protocol.md 2.12):

1. `start()` sends an `engine_info` request frame (request id 0) through `invoke('call', frame)`, checks that `protocol_version` equals the generated `PROTOCOL_VERSION`, and resolves with the versions. A mismatch rejects with `CoreFatalError('This app and its core speak different protocol versions. Reinstall Partsmith.')`.
2. `send(request)` queues the frame and applies the same FIFO, one-at-a-time and superseding rules as the worker host (protocol.md 2.11 rules 3 and 4), deciding superseding with the shared `isSuperseded` (protocol.md 2.11); a superseded request resolves with the synthesized `superseded` error frame. Each step calls `invoke('call', frame)` with the frame as the raw body and resolves with the returned `ArrayBuffer`.
3. An `invoke` rejection resolves the pending send with an error response frame for that request (`internal_error`, the rejection message), built with the same encoder; the desktop core lives in the app process, so `onFatal` is never called.

`chooseTransport` returns `createTauriTransport(environment.__TAURI__)` when `environment.__TAURI__?.core.invoke` is a function, else `createWorkerTransport()`.

### 2.5 Core client

`src/core/client.ts`:

```ts
import type { Command, CommandResult, ErrorCode } from '../generated/protocol'

/** A successful response with the frame's buffers (views alias the frame). */
export type Reply = {
  readonly result: CommandResult
  readonly buffers: readonly (Uint8Array | Uint32Array | Float32Array)[]
}

/** A protocol error response. `message` is the core's text, shown verbatim. */
export class CoreError extends Error {
  readonly code: ErrorCode
  readonly editIndex: number | null
  /** UTF-8 byte offsets of an `expression_error`'s span when the core reports one (2.13, open issue 9), else null. */
  readonly span: { readonly start: number; readonly end: number } | null
}

export type CoreClient = {
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
export type MutationQueue = {
  run<Result>(task: () => Promise<Result>): Promise<Result>
  readonly pending: number
}
export function createMutationQueue(): MutationQueue

/** Sends at most one input at a time; while one is in flight, newer inputs replace the waiting one. */
export type LatestOnly<Input> = {
  submit(input: Input): void
  /** Drops the waiting input and ignores the result of the one in flight. */
  cancel(): void
  /** Sends a waiting input now if idle and resolves when nothing is waiting or in flight. */
  flush(): Promise<void>
  readonly busy: boolean
}
export function createLatestOnly<Input, Output>(
  send: (input: Input) => Promise<Output>,
  onResult: (output: Output, input: Input) => void,
  onError: (error: unknown, input: Input) => void,
): LatestOnly<Input>
```

`createLatestOnly` semantics: when an in-flight send settles and a newer input is waiting, the settled result is discarded (neither callback runs) and the waiting input is sent; otherwise the matching callback runs. `flush()` resolves after the last callback has run, so a caller that awaits it never overtakes the last submitted input. A task that fails in `MutationQueue` rejects its own promise and does not block later tasks.

Every document-mutating `CoreApi` call (apply, rename parameter, transaction, focus, undo, redo, open, new, close, end of drag, save, mark saved) runs through one `MutationQueue` per app, so the UI's order of edits is the core's order on both transports. Inspector previews and live rollback-marker drags (unlabeled `apply_edits` inside a transaction) go through a `LatestOnly` whose `send` enqueues on that queue, so they stay ordered with other mutations and coalesce while waiting. Sketch drags, sketch probes, evaluations, measurements (3.8) and print analysis change nothing in the document; they use `LatestOnly` outside the queue.

### 2.6 Core API

`src/core/api.ts` wraps every command the M1 UI uses. Method names map one to one to protocol.md section 2.5 commands; argument and result shapes are the generated types.

```ts
import type {
  CatalogEntry, DocHandle, Edit, ElementDescriptor, ElementKind, ElementRef, ExportFormat, Diagnostic,
  Health, Pick, PointRef, PrinterProfile, Resolution, SketchParams, SketchSolution,
  Units, Dimension, Orientation, Measurement, MeasureTarget,
} from '../generated/protocol'

export type CoreApi = {
  engineInfo(): Promise<{ protocolVersion: number; engineVersion: string; formatVersion: number; kernel: string }>
  featureCatalog(): Promise<readonly CatalogEntry[]>
  listPrinterProfiles(): Promise<readonly PrinterProfile[]>
  newDocument(units?: Units): Promise<DocHandle>
  openDocument(text: string): Promise<{ documentHandle: DocHandle; migratedFrom: number | null }>
  saveDocument(documentHandle: DocHandle): Promise<{ text: string; revision: number }>
  markSaved(documentHandle: DocHandle, revision: number): Promise<void>
  closeDocument(documentHandle: DocHandle): Promise<void>
  applyEdits(documentHandle: DocHandle, label: string | null, edits: readonly Edit[]): Promise<number>
  renameParameter(documentHandle: DocHandle, parameter: string, name: string): Promise<number>
  beginTransaction(documentHandle: DocHandle, label: string): Promise<void>
  setFocus(documentHandle: DocHandle, feature: string | null): Promise<void>
  commitTransaction(documentHandle: DocHandle): Promise<{ revision: number; recorded: boolean }>
  cancelTransaction(documentHandle: DocHandle): Promise<number>
  undo(documentHandle: DocHandle): Promise<{ revision: number; label: string }>
  redo(documentHandle: DocHandle): Promise<{ revision: number; label: string }>
  solveSketch(documentHandle: DocHandle, feature: string, candidate?: SketchParams): Promise<SolvedSketch | null>
  dragSketch(documentHandle: DocHandle, feature: string, point: PointRef, to: readonly [number, number]): Promise<SolvedSketch | null>
  endSketchDrag(documentHandle: DocHandle, feature: string, commit: boolean): Promise<number>
  evaluate(documentHandle: DocHandle, expression: string): Promise<{ value: number; dimension: Dimension }>
  listElements(documentHandle: DocHandle, body: string, kind?: ElementKind): Promise<readonly ElementDescriptor[]>
  pickReference(documentHandle: DocHandle, pick: Pick): Promise<ElementDescriptor>
  resolveReferences(documentHandle: DocHandle, references: readonly ElementRef[]): Promise<readonly Resolution[]>
  measure(documentHandle: DocHandle, target: MeasureTarget): Promise<Measurement>
  analyzePrint(documentHandle: DocHandle, options: { profile?: string; orientation: Orientation }): Promise<Health | null>
  exportBodies(documentHandle: DocHandle, format: ExportFormat, bodies?: readonly string[], orientation?: Orientation): Promise<{ files: readonly ExportedFile[]; warnings: readonly Diagnostic[] }>
}

/** A sketch solution with its region fill buffers (2.7). */
export type SolvedSketch = { readonly solution: SketchSolution; readonly display: SketchDisplayData | null }

/** An export file with its bytes resolved from the frame buffer. */
export type ExportedFile = { readonly name: string; readonly mediaType: string; readonly bytes: Uint8Array }

export function createCoreApi(client: CoreClient): CoreApi
```

Rules:

- `solveSketch`, `dragSketch` and `analyzePrint` resolve with `null` when the core answers `superseded`; every other method rejects with that `CoreError`.
- `exportBodies` copies each file's `u8` buffer into a new `Uint8Array` (the frame is not retained); it sends `orientation` only when given (3.18).
- `measure` sends `{targets: [target]}`: the core takes exactly one target in M1 (protocol.md command 25).
- The UI uses no other command in M1. `get_document`, `set_tessellation`, `put_printer_profile`, `import_step` and `export_python` are unused.

### 2.7 Core events

`src/core/events.ts` converts protocol events into UI events. Buffer indices are replaced by typed array views (no copies).

```ts
export type CoreEvent =
  | { readonly type: 'document_changed'; readonly documentHandle: DocHandle; readonly revision: number; readonly document: Document; readonly dirty: boolean; readonly undo: string | null; readonly redo: string | null; readonly transactionOpen: boolean; readonly transaction: string | null; readonly focus: string | null }
  | { readonly type: 'recomputed'; readonly documentHandle: DocHandle; readonly revision: number; readonly features: readonly FeatureStatus[]; readonly parameters: readonly ParameterValue[]; readonly diagnostics: readonly Diagnostic[] }
  | { readonly type: 'sketch_solved'; readonly documentHandle: DocHandle; readonly revision: number; readonly solution: SketchSolution; readonly display: SketchDisplayData | null }
  | { readonly type: 'datum_plane'; readonly documentHandle: DocHandle; readonly revision: number; readonly feature: string; readonly placement: Placement | null }
  | { readonly type: 'body_mesh'; readonly documentHandle: DocHandle; readonly revision: number; readonly body: string; readonly layer: 'model' | 'preview'; readonly meshId: number; readonly mesh: BodyMeshData | null }
  | { readonly type: 'print_health'; readonly documentHandle: DocHandle; readonly revision: number; readonly report: PrintReport; readonly overhang: Readonly<Record<string, Float32Array>> }

/** Mesh buffers of one body in one layer (protocol.md MeshBuffers, 3.4 invariants). */
export type BodyMeshData = {
  readonly positions: Float32Array
  readonly normals: Float32Array
  readonly indices: Uint32Array
  readonly faceIds: Uint32Array
  readonly edgePositions: Float32Array
  readonly edgeIds: Uint32Array
  readonly vertexPositions: Float32Array
  /** Display vertex id of each vertex marker (the `index` of a vertex pick). */
  readonly vertexIds: Uint32Array
  readonly faceCount: number
  readonly edgeCount: number
  readonly vertexCount: number
  readonly boundingBox: BoundingBox
}

/** Region fill buffers of a sketch solution (protocol.md SketchDisplay). */
export type SketchDisplayData = {
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
  /** The latest message of the polite live region, such as "Undid Rename" (3.20), or null. */
  readonly announcement: Announcement | null
  /** The hint about a refused pick, shown for 3 s (3.7, 3.16.1), or null. */
  readonly pickHint: PickHint | null
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
export type MeshSummary = { readonly meshId: number; readonly boundingBox: BoundingBox; readonly faceCount: number; readonly edgeCount: number; readonly vertexCount: number }
export type FileReference = { readonly name: string; readonly handle: FileHandle | null }

export type SelectionItemKind =
  | 'face' | 'edge' | 'vertex' | 'body' | 'feature' | 'parameter'
  | 'sketch_region' | 'sketch_curve' | 'sketch_point' | 'sketch_constraint'
  | 'origin_plane' | 'origin_axis' | 'origin_point' | 'datum_plane'

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
  | { readonly kind: 'entity'; readonly sketch: string; readonly entity: string; readonly entityType: 'point' | 'line' | 'circle' | 'arc' }
  | { readonly kind: 'point'; readonly sketch: string; readonly point: PointRef }
  | { readonly kind: 'constraint'; readonly sketch: string; readonly constraint: string }
  | { readonly kind: 'region'; readonly sketch: string; readonly region: RegionInfo }
  | { readonly kind: 'origin'; readonly sketch: string; readonly which: 'point' | 'x_axis' | 'y_axis' }

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
  /** Body this command creates (for op new, or for a body-creating target, 2.12) and the field it fills, if any. */
  readonly newBody: { readonly id: string; readonly name: string; readonly path: 'body' | 'target' } | null
  /** The helper sketch of "Placing holes on a face" (3.7), or null. */
  readonly helper: HelperSketch | null
  /** Paths the user changed in this command, least recent first, each listed once at its latest change (3.7: the value field, the cut direction). */
  readonly changedPaths: readonly string[]
  /** The state before each user change in this command, newest last (3.7, command history). */
  readonly history: readonly CommandStep[]
  /** Changes undone with Mod+Z, for Mod+Shift+Z, newest last. */
  readonly future: readonly CommandStep[]
  readonly preview: 'none' | 'pending' | 'shown' | 'failed'
}

/** A sketch record the command writes for holes placed on a face (3.7): one point entity per hole center, no constraints. */
export type HelperSketch = { readonly id: string; readonly params: SketchParams; readonly applied: boolean }

/** The command state before one user change, with the label of the field the change touched. */
export type CommandStep = {
  readonly params: Readonly<Record<string, unknown>>
  readonly helper: HelperSketch | null
  readonly activeField: string | null
  readonly field: string
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
  /** Camera before enterSketch moved it, for leaving the sketch (3.16.11); null during plane picking. */
  readonly cameraBefore: Camera | null
}

export type PrintState = {
  readonly report: PrintReport | null
  /** Revision the report describes; the badge is stale when it differs from the document revision. */
  readonly reportRevision: number | null
  /** Orientation the report was computed with; the badge is also stale when it differs from `orientation`. */
  readonly reportOrientation: Orientation | null
  readonly running: boolean
  readonly overhangMap: boolean
  readonly focusedFinding: FindingKey | null
  /** Session print orientation set by Lay on face (3.17); null prints as modeled. */
  readonly orientation: Orientation | null
  /** True while Lay on face waits for a face. */
  readonly pickingFace: boolean
}

/** A finding of the print report: the `BodyPrint.body` id, the check, and the finding's index in that check's `findings`. */
export type FindingKey = { readonly body: string; readonly check: string; readonly index: number }

export type ViewState = {
  readonly projection: 'perspective' | 'orthographic'
  readonly named: StandardView | 'custom'
  readonly section: SectionState | null
  readonly filter: { readonly body: boolean; readonly face: boolean; readonly edge: boolean; readonly vertex: boolean }
  /** Session-only visibility of sketches, datum planes and origin items, by selection key. */
  readonly sessionVisible: Readonly<Record<string, boolean>>
  /** Session-only locked bodies (not pickable). */
  readonly locked: readonly string[]
  /** Session-only: whether the ground and sketch grid is drawn (3.12). */
  readonly grid: boolean
}
export type SectionState = { readonly plane: 'XY' | 'XZ' | 'YZ'; readonly offset: string; readonly flipped: boolean }

export type PanelsState = {
  readonly browserOpen: boolean
  readonly timelineCollapsed: boolean
  readonly paletteOpen: boolean
  readonly side: 'parameters' | 'health' | 'problems' | 'keys' | null
  readonly inspectorPosition: { readonly x: number; readonly y: number } | null
}

export type Notification = {
  readonly id: string
  readonly severity: 'info' | 'success' | 'warning' | 'error'
  readonly message: string
  readonly actions: readonly { readonly label: string; readonly run: () => void }[]
  readonly createdAt: number
  /** Feature ids a failure notification names; it is dismissed once none of them is in error (4.2). Absent for other notifications. */
  readonly features?: readonly string[]
}

/** A live region message; `id` changes with every announcement, so a repeated text is read again. */
export type Announcement = { readonly id: number; readonly message: string }

/** A hint about a pick the target refused: under the active command's selection field at `field`, or in the sketch hint while picking a plane (3.16.1) when `field` is null. */
export type PickHint = { readonly message: string; readonly field: string | null }

export type AppStore = StoreApi<AppState>
export const INITIAL_STATE: AppState
export function createAppStore(overrides?: Partial<AppState>): AppStore
```

`INITIAL_STATE`: core loading; no catalog, printers or document; context `model`; empty selection; no hover, command or sketch; print report null, overhang map on, orientation null, not picking a face; view perspective, named `iso`, no section, filter `{ body: false, face: true, edge: true, vertex: true }`, grid on; browser open (closed when `window.innerWidth < 1024` at startup), timeline expanded, palette closed, no side panel; settings from `loadSettings()` (2.15); no notifications, announcement or pick hint.

`announcement` is set by undo and redo (3.20) and read by the polite live region. `pickHint` is set by `capturePick` and `createSketchOn` for a refused pick and cleared after 3 s, by a newer hint, or when its command ends.

`src/state/selectors.ts` exports at least `selectFeatureLabel(state, id)`, `selectActiveFeatureIds(state)`, `selectSketchDof(state)`, `selectHealthBadge(state)`, `selectProblems(state)`, `selectEmptyState(state)` (3.2) and `selectOriginVisible(state, key)` (3.10); components read state only through selectors or `useAppState(selector)` so each panel re-renders only for the slice it shows.

### 2.9 App and actions

`src/app/createApp.ts`:

```ts
export type Platform = {
  readonly operatingSystem: 'mac' | 'windows' | 'linux' | 'other'
  readonly host: 'web' | 'desktop'
}

export type App = {
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

export type CreateAppOptions = {
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
export function pickAt(app: App, sample: PickResult, modifiers: { toggle: boolean }, pointer?: PointerRay): Promise<void>

// command.ts
export function startFeatureCommand(app: App, featureType: string): Promise<void>
export function editFeature(app: App, featureId: string): Promise<void>
export function setCommandField(app: App, path: string, value: unknown): void
export function setActiveField(app: App, path: string | null): void
/** `pointer` is where the click's ray went; a flat face picked for a `sketch_point` field needs it (3.7). */
export function capturePick(app: App, item: SelectionItem, pointer?: PointerRay): Promise<void>
/** Mod+Z and Mod+Shift+Z inside a command (3.7, command history). */
export function undoCommandChange(app: App): Promise<void>
export function redoCommandChange(app: App): Promise<void>
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
export function startRollbackDrag(app: App, start: number): RollbackDrag | null

// bodies.ts
export function setBodyVisible(app: App, bodyId: string, visible: boolean): Promise<void>
export function setBodyColor(app: App, bodyId: string, color: string | null): Promise<void>
export function setBodyMaterial(app: App, bodyId: string, material: string | null): Promise<void>
export function renameBody(app: App, bodyId: string, name: string): Promise<void>
export function setLocked(app: App, bodyId: string, locked: boolean): void
export function setSessionVisible(app: App, key: string, visible: boolean): void

// parameters.ts
export function addParameter(app: App, name: string, expression: string): Promise<boolean>
export function renameParameter(app: App, id: string, name: string): Promise<boolean>
export function updateParameter(app: App, id: string, change: { expression?: string; comment?: string | null }): Promise<boolean>
export function deleteParameter(app: App, id: string): Promise<boolean>

// print.ts
export function setPrinter(app: App, profileId: string | null): Promise<void>
export function setCheckHidden(app: App, check: string, hidden: boolean): void
export function runPrintAnalysis(app: App): Promise<void>
export function focusFinding(app: App, finding: FindingKey): void
/** Starts the Lay on face pick (3.17). */
export function startLayOnFace(app: App): void
/** Sets the session print orientation; null prints as modeled. */
export function setPrintOrientation(app: App, orientation: Orientation | null): void

// export.ts
export function exportModel(app: App, request: ExportRequest): Promise<void>

// notifications.ts
export function notify(app: App, notification: Omit<Notification, 'id' | 'createdAt'>): string
export function dismiss(app: App, id: string): void
```

`ExportRequest` is `{ format: '3mf' | 'stl' | 'obj' | 'step'; scope: 'visible' | 'selected' }`. `RollbackDrag` is `{ move(position: number): void; drop(): Promise<void>; cancel(): Promise<void> }`, the live marker drag of 3.9 from gap `start`. `SketchChange`, `SketchCommitResult`, `PickResult` and `PointerRay` are defined in 2.14 and 2.11. Every action that changes the document goes through `app.mutations`.

### 2.10 Commands, shortcuts, toolbars and palette search

`src/commands/registry.ts`:

```ts
export type CommandCategory = 'File' | 'Edit' | 'View' | 'Create' | 'Modify' | 'Pattern' | 'Construct' | 'Sketch' | 'Constrain' | 'Print' | 'Settings'

export type Availability =
  | { readonly state: 'enabled' }
  | { readonly state: 'disabled'; readonly reason: string }
  | { readonly state: 'hidden' }

export type InvocationSource = 'toolbar' | 'palette' | 'shortcut' | 'menu' | 'inspector'

/** A search word; a tagged word is another app's name for the command (2.10, `COMMAND_KEYWORDS`). */
export type Keyword = string | { readonly word: string; readonly app: 'Fusion' | 'FreeCAD' }

export type CommandDefinition = {
  /** Unique id, for example 'file.save' or 'feature.extrude'. */
  readonly id: string
  readonly title: string
  readonly category: CommandCategory
  /** lucide-react icon component, or null. */
  readonly icon: LucideIcon | null
  /** Extra search words; a tagged word is shown as "{app}: {word}" when it matched (Scoring, below). */
  readonly keywords: readonly Keyword[]
  /** At most two short plain sentences, 120 characters in all, ending with a period, saying what the command does. Required for feature commands, sketch tools, `sketch.finish` and constraints (the description table below). */
  readonly description?: string
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

export type ShortcutConflict = { readonly chord: string; readonly context: AppContext; readonly commands: readonly string[] }

export type CommandRegistry = {
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
export type Chord = { readonly primaryModifier: boolean; readonly control: boolean; readonly alt: boolean; readonly shift: boolean; readonly key: string }
export function parseShortcut(shortcut: string): Chord
export function chordToString(chord: Chord): string
export function formatShortcut(shortcut: string, platform: Platform): string
export function chordFromEvent(event: KeyboardEvent, platform: Platform): Chord | null
```

Shortcut grammar: `shortcut = (modifier "+")* key`, modifiers in the order `Mod`, `Ctrl`, `Alt`, `Shift`, each at most once. `Mod` is Cmd (`metaKey`) on macOS and Ctrl elsewhere; `Ctrl` is the Control key on every platform and is not used together with `Mod`. `key` is an uppercase letter `A` to `Z`, a digit, `F1` to `F12`, or one of `Enter`, `Escape`, `Delete`, `Backspace`, `Tab`, `Space`, `ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`, `Home`, `End`, `=`, `-`. `parseShortcut` throws `Error('Invalid shortcut "<shortcut>".')` for anything else, including an empty key, a repeated modifier or a wrong modifier order.

`chordFromEvent` normalizes the key: a single letter `event.key` is uppercased; when `event.key` is not an ASCII letter but `event.code` is `KeyA` to `KeyZ` (non-Latin layouts), the letter from `event.code` is used; `' '` becomes `Space`; other keys use `event.key` as is. It returns `null` for a lone modifier press. On macOS `primaryModifier` is `metaKey` and `control` is `ctrlKey`; elsewhere `primaryModifier` is `ctrlKey` and `control` is always false.

`formatShortcut` renders macOS chords as modifier symbols in Apple's order `⌃` (Ctrl), `⌥` (Alt), `⇧` (Shift), `⌘` (Mod), followed by the key, with no separators (`Mod+Shift+S` is `⇧⌘S`), and other platforms as words joined by `+` (`Ctrl+Shift+S`, with `Mod` written `Ctrl`). Keys: letters, digits and `F1` to `F12` as is; `Enter` is `↩` on macOS and `Enter` elsewhere; `Delete` is `⌦` and `Del`; `Backspace` is `⌫` and `Backspace`; `Escape` is `Esc`; `Space` is `Space`; arrows are `↑`, `↓`, `←`, `→` everywhere; `=` and `-` as is.

`src/commands/dispatcher.ts`:

```ts
export function installShortcutDispatcher(app: App, target: Window): () => void
```

Dispatch rules, applied to every `keydown` on `window` in the bubbling phase:

1. Ignore the event when `event.defaultPrevented`, `event.isComposing` or `event.repeat` is true, or `chordFromEvent` returns null.
2. When the event target is editable (an `input` of a text-like type, a `textarea`, a `select`, or an element with `isContentEditable`), only chords with `primaryModifier` are considered, and never `Mod+Z`, `Mod+Shift+Z`, `Mod+Y`, `Mod+A`, `Mod+C`, `Mod+X` or `Mod+V` (native text editing keeps them).
3. When the palette, a menu or a popover is open, only `Mod+K` is considered (it closes the palette).
4. Find `app.commands.forChord(app, chord, context)`. If found: `preventDefault()`, then run it with source `shortcut`. A disabled command shows its reason (info notification, 3 s) instead of running.

Components that handle keys themselves (inspector Enter and Esc, sketch dimension entry, tree and timeline arrow keys, viewport navigation keys) call `preventDefault()`, which rule 1 respects.

`src/commands/fuzzy.ts`:

```ts
/** Best score of query against text, or null when query is not a subsequence of text. */
export function fuzzyScore(query: string, text: string): number | null

export type PaletteItem = {
  readonly id: string
  readonly kind: 'command' | 'feature' | 'body' | 'parameter' | 'recent' | 'setting'
  /** The command category for `command` and `setting` items; null for every other kind. With an empty query, command and setting items are grouped by category in the context order of the Scoring rules, and items with a null category are not listed. */
  readonly category: CommandCategory | null
  readonly title: string
  readonly keywords: readonly Keyword[]
  readonly detail: string | null
  /** The command's description (shown in the palette footer, 3.4), or null. */
  readonly description: string | null
  readonly shortcut: string | null
  readonly disabledReason: string | null
  readonly run: () => void
}

export function rankPaletteItems(query: string, items: readonly PaletteItem[], recentIds: readonly string[], context: AppContext): readonly PaletteItem[]
```

Scoring, exactly:

- Comparison is case-insensitive (`toLocaleLowerCase('en')`) on both strings; leading and trailing whitespace of the query is removed and inner runs of whitespace are kept as single spaces, and a space in the query matches a space in the text.
- A match assigns each query character to a strictly increasing text position with an equal character. Its score is the sum over matched characters of: 1, plus 8 if the position is 0, else plus 6 if the position is a word start (the previous text character is a space, `-`, `_`, `/`, `.`, `:` or `(`, or the original text has a lowercase letter followed by an uppercase letter there), plus 4 if the previous query character matched the immediately preceding text position. `fuzzyScore` is the maximum over all matches (dynamic programming, O(query length × text length)) minus `0.01 × text length`.
- An item's score is the maximum of its title score and each keyword score minus 2 (a tagged keyword scores its `word`). Items with no score are excluded.
- In Sketch context, items of category Sketch or Constrain add 3 to their score; in Print context, items of category Print add 3.
- When an item's score comes from a keyword tagged with an app, its detail is "{app}: {word}" (for example "FreeCAD: Pad") unless it has a disabled reason. In Sketch context a feature command with no other detail has the detail "Finishes the sketch".
- `rankPaletteItems` sorts by score descending, then by position in `recentIds` (most recent first; items not in it after), then by title (`localeCompare` with locale `en`). It returns at most 50 items for a non-empty query.
- With an empty query, `rankPaletteItems` returns the items in `recentIds` order (at most 5), then every item of kind `command` or `setting`, grouped by category in the context order below and by title within a category, with no limit; features, bodies, parameters and recent files appear only for a non-empty query. Context order: Model: Create, Modify, Pattern, Construct, Edit, View, File, Print, Settings; Sketch: Sketch, Constrain, Edit, View, Create, Modify, Pattern, Construct, File, Print, Settings; Print: Print, View, Edit, File, Create, Modify, Pattern, Construct, Settings. A category a context order does not name follows it, in the order of `CommandCategory`.

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

`src/commands/features.ts` registers one command per catalog entry: id `feature.<type>`, title the manifest label, category from the manifest category (`create` Create, `modify` Modify, `pattern` Pattern, `construct` Construct, `sketch` Create), icon from `FEATURE_ICONS[manifest.icon]` (fallback `Shapes`), keywords from `COMMAND_KEYWORDS['feature.' + type]`, description from `FEATURE_DESCRIPTIONS['feature.' + type]`, shortcuts from `FEATURE_SHORTCUTS[type]` (3.5), contexts `model`, `sketch` and `print` (running it from Sketch or Print switches to Model first, 3.3). The `sketch` type's command runs `startNewSketch` instead of the inspector. The `import` type gets no command (STEP import UI is later). Its timeline item can be selected, renamed, suppressed and deleted; Edit is disabled with `import_not_editable` (4.3).

Feature command availability (`FEATURE_REQUIREMENTS` in `src/commands/features.ts`) is computed from the store, so a tool that has nothing to act on says what is missing before it is clicked; editing an existing feature is never affected:

- `extrude`, `revolve`: disabled with `needs_sketch_region` when no active sketch (not suppressed, not rolled back) has a region in its latest solution (`document.sketches`); the sketch being edited in a sketch session counts.
- `hole`, `fillet`, `chamfer`, `shell`, `mirror`, `linear_pattern`, `circular_pattern`, `move_body`, `split_body`: disabled with `needs_solid` when no body has model-layer geometry.
- `boolean`: disabled with `needs_two_bodies` when fewer than two bodies have model-layer geometry.
- Every other type is always enabled.

`src/commands/icons.ts` maps manifest icon names (features.md 3.13) to lucide icons: `sketch` PencilRuler, `extrude` ArrowUpFromLine, `revolve` Rotate3d, `hole` CircleDot, `box` Box, `cylinder` Cylinder, `sphere` Globe, `cone` Cone, `torus` Torus, `fillet` SquareRoundCorner, `chamfer` Octagon, `shell` PackageOpen, `boolean` Combine, `move-body` Move3d, `split-body` SquareSplitVertical, `mirror` FlipHorizontal2, `linear-pattern` Columns3, `circular-pattern` Orbit, `datum-plane` Layers2, `import` FileInput. Constraint glyphs are an in-house set of line icons drawn in the lucide style (24 by 24 `viewBox`, stroke width 2, round caps and joins, `currentColor`), one per constraint type, because lucide has no parallel, perpendicular or coincident symbols.

`COMMAND_KEYWORDS` (`src/commands/keywords.ts`) holds the keywords of every command, feature and built-in alike, by command id; a command without an entry has none. It replaces the former `FEATURE_KEYWORDS`, because built-in commands (sketch tools, view commands, Finish sketch) need other apps' names too. A tagged keyword is written "Word (App)" below and stored as `{word: 'Word', app: 'App'}`; it is how someone coming from Fusion or FreeCAD finds a command by its old name and sees the mapping in the option itself.

| Command | Keywords |
| --- | --- |
| `feature.sketch` | drawing, profile, new sketch, Create Sketch (Fusion), sketcher |
| `feature.extrude` | Pad (FreeCAD), Pocket (FreeCAD), Press Pull (Fusion), extrusion, pull, push |
| `feature.revolve` | Revolution (FreeCAD), Groove (FreeCAD), lathe, spin |
| `feature.hole` | drill, counterbore, countersink, screw hole |
| `feature.fillet` | round, radius, rounded edge |
| `feature.chamfer` | bevel |
| `feature.shell` | hollow, Thickness (FreeCAD) |
| `feature.boolean` | Combine (Fusion), union, subtract, intersect, join, cut, merge, group |
| `feature.mirror` | Mirrored (FreeCAD), symmetry |
| `feature.linear_pattern` | Rectangular Pattern (Fusion), LinearPattern (FreeCAD), array |
| `feature.circular_pattern` | PolarPattern (FreeCAD), polar pattern, array |
| `feature.datum_plane` | Offset Plane (Fusion), construction plane, work plane |
| `feature.move_body` | Move/Copy (Fusion), Placement (FreeCAD), translate, rotate, transform |
| `feature.split_body` | slice, cut body |
| `feature.box` | Additive Box (FreeCAD), cube, block, primitive, shape |
| `feature.cylinder` | Additive Cylinder (FreeCAD), rod, pin, primitive, shape |
| `feature.sphere` | Additive Sphere (FreeCAD), ball, primitive, shape |
| `feature.cone` | Additive Cone (FreeCAD), taper, primitive, shape |
| `feature.torus` | Additive Torus (FreeCAD), ring, donut, primitive, shape |
| `sketch.line` | Polyline (FreeCAD), segment |
| `sketch.rectangle_2point` | rectangle |
| `sketch.rectangle_center` | Centered Rectangle (FreeCAD), rectangle |
| `sketch.circle` | Center Diameter Circle (Fusion) |
| `sketch.arc_3point` | 3-Point Arc (Fusion) |
| `sketch.point` | hole center |
| `sketch.construction` | Toggle Construction (FreeCAD), construction geometry, reference |
| `sketch.dimension` | Sketch Dimension (Fusion), Constrain Distance (FreeCAD), distance, length, radius, diameter, angle |
| `sketch.constrain_fix` | Lock (FreeCAD), Block (FreeCAD), fixed, ground |
| `sketch.constrain_symmetric` | Symmetry (Fusion), symmetrical |
| `sketch.constrain_point_on_object` | on curve, on line |
| `sketch.constrain_equal` | same size |
| `sketch.constrain_midpoint` | middle |
| `sketch.finish` | Close (FreeCAD), close sketch, leave sketch, exit sketch |
| `view.fit` | Fit All (FreeCAD), zoom all, zoom extents |
| `view.home` | isometric, home |
| `view.projection` | perspective |
| `view.section` | Section Analysis (Fusion), Clipping Plane (FreeCAD), cross section |
| `view.parameters` | Change Parameters (Fusion), Spreadsheet (FreeCAD), user parameters, variables |
| `view.problems` | warnings, errors |
| `view.health` | printability, health |
| `view.visibility` | hide, show |
| `view.grid` | grid, floor |
| `app.keys` | help, shortcuts, hotkeys, navigation, mouse |
| `edit.suppress` | disable, turn off, skip |
| `edit.edit_feature` | change, modify |
| `edit.repeat` | again, last command |
| `file.export` | 3MF, STL, OBJ, STEP |

`FEATURE_DESCRIPTIONS` (`src/commands/descriptions.ts`) gives feature commands their `description`; a type without an entry has none. Sketch tool, Finish sketch and constraint commands carry theirs in their definitions. Descriptions use the words of the interface, never schema or code terms, and are shown in the toolbar tooltip (3.24) and the palette footer (3.4).

| Command | Description |
| --- | --- |
| `feature.sketch` | Draws a 2D shape on a plane or a flat face, to extrude, revolve or place holes from. |
| `feature.extrude` | Pulls a closed sketch shape straight out into a solid, or cuts it into one. |
| `feature.revolve` | Spins a closed sketch shape around an axis into a round solid, or cuts with it. |
| `feature.hole` | Drills round holes where you click on a flat face or at sketch points. |
| `feature.box` | Adds a box of the given length, width and height. |
| `feature.cylinder` | Adds a cylinder of the given radius and height. |
| `feature.sphere` | Adds a ball of the given radius. |
| `feature.cone` | Adds a cone, or a tapered cylinder, from two radii and a height. |
| `feature.torus` | Adds a ring from the radius of its center line and the radius of its tube. |
| `feature.fillet` | Rounds the selected edges. |
| `feature.chamfer` | Bevels the selected edges with a flat cut. |
| `feature.shell` | Hollows out a body, leaving walls of the given thickness and the picked faces open. |
| `feature.boolean` | Joins, cuts or intersects one body with other bodies. |
| `feature.move_body` | Moves and rotates a whole body. |
| `feature.split_body` | Cuts a body in two along a plane, for example to fit a large part on the bed. |
| `feature.mirror` | Mirrors a body across a plane, joined to it or as a new body. |
| `feature.linear_pattern` | Repeats a body in a row, or in a grid with a second direction. |
| `feature.circular_pattern` | Repeats a body around an axis. |
| `feature.datum_plane` | Adds a plane offset from another plane, to sketch on or mirror across. |
| `sketch.line` | Draws connected straight lines. Enter or a double-click ends the line. |
| `sketch.rectangle_2point` | Draws a rectangle from two opposite corners. |
| `sketch.rectangle_center` | Draws a rectangle from its center and a corner. |
| `sketch.circle` | Draws a circle from its center and a point on it. |
| `sketch.arc_3point` | Draws an arc through its start, its end and a point on it. |
| `sketch.point` | Places a point, for example to mark a hole center. |
| `sketch.construction` | Turns lines and curves into construction geometry, which guides the shape but is not part of it. |
| `sketch.dimension` | Sets an exact length, distance, diameter, radius or angle. Type a value or a parameter name. |
| `sketch.finish` | Leaves the sketch and returns to the 3D model. |
| `sketch.constrain_coincident` | Makes two points meet, or puts a point on a curve. |
| `sketch.constrain_point_on_object` | Keeps a point on a line, circle or arc. |
| `sketch.constrain_horizontal` | Makes a line horizontal, or lines up two points horizontally. |
| `sketch.constrain_vertical` | Makes a line vertical, or lines up two points vertically. |
| `sketch.constrain_parallel` | Makes two lines parallel. |
| `sketch.constrain_perpendicular` | Makes two lines meet at a right angle. |
| `sketch.constrain_tangent` | Makes a line and a curve, or two curves, touch smoothly. |
| `sketch.constrain_collinear` | Puts two lines on the same straight line. |
| `sketch.constrain_concentric` | Gives two circles or arcs the same center. |
| `sketch.constrain_equal` | Makes two lines the same length, or two circles or arcs the same size. |
| `sketch.constrain_symmetric` | Places two points symmetrically about a line. |
| `sketch.constrain_midpoint` | Puts a point at the middle of a line. |
| `sketch.constrain_fix` | Locks points or curves where they are. |

### 2.11 Viewport: camera, render backend, picking, scene

`src/viewport/camera.ts` (pure functions; lengths in mm, angles in radians, pixel arguments in CSS pixels):

```ts
export type Vector3 = readonly [number, number, number]
/** Components x, y, z, w. */
export type Quaternion = readonly [number, number, number, number]
export type StandardView = 'top' | 'bottom' | 'front' | 'back' | 'left' | 'right' | 'iso'

export type Camera = {
  readonly target: Vector3
  /** Camera-to-world rotation. The camera looks along its local -Z; local +Y is up; local +X is right. */
  readonly rotation: Quaternion
  /** Eye distance from the target. */
  readonly distance: number
  readonly projection: 'perspective' | 'orthographic'
  /** Vertical field of view, perspective only. */
  readonly verticalFieldOfView: number
  /** Visible height at the target, orthographic only. */
  readonly orthographicHeight: number
}

/** 30 degrees. */
export const DEFAULT_VERTICAL_FIELD_OF_VIEW: number
/** The iso view, target [0, 0, 0], distance 200, perspective, orthographicHeight 2 × 200 × tan(15°). */
export const DEFAULT_CAMERA: Camera
export function eye(camera: Camera): Vector3
export function forward(camera: Camera): Vector3
export function up(camera: Camera): Vector3
export function viewMatrix(camera: Camera): Float32Array
export function projectionMatrix(camera: Camera, aspect: number, near: number, far: number): Float32Array
export function depthRange(camera: Camera, sceneCenter: Vector3, sceneRadius: number): { near: number; far: number }
export function visibleHeightAtTarget(camera: Camera): number
export function standardViewRotation(view: StandardView): Quaternion
export function rotationLookingAlong(direction: Vector3, upHint: Vector3): Quaternion
export function classifyView(camera: Camera): StandardView | 'custom'
export function orbit(camera: Camera, deltaXPixels: number, deltaYPixels: number, pivot: Vector3): Camera
export function pan(camera: Camera, deltaXPixels: number, deltaYPixels: number, viewportHeightPixels: number): Camera
/** `cursorNormalized` is the cursor in normalized device coordinates, each -1 to 1. */
export function zoomAt(camera: Camera, factor: number, cursorNormalized: readonly [number, number], aspect: number): Camera
export function fitBounds(camera: Camera, bounds: BoundingBox, aspect: number, margin?: number): Camera
export function withProjection(camera: Camera, projection: 'perspective' | 'orthographic'): Camera
export function wheelZoomFactor(event: { deltaY: number; deltaMode: number; ctrlKey: boolean }, viewportHeightPixels: number): number
/** The camera to animate to when a sketch with plane normal `normal` is left (3.16.11), or null to stay. */
export function cameraAfterSketch(current: Camera, before: Camera | null, normal: Vector3): Camera | null
```

Definitions:

- Standard views (camera position relative to the target, up vector): `front` from -Y, up +Z; `back` from +Y, up +Z; `right` from +X, up +Z; `left` from -X, up +Z; `top` from +Z, up +Y; `bottom` from -Z, up -Y; `iso` from the direction (1, -1, 1), up the projection of +Z onto the view plane. With these, +X points right in the front, top and bottom views, and the origin plane sketches (architecture.md) are seen from their front: XY gives `top`, XZ gives `front`, YZ gives `right`.
- `classifyView` returns the standard view whose forward and up vectors are both within 1e-6 rad of the camera's, else `custom`.
- `orbit`: horizontal motion rotates about the world +Z axis through `pivot` by `-deltaXPixels × 0.4°`; vertical motion then rotates about the camera's right axis through `pivot` by `-deltaYPixels × 0.4°`. There is no elevation clamp. Target and rotation both change so `pivot` stays fixed on screen.
- `pan` moves the target so the point at the target depth under the cursor follows the cursor exactly: millimeters per pixel are `visibleHeightAtTarget / viewportHeightPixels`.
- `zoomAt` with factor f > 1 zooms out, f < 1 zooms in: perspective multiplies `distance` by f, orthographic multiplies `orthographicHeight` by f; the target moves so that the world point at the target depth under `cursorNormalized` stays under the cursor. Distance and orthographicHeight are clamped to [0.01, 1e7].
- `wheelZoomFactor`: the pixel delta is `deltaY` for `deltaMode` 0, `deltaY × 16` for mode 1, `deltaY × viewportHeightPixels` for mode 2; the factor is `1.0015 ^ pixels`, or `1.01 ^ pixels` when `ctrlKey` is true (trackpad pinch).
- `fitBounds`: keeps the rotation; sets the target to the box center; transforms the 8 corners into camera space; with `w` and `h` the camera-space x and y extents, the required visible height is `max(h, w / aspect) / (1 - 2 × margin)` (margin default 0.1, so the content fills 80 percent of the limiting dimension). Orthographic sets `orthographicHeight` to it; perspective sets `distance = visibleHeight / (2 tan(verticalFieldOfView / 2)) + d / 2`, where `d` is the camera-space z extent. An empty or degenerate box (all extents below 1e-9) is treated as a 100 mm cube around its center.
- `withProjection` preserves the apparent size at the target: switching to orthographic sets `orthographicHeight = 2 × distance × tan(verticalFieldOfView / 2)`; switching to perspective sets `distance = orthographicHeight / (2 tan(verticalFieldOfView / 2))`.
- `cameraAfterSketch`: when the angle between `forward(current)` and `normal` or `-normal` is more than 10 degrees, null (the user already turned the view, so it stays). Otherwise `before` when it is not null and its forward vector is itself more than 10 degrees from `normal` and `-normal`; otherwise `current` with the `iso` rotation (`standardViewRotation('iso')`), keeping target, distance, projection and orthographic height. It never returns a camera looking straight at the sketch plane, so the next feature's preview is seen in depth.
- `depthRange` covers the scene's bounding sphere with 10 percent slack: with `c` the distance of the sphere center along the forward axis, perspective uses `near = max(c - 1.1 × r, 0.001 × (c + 1.1 × r), 0.01)` and `far = c + 1.1 × r`; orthographic uses `near = c - 1.1 × r`, `far = c + 1.1 × r` (near may be negative).

`src/viewport/backend.ts` (ADR 0007):

```ts
/** Linear RGB, each 0 to 1: the space shading is computed in. */
export type Rgb = readonly [number, number, number]
/** sRGB-encoded RGB, each 0 to 1: a '#rrggbb' value divided by 255. */
export type EncodedRgb = readonly [number, number, number]

export type Geometry =
  | { readonly kind: 'triangles'; readonly positions: Float32Array; readonly normals: Float32Array; readonly indices: Uint32Array; readonly elementIds: Uint32Array; readonly elementCount: number }
  | { readonly kind: 'lines'; readonly positions: Float32Array; readonly elementIds: Uint32Array; readonly elementCount: number }
  | { readonly kind: 'points'; readonly positions: Float32Array; readonly elementIds: Uint32Array; readonly elementCount: number }
  | { readonly kind: 'fill'; readonly positions: Float32Array; readonly indices: Uint32Array; readonly elementIds: Uint32Array; readonly elementCount: number }

export type DrawCall = {
  readonly id: string
  readonly layer: 'opaque' | 'translucent' | 'overlay'
  readonly color: Rgb
  /** Optional per-element color, 3 floats per element (sketch DOF colors). */
  readonly elementColors: Float32Array | null
  readonly opacity: number
  /** Lines: width in CSS pixels; points: size in CSS pixels. */
  readonly size: number
  /** Lines only: dashed (construction geometry). */
  readonly dashed: boolean
  readonly shading: 'matcap' | 'flat'
  readonly depthTest: boolean
  readonly clipped: boolean
  /** First pick id of this draw, or null when not pickable. Element i has pick id pickBase + i. */
  readonly pickBase: number | null
  /** Pick footprint in CSS pixels for lines and points in the ID pass. */
  readonly pickSize: number
  /** Whether this draw colors its scalar field with RenderFrame.analysis. */
  readonly analysis: boolean
}

export type RenderFrame = {
  readonly view: Float32Array
  readonly projection: Float32Array
  readonly eye: Vector3
  readonly orthographic: boolean
  readonly pixelRatio: number
  readonly uiScale: number
  readonly theme: ViewportTheme
  readonly draws: readonly DrawCall[]
  readonly section: { readonly normal: Vector3; readonly offset: number } | null
  /** The analysis ramp ends are sRGB-encoded because the ramp is interpolated before linearizing (3.12 pass 3). */
  readonly analysis: { readonly low: EncodedRgb; readonly high: EncodedRgb; readonly threshold: number; readonly maximum: number } | null
}

/** The theme colors the backend itself uses, linearized from the viewport roles of 2.16.4 by `viewportTheme`. */
export type ViewportTheme = {
  readonly backgroundTop: Rgb
  readonly backgroundBottom: Rgb
  /** Hover and selection tint. */
  readonly highlight: Rgb
  readonly problem: Rgb
  readonly edgeDark: Rgb
  readonly edgeLight: Rgb
}

export type PickSample = {
  readonly pickId: number
  /** Distance from the queried point in CSS pixels. */
  readonly distance: number
  /** View-space distance in mm when the backend can read it, else null. */
  readonly depth: number | null
}

export type RenderBackend = {
  readonly kind: 'webgl2'
  /** Sets the canvas size in CSS pixels and the device pixel ratio. */
  resize(width: number, height: number, pixelRatio: number): void
  /** Uploads geometry under an id, replacing earlier geometry with that id. */
  upload(id: string, geometry: Geometry): void
  remove(id: string): void
  /** One byte per element: bit 0 hover, bit 1 selected, bit 2 problem. */
  setElementStates(id: string, states: Uint8Array): void
  /** One float per vertex, or null to clear. */
  setScalarField(id: string, values: Float32Array | null): void
  render(frame: RenderFrame): void
  /** Renders the ID pass if stale and reads every pick id within `radius` CSS pixels of the point (x, y) in CSS pixels. */
  pick(frame: RenderFrame, x: number, y: number, radius: number): readonly PickSample[]
  onContextLost(listener: () => void): () => void
  onContextRestored(listener: () => void): () => void
  dispose(): void
}

/** Returns null when the canvas cannot create a WebGL2 context. */
export function createWebGl2Backend(canvas: HTMLCanvasElement): RenderBackend | null
```

`src/viewport/picking.ts`:

```ts
/** RGBA8 encoding of a 32-bit pick id: red = bits 0-7, green = 8-15, blue = 16-23, alpha = 24-31. 0 means nothing. */
export function encodePickId(id: number): readonly [number, number, number, number]
export function decodePickId(red: number, green: number, blue: number, alpha: number): number

export type PickClass = 'point' | 'curve' | 'area'
export type PickCandidate = { readonly key: string; readonly kind: SelectionItemKind; readonly pickClass: PickClass; readonly distance: number }

/** The candidate the pointer means (section 3.6), or null. `radius` is in CSS pixels. */
export function choosePick(candidates: readonly PickCandidate[], accepts: (kind: SelectionItemKind) => boolean, radius: number): PickCandidate | null

/** What a pick resolved to before any core round trip. */
export type PickResult =
  | { readonly kind: 'mesh'; readonly body: string; readonly meshId: number; readonly element: 'face' | 'edge' | 'vertex'; readonly index: number; readonly key: string; readonly world: Vector3 | null }
  | { readonly kind: 'item'; readonly item: SelectionItem; readonly world: Vector3 | null }

/** The camera ray through the pointer at a click, in model millimeters, and the world size of one CSS pixel at the picked point multiplied by the UI scale. */
export type PointerRay = { readonly origin: Vector3; readonly direction: Vector3; readonly millimetersPerPixel: number }
```

`src/viewport/scene.ts` keeps every drawable's CPU-side data (so a lost WebGL context can be restored), allocates pick id ranges (consecutive, starting at 1, each draw's range is its element count), maps pick ids back to pick targets, and builds the `RenderFrame` for a camera and the store state:

```ts
export type Scene = {
  applyEvent(event: CoreEvent): void
  /** The pick target for a pick id, or null. */
  targetFor(pickId: number): { readonly result: PickResult; readonly pickClass: PickClass } | null
  /** Bounds of visible model geometry, sketches and datum planes; null when nothing is visible. */
  visibleBounds(state: AppState): BoundingBox | null
  /** Screen rectangle (CSS pixels) covering selection items, or null. */
  screenRectangle(items: readonly SelectionItem[], camera: Camera, size: { width: number; height: number }): Rectangle | null
  frame(state: AppState, camera: Camera, size: { width: number; height: number; pixelRatio: number }): RenderFrame
  restore(): void
}
export function createScene(backend: RenderBackend): Scene
```

`src/viewport/controller.ts`:

```ts
export type ViewportHandle = {
  getCamera(): Camera
  setCamera(camera: Camera, options?: { animate?: boolean; record?: boolean }): void
  fit(bounds?: BoundingBox): void
  setStandardView(view: StandardView): void
  toggleProjection(): void
  /** Puts the eye on the `normal` side of `target`, looking along -normal, with up from `upHint`. */
  lookAlong(normal: Vector3, upHint: Vector3, target: Vector3): void
  zoomToBounds(bounds: BoundingBox, margin: number): void
  screenRectangle(items: readonly SelectionItem[]): Rectangle | null
  requestRender(): void
}
export type ViewportController = ViewportHandle & { dispose(): void }
/** `overlay` is the overlay layer of 3.12 whose positioned children the controller moves each frame. */
export function createViewportController(options: { app: App; canvas: HTMLCanvasElement; overlay: HTMLElement; backend: RenderBackend; scene: Scene }): ViewportController
```

Camera animations run 250 ms with a cubic ease-out, and are instant when `matchMedia('(prefers-reduced-motion: reduce)')` matches.

`src/viewport/navigation.ts`:

```ts
export type NavigationPreset = 'fusion' | 'freecad'
export type Gesture = 'orbit' | 'pan' | 'none'
/** The presets in menu order; `rows` are the preset's column of the 3.13 table, in table order, as `{action, gesture}` pairs (the keyboard and mouse panel renders them, 3.5). */
export const NAVIGATION_PRESETS: readonly {
  readonly id: NavigationPreset
  readonly label: string
  readonly middleDoubleClickFits: boolean
  readonly rows: readonly { readonly action: string; readonly gesture: string }[]
}[]
/** The camera gesture for the pressed buttons (PointerEvent.buttons: 1 left, 2 right, 4 middle) and modifiers (section 3.13). */
export function gestureFor(preset: NavigationPreset, buttons: number, modifiers: { readonly shift: boolean }): Gesture
/** One-line legend of the preset's camera gestures, shown in the empty state (3.2). */
export function navigationLegend(preset: NavigationPreset): string
```

`gestureFor`: in both presets, right alone is `orbit` and right with Shift is `pan` (so a mouse without a middle button, a macOS trackpad's two-finger press and drag, and the Tinkercad and Onshape habit all orbit and pan). Fusion: middle alone is `pan`, middle with Shift is `orbit`; FreeCAD: middle alone is `pan`, middle with left or middle with right is `orbit`; every other combination is `none` (left alone selects or box-selects).

`navigationLegend`: Fusion "Orbit: Shift+middle-drag, right-drag or drag the view cube. Pan: middle-drag or Shift+right-drag. Zoom: wheel or pinch."; FreeCAD "Orbit: middle+left-drag, right-drag or drag the view cube. Pan: middle-drag or Shift+right-drag. Zoom: wheel or pinch."

`src/viewport/grid.ts`:

```ts
/** Minor grid spacing in millimeters for the scale at the camera target (3.12). */
export function gridSpacing(millimetersPerPixel: number, units: Units): number
```

`gridSpacing` returns the smallest of 1, 10, 100 and 1000 mm (in inch documents 2.54, 25.4, 254 and 2540 mm, that is 0.1, 1, 10 and 100 in) whose lines are at least 8 CSS pixels (times the UI scale) apart, that is `spacing / millimetersPerPixel >= 8`, where `millimetersPerPixel` is the world size of one CSS pixel at the camera target multiplied by the UI scale; the largest step when none is. Centimeter and meter documents use the millimeter steps.

`src/viewport/menu.ts`:

```ts
export type ViewportMenuItem =
  | { readonly kind: 'command'; readonly command: string; readonly label: string; readonly shortcut: string | null; readonly run: () => void }
  | { readonly kind: 'separator' }
/** The items of the viewport context menu for the current state (3.13), disabled commands left out. */
export function viewportMenuItems(app: App): readonly ViewportMenuItem[]
```

### 2.12 Inspector generation

Feature manifests arrive typed: `FeatureManifest`, `Category`, `FieldHint`, `FieldUnit`, `SelectionKind` and `OptionHint` are the generated types of `src/generated/protocol.ts` (model.md; protocol.md 2.5 command 2), so the UI declares no mirror. Their optional members are optional properties, not `null` or `false`: the inspector treats an absent `essential` as false, an absent `unit` or `selection` as null and an absent `options` as empty. A catalog entry whose `params_schema` is not an object schema is left out of the catalog and reported once with `console.error`; the rest of the catalog loads.

`src/inspector/model.ts`:

```ts
export type JsonSchema = Readonly<Record<string, unknown>>

export type FieldControl =
  | { readonly type: 'expression'; readonly unit: FieldUnit }
  | { readonly type: 'selection'; readonly accepts: SelectionKind; readonly multiple: boolean; readonly minimum: number }
  | { readonly type: 'choice'; readonly options: readonly { value: string; label: string }[]; readonly presentation: 'segmented' | 'select' }
  | { readonly type: 'switch' }
  | { readonly type: 'toggle' }
  | { readonly type: 'text' }
  | { readonly type: 'number'; readonly integer: boolean }
  | { readonly type: 'unsupported' }

export type InspectorField = {
  readonly path: string
  readonly label: string
  readonly help: string | null
  readonly essential: boolean
  readonly required: boolean
  readonly control: FieldControl
  readonly value: unknown
  readonly error: string | null
  /** For an inline toggle, the path of the field whose row it sits on; otherwise null. */
  readonly attachedTo: string | null
}

export type InspectorModel = {
  readonly essential: readonly InspectorField[]
  readonly more: readonly InspectorField[]
  /** True when a field in `more` has an error (including a mapped status message, 3.7), or when a required field in `more` is empty and not waiting for a pick (a `body` or `sketch` field whose sibling `face`, `edge` or `sketch_point` field is still empty waits: the first pick fills it, 2.12); it forces the More section open. */
  readonly forceMore: boolean
  /** Labels of shown More fields whose value differs from its starting value, in display order; `body`, `sketch` and `target` fields never count (rules of 2.12 fill them). */
  readonly changedMore: readonly string[]
  /** True when every required field has a value and no field has an error. */
  readonly complete: boolean
}

export function buildInspectorModel(input: {
  readonly feature: CatalogFeature
  readonly params: Readonly<Record<string, unknown>>
  /** Field errors by path, including the status messages mapped to fields (3.7). */
  readonly errors: Readonly<Record<string, string>>
  /** The starting values the params are compared with for `changedMore`: `defaultParams` without the reuse step. */
  readonly startParams: Readonly<Record<string, unknown>>
}): InspectorModel
export function defaultParams(feature: CatalogFeature, context: DefaultsContext): Record<string, unknown>
export function setParam(feature: CatalogFeature, params: Readonly<Record<string, unknown>>, path: string, value: unknown): Record<string, unknown>
export function getParam(params: Readonly<Record<string, unknown>>, path: string): unknown
export type DefaultsContext = {
  readonly bodies: readonly Body[]
  readonly bodiesWithGeometry: readonly string[]
  readonly lastTargetBody: string | null
  /** The recipe, for the reuse rule; null skips it. */
  readonly document: Document | null
  /** Feature statuses by id, so the reuse rule skips records in error. */
  readonly featureStatus: Readonly<Record<string, FeatureStatus>>
}
```

Schema subset (draft 2020-12 as schemars 1 emits it). `buildInspectorModel` resolves `$ref` against the schema's `$defs`, then maps each manifest field hint, in manifest order, to a control:

| Schema at the hint's path | Hint | Control |
| --- | --- | --- |
| `$ref` to the `Expr` definition, or a string schema whose title is `Expr` | `unit` set | `expression` with that unit |
| any | `selection` set | `selection`; `multiple` is true when the schema at the path is an array, or for `sketch_region` (a `ProfileRef` holds a list); `minimum` is 1 when the path is required, else 0 |
| string `enum`, or `oneOf` of `const` strings, or the tag property (`kind`) of a tagged union | `options` set | `choice`, `segmented` for at most 4 options, else `select`, with the hint's labels in the hint's order |
| `boolean` | none | `switch` |
| `string` | none | `text` |
| `integer` or `number` | none | `number` (`integer` for `integer`) |
| anything else | none | `unsupported`: shown read-only as compact JSON with the help text "Edit this value in the .ptsm file." |

- A field is shown only when its path exists in the current value's schema branch: for a tagged union, a hint like `extent.opposite_expr` is shown only while the selected variant (by its `kind` value) declares `opposite_expr`.
- `required` is true when every object on the path lists the property in `required` (for tag-dependent fields, in the selected variant).
- `label` is the hint label; `help` is `FIELD_HELP[type][path]` (`src/inspector/help.ts`, table below), else null. Schema descriptions are developer documentation and are never shown.
- `essential` fields go to `essential` in manifest order; the others to `more` in manifest order.
- Inline Reverse: in a feature whose params have a top-level boolean `reverse` and a top-level tagged union `extent` (Extrude, Revolve, Hole), `reverse` gets the `toggle` control and goes to `essential` right after the field it attaches to, whatever its manifest `essential` flag (an override of the manifest hint until the manifests mark it essential, open issue 15): the first shown expression field whose path starts with `extent.`, else `extent.kind`; `attachedTo` is that path. It is not shown while `extent.kind` is `symmetric` or `full`, where the feature ignores it (features/extrude.md open issue 1); its value is kept.
- `error` is `errors[path]` when present.
- `Option<T>` (`anyOf` with `{ "type": "null" }`) is the inner control with `required` false; clearing it removes the key from params.

`FIELD_HELP` holds the hover help of fields whose label alone does not say what they do, in the interface's words; a field without an entry has no help.

| Type | Path | Help |
| --- | --- | --- |
| `extrude` | `profile` | The closed sketch shapes to extrude. Click inside a shape to add it. |
| `extrude` | `extent.kind` | Distance goes one way, Symmetric the same distance both ways, Two sides sets each side, Through all goes through the whole body. |
| `extrude`, `revolve` | `op` | New body makes a separate part. Join adds to the body, Cut removes from it, and Intersect keeps only the overlap. |
| `extrude` | `reverse` | Extrudes to the other side of the sketch. |
| `extrude`, `revolve` | `body` | The body that Join, Cut and Intersect change. |
| `revolve` | `axis` | The line to spin around: an origin axis, a sketch line or a straight edge. |
| `revolve` | `extent.kind` | Full makes a complete turn. Angle stops part way. |
| `revolve` | `reverse` | Turns the other way around the axis. |
| `hole` | `points` | Where the holes go: click a flat face, or pick points of a sketch. |
| `hole` | `hole_type.kind` | Counterbore adds a wider flat recess for a screw head. Countersink adds a cone for a flat-head screw. |
| `hole` | `hole_type.diameter` | Diameter of the wider recess at the top of the hole. |
| `hole` | `hole_type.depth` | Depth of the recess. |
| `hole` | `hole_type.angle` | Angle of the countersink cone. 90° fits most flat-head screws. |
| `hole` | `extent.kind` | Depth stops at a set depth. Through all goes through the whole body. |
| `hole` | `tip_angle` | Gives the bottom a drill-point cone of this angle. Leave it empty for a flat bottom. |
| `hole` | `reverse` | Drills out of the face instead of into it. |
| `fillet` | `edges` | The edges to round. Click a face to add all of its edges. |
| `chamfer` | `edges` | The edges to bevel. Click a face to add all of its edges. |
| `chamfer` | `distance.kind` | Equal distance cuts the same amount from both faces. Two distances sets each side. |
| `shell` | `remove_faces` | Faces to leave open. With none, the body becomes a closed hollow. |
| `shell` | `thickness` | Wall thickness. For FDM printing, a multiple of the nozzle width prints best. |
| `boolean` | `target` | The body that changes. |
| `boolean` | `tools` | The bodies that join, cut or intersect the target. They are used up unless Keep tools is on. |
| `boolean` | `op` | Join merges the bodies, Cut removes the tools from the target, and Intersect keeps only the overlap. |
| `boolean` | `keep_tools` | Keeps the tool bodies after the operation instead of using them up. |
| `mirror` | `op` | Join adds the mirrored copy to the body. New body puts it in a separate body. |
| `split_body` | `target` | The body that gets the other part. Leave it empty to discard that part. |
| `split_body` | `flip` | Keeps the back part in Body instead of the front part. |
| `linear_pattern` | `direction.count` | How many instances along this direction, counting the original. |
| `linear_pattern` | `direction.spacing` | Distance between neighboring instances. |
| `circular_pattern` | `count` | How many instances around the axis, counting the original. |
| `circular_pattern` | `angle` | The angle the instances spread over. |
| `datum_plane` | `offset` | Distance from the base plane along its normal. |
| `move_body` | `rotation.axis` | The axis to rotate around. Leave it empty to only move. |

`defaultParams`: each expression field takes the first of (1) reuse: the text at the same path in the latest record of the same type in timeline order that has a value there and whose status is not `error`, when the path's union variant is the same (selection, body, op and other choice fields never carry over); (2) `START_VALUES[type][path]` (`src/inspector/start-values.ts`, below); (3) the schema `default`; (4) `"10 mm"` for length, `"90 deg"` for angle, `"1"` for scalar and `"2"` for count. Other properties with a schema `default` get it. A tagged union starts with the variant `START_VALUES` names for its tag path, else its first variant in schema order, with that variant's values by the same rules. A choice without default gets its first option. Op fields (path `op` with options containing `new`): `new` when `bodiesWithGeometry` is empty, else `join`. A `body` field with selection `body` starts at `lastTargetBody` when the feature's op is not `new` (or it has no op) and the field's parent object has no `face` or `edge` field, whose first pick fills it (rule below); otherwise it starts empty. Other selection fields start empty. Defaults are a starting point only; the core validates. Copying from the document, not from a setting, keeps parameter names valid (a radius of `edge_r` carries over) and carries a project's conventions with no hidden state.

| Type | Path | Start value |
| --- | --- | --- |
| `fillet` | `radius` | `1 mm` |
| `chamfer` | `distance.expr`, `distance.second_expr` | `1 mm` |
| `shell` | `thickness` | `2 mm` |
| `hole` | `diameter` | `3.2 mm` |
| `hole` | `extent.kind` | `through_all` |
| `hole` | `hole_type.depth` | `3 mm` |
| `hole` | `hole_type.angle` | `90 deg` |
| `revolve` | `extent.kind` | `full` |
| `datum_plane` | `offset` | `10 mm` |

Start values carry explicit units, so they mean the same in every document unit and work on printed parts of 20 to 60 mm, where 10 mm would make the first fillet, chamfer or shell preview fail. `hole_type.diameter` (the head of a counterbore or countersink) starts at twice Diameter: when Diameter's text is a plain number (`isPlainNumber`), the doubled number with the same unit word and spacing (`3.2 mm` gives `6.4 mm`); otherwise `2 * ({text})`.

`setParam` writes a value at a path, creating intermediate objects. Changing a tagged union's tag replaces the union object with the new variant's starting values (rules 2 to 4 above and the head diameter rule; never the reuse rule, which needs the document), keeping properties whose names exist in both variants (switching `distance` to `symmetric` keeps `expr`). Unknown keys present in edited params are preserved untouched (features.md 3.2).

`src/inspector/selection-values.ts` builds param values from selection items, exactly in the shapes of features.md 2.5:

```ts
/** Whether a selection field accepts the item. */
export function acceptsTarget(accepts: SelectionKind, item: SelectionItem): boolean
/** The param value for the items, or null when they cannot form one. */
export function selectionValue(accepts: SelectionKind, items: readonly SelectionItem[], multiple: boolean): unknown
/** Selection items for an existing param value, used when editing (labels from the document). */
export function itemsFromValue(accepts: SelectionKind, value: unknown, state: AppState): readonly SelectionItem[]
```

| Accepts | Items accepted | Value |
| --- | --- | --- |
| `body` | body rows; faces, edges and vertices (their body) | body id string; array of ids when `multiple` |
| `sketch` | sketch features (timeline, browser), sketch curves and regions (their sketch) | sketch feature id |
| `sketch_region` | regions of one sketch | `{"sketch": id, "regions": [RegionInfo.reference, ...]}`, each reference copied unchanged from the solution, in pick order; a region of another sketch replaces the list |
| `sketch_point` | sketch points of one sketch | array of `PointRef` strings in pick order (features/hole.md `points`), all of one sketch; a point of another sketch replaces the list |
| `face` | faces | `Reference` from `ElementDescriptor.reference`; array when `multiple` (the body goes in the sibling `body` field, rule below) |
| `edge` | edges | `Reference`; array when `multiple` (body as for `face`) |
| `plane` | origin planes, datum planes, faces whose `surface` is `plane` | `{"kind": "origin", "plane": "XY"}`, `{"kind": "datum", "feature": id}` or `{"kind": "face", "body": id, "face": <Reference>}` |
| `axis` | origin axes, sketch lines, edges whose `curve` is `line`, faces whose `surface` is `cylinder`, `cone`, `sphere` or `torus` | `{"kind": "origin", "axis": "Z"}`, `{"kind": "sketch_line", "sketch": id, "line": id}`, `{"kind": "edge", "body": id, "edge": <Reference>}` or `{"kind": "face", "body": id, "face": <Reference>}` |

During a command, `capturePick` also takes two kinds that no row of this table names, and converts them, so no face is ever stored for them (3.7): flat faces for a `sketch_point` field (holes placed on a face) and faces for a `multiple` `edge` field (all of the face's edges). Preselection (3.7 step 4) uses this table only, plus the sketch feature rule of 3.7 step 4.

When a `face` or `edge` field's parent object also has a `body` field (selection `body`) that is empty, the first accepted pick fills it with the item's body; later picks from another body are refused with `pick_same_body` (4.3). When that `face` or `edge` field becomes empty again and its first pick filled the `body` field, the `body` field is cleared too. Likewise a `sketch` field (selection `sketch`) beside a `sketch_point` field is set to the points' sketch, and a `body` field beside them is set to that sketch's `plane.body` when its `plane.kind` is `face` (a hole drills the body it was sketched on).

Body-creating targets: a `target` field with selection `body` receives the body the command creates, in place of the `body` field, in two cases: (a) features whose `op` options include `new` (Mirror, Linear pattern, Circular pattern): the field is shown only while op is `new`, and op `new` sets it to a new body; (b) features without an `op` field whose `target` is not required (Split body): it starts set to a new body, so both parts are kept. The `body` field of these features never receives a created body.

`src/inspector/placement.ts`:

```ts
export type Rectangle = { readonly x: number; readonly y: number; readonly width: number; readonly height: number }
export function placeInspector(input: { readonly anchor: Rectangle | null; readonly panel: { width: number; height: number }; readonly bounds: Rectangle; readonly gap?: number }): { x: number; y: number }
```

All values are CSS pixels. With `gap` default 16: if `anchor` is null, return `(bounds.x + gap, bounds.y + gap)`. Otherwise try the right side: `x = anchor.x + anchor.width + gap`; if `x + panel.width + gap <= bounds.x + bounds.width`, use it; else try the left side: `x = anchor.x - gap - panel.width`; if `x >= bounds.x + gap`, use it; else return the null-anchor position. On either side `y = clamp(anchor.y, bounds.y + gap, bounds.y + bounds.height - panel.height - gap)`, and if the panel is taller than the bounds minus two gaps, `y = bounds.y + gap`.

### 2.13 Expression input

`src/expression/evaluate.ts` mirrors features.md 3.4 so the inspector shows what a field's value will be before the feature builds:

```ts
export type FieldValue =
  | { readonly ok: true; readonly value: number }
  | { readonly ok: false; readonly message: string }
/** Converts an evaluate() result for a field of the given unit, or explains why it does not fit. */
export function convertForField(result: { value: number; dimension: Dimension }, unit: FieldUnit, units: Units, label: string, source: string): FieldValue
```

`Dimension` is model.md's `{length, angle}` exponent pair: length is `{length: 1, angle: 0}`, angle `{length: 0, angle: 1}`, dimensionless `{length: 0, angle: 0}`, area `{length: 2, angle: 0}`; any other dimension is named `a mm^l rad^a` with zero exponents omitted and exponent 1 written without `^` (for example `a mm^3`).

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
export function formatPoint(point: Vector3, units: Units): string
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
  /** `range` is the offending part of the text in UTF-16 indices (from `utf16Range`), or null when the core gave no span. */
  | { readonly state: 'invalid'; readonly message: string; readonly range: { readonly start: number; readonly end: number } | null }

export type ExpressionInputProps = {
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

Pressing Enter in an `invalid` state with a non-null `range` keeps the input focused and selects that range (`setSelectionRange(range.start, range.end)`), so the user can retype the offending part; the message stays in `field-error`.

Name completion, in every `ExpressionInput` (inspector fields, the selection inspector's dimension and size fields, the Parameters panel and the section offset; the sketch draw inputs and the dimension editor are plain text inputs and have none):

- Names: the document's parameters, shown as "{name} = {value}" (the value formatted as in 3.11), and the `printer.*` names of print.md (`printer.fit.press`, `printer.fit.snug`, `printer.fit.slip`, `printer.fit.loose`, `printer.layer_height`, and `printer.nozzle` for an FDM profile), shown as the name.
- Opening: while the caret ends an identifier (letters, digits, `_` and `.`, starting with a letter or `_`) that some name matches with a non-null `fuzzyScore`, a listbox under the input (`data-testid="expression-suggestions"`, options `role="option"` in `FONT_CLASSES.values`) lists up to 8 such names, best score first, then by name. It never opens for an identifier that directly follows a number (`10 mm`, `1 in`), that is a unit word, `pi` or a function name of expressions.md, or that equals a name exactly. A character that cannot be part of a name closes it.
- Keys: the input becomes a `role="combobox"` with `aria-expanded`, `aria-controls` and `aria-activedescendant`. No option is active until ArrowDown or ArrowUp moves through the list (they step plain numbers only while it is closed). Tab inserts the active option, or the first one, replacing the identifier, and keeps focus in the input. Enter inserts the active option when one was chosen with the arrow keys, and otherwise acts as if the list were closed (it commits, so typing a full name and pressing Enter never needs a second Enter). Esc closes the list (Esc ladder step 1). Clicking an option inserts it. Inserting closes the list without committing or moving focus.

`src/expression/span.ts` converts expression error spans. expressions.md reports a span as UTF-8 byte offsets into the expression source (`Span { start, end }`, end exclusive), while JavaScript strings, `setSelectionRange` and the DOM index UTF-16 code units, so the UI converts every span against the exact source string it sent:

```ts
/** A span as expressions.md reports it: UTF-8 byte offsets into the source, end exclusive. */
export type Utf8Span = { readonly start: number; readonly end: number }
/** The same range as UTF-16 indices into `source`, or null when a bound is past the end or inside a character's UTF-8 encoding. */
export function utf16Range(source: string, span: Utf8Span): { readonly start: number; readonly end: number } | null
```

`utf16Range` walks `source` by code points, counting 1 UTF-8 byte below U+0080, 2 below U+0800, 3 below U+10000 and 4 above (a code point above U+FFFF is 2 UTF-16 units); a lone surrogate counts 3 bytes, the length of the U+FFFD that `TextEncoder` writes for it. A byte offset maps to the UTF-16 index of the code point that starts there; the offset equal to the source's UTF-8 length maps to `source.length`. For `2 * Ø + x` (10 bytes, 9 UTF-16 units) the span 9..10 of `x` is 8..9, the span 4..6 of `Ø` is 4..5, and an offset of 5 (inside `Ø`) gives null. Spans reach the UI only when the core reports them with an `expression_error` (open issue 9); `CoreError` then carries the span and `useExpression` converts it, otherwise `range` is null.

### 2.14 Sketch module

```ts
// src/sketch/refs.ts: sketch.md's reserved references, fixed constants that constraints accept
// wherever they take a point or a line (no stored entities; entity and constraint ids may not use these names).
/** The sketch origin point, 'origin'. */
export const ORIGIN_POINT: PointRef
/** The sketch x axis, 'x_axis'. */
export const X_AXIS: string
/** The sketch y axis, 'y_axis'. */
export const Y_AXIS: string

// src/sketch/geometry.ts
export type Vector2 = readonly [number, number]
export function circleThroughThreePoints(first: Vector2, second: Vector2, third: Vector2): { center: Vector2; radius: number } | null
export function tessellateArc(center: Vector2, radius: number, start: number, end: number, toleranceMillimeters: number): Float32Array
export function distancePointToSegment(point: Vector2, start: Vector2, end: Vector2): number
export function distancePointToArc(point: Vector2, center: Vector2, radius: number, start: number, end: number): number

// src/sketch/recipes.ts
export type IdSource = { next(prefix: 'e' | 'c'): string }
export type SketchAddition = { readonly entities: readonly SketchEntity[]; readonly constraints: readonly SketchConstraint[] }
export function lineRecipe(input: { start: Vector2; end: Vector2; construction: boolean; ids: IdSource }): SketchAddition | null
export function rectangleTwoPointRecipe(input: { corner: Vector2; opposite: Vector2; construction: boolean; ids: IdSource }): SketchAddition | null
export function rectangleCenterRecipe(input: { center: Vector2; corner: Vector2; construction: boolean; ids: IdSource }): SketchAddition | null
export function circleRecipe(input: { center: Vector2; radius: number; construction: boolean; ids: IdSource }): SketchAddition | null
export function arcThreePointRecipe(input: { start: Vector2; end: Vector2; through: Vector2; construction: boolean; ids: IdSource }): SketchAddition | null
export function pointRecipe(input: { at: Vector2; construction: boolean; ids: IdSource }): SketchAddition

// src/sketch/inference.ts
export type SnapTarget =
  | { readonly kind: 'point'; readonly point: PointRef; readonly at: Vector2 }
  | { readonly kind: 'midpoint'; readonly line: string; readonly at: Vector2 }
  | { readonly kind: 'curve'; readonly entity: string; readonly at: Vector2 }
export type DirectionInference = { readonly kind: 'horizontal' } | { readonly kind: 'vertical' } | { readonly kind: 'tangent'; readonly entity: string }
export type InferenceInput = {
  readonly cursor: Vector2
  /** Sketch millimeters per CSS pixel at the cursor. */
  readonly millimetersPerPixel: number
  readonly solution: SketchSolution
  /** Start of the segment being drawn, for direction inference. */
  readonly anchor: { readonly at: Vector2; readonly point: PointRef | null } | null
  /** Tool being drawn, which decides which direction inferences apply. */
  readonly tool: SketchToolId
  readonly suppressed: boolean
}
export type InferenceResult = { readonly point: Vector2; readonly snap: SnapTarget | null; readonly direction: DirectionInference | null }
export function infer(input: InferenceInput): InferenceResult

// src/sketch/constraints.ts
export type ConstraintType = 'coincident' | 'point_on_object' | 'horizontal' | 'vertical' | 'parallel' | 'perpendicular' | 'tangent' | 'collinear' | 'concentric' | 'equal' | 'symmetric' | 'midpoint' | 'fix'
export type DimensionType = 'distance' | 'horizontal_distance' | 'vertical_distance' | 'radius' | 'diameter' | 'angle'
export function applicableConstraints(selection: readonly SelectionItem[], solution: SketchSolution): readonly ConstraintType[]
export function constraintFor(type: ConstraintType, selection: readonly SelectionItem[], solution: SketchSolution, ids: IdSource): SketchConstraint | null
export function constraintRequirement(type: ConstraintType): string

// src/sketch/dimensions.ts
export function dimensionTypeFor(selection: readonly SelectionItem[], labelAt: Vector2, solution: SketchSolution): DimensionType | { readonly unsupported: string }
export function measuredValue(type: DimensionType, selection: readonly SelectionItem[], labelAt: Vector2, solution: SketchSolution): number
export function dimensionFor(type: DimensionType, selection: readonly SelectionItem[], expression: string, driving: boolean, labelAt: Vector2, ids: IdSource): SketchConstraint
export function defaultLabelPosition(constraint: SketchConstraint, solution: SketchSolution, millimetersPerPixel: number): Vector2

// src/sketch/session.ts
export type SketchToolId = 'line' | 'rectangle_2point' | 'rectangle_center' | 'circle' | 'arc_3point' | 'point' | 'dimension' | `constrain_${ConstraintType}`
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
export type SketchCommitResult =
  | { readonly outcome: 'committed'; readonly revision: number }
  | { readonly outcome: 'over_constrained'; readonly constraint: string; readonly conflicting: readonly string[]; readonly redundant: boolean }
  | { readonly outcome: 'failed'; readonly message: string }
/** The step the sketch session waits for, which picks the step prompt (3.16.3). */
export type SketchStep =
  | { readonly kind: 'idle'; readonly empty: boolean }
  | { readonly kind: 'line'; readonly chain: boolean }
  | { readonly kind: 'rectangle_2point' | 'rectangle_center' | 'circle'; readonly clicks: 0 | 1 }
  | { readonly kind: 'arc_3point'; readonly clicks: 0 | 1 | 2 }
  | { readonly kind: 'point' }
  | { readonly kind: 'dimension'; readonly picked: 'nothing' | 'line' | 'point' | 'round' | 'pair' }
/** The step prompt for a step, shortcuts rendered with formatShortcut, or null when there is none. */
export function sketchStepHint(step: SketchStep, platform: Platform): string | null
/** The outline summary of the header (3.16.8): "1 open end", "{n} open ends", "1 outline problem" or "{n} outline problems"; null without profile findings. */
export function outlineProblems(solution: SketchSolution): string | null
```

`SketchEntity`, `SketchConstraint`, `SketchParams` and `PointRef` are the generated types from sketch.md's JSON. The recipes produce entities and constraints exactly as listed in section 3.16; their JSON is whatever sketch.md defines for those entity and constraint types.

### 2.15 Files, recent files, recovery and settings

`src/files/file-service.ts`:

```ts
export type FileKind = 'ptsm' | '3mf' | 'stl' | 'obj' | 'step'
/** A chosen file: a File System Access handle on the web, a host grant on desktop (desktop.md 2.11). */
export type DesktopFileHandle = { readonly kind: 'desktop'; readonly name: string; readonly grant: string }
export type FileHandle = FileSystemFileHandle | DesktopFileHandle
export type OpenedFile = { readonly name: string; readonly bytes: Uint8Array; readonly handle: FileHandle | null }
export type SaveTarget =
  | { readonly kind: 'handle'; readonly name: string; readonly handle: FileHandle }
  | { readonly kind: 'download'; readonly name: string }
export class FileServiceError extends Error { readonly reason: 'permission' | 'not_found' | 'io' }
export type FileService = {
  readonly fileSystemAccess: boolean
  /** Null when the user cancels. */
  pickToOpen(): Promise<OpenedFile | null>
  readHandle(handle: FileHandle): Promise<OpenedFile>
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

The desktop file service (Tauri dialogs) implements the same `FileService` type and is specified with the desktop host (open issue 2).

`src/files/recent.ts` and `src/files/recovery.ts`:

```ts
export type RecentFile = { readonly id: string; readonly name: string; readonly openedAt: number; readonly handle: FileHandle | null; readonly text: string | null }
export type RecentFilesStore = {
  list(): Promise<readonly RecentFile[]>
  remember(file: { readonly name: string; readonly handle: FileHandle | null; readonly text: string }): Promise<void>
  forget(id: string): Promise<void>
}
export function createIndexedDbRecentFiles(factory: IDBFactory, now?: () => number): RecentFilesStore
export function createMemoryRecentFiles(now?: () => number): RecentFilesStore

export type RecoveryEntry = { readonly name: string; readonly text: string; readonly savedAt: number; readonly handle: FileHandle | null; readonly reason: 'autosave' | 'replaced' }
export type RecoveryStore = { read(): Promise<RecoveryEntry | null>; write(entry: RecoveryEntry): Promise<void>; clear(): Promise<void> }
export function createIndexedDbRecovery(factory: IDBFactory): RecoveryStore
export function createMemoryRecovery(): RecoveryStore
```

- IndexedDB database `partsmith`, version 1, object stores `recent` (key path `id`) and `recovery` (key path `slot`, one record with slot `current`).
- `remember` keeps at most 10 entries, most recent first. A new entry replaces an existing one for the same file: same handle (`isSameEntry` resolves true) or, for handle-less entries, the same name. Entries with a handle store `text: null`; entries without one store the text (so reopening works without file access). `id` is `crypto.randomUUID()`.
- Every IndexedDB failure (private mode, quota, blocked) is caught: `list` and `read` resolve empty, writes resolve without effect, and a `console.warn` names the store. The app never depends on them working.

`src/settings/settings.ts`:

```ts
export type Settings = {
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

Stored as JSON under `localStorage` key `partsmith.settings.v1`; every change to the settings is saved at once. `DEFAULT_SETTINGS`: theme `system`, scale 1, navigation `fusion`, empty lists and map. Reading and writing wrap every access in `try`/`catch`; malformed or partial JSON yields defaults for the missing or invalid members. `paletteRecent` keeps at most 20 ids.

### 2.16 Theme: stylesheet, role-to-color mapping, fonts and UI scale

The visual direction is the Partsmith direction in [docs/guidelines/README.md](../guidelines/README.md), applied in 3.23. Everything visual is a Tailwind utility class; the stylesheet adds nothing of its own.

#### 2.16.1 Stylesheet

`src/index.css` is exactly the maintainer-approved block of ADR 0013:

```css
@import url('https://fonts.googleapis.com/css2?family=Inter:ital,opsz,wght@0,14..32,100..900;1,14..32,100..900&family=JetBrains+Mono:ital,wght@0,100..800;1,100..800&display=swap');
@import 'tailwindcss';

@custom-variant dark (&:where(.dark, .dark *));
```

There is no other CSS file, no CSS variable defined by the app and no `@theme` block: Tailwind's default theme and palette stay as Tailwind 4.3.3 ships them, and the `dark:` variant matches inside an element with the `dark` class. Changing this file needs human approval (ADR 0013, tailwind-css.md rule 2). U-91 checks it.

#### 2.16.2 The mapping module

`src/theme/roles.ts` is the single source of the role-to-color mapping, the font classes and the UI scale classes. It is the documented design-system mapping that tailwind-css.md allows to be single-sourced; it holds color, font and scale classes only, never layout. Every class string in it is a complete literal, so Tailwind's scanner finds it; nothing composes class names. Components combine the facets with their own inline layout classes through `clsx/lite`, for example `clsx('flex h-8 items-center gap-2 px-3', COLOR_CLASSES.text.foreground, COLOR_CLASSES.hover.background, selected && COLOR_CLASSES.selection.background)`.

```ts
export type ThemeName = 'light' | 'dark'
export type Facet = 'background' | 'active' | 'foreground' | 'placeholder' | 'border' | 'outline' | 'stroke' | 'fill'

/** sRGB '#rrggbb' of every Tailwind palette step the mapping uses (2.16.5), declared `as const`. */
export const PALETTE_HEX: { readonly white: '#ffffff'; readonly 'zinc-50': '#fafafa' /* ...one entry per row of 2.16.5 */ }
/** A palette step the mapping uses, such as 'zinc-900' or 'white'. */
export type PaletteStep = keyof typeof PALETTE_HEX

/** Interface roles and their facets as literal class strings (2.16.3), declared `as const`. */
export const COLOR_CLASSES: { readonly surface: { readonly background: 'bg-zinc-100 dark:bg-zinc-950' } /* ...one entry per role of 2.16.3 */ }
export type ColorRole = keyof typeof COLOR_CLASSES

export type ViewportRole =
  | 'viewportTop' | 'viewportBottom' | 'body' | 'edgeDark' | 'edgeLight' | 'highlight' | 'problem'
  | 'sketchUnder' | 'sketchFull' | 'sketchOver' | 'sketchConstruction' | 'sketchRegion' | 'sketchReference'
  | 'datum' | 'bed' | 'grid' | 'analysisLow' | 'analysisHigh'
/** Palette step of each viewport role per theme (2.16.4). */
export const VIEWPORT_STEPS: Readonly<Record<ViewportRole, Readonly<Record<ThemeName, PaletteStep>>>>
/** '#rrggbb' of each viewport role per theme: PALETTE_HEX[VIEWPORT_STEPS[role][theme]]. */
export const VIEWPORT_COLORS: Readonly<Record<ThemeName, Readonly<Record<ViewportRole, string>>>>

/** Body color swatches of the color popover (3.10), in order; `hex` null is Default (removes `Body.color`). */
export const BODY_SWATCHES: readonly { readonly name: string; readonly step: PaletteStep; readonly background: string; readonly hex: string | null }[]

/** Font family classes (2.16.7). */
export const FONT_CLASSES: { readonly interfaceText: 'font-[Inter,ui-sans-serif,system-ui,sans-serif]'; readonly values: "font-['JetBrains_Mono',ui-monospace,monospace]" }
/** Root font-size class of each UI scale (2.16.9). */
export const UI_SCALE_CLASSES: Readonly<Record<Settings['uiScale'], string>>
/** Classes the root element always carries: native controls and scrollbars follow the theme. */
export const ROOT_CLASSES: 'scheme-light dark:scheme-dark'

/** '#rrggbb' to linear RGB (sRGB decoding), for shading. */
export function linearFromHex(hex: string): Rgb
/** '#rrggbb' to sRGB-encoded channels divided by 255. */
export function encodedFromHex(hex: string): EncodedRgb
/** WCAG 2.2 contrast ratio of two '#rrggbb' colors (relative luminance with the 0.04045 sRGB threshold). */
export function contrastRatio(first: string, second: string): number
/** The backend's theme colors for one theme, linearized from VIEWPORT_COLORS. */
export function viewportTheme(theme: ThemeName): ViewportTheme
```

#### 2.16.3 Interface color roles

Each role names its Tailwind classes for the light theme and, with the `dark:` variant, for the dark theme; a role whose dark step equals its light step has no `dark:` class. Tailwind's `border` and `outline` utilities default to `currentColor`, so every border or outline also takes a color facet from this table.

| Role | Facet | Classes | Use |
| --- | --- | --- | --- |
| `surface` | background | `bg-zinc-100 dark:bg-zinc-950` | App root, top bar, browser, timeline, status bar, side panel |
| `surfaceRaised` | background | `bg-white dark:bg-zinc-900` | Inspector, menus, popovers, palette, tooltips, notifications, the sketch header, the legend and the chips over the viewport (empty state, hints, plane labels) |
| `field` | background | `bg-white dark:bg-zinc-950` | Text inputs, selects, selection boxes, segmented control tracks |
| `hover` | background | `hover:bg-zinc-200 dark:hover:bg-zinc-800` | Hovered rows, menu items, toolbar and icon buttons |
| | active | `bg-zinc-200 dark:bg-zinc-800` | The palette's active option and a menu's keyboard-active item |
| `selection` | background | `bg-orange-100 dark:bg-orange-950` | Selected rows and timeline items, pressed toggle buttons, the active tool, the checked segment of segmented controls (none of these change on hover) |
| `border` | border | `border-zinc-200 dark:border-zinc-800` | Hairline (`border`, 1 CSS pixel by Tailwind's default) dividers and outlines of panels, popovers and chips; decorative |
| `borderControl` | border | `border-zinc-500` | Boundaries of inputs, selects, switches, swatches and selection boxes |
| `text` | foreground | `text-zinc-900 dark:text-zinc-100` | Primary text and icons |
| `textMuted` | foreground | `text-zinc-600 dark:text-zinc-400` | Secondary text, captions, units, palette details, idle icons, the info notification icon |
| | placeholder | `placeholder:text-zinc-600 dark:placeholder:text-zinc-400` | Input placeholders, such as the live measured value in an empty draw input (3.16.4) |
| `textDisabled` | foreground | `text-zinc-400 dark:text-zinc-600` | Disabled text and icons (incidental, exempt from contrast) |
| `accent` | background | `bg-orange-600 dark:bg-orange-500` | Primary button fill (OK, Finish sketch, Export 3MF, Export), the rollback marker line |
| | foreground | `text-zinc-950` | Text and icons on the accent fill |
| | border | `border-orange-600 dark:border-orange-500` | The active selection field; required empty selection fields (with `border-dashed`) |
| | outline | `outline-orange-600 dark:outline-orange-500` | The timeline item of the feature being edited and of the sketch being edited (with `outline-2`) |
| | stroke | `stroke-orange-600 dark:stroke-orange-500` | The box selection rectangle (SVG overlay) |
| `accentHover` | background | `hover:bg-orange-500 dark:hover:bg-orange-400` | Primary button hover |
| `focusRing` | outline | `focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-orange-600 dark:focus-visible:outline-orange-500` | Keyboard focus on every interactive element |
| `errorMark` | foreground | `text-red-700 dark:text-red-500` | Error icons (`circle-x`) |
| | border | `border-red-700 dark:border-red-500` | Invalid field borders |
| | stroke | `stroke-red-700 dark:stroke-red-500` | Profile issue rings and finding markers (SVG overlay) |
| `errorText` | foreground | `text-red-700 dark:text-red-300` | Error messages, error states of the sketch header, parameter values and status bar |
| `errorSubtle` | background | `bg-red-50 dark:bg-red-950` | Error banners (`inspector-error` with an error) |
| `errorFill` | background | `bg-red-600` | Chips of conflicting constraint glyphs and dimension labels |
| | foreground | `text-white` | Text and icons on the error fill |
| `warningMark` | fill | `fill-yellow-400` | Fill of the `triangle-alert` icon, drawn as a hazard sign |
| | foreground | `text-zinc-950` | Outline and exclamation mark of that icon (lucide strokes use `currentColor`) |
| `warningText` | foreground | `text-yellow-800 dark:text-yellow-300` | Warning text (the outline and extra-constraint suffixes of the sketch header, 3.16.8) |
| `warningSubtle` | background | `bg-yellow-50 dark:bg-yellow-950` | Warning banners (`inspector-error` with warnings only) |
| `warningFill` | background | `bg-yellow-400` | Chips of redundant constraint glyphs and dimension labels |
| | foreground | `text-zinc-950` | Text and icons on the warning fill |
| `successMark` | foreground | `text-green-700 dark:text-green-400` | Success icons (`circle-check`) |
| `analysis` | background | `bg-linear-to-r/srgb from-sky-200 to-violet-700` | The overhang legend gradient, interpolated in sRGB like the viewport ramp (3.12 pass 3) |

Color carries meaning and nothing else: orange (`accent`, `selection`, `highlight`) means selection, focus, the current tool and the primary action; blue means under-constrained sketch geometry and closed regions; red means errors and conflicts; a yellow hazard sign means a warning; green means success; sky to violet is the analysis ramp. Warnings never use orange, and errors never use the accent's steps. The values need human review at the gate (open issue 13).

#### 2.16.4 Viewport color roles

The WebGL viewport needs numbers. `VIEWPORT_STEPS` holds the steps below and `VIEWPORT_COLORS` their hex values from `PALETTE_HEX`, so DOM and viewport draw from the same palette steps. Roles shared with the interface use its steps: `highlight` is `accent`, `problem` and `sketchOver` are `errorMark`, `body` is the Default swatch.

| Role | Light | Dark | Use |
| --- | --- | --- | --- |
| `viewportTop` | `white` `#ffffff` | `zinc-800` `#27272a` | Background gradient top |
| `viewportBottom` | `zinc-100` `#f4f4f5` | `zinc-900` `#18181b` | Background gradient bottom |
| `body` | `zinc-400` `#9f9fa9` | `zinc-400` `#9f9fa9` | Default body color (no `Body.color`) |
| `edgeDark` | `zinc-900` `#18181b` | `zinc-900` `#18181b` | Edges on light bodies |
| `edgeLight` | `zinc-200` `#e4e4e7` | `zinc-200` `#e4e4e7` | Edges on dark bodies |
| `highlight` | `orange-600` `#f54900` | `orange-500` `#ff6900` | Hover and selection tints, hovered and selected sketch items, snap targets |
| `problem` | `red-700` `#c10007` | `red-500` `#fb2c36` | Problem tints and profile issue halos |
| `sketchUnder` | `blue-600` `#155dfc` | `blue-400` `#51a2ff` | Under-constrained sketch geometry |
| `sketchFull` | `zinc-900` `#18181b` | `zinc-50` `#fafafa` | Fully constrained sketch geometry |
| `sketchOver` | `red-700` `#c10007` | `red-500` `#fb2c36` | Over-constrained (conflicting) sketch geometry |
| `sketchConstruction` | `zinc-600` `#52525c` | `zinc-300` `#d4d4d8` | Fully constrained construction geometry, dashed (3.16.2) |
| `sketchRegion` | `blue-500` `#2b7fff` | `blue-400` `#51a2ff` | Closed region fill, at 12 percent opacity |
| `sketchReference` | `zinc-500` `#71717b` | `zinc-400` `#9f9fa9` | Origin axes, other sketches, dimension lines |
| `datum` | `zinc-400` `#9f9fa9` | `zinc-500` `#71717b` | Datum and origin plane fill at 15 percent, outline at 60 percent |
| `bed` | `zinc-500` `#71717b` | `zinc-400` `#9f9fa9` | Printer bed outline |
| `grid` | `zinc-300` `#d4d4d8` | `zinc-600` `#52525c` | Ground and sketch grid lines (minor at 35 percent, major at 70 percent opacity, 3.12) |
| `analysisLow` | `sky-200` `#b8e6fe` | `sky-200` `#b8e6fe` | Overhang color map at the limit angle |
| `analysisHigh` | `violet-700` `#7008e7` | `violet-700` `#7008e7` | Overhang color map at 90 degrees |

The grid is decorative and never the only signal of anything (the status bar states its spacing, 3.21), so it has no row in 2.16.6.

The viewport is the brightest surface in the light theme and lighter than the chrome in the dark theme, so the model sits on a lit stage and the chrome recedes. Edges are drawn in `edgeDark` or `edgeLight`, whichever has the higher contrast ratio against the body's base color, so they stay visible on any body color. The analysis ramp is cool because orange, yellow and red already mean selection, warnings and errors; both of its ends lie inside the sRGB gamut, so the legend and the viewport show the same colors.

`viewportTheme(theme)` returns `linearFromHex` of `viewportTop`, `viewportBottom`, `highlight`, `problem`, `edgeDark` and `edgeLight`; the scene converts the body, sketch, datum and bed colors with `linearFromHex` for each draw, and passes `encodedFromHex` of the analysis ends in `RenderFrame.analysis`.

#### 2.16.5 Palette steps and body swatches

Tailwind 4.3.3 defines its palette in OKLCH. The hex of a step is the CSS Color 4 conversion of its OKLCH value (OKLCH to OKLab to linear sRGB), each channel clipped to [0, 1], sRGB-encoded and rounded to 8 bits. Steps marked clipped lie partly outside the sRGB gamut; on wide-gamut displays the DOM shows them a little more saturated than the hex the viewport draws (open issue 11). U-90 recomputes every value from `tailwindcss/colors`.

| Step | Tailwind 4.3.3 value | sRGB hex | In sRGB gamut |
| --- | --- | --- | --- |
| `white` | `#fff` | `#ffffff` | yes |
| `zinc-50` | `oklch(98.5% 0 none)` | `#fafafa` | yes |
| `zinc-100` | `oklch(96.7% 0.001 286.375)` | `#f4f4f5` | yes |
| `zinc-200` | `oklch(92% 0.004 286.32)` | `#e4e4e7` | yes |
| `zinc-300` | `oklch(87.1% 0.006 286.286)` | `#d4d4d8` | yes |
| `zinc-400` | `oklch(70.5% 0.015 286.067)` | `#9f9fa9` | yes |
| `zinc-500` | `oklch(55.2% 0.016 285.938)` | `#71717b` | yes |
| `zinc-600` | `oklch(44.2% 0.017 285.786)` | `#52525c` | yes |
| `zinc-800` | `oklch(27.4% 0.006 286.033)` | `#27272a` | yes |
| `zinc-900` | `oklch(21% 0.006 285.885)` | `#18181b` | yes |
| `zinc-950` | `oklch(14.1% 0.005 285.823)` | `#09090b` | yes |
| `orange-100` | `oklch(95.4% 0.038 75.164)` | `#ffedd4` | no, clipped |
| `orange-400` | `oklch(75% 0.183 55.934)` | `#ff8904` | no, clipped |
| `orange-500` | `oklch(70.5% 0.213 47.604)` | `#ff6900` | no, clipped |
| `orange-600` | `oklch(64.6% 0.222 41.116)` | `#f54900` | no, clipped |
| `orange-950` | `oklch(26.6% 0.079 36.259)` | `#441306` | yes |
| `yellow-50` | `oklch(98.7% 0.026 102.212)` | `#fefce8` | yes |
| `yellow-300` | `oklch(90.5% 0.182 98.111)` | `#ffdf20` | no, clipped |
| `yellow-400` | `oklch(85.2% 0.199 91.936)` | `#fdc700` | no, clipped |
| `yellow-800` | `oklch(47.6% 0.114 61.907)` | `#894b00` | no, clipped |
| `yellow-950` | `oklch(28.6% 0.066 53.813)` | `#432004` | yes |
| `red-50` | `oklch(97.1% 0.013 17.38)` | `#fef2f2` | yes |
| `red-300` | `oklch(80.8% 0.114 19.571)` | `#ffa2a2` | no, clipped |
| `red-500` | `oklch(63.7% 0.237 25.331)` | `#fb2c36` | yes |
| `red-600` | `oklch(57.7% 0.245 27.325)` | `#e7000b` | no, clipped |
| `red-700` | `oklch(50.5% 0.213 27.518)` | `#c10007` | no, clipped |
| `red-950` | `oklch(25.8% 0.092 26.042)` | `#460809` | yes |
| `green-400` | `oklch(79.2% 0.209 151.711)` | `#05df72` | yes |
| `green-600` | `oklch(62.7% 0.194 149.214)` | `#00a63e` | no, clipped |
| `green-700` | `oklch(52.7% 0.154 150.069)` | `#008236` | no, clipped |
| `teal-600` | `oklch(60% 0.118 184.704)` | `#009689` | no, clipped |
| `sky-200` | `oklch(90.1% 0.058 230.902)` | `#b8e6fe` | yes |
| `blue-400` | `oklch(70.7% 0.165 254.624)` | `#51a2ff` | no, clipped |
| `blue-500` | `oklch(62.3% 0.214 259.815)` | `#2b7fff` | no, clipped |
| `blue-600` | `oklch(54.6% 0.245 262.881)` | `#155dfc` | yes |
| `violet-700` | `oklch(49.1% 0.27 292.581)` | `#7008e7` | yes |
| `purple-600` | `oklch(55.8% 0.288 302.321)` | `#9810fa` | yes |
| `pink-600` | `oklch(59.2% 0.249 0.584)` | `#e60076` | no, clipped |
| `amber-800` | `oklch(47.3% 0.137 46.201)` | `#973c00` | no, clipped |

Body color swatches (3.10), `BODY_SWATCHES` in this order; each swatch button has the background class `bg-{step}` with a `borderControl` border, and choosing it stores the hex in `Body.color`:

| Name | Step | Stored `Body.color` |
| --- | --- | --- |
| Default | `zinc-400` (the `body` role) | none: removes `color` |
| White | `zinc-100` | `#f4f4f5` |
| Black | `zinc-800` | `#27272a` |
| Gray | `zinc-500` | `#71717b` |
| Red | `red-600` | `#e7000b` |
| Orange | `orange-500` | `#ff6900` |
| Yellow | `yellow-400` | `#fdc700` |
| Green | `green-600` | `#00a63e` |
| Teal | `teal-600` | `#009689` |
| Blue | `blue-600` | `#155dfc` |
| Purple | `purple-600` | `#9810fa` |
| Pink | `pink-600` | `#e60076` |
| Brown | `amber-800` | `#973c00` |

#### 2.16.6 Contrast and distinctness

WCAG 2.2 AA: text at least 4.5:1 (every text pair is held to 4.5:1, large or not) and non-text at least 3:1 (control boundaries, focus indicators, meaningful icons and viewport geometry). The ratios are computed from the hex values of 2.16.5 with the WCAG relative luminance, truncated to two decimals; U-70 recomputes each pair and checks that it equals the table and meets the threshold.

Text:

| Text role | On | Required | Light | Dark |
| --- | --- | --- | --- | --- |
| `text` | `surface` | 4.5 | 16.11 | 18.10 |
| `text` | `surfaceRaised` | 4.5 | 17.71 | 16.11 |
| `text` | `field` | 4.5 | 17.71 | 18.10 |
| `text` | `hover` | 4.5 | 13.96 | 13.55 |
| `text` | `selection` | 4.5 | 15.45 | 14.24 |
| `text` | `errorSubtle` | 4.5 | 16.19 | 14.70 |
| `text` | `warningSubtle` | 4.5 | 17.12 | 13.20 |
| `textMuted` | `surface` | 4.5 | 7.02 | 7.58 |
| `textMuted` | `surfaceRaised` | 4.5 | 7.71 | 6.75 |
| `textMuted` | `field` | 4.5 | 7.71 | 7.58 |
| `textMuted` | `hover` | 4.5 | 6.08 | 5.67 |
| `textMuted` | `selection` | 4.5 | 6.73 | 5.96 |
| `errorText` | `surface` | 4.5 | 5.84 | 10.36 |
| `errorText` | `surfaceRaised` | 4.5 | 6.42 | 9.23 |
| `errorText` | `errorSubtle` | 4.5 | 5.86 | 8.42 |
| `warningText` | `surface` | 4.5 | 6.22 | 14.99 |
| `warningText` | `surfaceRaised` | 4.5 | 6.84 | 13.35 |
| `warningText` | `warningSubtle` | 4.5 | 6.61 | 10.93 |
| `onAccent` | `accent` | 4.5 | 5.52 | 6.89 |
| `onAccent` | `accentHover` | 4.5 | 6.89 | 8.36 |
| `onWarningFill` | `warningFill` | 4.5 | 12.64 | 12.64 |
| `onErrorFill` | `errorFill` | 4.5 | 4.76 | 4.76 |

Non-text (`warningMark` is measured on its `zinc-950` outline in the light theme and on its `yellow-400` fill in the dark theme, the part that contrasts with the surface; `accent` on `selection` is the focus ring of a selected row):

| Mark role | Against | Required | Light | Dark |
| --- | --- | --- | --- | --- |
| `borderControl` | `surface` | 3 | 4.39 | 4.12 |
| `borderControl` | `surfaceRaised` | 3 | 4.82 | 3.67 |
| `borderControl` | `field` | 3 | 4.82 | 4.12 |
| `accent` | `surface` | 3 | 3.27 | 6.89 |
| `accent` | `surfaceRaised` | 3 | 3.59 | 6.13 |
| `accent` | `selection` | 3 | 3.13 | 5.42 |
| `accent` | `viewportTop` | 3 | 3.59 | 5.15 |
| `accent` | `viewportBottom` | 3 | 3.27 | 6.13 |
| `errorMark` | `surface` | 3 | 5.84 | 5.22 |
| `errorMark` | `surfaceRaised` | 3 | 6.42 | 4.65 |
| `errorMark` | `viewportTop` | 3 | 6.42 | 3.91 |
| `errorMark` | `viewportBottom` | 3 | 5.84 | 4.65 |
| `successMark` | `surface` | 3 | 4.49 | 11.18 |
| `successMark` | `surfaceRaised` | 3 | 4.94 | 9.95 |
| `warningMark` | `surface` | 3 | 18.10 | 12.64 |
| `warningMark` | `surfaceRaised` | 3 | 19.89 | 11.25 |
| `warningMark` | `hover` | 3 | 15.67 | 9.46 |
| `warningMark` | `selection` | 3 | 17.35 | 9.94 |
| `sketchUnder` | `viewportTop` | 3 | 5.24 | 5.64 |
| `sketchUnder` | `viewportBottom` | 3 | 4.77 | 6.71 |
| `sketchFull` | `viewportTop` | 3 | 17.71 | 14.27 |
| `sketchFull` | `viewportBottom` | 3 | 16.11 | 16.97 |
| `sketchOver` | `viewportTop` | 3 | 6.42 | 3.91 |
| `sketchOver` | `viewportBottom` | 3 | 5.84 | 4.65 |
| `sketchConstruction` | `viewportTop` | 3 | 7.71 | 10.07 |
| `sketchConstruction` | `viewportBottom` | 3 | 7.02 | 11.98 |
| `sketchReference` | `viewportTop` | 3 | 4.82 | 5.67 |
| `sketchReference` | `viewportBottom` | 3 | 4.39 | 6.75 |
| `bed` | `viewportTop` | 3 | 4.82 | 5.67 |
| `bed` | `viewportBottom` | 3 | 4.39 | 6.75 |

The lowest ratios are the light theme's `accent` marks (3.13 to 3.59); they use `orange-600` because the brighter `orange-500` falls below 3:1 on white (2.89). Distinctness, as OKLab distances from the OKLCH values (about 0.02 is one just-noticeable difference): `accent` and `errorMark` 0.150 light and 0.113 dark (hues 14 and 22 degrees apart, and the light red is darker); `accent` and the warning yellow 0.275 and 0.214 (51 and 44 degrees apart); `errorText` and `warningText` 0.138 and 0.217; `accent` and `sketchUnder` 0.448 and 0.368. Hue is never the only signal (3.24).

#### 2.16.7 Fonts

`FONT_CLASSES.interfaceText` (`font-[Inter,ui-sans-serif,system-ui,sans-serif]`) and `FONT_CLASSES.values` (`font-['JetBrains_Mono',ui-monospace,monospace]`) are arbitrary font-family values, because no `@theme` defines font tokens; Tailwind emits `font-family: Inter,ui-sans-serif,system-ui,sans-serif` and `font-family: 'JetBrains Mono',ui-monospace,monospace`. Both families come from the Google Fonts import in `src/index.css`.

- `interfaceText` is on the app root element and on the unsupported page's root, so all interface text inherits Inter.
- `values` is on values only: expression inputs and `expression-result`, the options of `expression-suggestions`, draw inputs, the dimension editor's input, sketch dimension labels, parameter expressions and `parameter-value`, `body-size`, `body-center`, `entity-size`, the values of `measure-row` and the area, length and position values of the selection inspector, the number in `status-recompute` and `status-dof`, and the hex field of the color popover. Labels, headings, buttons, menus, tooltips and shortcut hints stay in Inter (the frontend guideline's avoid list names monospace labels).
- When the fonts cannot load (offline), the stacks fall back to the system UI and monospace fonts (open issue 10).

#### 2.16.8 Theme switching

`src/theme/theme.ts`:

```ts
export function resolveTheme(setting: Settings['theme'], prefersDark: boolean): ThemeName
/** Adds the `dark` class to the root element for 'dark' and removes it for 'light'. */
export function applyTheme(root: HTMLElement, theme: ThemeName): void
/** Replaces the root element's UI scale class with UI_SCALE_CLASSES[scale]. */
export function applyUiScale(root: HTMLElement, scale: Settings['uiScale']): void
/** The explicit theme the toggle switches to: the opposite of the resolved theme. */
export function toggledTheme(resolved: ThemeName): ThemeName
```

- The root element is `document.documentElement`. Before the first render `main.tsx` adds `ROOT_CLASSES` to it and applies the resolved theme and the UI scale; `index.html` declares `<meta name="color-scheme" content="light dark">` so the page before the bundle runs follows the OS too.
- Default: `settings.theme` is `system`, which follows `matchMedia('(prefers-color-scheme: dark)')`, including changes while the app runs (a `change` listener).
- Toggle: the top bar's "Dark theme" button (3.2) and the palette command "Toggle dark theme" set `settings.theme` to `toggledTheme(resolved)`. The Document menu's Theme submenu and the palette commands "Theme: System", "Theme: Light" and "Theme: Dark" choose any of the three; System returns to following the OS.
- Persistence: every settings change is saved at once with `saveSettings` under the localStorage key `partsmith.settings.v1` (2.15), so the choice survives reloads. When localStorage fails, the choice stays in effect for the session.
- A change of the resolved theme re-renders the viewport with `viewportTheme(theme)`.

#### 2.16.9 UI scale

`UI_SCALE_CLASSES` maps `1` to `text-[100%]`, `1.25` to `text-[125%]`, `1.5` to `text-[150%]`, `2` to `text-[200%]` and `3` to `text-[300%]`; `applyUiScale` puts the class on `<html>`, setting the root font size as a percentage of the browser default. These are arbitrary values because Tailwind has no root font-size utility, and they are percentages, never pixels. All chrome sizes are Tailwind rem classes, so they follow the scale; container queries in rem (the top bar's `@7xl`, 3.2) follow it too, while media query breakpoints do not (CSS evaluates rem in media queries against the initial font size). Viewport quantities this spec gives in CSS pixels (line widths, pick radii, snap distances, overlay offsets) are multiplied by the scale in code.

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
/** "Top plane (XY)", "Front plane (XZ)" or "Right plane (YZ)" (3.10). */
export function originPlaneName(plane: 'XY' | 'XZ' | 'YZ'): string
```

New features are created without a `name`; their label comes from `featureLabel`. New bodies are named with `nextBodyName`.

### 2.18 Performance marks

`src/performance/marks.ts` records User Timing entries that performance tests read; they are cheap and stay in production builds.

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

1. `main.tsx` checks capabilities before anything else: `typeof WebAssembly === 'object'` and a non-null `document.createElement('canvas').getContext('webgl2')`. If either fails it renders only the unsupported page (`data-testid="unsupported"`, with the interface font and the `surface` and `text` roles of 2.16) with the message from 4.3 and starts nothing else.
2. It loads settings, adds `ROOT_CLASSES` to `<html>` and applies the resolved theme (the `dark` class) and the UI scale class to it (2.16.8, 2.16.9), creates the transport with `chooseTransport(window)`, the stores and services, calls `createApp`, installs the shortcut dispatcher, and renders `App`.
3. The shell renders at once. The viewport shows the loading overlay (`data-testid="core-overlay"`, text "Loading the geometry core") while `app.store.getState().core.status` is `loading`.
4. `startCore` runs `transport.start()`, then `featureCatalog()` and `listPrinterProfiles()` in parallel, registers the feature commands, and sets the core status to `ready`. A failure sets it to `failed` and shows the fatal state (4.4).
5. It reads the recovery store. An entry produces the recovery notification (3.19); the app does not open it automatically.
6. It opens a new document (`newDocument('mm')`) named "Untitled". The `app` root element gets `data-core-status="ready"` only after this document's first `document_changed` is applied.
7. When the first viewport frame renders with that document, `ps:usable` is marked.

### 3.2 Layout regions

The window is a fixed layout with no page scroll. The app root element (`data-testid="app"`) is `h-dvh w-dvw overflow-hidden` with `FONT_CLASSES.interfaceText`, `surface` and `text` (2.16), and `antialiased`. Sizes are Tailwind classes on the rem scale (at 100 percent UI scale, 1rem is 16 CSS pixels):

| Region | Element | Size | Content |
| --- | --- | --- | --- |
| Top bar | `<header>`, `data-testid="top-bar"` | full width, `h-12` | Browser toggle, document name and save state, context switcher, context toolbar, palette button, Export button, theme toggle |
| Browser | `<nav aria-label="Browser">`, `data-testid="browser"` | `w-66`, left | Tree (3.10) |
| Viewport | `<main aria-label="3D view">`, `data-testid="viewport"` | remaining space | Canvas, DOM overlay, view cube, viewport controls, top-center slot (empty state, sketch header and hint, print hint), legends, inspector, notifications |
| Side panel | `<aside>`, `data-testid="side-panel"` | `w-90`, right, only while open | Parameters, Print check, Problems or Keyboard and mouse (one at a time) |
| Timeline | `<section aria-label="Timeline">`, `data-testid="timeline"` | full width, `h-14` (`h-7` collapsed) | Timeline (3.9) |
| Status bar | `<footer>`, `data-testid="status-bar"` | full width, `h-7` | Status items (3.21) |

Top bar, left to right:

- Browser toggle: icon button "Toggle browser" (`panel-left`), `aria-pressed` reflecting the browser state.
- Document name: a button (`data-testid="document-name"`, text the document name, `data-dirty="true|false"`) that opens the Document menu. Next to it, `data-testid="save-state"` shows "Edited" in `textMuted` while the document is dirty (core `dirty` or `localUnsaved`) and is empty otherwise.
- Context switcher: `role="radiogroup"` with `aria-label="Context"` and three radios "Model", "Sketch", "Print" (segmented control, one tab stop, arrow keys move).
- Context toolbar: `role="toolbar"` with `aria-label` "Model tools", "Sketch tools" or "Print tools" (3.3).
- Palette button: "Search" with the formatted `Mod+K` hint (`data-testid="palette-button"`).
- Export button: "Export" (`download` icon) opening the export popover (3.18).
- Theme toggle (`data-testid="theme-toggle"`): icon button "Dark theme" (`moon` icon), `aria-pressed` true while the resolved theme is dark; it switches to the opposite theme and persists the choice (2.16.8).

Document menu (`role="menu"`, `data-testid="document-menu"`), in order: New (desktop shortcut shown), Open..., Open recent (submenu, 3.19), Save, Save as..., Export..., separator, Theme (submenu of radio items System, Light, Dark), Navigation (submenu: Fusion, FreeCAD), UI scale (submenu: 100%, 125%, 150%, 200%, 300%).

Responsive rules:

- When the window is narrower than 1024 CSS pixels at startup (`window.innerWidth < 1024`), the browser starts closed. Below the `lg` breakpoint (64rem, 1024 CSS pixels) the browser and side panel open as overlays above the viewport (`max-lg:absolute`; they do not resize it) and close on Esc or on a pointer press outside them.
- The top bar is a container (`@container`). Toolbar buttons show icon and label when it is at least 80rem wide (`@7xl:`, 1280 CSS pixels at 100 percent UI scale and proportionally more at larger scales) and icon only below (the label stays the accessible name and appears in the tooltip). Primary-style toolbar buttons (Finish sketch, Export 3MF) keep their label down to 64rem (`@5xl:`, 1024 CSS pixels at 100 percent), so the way out of a sketch is named on common laptop windows; below that they are icon only like the others (3.24).
- The minimum supported window is 800 by 600 CSS pixels at 100 percent scale.

Floating elements (all `surfaceRaised` with a `border` hairline, `rounded-md` and `shadow-lg`): the inspector (3.7, 3.8) inside the viewport; the notifications stack at the viewport's bottom-right (3.22); the palette, `w-140 max-w-full`, centered in a layer `fixed inset-x-4 top-1/6 flex justify-center` (so it keeps 1rem from the window's sides at any UI scale); menus, popovers and tooltips anchored to their triggers (native Popover API, light dismiss), positioned at runtime (3.12).

Top-center slot: a column at the viewport's top (`absolute top-4 inset-x-32 flex flex-col items-center gap-2 pointer-events-none`; the `inset-x-32` margin keeps it clear of the `size-24` view cube at any UI scale) holding one of: the empty-state chip (below); the sketch header (3.16.8) with the sketch hint (3.16.3) under it; or the print hint (3.17). Interactive children are `pointer-events-auto`, so the slot never blocks picking.

Empty state: while `selectEmptyState(state)` holds (a ready core, a document without features, no feature command, no sketch session), the slot shows a chip (`data-testid="empty-state"`, `role="group"`, `aria-label="First step"`, `surfaceRaised` with a `border` hairline, `rounded-sm`, `p-3`, `max-w-140`):

- Line 1 (`text-sm`): "Start with a sketch on a plane, or add a box." followed by the buttons "Sketch" (runs `feature.sketch`) and "Box" (runs `feature.box`; left out when the catalog has no `box`).
- Line 2 (`text-xs`, `textMuted`): `navigationLegend(settings.navigation)` (2.11), a space, and "Search every command: {formatShortcut('Mod+K')}."

The chip follows the document only: it disappears with the first feature, returns whenever the document has no features again (Delete, Undo, New), and has no first-run flag and no setting, so an experienced user sees it only in an empty document, where it costs nothing. In a document without features the origin axes and point are shown too (3.10), and the ground grid gives scale (3.12).

### 3.3 Contexts and toolbars

`TOOLBARS` (labels are command titles; feature titles are manifest labels):

| Context | Entries in order |
| --- | --- |
| Model | Sketch (`feature.sketch`), Extrude, Revolve, Hole, Fillet, Chamfer, Shell, group "Shapes" (`cuboid` icon: Box, Cylinder, Sphere, Cone, Torus), group "Pattern" (`grid-3x3` icon: Linear pattern, Circular pattern), Mirror, Boolean |
| Sketch | Line, group "Rectangle" (`rectangle-horizontal`: 2-point rectangle, Center rectangle), Circle, Arc, Point, Construction, Dimension, group "Constrain" (`magnet`: the 13 constraint commands in the order of 3.16.6), separator, Finish sketch (primary style) |
| Print | Printer (menu button showing "Printer: {name}"), Overhang map (toggle), Checks (popover), Print check (opens the print check panel; shows the finding count), Lay on face (toggle, 3.17), separator, Export 3MF (primary style), group "Export" (`file-output`: STL, OBJ, STEP) |

Datum plane, Move body and Split body are in the palette only; the selection inspector also suggests them for what is selected (3.8). A group button opens a menu (`role="menu"`) of its commands with icon, title and shortcut. Disabled toolbar commands render with `aria-disabled="true"`, keep their place, show the reason in their tooltip, and show it as an info notification when clicked (as their shortcut does, 2.10). A group button whose commands are all disabled is disabled the same way with the first command's reason.

Context rules:

- The app starts in Model. `data-context` on the `app` root mirrors the context.
- Model to Print, or Print to Model: an active feature command is canceled first (`cancel_transaction`). The selection is kept.
- Choosing the Sketch radio (or `context.sketch`) outside sketch mode runs `enterSketch` when the app selection is exactly one sketch feature, or items of one sketch (curves, points, regions), and `startNewSketch` (3.16.1) otherwise; its tooltip reads "Edit {label}" or "New sketch". The toolbar's Sketch (`feature.sketch`) always starts a new sketch. In sketch mode, choosing Model or Print finishes the sketch (3.16.11) and then switches.
- A feature command run from Sketch finishes the sketch, switches to Model and starts the command. When the app selection is empty, the finished sketch preselects: for a `sketch_region` field, its region when it has exactly one; for a `sketch_point` field, its non-construction point entities, or, when it has none, the centers (`{entity}.center`) of its non-construction circles and arcs. Run from Print, it switches to Model first.
- Print commands are listed and active in Print only. Sketch tools are listed and active in Sketch only, and only while a sketch is being edited (not during plane picking).

### 3.4 Command palette

- Opens with `Mod+K` (from anywhere, including text fields) or `S` (when focus is not editable), or the palette button. `Mod+K` while open closes it.
- Layout: a container `role="dialog"` with `aria-label="Command palette"` and `data-testid="palette"`, holding a `role="combobox"` input (`data-testid="palette-input"`, placeholder "Search commands, features, bodies, parameters and settings") with `aria-controls` pointing at a `role="listbox"`, and a footer. Options (`role="option"`, `data-testid="palette-option"`, `data-item-id`, `data-kind`) show icon, title, detail (`textMuted`), a kind tag for non-commands ("Feature", "Body", "Parameter", "Recent file", `text-xs` in `textMuted`), and the formatted shortcut (`data-testid="palette-shortcut"`, `textMuted`, Inter). The active option has the `hover` role's `active` background.
- Items (`paletteItems`): every registered command whose availability is not `hidden` in the current context; every feature in the document ("{label}", kind feature, runs: select it in the timeline and scroll it into view); every body ("{name}", kind body, runs: select it and frame it); every parameter ("{name} = {expr}", kind parameter, runs: open the Parameters panel with that row's expression input focused); recent files ("Open recent: {name}", kind recent). Settings are ordinary commands of category Settings: "Theme: System", "Theme: Light", "Theme: Dark", "Navigation: Fusion", "Navigation: FreeCAD", "UI scale: 100%" to "UI scale: 300%", each with `checked` showing the current value.
- Ranking: `rankPaletteItems` with `settings.paletteRecent` and the current context (2.10). The first option is active when the list changes.
- With an empty query the options are grouped under headings (`role="group"` with `aria-label` the heading; the heading is `text-xs` in `textMuted`, `data-testid="palette-heading"`, and is not an option): "Recent" when there are recent items, then the category names in the context order of 2.10. So every command can be browsed without knowing its name. Arrow keys move over options only.
- When a non-empty query matches nothing, the listbox shows one row that is not an option (`data-testid="palette-empty"`, `textMuted`): "Nothing matches {query}."
- Keys: ArrowDown and ArrowUp move the active option and wrap; PageDown and PageUp move by 8; Enter runs the active option and closes the palette; Esc closes it. Focus stays in the input. Tab is ignored.
- Mouse: hovering an option makes it active; clicking runs it.
- Footer (`data-testid="palette-footer"`, `text-xs` in `textMuted`): the active option's description (2.10), which the option references with `aria-describedby` (the footer is not a live region, so arrow keys are not announced twice). A disabled option is dimmed with its reason as the detail; Enter on it shows the reason in a `role="status"` line of the footer instead and keeps the palette open.
- Running any item records its id at the front of `settings.paletteRecent`; running a feature command from any other source (toolbar, shortcut, menu, suggestion) records it too.
- Closing returns focus to the element that had it before the palette opened. A pointer press outside closes it.
- Each keystroke re-ranks synchronously; `ps:palette` measures it.

### 3.5 Keyboard shortcuts

Registry shortcuts (displayed with `formatShortcut`; `Mod` is Cmd on macOS and Ctrl elsewhere):

| Command id | Title | Shortcut | Contexts |
| --- | --- | --- | --- |
| `app.palette` | Command palette | Mod+K, S | all |
| `app.keys` | Keyboard and mouse | F1 | all |
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
| `edit.repeat` | Repeat {title} ("Repeat" before any feature command has run) | model, print |
| `edit.suppress`, `edit.unsuppress` | Suppress, Unsuppress | model, print (features selected) |
| `edit.clear_selection` | Clear selection | all |
| `view.parameters`, `view.problems`, `view.health` | Parameters, Problems, Print check | all |
| `view.timeline` | Toggle timeline | all |
| `view.grid` | Show grid (toggle, `checked` while the grid is drawn; session state, 3.12) | model, sketch |
| `view.standard.<view>` for the seven standard views | View: Top, View: Bottom, View: Front, View: Back, View: Left, View: Right, View: Home | all |
| `view.filter.body`, `view.filter.face`, `view.filter.edge`, `view.filter.vertex` | Select bodies, Select faces, Select edges, Select vertices (toggles) | model, print |
| `view.lock`, `view.unlock` | Lock selection, Unlock selection | model, print |
| `sketch.rectangle_center`, `sketch.arc_3point`, `sketch.point` | Center rectangle, Arc, Point | sketch |
| `sketch.constrain_<type>` for the constraints without a key | Point on object, Parallel, Perpendicular, Tangent, Collinear, Concentric, Equal, Symmetric, Midpoint, Fix | sketch |
| `sketch.show_constraints` | Show constraints (toggle) | sketch |
| `print.printer`, `print.overhangs`, `print.checks`, `print.check_now`, `print.lay_on_face` | Choose printer, Overhang map (toggle), Checks, Check print, Lay on face (toggle, 3.17) | print (`print.check_now` all) |
| `print.export_3mf`, `print.export_stl`, `print.export_obj`, `print.export_step` | Export 3MF, Export STL, Export OBJ, Export STEP | all |
| `document.units.mm`, `document.units.cm`, `document.units.m`, `document.units.in` | Units: mm, Units: cm, Units: m, Units: in | all |
| `settings.theme.system`, `settings.theme.light`, `settings.theme.dark` | Theme: System, Theme: Light, Theme: Dark | all |
| `settings.theme.toggle` | Toggle dark theme (`checked` while the resolved theme is dark, 2.16.8) | all |
| `settings.navigation.fusion`, `settings.navigation.freecad` | Navigation: Fusion, Navigation: FreeCAD | all |
| `settings.scale.100`, `.125`, `.150`, `.200`, `.300` | UI scale: 100%, 125%, 150%, 200%, 300% | all |

`edit.repeat` runs `startFeatureCommand` for the type of the last feature command run in this session from any source, with the current selection preselected; before any it is disabled with `nothing_to_repeat`. It is reached from the viewport context menu (3.13) and with Enter on the focused viewport (Enter ladder, below).

`FEATURE_SHORTCUTS` holds the four feature entries above; other commands have no shortcut. Letters follow Fusion where Fusion has a default (E, H, F, M, L, R, C, D, X, S); `Mod+N` is desktop only because browsers reserve Ctrl+N. Unit test U-22 asserts `conflicts()` is empty for the built-in registry with the test catalog on every platform. Letters stay shortcuts: typed entry on the viewport routes only characters that start a number (3.7, 3.16.4), so S, V and the feature letters keep working during commands and while a shape is rubber-banding; once a value input has focus, every key types into it.

Keys handled by components, not the registry:

| Key | Where | Effect |
| --- | --- | --- |
| Esc | anywhere | The Esc ladder below |
| Enter | inspector fields, inline editors, line tool, focused viewport | The Enter ladder below |
| Arrow keys | focused viewport | Orbit 15 degrees per press (Left and Right about world Z, Up and Down about the camera right axis) |
| Shift+arrow keys | focused viewport | Pan by 10 percent of the viewport height |
| `=` or `+`, `-` | focused viewport | Zoom in or out by 1.25 at the viewport center; `-` types into a value instead while a feature command is active (3.7) or a shape is rubber-banding (3.16.4) |
| Digit, `.`, `-`, `(` | focused viewport or selection field box during a feature command | Focus the command's value field and type the character (3.7) |
| Tab | focused viewport during a feature command | Focus the command's value field with its text selected (3.7) |
| Digit, `.`, `-`, `(` | sketch mode while a shape is rubber-banding | Focus the first draw input and type the character (3.16.4) |
| Arrow keys, Home, End | toolbars, timeline, tree, menus, segmented controls | Move focus (roving tab index) |
| Shift+F10, ContextMenu | timeline item, browser row, focused viewport | Open the context menu (the viewport's: 3.13) |
| Space | timeline item, browser row | Select (with Mod: toggle) |
| Tab | sketch draw inputs | Cycle between the inputs |
| Hold Ctrl (Windows, Linux) or Cmd (macOS) | sketch drawing | Suppress inference (3.16.5) |
| Shift or Mod with click | viewport, timeline, browser | Toggle the item in the selection |

Esc ladder: each press does the first step that applies: (1) close the innermost open menu, popover, palette, name completion list, inline rename or inline editor; (2) in sketch mode, cancel the in-progress drawing, drag or box selection (the tool stays active); (3) exit the active sketch tool or constraint pick mode; (4) cancel the active feature command; (5) cancel the plane pick of a new sketch (back to Model) or the face pick of Lay on face (3.17); (6) clear the selection. Esc never leaves sketch mode once a sketch is being edited; when an Esc press in sketch mode finds no step to do, the sketch hint shows `sketch_leave` for 3 s, so the way out is named.

Enter ladder: (1) in an open name completion list with an option chosen by the arrow keys, insert it (2.13); otherwise commit the focused inline editor (rename, dimension editor, draw inputs); (2) in the line tool with a chain in progress and no draw input focused, end the chain; (3) when focus is in the inspector (not on a button) or on the viewport and a feature command is active, confirm it if it is complete; (4) on the focused viewport in Model or Print with no feature command active and a non-empty app selection, run `edit.repeat` (a held key's repeated `keydown` events, `event.repeat`, are ignored, so holding Enter never chains commands). Enter on a focused button activates that button only.

Keyboard and mouse reference: `app.keys` (F1; the browser's own F1 is prevented) opens the side panel `keys` (`data-testid="keys-panel"`, title "Keyboard and mouse", close button), rendered from data, never hand-written prose, so it cannot drift from the behavior:

- "Mouse": the `rows` of the current preset from `NAVIGATION_PRESETS` (2.11) as label and gesture rows, under a segmented control "Fusion | FreeCAD" that runs the navigation setting commands.
- "Keys in the view and panels": `COMPONENT_KEYS` (`src/commands/component-keys.ts`), the component-key table above as data (key, where, effect), which the components read too.
- "Typing values": from the same data, the expression input keys of 2.13 (ArrowUp and ArrowDown step a plain number, Shift ten times as far, Alt a tenth; Tab completes a parameter name), the draw input keys of 3.16.4 and the typed-value keys of 3.7, with one line saying that any size or angle field takes a number, a value with a unit such as `1 in`, or a formula with parameter names.
- A last line: "Every command and its shortcut: Search ({formatShortcut('Mod+K')})."

Nothing appears until the user asks for it; the palette lists every registry command and shortcut, and this panel covers what the palette cannot: keys components handle themselves and the mouse.

### 3.6 Selection, hover and picking

Pick classes and priority (`choosePick`): `point` (vertex, sketch point, origin point) candidates within the radius win over `curve` (edge, sketch curve, origin axis) candidates within the radius, which win over `area` (face, sketch region, origin plane, datum plane) candidates at distance 0. Within a class the smallest distance wins; at equal distance, items of the active sketch and model elements win over origin items, then the smallest pick id wins. The radius is 4 CSS pixels times the UI scale. In the ID pass, edges and sketch curves are drawn 7 CSS pixels wide, vertices as 9-pixel squares and sketch points as 11-pixel squares (times the UI scale), all depth-tested; faces use a polygon offset so coplanar edges win.

Hover:

- On `pointermove` with no button pressed and no camera gesture, at most once per animation frame, the controller calls `backend.pick` and `choosePick` with the accepted kinds, sets the hover target, and updates highlights. The viewport element mirrors it in `data-hover-kind` (`none` or a `SelectionItemKind`) and `data-hover-key`.
- Mesh hover keys are `mesh:{meshId}:{face|edge|vertex}:{index}` and need no core call. The cursor is `pointer` over a pickable target.
- Hovered faces are tinted with `highlight` (2.16.4) at 20 percent, hovered edges drawn 2.5 CSS pixels wide in `highlight` at 70 percent, hovered vertices as 7-pixel `highlight` dots, hovered sketch items in `highlight`.

Accepted kinds:

- Model and Print contexts, no command: the selection filter (viewport control) has Bodies, Faces, Edges and Vertices; default Faces, Edges and Vertices on and Bodies off. With Faces off and Bodies on, a face candidate becomes its body. Sketch curves, points and regions of visible sketches, visible datum planes and visible origin items are always accepted. Locked bodies are not drawn into the ID pass.
- With a feature command: only what the active selection field accepts (2.12 table), plus flat faces (`surface` `plane`) of visible, unlocked bodies for a `sketch_point` field (placing holes on a face) and faces for a `multiple` `edge` field (all of their edges), both 3.7; with no active selection field, nothing (camera only).
- Plane picking of a new sketch: as 3.16.1.
- Lay on face in Print (3.17): flat faces of visible part bodies only.
- Sketch mode: sketch items of the active sketch (curves, points, constraint glyphs and dimension labels through the DOM) and the origin point and axes.

Click (pointer down and up within 4 CSS pixels and 400 ms, primary button):

- Mesh picks call `pickReference(documentHandle, { mesh_id, kind, index })` to get the `ElementDescriptor`; the resulting item has key `element:{body}:{name}`. A `stale_pick` or `preview_not_pickable` error ignores the click silently.
- No modifier: the selection becomes that item, or empty when nothing was hit. Shift or Mod: the item is toggled in the selection.
- During a feature command, clicks go to `capturePick` instead (3.7), with the click's `PointerRay` (2.11).

Double-click:

- Face: edits `element.feature` (protocol.md `ElementDescriptor.feature`) when it names a feature in the document; otherwise nothing.
- Sketch curve, point or region in Model context: `enterSketch` for its sketch. Datum plane: `editFeature`. Anything else: nothing.

Selection keys: faces, edges and vertices `element:{body id}:{element name}`; bodies `body:{id}`; features (timeline, sketches, datums in the browser) `feature:{id}`; datum planes picked in the viewport `feature:{id}` with kind `datum_plane`; sketch curves `sketch:{sketch id}:{entity id}`; sketch points `point:{sketch id}:{point ref}`; regions `region:{sketch id}:{RegionInfo.reference.key}`; constraints `constraint:{sketch id}:{constraint id}`; origin items `origin:XY`, `origin:XZ`, `origin:YZ`, `origin:X`, `origin:Y`, `origin:Z`, `origin:O`; parameters `parameter:{id}`.

Selection after rebuilds: when model-layer meshes change, the UI calls `resolveReferences` with `{body, reference}` of each selected face, edge and vertex (from its `ElementDescriptor`). Resolved items keep their selection and are highlighted at the matched display ids; missing or ambiguous items leave the selection silently. Features, bodies and sketch items leave the selection when they no longer exist in the recipe or solution.

Selected faces are tinted with `highlight` at 35 percent; selected edges drawn 2.5 CSS pixels wide in `highlight`; selected vertices as 7-pixel `highlight` dots. The viewport element mirrors the selection size in `data-selection-count`. A polite live region announces "{label} selected", "{n} items selected" or "Selection cleared", debounced by 500 ms.

Feature highlighting: while the app selection holds features (timeline items, sketch and datum rows) and no feature command is active, the faces those features created are drawn in the selected state. The scene gets them from the core: for every visible body with a model-layer mesh it calls `listElements(documentHandle, body, 'face')` through a `LatestOnly` outside the mutation queue (the command changes nothing), and marks each face whose `ElementDescriptor.feature` is a selected feature id; the face's display id is its position in the returned list (protocol.md 3.4: display ids are positions in `list_elements` order). Lists are cached per body and model-layer mesh id. A list whose body got a new model-layer mesh after the request was sent is discarded and requested again; a new mesh drops that body's cached list. Features whose output has no faces at the display state (sketches, datum planes, suppressed and rolled-back features) highlight nothing.

### 3.7 Feature commands, inspector and live preview

Starting a new feature (`startFeatureCommand(type)`):

1. Cancel an active command; finish sketch mode (remembering the finished sketch); switch to Model.
2. `beginTransaction(documentHandle, manifest.label)`.
3. Generate the feature id (`f-`), set `params = defaultParams(feature, context)` where `lastTargetBody` is the `body` param of the latest active feature in timeline order that has a string `body` param naming a body with model-layer geometry, `bodiesWithGeometry` comes from the model-layer meshes, `context.document` is the recipe and `context.featureStatus` the latest statuses.
4. Preselection: walk the selection fields in manifest order; each takes the app selection items it accepts (in selection order; one item when not `multiple`) that no earlier field took. A selected sketch feature (browser row or timeline item) counts as its region for a `sketch_region` field when its solution has exactly one, and as the points of the 3.3 rule for a `sketch_point` field; this is a preselection rule only, and clicks during the command follow the 2.12 table. The finished-sketch rule of 3.3 applies when the app selection is empty. The app selection is cleared.
5. The active field is the first selection field that is required and empty, skipping a `body` field whose parent object also has a `face` or `edge` field (the first pick fills it, 2.12); if there is none, the first essential selection field whose control is `multiple` (for example Profile, Edges, Faces to remove), so further clicks keep adding to it; if there is none, no field. Unless the active field is a required empty field, focus goes to the first essential expression field with its text selected.
6. The inspector opens (placement below). If the params are complete, the first preview is applied at once.

Editing an existing feature (`editFeature(id)`): a `sketch` feature enters sketch mode instead. Otherwise: cancel an active command, `beginTransaction(documentHandle, 'Edit {label}')`, `setFocus(documentHandle, id)`, take `params` from the record, mark the command `applied`, and open the inspector titled with the feature label. Selection fields show `itemsFromValue` and their references are highlighted through `resolveReferences`, each reference paired with its sibling `body` param (features.md 2.5 shapes), on the model layer (which shows the state before the feature). If `setFocus` fails with `focus_inactive`, the transaction is canceled and the core's message is shown as an error notification with the action "Unsuppress" (suppressed feature: unsuppresses it and retries the edit) or "Move marker to the end" (rolled-back feature: `set_rollback` with no id, so nothing is rolled back, then retries). Each `Repair` of the feature's status with a `centroid` gets a dashed ring of 16 CSS pixels diameter in `errorMark` at that point (`data-testid="repair-marker"`, an SVG `circle` in the overlay with the static attribute `stroke-dasharray="4 3"`, 3.12) with the SVG `title` "Where the missing {noun} was" (face, edge or vertex, from the field's selection kind), until the command ends, so the user sees where to pick again.

Preview cycle: every field change recomputes local field errors (expression states, selection minimums). When the params are complete (every required field set, no field error, no expression pending), the UI submits a preview through a `LatestOnly` over `applyEdits(documentHandle, null, edits)`:

- First application: `add_body` at index `bodies.length` (when the command creates a body, for op `new` with an empty `body` field or for a body-creating `target` set to a new body (2.12): a new body with a `b-` id and `nextBodyName`, and that field is set to it), then the helper sketch's `add_feature` when there is one (placing holes on a face, below), then `add_feature` with the record `{ id, type, ...params }` (no name) at index `features.length` when `rollback` is null, else at the rollback feature's index (the new feature is active; no `set_rollback`); then `setFocus(documentHandle, id)`.
- Later applications: `update_feature` with the full record. Switching op away from `new` removes the body this command created (`remove_body`) and sets `body` to `lastTargetBody`, or clears a body-creating `target`; switching back to `new` adds a fresh body. Clearing a body-creating `target` removes its created body (for Split body this discards the other part); choosing "New body" from its Choose menu creates one again.
- The response's events update the store: `recomputed` gives the feature's status; `body_mesh` events with layer `preview` carry the preview geometry, shown as normal shaded bodies; the model layer shows the state before the feature and stays pickable.
- Incomplete or invalid params after an application hide the preview layer (nothing applied); the next complete state applies again.
- `ps:preview` measures from the change to the drawn preview.

Cut direction: in an Extrude command, when Operation changes to Cut or Intersect while the profile's sketch has `plane.kind` `face` and `plane.body` equal to the `body` param, Reverse becomes true; when Operation changes back to Join or New body under the same condition, Reverse becomes false. Once the user has changed Reverse in this command (`changedPaths` holds `reverse`), Operation changes leave it alone. A sketch on a face has its normal pointing out of the body, so a cut sketched on a face goes into the material by default instead of doing nothing.

The inspector shows the focused feature's status messages in a banner (`data-testid="inspector-error"`): errors with `role="alert"` on `errorSubtle` with an `errorMark` icon and `errorText` messages, warnings (when there is no error) on `warningSubtle` with the `warningMark` hazard icon and `text` messages. Each message is followed by the fix actions in 4.2. A message whose `field` maps to a shown field is also shown as that field's `field-error`. A message with a `suggestion` adds the action "Use {suggestion}", which calls `setCommandField(path, suggestion)` with the path of the field whose pointer equals `field` (suggestions are only made for such scalar fields). Mapping rule: a hint path `a.b[1]` is the pointer `/a/b/1`; the field is the hint whose pointer equals `field`, else the hint whose pointer is the longest whole-segment prefix of it (so `/edges/1` maps to `edges`). The same rule finds the field for `Repair.path`.

Confirming (Enter ladder, or the OK button): allowed only when complete. `flush()` of the preview `LatestOnly` is awaited first, so the latest params are applied. Then `commitTransaction(documentHandle)`, the inspector closes, the command clears and focus returns to where it was before the command. A sketch referenced by the committed params through a `sketch` key is hidden for the session (3.10). If the committed feature's status is `error`, the failure notification of 4.2 appears.

Canceling (Esc ladder, the Cancel button or the close button): `cancelTransaction(documentHandle)`, the inspector closes, nothing is recorded.

Other edits during a command: while a feature command's transaction is open,

- Parameter changes from the Parameters panel (add, expression, comment, delete) are sent through `applyEdits` with no label, so they join the transaction (a rename already does, protocol.md command 31), and the preview follows at once: while such an edit is in flight `data-preview` is `pending`, and it returns to `shown` (or `failed`) when the edit's events arrive. OK records them with the feature as one undo entry and Cancel reverts them. They are not command history entries: `Mod+Z` with an empty history cancels the command and reverts them with it, and Restore (below) does not bring them back. The Parameters panel shows under its title, in `textMuted`: "Changes are part of {title} until you press OK." (`data-testid="parameters-transaction"`).
- Session view state stays available: sketch, datum and origin eyes, body locks, the section view, the camera, the selection filter, the grid and every panel.
- Every other document edit is disabled with `finish_command_first`: Save and Save as, body visibility, color, material, rename and delete, timeline rename, suppress, unsuppress, delete, reorder and the rollback marker, units and printer. Their controls are `aria-disabled` with the reason as tooltip; the timeline drag and the marker do not start.
- New, Open and Open recent cancel the command first.

Command history: each change the user makes in a command (a pick added or removed, a field cleared, a choice, switch or toggle changed, the first text change of an expression field after it gains focus, a "Use" or fix action, a "New body" choice) pushes a `CommandStep` (the params, helper sketch and active field before it, with the changed field's label) onto `history` and clears `future`; its path joins `changedPaths`. Preselection and starting values are not changes, and the rules a change triggers (the sibling body of 2.12, the cut direction above) belong to that change. With focus outside a text field, `Mod+Z` pops `history`, restores that state (including the helper sketch of placing holes on a face) and previews again; `Mod+Shift+Z` pops `future` likewise. With an empty `history`, `Mod+Z` cancels the command and calls no core `undo`. In a text field, `Mod+Z` stays the field's own text undo (2.10 dispatch rule 2). So a reflex `Mod+Z` after a wrong pick undoes that pick, not every pick.

Implicit cancel: when a command whose `history` is not empty ends without OK or Cancel because the user did something else (another feature command starts, another feature is edited, a sketch is entered, or the context changes), it is canceled as before and the info notification `command_canceled` offers "Restore". Restore cancels whatever command is then active, explicitly (that cancel raises no notification of its own), and reopens this one (`startFeatureCommand` without preselection, or `editFeature` of the same feature) with the saved params, helper sketch and active field. New, Open, Open recent, the recovery Open and a core restart cancel without this notification, because Restore would reopen the command against another document. A command whose `history` is empty ends silently, as before. Committing instead of canceling is deliberately not done: a newcomer who clicked Extrude and then Revolve would get a feature they did not want and a hidden sketch.

Inspector panel (`role="dialog"`, `aria-modal="false"`, `aria-labelledby` its title, `data-testid="inspector"`, `data-mode="command"`, `data-preview` mirroring `ActiveCommand.preview`: `none` before anything is applied or while the params are incomplete, `pending` while a preview request is in flight, `shown` when the preview layer for the latest params is drawn, `failed` when the focused feature's status is `error`), `w-80`, at most the viewport height minus 2rem (`max-h-[calc(100%-2rem)]`, the inspector's one arbitrary value: its height depends on the viewport, and it is absolutely positioned at a runtime translate), `overflow-y-auto`, a `surfaceRaised` panel with a `border` hairline, `rounded-md` and `shadow-lg`:

- Header: feature icon, title (manifest label for a new feature, feature label when editing; the title has the command's description, 2.10, as its tooltip), close button "Cancel". The header is the drag handle.
- Status banner as above.
- Essential fields, then a disclosure button (`data-testid="inspector-more"`, `aria-expanded`) when there are more fields: "Fewer options" while open; while closed "More options", followed by " · " and the `changedMore` labels joined with ", " when there are any ("More options · Tip angle"), so a value changed out of sight never surprises anyone. Its tooltip lists every field in More. Its state is remembered per feature type in `settings.moreExpanded`; `forceMore` opens it, so More opens by itself only when a field in it needs attention (an error or a required empty value).
- Footer: "Cancel" and "OK" (`data-testid="inspector-ok"`, primary: `accent` fill); Cancel's tooltip shows "Cancel" with "Esc", OK's "OK" with "Enter" when complete. OK is `aria-disabled` while incomplete, with the tooltip "Fill in the highlighted fields first."; required empty selection fields get a dashed `accent` border and invalid fields an `errorMark` border.
- Each field is a container with `data-testid="inspector-field"` and `data-path`.

Field controls:

- Labels: a field whose `help` (2.12) is not null shows it in a tooltip when the pointer rests on its label (the label has `cursor-help`, the only cue, so the layout gains nothing), and the help is also a visually hidden description (`sr-only`) referenced by `aria-describedby` from the control. Labels without help have no tooltip.
- Expression: `ExpressionInput` (2.13) labeled with the hint label. To its right, `data-testid="expression-result"` (in `FONT_CLASSES.values`, `textMuted`) shows the unit symbol when `isPlainNumber(text)` holds and the text has no unit word (`mm`, `°`), otherwise `= {formatQuantity(value)}` (`= 10 mm`), or nothing while empty or pending. Below it, `data-testid="field-error"` (`aria-live="polite"`, referenced by `aria-describedby`) shows the message of an invalid state. ArrowUp and ArrowDown step plain numbers (`stepExpression`; Shift coarse, Alt fine) and do nothing otherwise. In the command and selection inspectors, when the field is invalid with a non-null `range` whose text is an identifier that names no parameter and does not start with `printer.`, `field-error` is followed by a button "Add parameter {name}". It opens the Parameters panel with a draft row named {name} whose expression is `formatQuantity` of the field's last valid value (empty when it never had one), with the expression input focused. Committing the draft adds the parameter (joining the open transaction during a command, above) and returns focus to the field, which re-evaluates at the new revision; Esc discards the draft and returns focus. So a parameter is created from the field that needs it.
- Selection: a box (`data-testid="selection-field"`, `role="button"`, `data-active`, `data-count`) showing the placeholder `Select {noun}` (`Select edges or a face` for a `multiple` `edge` field, `Select a plane`, `Select regions`, `Select a body`, `Select bodies`, `Select a sketch`, `Click a flat face or pick sketch points` for `sketch_point`, `Select faces`, `Select an axis`), one item's label, or `{n} {plural noun}` (`3 edges`). A field set to the command's created body reads "{name} (new)". Clicking it makes it the active field (`accent` border instead of `borderControl`); a clear button "Clear {label}" empties it. For `body`, `sketch`, `plane` and `axis` fields and for `sketch_region` fields, a "Choose" menu button lists the enumerable candidates (bodies with geometry; sketches; origin planes and datum planes; origin axes; regions of the current profile's sketch, or of every visible sketch when empty, as "Region {n} ({area})") so these fields work without a pointer. For a body-creating `target` (2.12) the Choose menu lists "New body" first, then the bodies without model-layer geometry (a target must be empty). Hovering an item's entry in the box highlights it in the viewport.
- Choice: a segmented control (`role="radiogroup"`) for up to 4 options, else a native `<select>`, with the hint's option labels.
- Switch: `role="switch"`. Toggle: an icon button (`arrow-up-down`, `size-8`, accessible name the field label, `aria-pressed`, the field's help as its tooltip) at the end of the row of the field it attaches to (`attachedTo`, 2.12), so Reverse sits beside the distance it reverses; it keeps its own inline `inspector-field` container with its `data-path`. Text and number: text inputs; number fields reject non-numeric text with "{label} must be a number."
- Unsupported: read-only compact JSON with the help "Edit this value in the .ptsm file."

Selection capture (`capturePick(item, pointer)`): when the active field accepts the item (`acceptsTarget`; geometry checks such as a flat face use the item's `ElementDescriptor`), the item is toggled in the field: added (replacing the single value when not `multiple`) or removed when already present. A refused item shows a hint under the field (`pick_plane` for `plane`, `pick_axis` for `axis`, `pick_points` for `sketch_point`, `pick_kind` otherwise; 4.3) for 3 s. When a non-multiple field receives its item, or a `multiple` field receives its first item, the active field advances to the next required empty selection field, if any. Clicking a field's box makes it the active field again. Focusing an expression field does not change the active field, which keeps its `accent` border, so clicks on the model still go to it while the user types. The sibling `body` rule of 2.12 applies.

Faces for edges: while the active field accepts `edge` and is `multiple` (Fillet, Chamfer), faces are accepted too: clicking a face adds every edge named in its `ElementDescriptor.adjacent` that the field does not hold, in `adjacent` order, or removes them all when it holds every one. Their references come from the `listElements(documentHandle, body, 'edge')` descriptors with those names, cached per body and model-layer mesh id as in 3.6, so the UI still never builds a reference. Hovering a face in this state highlights its edges instead of the face. The sibling body rule (2.12) applies.

Placing holes on a face: while the active field accepts `sketch_point`, a click on a flat face (`surface` `plane`) of a visible, unlocked body places a hole center, so Hole works from a face as well as from sketch points, with a real sketch behind it:

- The center is where the click's `PointerRay` meets the face's plane (`ElementDescriptor.placement`), in that placement's sketch coordinates (sketch.md); when the face's `centroid` is within 8 times `millimetersPerPixel` of that point, the center is the centroid.
- The first such click gives the command a helper sketch (`ActiveCommand.helper`): a `sketch` record with a new `f-` id, `plane` `{"kind": "face", "body": <the face's body>, "face": <its Reference>, "fallback": <its placement>}` exactly as `createSketchOn` builds it (3.16.1), one point entity `{"id": <new e- id>, "type": "point", "position": [x, y]}` and no constraints. Points becomes that entity's `PointRef`, the sibling `sketch` field the helper's id and the `body` field the face's body; points of another sketch that Points held are replaced (the 2.12 rule for points of another sketch).
- Further clicks on the same face add a point entity to the helper and its `PointRef` to Points. The helper's points are drawn from the command state like sketch points as soon as they exist, before any preview, and are pickable, so clicking one removes it (`capturePick`'s toggle). A click on another face is refused with `hole_one_face`.
- Every preview application carries the helper: the first one sends `add_feature` of the helper at the index the marker rule gives the hole, followed by the hole's `add_feature` right after it (when editing, the helper goes at the hole's index, before it); later ones send `update_feature` of the helper. When Points no longer holds a point of the helper, the next application removes it (`remove_feature`) and `helper` becomes null.
- Confirming commits the helper and the hole in the one transaction (one undo entry, the command's label); Cancel discards both. The committed helper is hidden like any sketch the committed params reference (Confirming, above); opening it later (double-click its browser row) lets the user dimension the centers exactly.

Typing values: the command's value field is the expression field the user changed last in this command (the last expression path in `changedPaths`), else the first essential expression field. While a feature command is active and focus is on the viewport or on a selection field box, a digit, `.`, `-` or `(` pressed without Mod, Ctrl or Alt focuses the value field, replaces its text with that character and leaves the caret after it; Tab on the focused viewport focuses the value field with its text selected. The Enter ladder (3.5) then confirms, so pick, type `2`, Enter makes a 2 mm fillet. Letters keep their shortcuts.

Placement: when the inspector opens, it is placed with `placeInspector` where `anchor` is the screen rectangle of the preselected items (or null) and `bounds` the viewport rectangle; it then stays put while open. Dragging the header moves it within the viewport, and the dragged position (`panels.inspectorPosition`) is used for every inspector until reload. The position is applied with `positionElement` (3.12), never through a `style` prop.

### 3.8 Selection inspector

With no feature command active and a non-empty app selection (Model and Print contexts), or a non-empty sketch selection with no tool active (sketch mode), the inspector shows the selection (`data-mode="selection"`, accessible name "Selection"). It uses the same panel, placement and dragging rules, and closes when the selection empties.

| Selection | Title (`data-testid="selection-title"`) | Rows and actions |
| --- | --- | --- |
| One body | Body name | Size (`data-testid="body-size"`, `formatSize` of the model-layer bounding box, or "Empty"), Center (`data-testid="body-center"`, `formatPoint` of the box center), Color (swatch button), Material (combobox with suggestions PLA, PETG, ABS, ASA, TPU, Resin; Enter commits "Change material"; empty clears), Visible (switch). Suggested commands: Move body, Mirror, Split body |
| One feature, sketch or datum | Feature label | Type (manifest label), status messages with fix actions (4.2), buttons "Edit" and "Suppress" or "Unsuppress". For a sketch, suggested commands: Extrude and Revolve when its solution has regions, Hole when it has non-construction points |
| Faces, edges and vertices | "1 face", "3 edges", "2 vertices", or "{n} items" when kinds mix | Body name(s), each a button "Select {name}" that replaces the selection with that body; for one face, edge or vertex whose `element.feature` names a feature in the document, a row "Made by {label}" with a button "Edit {label}" (`editFeature`; for a face its tooltip adds "Or double-click the face."), and hovering the row gives that feature's timeline item the `hover` role's `active` background; for one face its surface ("Flat face", "Cylindrical face", "Conical face", "Spherical face", "Toroidal face", "Freeform face", "Face") and area (`formatNumber` in the document unit followed by `{unit}²`); for one edge its curve ("Straight edge", "Circular edge", ...) and length; for one vertex its position. Suggested commands: edges: Fillet, Chamfer; one flat face: Sketch, Hole, Shell; other faces: Shell |
| Regions | "1 region", "{n} regions" | Suggested commands: Extrude, Revolve |
| Sketch points (Model and Print contexts) | "1 point", "{n} points" | Suggested command: Hole |
| Two or more bodies | "{n} bodies" | Suggested command: Boolean |
| One origin plane or datum plane | "Top plane (XY)", "Front plane (XZ)", "Right plane (YZ)" or the datum label | Suggested commands: Sketch, Datum plane |
| One origin axis or the origin point | "X axis", "Y axis", "Z axis" or "Origin" | none |
| One body and one origin or datum plane | "2 items" | Suggested commands: Mirror, Split body |
| Anything else mixed | "{n} items" | none |
| Sketch entities and points | "1 line", "2 lines", "1 circle", "1 arc", "1 point", "{n} items" | Constraint suggestions (`applicableConstraints`, each a button with glyph, title and shortcut), Construction (switch, for entities), Delete. For one line, circle or arc, a size field (below) |
| One sketch constraint | Constraint title ("Horizontal", "Distance") | For dimensions: an expression field with the stored expression (Enter commits "Change dimension") and a Reference switch; Delete |

Suggested commands (`suggestedCommands` in `src/inspector/suggestions.ts`, the rows above) are buttons with the command's icon, title and formatted shortcut (the shortcut `aria-hidden` and given through `aria-keyshortcuts`, as for the constraint suggestions). They run the command with the selection preselected (3.7 step 4). A suggestion whose command is disabled (2.10) is left out. So commands that are in the palette only (Datum plane, Move body, Split body) are offered where they apply, with no toolbar growth.

Size field: for one line, circle or arc in the sketch selection, a field (`data-testid="entity-size"`, `ExpressionInput` with unit length) labeled "Length", "Diameter" or "Radius". Its text is the expression of the driving dimension already on the entity (a `distance` on the line or on its two end points; a `diameter` or `radius` on the circle or arc) when there is one, else `formatNumber` of the solved size. Enter with a changed valid text commits through `commitSketchChange`: with a driving dimension, its replacement labeled "Change dimension"; otherwise a new driving dimension (`dimensionFor`: `distance` for a line, `diameter` for a circle, `radius` for an arc) at `defaultLabelPosition`, labeled "Dimension", which then shows on the canvas. When the new dimension would over-constrain the sketch, the field shows `dimension_over_constrained` with "Add reference" and "Cancel" (3.16.7). So a shape's size can be typed after selecting it, with expressions and parameters as everywhere else.

Measurements: selecting something measures it, with no tool and no mode. Rows (`selectionMeasurements` in `src/inspector/measurements.ts`; `data-testid="measure-row"`, `data-measure`) show a label and a value in `FONT_CLASSES.values` (selectable text, `formatQuantity`):

| Selection | Rows (`data-measure`) |
| --- | --- |
| One circular edge or cylindrical face | Diameter (`diameter`): 2 × the descriptor's `radius` |
| One spherical face | Diameter (`diameter`): 2 × the descriptor's `radius` |
| One body with geometry (Size is not "Empty") | Volume (`volume`): `measure` with `{kind: 'body', body}` through a `LatestOnly` keyed by the body's model-layer mesh id; the volume divided by the cube of millimeters per document unit, rounded as a length (2.13), followed by `{unit}³`; empty while pending |

Distances and angles between two selected items need the core's two-target `measure`, which is later (1.2).

### 3.9 Timeline

Layout, left to right: collapse button ("Collapse timeline" / "Expand timeline"); the items in timeline order with the rollback marker in the gap before the rollback feature (the first rolled-back feature; at the end when nothing is rolled back); at the right end, the problems button (`data-testid="timeline-problems"`, text "1 problem" or "{n} problems", hidden when there are none). The item strip scrolls horizontally; a newly added item and the marker scroll into view. With no features, the item strip shows `text-xs` text in `textMuted` (`data-testid="timeline-empty"`): "Each step you add appears here. Double-click a step to change it later."

Items (`data-testid="timeline-item"`, `data-feature-id`, `data-status` from the latest `recomputed` status (`ok`, `warning`, `error`, `suppressed`, `rolled_back`), or `pending` before the first status for a new feature, `data-suppressed`, `data-rolled-back`):

- A `size-10` button with the feature icon (`size-5`). Its accessible name is the feature label; `aria-describedby` points at a hidden text with the status ("Error: {first message}", "Warning: {first message}", "Suppressed", "Rolled back").
- A status badge at the bottom-right: the `triangle-alert` hazard sign (`warningMark`) for warnings, `circle-x` in `errorMark` for errors. The tooltip shows the label and every message.
- Suppressed items draw their icon at `opacity-40` with a diagonal strike (an absolutely positioned bar `inset-x-1 top-1/2 h-0.5 -rotate-45 bg-current` in `textMuted`). Items after the marker draw at `opacity-40`. While a feature is being edited, it gets an `accent` outline (`outline-2`) and items after it draw at `opacity-40`; the sketch being edited gets an `accent` outline.
- Selected items have the `selection` background.

Interactions:

- Click selects the feature (Shift or Mod toggles), which highlights the faces it created (3.6). Double-click or Enter edits it (`editFeature`). F2 renames inline: a text field over the item; Enter commits `update_feature` with the new `name` (label "Rename"); an empty name removes `name` (the default label returns); Esc cancels.
- Context menu (right-click, Shift+F10, ContextMenu key; `role="menu"`, `data-testid="context-menu"`): Edit, Rename, Suppress or Unsuppress, Roll back to here, Delete. "Roll back to here" moves the marker to the gap after the item, so the item stays active (`setRollback` with the item's 1-based position).
- Suppress: `update_feature` with `suppressed: true` (label "Suppress {label}"); unsuppress likewise ("Unsuppress {label}"). Several selected features change in one edit.
- Delete (Delete key or menu): one `apply_edits` labeled "Delete {label}" or "Delete {n} features" with `remove_feature` edits; when the rollback feature is among them, a `set_rollback` to the nearest later feature that is not deleted (no id when none) comes first. An info notification "Deleted {label}." offers "Undo".
- Reorder: pressing an item and moving more than 4 CSS pixels horizontally starts a drag: a ghost follows the pointer and an insertion bar (`w-0.5`, `accent` background) marks the nearest gap; both are moved with `positionElement` (3.12). Dropping at gap g (0 to N) moves the feature to index g when g is less than its current index, else g minus 1; the same position does nothing. The edit is `move_feature` labeled "Reorder". Esc cancels the drag. The core does not check dependencies when moving (model.md 3.9); a feature that now comes before something it uses fails and triggers the failure notification of 4.2, whose "Undo" restores the order.
- Rollback marker (`data-testid="timeline-marker"`): a `w-2` handle with a `w-0.5` `accent` line, `role="slider"`, `aria-label="Rollback marker"`, `aria-valuemin="0"`, `aria-valuemax` the feature count, `aria-valuenow` the number of active features, `aria-valuetext` "At the start" (p = 0), "After {label}" (the label of feature p) or "At the end" (p = N). Position p (0 to N) means features 1 to p are active; p = N writes `set_rollback` with no id, p < N writes the id of feature p + 1 (the first rolled-back feature).
  - Dragging: `beginTransaction(documentHandle, 'Roll back')`, then on every gap change a `LatestOnly` `apply_edits` with the `set_rollback`, so the model updates live; dropping commits; Esc cancels the transaction. Dropping at the starting gap cancels.
  - Keys: ArrowLeft and ArrowRight move by one gap, Home to position 0, End to the end; each key press applies its own edit labeled "Roll back".
- Collapsed (`h-7`): only the expand button, the text "{n} features" and the problems button remain.
- Keyboard: the strip is a toolbar with one tab stop; Left and Right move focus between items and the marker; Space selects; Mod+Space toggles.

Problems panel (side panel `problems`, `data-testid="problems-panel"`), opened by the problems button or the palette command "Problems": every feature whose status is warning or error, in timeline order, then document diagnostics. Each row shows the status icon, the feature label (a button that selects it), its messages, and an "Edit" button (`editFeature`). Empty: "No problems."

### 3.10 Browser

A tree (`role="tree"`, `aria-label="Browser"`) with these groups (`role="treeitem"` with children; expansion kept for the session):

| Group | Rows (`data-testid="browser-row"`, `data-kind`, `data-id`) | Default |
| --- | --- | --- |
| Origin | Top plane (XY), Front plane (XZ), Right plane (YZ) (kind `origin_plane`); X axis, Y axis, Z axis (kind `origin_axis`); Origin (kind `origin_point`) | collapsed; planes hidden, axes and origin point shown while the document has no features |
| Bodies | One per `Document.bodies`, in order (kind `body`) | expanded |
| Sketches | One per `sketch` feature, in timeline order (kind `sketch`) | expanded |
| Datum planes | One per `datum_plane` feature (kind `datum`) | expanded |
| Parameters | One per parameter, in order (kind `parameter`) | expanded |

Origin planes are named by view and coordinates everywhere (browser rows, selection titles, field boxes, Choose menus and plane labels): XY "Top plane (XY)", XZ "Front plane (XZ)", YZ "Right plane (YZ)" (`originPlaneName`, 2.17), matching the standard views of 2.11, so the print bed's plane is recognizable and people who think in coordinates keep "XY". "Construction" means only sketch construction geometry; the group of datum planes is "Datum planes".

Row content:

- Body: color swatch button ("Color of {name}"), name, "(empty)" in `textMuted` when the body has no model-layer geometry, lock button ("Lock {name}" / "Unlock {name}", `aria-pressed`), eye button ("Hide {name}" / "Show {name}").
- Sketch and datum: icon, feature label, status badge as in the timeline, eye button ("Hide {label}" / "Show {label}").
- Origin item: icon, name ("Top plane (XY)", "X axis", "Origin"), eye button ("Hide {name}" / "Show {name}").
- Parameter: "{name} = {expr}" and the evaluated value in `textMuted` and `FONT_CLASSES.values` when the expression is not a plain number; the group header has an icon button "Edit parameters" (`sliders-horizontal`).

Behavior:

- Rows map to selection items: body rows to kind `body`, sketch rows to `feature`, datum rows to `datum_plane` (key `feature:{id}`), origin rows to `origin_plane`, `origin_axis` or `origin_point`, parameter rows to `parameter`.
- Click selects (Shift or Mod toggles). While a feature command is active, clicking a row whose kind the active field accepts captures it instead (origin planes and datums for `plane`, origin axes for `axis`, bodies for `body`, sketches for `sketch`).
- Double-click or Enter: sketch: `enterSketch`; datum: `editFeature`; body: inline rename; parameter: opens the Parameters panel focused on it.
- F2 renames bodies (`update_body`, label "Rename") and features (as in the timeline). Delete removes selected sketches and datums (`deleteFeatures`) and selected bodies (`remove_body`, label "Delete {name}"; the core does not check use, so dependent features fail and the failure notification offers Undo). V toggles visibility of selected rows.
- Body eye: `update_body` with `visible` flipped (labels "Hide {name}" / "Show {name}"; undoable). Hidden bodies are not drawn or picked.
- Sketch, datum and origin eyes change `view.sessionVisible` only (not saved, not undoable). On opening a document, a sketch starts hidden when another feature's params contain an object whose `sketch` key equals its id (the `ProfileRef`, `AxisRef` and hole shapes of features.md 2.5), and visible otherwise; datum planes start visible; origin planes start hidden. The origin axes and the origin point are shown while the document has no features and hidden once it has one, so an empty view has an anchor without putting a pickable Z axis through most models. A `view.sessionVisible` entry, written only by their eye button or V, always wins (`selectOriginVisible(state, key)`). Origin planes are always shown while a plane is being picked.
- Lock: `view.locked` only (session); locked bodies are drawn but not pickable.
- Color: a popover (`data-testid="color-popover"`) with the swatch buttons of `BODY_SWATCHES` (2.16.5), named Default (removes `color`), White `#f4f4f5`, Black `#27272a`, Gray `#71717b`, Red `#e7000b`, Orange `#ff6900`, Yellow `#fdc700`, Green `#00a63e`, Teal `#009689`, Blue `#155dfc`, Purple `#9810fa`, Pink `#e60076` and Brown `#973c00`, and a hex field accepting `#rrggbb` (in `FONT_CLASSES.values`). Choosing commits `update_body` with the color (label "Change color"). A body's current color (in its browser row and in the selection inspector) can be any hex from the document, so it is drawn as an inline SVG square whose `rect` has the stored hex (or `body`'s hex for no color) as its `fill` attribute, inside a `borderControl` border: document data known only at runtime goes into an SVG attribute, never a `style` (3.12).
- Keyboard: WAI-ARIA tree pattern (Up, Down, Right expands or enters, Left collapses or goes to the parent, Home, End), no type-ahead (letters stay global shortcuts). Row buttons are not tab stops; their actions are V (visibility), the selection inspector (color, material, visibility) and the palette commands "Lock selection" and "Unlock selection".

### 3.11 Parameters panel

The side panel `parameters` (`data-testid="parameters-panel"`, title "Parameters", close button) opens from the browser's "Edit parameters" button, a parameter row, or the palette command "Parameters".

- One row per parameter (`data-testid="parameter-row"`, `data-id`): name field ("Name of {name}"), expression field (`ExpressionInput` with unit `any`, "Expression of {name}"), value (`data-testid="parameter-value"`: the `recomputed` value formatted by its dimension, in `FONT_CLASSES.values`, or the error message in `errorText` with a `circle-x` icon in `errorMark`), comment field ("Comment of {name}"), delete button ("Delete {name}").
- `ExpressionInput` with unit `any` accepts every dimension and shows `= {value}` formatted by the returned dimension: length and angle as in 2.13, dimensionless as a scalar, any other dimension as `{value} mm^l rad^a` in internal units (zero exponents omitted, exponent 1 without `^`).
- Name: Enter or blur calls `renameParameter` when changed; the core rewrites every expression that uses the old name and labels the undo entry "Rename parameter {old} to {new}". Locally, a name that does not match `^[A-Za-z_][A-Za-z0-9_]*$` or is longer than 64 characters shows "Names start with a letter or _ and use only letters, digits and _." and is not sent; other rules (reserved names, uniqueness) come back as the core's error message under the field. Esc reverts.
- Expression: Enter or blur commits `update_parameter` (label "Change {name}") when valid and changed; Esc reverts.
- Comment: Enter or blur commits (label "Change comment of {name}"); empty removes the comment.
- "Add parameter" appends a draft row whose name field is focused; Enter in the name moves to the expression (default text empty); Enter in a valid expression commits `add_parameter` at the end (label "Add parameter {name}"); Esc discards the draft. A field's "Add parameter {name}" button (3.7) opens the same draft with the name and expression filled in.
- With no parameters, the panel shows above "Add parameter", in `textMuted` (`data-testid="parameters-empty"`): "Parameters are named values, such as wall = 2 mm. Type a parameter's name in any size or angle field to use it."
- During a feature command, parameter changes join its transaction and the panel says so (`parameters-transaction`, 3.7).
- Delete commits `remove_parameter` (label "Delete parameter {name}"). Expressions that used it then fail; the failure notification offers Undo.

### 3.12 Viewport rendering

- The canvas fills the viewport region; its drawing buffer is the CSS size times `min(devicePixelRatio, 2)`. Rendering is on demand: a frame is drawn only after a change to the camera, scene data, highlights, theme, size, section or analysis, batched with `requestAnimationFrame`. Nothing renders while idle.
- Shading is computed in linear space and written as sRGB.

Passes, in order:

1. Background: vertical gradient from `viewportTop` to `viewportBottom` (2.16.4).
2. Opaque bodies (model layer, and preview layer during a command): matcap-style shading from the view-space normal `n`: `k = 0.32 + 0.50 × max(dot(n, L), 0) + 0.18 × max(n.z, 0)` with `L = normalize(0.3, 0.6, 0.74)`, color `base × k + 0.06 × (1 - max(n.z, 0))^3` (rim). `base` is `Body.color` or `body`. Element states mix the result toward `highlight` (hover 0.20, selected 0.35) or `problem` (problem 0.5). Faces use a polygon offset (factor 1, units 1). With a section, back faces are drawn flat in `base × 0.55` (the cut cap). Bodies whose role is not `part` draw at 40 percent opacity in the translucent pass.
3. Overhang map (Print context, 3.17): for a vertex value `v` (radians from vertical) above the limit `a`, `t = clamp((v - a) / (π/2 - a), 0, 1)` the ramp color is `ramp = linearize(mix(analysisLow, analysisHigh, t))`, mixing the sRGB-encoded ends (as the legend's `bg-linear-to-r/srgb` gradient does) and linearizing the result, and the color becomes `mix(shaded, ramp × k, 0.85)`.
4. Edges: B-rep edges from `edge_positions` as screen-space quads (instanced per segment), 1.25 CSS pixels wide (times the UI scale), anti-aliased with a 1-pixel smoothstep falloff, depth-tested against the faces, in `edgeDark` or `edgeLight` per body (2.16.4). Selected and hovered edges as in 3.6.
5. Vertices: only hovered and selected vertices are drawn.
6. Translucent: the grid (below); datum and origin planes as squares with `datum` fill at 15 percent and outline at 60 percent; non-part bodies.
7. Overlay (no depth test): sketch curves, points and region fills (3.16.2); in Model context visible sketches draw in `sketchReference` at 1.25 CSS pixels with no region fill, except hovered or selected regions (`highlight` at 20 or 35 percent) and, while the active selection field accepts `sketch_region`, every region of every visible sketch in `sketchRegion` at 12 percent (20 percent when hovered), so the shapes that can be clicked show. The helper sketch points of a Hole command (3.7) draw here too.
8. Print bed outline (depth-tested, 3.17).

- Grid: in Model context, lines on the XY plane (z = 0); in sketch mode, lines on the sketch plane through the placement origin along the placement axes instead; none in Print, which draws the bed outline, and none while the grid is turned off (the palette toggle "Show grid", `ViewState.grid`). The minor spacing is `gridSpacing` (2.11) at the camera target; every tenth line is major. The grid covers a square of side `max(200 mm, 2 × the diagonal of the visible bounds)` centered on the origin (or the placement origin), rounded out to whole major cells. Lines are 1 CSS pixel wide (times the UI scale) in `grid`, minor at 35 percent and major at 70 percent opacity, depth-tested so bodies hide them; they are not drawn into the ID pass and are never snap targets. The viewport mirrors the minor spacing in the document unit (`formatNumber`) in `data-grid-spacing`, empty when no grid is drawn, and the status bar shows it (3.21), so the grid gives scale and not only orientation.

- Datum and origin planes are drawn as squares of side `max(50 mm, 1.2 × the diagonal of the visible bounds)` centered at the projection of the visible bounds center onto the plane. Origin axes are drawn as dashed lines of the same length through the origin in `sketchReference`, with DOM labels "X", "Y", "Z" (`textMuted`) at their positive ends.
- The overlay layer (`absolute inset-0 overflow-hidden pointer-events-none` over the canvas; interactive children `pointer-events-auto`) holds an SVG (`absolute inset-0 size-full`) for the box selection rectangle, profile issue rings, finding markers and repair markers, and HTML elements for sketch dimension labels, constraint glyphs, draw inputs, the dimension editor, inference glyphs, axis labels and plane labels. Their positions follow the runtime positions rule below and update in the same animation frame as the canvas.
- WebGL context loss: `preventDefault` on `webglcontextlost`, a status overlay "Restoring the 3D view" (`role="status"`), and on restore `scene.restore()` re-uploads every drawable and renders.

Runtime positions, the one exception to the no-inline-style rule (its scope: open issue 14):

- Which: the overlay's HTML elements above (plane labels included, 3.16.1); the inspector (3.7); the timeline's drag ghost and insertion bar (3.9); menus, popovers and tooltips anchored to their triggers (the native Popover API places them in the top layer, and Tailwind 4.3.3 has no anchor positioning utilities), and the viewport context menu anchored at the pointer (3.13); and the view cube's rotation (3.14). The overlay's SVG marks use SVG geometry attributes instead (`x`, `y`, `width`, `height`, `cx`, `cy`, `r`), with colors from stroke classes (`accent`, `errorMark`) and the dashes of the crossing selection and the repair marker as the static attribute `stroke-dasharray="4 3"`.
- Why: these values come from projecting 3D geometry through the camera every frame (or from the pointer during a drag, or from a trigger's measured rectangle), so no class known at build time can express them, and re-rendering React every frame would break the rule that the viewport bypasses React (ADR 0002).
- How: React renders each element once with its classes, test ids and accessible names and never passes a `style` prop. The viewport controller, or the owning component through a ref, writes the position imperatively, through the two functions of `src/viewport/overlay.ts`, which are the only code in `src/` that touches `element.style`:

```ts
/** Moves an element to (x, y) CSS pixels from its layer's origin: sets only `element.style.translate` (`${x}px ${y}px`) and marks the element `data-positioned`. */
export function positionElement(element: HTMLElement, x: number, y: number): void
/** Sets only `element.style.transform` to the CSS `matrix3d` of a column-major 4 × 4 matrix and marks the element `data-positioned`. */
export function orientElement(element: HTMLElement, matrix: Float32Array): void
```

- Positioned elements sit at `absolute left-0 top-0` of their layer. The inline `translate` replaces any translate class on the same element, so an element centered on its anchor wraps its content in an inner element with `-translate-x-1/2 -translate-y-1/2`. An element whose anchor is behind the camera or outside the canvas gets the `hidden` attribute instead of a position. SVG geometry attributes are written with `setAttribute` in the same frame.
- Document data drawn in the DOM (a body's current color, 3.10) uses SVG presentation attributes (`fill`), not styles.
- No other element has a `style` attribute: U-91 checks the source and E-61 the rendered app.

### 3.13 Navigation

| Action | Fusion preset (default) | FreeCAD preset |
| --- | --- | --- |
| Select | Left click | Left click |
| Pan | Middle drag | Middle drag |
| Orbit | Shift + middle drag | Middle + left drag, or middle + right drag (press middle first) |
| Orbit without a middle button (also a two-finger press and drag on a macOS trackpad) | Right drag | Right drag |
| Pan without a middle button | Shift + right drag | Shift + right drag |
| Zoom | Wheel at the cursor | Wheel at the cursor |
| Zoom (trackpad pinch: wheel with `ctrlKey`) | At the cursor | At the cursor |
| Fit to view | Middle double-click | none |
| Orbit with the view cube | Left drag on the cube | Left drag on the cube |
| Box selection | Left drag on empty space, sketch mode only | same |

- A gesture starts after 3 CSS pixels of motion; below that, a press and release is a click. Orbit rate 0.4 degrees per CSS pixel about the pivot: the world point under the cursor at the gesture start (from the ID pass depth) or the target when nothing is there or the backend cannot read depth.
- Wheel uses `wheelZoomFactor`; one notch with a pixel delta of 100 zooms out by 1.0015^100 ≈ 1.162 (wheel back) or in by the inverse (wheel forward). Wheel listeners are non-passive and call `preventDefault`.
- Middle-button `pointerdown` and `mousedown` call `preventDefault` (no browser autoscroll). The canvas `contextmenu` event always calls `preventDefault` (on macOS and Linux it fires on press, before a right drag can start). A right-button press that moves 3 CSS pixels or more is a camera gesture (the threshold above); a right press released before that opens the viewport context menu.
- Viewport context menu (`data-testid="viewport-menu"`, `role="menu"`), at the pointer; a pick target under the pointer that is not selected first becomes the selection (as a click would). Shift+F10 or the ContextMenu key on the focused viewport opens it at the center of the selection's screen rectangle, or the viewport center. Items come from `viewportMenuItems(app)` (2.11), each with its formatted shortcut, leaving out disabled ones, so the menu and the radial marking menu of 1.2 can share them:
  - A feature command active: "OK" (when complete) and "Cancel".
  - Sketch mode: the applicable constraints of the sketch selection (3.8), "Construction" and "Delete" when something is selected, a separator, "Finish sketch".
  - Otherwise: "Repeat {title}" (`edit.repeat`, when there is a command to repeat), the selection's suggested commands (3.8), "Edit {label}" (one face, edge or vertex: its `element.feature`; one feature: that feature), "Hide" (`view.visibility`, when the selection holds bodies, sketches, datum planes or origin items, which have visibility), "Look at" (when enabled), a separator, "Fit to view".
- During a gesture hover picking pauses; the ID pass is re-rendered on the next pick after the camera has been still for 100 ms.
- The viewport is focusable (`tabindex="0"`, `role="application"`, `aria-roledescription="3D view"`) and takes the keys of 3.5.
- The navigation preset is a setting (`settings.navigation`), mirrored on the viewport as `data-nav-preset` (`fusion` or `freecad`). The right-drag gestures work in both presets, so no one has to pick a preset to orbit; the preset only decides the middle-button gestures.
- The viewport mirrors the camera in `data-view` (`classifyView`), `data-projection`, and `data-camera-target` (`"{x},{y},{z}"` with each coordinate rounded to 2 decimals, trailing zeros trimmed, `-0` as `0`).

### 3.14 View cube and viewport controls

- The view cube (`data-testid="view-cube"`, `role="group"`, `aria-label="View cube"`) is a DOM element, `size-24` (6rem, so it follows the UI scale) at `top-4 right-4` of the viewport. Its static 3D layout uses Tailwind's 3D transform utilities only (`perspective-*`, `transform-3d`, `backface-hidden`, `rotate-x-*`, `rotate-y-*`, `translate-x-*`, `translate-y-*`, `translate-z-12`; translations are in the parent's frame, since Tailwind applies `translate` before `transform`), and an inner rotor turns every frame to match the camera through `orientElement` (3.12). Faces are `surfaceRaised` with `border` hairlines and `textMuted` labels, and turn `hover` on hover. Its 6 faces are buttons labeled "Top", "Bottom", "Front", "Back", "Left", "Right" with accessible names "{Face} view"; its 12 edges are buttons named "{Face} {face} edge view" (pairs in the order Front, Back, Left, Right, Top, Bottom: "Front top edge view", "Front left edge view", "Right bottom edge view", ...); its 8 corners are buttons named "{Front or Back} {Left or Right} {Top or Bottom} corner view".
- Clicking a face sets that standard view; an edge sets the view looking from the normalized sum of its two face directions; a corner from the sum of three (Front right top is `iso`). Up is the projection of +Z onto the view plane, except views looking straight down or up (top and bottom faces) which use the up vectors of 2.11. The target, distance and projection are kept; the change animates.
- Dragging on the cube orbits about the target at the same rate as the navigation orbit.
- A "Home view" button (`house` icon) at the cube's top-left corner sets `iso` and fits the visible bounds.
- Viewport controls (`data-testid="viewport-controls"`), a vertical stack of `size-8` icon buttons under the cube: "Fit to view" (`maximize`), "Orthographic" (`cuboid`, `aria-pressed` true in orthographic), "Section view" (`scan-line`, `aria-pressed`; turning it on opens the section popover), "Selection filter" (`mouse-pointer-2`, a menu of `menuitemcheckbox` items Bodies, Faces, Edges, Vertices; hidden in sketch mode).
- Fit to view frames `scene.visibleBounds` with `fitBounds` (margin 0.1), animated; with nothing visible it frames a 100 mm cube at the origin.
- Look at (`view.look_at`): with one flat face (its `placement`), origin plane or datum plane selected, or in sketch mode (the sketch placement), the camera looks along the negated normal from the front side with the plane's y-axis up, target the plane point (face centroid, plane origin), distance and projection kept. Otherwise the command is disabled with "Select a flat face or a plane first."
- Orthographic toggle uses `withProjection`.

### 3.15 Section view

- Toggled by the "Section view" control or Shift+X. When turned on, the section popover (`data-testid="section-popover"`) opens anchored to the control: Plane (segmented XY, XZ, YZ; default XY), Offset (`ExpressionInput`, unit length, default the center of the visible bounds along the plane normal formatted with `formatNumber`), Flip (switch).
- The section removes the half-space where `dot(n, p) > offset`, with `n` the plane normal (XY +Z, XZ -Y, YZ +X; Flip negates both `n` and `offset`). It clips bodies, edges, vertices and the ID pass (removed parts cannot be picked). Sketches, datum and origin planes and the bed are not clipped.
- The section is a view setting: not saved, not undoable, kept across context changes until turned off. `data-section` on the viewport is `on` or `off`.

### 3.16 Sketch mode

#### 3.16.1 Entering

- `startNewSketch`: when the selection is exactly one plane-like item (origin plane, datum plane, or a face whose `surface` is `plane`), `createSketchOn(item)`. Otherwise the app enters plane picking: context Sketch with `sketchId` null; the sketch header reading "Pick a plane or a flat face to draw on" followed by the buttons "Top", "Front" and "Right" (accessible names "Sketch on Top plane (XY)", "Sketch on Front plane (XZ)", "Sketch on Right plane (YZ)"), which run `createSketchOn` for that origin plane, so the first plane is one click or one Tab away; origin planes shown, each with a label at the corner of its square in the plane's positive first and second axis directions (`data-testid="plane-label"`, `pointer-events-none`, `text-xs` on a `surfaceRaised` chip, positioned per 3.12): "Top (XY)", "Front (XZ)", "Right (YZ)"; the sketch hint (3.16.3) reading `plane_pick_hint` ("Top plane (XY) is the print bed."); and only origin planes, datum planes and faces accepted. A face that is not flat shows the hint `pick_plane`. Clicking a browser origin plane or datum row works too. Esc returns to Model.
- `createSketchOn(item)`: commits one `apply_edits` labeled "New sketch" with `add_feature` of a `sketch` record whose `plane` is sketch.md's `SketchPlane` for the item: `{"kind": "origin", "plane": "XY"}`, `{"kind": "datum", "feature": id}` or, for a face, `{"kind": "face", "body": id, "face": <Reference>, "fallback": <Placement>}` with the fallback from the face's `ElementDescriptor.placement`; empty entities and constraints; inserted by the marker rule of 3.7. Then `enterSketch(new id)`.
- `enterSketch(id)`: cancels an active command, sets context Sketch, shows the sketch for the session, gets its solution (`sketch_solved` events or `solveSketch(documentHandle, id)`), and moves the camera: `lookAlong` the placement normal with the placement y-axis up, then, when the sketch has entities, frames their bounds in the sketch plane (`fitBounds`, margin 0.1); an empty sketch keeps the visible height and targets the placement origin. The projection is kept. The camera before entering is pushed as a sketch view entry (3.20) and kept as `cameraBefore` (3.16.11). Downstream features stay visible and recompute after each sketch edit.

#### 3.16.2 Display

- Active sketch curves: overlay lines 1.5 CSS pixels wide (times the UI scale) colored by `SolvedEntity.constrained` (sketch.md `EntityState`): `under_constrained` `sketchUnder`, `fully_constrained` `sketchFull`, `conflicting` `sketchOver` (2.16.4). Construction entities are dashed (6-pixel dashes, 4-pixel gaps) and drawn in `sketchConstruction` instead of `sketchFull` when fully constrained, so finished construction geometry recedes behind the profile; under-constrained and conflicting construction entities keep `sketchUnder` and `sketchOver`. Hovered and selected curves are 2.5 CSS pixels wide in `highlight`. Circles and arcs are tessellated by the UI from entity data with a chord tolerance of 0.25 CSS pixels, re-tessellated when the scale changes by more than a factor 1.5.
- Points: line ends and arc ends as 5-pixel squares and centers as 5-pixel crosses in their entity's color; point entities as 6-pixel filled circles; points shared by a coincident constraint as filled squares; hovered and selected points 7 pixels in `highlight`.
- The origin point (9-pixel cross) and the X and Y axes (dashed lines across the view) in `sketchReference`, pickable for snapping and constraints, which refer to them by sketch.md's reserved references `origin`, `x_axis` and `y_axis` (2.14).
- Closed regions filled with `sketchRegion` at 12 percent from the solution's display buffers; hovered regions at 20 percent.
- Profile issues: each `profile_findings` entry gets a ring of 12 CSS pixels diameter stroked in `errorMark` (an SVG `circle` in the overlay, `data-testid="profile-issue"`, with an SVG `title` naming the issue): `open_endpoint` at `point` ("Open end"), `overlap` at the midpoint of `start` and `end` ("Overlapping curves"), `self_intersection` at `point` ("Curves cross"); `zero_length` gets no ring; the entities named by `entity` or `entities` get a 4-pixel `problem` halo at 30 percent in the WebGL overlay pass. The header and the finishing notification call these warnings, because the sketch stays valid and its other shapes still extrude; the rings stay `errorMark` because they mark exact spots and a yellow ring would not reach 3:1 against the viewport (2.16.6).
- Other visible sketches draw in `sketchReference` and are not pickable in sketch mode.
- Constraint glyphs (when "Show constraints" is on, the default; palette command "Show constraints" toggles it): one DOM button per geometric constraint except `coincident` and `point_on_object` (which have no glyph; the shared point is drawn filled), `size-4.5` (so it follows the UI scale), `data-testid="constraint-glyph"`, `data-constraint-id`, `data-type`, `data-state` (`satisfied`, `redundant`, `conflicting`, `unsatisfied`), accessible name the constraint title. Placement (runtime positions, 3.12): anchored at the midpoint of the first constrained curve (lines), the point at 45 degrees on circles and arcs, or the point itself, offset 14 CSS pixels (times the UI scale) perpendicular to the curve on screen; further glyphs on the same anchor stack 20 pixels apart along the curve. Style: a `rounded-sm` chip with a `border` hairline; `satisfied` and `unsatisfied`: icon in `textMuted` on `surfaceRaised` at `opacity-90`; `redundant`: `warningFill`; `conflicting`: `errorFill`. Hovering highlights the constrained items; clicking selects the constraint.
- Dimensions: dimension geometry (extension lines, dimension line with 8-pixel arrowheads, radius or diameter leader, angle arc) in `sketchReference` at 1 CSS pixel, plus a DOM label button (`data-testid="sketch-dimension"`, `data-constraint-id`, `data-type`, `data-driving`) at the label position (stored by sketch.md if it stores one, else `defaultLabelPosition`; positioned per 3.12). Label text, in `FONT_CLASSES.values`: `formatNumber` of the solved value without unit; prefixed `⌀` for diameter and `R` for radius; suffixed `°` for angles; a non-plain-number expression is prefixed `fx ` (`fx 40`); reference dimensions are wrapped in parentheses (`(40)`). A `rounded-sm` chip: `surfaceRaised` at `opacity-90` with `text`; `errorFill` when conflicting, `warningFill` when redundant. Double-click or Enter opens the dimension editor.

#### 3.16.3 Tools and commits

- Tools are activated by the toolbar, shortcuts or the palette. Activating another tool cancels the current operation. The active tool's toolbar button has `aria-pressed="true"`. The cursor is a crosshair while a drawing tool is active.
- The pointer maps to sketch coordinates by intersecting the camera ray with the sketch plane; when the ray is within 1e-6 of parallel to the plane, pointer input is ignored.
- `commitSketchChange(label, change)`:
  1. Builds candidate params: current entities and constraints plus the change.
  2. Calls `solveSketch(documentHandle, sketch, candidate)`. When a `requested` constraint id is in `conflicting` or `redundant`, returns `over_constrained` and applies nothing. When `inferred` ids are, removes them and solves once more.
  3. Calls `applyEdits(documentHandle, label, [update_feature])` with the candidate params whose entities carry the solved geometry (each `SolvedEntity` without `constrained`); for a sketch on a face, `plane.fallback` is set to `solution.placement`.
  4. A solution with `failed` true, or an `invalid_params` or `expression_error` `CoreError`, returns `failed` with the solver's or the core's message; nothing is applied.
- Labels: "Line", "Rectangle", "Circle", "Arc", "Point", the constraint title, "Dimension", "Change dimension", "Construction", "Delete". Drags are committed by `end_sketch_drag` (the core labels them).
- Step prompt: the sketch hint (`data-testid="sketch-hint"`, `role="status"`, a `surfaceRaised` chip with `text-xs` text in `textMuted`, under the sketch header in the top-center slot of 3.2) shows `sketchStepHint(step, platform)` (2.14): one sentence for the current step, from the `sketch_step_*` messages (4.3), with shortcuts rendered by `formatShortcut`. With no tool active it shows `sketch_step_start` in a sketch without entities and nothing otherwise, so a prompt appears only while a tool is active or the sketch is empty. A refused pick, a constraint tool's `constraintRequirement`, `arc_collinear` and `sketch_leave` (3.5) replace it while they apply. While an inference glyph is visible (3.16.5), the chip appends a space and `sketch_free_placement` in a span with `aria-live="off"`; the live region announces the hint only when the step changes, never on pointer moves.

| Step (`SketchStep`) | Message id |
| --- | --- |
| No tool, sketch without entities | `sketch_step_start` |
| Line, before the first click; chain in progress | `sketch_step_line_start`; `sketch_step_line_next` |
| 2-point rectangle, before and after the first click | `sketch_step_corner`; `sketch_step_opposite_corner` |
| Center rectangle, before and after the first click | `sketch_step_center`; `sketch_step_rectangle_corner` |
| Circle, before and after the first click | `sketch_step_center`; `sketch_step_circle_size` |
| Arc, before click 1, before click 2, before click 3 | `sketch_step_arc_start`; `sketch_step_arc_end`; `sketch_step_arc_shape` |
| Point | `sketch_step_point` |
| Dimension: nothing picked; a line picked; a point picked; a circle or arc, or two items picked | `sketch_step_dimension`; `sketch_step_dimension_place`; `sketch_step_dimension_second`; `sketch_step_dimension_label` |

#### 3.16.4 Drawing tools

Each tool's output is fixed (unit tests U-50 to U-54 check `recipes.ts`); "inferred" constraints come from 3.16.5; everything else is requested.

- **Line / polyline** (`line`, L). Click 1 sets the start S. A rubber band follows the cursor. Click 2 sets the end E and commits a line S to E; segments shorter than 2 CSS pixels are ignored. The chain continues from E: the next line starts at E with an inferred `coincident` between the previous line's end and the new line's start. The chain ends on Esc (the rubber band is discarded, the tool stays), Enter with no draw input focused, a double-click (which commits the segment to the clicked point first), or a click on the chain's first point (committing a closing segment with `coincident` to it).
- **2-point rectangle** (`rectangle_2point`, R). Click 1 sets corner C1, click 2 the opposite corner C2; a width or height under 2 CSS pixels is ignored. Output: lines L1 C1 to (C2.x, C1.y), L2 (C2.x, C1.y) to C2, L3 C2 to (C1.x, C2.y), L4 (C1.x, C2.y) to C1; `coincident` L1.end-L2.start, L2.end-L3.start, L3.end-L4.start, L4.end-L1.start; `horizontal` L1 and L3; `vertical` L2 and L4. Snaps at C1 apply to L1.start and at C2 to L2.end (inferred).
- **Center rectangle** (`rectangle_center`). Click 1 sets the center P, click 2 a corner K. Output: the 2-point rectangle with C1 = 2P - K and C2 = K, plus a construction line D from L1.start to L2.end with `coincident` D.start-L1.start and D.end-L2.end, and a construction point Q at P with `midpoint` (Q, D). Snaps at P apply to Q and at K to L2.end (inferred).
- **Circle** (`circle`, C). Click 1 sets the center, click 2 a point on the circle (radius under 2 CSS pixels is ignored). Output: one circle. A snap at the center applies to its center point; a point snap at click 2 adds `point_on_object` (that point, the circle) (inferred).
- **3-point arc** (`arc_3point`). Click 1 sets A, click 2 sets B, then the arc follows the cursor T through A, B and T; click 3 commits. When `circleThroughThreePoints` returns null or a radius above 1e6 mm, the rubber band is a straight segment, click 3 is ignored, and the sketch hint (`data-testid="sketch-hint"`, `role="status"`) says "These three points are in a line. Move the last point off the line to make an arc." Arcs are stored counterclockwise: when T lies on the clockwise path from A to B, the arc runs from B to A. Snaps at A and B apply to the arc's ends; a tangent inference at A applies.
- **Point** (`point`). Each click adds a point entity at the (snapped) location.
- **Construction** (`construction`, X). With entities selected and no drawing tool active, toggles their `construction` flag in one commit labeled "Construction". With a drawing tool active, toggles construction mode for new entities (`aria-pressed` on the button). The mode resets when sketch mode ends.

Dimension as you draw (draw inputs: DOM text inputs next to the rubber band's midpoint, `data-testid="draw-input"`, accessible names as listed):

- Line: "Length" (unit length) and "Angle" (unit angle, counterclockwise from the sketch +X axis). Rectangles: "Width" (along X) and "Height" (along Y). Circle: "Diameter". Arc, after click 2: "Radius".
- While a shape is rubber-banding, each empty draw input shows the live measured value as its placeholder (`formatNumber`, in `FONT_CLASSES.values` with the `textMuted` placeholder facet), written by the sketch controller with `setAttribute('placeholder', ...)` in the same frame as the rubber band (never through React, 2.1), so typed entry can be seen. Typing a digit, `.`, `-` or `(` focuses the first draw input and inserts the character; Tab focuses the first input, then cycles between the inputs; once a draw input has focus every key types into it, so an expression that starts with a name (`width / 2`) is typed after Tab. Letters with no draw input focused stay tool shortcuts. A valid typed value locks that quantity (the rubber band follows it; the other quantity still follows the cursor). Esc in a draw input clears the inputs and returns focus to the canvas.
- Enter commits the shape with the typed values: the end point is computed from them (line: `S + L(cos a, sin a)`; rectangles: the corner sign from the cursor's quadrant; circle: diameter; arc: radius on the cursor's side), and each typed value adds a driving dimension with the typed expression: Length a `distance` on the line; Angle: `horizontal` when the angle is a multiple of 180 degrees, `vertical` for 90 plus a multiple of 180, otherwise an `angle` between the line and the sketch X axis (`X_AXIS`); Width a `distance` on L1; Height a `distance` on L4; Diameter a `diameter`; Radius a `radius`. Without a typed angle, direction inference applies as usual. Snap inferences at a point computed from typed values are dropped; snaps at clicked points (S, C1, the center) still apply.

#### 3.16.5 Inference while drawing

- Snap targets within 8 CSS pixels (times the UI scale) of the cursor: every solved point of the active sketch and the origin point (`point`), the midpoints of lines (`midpoint`), and, within 6 pixels, curves including the origin axes (`curve`, at the closest point). Priority point, then midpoint, then curve; nearest first within a class.
- Direction inference for line segments with a start S: `horizontal` when the segment's angle to the sketch X axis is within 3 degrees modulo 180, `vertical` likewise to the Y axis; `tangent` when S is the end of an arc (or, for a 3-point arc, A is the end of a line or arc) and the directions differ by at most 5 degrees. When tangent applies, horizontal and vertical do not. Without a snap at the end, a horizontal or vertical inference moves the end exactly onto the horizontal or vertical through S.
- Constraints from snaps (all inferred): point: `coincident` (new point, target point; the origin point is `ORIGIN_POINT`); midpoint: `midpoint` (new point, line); curve: `point_on_object` (new point, curve; the axes are `X_AXIS` and `Y_AXIS`). Direction: `horizontal`, `vertical` or `tangent` on the new curve.
- While inferring, one inference glyph per inferred constraint (`data-testid="inference-glyph"`, `data-type` the constraint type; snap first, then direction, side by side) shows 16 CSS pixels (times the UI scale) to the right of and below the cursor (positioned per 3.12), and the snap target is highlighted in `highlight`.
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

- Over-constrained (requested id conflicting): a warning notification `over_constrained` (4.3), where the list names the other conflicting constraints by title (dimensions as "Distance 40"), joined with ", " and " and "; actions "Replace them" (one commit labeled with the title that removes those constraints and adds the new one) and "Cancel". Redundant: an info notification "{Title} is already implied by other constraints, so it was not added."

#### 3.16.7 Dimension tool

- `sketch.dimension` (D). Click a curve or point; for a circle or arc the label follows the cursor and the next click places it. For a line, the next click on empty space places its length dimension; a click on a point or line instead adds the second operand, then the next click places the label. For a point, the next click must be a point or a line.
- `dimensionTypeFor`: two points (a line's ends count) not axis-aligned (both coordinate differences above 1e-9 mm): label x within the points' x range and y outside the y range gives `horizontal_distance`; y within and x outside gives `vertical_distance`; otherwise `distance`. Axis-aligned pairs give `distance`. A point and a line: `distance`. Two parallel lines (within 1e-6 rad): `distance`. Two other lines: `angle`, measuring the sector between them that contains the label. A circle: `diameter`. An arc: `radius`. Anything else: unsupported, with `cannot_dimension` (4.3; {a} and {b} as `a point and a circle`).
- While placing, the dimension draws with its measured value. The placing click opens the dimension editor (`data-testid="dimension-editor"`): a text input "Dimension value" prefilled with `formatNumber(measuredValue)` (selected), a toggle button "Reference" (`aria-pressed`), and for circles and arcs a segmented "Diameter | Radius". Enter commits a dimension with the text as its expression and `driving` true unless Reference is pressed; the text is validated live as an expression of the dimension's unit (length or angle) and an invalid text keeps the editor open with the message.
- When the probe finds the new driving dimension redundant or conflicting, the editor stays open and shows, with `role="alert"`, `dimension_over_constrained` (4.3) and the buttons "Add reference" (commits it with `driving` false) and "Cancel".
- The tool stays active after a commit. Esc closes the editor without adding anything.
- Editing an existing dimension (double-click its label, Enter on the focused label, or the selection inspector) opens the editor prefilled with the stored expression text; Enter commits a replacement labeled "Change dimension". When the probe reports a non-empty `conflicting` list or `failed` true, the editor keeps the text and shows "The sketch cannot be solved with this value. Check for dimensions that cannot all be true at once."

#### 3.16.8 DOF display

- Entity colors come from `SolvedEntity.constrained` (3.16.2).
- The sketch header (`data-testid="sketch-header"`, a `surfaceRaised` chip button at the viewport's top center, `data-state`, `data-dof`) shows the sketch label and a status in plain words. `data-state` is the first that applies: `empty` "No geometry yet" (no entities); `conflict` "{n} conflicting constraints" in `errorText` (`conflicting` non-empty); `failed` "Cannot solve" in `errorText` (`failed` true); `full` "Fully constrained" (`dof` 0); `under` "Not fully constrained · {n} DOF". A `status` of `redundant` maps by the same rules. When the solution has `profile_findings`, " · {outline}" follows in `warningText`, where {outline} is `outlineProblems(solution)` (2.14): "1 open end" or "{n} open ends" when every finding is `open_endpoint`, else "1 outline problem" or "{n} outline problems", n being the number of findings; the header's `data-outline` is that number (0 otherwise). An open outline, which is what actually keeps a shape from extruding, is thus named where the user looks, without changing `data-state`. When the solution has redundant constraints, " · 1 extra constraint" or " · {n} extra constraints" follows in `warningText`. During plane picking the header is a group (not a button) holding the prompt and the three plane buttons of 3.16.1, with `data-state="picking"`.
- Clicking the header opens a popover (`data-testid="sketch-problems"`) listing conflicting and extra constraints (title and a "Delete" button each) and outline problems (text and a "Show" button that frames the problem). When `dof` is above 0 it also lists "Free geometry: {counts}" (the `under_constrained` entities counted by type, for example "3 lines, 1 circle") with a button "Select them" that puts those entities in the sketch selection, so the selection inspector (3.8) offers their constraints. It always ends with `sketch_dof_help`.
- The status bar shows `status-dof` (3.21).

#### 3.16.9 Drag and box selection

- With no tool active, pressing on a point of the active sketch and moving more than 3 CSS pixels drags it. Points only in M1; pressing on a curve starts nothing.
- Each pointer move maps the cursor to sketch coordinates (no inference) and submits `dragSketch(documentHandle, sketch, point, to)` through a `LatestOnly`. Each non-null solution redraws the sketch in the next frame (`ps:sketch-drag`).
- Release: when at least one drag request was sent, the UI awaits the drag `LatestOnly`'s `flush()`, then `endSketchDrag(documentHandle, sketch, true)` through the mutation queue (one undo entry); otherwise nothing. Esc during the drag: `endSketchDrag(documentHandle, sketch, false)` and the stored geometry is redrawn.
- Over a point whose entity is `fully_constrained`, the cursor is `not-allowed`.
- Pressing on empty space and moving more than 3 CSS pixels draws a selection rectangle (`data-testid="box-selection"`, an SVG `rect` in the overlay stroked in `accent`, 3.12). Dragging right (end x at or above start x) is a window selection: entities whose every point and tessellation vertex lie inside. Dragging left is a crossing selection (dashed stroke): entities with any part inside. Shift or Mod toggles the result into the sketch selection; otherwise it replaces it.

#### 3.16.10 Sketch selection and deletion

- Clicks select one entity, point, glyph or dimension label (Shift or Mod toggles; a click on empty space clears). `Mod+A` selects every entity.
- Delete removes the selected entities and constraints, plus every constraint that references a removed entity or one of its points, in one commit labeled "Delete".

#### 3.16.11 Finishing

- Finish sketch (toolbar, `Mod+Enter`), the Model or Print radio, or a feature command ends sketch mode: the context becomes Model (or the chosen one), the sketch session clears, and the sketch stays visible for the session.
- Camera: when `cameraAfterSketch(camera, cameraBefore, placement normal)` (2.11) returns a camera, the view animates to it (250 ms, 2.11); otherwise the camera stays. So a camera still looking straight at the sketch returns to the view from before the sketch, or turns to `iso` when that view was straight on too, and the next feature's preview is seen in depth; a view the user already turned away during the session stays where it is.
- Finishing with Finish sketch (toolbar or `Mod+Enter`) or the Model radio sets the app selection from the finished sketch's solution: its region when it has exactly one; otherwise, when it has no region, its non-construction point entities; otherwise, when it has entities, the sketch feature itself. The selection inspector (3.8) then shows the next step (Extrude and Revolve for a region or a sketch with regions, Hole for points). A ring with a hole is two regions, so the sketch feature is selected for it. A feature command run from Sketch uses the preselection rule of 3.3 instead.
- When the solution has profile findings, the warning notification `profile_problems` offers "Back to sketch".

### 3.17 Print context

- Printer: the toolbar menu button "Printer: {profile name}", or "Printer: {default name} (default)" when the document has no printer, the default being the profile `generic-fdm` from `listPrinterProfiles`, which the engine then uses (print.md), so the user sees which printer the checks used before trusting "Ready to print". It opens `data-testid="printer-menu"` (`role="menu"`): radio items for every profile, each with a `textMuted` detail of its key numbers (`printerDetail` in `src/print/printer.ts`; FDM: "{width} × {depth} × {height} mm bed · {nozzle} mm nozzle", or "⌀{diameter} × {height} mm bed · {nozzle} mm nozzle" for a round bed; resin: "{width} × {depth} × {height} mm"), under the headings "FDM" and "Resin" by technology, then "Default ({default name})", which clears the printer. Choosing one commits `set_printer` (label "Change printer"). The status bar printer button opens the same menu. `printerButtonText` gives the button's text.
- Overhang map: a toggle (`aria-pressed`, default on, session state) that alone shows or hides the overhang color map and its legend.
- Checks: a popover (`data-testid="checks-popover"`) with one switch per check in this order and wording: Overhangs (`overhangs`), Bed fit (`bed_fit`), First layer contact (`first_layer_contact`), Thin walls (`thin_walls`), Small features (`small_features`), Islands (`islands`), Trapped volumes (`trapped_volumes`), Mesh validity (`mesh_validity`, always on and disabled, help "Always checked before export"). The core runs every check (protocol.md `analyze_print`); a switched-off check only hides its findings from the badge counts and the list. The choice persists in `settings.hiddenChecks`.
- Print check: the toolbar button "Print check" (with the shown finding count) opens the side panel `health`, titled "Print check". The verb "Check print" names the action of running the checks everywhere (the `none` badge, `print.check_now`, the export popover); the noun "Print check" names the panel and the badge.
- Lay on face: a toolbar button "Lay on face" (`arrow-down-to-line` icon, `print.lay_on_face`, `aria-pressed` while an orientation is set; tooltip "Lay on face: pick the face that should rest on the bed. Click again to print as modeled."). With no orientation set it starts a face pick (`startLayOnFace`, `print.pickingFace` true): the print hint (`data-testid="print-hint"`, `role="status"`, a chip like the sketch hint in the top-center slot of 3.2) reads `print_pick_face`, only flat faces of visible part bodies are accepted (another face shows `pick_flat_face` in the hint for 3 s), and Esc cancels (Esc ladder step 5). The picked face sets `print.orientation` to `{kind: 'face_down', body, face: <its Reference>}`. With an orientation set, the button resets it to as modeled. A `CoreError` with code `invalid_orientation` or `reference_unresolved` resets it and shows the core message as a warning notification. Rows of the `overhangs`, `first_layer_contact` and `islands` checks have a button "Lay on a face" at the row's end that starts the pick; in these rows the finding text is the button that focuses the finding, so buttons never nest. The orientation is session state like body locks (open issue 3): it never changes the design, and while it is set the print check panel header and the export popover say so (`orientation_set`, 4.3, with "Print as modeled").

Analysis runs:

- In Print context, 300 ms after the document revision or `print.orientation` last changed with no transaction open, and on entering Print when the report is stale: `analyzePrint(documentHandle, { orientation: print.orientation ?? { kind: 'as_modeled' } })` through a `LatestOnly` (a `null` superseded result is ignored); `reportOrientation` records the orientation sent. `print.running` is true meanwhile.
- Outside Print, analysis never runs on its own. The badge shows the last report as stale; clicking the badge runs the analysis and opens the print check panel.

Health badge (`data-testid="health-badge"`, a button in the status bar, `data-state`, accessible name as listed):

| State | When | Icon and text | Accessible name |
| --- | --- | --- | --- |
| `none` | No report yet | `circle-dashed`, "Check print" | "Print check: Check print" |
| `running` | Analysis in flight | `loader-circle` (`animate-spin motion-reduce:animate-none`), "Checking" | "Print check: Checking" |
| `ok` | No shown errors or warnings | `circle-check` in `successMark`, "Ready to print" | "Print check: Ready to print" |
| `warning` | Shown warnings, no shown errors | `triangle-alert` hazard sign (`warningMark`), "1 warning" or "{n} warnings" | "Print check: {visible text}", for example "Print check: 2 warnings" |
| `error` | Shown errors | `circle-x` in `errorMark`, "1 error" or "{n} errors" | "Print check: {visible text}, {w} warning(s)" when there are warnings too, for example "Print check: 1 error, 2 warnings" |

The badge counts the errors and warnings of non-hidden checks over every `report.bodies[i].report.checks[j]`, adding each check's `omitted` to the count of its listed findings' severity; info findings never change the badge. Each accessible name starts with the visible text (WCAG 2.5.3). While the analysis runs the badge shows `running` and is not marked out of date. A report whose revision differs from the document revision, or whose `reportOrientation` differs from `print.orientation`, is stale: the badge keeps its state, draws at `opacity-50`, sets `data-stale="true"`, appends " (out of date)" to its text and ", out of date" to its name. Clicking the badge in any state runs the analysis when stale or absent, and opens the print check panel.

Print check panel (side panel `health`, `data-testid="health-list"`):

- Header: "Print check: {profile name}" and a "Check again" button; while `print.orientation` is set, a line with `orientation_set` and its "Print as modeled" button under it.
- Rows (`data-testid="health-item"`, `data-check`, `data-severity`): rows come from `report.bodies[i].report.checks[j].findings[k]`; the body name comes from `bodies[i].body`, `data-check` from `checks[j].check`, and `data-severity` from the finding (`info`, `warning`, `error`; info rows use the `info` icon in `textMuted`, warning rows the `warningMark` hazard sign, error rows `circle-x` in `errorMark`). Each row shows the severity icon, the finding's message verbatim and the body name in `textMuted`. Sorted by severity (errors first), then by the check order above, then report order. Hidden checks are left out.
- Clicking a row or pressing Enter on it focuses the finding (`focusFinding` with its `FindingKey`): the camera frames a cube of side `max(20 mm, 0.25 × the body's bounding-box diagonal)` centered at `location.at` (a point location) or the center of `location.min` to `location.max` (a bounds location) (`zoomToBounds`, margin 0.3), and a ring of 16 CSS pixels diameter stroked in `errorMark` (`data-testid="finding-marker"`, an SVG `circle` in the overlay, 3.12) is drawn there. Print findings always have a location. The marker stays until another row is chosen, the panel closes or the document changes.
- Empty: "No print issues found with {profile name}." With no visible part body that has geometry (engine.md 3.12 analyzes only those): "Add a body to check it for printing." Before the first report: "Not checked yet."; while the first analysis runs: "Checking the model for {profile name}."

Overhang map and legend:

- Shown in Print context while Overhang map is on. For each `BodyPrint` whose `mesh_id` equals the body's current model-layer mesh id, its overhang buffer is set as the body's scalar field (3.12 pass 3); a body whose mesh changed since the report shows no map until the next report.
- The limit `a` is the `limit` of the first finding of an `overhangs` check in the report when present, else the profile's `max_overhang_angle` (print.md).
- Legend (`data-testid="overhang-legend"`, bottom-left of the viewport): title "Overhangs", a `h-2` bar with the `analysis` gradient (2.16.3; `analysisLow` to `analysisHigh`), labels "{a}°" and "90°" (degrees with `formatNumber`), and the caption "Faces steeper than {a}° from vertical need support."

Bed outline: in Print context, after a report, the bed outline of the report's profile (the `printers` entry with id `report.profile`): the rectangle [0, width] × [0, depth], or the circle of `diameter` around the bed origin, at z = 0 in bed coordinates, mapped into model coordinates by the inverse of the first `BodyPrint.transform`, drawn 1.5 CSS pixels wide in `bed`, depth-tested.

### 3.18 Export

- While a feature command is active, the export commands are disabled with `finish_command_first` (export uses the model state, which ignores the focus).
- The top-bar Export button, `Mod+Shift+E` and the Document menu open the export popover (`data-testid="export-popover"`): Format (radio group 3MF, STL, OBJ, STEP; default 3MF), Bodies (radio group "All visible bodies", "Selected bodies"; the second is disabled unless bodies are selected), and an "Export" button.
- The Print toolbar's "Export 3MF" exports 3MF of all visible bodies directly; its Export group exports STL, OBJ or STEP of all visible bodies directly.
- The export popover shows, above its Export button, up to three status lines (`data-testid="export-status"`, `text-xs`; `exportStatus` in `src/export/status.ts`), so what is about to be printed is clear while export stays one click with no confirmation: (1) when an active feature has status `error`, `export_failed_feature` (one feature) or `export_failed_features` (several) with the `warningMark` hazard icon and a button "Show problems" (opens the Problems panel); (2) for 3MF and STL only, "Print check: {state}" with the health badge's icon and text (3.17; "Not checked" for `none`, " (out of date)" appended when stale) and, when the badge is `none` or stale, a button "Check print" that runs the analysis and opens the print check panel; (3) for 3MF and STL while `print.orientation` is set, `orientation_set` with its "Print as modeled" button.
- `exportModel(request)`: the bodies are the visible bodies with model-layer geometry (for STL, OBJ and STEP only part bodies, since modifiers are not solid geometry; engine.md 3.13), or the selected bodies. With none, the error notification "There is no visible body with geometry to export." appears and the core is not called. Otherwise `exportBodies(documentHandle, format, bodies, orientation)` with `{"kind": "3mf"}`, `{"kind": "stl", "binary": true, "per_body": false}`, `{"kind": "obj"}` or `{"kind": "step"}`; 3MF and STL pass `print.orientation` when one is set, OBJ and STEP stay as modeled.
- The single returned file is saved with `files.chooseSaveTarget('{document name}.{3mf|stl|obj|step}', kind)` and `files.write`. A canceled picker does nothing. Success shows the success notification "Exported {file name}.", or, when an active feature has status `error`, the warning notification `exported_with_failures` with "Show problems" instead (this covers the Print toolbar's direct exports, which have no popover). Each warning diagnostic shows a warning notification with its message. A `CoreError` shows its message as an error notification.

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
- Save (`Mod+S`): with a file handle, `saveDocument(documentHandle)` and `files.write({ kind: 'handle', ... }, 'ptsm', utf8)`; without one, Save as. On a successful write, `markSaved(documentHandle, revision)` with the revision `saveDocument` returned, then `localUnsaved` false and the recovery store cleared. A `FileServiceError` sends no `markSaved` (the core stays dirty) and shows: permission: "Could not save {file} because Partsmith no longer has permission to write it." (action "Save as"); not_found: "{file} was moved or deleted. Save it again to choose a location." (action "Save as"); io: "Could not save {file}: {reason}" (action "Save as").
- Save as (`Mod+Shift+S`): `files.chooseSaveTarget('{name}.ptsm', 'ptsm')` first (a cancel stops with nothing changed), then `saveDocument(documentHandle)`, `files.write` and, on success, `markSaved(documentHandle, revision)` as in Save (a failed write sends no `markSaved`). A handle target becomes the document's file (name from the handle) and is remembered in recent files; a download target clears the handle, and the first download in a session shows the info notification "This browser saves by downloading. {file name} is in your downloads folder."
- Open recent (Document menu submenu, palette items): up to 10 entries, newest first, each showing the name and its open time (`toLocaleString('en-US', { dateStyle: 'medium', timeStyle: 'short' })`). An entry with a handle reads it (`readHandle`, which may prompt for permission on this click): permission denied shows "Partsmith needs permission to open {file}. Choose it again to grant access." (action "Open..."); not found shows "{file} has moved or been deleted, so it was removed from recent files." and forgets it. An entry with text opens that text (no handle; Save goes to Save as).
- Recovery copy: while the document is dirty, 2 s after the last `document_changed` (debounced), the UI writes a recovery entry (`reason: 'autosave'`, `JSON.stringify(recipe, null, 2)`, the handle). A successful save, a New without changes, or Discard clears it. `beforeunload` calls `preventDefault()` while the document is dirty so the browser asks before leaving.
- At startup a recovery entry shows the info notification (persistent until acted on) "Partsmith kept unsaved changes to {name} from {time}." with actions "Open" (opens the text as a document named {name}, attaches the stored handle, sets `localUnsaved` true) and "Discard" (clears the store).
- The desktop host replaces `FileService` with native dialogs (open issue 2); every behavior above stays.

### 3.20 Undo and redo

- `edit.undo` and `edit.redo` call the core's `undo` and `redo` through the mutation queue. Their palette details are "Undo {undoLabel}" and "Redo {redoLabel}"; with nothing to undo or redo they are disabled with "Nothing to undo." or "Nothing to redo." After either, a polite live region announces "Undid {label}" or "Redid {label}".
- With a feature command active, `Mod+Z` and `Mod+Shift+Z` step through the command history (3.7). With an empty history `Mod+Z` cancels the command, and with an empty redo stack `Mod+Shift+Z` is disabled with `finish_command_first`. Their palette details are "Undo change to {field label}", "Redo change to {field label}" and, with an empty history, "Cancel {title}".
- In sketch mode with an operation in progress (rubber band, drag, box selection, open editor), `Mod+Z` cancels that operation only.
- Sketch view history (technical design 13, principle 6): during a sketch session the UI keeps a stack of entries, each either `view` (a camera) or `document` (a marker for a committed sketch edit). Entering the sketch pushes the camera from before entering; each completed orbit or pan gesture, each wheel burst (ended by 300 ms without wheel events), view cube click, fit, home and look at pushes the camera from before it; each committed sketch edit pushes a `document` marker. In sketch mode, `Mod+Z` pops the top entry: a `view` entry restores its camera (animated) and moves to a redo stack with the camera it replaced; a `document` entry calls the core's `undo`. With the stack empty, `Mod+Z` calls the core's `undo` directly. `Mod+Shift+Z` mirrors this with the redo stack. A new view change or edit clears the redo stack. Leaving sketch mode discards both stacks.

### 3.21 Status bar

Left to right (`data-testid` in parentheses):

- Core status (`status-core`), shown only when not idle: "Loading core" with a spinner, "Rebuilding" with a spinner when a mutating request has been pending for more than 300 ms, or "Core stopped" in `errorText`. Spinners are `loader-circle` with `animate-spin motion-reduce:animate-none`.
- Sketch DOF (`status-dof`, sketch mode with a sketch only): "{n} DOF" (the number in `FONT_CLASSES.values`), with `data-dof` and `data-state` as the sketch header, and the tooltip "Degrees of freedom: how many ways the sketch can still move."
- Grid (`status-grid`, shown while a grid is drawn, 3.12): "Grid {formatQuantity(spacing)}" ("Grid 10 mm") in `textMuted`.
- Rebuild time (`status-recompute`): "Rebuilt in {n} ms" (the duration in `FONT_CLASSES.values`; integer) or "Rebuilt in {s} s" (one decimal) for durations of 1000 ms or more, measured from sending the last mutating request to receiving its response; tooltip "Time the last change took to rebuild".
- Units (`status-units`): a button with the unit ("mm") opening a menu of radio items mm, cm, m, in. Choosing one commits `set_units` (label "Change units to {unit}") and shows the info notification "Units are now {unit}. Numbers typed without a unit now mean {unit}." (model.md 3.12).
- Printer (`status-printer`): a button with the profile name, or "{default name} (default)" when the document has none, opening the printer menu (3.17).
- Print check badge (`health-badge`, 3.17).

### 3.22 Notifications

- Stack at the viewport's bottom-right (`bottom-4 right-4`), `w-90`, newest at the bottom, at most 3 shown; each is a `surfaceRaised` panel with a `border` hairline, `rounded-md` and `shadow-lg`. A fourth removes the oldest non-error notification (or the oldest when all are errors).
- Each (`data-testid="notification"`, `data-severity`) has a severity icon (`info` in `textMuted`, `circle-check` in `successMark`, the `triangle-alert` hazard sign of `warningMark`, `circle-x` in `errorMark`), the message (left-aligned, `text`), action buttons, and a "Dismiss" button. Running an action dismisses the notification.
- Info and success dismiss after 5 s, warnings after 8 s; errors stay until dismissed. Timers pause while the pointer is over the notification or focus is inside it.
- Errors are announced through a `role="alert"` region; the others through a `role="status"` region.
- A notification with the same message as a visible one does not appear again; the visible one restarts its timer.
- A failure notification that names features (`Notification.features`) is dismissed automatically when none of them is in error anymore (after an undo or a fix), so it never outlives its cause (4.2).

### 3.23 Visual design

- Direction: a precision instrument used for hours at a time ([docs/guidelines/README.md](../guidelines/README.md)). Graphite and zinc surfaces, hairline `border` dividers, generous spacing and restrained motion; the model in the viewport is the hero and the chrome recedes (the viewport is lighter than the chrome in both themes, 2.16.4). It avoids the frontend guideline's generic looks: no decorative gradients (the only gradients are the viewport background and the analysis legend), no cream or off-white backgrounds (surfaces are neutral zinc), no pill-shaped buttons, no monospace labels, no numbered section labels and no italic accent words.
- Color: by role only (2.16.3, 2.16.4). One signal orange for selection, focus, the current tool and the primary action; blue only for under-constrained sketch geometry; red only for errors and conflicts; yellow hazard signs for warnings; green checks for success; the sky-to-violet ramp for analysis. Everything else is zinc.
- Type: Inter (`FONT_CLASSES.interfaceText`) for all interface text and JetBrains Mono (`FONT_CLASSES.values`) for values (2.16.7). Body text `text-sm` (0.875rem), secondary text and captions `text-xs` (0.75rem, the minimum anywhere), panel titles `text-base font-semibold`. Inter text that shows changing counts (badges, "3 edges", "{n} features") uses `tabular-nums`.
- Alignment: left by default (text, rows, panel headers, field labels, notifications, empty states). Centered only where this spec says so: the palette's position, the top-center slot of the viewport (the empty-state chip, the sketch header, the sketch hint and the print hint, 3.2), the loading and fatal overlays and the view cube's face labels.
- Spacing on Tailwind's 0.25rem scale: panel padding `p-4`, panel sections separated by `gap-4` and a `border` hairline, rows and controls `h-8`, toolbar buttons `h-10` with labels (`px-3 gap-2`) and `size-9` icon-only, gaps `gap-2`, `gap-3` and `gap-4`.
- Shape: `rounded-sm` (0.25rem) for controls, chips and swatches; `rounded-md` (0.375rem) for panels, popovers and menus; never `rounded-full` on a button. Floating panels are `surfaceRaised` with a `border` hairline and `shadow-lg`.
- Icons: lucide-react line icons with `strokeWidth={1.75}` (a component prop, not a style), sized with `size-5` in toolbars and `size-4` in lists and fields, colored through `currentColor` from the element's foreground role (`text`, `textMuted`, or a status role for status icons).
- Motion, restrained and with Tailwind utilities only: menus, popovers, tooltips, the palette and the inspector fade in over 150 ms (`transition-opacity duration-150 ease-out` with `starting:opacity-0`, or `starting:open:opacity-0` for popovers, which stay in the DOM while closed); hover color changes use `transition-colors duration-150`; spinners use `animate-spin`. Nothing slides or bounces. `motion-reduce:transition-none` and `motion-reduce:animate-none` remove all of it under `prefers-reduced-motion`, and camera animations (250 ms, 2.11) become instant. There is no animation library in M1.
- Focus: `focusRing` (a 2-pixel `accent` outline offset by 2 pixels on `:focus-visible`, Tailwind's `outline-2` and `outline-offset-2`) on every interactive element.

### 3.24 Accessibility

- Contrast: every text and non-text pair of 2.16.6 meets WCAG 2.2 AA (text 4.5:1, non-text 3:1) in both themes; U-70 enforces them. Text drawn over the viewport always sits on a `surfaceRaised`, `warningFill` or `errorFill` chip.
- Keyboard: every function is reachable without a pointer except picking faces, edges and vertices in the viewport, drawing sketch geometry, dragging sketch points and dragging the inspector (later). Selection fields for bodies, sketches, planes, axes and regions offer the "Choose" menu.
- Tab order: top bar (document name, context radios, toolbar, palette button, Export, theme toggle), browser (one stop), viewport (one stop), the buttons of the top-center slot while shown (the empty-state chip's, the sketch header or its plane buttons), the inspector when open, timeline (one stop), side panel, status bar buttons, notifications. Opening the inspector moves focus into it; closing it returns focus. During a feature command, Tab on the focused viewport goes to the command's value field instead (3.7).
- Roving tab index with arrow keys in toolbars, segmented controls, the timeline, the tree and menus. Menus close on Esc and return focus to their button.
- Tooltips appear after 500 ms of hover and immediately on keyboard focus, close on Esc, are referenced by `aria-describedby`, and show the command title and formatted shortcut, then the command's description (2.10) on a second line in `textMuted`. Field help (3.7) shows on hovering a field's label and reaches assistive technology through the control's `aria-describedby`.
- Every interactive target is at least `size-6` (1.5rem, 24 CSS pixels at 100 percent scale). Every icon-only button has an accessible name.
- Color is never the only signal: status has icons and text (warnings a hazard shape, errors a crossed circle), DOF has the header text, selection has thicker strokes, the analysis map has a legend.
- At 200 percent UI scale on a 1280 by 720 CSS-pixel window every region stays usable (toolbars go icon-only, primary buttons included, since the top bar is narrower than 64rem; panels scroll).
- Live regions: selection, undo and redo announcements, notifications, the sketch hint and the print hint (step changes only, 3.16.3).

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
| Invalid expression in a field | Under the field (`field-error`, `errorText`), field border `errorMark`; OK disabled | Edit the field |
| Invalid sketch dimension value | In the dimension editor | Edit the value, "Add reference" when over-constraining |
| Feature status during a command (preview) | Inspector banner (`inspector-error`), and the mapped field's `field-error` (3.7) | Errors and warnings whose `field` maps to a field: the field is highlighted and focused by clicking the message; "Use {suggestion}" when the message has a `suggestion`. In a feature with an inline Reverse (2.12): an `out_of_range` error whose `field` maps to the field Reverse is attached to (`attachedTo`), holding a plain number (2.13) that starts with `-`, offers "Reverse and use {text without the leading -}", which toggles Reverse and sets that text; a `no_effect` warning while Operation is Cut or Intersect, and a `through_all_misses` error, offer "Reverse", which toggles Reverse |
| Feature status after a user change: a feature goes from not-error to error | One notification (`Notification.features` naming those features): for one feature "{label} failed. {message}" with "Edit" and "Undo"; for several "{n} features failed after this change." with "Show problems" and "Undo". During a sketch session, sketch edits raise none (the timeline and browser badges show the failures); when the session ends, the features in error that were not in error when it started raise this notification once. The notification is dismissed automatically when none of the features it names is in error anymore | As listed |
| Feature warnings and errors at rest | Timeline and browser badges, tooltips, Problems panel, selection inspector | For a feature whose status has `repairs`, the "Edit" action (notification, Problems panel row, selection inspector) reads "Re-pick {field label}", the label of the field the first repair's `path` maps to (3.7), and runs `editFeature` with that field active, as "Edit" does; the repair marker (3.7) shows where the lost element was. "Edit" opens the inspector, where repairs re-pick the reference (the field `Repair.path` maps to, 3.7, becomes the active selection field); "Use {suggestion}" applies `update_feature` with the value at `field` replaced (label "Change {label}") |
| Sketch over-constraint while adding | Notification (3.16.6) or dimension editor (3.16.7) | "Replace them", "Add reference", "Cancel" |
| Sketch conflicts and extra constraints at rest | Sketch header, glyph and label colors, sketch problems popover | "Delete" per constraint; "Select them" for free geometry (3.16.8) |
| Profile (outline) issues | Rings and halos in sketch mode, the header's outline suffix; warning notification on finishing | "Back to sketch" |
| Command failed in the core (`CoreError`) | Error notification with the core message | "Undo" when the error followed an edit that was applied; otherwise none |
| `focus_inactive` when editing | Error notification with the core message | "Unsuppress" or "Move marker to the end" |
| File, export and permission errors | Error notification (4.3) | "Save as", "Open..." |
| Core fatal | Fatal state (4.4) | "Restart core", "Reload" |
| Unsupported browser | Unsupported page | none |

### 4.3 UI-owned messages

`src/messages.ts` exports each of these as a function of its placeholders; tests compare exact text. A shortcut placeholder such as `{L}` or `{Mod+Enter}` is `formatShortcut` of that shortcut for the platform; `{key}` in `sketch_free_placement` is "Ctrl" on Windows and Linux and "⌘" on macOS; `{failed}` in `exported_with_failures` is the text of `export_failed_feature` or `export_failed_features`.

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
| `exported_with_failures` | Exported {file}. {failed} | warning | Show problems |
| `export_failed_feature` | {label} failed, so it is not in the export. | status line | Show problems |
| `export_failed_features` | {n} features failed, so they are not in the export. | status line | Show problems |
| `deleted` | Deleted {label}. | info | Undo |
| `feature_failed` | {label} failed. {message} | error | Edit, Undo |
| `features_failed` | {n} features failed after this change. | error | Show problems, Undo |
| `over_constrained` | {title} conflicts with {list}, so it was not added. | warning | Replace them, Cancel |
| `redundant_constraint` | {title} is already implied by other constraints, so it was not added. | info | none |
| `dimension_over_constrained` | Other dimensions and constraints already set this size. Add it as a reference dimension that only shows the measurement? | inline | Add reference, Cancel |
| `dimension_unsolvable` | The sketch cannot be solved with this value. Check for dimensions that cannot all be true at once. | inline | none |
| `cannot_dimension` | Partsmith cannot dimension {a} and {b}. Pick two points, a point and a line, two lines, or one circle or arc. | hint | none |
| `arc_collinear` | These three points are in a line. Move the last point off the line to make an arc. | hint | none |
| `profile_problems` | {label} has {outline}, so some shapes cannot be extruded. | warning | Back to sketch |
| `sketch_dof_help` | Blue geometry can still move or change size. Add dimensions ({D}) or constraints where the size matters; you can extrude at any time. | popover | none |
| `units_changed` | Units are now {unit}. Numbers typed without a unit now mean {unit}. | info | none |
| `pick_plane` | Pick a flat face, a datum plane or an origin plane. | hint | none |
| `pick_axis` | Pick a straight edge, a sketch line, a round face or an origin axis. | hint | none |
| `pick_kind` | Pick {plural noun}. | hint | none |
| `pick_points` | Click a flat face or pick sketch points. | hint | none |
| `pick_flat_face` | Pick a flat face. | hint | none |
| `pick_same_body` | All {plural noun} must be on '{body}'. Remove them to pick {plural noun} of another body. | hint | none |
| `hole_one_face` | One Hole puts its holes on one face. Press Enter to finish this one, then start another. | hint | none |
| `parameter_name` | Names start with a letter or _ and use only letters, digits and _. | inline | none |
| `field_required` | {label} needs a value. | inline | none |
| `field_not_number` | {label} must be a number. | inline | none |
| `look_at_disabled` | Select a flat face or a plane first. | disabled reason | none |
| `nothing_to_undo` | Nothing to undo. | disabled reason | none |
| `nothing_to_redo` | Nothing to redo. | disabled reason | none |
| `finish_command_first` | Finish or cancel {title} first. | disabled reason | none |
| `needs_sketch_region` | Draw a closed shape in a sketch first. | disabled reason | none |
| `needs_solid` | Make a solid first, for example with Extrude or Box. | disabled reason | none |
| `needs_two_bodies` | Boolean needs two bodies with geometry. Make a second body first, for example with Box. | disabled reason | none |
| `nothing_to_repeat` | No command to repeat yet. | disabled reason | none |
| `import_not_editable` | Imported geometry has no settings to edit. | disabled reason | none |
| `fill_fields` | Fill in the highlighted fields first. | disabled reason | none |
| `core_not_running` | The geometry core is not running. | disabled reason | none |
| `command_canceled` | {title} was canceled. | info | Restore |
| `plane_pick_hint` | Top plane (XY) is the print bed. | hint | none |
| `sketch_step_start` | Choose Line ({L}), Rectangle ({R}) or Circle ({C}) to start drawing. | hint | none |
| `sketch_step_line_start` | Click to start a line. | hint | none |
| `sketch_step_line_next` | Click to end the segment, or type a length. Click the first point to close the shape, or press {Enter} to stop. | hint | none |
| `sketch_step_corner` | Click the first corner. | hint | none |
| `sketch_step_opposite_corner` | Click the opposite corner, or type the width. | hint | none |
| `sketch_step_center` | Click the center. | hint | none |
| `sketch_step_rectangle_corner` | Click a corner, or type the width. | hint | none |
| `sketch_step_circle_size` | Click to set the size, or type the diameter. | hint | none |
| `sketch_step_arc_start` | Click where the arc starts. | hint | none |
| `sketch_step_arc_end` | Click where it ends. | hint | none |
| `sketch_step_arc_shape` | Move to shape the arc, then click, or type the radius. | hint | none |
| `sketch_step_point` | Click to place a point. | hint | none |
| `sketch_step_dimension` | Click a line, circle, arc or point. | hint | none |
| `sketch_step_dimension_place` | Click to place the dimension, or click a second point or line to measure between them. | hint | none |
| `sketch_step_dimension_second` | Click a second point or a line. | hint | none |
| `sketch_step_dimension_label` | Click to place the dimension. | hint | none |
| `sketch_free_placement` | Hold {key} to place freely. | hint | none |
| `sketch_leave` | To leave the sketch, click Finish sketch or press {Mod+Enter}. | hint (3 s) | none |
| `print_pick_face` | Pick the face that should touch the bed. | hint | none |
| `orientation_set` | Printing with the picked face of {body} on the bed. | status line | Print as modeled |
| `region_crashed` | This part of the window hit a problem. Reload to restore it. | region fallback | Reload |

### 4.4 Fatal states

- Core load failure (`transport.start()` rejects, including a protocol version mismatch reported by the worker host): the viewport shows a panel (`data-testid="core-fatal"`, `role="alert"`) with `core_load_failed` and a "Reload" button (`location.reload()`); the rest of the shell stays but every core-dependent command is disabled with "The geometry core is not running."
- Core stopped later (fatal listener): the same panel with `core_stopped` and "Restart core". `app.restartCore()` creates a new transport and client, starts it, reloads the catalog and printers, and reopens the last recipe with `openDocument(JSON.stringify(recipe, null, 2))`, keeping the document name and handle and setting `localUnsaved` true. The recipe is the one of the last `document_changed` with `transactionOpen` false, so the edits of an open command (its preview, which may be what stopped the core) are never reopened as permanent features. A restart that fails keeps that recipe, name and handle for the next attempt; with no document open, the restart opens a new "Untitled" document. Undo history is lost; selection, command and sketch session are cleared.
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

Vitest unit tests live next to the code as `src/**/*.test.ts(x)` (jsdom; no WebGL; fakes from `src/test/fakes`). Playwright end-to-end tests live in `ui/e2e/*.spec.ts` and run against the production build with the real WebAssembly core in `ui/public/core/` (`pnpm -C ui e2e`; the project is Desktop Chrome, viewport 1280 by 720, device scale factor 1, Linux, so `Mod` is Control; use Playwright's `ControlOrMeta`). Protocol-level codec, bridge and worker host tests are protocol.md tests 53 to 66 and are not repeated here. Next to the tests, every change passes `pnpm -C ui lint` (`oxlint --deny-warnings`, then `oxfmt --check`) and `pnpm -C ui typecheck` (`tsc -b`); test files are linted like other code (oxlint's `vitest` plugin), and `src/generated` is excluded from both tools.

### 6.1 Selectors

Tests use these stable selectors only (plus accessible roles and names given in section 3).

| `data-testid` | Element | Attributes |
| --- | --- | --- |
| `app` | Root | `data-core-status` (`loading`, `ready`, `failed`), `data-context` (`model`, `sketch`, `print`) |
| (none; `html` element) | Document root | class `dark` in the dark theme (2.16.8) |
| `unsupported` | Unsupported page | |
| `core-overlay`, `core-fatal` | Loading overlay, fatal panel | |
| `top-bar`, `document-name`, `save-state`, `document-menu`, `palette-button`, `theme-toggle` | Top bar parts | `document-name`: `data-dirty`; `theme-toggle`: `aria-pressed` |
| `palette`, `palette-input`, `palette-option`, `palette-shortcut`, `palette-heading`, `palette-empty`, `palette-footer` | Palette | option: `data-item-id`, `data-kind` |
| `browser`, `browser-row` | Browser | row: `data-kind`, `data-id` |
| `color-popover` | Body color popover | |
| `viewport` | Viewport | `data-hover-kind`, `data-hover-key`, `data-selection-count`, `data-view`, `data-projection`, `data-nav-preset`, `data-section`, `data-camera-target`, `data-grid-spacing` |
| `view-cube`, `viewport-controls`, `section-popover`, `viewport-menu`, `empty-state`, `plane-label`, `repair-marker` | Viewport chrome | |
| `inspector` | Inspector | `data-mode` (`command`, `selection`), `data-preview` (`none`, `pending`, `shown`, `failed`) |
| `inspector-field`, `inspector-more`, `inspector-ok`, `inspector-error` | Inspector parts | field: `data-path` |
| `selection-field` | Selection field box | `data-active`, `data-count` |
| `expression-result`, `field-error`, `expression-suggestions` | Expression field parts | |
| `selection-title`, `body-size`, `body-center`, `entity-size`, `measure-row` | Selection inspector parts | measure row: `data-measure` |
| `timeline`, `timeline-item`, `timeline-marker`, `timeline-problems`, `context-menu`, `timeline-empty` | Timeline | item: `data-feature-id`, `data-status`, `data-suppressed`, `data-rolled-back` |
| `side-panel`, `parameters-panel`, `parameter-row`, `parameter-value`, `parameters-empty`, `parameters-transaction`, `problems-panel`, `health-list`, `health-item`, `keys-panel` | Side panels | row: `data-id`; health item: `data-check`, `data-severity` |
| `sketch-header`, `sketch-problems`, `sketch-hint` | Sketch chrome | header: `data-state`, `data-dof`, `data-outline` |
| `sketch-dimension`, `dimension-editor`, `constraint-glyph`, `inference-glyph`, `draw-input`, `profile-issue`, `box-selection` | Sketch overlay | dimension: `data-constraint-id`, `data-type`, `data-driving`; glyph: `data-constraint-id`, `data-type`, `data-state`; inference: `data-type` |
| `status-bar`, `status-core`, `status-dof`, `status-grid`, `status-recompute`, `status-units`, `status-printer`, `health-badge` | Status bar | dof: `data-dof`, `data-state`; badge: `data-state`, `data-stale` |
| `printer-menu`, `checks-popover`, `overhang-legend`, `finding-marker`, `print-hint`, `export-popover`, `export-status`, `drop-overlay` | Print and export | |
| `notification` | Notification | `data-severity`, `data-code` |
| (any) | Element positioned at runtime (3.12) | `data-positioned`; its `style` attribute holds only `translate` (the view cube rotor: only `transform`) |

### 6.2 Fixtures

`ui/e2e/fixtures/` holds `.ptsm` files written by the test author from model.md, sketch.md and the feature specs. Every fixture is canonical (`partsmith fmt` leaves it unchanged), valid (`partsmith validate` is clean), unit `mm`, with unnamed features (labels come from features.md 3.12).

| File | Content | Result |
| --- | --- | --- |
| `cube.ptsm` | Body `b-cube` "Cube". `f-sk1`: sketch on XY with a fully constrained 20 by 20 rectangle centered on the origin. `f-ext1`: extrude of its region, distance `20 mm`, op new, body `b-cube`. | Cube x, y in [-10, 10], z in [0, 20]; timeline "Sketch 1", "Extrude 1" |
| `rect-sketch.ptsm` | `f-sk1`: sketch on XY: rectangle with a corner coincident with the origin (`coincident` with sketch.md's reserved `origin` point), horizontal and vertical sides, driving `distance` `40 mm` on the bottom line and `20 mm` on the left line; no bodies. | Fully constrained, one region of 40 by 20 at x in [0, 40], y in [0, 20] |
| `underconstrained.ptsm` | `f-sk1`: sketch on XY with line `e-1` from (0, 0) to (30, 0) and circle `e-2` centered (50, 0) radius 5; no constraints; no bodies. | DOF 7 |
| `parametric.ptsm` | Parameter `p-size` `size` = `20 mm`; otherwise `cube.ptsm` with every dimension and the extrude distance set to `size`. | Cube of side `size` |
| `cube-features.ptsm` | `cube.ptsm` plus `f-fil1` fillet `2 mm` on the 4 vertical edges and `f-sh1` shell `2 mm` removing the top face. | 4 features, all ok |
| `two-bodies.ptsm` | `cube.ptsm` plus body `b-peg` "Peg" and `f-cyl1` cylinder radius `4 mm`, height `20 mm`, base center (0, 0, 10), axis +Z, op new, body `b-peg`. | Timeline "Sketch 1", "Extrude 1", "Cylinder 1" |
| `fillet-error.ptsm` | `cube.ptsm` plus `f-fil1` fillet `15 mm` on the 4 top edges. | "Fillet 1" fails with `fillet_too_large`, with `field` `/radius` and a `suggestion` |
| `hole-cube.ptsm` | `cube.ptsm` plus `f-sk2`, a sketch on the cube's top face (`{"kind": "face", "body": "b-cube", "face": <reference of f-ext1/end-cap>, "fallback": <placement at z 20>}`) with point entity `e-p1` at (0, 0). | Timeline "Sketch 1", "Extrude 1", "Sketch 2" |
| `revolve-sketch.ptsm` | `f-sk1`: sketch on XZ with a fully constrained rectangle at sketch x in [10, 20], y in [0, 10]; no bodies. | One region; sketch visible |
| `offset-cube.ptsm` | As `cube.ptsm` with the rectangle at x in [10, 30], y in [-10, 10]. | Cube x in [10, 30] |
| `overhang.ptsm` | Body "Mushroom": a 10 by 10 by 20 post centered on the origin (z 0 to 20) joined with a 30 by 30 by 5 cap (z 20 to 25); `printer` set to the default FDM profile id (print.md). | At least one `overhangs` finding |
| `oversize.ptsm` | Body "Slab": a 400 by 400 by 10 box; `printer` the default FDM profile. | A `bed_fit` error |
| `notes.txt` | The text `hello`. | Not a model |
| `sketch-500.ptsm` | One sketch with at least 500 constraints in one connected component and DOF at least 2, where dragging point `e-drag.end` moves at least 100 other entities. | Perf |
| `part-50.ptsm` | A copy of `examples/bench/fifty-features/model.ptsm` (engine.md section 5): 50 features whose first sketch is driven by parameter `base`. | Perf |
| `dense.ptsm` | At least 2,000,000 display triangles at standard quality (for example a 20 by 20 linear pattern of spheres). | Perf |
| `palette-2000.ptsm` | 1,980 parameters `p0` to `p1979` and 20 features. | Perf |

### 6.3 Helpers (`ui/e2e/helpers.ts`)

- `openApp(page, { fileSystemAccess = true } = {})`: when false, an init script deletes `window.showOpenFilePicker` and `window.showSaveFilePicker`; then `page.goto('/')` and waits for `[data-testid=app][data-core-status=ready]`.
- `dropFile(page, name)`: reads the fixture, builds a `DataTransfer` with a `File` in the page, and dispatches `dragenter`, `dragover` and `drop` on the app root; waits for `document-name` to equal the name without extension (or for a notification).
- `canvas(page)`: the canvas bounding box `{ x, y, width, height }`, its center `(cx, cy)` and `H = height`.
- `fitView(page, view)`: clicks "{View} view" on the cube, turns "Orthographic" on, clicks "Fit to view", and waits for `data-view` and `data-projection`.
- `toScreen(box, bounds, view)`: maps model millimeters to page pixels using the fit rule of 2.11 (content extents scaled to 80 percent of the limiting dimension, bounds center at the canvas center). For the cube in the front view: `s = 0.04 × H` pixels per mm, `pageX = cx + x × s`, `pageY = cy - (z - 10) × s`; the top edge of the front face is at `top = cy - 0.4 H`.
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
7. U-7. Tauri transport: three `drag_sketch` frames for document handle 1 and `f-sk1` sent while the first `invoke` is pending: `invoke` runs for the first and third only; the second resolves with a `superseded` error frame for its id.
8. U-8. Tauri transport: an `invoke` rejection with `Error('boom')` resolves the send with an error response frame of code `internal_error` for that request id.
9. U-9. `chooseTransport({ __TAURI__: { core: { invoke } } })` has kind `tauri`; `chooseTransport({})` has kind `worker`.
10. U-10. `createCoreClient`: three requests carry ids 1, 2 and 3, and the first frame's header equals `{protocol_version: 1, messages: [{kind: 'request', id: 1, command}], buffers: []}`.
11. U-11. A response frame `[document_changed, recomputed, response ok]`: listeners see both events, in order, before the request promise resolves.
12. U-12. An error outcome rejects with `CoreError` carrying the code, the message verbatim and `editIndex`.
13. U-13. A response with another id rejects with `CoreFatalError` and notifies fatal listeners.
14. U-14. An event of unknown type is ignored; a listener that throws does not stop the next listener or the request.
15. U-15. `createMutationQueue`: the second task starts only after the first settles; a rejected task does not block the next; `pending` counts queued and running tasks.
16. U-16. `createLatestOnly`: submitting a, b and c while a is in flight sends a then c (never b); `onResult` runs once, for c; `cancel()` drops a waiting input and suppresses the in-flight result; `flush()` sends the waiting input and resolves after its `onResult`.
17. U-17. `decodeEvent` of a `body_mesh` returns typed arrays whose `buffer` is the frame's `ArrayBuffer`; an out-of-range buffer index throws `CoreFatalError`; an unknown event type returns null.
18. U-18. The store ignores a `document_changed` whose revision is lower than the stored one and applies one with an equal revision.
19. U-19. `generateId('f', () => false, () => 0x3fa91c0e72b4)` is `f-3fa91c0e72b4`; `generateId('b', () => false, () => 1)` is `b-000000000001`; a predicate that rejects the first candidate gets the second; 64 rejections throw "Could not generate a free id after 64 attempts."
20. U-20. `featureLabel`: the second unnamed extrude is "Extrude 2"; a named record shows its name; suppressed records count; an unknown type uses the type name. `nextBodyName` with bodies "Body 1" and "Body 3" is "Body 2".

Commands, shortcuts and palette:

21. U-21. `formatShortcut`: `Mod+Shift+S` is `⇧⌘S` on macOS and `Ctrl+Shift+S` on Linux; `Mod+Enter` is `⌘↩` and `Ctrl+Enter`; `Delete` is `⌦` and `Del`; `F6` is `F6`. `parseShortcut` throws `Invalid shortcut "Shift+".`, and likewise for `Shift+Mod+S`, `Mod+Mod+S` and `Mod+Ctrl+S`.
22. U-22. The built-in registry plus the test catalog has no `conflicts()` on macOS, Windows or Linux.
23. U-23. `chordFromEvent`: `{key: 's', metaKey: true, shiftKey: true}` on macOS gives `Mod+Shift+S`; the same with `ctrlKey` on Linux; `{key: 'д', code: 'KeyL'}` gives `L`; a lone Shift gives null; `{key: ' '}` gives `Space`.
24. U-24. Dispatcher: `e` with an `<input>` target runs nothing; `e` with `document.body` and a sketch region in the state (Extrude is enabled, 2.10) runs `feature.extrude`; `Mod+S` in an input runs `file.save`; `Mod+Z` in an input runs nothing; `defaultPrevented` and `repeat` events run nothing; with the palette open only `Mod+K` runs.
25. U-25. Registering a duplicate id throws "Command x is already registered."; running a disabled command calls no `run` and shows an info notification with its reason.
26. U-26. `fuzzyScore('ext', 'Extrude')` is 18.93, `('ext', 'Export 3MF')` 14.9, `('ext', 'Text')` 10.96 (within 1e-9), `('xyz', 'Extrude')` null; `rankPaletteItems('ext', items, [], 'model')` orders Extrude, Export 3MF, Text; query `pad` matches Extrude through its tagged keyword `{word: 'Pad', app: 'FreeCAD'}` with score 16.97.
27. U-27. Equal scores order by `recentIds`, then title. An empty query lists up to 5 recent items, then every command and setting item in the context's category order (2.10), with no feature, body, parameter or recent-file items; in Sketch context the first non-recent item has category Sketch. Non-empty results never exceed 50.
28. U-28. `paletteItems` in Model context with `cube.ptsm` state: contains "Extrude 1" (feature), "Cube" (body), "Theme: Dark" (setting, `checked` false under the light theme) and no sketch tools; in sketch mode it contains "Line".
29. U-29. The catalog commands: one `feature.<type>` per entry, title the manifest label, `extrude` with shortcut `E`, keywords including `{word: 'Pad', app: 'FreeCAD'}`, the description of `FEATURE_DESCRIPTIONS`, and an unknown icon name falling back to `Shapes`.

Inspector:

30. U-30. The catalog store maps `feature_catalog` entries to `Catalog` with `byType` keyed by type and manifests passed through unchanged; an entry whose `params_schema` is not an object is left out.
31. U-31. `buildInspectorModel` with the extrude test catalog entry (schema and manifest of features/extrude.md) and params `{extent: {kind: 'distance', expr: '30 mm'}, op: 'new'}`: essential paths `profile`, `extent.kind`, `extent.expr`, `reverse`, `op`; more paths `body`; `extent.opposite_expr` absent; controls: `profile` selection `sketch_region` multiple, `extent.kind` segmented choice with 4 options, `extent.expr` expression length, `reverse` toggle with `attachedTo` `extent.expr`, `op` segmented choice, `body` selection `body` not multiple. With `kind: 'two_sides'` the `extent.opposite_expr` field appears and `reverse` is attached to `extent.expr`; with `through_all`, `extent.expr` disappears and `reverse` is attached to `extent.kind`; with `symmetric` there is no `reverse` field. For Revolve with `extent.kind` `full` there is no `reverse` field.
32. U-32. `complete` is false while `profile` is missing and true once set; `forceMore` is true with `errors = {body: '...'}`.
33. U-33. `defaultParams` for extrude, with a context whose `document` has no extrude: `extent` `{kind: 'distance', expr: '10 mm'}`; `op` `new` without bodies with geometry; `op` `join` and `body` equal to `lastTargetBody` with them.
34. U-34. `setParam(..., 'extent.kind', 'two_sides')` keeps `expr` and adds `opposite_expr` `10 mm`; an unknown key `taper_angle` in params is preserved by every `setParam`.
35. U-35. `selectionValue`: origin XY for `plane` gives `{kind: 'origin', plane: 'XY'}`; a flat face gives `{kind: 'face', body, face: reference}`; origin Z for `axis` gives `{kind: 'origin', axis: 'Z'}`; a sketch line gives `{kind: 'sketch_line', sketch, line}`; two regions of `f-sk1` give a `ProfileRef` with both regions in pick order; edges with `multiple` give an array of references.
36. U-36. `acceptsTarget('plane', face)` is true for `surface: 'plane'` and false for `cylinder`; `acceptsTarget('axis', edge)` is true for `curve: 'line'` and false for `circle`; a cylindrical face is accepted by `axis`.
37. U-37. `placeInspector` with bounds (0, 0, 1000, 600), panel 320 by 400: anchor (100, 100, 50, 50) gives (166, 100); anchor (700, 100, 200, 50) gives (364, 100); anchor (100, 100, 800, 50) gives (16, 16); anchor (100, 500, 50, 50) gives y 184; null gives (16, 16).

Expression input:

38. U-38. `convertForField`: `{value: 2, dimension: {length: 0, angle: 0}}` for a length in inches is 50.8; 90 dimensionless for an angle is π/2; 3.0000000001 for a count is 3; 2.5 for a count with label Count gives "Count must be a whole number, but it is 2.5. Use 2 or 3."; an angle for a length labeled Distance with source `10 deg` gives "Distance must be a length, but '10 deg' is an angle. Use a value such as 10 mm."
39. U-39. Formatting: 12.5 mm "12.5 mm"; 2.4 mm in an inch document "0.0945 in"; 1/3 mm "0.333 mm"; -0 "0 mm"; 1234.5 "1234.5 mm"; 25.4 mm in cm "2.54 cm"; 1 mm in m "0.001 m"; π/4 rad "45°"; `formatSize` of (0, 0, 0) to (40, 20, 10) "40 × 20 × 10 mm"; `formatPoint` (10, 0, 10) "10, 0, 10 mm".
40. U-40. `stepExpression`: `10` up gives `11`; `10 mm` up gives `11 mm`; `10mm` up gives `11mm`; `2.5` up coarse gives `12.5`; `2.5` up fine gives `2.6`; `-0.1` up fine gives `0`; `wall` gives null; a count `0` down gives `0`; an angle `45 deg` up gives `46 deg`.
41. U-41. `isPlainNumber` is true for `10`, `-2.5`, `10mm`, `10 mm`, `45 deg` and false for `2*5`, `wall`, `10 mm + 1` and the empty string.
42. U-42. `ExpressionInput` with a fake `evaluate`: typing `2 * 5` shows `= 10 mm`; typing `10` shows `mm`; a rejected evaluation shows the core message in `field-error`; when the result for `1` arrives after the result for `12`, the input shows the result for `12`.
43. U-43. Enter calls `onCommit` only in the valid state; Esc calls `onCancel`; ArrowUp on `10` changes the value to `11`.

Viewport math and picking:

44. U-44. `standardViewRotation` forward and up vectors match 2.11 for all seven views; `classifyView` returns each view for its own rotation and `custom` after a 1e-3 rad perturbation.
45. U-45. `fitBounds` orthographic, front view, bounds (-10, -10, 0) to (10, 10, 20): aspect 2 gives `orthographicHeight` 25 and target (0, 0, 10); aspect 0.5 gives 50.
46. U-46. `fitBounds` perspective, top view, bounds (0, 0, 0) to (40, 20, 0), aspect 1.6: `distance` equals `31.25 / (2 tan 15°)` within 1e-9.
47. U-47. `zoomAt` keeps the world point under the cursor at the same NDC position within 1e-9 for both projections; distance and orthographicHeight clamp to [0.01, 1e7].
48. U-48. `withProjection` round trip keeps `visibleHeightAtTarget`; `orbit(c, 100, 0, target)` rotates about world Z by -40°; `pan` moves the target by exactly the pixel delta times mm per pixel; `wheelZoomFactor({deltaY: 100, deltaMode: 0, ctrlKey: false}, 600)` is 1.0015^100, with `deltaMode: 1, deltaY: 3` it is 1.0015^48, and with `ctrlKey` 1.01^100.
49. U-49. `encodePickId` and `decodePickId` round-trip 1, 255, 256, 65536, 16777216 and 4294967295; `choosePick` prefers a vertex at 3 pixels over an edge at 1 pixel over a face at 0 pixels; with vertices not accepted it picks the edge; an edge at 5 pixels with radius 4 loses to the face; a face at 1 pixel is ignored; a sketch curve and an origin axis both at 0 pixels give the sketch curve.

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
59. U-59. `dimensionTypeFor` with points (0, 0) and (10, 5): label (5, 10) `horizontal_distance`, (15, 2) `vertical_distance`, (12, 8) `distance`; points (0, 0) and (10, 0) with label (5, 5) `distance`; a circle `diameter`; an arc `radius`; lines at 90° `angle`; parallel lines `distance`; a point and a circle unsupported with "Partsmith cannot dimension a point and a circle. Pick two points, a point and a line, two lines, or one circle or arc."
60. U-60. `measuredValue('angle', ...)` for a line along +X and a line at 60° through the origin is π/3 with the label at (cos 30°, sin 30°) and 2π/3 with the label at (-1, 0.5).
61. U-61. `commitSketchChange` with a fake core: a requested id reported conflicting returns `over_constrained` and calls no `applyEdits`; an inferred id reported redundant is removed, the sketch is solved again, and `applyEdits` gets the params without it and with solved geometry; a solution with `failed` true returns `failed` with the solver message, and an `invalid_params` `CoreError` returns `failed` with the core's message.
62. U-62. Sketch view history: enter (view entry), orbit (view entry), commit an edit (document marker); three `Mod+Z` presses restore the orbit camera, call core `undo` once, and restore the pre-entry camera, in that order; `Mod+Shift+Z` replays them in reverse.
63. U-63. Sketch header mapping: no entities `empty`; `under_constrained` with dof 3 `under` and "Not fully constrained · 3 DOF"; `fully_constrained` `full`; status `conflicting` with 2 conflicting ids `conflict` and "2 conflicting constraints"; `failed` true with no conflicting ids `failed` and "Cannot solve"; status `redundant` with dof 0 and one redundant id `full` with " · 1 extra constraint" appended; two `open_endpoint` findings with dof 0: `full`, `data-outline` 2 and " · 2 open ends" appended; an `open_endpoint` and an `overlap`: " · 2 outline problems".

Store and actions:

64. U-64. `startFeatureCommand('extrude')` with a preselected region and no bodies: `beginTransaction(documentHandle, 'Extrude')`, then one `applyEdits(documentHandle, null, [add_body "Body 1", add_feature])` with the body id in the record, then `setFocus(documentHandle, featureId)`.
65. U-65. With `rollback` set to the second of 3 features (index 1), the first preview's `add_feature` has index 1 and no `set_rollback` follows.
66. U-66. `confirmCommand` calls `commitTransaction`; `cancelCommand` calls `cancelTransaction`; with an empty command history, `Mod+Z` cancels the command and calls no `undo`; after two picks, `Mod+Z` removes the second pick and calls neither `cancelTransaction` nor `undo`, and `Mod+Shift+Z` restores it.
67. U-67. `editFeature('f-ext1')` calls `beginTransaction(documentHandle, 'Edit Extrude 1')` and `setFocus(documentHandle, 'f-ext1')`; a `focus_inactive` error on a suppressed feature calls `cancelTransaction` and shows an error notification with the action "Unsuppress".
68. U-68. Timeline edits: deleting the rollback feature sends `set_rollback` to the nearest later remaining feature (no id when none) before `remove_feature`; dropping feature index 3 at gap 1 sends `move_feature` index 1, at gap 4 index 3 (no edit); `setRollback(N)` sends `set_rollback` without an id, `setRollback(2)` the id of the third feature and `setRollback(0)` the id of the first.
69. U-69. Esc ladder: with an open popover, a sketch drawing in progress, an active tool, a command and a selection, successive Esc presses close the popover, cancel the drawing, exit the tool, cancel the command, then clear the selection.
70. U-70. Contrast: for every row of both tables in 2.16.6 and each theme, `contrastRatio` of the two roles' hex values (`PALETTE_HEX` of the steps in 2.16.3 and 2.16.4; `warningMark` is `zinc-950` light and `yellow-400` dark) truncated to two decimals equals the table's value and is at least 4.5 for text rows and 3 for non-text rows; `contrastRatio('#ffffff', '#000000')` is 21.
71. U-71. `resolveTheme('system', true)` is `dark` and `resolveTheme('light', true)` is `light`; `applyTheme(root, 'dark')` adds the class `dark` and `applyTheme(root, 'light')` removes it, leaving the root's other classes untouched; `applyUiScale(root, 1.5)` leaves exactly one scale class, `text-[150%]`, after `applyUiScale(root, 2)`; `loadSettings` of malformed JSON returns `DEFAULT_SETTINGS`, and of `{"theme": "dark", "uiScale": 7}` returns theme dark with scale 1.
72. U-72. Notifications with fake timers: info disappears after 5 s, warnings after 8 s, errors stay; a fourth notification removes the oldest non-error; a duplicate message restarts the existing timer instead of adding one.
73. U-73. Every message id of 4.3 renders exactly its text for sample placeholders (table-driven).
74. U-74. Web file service with mocked File System Access: `pickToOpen` returns name, bytes and handle; an `AbortError` returns null; `chooseSaveTarget` returns a handle target; `write` calls `requestPermission` when `queryPermission` is `prompt` and throws `FileServiceError` `permission` when it is denied; a `NotFoundError` becomes `not_found`.
75. U-75. Without the API: `chooseSaveTarget` returns `{kind: 'download', name}`; `write` clicks an anchor whose `download` is the name and revokes its object URL; `pickToOpen` resolves from the hidden input's `change` event.
76. U-76. `createMemoryRecentFiles`: at most 10 entries, newest first; a text entry with an existing name replaces it; a handle entry whose handle `isSameEntry` an existing one replaces it.
77. U-77. Recovery: a dirty `document_changed` writes an entry after 2 s (fake timers); another change within 2 s postpones it; a successful save clears the store.
78. U-78. Save as canceled at the picker never calls `saveDocument`; Save to a handle whose write fails with `permission` calls no `markSaved`, leaves `data-dirty` true and shows `save_permission` with the action "Save as".
79. U-79. `openFile`: `model.stl` shows `not_ptsm`; invalid UTF-8 shows `not_text`; a `parse_error` shows `open_failed` with the core message and keeps the current document; with a dirty current document the stash is written before `closeDocument`.
80. U-80. `exportModel` with no visible bodies shows `nothing_to_export` and calls no `exportBodies`; formats map to `{"kind": "3mf"}`, `{"kind": "stl", "binary": true, "per_body": false}`, `{"kind": "obj"}` and `{"kind": "step"}`; the suggested name is `{document name}.3mf`.
81. U-81. Print: badge states derive as in 3.17, hidden checks are excluded from counts, a report for an older revision is stale; in Print context a revision change schedules `analyzePrint` after 300 ms (fake timers) and a `null` result changes nothing; in Model context no analysis is scheduled.
82. U-82. After a mesh change, a selected face whose reference resolves `missing` leaves the selection and one that resolves is kept.
83. U-83. Rendering `App` with fakes produces the landmarks banner, navigation "Browser", main "3D view", region "Timeline" and contentinfo, and every `button` has a non-empty accessible name.
84. U-84. `app.restartCore()` with fake transports: the new core receives `open_document` with `JSON.stringify(recipe, null, 2)`, the document keeps its name and handle, and `localUnsaved` is true.
85. U-85. Committing a new name in the Parameters panel calls `renameParameter`, which sends `rename_parameter` and no `update_parameter`.
86. U-86. Save to a handle calls `saveDocument`, `files.write`, then `markSaved` with the returned revision, in that order.
87. U-87. During an edit, a status message `{severity: 'error', code: 'fillet_too_large', field: '/radius', suggestion: '2.3 mm'}` shows under the `radius` field and its action "Use 2.3 mm" sets `radius` to `2.3 mm`; a message with `field: '/edges/1'` maps to the `edges` field.
88. U-88. Double-click on a face whose descriptor has `feature: 'f-ext1'` calls `editFeature('f-ext1')`.
89. U-89. With an active feature command, the export commands are disabled with `finish_command_first`; the timeline item of an `import` feature offers Rename, Suppress and Delete, and its Edit is disabled with `import_not_editable`.

Theme, styling and cross-spec helpers:

90. U-90. Mapping table and viewport values agree: (1) every `PALETTE_HEX` value equals the conversion of 2.16.5 (CSS Color 4 OKLCH to linear sRGB, per-channel clip, sRGB encoding, 8-bit rounding) of that step's OKLCH string from `tailwindcss/colors` (`white` from `#fff`); (2) `COLOR_CLASSES` equals the table of 2.16.3, held in the test as data (role, facet, class string), and every color class in it names a step of `PALETTE_HEX`; (3) `VIEWPORT_STEPS` equals the table of 2.16.4 (including `grid`), and `VIEWPORT_COLORS[theme][role]` equals both `PALETTE_HEX[VIEWPORT_STEPS[role][theme]]` and the table's hex; (4) `highlight` has the steps of `accent`'s background, `problem` and `sketchOver` those of `errorMark`'s foreground, `body` the Default swatch's step, and the `analysis` classes name the steps of `analysisLow` and `analysisHigh`; (5) `BODY_SWATCHES` equals the swatch table of 2.16.5, each `background` is `bg-{step}` and each non-null `hex` is `PALETTE_HEX[step]`; (6) `viewportTheme('dark').highlight` equals `linearFromHex('#ff6900')` within 1e-12; `FONT_CLASSES`, `UI_SCALE_CLASSES` and `ROOT_CLASSES` equal 2.16.7, 2.16.9 and 2.16.2.
91. U-91. Styles in source: `src/index.css` equals the block of 2.16.1 byte for byte and is the only `.css` file under `src/`; no `.tsx` file under `src/` has a `style` JSX attribute; outside `src/viewport/overlay.ts` no file under `src/` (excluding `src/generated`) contains `.style.` or `.style[`; no string under `src/` contains `px]` or `[#`; rendering `App` with fakes (no viewport controller) produces no element with a `style` attribute; `positionElement(element, 12.5, 40)` leaves a `style` attribute that declares only `translate: 12.5px 40px` and sets `data-positioned`.
92. U-92. `utf16Range('2 * Ø + x', ...)`: span 9..10 gives 8..9, 4..6 gives 4..5, 0..10 gives 0..9, an offset of 5 (inside `Ø`) gives null and an end of 11 gives null; for `a😀b` span 5..6 gives 3..4. `ExpressionInput` with a fake `evaluate` that rejects `2 * Ø + x` with a `CoreError` of span 9..10 shows the message in `field-error`, and Enter selects the range 8..9 of the input.
93. U-93. Feature highlighting with the `cube.ptsm` state and a fake core: selecting the timeline item `f-ext1` calls `listElements(documentHandle, 'b-cube', 'face')` once and gives the selected state to exactly the faces whose descriptor has `feature` `f-ext1`, at their positions in the list; selecting it again with the same mesh id sends no request; a `body_mesh` with a new mesh id for `b-cube` arriving before the response discards it and requests again; selecting `f-sk1` marks no face.
94. U-94. Theme toggle: with `settings.theme` `system`, a light OS and a fake `Storage`, clicking `theme-toggle` sets `settings.theme` to `dark`, adds the class `dark` to the root element and stores JSON with `"theme":"dark"` under `partsmith.settings.v1`; clicking again sets `light` and removes the class; `toggledTheme('dark')` is `light`.

One interface for everyone (open issue 15):

95. U-95. `selectEmptyState` is true for a ready core with a recipe without features, no command and no sketch session. It is false with one feature, with an active command, during plane picking, and while the core is loading.
96. U-96. `selectOriginVisible`: with no features, `origin:X` and `origin:O` are visible and `origin:XY` is not; with one feature, `origin:X` is hidden; `sessionVisible` `{'origin:X': true}` shows it with features, and `{'origin:Z': false}` hides it with none.
97. U-97. `gestureFor('fusion', 2, {shift: false})` and `gestureFor('freecad', 2, {shift: false})` are `orbit`; with `shift: true` both are `pan`; `gestureFor('freecad', 6, {shift: false})` is `orbit`; `gestureFor('fusion', 1, {shift: false})` is `none`; `navigationLegend` returns exactly the two strings of 2.11; every `NAVIGATION_PRESETS` entry has one row per row of the 3.13 table with that preset's column.
98. U-98. Availability with the test catalog: in a new document, `feature.extrude` is disabled with "Draw a closed shape in a sketch first.", `feature.fillet` with "Make a solid first, for example with Extrude or Box.", and the toolbar group "Pattern" renders `aria-disabled` with that reason; `feature.box` and `feature.sketch` are enabled. With the `rect-sketch.ptsm` state, extrude and revolve are enabled and fillet is disabled. With `cube.ptsm`, fillet is enabled and boolean is disabled with "Boolean needs two bodies with geometry. Make a second body first, for example with Box."; with `two-bodies.ptsm`, boolean is enabled. With `cube-features.ptsm` and "Extrude 1" suppressed, `feature.fillet` is disabled and `editFeature('f-fil1')` still calls `beginTransaction`.
99. U-99. With a fake core: finishing a sketch whose solution has one region selects `region:{sketch}:{key}`; two regions select `feature:{sketch}`; no region with point entities `e-p1` and `e-p2` (construction) selects only `point:{sketch}:e-p1`. `startFeatureCommand('hole')` run from Sketch with point entities `e-p1` and `e-p2` preselects points `['e-p1', 'e-p2']`; with only circle `e-5`, it preselects `['e-5.center']`. With the browser row of `f-sk1` (one region) selected, `startFeatureCommand('extrude')` sets `profile` to that region; with a sketch of two regions selected, `profile` stays empty and is the active field.
100. U-100. `capturePick` in a Hole command with a flat-face item (placement origin (0, 0, 20), x axis (1, 0, 0), normal (0, 0, 1)) and a `PointerRay` hitting (3, 4, 20), far from the centroid: the command gets a helper sketch with plane kind `face` and the descriptor's body, reference and placement, and one point entity at [3, 4]; `points` is its `PointRef`, `sketch` the helper id and `body` the face's body. A second click at (-2, 1, 20) adds a second entity. A click on another face leaves the params unchanged and shows `hole_one_face`. With `points` holding points of `f-sk2`, a face click replaces them with the helper's point. A ray within 8 times `millimetersPerPixel` of the centroid places [0, 0].
101. U-101. Hole preview edits with a fake core: the first application sends, in one `applyEdits`, `add_feature` of the helper at index n and `add_feature` of the hole at index n + 1; the next sends `update_feature` of both; after Points is replaced by points of `f-sk2`, the next sends `remove_feature` of the helper; `cancelCommand` calls `cancelTransaction` and nothing else.
102. U-102. `sketchStepHint` on Linux: `{kind: 'idle', empty: true}` gives "Choose Line (L), Rectangle (R) or Circle (C) to start drawing."; `{kind: 'idle', empty: false}` gives null; line before a click "Click to start a line."; line with a chain "Click to end the segment, or type a length. Click the first point to close the shape, or press Enter to stop." (on macOS it ends "press ↩ to stop."); circle after click 1 "Click to set the size, or type the diameter."; dimension with a point picked "Click a second point or a line."; with a circle picked "Click to place the dimension.". `sketch_free_placement` renders "Hold Ctrl to place freely." on Linux and "Hold ⌘ to place freely." on macOS.
103. U-103. `defaultParams` with the test catalog in a document without features: fillet `radius` `1 mm`; chamfer `distance` `{kind: 'equal', expr: '1 mm'}`; shell `thickness` `2 mm`; hole `diameter` `3.2 mm` and `extent` `{kind: 'through_all'}`; revolve `extent` `{kind: 'full'}`; datum plane `offset` `10 mm`; extrude as in U-33. In a document whose latest fillet has `radius` `edge_r`, a new fillet starts at `edge_r`, and its `edges` and `body` do not carry over; when that latest fillet's status is `error`, the radius of the latest fillet that is not in error is used. A fillet's `body` starts empty and a hole's starts at `lastTargetBody`. `setParam` of `hole_type.kind` to `counterbore` with diameter `3.2 mm` gives `hole_type` `{kind: 'counterbore', diameter: '6.4 mm', depth: '3 mm'}`; with diameter `screw` the head diameter is `2 * (screw)`; `countersink` gives angle `90 deg`; `setParam` never reads the document.
104. U-104. With a Fillet command active (edges set) and the viewport focused, the keys `2` and Enter make `confirmCommand` commit params with `radius` `2`. With Extrude active, `.5` gives `extent.expr` `.5`; after the user edited Opposite distance, `3` goes there. `e` types nothing and stays a shortcut. Tab on the viewport focuses the first essential expression field with its text selected. Fillet started with one preselected edge has `activeField` `edges` and focus in Radius. In Revolve, a profile pick advances the active field to `axis`.
105. U-105. Split body with `body` `b-cube` and `plane` origin XZ preselected: the first `applyEdits` is `[add_body "Body 1", add_feature]` with `target` the new body's id; clearing Target body makes the next application send `update_feature` without `target`, and `remove_body`. Mirror with op `join` has no `target` field in the model; setting op `new` shows it as "Body 1 (new)", and the first application adds that body; setting op back to `join` removes it. Extrude with op `new` still creates its body in `body`.
106. U-106. Extrude whose profile sketch is on a face of `b-cube`, with `body` `b-cube`: `setCommandField('op', 'cut')` sets `reverse` true and `'join'` sets it false. After `setCommandField('reverse', false)` while op is cut, later op changes keep it. With the sketch on origin XY, op changes never touch `reverse`.
107. U-107. An Extrude command with `extent.expr` `-5 mm` and the status message `{severity: 'error', code: 'out_of_range', field: '/extent/expr'}` shows the action "Reverse and use 5 mm", which sets `reverse` true and `extent.expr` `5 mm`. With `extent.expr` `-wall` there is no such action, nor for a Hole whose `diameter` `-3 mm` has the message on `/diameter`. A `no_effect` warning with op `cut` shows the action "Reverse".
108. U-108. With an Extrude command active, `updateParameter(app, 'p-size', {expression: '30 mm'})` calls `applyEdits(documentHandle, null, [update_parameter])` and sets `preview` to `pending` until its events arrive; without a command, the label is "Change size". During the command, `file.save`, `edit.suppress` and `document.units.in` are disabled with "Finish or cancel Extrude first.", and `view.section` is enabled.
109. U-109. A Fillet command with two picks, then `startFeatureCommand('chamfer')`: `cancelTransaction` is called and the notification "Fillet was canceled." offers "Restore". Running Restore cancels the Chamfer command without another notification and reopens Fillet with both edges and the same active field. A command with an empty history that ends the same way shows no notification, nor does a command with picks ended by `newDocument`.
110. U-110. With a fake core: in a sketch session, an applied sketch edit after which `f-fil1` goes from ok to error raises no notification; `finishSketch` then raises one `feature_failed` for "Fillet 1" with `features` `['f-fil1']`, and a `recomputed` event with `f-fil1` ok removes it. A status with `repairs` `[{kind: 'element', path: '/edges/0', centroid: [0, 0, 20], ...}]` gives the action "Re-pick Edges", and `editFeature` of that feature renders one `repair-marker`.
111. U-111. Every registered command whose id starts with `feature.` (test catalog) or `sketch.constrain_`, every sketch tool and `sketch.finish` has a `description` of 1 to 120 characters that ends with "." and contains none of the words "omitted", "default", "params", "None" or "serde". Every `FIELD_HELP` path names a field hint of that type in the test catalog. `buildInspectorModel` gives the extrude `op` the table's help, and gives a field without an entry `help` null even when its schema has a `description`.
112. U-112. With the built-in registry, the test catalog and the `cube.ptsm` state (in Sketch context, "Sketch 1" is being edited), each query gives this first item and detail:

     | Context | Query | First item | Detail |
     | --- | --- | --- | --- |
     | Sketch | "symmetry" | `sketch.constrain_symmetric` | "Fusion: Symmetry" |
     | Sketch | "radius" | `sketch.dimension` | |
     | Sketch | "lock" | `sketch.constrain_fix` | "FreeCAD: Lock" |
     | Sketch | "polyline" | `sketch.line` | "FreeCAD: Polyline" |
     | Sketch | "close sketch" | `sketch.finish` | |
     | Sketch | "fillet" | `feature.fillet` | "Finishes the sketch" |
     | Model | "new sketch" | `feature.sketch` | |
     | Model | "isometric" | `view.home` | |
     | Model | "pad" | `feature.extrude` | "FreeCAD: Pad" |
     | Model | "press pull" | `feature.extrude` | "Fusion: Press Pull" |
     | Model | "spreadsheet" | `view.parameters` | "FreeCAD: Spreadsheet" |

113. U-113. `originPlaneName('XY')` is "Top plane (XY)", `'XZ'` is "Front plane (XZ)" and `'YZ'` is "Right plane (YZ)". With the browser row of `f-sk1` selected, `context.sketch` calls `enterSketch('f-sk1')` and its tooltip is "Edit Sketch 1"; the same holds with a region of `f-sk1` selected. With a flat face selected, it calls `startNewSketch` and its tooltip is "New sketch".
114. U-114. `suggestedCommands` gives `feature.move_body`, `feature.mirror`, `feature.split_body` for one body; `feature.boolean` for two bodies; `feature.mirror`, `feature.split_body` for a body and origin XZ; `feature.sketch`, `feature.datum_plane` for origin XY; `feature.sketch`, `feature.hole`, `feature.shell` for one flat face; `feature.extrude`, `feature.revolve` for the sketch feature of `rect-sketch.ptsm`; `feature.hole` for a sketch point in Model context. With a catalog without `split_body`, one body gives only the first two.
115. U-115. For a line with a driving `distance` on `e-1.start` and `e-1.end` with expression `width`, the size field text is `width`, and committing `width + 5` sends a `replaceConstraint` with that expression. For a circle without a dimension, the text is `formatNumber` of its diameter, and committing `12` adds a requested `diameter` constraint with expression `12`.
116. U-116. `selectionMeasurements` with fixture descriptors: a circular edge of radius 2.5 gives Diameter "5 mm"; a cylindrical face of radius 4 gives "8 mm"; a spherical face of radius 5 gives "10 mm"; a body whose `measure` returns volume 8000 gives Volume "8000 mm³", and "0.4882 in³" in an inch document; a body whose Size is "Empty" gives no Volume row and calls no `measure`; two selected vertices give no rows.
117. U-117. `changedMore`: a box with `position[0]` `20 mm` gives ["Position X"]; an extrude whose `body` came from the op rule gives []; a hole with `tip_angle` `118 deg` gives ["Tip angle"]. `forceMore` is true for a hole whose `points` are set and whose `body` is empty, and false for a fillet with no edges and no `body`. In a fillet whose first edge pick filled `body`, clearing the edges clears `body`. `pick_same_body` renders "All edges must be on 'Cube'. Remove them to pick edges of another body."
118. U-118. `ExpressionInput` with parameters `size` = `20 mm` and `slot` = `3 mm` and a fake `evaluate`: typing `si` opens `expression-suggestions` with the first option "size = 20 mm" and no active option; Tab makes the value `size` without calling `onCommit`. Typing `size` opens nothing, and Enter calls `onCommit`. Typing `si`, ArrowDown, Enter makes the value `size` without calling `onCommit`. Typing `10` and `1 in` opens nothing, and ArrowUp on `10` makes it `11`; `2 * pi` opens nothing. Esc on an open list closes it without calling `onCancel`. Typing `printer.n` lists `printer.nozzle` first for an FDM profile.
119. U-119. `exportStatus` for 3MF with `f-fil1` in error and no report gives "Fillet 1 failed, so it is not in the export." and "Print check: Not checked"; with two features in error, "2 features failed, so they are not in the export."; with no errors and a current ok report, the one line "Print check: Ready to print"; for OBJ with no errors, no line. With a face-down orientation set, 3MF adds the `orientation_set` line. An export with `f-fil1` in error shows the warning notification "Exported cube.3mf. Fillet 1 failed, so it is not in the export.".
120. U-120. `gridSpacing(0.05, 'mm')` is 1, `gridSpacing(0.5, 'mm')` is 10, `gridSpacing(5, 'mm')` is 100, and `gridSpacing(0.05, 'in')` is 2.54.
121. U-121. `printerDetail` of `generic-fdm` is "256 × 256 × 256 mm bed · 0.4 mm nozzle"; a round FDM bed of diameter 180 and height 180 gives "⌀180 × 180 mm bed · 0.4 mm nozzle". `printerButtonText` for a document without a printer is "Printer: Generic FDM 0.4 mm (default)".
122. U-122. `viewportMenuItems`: with one edge of `cube.ptsm` selected after a Fillet run, the command labels are "Repeat Fillet", "Fillet", "Chamfer", "Edit Extrude 1", "Fit to view"; with the body "Cube" selected they include "Hide"; in sketch mode with one line selected, "Horizontal", "Vertical", "Fix", "Construction", "Delete", "Finish sketch"; with a complete command active, "OK", "Cancel". `edit.repeat` before any feature command is disabled with "No command to repeat yet.".
123. U-123. `capturePick` in a Fillet command with a face whose `adjacent` names 4 edges, and a fake `listElements` returning their descriptors: `edges` becomes those 4 references, in `adjacent` order. The same click again empties it. With 2 of them already held, it adds the other 2.
124. U-124. With `print.orientation` set to a face-down orientation, `runPrintAnalysis` sends it and records it in `reportOrientation`; resetting the orientation makes the badge stale. `exportModel` passes it for 3MF and STL, and not for OBJ and STEP. An `invalid_orientation` `CoreError` clears it and shows a warning notification with the core message.
125. U-125. `COMPONENT_KEYS` has one entry per row of the 3.5 component-key table, with the same key, place and effect (table-driven).
126. U-126. `cameraAfterSketch(current, before, normal)`: with `current` looking along -Z, `before` `DEFAULT_CAMERA` and the normal (0, 0, 1), it returns `before`; with `before` also looking along -Z, it returns a camera that `classifyView` reports as `iso`, with the same target, distance and projection; with `before` null, also `iso`; with `current` 15° away from the normal, it returns null.

### 6.5 Playwright end-to-end tests

Each test starts with `openApp` unless it says otherwise. "Click X" means clicking the element with accessible name X in the region named. Positions are relative to the canvas center `(cx, cy)` in CSS pixels, with `H` the canvas height.

Startup and files:

1. E-1. Cold start: `document-name` is "Untitled" with `data-dirty="false"`; the radio "Model" is checked; the toolbar "Model tools" has, in order, buttons named Sketch, Extrude, Revolve, Hole, Fillet, Chamfer, Shell, Shapes, Pattern, Mirror, Boolean; `status-units` reads "mm"; `health-badge` has `data-state="none"`.
2. E-2. With an init script making `getContext('webgl2')` return null: `unsupported` is visible with the `unsupported_webgl2` text and the page has no worker.
3. E-3. Drop `cube.ptsm`: `document-name` "cube"; two `timeline-item`s named "Sketch 1" and "Extrude 1" with `data-status="ok"`; a `browser-row[data-kind=body]` with text "Cube"; `status-recompute` matches `^Rebuilt in \d+(\.\d)? (ms|s)$`; `data-dirty="false"`.
4. E-4. `openApp(page, {fileSystemAccess: false})`, press ControlOrMeta+O, answer the file chooser with `cube.ptsm`: same expectations as E-3.
5. E-5. Drop `notes.txt`: a `notification[data-severity=error]` reads "notes.txt is not a Partsmith model. Partsmith opens .ptsm files."; `document-name` stays "Untitled".
6. E-6. Without File System Access, drop `cube.ptsm`, press ControlOrMeta+S: a download named `cube.ptsm` whose bytes equal the fixture's bytes; an info notification with `download_hint` text.
7. E-7. Drop `cube.ptsm`; click the body row "Cube", press F2, type "Box", Enter: `save-state` reads "Edited" and `data-dirty="true"`. Press ControlOrMeta+S (no File System Access): the download contains `"name": "Box"`; `data-dirty="false"` and `save-state` is empty.
8. E-8. Without File System Access, drop `cube.ptsm`, reload, open the Document menu, open "Open recent", click "cube": `document-name` "cube" and two timeline items.
9. E-9. Drop `cube.ptsm`, rename the body to "Box", wait 3 s, register a dialog handler that records the dialog type and accepts it, reload: the recorded type is `beforeunload`; after the reload a notification starts with "Partsmith kept unsaved changes to cube from"; click "Open": a body row "Box" and `data-dirty="true"`.

Palette and shortcuts:

10. E-10. Drop `rect-sketch.ptsm` (Extrude needs a sketch region, 2.10). Press ControlOrMeta+K: `palette` visible and `palette-input` focused. Type "ext": the first `palette-option` contains "Extrude" and its `palette-shortcut` reads "E". Enter: `inspector[data-mode=command]` named "Extrude"; Escape closes it. ControlOrMeta+K, type "pad": an option "Extrude" whose detail reads "FreeCAD: Pad" is listed. With `cube.ptsm` dropped, ControlOrMeta+K, type "Cube", Enter: `data-selection-count="1"` and `selection-title` "Cube".
11. E-11. Drop `rect-sketch.ptsm`. Open the palette, run "Parameters", click "Add parameter", type "e": the name field contains "e" and no command inspector opened. Escape, click the canvas at `(cx, cy)`, press "e": the inspector "Extrude" opens.

Sketching:

12. E-12. Click "Sketch" in the toolbar: `data-context="sketch"`, `sketch-header` `data-state="picking"`. Expand "Origin" in the browser and click the row "Top plane (XY)": a timeline item "Sketch 1", `data-view="top"`, header `data-state="empty"`. Press R, move to `(cx, cy)`: an `inference-glyph[data-type=coincident]` is visible. Click there, click `(cx + 200, cy - 100)`: header `data-state="under"`, `data-dof="2"`, `status-dof` `data-dof="2"`. Press D, click `(cx + 100, cy)`, click `(cx + 100, cy + 40)`: `dimension-editor` visible; type "40", Enter: a `sketch-dimension` with text "40" and `data-dof="1"`. Click `(cx, cy - 50)`, click `(cx - 40, cy - 50)`, type "20", Enter: `data-dof="0"`, `data-state="full"`, header text contains "Fully constrained".
13. E-13. Continue from E-12 (or open `rect-sketch.ptsm` and double-click "Sketch 1"). Press E: `data-context="model"`; the inspector "Extrude"; the `selection-field` in `inspector-field[data-path=profile]` has `data-count="1"`; the Distance input is focused. Type "10": `expression-result` reads "mm". Enter: timeline items "Sketch 1" and "Extrude 1", both `data-status="ok"`; a body row "Body 1"; clicking it shows `body-size` "40 × 20 × 10 mm" and `body-center` "20, 10, 5 mm".
14. E-14. Drop `rect-sketch.ptsm`, `fitView(page, 'Top')`, press E, click `(cx, cy)`: the profile field has `data-count="1"`. Fill Distance with "10 deg": `field-error` visible containing "must be a length", `inspector-ok` `aria-disabled="true"`. Fill "2 * 5": `expression-result` "= 10 mm", no `field-error`, OK enabled.
15. E-15. New sketch on XY (as in E-12). Press L, click `(cx, cy)`, move to `(cx + 200, cy - 10)` (2.9 degrees, outside the 6-pixel axis snap): `inference-glyph[data-type=horizontal]` visible. Click, Escape, Escape: `data-dof="1"`. Press L, hold Control, click `(cx, cy - 100)`, move to `(cx + 200, cy - 103)`: no `inference-glyph`; click, release Control, Escape, Escape: `data-dof="5"`.
16. E-16. New sketch on XY. Press L, click `(cx, cy)`, move to `(cx + 150, cy)`, type "25": `draw-input` named "Length" has value "25". Enter: a `sketch-dimension` with text "25". Escape, Escape: `data-dof="0"`, `data-state="full"`.
17. E-17. Draw the horizontal line of E-15 and leave the tool. Click `(cx + 100, cy)`: `selection-title` "1 line" and a button "Vertical". Press V: a `notification[data-severity=warning]` reads "Vertical conflicts with Horizontal, so it was not added."; `data-dof="1"`. Click "Replace them": one `constraint-glyph[data-type=vertical]`, none of type horizontal, `data-dof="1"`.
18. E-18. Drop `rect-sketch.ptsm`, double-click the timeline item "Sketch 1": `data-state="full"`. With `s = sketchToScreen` for content (0, 0) to (40, 20): press D, click the top line at `(cx, cy - 10 s)`, click `(cx, cy - 10 s - 30)`, type "40", Enter: the editor shows the `dimension_over_constrained` text in a `role=alert`. Click "Add reference": a new `sketch-dimension` with `data-driving="false"` and text "(40)".
19. E-19. New sketch on XY. Holding Control, draw line A from `(cx - 150, cy + 60)` to `(cx + 150, cy + 60)` and line B from `(cx - 100, cy - 60)` to `(cx + 100, cy - 140)`, leaving the tool after each: `data-dof="8"`. Click A at `(cx, cy + 60)`, Shift+click B at `(cx, cy - 100)`: the selection inspector has buttons Parallel, Perpendicular, Collinear, Equal. Click "Perpendicular": `data-dof="7"` and one `constraint-glyph[data-type=perpendicular]`.
20. E-20. Drop `underconstrained.ptsm`, double-click "Sketch 1": `data-dof="7"`. With the mapping for content (0, -5) to (55, 5), the line end (30, 0) is a `sketch_point` on hover. Drag it in 10 steps to (30, 10) mm and release: `data-dof="7"`; hovering the new position gives `sketch_point`, the old one does not. ControlOrMeta+Z: hovering the old position gives `sketch_point` again.
21. E-21. Drop `rect-sketch.ptsm`, enter the sketch. Drag on empty space from 30 pixels above and left of the rectangle's top-left corner to 30 pixels below and right of its bottom-right corner: `selection-title` "4 lines". Drag from right to left across only the bottom line's middle: `selection-title` "1 line".
22. E-22. In sketch mode press ControlOrMeta+Enter: `data-context="model"`; the browser shows a button "Hide Sketch 1" (the sketch stays visible).

Inspector, timeline and browser:

23. E-23. Drop `parametric.ptsm`; click the body row "Cube": `body-size` "20 × 20 × 20 mm". Run "Parameters"; fill "Expression of size" with "30 mm", Enter: `parameter-value` "30 mm" and `body-size` "30 × 30 × 30 mm". ControlOrMeta+Z: "20 × 20 × 20 mm".
24. E-24. Drop `cube-features.ptsm`; drag `timeline-marker` to the gap between the first and second items: items 2 to 4 have `data-rolled-back="true"`, the marker's `aria-valuenow` is 1, the body row reads "Cube (empty)", and hovering `(cx, cy)` does not give `face`. ControlOrMeta+Z: `aria-valuenow` 4 and no rolled-back items.
25. E-25. Drop `cube-features.ptsm`; focus the marker; ArrowLeft: `aria-valuenow` 3; End: 4 with `aria-valuetext` "At the end"; Home: 0 with `aria-valuetext` "At the start", and every `timeline-item` has `data-rolled-back="true"`.
26. E-26. Drop `two-bodies.ptsm`; drag "Cylinder 1" to the first gap: item names in order "Cylinder 1", "Sketch 1", "Extrude 1", all ok. Drag "Extrude 1" to the gap between "Cylinder 1" and "Sketch 1": "Extrude 1" has `data-status="error"` and an error notification starts with "Extrude 1 failed."; click its "Undo": the order is "Cylinder 1", "Sketch 1", "Extrude 1" and "Extrude 1" is ok.
27. E-27. Drop `cube-features.ptsm`; right-click "Fillet 1", choose "Suppress": `data-suppressed="true"`, `data-status="suppressed"`. ControlOrMeta+Z: `data-suppressed="false"`.
28. E-28. Drop `fillet-error.ptsm`: "Fillet 1" has `data-status="error"`; `timeline-problems` reads "1 problem"; clicking it opens `problems-panel` listing "Fillet 1" with a message containing "Fillet radius"; clicking "Edit" opens the inspector "Fillet 1" whose `inspector-error` has `role=alert`.
29. E-29. Drop `cube.ptsm`; double-click "Extrude 1": the inspector "Extrude 1" with the Distance input value "20 mm". Fill "30" and wait for `data-preview="shown"`; Escape: the inspector closes and `body-size` (select the body) is "20 × 20 × 20 mm". Double-click "Extrude 1", fill "30", Enter: "20 × 20 × 30 mm". ControlOrMeta+Z: "20 × 20 × 20 mm".
30. E-30. Drop `cube.ptsm`; press E: visible `inspector-field`s have paths `profile`, `extent.kind`, `extent.expr`, `reverse`, `op`; `inspector-more` has `aria-expanded="false"`. Click it: `body` appears. Escape, press E: `aria-expanded="true"`.
31. E-31. Drop `cube.ptsm`, `fitView(page, 'Front')`, press F: the `selection-field` of the edges field (features/fillet.md manifest) has `data-active="true"` (the body field is skipped, 3.7 step 5). Click `(cx, cy - 0.4 H)` (the top front edge): its `data-count="1"`; click again: 0; click again: 1. Fill the radius field (features/fillet.md manifest) with "2", Enter: "Fillet 1" ok.
32. E-32. Drop `cube.ptsm`, front fit, click `(cx, cy - 0.4 H)`: `selection-title` "1 edge". Press F: the edges field has `data-count="1"`.
33. E-33. Drop `cube.ptsm`, front fit: hover `(cx, cy)` gives `face`, `(cx, cy - 0.4 H)` gives `edge`, `(cx + 0.4 H, cy - 0.4 H)` gives `vertex`. In "Selection filter" uncheck Edges and Vertices: `(cx, cy - 0.4 H + 2)` gives `face` and `(cx + 0.4 H - 2, cy - 0.4 H + 2)` gives `face`.
34. E-34. Drop `cube.ptsm`, front fit: click `(cx, cy)`: `selection-title` "1 face". Shift+click `(cx, cy - 0.4 H)`: "2 items". Click `(cx, cy + 0.45 H)`: `data-selection-count="0"` and no inspector.
35. E-35. Drop `cube.ptsm`, front fit, double-click `(cx, cy)`: the inspector "Extrude 1".
36. E-36. Drop `cube.ptsm`, front fit. Click "Hide Cube": hover `(cx, cy)` gives `none`. Click "Show Cube": `face`. Click "Lock Cube": `none`. Click "Unlock Cube": `face`.
37. E-37. Without File System Access, drop `cube.ptsm`; click "Color of Cube", click "Red" in `color-popover`; ControlOrMeta+S: the download contains `"color": "#e7000b"`.
38. E-38. Drop `cube.ptsm`; press Tab until a `timeline-item` has focus (at most 30 presses): it is named "Sketch 1". ArrowRight: "Extrude 1" has focus. Enter: the inspector "Extrude 1" opens with focus inside it. Escape: it closes and "Extrude 1" has focus again.
39. E-39. Drop `cube.ptsm`; rename the body to "Box"; ControlOrMeta+Z: the body row reads "Cube"; ControlOrMeta+Shift+Z: "Box". Run "Parameters", click "Add parameter", type "abc", press ControlOrMeta+Z: the body row still reads "Box".
40. E-40. Drop `cube.ptsm`: every visible `button` and `[role=button]` has a non-empty accessible name (`aria-label`, `aria-labelledby` text, or text content).

Viewport:

41. E-41. Drop `cube.ptsm`. Click "Top view": `data-view="top"`; "Front view": `front`; "Front right top corner view": `iso`; "Home view": `iso`. Shift+middle-drag 100 pixels on the canvas: `custom`.
42. E-42. Click "Orthographic": `aria-pressed="true"` and `data-projection="orthographic"`. Press Shift+P: `perspective`.
43. E-43. Drop `cube.ptsm`, front fit. Middle-drag from `(cx, cy)` to `(cx + 0.6 H, cy)`: `data-camera-target` changed, hover `(cx, cy)` gives `none` and `(cx + 0.6 H, cy)` gives `face`. Press F6, then five wheel events of `deltaY: -100` at `(cx, cy)`: hover `(cx, cy + 0.45 H)` gives `face`.
44. E-44. Run "Navigation: FreeCAD": `data-nav-preset="freecad"`. Front fit; press the middle button, then the left button, move 100 pixels, release both: `data-view="custom"`. Click "Front view"; middle-drag 100 pixels: `data-view` stays `front` and `data-camera-target` changes.
45. E-45. Drop `cube.ptsm`, front fit, press Shift+X: `section-popover` visible; fill Offset with "10", Enter: `data-section="on"`; hover `(cx, cy - 0.2 H)` gives `none` and `(cx, cy + 0.2 H)` gives `face`. Shift+X: `data-section="off"` and `(cx, cy - 0.2 H)` gives `face`.
46. E-46. Drop `cube.ptsm`, front fit; ten wheel events of `deltaY: 100`: hover `(cx, cy - 0.39 H)` gives `none`. Press F6: `(cx, cy - 0.38 H)` gives `face` and `(cx, cy - 0.42 H)` gives `none`.

Print and export:

47. E-47. Drop `overhang.ptsm`; click the radio "Print": `data-context="print"`; the toolbar "Print tools" has buttons starting "Printer:", "Overhang map" (`aria-pressed="true"`), "Checks", "Print check", "Lay on face", "Export 3MF", "Export". `health-badge` reaches `data-state` `warning` or `error` with `data-stale="false"`; `overhang-legend` is visible and contains "°". Click the badge: `health-list` has at least one `health-item[data-check=overhangs]`. Clicking it changes `data-camera-target` and shows `finding-marker`.
48. E-48. Drop `cube.ptsm`, go to Print, click the "Printer:" button: `printer-menu` has at least two `menuitemradio`s. Choose one that is not checked: `status-printer` shows its name. ControlOrMeta+Z: the previous name.
49. E-49. Drop `oversize.ptsm`, go to Print: `health-badge` `data-state="error"` and a `health-item[data-check=bed_fit][data-severity=error]`.
50. E-50. Drop `overhang.ptsm`, go to Print, open "Checks", switch "Overhangs" off: no `health-item[data-check=overhangs]`, and `overhang-legend` is still visible. Close the popover and click "Overhang map": no `overhang-legend`. Reload, drop the file, go to Print: the switch is still off.
51. E-51. Drop `overhang.ptsm`, go to Print and wait for a report; click "Model", rename the body: `health-badge` `data-stale="true"`. Click it: `data-stale="false"` and `health-list` visible.
52. E-52. Without File System Access, drop `cube.ptsm`, go to Print, click "Export 3MF": a download `cube.3mf` starting with bytes `50 4b 03 04`, and a notification "Exported cube.3mf."
53. E-53. Open the export popover, choose STL, click "Export": a download `cube.stl` whose length is 84 plus 50 times the little-endian `u32` at offset 80, which is at least 12.
54. E-54. STEP: `cube.step` starting with `ISO-10303-21;`. OBJ: `cube.obj` containing a line starting `v ` and a line starting `f `.
55. E-55. Drop `cube.ptsm`, click "Hide Cube", go to Print, click "Export 3MF": the notification reads "There is no visible body with geometry to export." and no download starts within 2 s.

Themes:

56. E-56. Run "Theme: Dark": `html` has the class `dark`; reload: still dark. Run "Theme: Light": `html` has no `dark` class. With `page.emulateMedia({ colorScheme: 'dark' })`, run "Theme: System": `html` has the class `dark`.

Feature coverage (E-57, one test per row, parametrized). Fields are found by the labels in each feature's manifest table (`docs/spec/features/<type>.md`); selection fields are filled by the clicks listed, and a field that already holds the listed item (a starting value, 2.12) is left as is. Each test ends with Enter and expects a new timeline item with the feature's default label and `data-status="ok"`, plus the listed result.

| Type | Fixture and view | Steps | Result |
| --- | --- | --- | --- |
| `revolve` | `revolve-sketch.ptsm`, front fit | Run Revolve; click `(cx, cy)` (the region); activate the axis field; expand Origin and click "Z axis" (Extent starts at Full) | Body 1 `body-size` "40 × 40 × 10 mm" |
| `fillet` | `cube.ptsm`, front fit | E-31 | Cube "20 × 20 × 20 mm" |
| `chamfer` | `cube.ptsm`, front fit | Run Chamfer; click `(cx, cy - 0.4 H)`; distance `2` | Cube "20 × 20 × 20 mm" |
| `shell` | `cube.ptsm`, top fit | Run Shell; click `(cx, cy)` (top face); thickness `2` | Cube "20 × 20 × 20 mm" |
| `hole` | `hole-cube.ptsm`, top fit | Run Hole; click `(cx, cy)` (point `e-p1`); Diameter `5`; Extent Through all | Cube "20 × 20 × 20 mm"; hover `(cx, cy)` gives `none` |
| `box` | new document | Shapes, Box; sizes `10`, `10`, `10`; op New body | Body 1 "10 × 10 × 10 mm" |
| `cylinder` | new document | Shapes, Cylinder; radius `5`, height `10` | Body 1 "10 × 10 × 10 mm" |
| `sphere` | new document | Shapes, Sphere; radius `5` | Body 1 "10 × 10 × 10 mm" |
| `cone` | new document | Shapes, Cone; bottom radius `5`, top radius `0`, height `10` | Body 1 "10 × 10 × 10 mm" |
| `torus` | new document | Shapes, Torus; major radius `10`, minor radius `2` | Body 1 "24 × 24 × 4 mm" |
| `boolean` | `two-bodies.ptsm` | Run Boolean; target: click the row "Cube"; tools: click the row "Peg"; operation Cut | Cube "20 × 20 × 20 mm"; the Peg row reads "Peg (empty)" |
| `move_body` | `cube.ptsm` | Palette "Move body"; body: row "Cube"; translation X `10` | Cube `body-center` "10, 0, 10 mm" |
| `split_body` | `cube.ptsm` | Palette "Split body"; body: row "Cube"; plane: row "Front plane (XZ)" (Target body starts as a new body, 2.12) | Two body rows with geometry, each "20 × 10 × 20 mm" |
| `mirror` | `offset-cube.ptsm` | Run Mirror; body: row "Cube"; plane: row "Right plane (YZ)"; op Join | Cube "60 × 20 × 20 mm", center "0, 0, 10 mm" |
| `linear_pattern` | `offset-cube.ptsm` | Pattern, Linear pattern; body: row "Cube"; direction: row "X axis"; count `3`; spacing `30`; op Join | Cube "80 × 20 × 20 mm" |
| `circular_pattern` | `offset-cube.ptsm` | Pattern, Circular pattern; body: row "Cube"; axis: row "Z axis"; count `4`; angle `360 deg`; op Join | Cube "60 × 60 × 20 mm" |
| `datum_plane` | `cube.ptsm` | Palette "Datum plane"; base plane: row "Top plane (XY)"; offset `10` | A Datum planes row "Datum plane 1" |

`sketch` and `extrude` are covered by E-12 and E-13.

Fixes and renames:

58. E-58. Drop `fillet-error.ptsm`; open the problems panel and click "Edit" on "Fillet 1": `inspector-error` has a button whose name starts with "Use "; click it, wait for `data-preview="shown"`, press Enter: "Fillet 1" has `data-status="ok"`.
59. E-59. Drop `parametric.ptsm`; run "Parameters"; set "Name of size" to "side", Enter: every `timeline-item` has `data-status="ok"`; ControlOrMeta+Z restores the name "size".

Theme, styles and fonts:

60. E-60. Theme toggle: with `page.emulateMedia({ colorScheme: 'light' })`, `openApp`: `html` has no `dark` class and `theme-toggle` has `aria-pressed="false"`. Click it: `html` has the class `dark`, `aria-pressed="true"`, `localStorage.getItem('partsmith.settings.v1')` parses to an object with `theme` `dark`, and the computed `background-color` of `top-bar` differs from its value before the click. Reload: `html` has the class `dark` as soon as `app` is ready. Click it again: no `dark` class and the stored `theme` is `light`. Emulate `colorScheme: 'dark'` and reload: still no `dark` class (the explicit choice wins). Run "Theme: System": `html` has the class `dark`.
61. E-61. No inline styles except runtime positions: in each of these states, every element matching `[style]` has `data-positioned`, and its `style` attribute declares only `translate`, except the view cube rotor, which declares only `transform`: (a) right after `openApp`; (b) `rect-sketch.ptsm` dropped and "Sketch 1" double-clicked, with at least one `sketch-dimension` and one `constraint-glyph` visible, each with `data-positioned`; (c) `cube.ptsm` dropped, "Extrude 1" double-clicked (the `inspector` has `data-positioned`) and the Document menu open; (d) `overhang.ptsm` dropped, Print context with `overhang-legend` visible.
62. E-62. Fonts: drop `rect-sketch.ptsm` and double-click "Sketch 1": the computed `font-family` of `document-name`, of `status-units` and of `sketch-header` starts with `Inter`; that of the first `sketch-dimension` starts with `"JetBrains Mono"` (Chrome keeps the quotes). Finish the sketch, press E: that of `expression-result` starts with `"JetBrains Mono"` and that of the `inspector-field` label starts with `Inter`.

One interface for everyone (open issue 15):

63. E-63. Cold start: `empty-state` is visible, contains "Start with a sketch on a plane, or add a box." and "right-drag or drag the view cube", and holds buttons "Sketch" and "Box"; `timeline-empty` reads "Each step you add appears here. Double-click a step to change it later."; expanding "Origin" in the browser shows the button "Hide X axis". Click "Box" in `empty-state`: `empty-state` is hidden and the inspector "Box" is open. Press Enter: a `timeline-item` "Box 1", no `empty-state`, no `timeline-empty`, and the button "Show X axis". ControlOrMeta+Z: `empty-state` is visible.
64. E-64. Cold start, click "Sketch" in `empty-state`: `data-context="sketch"`, `sketch-header` `data-state="picking"`, no `empty-state`. Escape: `data-context="model"` and `empty-state` is visible.
65. E-65. Drop `cube.ptsm`, front fit. Right-drag 100 pixels on the canvas: `data-view="custom"`. Click "Front view"; Shift+right-drag 100 pixels: `data-view` stays `front` and `data-camera-target` changes. Right-click `(cx, cy)` without moving: `viewport-menu` is visible, `data-selection-count="1"` and `data-view="front"`; Escape closes it.
66. E-66. Cold start: the toolbar button "Extrude" has `aria-disabled="true"`. Click it: an info `notification` reads "Draw a closed shape in a sketch first." and no `inspector` opens. Click the canvas at `(cx, cy)` and press F: an info `notification` reads "Make a solid first, for example with Extrude or Box."
67. E-67. Drop `rect-sketch.ptsm`, double-click "Sketch 1", click "Finish sketch": `data-context="model"`, `selection-title` "1 region", and the inspector has a button "Extrude". Click it: the inspector "Extrude" whose profile `selection-field` has `data-count="1"`.
68. E-68. Drop `hole-cube.ptsm`, double-click "Sketch 2", press ControlOrMeta+Enter: `selection-title` "1 point" and a button "Hole". Press H: the inspector "Hole" whose points `selection-field` has `data-count="1"`; Enter: "Hole 1" has `data-status="ok"`.
69. E-69. Drop `cube.ptsm`, top fit, click `(cx, cy)`: `selection-title` "1 face" and a button "Hole". Click it: the inspector "Hole" whose points `selection-field` has `data-active="true"` and reads "Click a flat face or pick sketch points". Click `(cx, cy)`: `data-count="1"`. Press Enter: timeline items "Sketch 1", "Extrude 1", "Sketch 2", "Hole 1", all `data-status="ok"`; hover `(cx, cy)` gives `none` (Through all is the start value, 2.12). ControlOrMeta+Z: two timeline items.
70. E-70. Set the viewport to 1024 by 720. New sketch on "Top plane (XY)" (as E-12): the button "Finish sketch" shows the text "Finish sketch"; `sketch-hint` reads "Choose Line (L), Rectangle (R) or Circle (C) to start drawing.". Press L: "Click to start a line.". Click `(cx, cy)` and move to `(cx + 150, cy)`: the hint contains "type a length" and the `draw-input` named "Length" has a placeholder matching `^\d+(\.\d+)?$`. Press Escape three times: `sketch-hint` reads "To leave the sketch, click Finish sketch or press Ctrl+Enter." and `data-context` is still "sketch".
71. E-71. New sketch on "Top plane (XY)". Run "Parameters" from the palette, click "Add parameter", type "width", Enter, type "40 mm", Enter, and close the panel. Click the canvas at `(cx, cy + 100)`, press L, click `(cx, cy)`, move to `(cx + 100, cy)`, press Tab, type "width / 2", Enter: a `sketch-dimension` with text "fx 20".
72. E-72. Drop `cube.ptsm`, front fit, click the top front edge `(cx, cy - 0.4 H)`, press F: the radius input has the value "1 mm" and `data-preview` reaches `shown` with no `inspector-error`. Fill "2", Enter. Click the bottom front edge `(cx, cy + 0.4 H)`, press F: the radius input has the value "2".
73. E-73. Drop `cube.ptsm`, front fit, click the top front edge `(cx, cy - 0.4 H)`, press F: the edges field has `data-count="1"` and `data-active="true"`. Click the right front edge `(cx + 0.4 H, cy)`: `data-count="2"`. Press "2", then Enter: "Fillet 1" has `data-status="ok"`; double-click it: the radius input has the value "2".
74. E-74. Drop `cube.ptsm`, top fit, click "Shell" in the toolbar: the faces field has `data-active="true"`. Click `(cx, cy)`: `data-count="1"`. Enter: "Shell 1" has `data-status="ok"`.
75. E-75. Drop `offset-cube.ptsm`. Run Mirror: the body field reads "Cube" (its starting value) and the plane field has `data-active="true"`; click the row "Right plane (YZ)": there is no `inspector-field[data-path=target]`. Choose Operation "New body": the target field reads "Body 1 (new)". Enter: body rows "Cube" and "Body 1"; clicking "Body 1" shows `body-size` "20 × 20 × 20 mm" and `body-center` "-20, 0, 10 mm".
76. E-76. Drop `cube.ptsm`, top fit, click `(cx, cy)`, click the suggestion "Sketch" in the inspector: `data-context="sketch"`. Press C, click `(cx, cy)`, click `(cx + 40, cy)`, click "Finish sketch": `selection-title` "1 region". Press E and choose Operation "Cut": the button "Reverse" has `aria-pressed="true"`. Enter: "Extrude 2" has `data-status="ok"` (a cut pointing out of the cube would be a `no_effect` warning).
77. E-77. Drop `rect-sketch.ptsm`, top fit, press E, click `(cx, cy)`, fill Distance with "-5": `inspector-error` has a button "Reverse and use 5". Click it: the Distance input has the value "5", "Reverse" has `aria-pressed="true"`, and `data-preview` reaches `shown`.
78. E-78. Drop `parametric.ptsm`, double-click "Extrude 1", run "Parameters": `parameters-transaction` reads "Changes are part of Extrude 1 until you press OK.". Fill "Expression of size" with "30 mm", Enter: `data-preview` reaches `shown` and no `notification[data-severity=error]` appears. Click "Cancel" in the inspector footer: `parameter-value` reads "20 mm". Double-click "Extrude 1", set size to "30 mm" the same way and click "OK": `parameter-value` "30 mm"; ControlOrMeta+Z once: "20 mm".
79. E-79. Drop `cube.ptsm`, front fit, press F, click the top front edge `(cx, cy - 0.4 H)` and the right front edge `(cx + 0.4 H, cy)`: the edges field has `data-count="2"`. ControlOrMeta+Z: `data-count="1"` and the inspector is open; ControlOrMeta+Z: `data-count="0"`; ControlOrMeta+Z: the inspector is closed and the timeline still has two items.
80. E-80. Drop `cube.ptsm`, front fit, press F, click the top front edge, click "Chamfer" in the toolbar: an info notification "Fillet was canceled.". Click its "Restore": the inspector "Fillet" whose edges field has `data-count="1"`.
81. E-81. New sketch on "Top plane (XY)". Press L, click `(cx, cy)`, `(cx + 100, cy)` and `(cx + 100, cy - 80)`, press Escape twice: `sketch-header` has `data-outline="2"` and contains "2 open ends". Press ControlOrMeta+Enter: a `notification[data-severity=warning]` reads "Sketch 1 has 2 open ends, so some shapes cannot be extruded." with a button "Back to sketch".
82. E-82. Drop `underconstrained.ptsm`, double-click "Sketch 1": `sketch-header` contains "Not fully constrained · 7 DOF". Click it: `sketch-problems` contains "Free geometry: 1 line, 1 circle" and "you can extrude at any time". Click "Select them": `selection-title` "2 items".
83. E-83. Drop `cube-features.ptsm`, double-click "Fillet 1", fill the radius with "15", Enter: a `notification[data-severity=error]` is visible. ControlOrMeta+Z: within 2 s no `notification[data-severity=error]` remains.
84. E-84. Drop `cube.ptsm`. Hover the toolbar button "Shell" for 600 ms: a `[role=tooltip]` contains "Hollows out a body". Press E and hover the label "Operation" for 600 ms: a tooltip contains "Intersect keeps only the overlap". Escape. Press ControlOrMeta+K, type "shell": `palette-footer` contains "Hollows out a body".
85. E-85. Cold start, press ControlOrMeta+K: the first `palette-heading` reads "Create" and an option "Fillet" is listed. Type "zzz": `palette-empty` reads "Nothing matches zzz.".
86. E-86. Cold start, click "Sketch" in the toolbar: three `plane-label`s read "Top (XY)", "Front (XZ)" and "Right (YZ)"; `sketch-hint` reads "Top plane (XY) is the print bed."; click "Sketch on Front plane (XZ)": `data-view="front"`, a timeline item "Sketch 1" and header `data-state="empty"`.
87. E-87. Drop `rect-sketch.ptsm`, click the browser row "Sketch 1", choose the radio "Sketch": `data-context="sketch"`, header `data-state="full"` and one `timeline-item`.
88. E-88. Drop `cube.ptsm`, click the body row "Cube": the inspector has buttons "Move body" (its text includes "M"), "Mirror" and "Split body". Click "Move body": the inspector "Move body" whose body field has `data-count="1"`.
89. E-89. Drop `cube.ptsm`, front fit, click `(cx, cy)`: the inspector shows "Made by Extrude 1" and a button "Select Cube". Click "Edit Extrude 1": the inspector "Extrude 1" with `data-mode="command"`. Escape, click `(cx, cy)`, click "Select Cube": `selection-title` "Cube".
90. E-90. Drop `two-bodies.ptsm`, click the row "Cube", Shift+click the row "Peg": `selection-title` "2 bodies" and a button "Boolean". Click it: the target field and the tools field each have `data-count="1"`.
91. E-91. Drop `underconstrained.ptsm`, double-click "Sketch 1". With the mapping for content (0, -5) to (55, 5), click the line at (15, 0): `selection-title` "1 line" and `entity-size` labeled "Length" with the value "30". Fill "40", Enter: a `sketch-dimension` with text "40" and `data-dof="6"`. Click the circle at (55, 0): `entity-size` labeled "Diameter" with the value "10"; fill "12", Enter: a `sketch-dimension` with text "⌀12" and `data-dof="5"`.
92. E-92. Drop `cube.ptsm`, click the body row "Cube": `measure-row[data-measure=volume]` contains "Volume" and "8000 mm³".
93. E-93. Drop `cube.ptsm`, open "Shapes" and click "Box", click "More options", fill "Position X" with "40", choose Operation "New body", click "Fewer options", click "OK": "Box 1" has `data-status="ok"`. Double-click "Box 1": `inspector-more` reads "More options · Position X".
94. E-94. Drop `parametric.ptsm`, double-click "Extrude 1", fill Distance with "si": an `expression-suggestions` option reads "size = 20 mm". Press Tab: the input value is "size" and `expression-result` reads "= 20 mm". Fill "width": `field-error` contains "width" and a button "Add parameter width" is shown. Click it: `parameters-panel` has a draft row named "width" with the expression "20 mm", focused. Press Enter: a `parameter-row` for width exists, and the Distance input reads "width" with `expression-result` "= 20 mm".
95. E-95. Without File System Access, drop `fillet-error.ptsm`, click "Export" in the top bar: `export-status` contains "Fillet 1 failed, so it is not in the export." and "Print check: Not checked", with buttons "Show problems" and "Check print". Escape, choose the radio "Print", wait for `health-badge` `data-stale="false"`, click "Export 3MF": a `notification[data-severity=warning]` reads "Exported fillet-error.3mf. Fillet 1 failed, so it is not in the export." with a button "Show problems".
96. E-96. Cold start: `viewport` has a numeric `data-grid-spacing` and `status-grid` reads "Grid {that value} mm". Twenty wheel events of `deltaY: 100` at `(cx, cy)`: `data-grid-spacing` is larger. Click the radio "Print": `data-grid-spacing` is empty and `status-grid` is hidden.
97. E-97. Drop `cube.ptsm`, choose the radio "Print": the toolbar button reads "Printer: Generic FDM 0.4 mm (default)" and `status-printer` "Generic FDM 0.4 mm (default)"; open it: the item "Generic FDM 0.4 mm" contains "256 × 256 × 256 mm bed · 0.4 mm nozzle".
98. E-98. Drop `cube.ptsm`, front fit. Right-click the top front edge `(cx, cy - 0.4 H)`: `viewport-menu` has items "Fillet", "Chamfer" and "Edit Extrude 1". Choose "Chamfer", press Enter: "Chamfer 1" has `data-status="ok"`. Click the right front edge `(cx + 0.4 H, cy)` and press Enter: the inspector "Chamfer" whose edges field has `data-count="1"` and whose Distance input has the value "1 mm"; Enter: "Chamfer 2" has `data-status="ok"`.
99. E-99. Drop `cube.ptsm`, top fit, press F: the edges field reads "Select edges or a face". Click `(cx, cy)`: `data-count="4"`. Click again: `data-count="0"`. Click again and press Enter: "Fillet 1" has `data-status="ok"`.
100. E-100. Drop `cube.ptsm`, choose "Print" and wait for `health-badge` `data-stale="false"`. Click "Lay on face": `print-hint` reads "Pick the face that should touch the bed.". `fitView(page, 'Front')`, click `(cx, cy)`: "Lay on face" has `aria-pressed="true"` and `health-badge` reaches `data-stale="false"` again. Click "Export" in the top bar: `export-status` contains "Printing with the picked face of Cube on the bed.". Escape, click "Lay on face": `aria-pressed="false"`.
101. E-101. Press F1: `keys-panel` is visible and its "Orbit" row contains "Shift + middle drag". Click "FreeCAD" in it: `data-nav-preset="freecad"` and the "Orbit" row contains "Middle + left drag".
102. E-102. Drop `rect-sketch.ptsm`, double-click "Sketch 1", press Escape five times: `data-context` is still "sketch".
103. E-103. Drop `rect-sketch.ptsm`, double-click "Sketch 1"; with `s = sketchToScreen` for content (0, 0) to (40, 20), click the top line at `(cx, cy - 10 s)` and press H: an info `notification` reads "Horizontal is already implied by other constraints, so it was not added." and the number of `constraint-glyph[data-type=horizontal]` is unchanged.
104. E-104. Cold start, click "Sketch" in `empty-state`, then "Sketch on Top plane (XY)": `data-view="top"`. Press R, click `(cx, cy)`, click `(cx + 100, cy + 60)`, press ControlOrMeta+Enter: `data-view="iso"` and `selection-title` "1 region". Double-click the timeline item "Sketch 1", right-drag 100 pixels upward, click "Finish sketch": `data-view="custom"`.

### 6.6 Visual tests

Tagged `@visual`, using `expect(page).toHaveScreenshot` with `animations: 'disabled'` and `maxDiffPixelRatio: 0.002`. Themes are chosen with `page.emulateMedia({ colorScheme })` before `openApp` (settings stay `system`). Before each screenshot the test awaits `document.fonts.ready` and fails unless `document.fonts.check('14px Inter')` and `document.fonts.check('14px "JetBrains Mono"')` are true, so baselines never capture fallback fonts (the runner needs access to Google Fonts, open issue 10). Baselines are reviewed and approved by a human at the gate (technical design 16 and 17); a changed baseline needs reviewer approval.

1. V-1. Empty app, light theme. V-2. Empty app, dark theme.
2. V-3. `cube.ptsm` with "Home view", light. V-4. The same, dark.
3. V-5. `rect-sketch.ptsm` in sketch mode (fully constrained colors, dimensions), light. V-6. `underconstrained.ptsm` in sketch mode, dark.
4. V-7. `overhang.ptsm` in Print with the overhang map and legend, light.
5. V-8. `cube.ptsm` with the Extrude inspector open in edit mode, light. V-9. `cube.ptsm` with the front face selected and the top edge hovered, light.
6. V-10. `fillet-error.ptsm` with the Problems panel open and the inspector of "Fillet 1" showing its error banner, light. V-11. The same, dark.
7. V-12. `rect-sketch.ptsm` in Model context with the Extrude inspector open and its empty Profile field active (the region fills of 3.12 pass 7 shown), light.

V-1 to V-11 are re-approved after the empty state (3.2), the origin axes of an empty document (3.10), the ground grid (3.12) and the renamed chrome (3.3, 3.17) changed them.

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

1. **Dimension label positions.** Nothing in the drafts stores a dimension's label position. The UI computes `defaultLabelPosition` and uses a stored position if sketch.md adds one; dragging labels is later.
2. **Desktop host pieces.** Resolved by [desktop.md](desktop.md) and ADR 0014. The Tauri transport reads `window.__TAURI__` (requires `app.withGlobalTauri: true`, set in desktop.md 2.8), so `@tauri-apps/api` is not needed. The desktop `FileService` (`createDesktopFileService`, chosen by `chooseFileService`) calls host commands that show native dialogs through `tauri-plugin-dialog` from Rust and read or write only the files the user picked, so no fs plugin and no npm package are used (desktop.md 2.11 and 3.7). Because no desktop webview can produce a `FileSystemFileHandle`, the file types of 2.8 and 2.15 name `FileHandle` (`FileSystemFileHandle | DesktopFileHandle`, desktop.md 2.11). The superseding logic both transports share is protocol.md's `isSuperseded` (2.11).
3. **Session-only view state.** Sketch, datum and origin visibility and body locks are not saved, because the document has no fields for them. Persisting them is a format change (ADR and human review). The print orientation of Lay on face (3.17) is session state too; saving it with the document is a format change.
4. **When print analysis runs.** The technical design runs checks after every recompute. With one web worker, `analyze_print` would delay edits, so this spec runs it automatically only in the Print context and shows a stale badge elsewhere. Revisit with desktop background analysis (protocol.md "later").
5. **Check toggles.** `analyze_print` always runs every check, so the UI's switches only hide findings. An optional check list on the command would save work.
6. **Inference hold key on macOS.** This spec suppresses inference with Cmd on macOS (Fusion's convention) because Ctrl-click is a secondary click there.
7. **Dragging sketch curves.** `drag_sketch` pins one point, so M1 drags points only; translating lines and resizing circles by drag needs multi-point drags.
8. **Fixtures depend on unfinished JSON.** The e2e fixtures need sketch.md, print.md and the per-feature specs; they are written with `partsmith fmt` once those exist, and E-57's field labels come from those manifests.
9. **Expression error spans on the wire.** Resolved: protocol.md `ErrorInfo` carries an optional UTF-8 `span`, which the UI converts with `utf16Range` (2.13).
10. **Google Fonts dependency.** `src/index.css` loads Inter and JetBrains Mono from Google Fonts (ADR 0013). Offline (the desktop app without a network, air-gapped use, sandboxed CI) the font stacks fall back to system fonts, so the app looks different and the visual tests (6.6) cannot run; every launch sends the user's IP address to Google, a privacy question for a local CAD tool (and a GDPR question in the EU); the CSS `@import` adds a request chain before the fonts apply, which counts against the cold start budget (5); and the desktop content security policy must allow `fonts.googleapis.com` and `fonts.gstatic.com`. Self-hosting the variable fonts (for example the OFL-1.1 packages `@fontsource-variable/inter` and `@fontsource-variable/jetbrains-mono`) removes all four, but adds dependencies, a font license to the license policy and a change to `src/index.css`, so it needs human approval.
11. **Wide-gamut colors.** Many Tailwind 4 palette steps lie outside sRGB (2.16.5), the accent's `orange-500` and `orange-600` among them. The DOM renders their OKLCH values, so on Display P3 screens the interface's orange and red are more saturated than the same roles in the viewport, which draws the sRGB-clipped hex, and than stored body colors. Matching them needs a P3 WebGL drawing buffer (`drawingBufferColorSpace = 'display-p3'`) with P3 values in `roles.ts`, or sRGB arbitrary colors in the DOM, which the Tailwind guideline discourages.
12. **Default display color in io.md.** Resolved: io.md now uses the `body` role value `#9f9fa9`.
13. **Design-system values need human review.** The mapping of 2.16.3 to 2.16.5 is a design decision for the gate: the zinc steps of surfaces and text, `orange-600` and `orange-500` as the accent (light marks need `orange-600` to reach 3:1 on white), the yellow hazard-sign treatment of warnings, `red-700` and `red-500` for errors, the cool analysis ramp in place of a warm one, the default body color and the swatches. An orange body hides the orange hover and selection tints of its faces; its edges and the selection inspector still show the selection. The visual baselines V-1 to V-11 are the review material.
14. **Scope of the runtime style exception.** The guidelines name the floating inspector and dimension labels as runtime positions. This spec applies the same exception, only through `positionElement` and `orientElement`, to constraint glyphs, draw inputs, inference glyphs, axis labels, the timeline's drag ghost, menus, popovers and tooltips anchored to their triggers (Tailwind 4.3.3 has no anchor positioning utilities, and the Popover API's top layer ignores ancestor positioning) and the view cube's rotation, and uses SVG attributes for overlay marks and body color swatches. It also uses three kinds of arbitrary values: the two font families, the UI scale percentages and the inspector's `max-h-[calc(100%-2rem)]`. The maintainer should confirm this scope.
15. **One interface for everyone (decision).** Partsmith has no beginner mode, expert mode, simple view, skill setting, first-run flag or onboarding UI that changes which tools exist ([guidelines](../guidelines/README.md), "One interface for everyone"). Newcomers and experienced users see the same screens with the full power of the app, and approachability comes from how those screens present it: plain words, starting values that work, availability reasons before a click, hints in context that cost an experienced user nothing, and typed, keyboard and direct paths in the same surfaces. Disclosure is the same for everyone (More options, hover help, panels opened on request) and keeps power one obvious step away.
    - Added: the empty-document first step, origin axes and grid (3.2, 3.10, 3.12), which follow the document, not the user; right-drag orbit and pan in both presets (3.13); availability reasons for feature commands (2.10); the hand-off from a finished sketch to the next feature (3.3, 3.16.11) and the camera that leaves the sketch plane (3.16.11); holes placed by clicking a face, with a real sketch behind them (3.7); sketch step prompts and visible typed sizes (3.16.3, 3.16.4); starting values that work on printed parts and reuse of the document's last values (2.12); picks and typed digits that always land in the command (3.7); new bodies for copies and split parts (2.12); Reverse beside the distance and cuts that go into the part (2.12, 3.7); parameter edits during a command (3.7); undo inside a command and Restore (3.7); plain sketch status (3.16.8); failure notifications that follow the fix (4.2); descriptions and field help on hover (2.10, 2.12); the palette's other-app names, context ranking and browsable empty list (2.10, 3.4); view-named planes with one-click plane buttons and a Sketch radio that edits the selected sketch (3.3, 3.10, 3.16.1); next steps, a size field and measurements in the selection inspector (3.8); "More options" that shows what it hides (3.7); parameter name completion and one-click creation (2.13, 3.7); export status (3.18); one name per concept and the printer in use (3.17); the viewport context menu and Repeat (3.5, 3.13); faces for fillet and chamfer edges (3.7); Lay on face (3.17); the keyboard and mouse reference (3.5).
    - Left out on purpose: letters routed into values while drawing or during commands (they stay shortcuts, so S, V and the feature letters always work; digits and Tab cover typed entry); committing a command that ends implicitly (it would create features nobody asked for; Restore recovers the work instead); start values remembered in settings across documents (hidden state that breaks parameter names; reuse comes from the document); a separate "Coming from Fusion / FreeCAD" panel (the tagged keywords show the mapping in the palette everyone uses); a plane-pick popover, a dotted underline under every label with help and a hover tooltip on every numeric field (visual noise for little gain; the label tooltip, `cursor-help` and the keyboard and mouse panel carry the same help); clicking a sketch curve during plane picking to edit that sketch (a click near a visible sketch line would open the old sketch instead of starting a new one; the Sketch radio and double-click edit it); distances and angles between two selected items (they need the core's two-target `measure`, 1.2); renaming Boolean to "Combine" (the manifest label belongs to the core; "Combine (Fusion)" is a keyword); a toolbar width ladder; and placeholder rows in empty browser groups.
    - Later, outside this spec: on-canvas handles (1.2; they need a handle anchor per field from the core); dependency marks on timeline items and reorder drops that refuse to break a dependency (they need `uses` in `FeatureStatus`); face edges as sketch snap and dimension targets (sketch.md and engine changes); custom printer profiles (`put_printer_profile`, with a decision on documents that name a profile another machine lacks); one file per body and a mesh tolerance in export; a curated sample part opened from the empty state; trackpad two-finger pan and orbit.
    - Companion changes for human review at the next gate: features.md message wording (core messages that point to a "repair panel" or say "op" should say "Edit this feature and pick it again." and use the inspector's labels, with a test that no message contains "op " or "repair panel"), because the UI shows core messages verbatim; moving `FEATURE_DESCRIPTIONS`, `FIELD_HELP`, `START_VALUES` and the body-creating `target` rule into `FeatureManifest` and `FieldHint` for plugins, the CLI and Python (a schema change that needs an ADR), and marking `reverse` essential in the extrude, revolve and hole manifests so the inline Reverse rule of 2.12 needs no override; hole.md's Later entry "Placement on a face by click without a sketch", which the UI now provides with a helper sketch (3.7); and technical design 13's onboarding bullet, whose "Coming from Fusion / FreeCAD" panel is the palette's tagged keywords in M1.
